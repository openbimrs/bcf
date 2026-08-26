//! Conformance against real BCF archives.
//!
//! Neither corpus can be vendored: the buildingSMART test cases are CC BY-ND
//! and the third-party archives are other people's project data. So these
//! tests are **opt-in by environment variable** and skip cleanly when the
//! corpora are absent, rather than being deleted for CI's convenience.
//!
//! ```bash
//! BCF_OFFICIAL_CORPUS="$PWD/references/test-cases" \
//! BCF_FIELD_CORPUS=/path/to/real/projects \
//!   cargo test -p openbim-bcf --test corpus -- --nocapture
//! ```
//!
//! `scripts/fetch-official-references.py` populates the official corpus.

use openbim_bcf::{BcfError, Tolerance};
use openbim_core::Detected;
use std::path::{Path, PathBuf};

fn archives_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if ext == "bcf" || ext == "bcfzip" {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

struct Summary {
    archives: usize,
    read: usize,
    topics: usize,
    comments: usize,
    /// Files that are not ZIPs at all — the official corpus ships unzipped
    /// `markup.bcf` documents alongside the packaged archives.
    not_archives: usize,
    failures: Vec<(PathBuf, String)>,
}

fn sweep(root: &Path) -> Summary {
    let mut s = Summary {
        archives: 0,
        read: 0,
        topics: 0,
        comments: 0,
        not_archives: 0,
        failures: Vec::new(),
    };
    for path in archives_under(root) {
        s.archives += 1;
        match openbim_bcf::read_path(&path) {
            Ok(archive) => {
                s.read += 1;
                s.topics += archive.topic_count();
                s.comments += archive.topics().map(|t| t.comments.len()).sum::<usize>();
                for topic in archive.topics() {
                    // The contract that matters: nothing is silently invented.
                    if let Some(status) = topic.status() {
                        assert!(
                            !status.is_empty(),
                            "{}: empty status surfaced",
                            path.display()
                        );
                    }
                    for c in &topic.comments {
                        assert!(
                            c.comment.is_some() || c.guid.is_some(),
                            "{}: comment with neither text nor guid",
                            path.display()
                        );
                    }
                }
            }
            // A loose markup.bcf is a bare XML document, not an archive.
            Err(BcfError::NotAnArchive { .. }) => s.not_archives += 1,
            Err(e) => s.failures.push((path, e.to_string())),
        }
    }
    s
}

fn corpus_root(var: &str) -> Option<PathBuf> {
    let raw = std::env::var_os(var)?;
    let path = PathBuf::from(raw);
    path.is_dir().then_some(path)
}

#[test]
fn official_buildingsmart_test_cases_read_without_fatal_errors() {
    let Some(root) = corpus_root("BCF_OFFICIAL_CORPUS") else {
        eprintln!("skipped: BCF_OFFICIAL_CORPUS unset or not a directory");
        return;
    };
    let s = sweep(&root);
    println!(
        "official: {} files, {} archives read, {} loose xml, {} topics, {} comments",
        s.archives, s.read, s.not_archives, s.topics, s.comments
    );
    assert!(s.archives > 0, "corpus at {} is empty", root.display());
    assert!(
        s.failures.is_empty(),
        "official archives failed to read: {:?}",
        s.failures
    );
    assert!(s.read > 0 && s.topics >= s.read);
}

#[test]
fn field_archives_read_without_fatal_errors() {
    let Some(root) = corpus_root("BCF_FIELD_CORPUS") else {
        eprintln!("skipped: BCF_FIELD_CORPUS unset or not a directory");
        return;
    };
    let s = sweep(&root);
    println!(
        "field: {} files, {} archives read, {} topics, {} comments",
        s.archives, s.read, s.topics, s.comments
    );
    assert!(s.archives > 0, "corpus at {} is empty", root.display());
    assert!(
        s.failures.is_empty(),
        "field archives failed to read: {:?}",
        s.failures
    );
}

/// The tolerance policy exists because real files need it. If a sweep of the
/// field corpus produced *no* diagnostics, either the corpus changed or the
/// reader stopped reporting — both worth failing on.
#[test]
fn field_archives_exercise_the_tolerance_path() {
    let Some(root) = corpus_root("BCF_FIELD_CORPUS") else {
        eprintln!("skipped: BCF_FIELD_CORPUS unset or not a directory");
        return;
    };
    let mut missing_version = 0usize;
    let mut any_diagnostics = 0usize;
    let mut seen = 0usize;
    for path in archives_under(&root) {
        let Ok(archive) = openbim_bcf::read_path(&path) else {
            continue;
        };
        seen += 1;
        if !archive.diagnostics().is_empty() {
            any_diagnostics += 1;
        }
        if archive
            .diagnostics()
            .iter()
            .any(|d| d.tolerance == Tolerance::MissingVersionEntry)
        {
            missing_version += 1;
        }
    }
    println!(
        "field: {seen} read, {any_diagnostics} with diagnostics, {missing_version} without bcf.version"
    );
    assert!(seen > 0);
    assert!(
        missing_version > 0,
        "expected archives lacking bcf.version; none found — has the corpus changed?"
    );
}

/// Version detection must not contradict buildingSMART's own files.
///
/// Every official test archive declares a `bcf.version` and is, by
/// construction, a valid document of that version. So a `Conflict` here is
/// always a defect in *this* reader, never in the corpus — which makes this
/// the sharpest available oracle for the detection heuristics.
///
/// It caught two real defects: treating `Markup/Viewpoints` as a 3.0 marker
/// (2.x uses that element as the viewpoint itself), and treating attribute
/// `TopicStatus` as 2.1-only (it is an attribute in 2.0 too).
#[test]
fn official_archives_never_contradict_their_declared_version() {
    let Some(root) = corpus_root("BCF_OFFICIAL_CORPUS") else {
        eprintln!("skipped: BCF_OFFICIAL_CORPUS unset or not a directory");
        return;
    };
    let mut declared = 0usize;
    let mut conflicts = Vec::new();
    for path in archives_under(&root) {
        let Ok(archive) = openbim_bcf::read_path(&path) else {
            continue;
        };
        match archive.version() {
            Detected::Declared(_) => declared += 1,
            Detected::Conflict { declared, observed } => conflicts.push(format!(
                "{}: declares {}, observed {}",
                path.display(),
                declared.version_id(),
                observed.version_id()
            )),
            Detected::Inferred(_) => {}
        }
    }
    println!("official: {declared} archives with an agreed declared version");
    assert!(declared > 0, "corpus at {} is empty", root.display());
    assert!(
        conflicts.is_empty(),
        "detection contradicts the official corpus:\n{}",
        conflicts.join("\n")
    );
}
