//! Entry points for reading a BCF archive.

use crate::archive::{BcfArchive, Limits};
use crate::error::BcfError;
use std::io::{Read, Seek};
use std::path::Path;

/// Read a BCF archive from a path, with default [`Limits`].
///
/// # Errors
///
/// Returns [`BcfError`] when the file cannot be opened, is not a ZIP, contains
/// no markup, exceeds a limit, or holds an entry that escapes the archive root.
/// Spec deviations that still leave a readable document are reported through
/// [`BcfArchive::diagnostics`] instead.
pub fn read_path(path: impl AsRef<Path>) -> Result<BcfArchive, BcfError> {
    read_path_with(path, Limits::default())
}

/// Read a BCF archive from a path with explicit limits.
///
/// # Errors
///
/// As [`read_path`].
pub fn read_path_with(path: impl AsRef<Path>, limits: Limits) -> Result<BcfArchive, BcfError> {
    let path = path.as_ref();
    let file = std::fs::File::open(path).map_err(|source| BcfError::Io {
        path: Some(path.to_path_buf()),
        source,
    })?;
    read_reader_with(file, limits).map_err(|e| match e {
        BcfError::Io { path: None, source } => BcfError::Io {
            path: Some(path.to_path_buf()),
            source,
        },
        other => other,
    })
}

/// Read a BCF archive from bytes already in memory.
///
/// # Errors
///
/// As [`read_path`].
pub fn read_slice(bytes: &[u8]) -> Result<BcfArchive, BcfError> {
    read_reader(std::io::Cursor::new(bytes))
}

/// Read a BCF archive from bytes already in memory, with explicit limits.
///
/// # Errors
///
/// As [`read_path`].
pub fn read_slice_with(bytes: &[u8], limits: Limits) -> Result<BcfArchive, BcfError> {
    read_reader_with(std::io::Cursor::new(bytes), limits)
}

/// Read a BCF archive from any seekable reader, with default [`Limits`].
///
/// # Errors
///
/// As [`read_path`].
pub fn read_reader<R: Read + Seek>(reader: R) -> Result<BcfArchive, BcfError> {
    read_reader_with(reader, Limits::default())
}

/// Read a BCF archive from any seekable reader with explicit limits.
///
/// # Errors
///
/// As [`read_path`].
pub fn read_reader_with<R: Read + Seek>(reader: R, limits: Limits) -> Result<BcfArchive, BcfError> {
    let zip = zip::ZipArchive::new(reader).map_err(|e| match e {
        zip::result::ZipError::Io(source) => BcfError::Io { path: None, source },
        other => BcfError::NotAnArchive {
            detail: other.to_string(),
        },
    })?;
    BcfArchive::from_zip(zip, limits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_zip_bytes_are_reported_as_not_an_archive() {
        let err = read_slice(b"this is plainly not a zip").unwrap_err();
        assert!(matches!(err, BcfError::NotAnArchive { .. }), "{err:?}");
    }

    #[test]
    fn a_missing_file_keeps_its_path_in_the_error() {
        let err = read_path("/nonexistent/definitely-not-here.bcfzip").unwrap_err();
        match err {
            BcfError::Io { path: Some(p), .. } => {
                assert!(p.to_string_lossy().contains("definitely-not-here"));
            }
            other => panic!("expected Io with path, got {other:?}"),
        }
    }
}
