//! Errors that stop a BCF archive from being read at all.
//!
//! The dividing line: a [`BcfError`] means the caller gets **nothing**. Every
//! deviation that still leaves a usable document is a
//! [`Diagnostic`][crate::Diagnostic] instead. Given a corpus where 21 of 44
//! real archives lack `bcf.version` and none carry `project.bcfp`, putting
//! spec deviations in this type would make the crate useless.

use std::fmt;
use std::path::PathBuf;

/// A failure that prevents reading a BCF archive.
#[derive(Debug)]
#[non_exhaustive]
pub enum BcfError {
    /// The file could not be opened or read.
    Io {
        /// The path involved, when one is known.
        path: Option<PathBuf>,
        /// The underlying I/O failure.
        source: std::io::Error,
    },
    /// The bytes are not a readable ZIP container.
    NotAnArchive {
        /// What the ZIP reader reported.
        detail: String,
    },
    /// The archive contains no markup document, so there is no BCF content.
    ///
    /// Distinguished from an archive with unreadable topics: that yields
    /// diagnostics and whatever parsed, while this means the container held
    /// nothing recognisable as BCF at all.
    NoTopics,
    /// A declared limit in [`Limits`][crate::Limits] was exceeded.
    ///
    /// BCF archives arrive from untrusted parties, so decompression is bounded
    /// rather than trusted. This is refusal, not corruption.
    LimitExceeded {
        /// Which limit tripped.
        limit: &'static str,
        /// The bound that was set.
        allowed: u64,
        /// What the archive asked for, when it is known up front.
        requested: u64,
    },
    /// An archive entry escapes the extraction root, or is otherwise unsafe.
    ///
    /// Kept fatal on purpose: a traversal path is not a tolerable quirk, it is
    /// an attempt to write outside the archive.
    UnsafeEntry {
        /// The offending entry name, verbatim.
        name: String,
    },
}

impl fmt::Display for BcfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BcfError::Io { path: Some(p), .. } => {
                write!(f, "cannot read BCF archive {}", p.display())
            }
            BcfError::Io { path: None, .. } => f.write_str("cannot read BCF archive"),
            BcfError::NotAnArchive { detail } => {
                write!(f, "not a readable BCF (ZIP) archive: {detail}")
            }
            BcfError::NoTopics => f.write_str("archive contains no BCF markup document"),
            BcfError::LimitExceeded {
                limit,
                allowed,
                requested,
            } => write!(
                f,
                "BCF archive exceeds the {limit} limit ({requested} > {allowed})"
            ),
            BcfError::UnsafeEntry { name } => {
                write!(f, "BCF archive entry escapes the archive root: {name:?}")
            }
        }
    }
}

impl std::error::Error for BcfError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BcfError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for BcfError {
    fn from(source: std::io::Error) -> Self {
        BcfError::Io { path: None, source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn io_errors_keep_their_source_and_path() {
        let err = BcfError::Io {
            path: Some(PathBuf::from("/tmp/x.bcfzip")),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "missing"),
        };
        assert!(err.to_string().contains("/tmp/x.bcfzip"));
        assert!(err.source().is_some());
    }

    #[test]
    fn limit_messages_name_the_limit_and_both_numbers() {
        let msg = BcfError::LimitExceeded {
            limit: "uncompressed size",
            allowed: 10,
            requested: 99,
        }
        .to_string();
        assert!(msg.contains("uncompressed size"), "{msg}");
        assert!(msg.contains("99") && msg.contains("10"), "{msg}");
    }

    #[test]
    fn unsafe_entries_are_reported_verbatim() {
        let msg = BcfError::UnsafeEntry {
            name: "../../etc/passwd".into(),
        }
        .to_string();
        assert!(msg.contains("../../etc/passwd"), "{msg}");
    }

    #[test]
    fn no_topics_is_distinct_from_not_an_archive() {
        assert_ne!(
            BcfError::NoTopics.to_string(),
            BcfError::NotAnArchive { detail: "x".into() }.to_string()
        );
    }
}
