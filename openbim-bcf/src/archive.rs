//! The archive: bounded ZIP access, entry classification, and the read result.

use crate::diagnostic::{Diagnostic, Tolerance};
use crate::error::BcfError;
use crate::markup::{self, Markup};
use crate::version::BcfVersion;
use crate::xml;
use openbim_core::Detected;
use std::io::{Read, Seek};

/// Bounds applied while reading an archive.
///
/// BCF files arrive from third parties, so decompression is bounded rather
/// than trusted. A 5 MB archive of 44 real files expands to well under the
/// defaults; a zip bomb does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum total uncompressed bytes read from the archive.
    pub max_total_uncompressed: u64,
    /// Maximum uncompressed bytes for a single XML document.
    pub max_entry_uncompressed: u64,
    /// Maximum number of entries examined.
    pub max_entries: u64,
}

impl Default for Limits {
    /// 512 MiB total, 32 MiB per document, 100 000 entries.
    fn default() -> Self {
        Self {
            max_total_uncompressed: 512 * 1024 * 1024,
            max_entry_uncompressed: 32 * 1024 * 1024,
            max_entries: 100_000,
        }
    }
}

/// A BCF archive that has been read.
#[derive(Debug, Clone)]
pub struct BcfArchive {
    version: Detected<BcfVersion>,
    markups: Vec<Markup>,
    entries: Vec<String>,
    diagnostics: Vec<Diagnostic>,
}

impl BcfArchive {
    /// The container version, with the evidence that established it.
    #[must_use]
    pub fn version(&self) -> &Detected<BcfVersion> {
        &self.version
    }

    /// Every topic that could be read, in archive order.
    pub fn topics(&self) -> impl Iterator<Item = &Markup> {
        self.markups.iter()
    }

    /// Number of topics read.
    #[must_use]
    pub fn topic_count(&self) -> usize {
        self.markups.len()
    }

    /// Every entry name in the archive, normalised to forward slashes.
    #[must_use]
    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    /// Everything the reader tolerated. Empty means the archive was
    /// spec-clean for every field this crate reads.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub(crate) fn from_zip<R: Read + Seek>(
        mut zip: zip::ZipArchive<R>,
        limits: Limits,
    ) -> Result<Self, BcfError> {
        let mut diagnostics = Vec::new();
        let scan = scan_entries(&mut zip, limits, &mut diagnostics)?;
        let declared = read_declared_version(&mut zip, &scan, limits, &mut diagnostics);
        let (markups, observed) = read_markups(&mut zip, &scan, limits, &mut diagnostics);

        if markups.is_empty() {
            return Err(BcfError::NoTopics);
        }

        let version = BcfVersion::reconcile(declared, observed);
        if let Detected::Conflict { declared, observed } = version {
            diagnostics.push(Diagnostic::in_archive(Tolerance::VersionConflict {
                declared,
                observed,
            }));
        }

        diagnostics.extend(dangling_references(&markups, &scan.entries));

        Ok(Self {
            version,
            markups,
            entries: scan.entries,
            diagnostics,
        })
    }
}

/// Entry names and the indices of the documents worth opening.
struct Scan {
    entries: Vec<String>,
    /// `(zip index, normalised name)` for each markup document.
    markups: Vec<(usize, String)>,
    /// `(zip index, normalised name)` of `bcf.version`, when present.
    version: Option<(usize, String)>,
}

/// Walk the central directory once: normalise names, enforce safety and size
/// limits, and classify entries. Nothing is decompressed here.
fn scan_entries<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    limits: Limits,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Scan, BcfError> {
    // `zip.len()` comes from the attacker-controlled central directory, so it
    // is bounded rather than trusted — and widened, never truncated.
    let count = zip.len() as u64;
    if count > limits.max_entries {
        return Err(BcfError::LimitExceeded {
            limit: "entry count",
            allowed: limits.max_entries,
            requested: count,
        });
    }

    let mut scan = Scan {
        entries: Vec::with_capacity(zip.len()),
        markups: Vec::new(),
        version: None,
    };
    let mut declared_total: u64 = 0;

    for i in 0..zip.len() {
        // `by_index_raw` reads the header without decompressing the payload.
        let file = zip.by_index_raw(i).map_err(zip_err)?;
        let raw = file.name().to_string();
        if raw.contains('\\') {
            diagnostics.push(Diagnostic::in_archive(Tolerance::BackslashSeparator {
                name: raw.clone(),
            }));
        }
        // `enclosed_name()` is `None` exactly for absolute paths, drive
        // prefixes, and `..` traversal — all fatal, never tolerated.
        if file.enclosed_name().is_none() {
            return Err(BcfError::UnsafeEntry { name: raw });
        }
        let size = file.size();
        drop(file);

        let name = raw.replace('\\', "/");
        declared_total = declared_total.saturating_add(size);
        if declared_total > limits.max_total_uncompressed {
            return Err(BcfError::LimitExceeded {
                limit: "uncompressed size",
                allowed: limits.max_total_uncompressed,
                requested: declared_total,
            });
        }

        // Extensions are matched case-insensitively: writers in the corpus emit
        // `.BCF` and `.BCFZIP` as readily as lowercase.
        let lower = name.to_ascii_lowercase();
        // Clippy's case-sensitive-extension lint is a false positive here:
        // `lower` is already ASCII-lowercased on the line above.
        #[allow(clippy::case_sensitive_file_extension_comparisons)]
        if lower.ends_with(".bcf") {
            scan.markups.push((i, name.clone()));
        } else if lower.ends_with("bcf.version") {
            scan.version = Some((i, name.clone()));
        }
        scan.entries.push(name);
    }

    Ok(scan)
}

/// The version `bcf.version` declares, if it declares a version this crate
/// knows. Anything else is a tolerance, not a failure.
fn read_declared_version<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    scan: &Scan,
    limits: Limits,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<BcfVersion> {
    let Some((index, name)) = &scan.version else {
        diagnostics.push(Diagnostic::in_archive(Tolerance::MissingVersionEntry));
        return None;
    };

    let parsed = read_entry(zip, *index, limits)
        .ok()
        .and_then(|bytes| xml::parse(&bytes).ok());
    let Some(node) = parsed else {
        diagnostics.push(Diagnostic::in_archive(Tolerance::MissingVersionEntry));
        return None;
    };

    let raw = node.attr("VersionId").unwrap_or_default().to_string();
    if let Some(v) = BcfVersion::from_version_id(&raw) {
        Some(v)
    } else {
        diagnostics.push(Diagnostic::in_entry(
            name.clone(),
            Tolerance::UnknownVersionId { raw },
        ));
        None
    }
}

/// Read every markup document, and the oldest version any of them looks like.
///
/// Oldest wins: 2.0-only shapes are decisive evidence, and a mixed archive is
/// only readable if interpreted by its oldest member.
fn read_markups<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    scan: &Scan,
    limits: Limits,
    diagnostics: &mut Vec<Diagnostic>,
) -> (Vec<Markup>, Option<BcfVersion>) {
    let mut markups = Vec::new();
    let mut observed: Option<BcfVersion> = None;

    for (index, name) in &scan.markups {
        let bytes = match read_entry(zip, *index, limits) {
            Ok(b) => b,
            Err(e) => {
                diagnostics.push(Diagnostic::in_entry(
                    name.clone(),
                    Tolerance::UnreadableMarkup {
                        detail: e.to_string(),
                    },
                ));
                continue;
            }
        };
        let tree = match xml::parse(&bytes) {
            Ok(t) => t,
            Err(detail) => {
                diagnostics.push(Diagnostic::in_entry(
                    name.clone(),
                    Tolerance::UnreadableMarkup { detail },
                ));
                continue;
            }
        };
        if let Some(v) = markup::observe_version(&tree) {
            observed = Some(observed.map_or(v, |prev| prev.min(v)));
        }
        if let Some(m) = markup::interpret(name, &tree, diagnostics) {
            markups.push(m);
        }
    }

    (markups, observed)
}

/// Viewpoint and snapshot references naming an entry the archive does not
/// contain. Reported, never dropped: the reference is what the file says.
fn dangling_references(markups: &[Markup], entries: &[String]) -> Vec<Diagnostic> {
    let known: std::collections::HashSet<&str> = entries.iter().map(String::as_str).collect();
    let mut out = Vec::new();

    for m in markups {
        let dir = m.entry.rsplit_once('/').map_or("", |(d, _)| d);
        for vp in &m.viewpoints {
            for target in [vp.viewpoint.as_deref(), vp.snapshot.as_deref()]
                .into_iter()
                .flatten()
            {
                let joined = if dir.is_empty() {
                    target.to_string()
                } else {
                    format!("{dir}/{target}")
                };
                if !known.contains(joined.as_str()) && !known.contains(target) {
                    out.push(Diagnostic::in_entry(
                        m.entry.clone(),
                        Tolerance::DanglingReference { target: joined },
                    ));
                }
            }
        }
    }
    out
}

fn read_entry<R: Read + Seek>(
    zip: &mut zip::ZipArchive<R>,
    index: usize,
    limits: Limits,
) -> Result<Vec<u8>, BcfError> {
    let mut file = zip.by_index(index).map_err(zip_err)?;
    if file.size() > limits.max_entry_uncompressed {
        return Err(BcfError::LimitExceeded {
            limit: "entry size",
            allowed: limits.max_entry_uncompressed,
            requested: file.size(),
        });
    }
    // Read through a hard cap rather than trusting the declared size: the
    // central directory is attacker-controlled and can understate the payload.
    let cap = limits.max_entry_uncompressed;
    let mut buf = Vec::with_capacity(file.size().min(1 << 20) as usize);
    let read = (&mut file).take(cap + 1).read_to_end(&mut buf)?;
    if read as u64 > cap {
        return Err(BcfError::LimitExceeded {
            limit: "entry size",
            allowed: cap,
            requested: read as u64,
        });
    }
    Ok(buf)
}

fn zip_err(e: zip::result::ZipError) -> BcfError {
    match e {
        zip::result::ZipError::Io(source) => BcfError::Io { path: None, source },
        other => BcfError::NotAnArchive {
            detail: other.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_are_bounded_and_ordered() {
        let l = Limits::default();
        assert!(l.max_entry_uncompressed < l.max_total_uncompressed);
        assert!(l.max_entries > 0);
    }
}
