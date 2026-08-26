//! `openbim-bcf` — BIM Collaboration Format (BCF-XML).
//!
//! # What this is
//!
//! BCF is the open issue-exchange format of openBIM: a ZIP archive holding one
//! directory per topic, each with a markup document and optionally viewpoints
//! (camera plus component visibility) and snapshot images. It is how findings
//! from a model audit leave one toolchain and land in any BCF-aware reviewer.
//!
//! ```no_run
//! # fn main() -> Result<(), openbim_bcf::BcfError> {
//! let archive = openbim_bcf::read_path("issues.bcfzip")?;
//! for topic in archive.topics() {
//!     // Status is whatever the file says — see the tolerance section below.
//!     println!("{} [{}]", topic.title(), topic.status().unwrap_or("<unset>"));
//! }
//! # Ok(()) }
//! ```
//!
//! # BCF is two standards
//!
//! **BCF-XML** (buildingSMART S1005) is the file container this crate targets.
//! **BCF-API** (S1006) is a separate REST/JSON service specification for the
//! same domain. They share a data model and nothing else; conflating them is
//! why this crate is not simply named for the file extension. BCF-API is out
//! of scope here and, if implemented, gets its own crate.
//!
//! # 🚨 The reader is tolerant, and that is evidence-based
//!
//! Measured with [`scripts/measure-corpus.py`][measure] over 44 real
//! third-party archives (Schependomlaan plus four German audit projects) and
//! the 33 official buildingSMART BCF-XML v3.0 test archives:
//!
//! | The spec says | The corpus says |
//! | --- | --- |
//! | `bcf.version` declares the version | **21 of 44** third-party archives have none |
//! | `project.bcfp` describes the project | **0 of 44** third-party archives have one |
//! | `TopicStatus` comes from an agreed set | free text: `Open`, `OPEN`, `Offen`, `Active`, `ReOpened`, `In Progress` |
//! | `TopicType` comes from an agreed set | free text: `Error`, `ERROR`, `formale Prüfung`, `Sichprüfung`, `Clash` |
//!
//! A spec-strict reader rejects nearly every file in that corpus — files every
//! other BIM tool opens without complaint. So this crate rejects only what
//! cannot be interpreted at all, and keeps status/type strings **verbatim**
//! rather than mapping them onto an enum. `"Offen"` is not a parse failure; it
//! is what the file says, and normalising it would corrupt a round-trip.
//!
//! Everything the reader had to tolerate is reported as a
//! [`Tolerance`] diagnostic rather than being swallowed,
//! so a caller that *wants* strictness can enforce it with
//! [`BcfArchive::diagnostics`].
//!
//! # Version detection
//!
//! [`BcfVersion`] is resolved through [`openbim_core::Detected`], so a caller
//! can always tell a declared version from an inferred one, and a file whose
//! `bcf.version` disagrees with its actual shape yields
//! [`Detected::Conflict`][openbim_core::Detected::Conflict] instead of a
//! silent pick. This matters: 2.0 and 2.1 differ in where `TopicStatus` lives
//! and whether comments nest a back-reference `Topic` element, and guessing
//! wrong yields a *different* document rather than an error.
//!
//! # Status
//!
//! Implemented: archive scanning, version detection, and tolerant markup
//! reading (topics, comments, viewpoint references, header files, labels).
//! Not implemented: viewpoint (`.bcfv`) geometry, snapshots as decoded images,
//! project extensions, and **writing**. Read and write support are tracked
//! separately and must never be inferred from one another.
//!
//! [measure]: https://github.com/openbimrs/bcf/blob/main/scripts/measure-corpus.py

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod archive;
pub mod diagnostic;
mod error;
mod markup;
mod read;
mod version;
mod xml;

pub use archive::{BcfArchive, Limits};
pub use diagnostic::{Diagnostic, Tolerance};
pub use error::BcfError;
pub use markup::{Comment, HeaderFile, Markup, Topic, ViewPointRef};
pub use read::{
    read_path, read_path_with, read_reader, read_reader_with, read_slice, read_slice_with,
};
pub use version::BcfVersion;
