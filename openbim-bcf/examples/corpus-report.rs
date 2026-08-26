//! Report reader behaviour across a corpus. Not a test: an evidence tool whose
//! output is quoted in README/docs and must be reproducible.
//!
//! ```bash
//! cargo run --example corpus-report -- <dir>...
//! ```

use openbim_bcf::{BcfError, Tolerance};
use openbim_core::Detected;
use std::collections::BTreeMap;
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
            } else {
                let ext = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                if ext == "bcf" || ext == "bcfzip" {
                    out.push(path);
                }
            }
        }
    }
    out.sort();
    out
}

fn label(t: &Tolerance) -> &'static str {
    match t {
        Tolerance::MissingVersionEntry => "MissingVersionEntry",
        Tolerance::UnknownVersionId { .. } => "UnknownVersionId",
        Tolerance::VersionConflict { .. } => "VersionConflict",
        Tolerance::UnreadableMarkup { .. } => "UnreadableMarkup",
        Tolerance::MarkupWithoutTopic => "MarkupWithoutTopic",
        Tolerance::TopicWithoutGuid => "TopicWithoutGuid",
        Tolerance::CommentWithoutGuid => "CommentWithoutGuid",
        Tolerance::TopicWithoutTitle => "TopicWithoutTitle",
        Tolerance::DanglingReference { .. } => "DanglingReference",
        Tolerance::BackslashSeparator { .. } => "BackslashSeparator",
        Tolerance::UnparseableDateTime { .. } => "UnparseableDateTime",
        _ => "Other",
    }
}

fn main() {
    let roots: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    if roots.is_empty() {
        eprintln!("usage: corpus-report <dir>...");
        std::process::exit(2);
    }

    let (mut read, mut loose, mut failed, mut topics, mut comments) = (0, 0, 0, 0usize, 0usize);
    let mut versions: BTreeMap<String, usize> = BTreeMap::new();
    let mut tolerances: BTreeMap<&str, usize> = BTreeMap::new();
    let mut statuses: BTreeMap<String, usize> = BTreeMap::new();

    for root in &roots {
        for path in archives_under(root) {
            match openbim_bcf::read_path(&path) {
                Ok(a) => {
                    read += 1;
                    topics += a.topic_count();
                    comments += a.topics().map(|t| t.comments.len()).sum::<usize>();
                    let v = match a.version() {
                        Detected::Declared(v) => format!("Declared({})", v.version_id()),
                        Detected::Inferred(v) => format!("Inferred({})", v.version_id()),
                        Detected::Conflict { declared, observed } => format!(
                            "Conflict({} vs {})",
                            declared.version_id(),
                            observed.version_id()
                        ),
                    };
                    *versions.entry(v).or_default() += 1;
                    for d in a.diagnostics() {
                        *tolerances.entry(label(&d.tolerance)).or_default() += 1;
                    }
                    for t in a.topics() {
                        *statuses
                            .entry(t.status().unwrap_or("<none>").to_string())
                            .or_default() += 1;
                    }
                }
                Err(BcfError::NotAnArchive { .. }) => loose += 1,
                Err(e) => {
                    failed += 1;
                    println!("FAILED {}: {e}", path.display());
                }
            }
        }
    }

    println!("archives read : {read}");
    println!("loose xml     : {loose}");
    println!("failed        : {failed}");
    println!("topics        : {topics}");
    println!("comments      : {comments}");
    println!("\n-- version detection --");
    for (k, v) in &versions {
        println!("{k:28} {v}");
    }
    println!("\n-- tolerances --");
    for (k, v) in &tolerances {
        println!("{k:28} {v}");
    }
    println!("\n-- TopicStatus values (verbatim) --");
    for (k, v) in &statuses {
        println!("{k:28} {v}");
    }
}
