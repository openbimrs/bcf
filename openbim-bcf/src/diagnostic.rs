//! Non-fatal deviations the reader had to tolerate.
//!
//! Every place the reader accepts something the specification does not
//! sanction is recorded here rather than silently absorbed. That keeps
//! tolerance auditable: a caller that needs strictness inspects
//! [`BcfArchive::diagnostics`][crate::BcfArchive::diagnostics] and applies its
//! own policy, instead of the crate choosing strictness for everybody and
//! rejecting most real files.

use std::fmt;

/// The specific deviation observed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Tolerance {
    /// No `bcf.version` entry; the version was inferred from document shape.
    ///
    /// Measured in 21 of 44 third-party archives.
    MissingVersionEntry,
    /// `bcf.version` exists but its `VersionId` is not a version this crate
    /// knows.
    UnknownVersionId {
        /// The value as written in the file.
        raw: String,
    },
    /// `bcf.version` declares one version while the markup has the shape of
    /// another. Never resolved silently.
    VersionConflict {
        /// What `bcf.version` claimed.
        declared: crate::BcfVersion,
        /// What the markup shape indicates.
        observed: crate::BcfVersion,
    },
    /// A markup document could not be parsed; its topic is absent from the
    /// result while the rest of the archive still reads.
    UnreadableMarkup {
        /// The XML parse failure, as text.
        detail: String,
    },
    /// A markup document parsed but contained no `Topic` element.
    MarkupWithoutTopic,
    /// A `Topic` carried no `Guid` attribute, which the schema requires.
    TopicWithoutGuid,
    /// A `Comment` carried no `Guid` attribute.
    CommentWithoutGuid,
    /// A `Topic` carried no `Title`, which the schema requires.
    TopicWithoutTitle,
    /// A viewpoint or snapshot named by markup is not present in the archive.
    ///
    /// The reference is kept: it is what the file says, and dropping it would
    /// lose information a caller may want to report.
    DanglingReference {
        /// The referenced entry name, verbatim.
        target: String,
    },
    /// An archive entry uses Windows path separators.
    ///
    /// Tolerated because some writers emit them and every reader in the field
    /// accepts them; recorded because it is not what the ZIP spec says.
    BackslashSeparator {
        /// The entry name, verbatim.
        name: String,
    },
    /// A date/time field is not parseable as the schema's `xs:dateTime`.
    ///
    /// The raw string is preserved in the model regardless.
    UnparseableDateTime {
        /// Which field.
        field: &'static str,
        /// The value as written.
        raw: String,
    },
}

/// A tolerated deviation together with where it was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// The archive entry the deviation was observed in, when attributable to
    /// one. `None` means it is a property of the archive as a whole.
    pub entry: Option<String>,
    /// What was tolerated.
    pub tolerance: Tolerance,
}

impl Diagnostic {
    /// A diagnostic attributed to a specific archive entry.
    #[must_use]
    pub fn in_entry(entry: impl Into<String>, tolerance: Tolerance) -> Self {
        Self {
            entry: Some(entry.into()),
            tolerance,
        }
    }

    /// A diagnostic about the archive as a whole.
    #[must_use]
    pub fn in_archive(tolerance: Tolerance) -> Self {
        Self {
            entry: None,
            tolerance,
        }
    }
}

impl fmt::Display for Tolerance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tolerance::MissingVersionEntry => {
                f.write_str("no bcf.version entry; version inferred from shape")
            }
            Tolerance::UnknownVersionId { raw } => {
                write!(f, "unrecognised bcf.version VersionId {raw:?}")
            }
            Tolerance::VersionConflict { declared, observed } => write!(
                f,
                "bcf.version declares {} but markup has {} shape",
                declared.version_id(),
                observed.version_id()
            ),
            Tolerance::UnreadableMarkup { detail } => {
                write!(f, "markup could not be parsed: {detail}")
            }
            Tolerance::MarkupWithoutTopic => {
                f.write_str("markup document contains no Topic element")
            }
            Tolerance::TopicWithoutGuid => f.write_str("Topic has no Guid attribute"),
            Tolerance::CommentWithoutGuid => f.write_str("Comment has no Guid attribute"),
            Tolerance::TopicWithoutTitle => f.write_str("Topic has no Title"),
            Tolerance::DanglingReference { target } => {
                write!(f, "referenced entry {target:?} is not in the archive")
            }
            Tolerance::BackslashSeparator { name } => {
                write!(f, "entry {name:?} uses backslash separators")
            }
            Tolerance::UnparseableDateTime { field, raw } => {
                write!(f, "{field} is not a valid dateTime: {raw:?}")
            }
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.entry {
            Some(entry) => write!(f, "{entry}: {}", self.tolerance),
            None => write!(f, "{}", self.tolerance),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BcfVersion;

    #[test]
    fn entry_scoped_diagnostics_name_their_entry() {
        let d = Diagnostic::in_entry("topic/markup.bcf", Tolerance::TopicWithoutGuid);
        assert_eq!(d.entry.as_deref(), Some("topic/markup.bcf"));
        assert!(d.to_string().starts_with("topic/markup.bcf: "), "{d}");
    }

    #[test]
    fn archive_scoped_diagnostics_have_no_entry_prefix() {
        let d = Diagnostic::in_archive(Tolerance::MissingVersionEntry);
        assert_eq!(d.entry, None);
        assert!(!d.to_string().contains(both_separator()), "{d}");
    }

    fn both_separator() -> &'static str {
        ".bcf: "
    }

    #[test]
    fn version_conflict_reports_both_sides() {
        let msg = Tolerance::VersionConflict {
            declared: BcfVersion::V2_1,
            observed: BcfVersion::V2_0,
        }
        .to_string();
        assert!(msg.contains("2.1") && msg.contains("2.0"), "{msg}");
    }

    #[test]
    fn unknown_version_ids_are_quoted_verbatim() {
        let msg = Tolerance::UnknownVersionId { raw: "4.2".into() }.to_string();
        assert!(msg.contains("\"4.2\""), "{msg}");
    }
}
