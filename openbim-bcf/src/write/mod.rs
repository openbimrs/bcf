//! Writing BCF-XML: strict, deterministic `.bcfzip` output for 2.1 and 3.0.
//!
//! The reader is tolerant because real files are not valid. This writer is
//! the opposite on purpose: it refuses anything the official schemas reject,
//! so a file it produces opens in every schema-validating tool. Nothing is
//! written until the whole [`Document`] has been checked.
//!
//! ```
//! use openbim_bcf::write::{self, Comment, Document, TargetVersion, Topic, Viewpoint};
//! use openbim_bcf::Component;
//!
//! let doc = Document {
//!     version: TargetVersion::V2_1,
//!     extensions: None,
//!     topics: vec![Topic {
//!         guid: "3f2504e0-4f89-41d3-9a0c-0305e82c3301".into(),
//!         title: "Duct clashes with beam".into(),
//!         topic_type: Some("Clash".into()),
//!         topic_status: Some("Open".into()),
//!         creation_date: "2026-09-26T10:00:00Z".into(),
//!         creation_author: "checker@example.com".into(),
//!         comments: vec![Comment {
//!             guid: "0b7c3c1e-9d0a-4d2b-8f55-1a2b3c4d5e6f".into(),
//!             date: "2026-09-26T10:00:00Z".into(),
//!             author: "checker@example.com".into(),
//!             comment: "Found by rule C-12.".into(),
//!             viewpoint: Some("7e1d2c3b-4a59-4687-9a1b-2c3d4e5f6a7b".into()),
//!         }],
//!         viewpoints: vec![Viewpoint {
//!             guid: "7e1d2c3b-4a59-4687-9a1b-2c3d4e5f6a7b".into(),
//!             selection: vec![Component::ifc("0fXw$sQh19ixbI4tZgfkXu")],
//!             camera: None,
//!         }],
//!         ..Topic::default()
//!     }],
//! };
//!
//! let bytes = write::to_vec(&doc)?;
//! let archive = openbim_bcf::read_slice(&bytes)?;
//! assert!(archive.diagnostics().is_empty());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Determinism
//!
//! The caller supplies every GUID and timestamp; the writer invents neither.
//! Entry order, element order, attribute order, and indentation are fixed.
//! Every ZIP entry gets a fixed 1980-01-01 timestamp and `0644` permissions.
//!
//! By default entries are *stored* uncompressed, because compressed bytes
//! depend on the deflate implementation's version: stored output is identical
//! across runs, machines, *and* dependency upgrades. [`Options`] can select
//! [`Compression::Deflated`] at an explicit level instead, which stays
//! deterministic for a given dependency tree but may change when the codec
//! does.
//!
//! # What is refused
//!
//! - GUIDs not matching the target version's `Guid` pattern (3.0 accepts
//!   lowercase hex only), and GUIDs used twice anywhere in the document.
//! - An `IfcGuid` that is not 22 characters of IFC base64 starting `0`–`3`.
//! - Dates outside the `xs:dateTime` lexical space.
//! - Blank text, text with leading or trailing whitespace (the reader trims,
//!   so it could not round-trip), and characters XML 1.0 cannot represent.
//! - A 3.0 topic without `TopicType` or `TopicStatus`, or a 3.0 viewpoint
//!   without a camera: the 3.0 schema requires both.
//! - A type, status, priority, or label missing from supplied
//!   [`Extensions`].
//! - A comment anchored to a viewpoint the topic does not have.
//!
//! # Scope
//!
//! Topics carry GUID, title, description, type, status, priority, labels,
//! creation author and date, comments, and viewpoints. A viewpoint carries a
//! component selection and optionally a camera. Header files, snapshots,
//! visibility, colouring, clipping planes, document references, and
//! `project.bcfp` are not written.

mod check;
mod emit;

use std::fmt;
use std::io::{Seek, Write};
use std::path::Path;

pub use crate::markup::Component;
use crate::version::BcfVersion;

/// The BCF-XML version a [`Document`] is written as.
///
/// 2.0 is readable but not writable: it is superseded, and its comment shape
/// (a mandatory back-reference `Topic`, `Status`, `VerbalStatus`) is not
/// worth generating anew.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TargetVersion {
    /// BCF-XML 2.1.
    V2_1,
    /// BCF-XML 3.0. Adds `extensions.xml` and requires a camera per viewpoint.
    V3_0,
}

impl From<TargetVersion> for BcfVersion {
    fn from(v: TargetVersion) -> Self {
        match v {
            TargetVersion::V2_1 => BcfVersion::V2_1,
            TargetVersion::V3_0 => BcfVersion::V3_0,
        }
    }
}

/// Everything written into one `.bcfzip`.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    /// The version to write.
    pub version: TargetVersion,
    /// The project's vocabulary.
    ///
    /// When supplied, every topic's type, status, priority, and labels must
    /// be listed in it. For 3.0 it is written as `extensions.xml`; when
    /// `None`, a 3.0 archive gets one listing exactly the values the topics
    /// use, in order of first use. For 2.1 it is a check only: 2.1 expresses
    /// the vocabulary as an `extensions.xsd`, which this crate does not write.
    pub extensions: Option<Extensions>,
    /// The topics, in the order they are written and read back.
    pub topics: Vec<Topic>,
}

/// The BCF 3.0 `extensions.xml` vocabulary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Extensions {
    /// Allowed `TopicType` values.
    pub topic_types: Vec<String>,
    /// Allowed `TopicStatus` values.
    pub topic_statuses: Vec<String>,
    /// Allowed `Priority` values.
    pub priorities: Vec<String>,
    /// Allowed labels.
    pub topic_labels: Vec<String>,
    /// Known users. Written, not checked: this crate writes no field the 3.0
    /// schema ties to it.
    pub users: Vec<String>,
    /// Allowed `BimSnippet` types. Written, not checked.
    pub snippet_types: Vec<String>,
    /// Allowed stages. Written, not checked.
    pub stages: Vec<String>,
}

/// One topic to write.
///
/// Type, status, and priority are free strings, exactly as the reader keeps
/// them: BCF defines their vocabulary per project, not per format.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Topic {
    /// The topic GUID; also names the topic's directory in the archive.
    pub guid: String,
    /// The title.
    pub title: String,
    /// Free-form description.
    pub description: Option<String>,
    /// `TopicType`. Required for 3.0.
    pub topic_type: Option<String>,
    /// `TopicStatus`. Required for 3.0.
    pub topic_status: Option<String>,
    /// Priority.
    pub priority: Option<String>,
    /// Labels, in order.
    pub labels: Vec<String>,
    /// Creation timestamp, an `xs:dateTime`.
    pub creation_date: String,
    /// Creation author.
    pub creation_author: String,
    /// Comments, in order.
    pub comments: Vec<Comment>,
    /// Viewpoints, in order.
    pub viewpoints: Vec<Viewpoint>,
}

/// One comment to write.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Comment {
    /// The comment GUID.
    pub guid: String,
    /// Timestamp, an `xs:dateTime`.
    pub date: String,
    /// Author.
    pub author: String,
    /// The comment text.
    pub comment: String,
    /// GUID of a viewpoint of the same topic this comment is anchored to.
    pub viewpoint: Option<String>,
}

/// One viewpoint to write, as `Viewpoint_<guid>.bcfv` in the topic directory.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Viewpoint {
    /// The viewpoint GUID.
    pub guid: String,
    /// Components to highlight. Each needs an `IfcGuid` or an
    /// `AuthoringToolId`, or it identifies nothing.
    pub selection: Vec<Component>,
    /// The camera. Optional in 2.1, required in 3.0.
    pub camera: Option<Camera>,
}

/// A viewpoint camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Perspective or orthogonal, with the projection's own parameter.
    pub projection: Projection,
    /// Camera location.
    pub view_point: Vector3,
    /// Viewing direction. Must not be the zero vector.
    pub direction: Vector3,
    /// Up direction. Must not be the zero vector.
    pub up_vector: Vector3,
    /// Width over height of the view. Required in 3.0; 2.1 has no such
    /// element, so supplying one for 2.1 is refused rather than dropped.
    pub aspect_ratio: Option<f64>,
}

/// How a [`Camera`] projects.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection {
    /// A perspective camera.
    Perspective {
        /// Vertical field of view in degrees. 2.1 allows 45–60 inclusive,
        /// 3.0 anything strictly between 0 and 180.
        field_of_view: f64,
    },
    /// An orthogonal camera.
    Orthogonal {
        /// Visible size of the view in metres. Must be positive.
        view_to_world_scale: f64,
    },
}

/// A point or direction in model coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vector3 {
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
    /// Z.
    pub z: f64,
}

impl Vector3 {
    /// A vector from its components.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

/// Why a document could not be written.
#[derive(Debug)]
#[non_exhaustive]
pub enum WriteError {
    /// A value violates the target version's schema. Nothing was written.
    Invalid {
        /// Where, as a path into the [`Document`], e.g.
        /// `topics[0].comments[1].date`.
        at: String,
        /// What is wrong with it.
        problem: Invalid,
    },
    /// Writing to the destination failed.
    Io(std::io::Error),
    /// The ZIP encoder failed for a reason other than I/O.
    Zip {
        /// What the encoder reported.
        detail: String,
    },
}

/// What is wrong with a value in a [`Document`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Invalid {
    /// Not a GUID in the target version's `Guid` pattern.
    Guid {
        /// The value as given.
        value: String,
    },
    /// Not 22 characters of IFC base64 starting `0`–`3`.
    IfcGuid {
        /// The value as given.
        value: String,
    },
    /// Not in the `xs:dateTime` lexical space.
    DateTime {
        /// The value as given.
        value: String,
    },
    /// Empty or whitespace only.
    Blank,
    /// Leading or trailing whitespace, which the reader would trim away.
    SurroundingWhitespace {
        /// The value as given.
        value: String,
    },
    /// A character XML 1.0 cannot represent, such as most C0 controls.
    ForbiddenCharacter {
        /// The offending character.
        ch: char,
    },
    /// A value this target version requires is absent.
    Missing,
    /// A value this target version cannot represent.
    NotInVersion {
        /// The version that lacks it.
        version: TargetVersion,
    },
    /// A value absent from the supplied [`Extensions`].
    NotInExtensions {
        /// The value as given.
        value: String,
    },
    /// A GUID already used elsewhere in the document (compared
    /// case-insensitively, as GUIDs are).
    DuplicateGuid {
        /// The value as given.
        value: String,
    },
    /// A comment anchored to a viewpoint GUID the topic does not have.
    UnknownViewpoint {
        /// The referenced GUID.
        guid: String,
    },
    /// A component with neither `IfcGuid` nor `AuthoringToolId`.
    UnidentifiedComponent,
    /// A number outside its allowed range.
    Number {
        /// The value as given.
        value: f64,
        /// The allowed range, in words.
        expected: &'static str,
    },
    /// A document without topics: its archive would hold no markup and read
    /// back as [`BcfError::NoTopics`][crate::BcfError::NoTopics].
    NoTopics,
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Invalid { at, problem } => write!(f, "cannot write BCF: {at}: {problem}"),
            WriteError::Io(_) => f.write_str("cannot write BCF archive"),
            WriteError::Zip { detail } => write!(f, "cannot encode BCF archive: {detail}"),
        }
    }
}

impl fmt::Display for Invalid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Invalid::Guid { value } => write!(f, "{value:?} is not a valid GUID"),
            Invalid::IfcGuid { value } => write!(f, "{value:?} is not a valid IFC GlobalId"),
            Invalid::DateTime { value } => write!(f, "{value:?} is not an xs:dateTime"),
            Invalid::Blank => f.write_str("value is blank"),
            Invalid::SurroundingWhitespace { value } => {
                write!(f, "{value:?} has leading or trailing whitespace")
            }
            Invalid::ForbiddenCharacter { ch } => {
                write!(f, "character {ch:?} cannot be represented in XML 1.0")
            }
            Invalid::Missing => f.write_str("required by the target version but absent"),
            Invalid::NotInVersion { version } => write!(
                f,
                "not representable in BCF {}",
                BcfVersion::from(*version).version_id()
            ),
            Invalid::NotInExtensions { value } => {
                write!(f, "{value:?} is not listed in the supplied extensions")
            }
            Invalid::DuplicateGuid { value } => write!(f, "GUID {value:?} is used more than once"),
            Invalid::UnknownViewpoint { guid } => {
                write!(f, "no viewpoint {guid:?} in this topic")
            }
            Invalid::UnidentifiedComponent => {
                f.write_str("component has neither IfcGuid nor AuthoringToolId")
            }
            Invalid::Number { value, expected } => write!(f, "{value} is not {expected}"),
            Invalid::NoTopics => f.write_str("a BCF archive needs at least one topic"),
        }
    }
}

impl std::error::Error for WriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WriteError::Io(source) => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for WriteError {
    fn from(source: std::io::Error) -> Self {
        WriteError::Io(source)
    }
}

/// How the ZIP container is packed. Nothing about the BCF content changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Options {
    /// How entries are compressed. Defaults to [`Compression::Stored`].
    pub compression: Compression,
}

impl Options {
    /// These options with `compression` instead.
    #[must_use]
    pub const fn compression(mut self, compression: Compression) -> Self {
        self.compression = compression;
        self
    }
}

/// How archive entries are compressed.
///
/// Both choices are deterministic for a given build: the same input and the
/// same `openbim-bcf` dependency tree give the same bytes. Only
/// [`Stored`][Compression::Stored] also keeps the bytes stable across
/// dependency upgrades, because deflate output depends on the codec's version.
/// Choose it when archives are diffed or cached by content hash; choose
/// [`Deflated`][Compression::Deflated] when size matters more.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Compression {
    /// No compression. Byte-identical across codec versions.
    #[default]
    Stored,
    /// Deflate at an explicit level, `1` (fastest) to `9` (smallest). There
    /// is deliberately no "default level": a codec's default may change, and
    /// the level is part of what makes output reproducible.
    Deflated {
        /// `1..=9`.
        level: u8,
    },
}

/// Write a document into memory, with default [`Options`].
///
/// # Errors
///
/// [`WriteError::Invalid`] for the first value the target version's schema
/// would reject.
pub fn to_vec(doc: &Document) -> Result<Vec<u8>, WriteError> {
    to_vec_with(doc, Options::default())
}

/// Write a document into memory with explicit [`Options`].
///
/// # Errors
///
/// As [`to_vec`], plus [`WriteError::Invalid`] for an out-of-range option.
pub fn to_vec_with(doc: &Document, options: Options) -> Result<Vec<u8>, WriteError> {
    let cursor = to_writer_with(doc, std::io::Cursor::new(Vec::new()), options)?;
    Ok(cursor.into_inner())
}

/// Write a document to a seekable sink with default [`Options`], returning
/// the sink.
///
/// The document is validated in full before the first byte reaches `sink`.
///
/// # Errors
///
/// As [`to_vec`], plus [`WriteError::Io`] when the sink fails.
pub fn to_writer<W: Write + Seek>(doc: &Document, sink: W) -> Result<W, WriteError> {
    to_writer_with(doc, sink, Options::default())
}

/// Write a document to a seekable sink with explicit [`Options`].
///
/// # Errors
///
/// As [`to_vec_with`], plus [`WriteError::Io`] when the sink fails.
pub fn to_writer_with<W: Write + Seek>(
    doc: &Document,
    sink: W,
    options: Options,
) -> Result<W, WriteError> {
    emit::check_options(options)?;
    let entries = emit::entries(doc)?;
    emit::zip(&entries, sink, options)
}

/// Write a document to a file with default [`Options`], replacing the file
/// if it exists.
///
/// An invalid document leaves the filesystem untouched.
///
/// # Errors
///
/// As [`to_writer`].
pub fn to_path(doc: &Document, path: impl AsRef<Path>) -> Result<(), WriteError> {
    to_path_with(doc, path, Options::default())
}

/// Write a document to a file with explicit [`Options`].
///
/// # Errors
///
/// As [`to_writer_with`].
pub fn to_path_with(
    doc: &Document,
    path: impl AsRef<Path>,
    options: Options,
) -> Result<(), WriteError> {
    emit::check_options(options)?;
    let entries = emit::entries(doc)?;
    let file = std::fs::File::create(path)?;
    let mut file = emit::zip(&entries, std::io::BufWriter::new(file), options)?;
    file.flush()?;
    Ok(())
}
