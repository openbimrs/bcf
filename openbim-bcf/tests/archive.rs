//! Archive-level behaviour, exercised against synthesised BCF containers.
//!
//! Fixtures are built in-memory rather than committed, for two reasons: the
//! official buildingSMART test cases are CC BY-ND and the third-party corpus
//! is other people's project data, so neither may be vendored into this AGPL-3.0-or-later
//! repository. `corpus.rs` covers those files where they exist locally.

use openbim_bcf::{BcfError, BcfVersion, Limits, Tolerance};
use openbim_core::Detected;
use std::io::{Cursor, Write};

/// Build a ZIP from `(name, bytes)` pairs.
fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
        let opts: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in entries {
            w.start_file(*name, opts).unwrap();
            w.write_all(bytes).unwrap();
        }
        w.finish().unwrap();
    }
    buf
}

const MARKUP_2_1: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Markup>
  <Header><File IfcProject="p"><Filename>m.ifc</Filename></File></Header>
  <Topic Guid="t1" TopicStatus="Offen" TopicType="formale Prüfung">
    <Title>Kollision Lüftung</Title>
    <CreationDate>2015-04-14T15:51:25Z</CreationDate>
  </Topic>
  <Comment Guid="c1"><Comment>Bitte prüfen</Comment></Comment>
  <Viewpoints Guid="v1"><Viewpoint>viewpoint.bcfv</Viewpoint><Snapshot>snapshot.png</Snapshot></Viewpoints>
</Markup>"#;

const MARKUP_2_0: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<Markup>
  <Topic Guid="t1">
    <Title>Old shape</Title>
    <TopicStatus>Open</TopicStatus>
    <TopicType>Error</TopicType>
  </Topic>
  <Comment Guid="c1"><Topic Guid="t1"/><Comment>back-reference</Comment></Comment>
</Markup>"#;

fn version_entry(id: &str) -> Vec<u8> {
    format!(r#"<?xml version="1.0" encoding="utf-8"?><Version VersionId="{id}"><DetailedVersion>{id}</DetailedVersion></Version>"#)
        .into_bytes()
}

#[test]
fn reads_a_2_1_archive_and_reports_a_declared_version() {
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("2.1")),
        ("t1/markup.bcf", MARKUP_2_1.as_bytes()),
        ("t1/viewpoint.bcfv", b"<VisualizationInfo/>"),
        ("t1/snapshot.png", b"\x89PNG\r\n\x1a\n"),
    ]);
    let archive = openbim_bcf::read_slice(&bytes).unwrap();

    assert_eq!(*archive.version(), Detected::Declared(BcfVersion::V2_1));
    assert_eq!(archive.topic_count(), 1);

    let topic = archive.topics().next().unwrap();
    assert_eq!(topic.title(), "Kollision Lüftung");
    assert_eq!(topic.status(), Some("Offen"));
    assert_eq!(topic.topic.topic_type.as_deref(), Some("formale Prüfung"));
    assert_eq!(topic.comments.len(), 1);
    assert_eq!(topic.viewpoints.len(), 1);
    assert_eq!(topic.header_files[0].filename.as_deref(), Some("m.ifc"));

    assert!(
        archive.diagnostics().is_empty(),
        "{:?}",
        archive.diagnostics()
    );
}

#[test]
fn a_missing_version_entry_is_tolerated_and_the_version_inferred() {
    let bytes = zip_of(&[
        ("t1/markup.bcf", MARKUP_2_1.as_bytes()),
        ("t1/viewpoint.bcfv", b"<VisualizationInfo/>"),
        ("t1/snapshot.png", b"\x89PNG"),
    ]);
    let archive = openbim_bcf::read_slice(&bytes).unwrap();

    assert_eq!(*archive.version(), Detected::Inferred(BcfVersion::V2_1));
    assert!(archive
        .diagnostics()
        .iter()
        .any(|d| d.tolerance == Tolerance::MissingVersionEntry));
    // Tolerated, not silently absorbed: the caller can still refuse it.
    assert_eq!(archive.topic_count(), 1);
}

#[test]
fn a_declared_version_contradicting_the_markup_shape_is_a_conflict() {
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("2.1")),
        ("t1/markup.bcf", MARKUP_2_0.as_bytes()),
    ]);
    let archive = openbim_bcf::read_slice(&bytes).unwrap();

    assert_eq!(
        *archive.version(),
        Detected::Conflict {
            declared: BcfVersion::V2_1,
            observed: BcfVersion::V2_0
        }
    );
    // resolved() refusing to pick is the whole point.
    assert_eq!(archive.version().resolved(), None);
    assert!(archive
        .diagnostics()
        .iter()
        .any(|d| matches!(d.tolerance, Tolerance::VersionConflict { .. })));
}

#[test]
fn an_unknown_version_id_is_reported_verbatim_and_falls_back_to_shape() {
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("4.2")),
        ("t1/markup.bcf", MARKUP_2_0.as_bytes()),
    ]);
    let archive = openbim_bcf::read_slice(&bytes).unwrap();

    assert_eq!(*archive.version(), Detected::Inferred(BcfVersion::V2_0));
    assert!(archive.diagnostics().iter().any(|d| matches!(
        &d.tolerance,
        Tolerance::UnknownVersionId { raw } if raw == "4.2"
    )));
}

#[test]
fn multiple_topics_are_all_read_and_ordered() {
    let second = MARKUP_2_1
        .replace("t1", "t2")
        .replace("Kollision Lüftung", "Zweites Thema");
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("2.1")),
        ("t1/markup.bcf", MARKUP_2_1.as_bytes()),
        ("t2/markup.bcf", second.as_bytes()),
    ]);
    let archive = openbim_bcf::read_slice(&bytes).unwrap();
    assert_eq!(archive.topic_count(), 2);
    let titles: Vec<_> = archive.topics().map(openbim_bcf::Markup::title).collect();
    assert_eq!(titles, ["Kollision Lüftung", "Zweites Thema"]);
}

#[test]
fn one_unreadable_topic_does_not_lose_the_readable_ones() {
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("2.1")),
        ("bad/markup.bcf", b"\x00\x01 not xml"),
        ("t1/markup.bcf", MARKUP_2_1.as_bytes()),
    ]);
    let archive = openbim_bcf::read_slice(&bytes).unwrap();

    assert_eq!(archive.topic_count(), 1);
    assert!(archive.diagnostics().iter().any(|d| {
        d.entry.as_deref() == Some("bad/markup.bcf")
            && matches!(d.tolerance, Tolerance::UnreadableMarkup { .. })
    }));
}

#[test]
fn an_archive_with_no_markup_at_all_is_an_error_not_an_empty_result() {
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("2.1")),
        ("readme.txt", b"hi"),
    ]);
    assert!(matches!(
        openbim_bcf::read_slice(&bytes).unwrap_err(),
        BcfError::NoTopics
    ));
}

#[test]
fn viewpoint_references_with_no_matching_entry_are_reported_but_kept() {
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("2.1")),
        ("t1/markup.bcf", MARKUP_2_1.as_bytes()),
        // viewpoint.bcfv and snapshot.png deliberately absent
    ]);
    let archive = openbim_bcf::read_slice(&bytes).unwrap();

    let dangling: Vec<_> = archive
        .diagnostics()
        .iter()
        .filter_map(|d| match &d.tolerance {
            Tolerance::DanglingReference { target } => Some(target.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(dangling, ["t1/viewpoint.bcfv", "t1/snapshot.png"]);

    // The reference itself survives — dropping it would lose information.
    let vp = &archive.topics().next().unwrap().viewpoints[0];
    assert_eq!(vp.viewpoint.as_deref(), Some("viewpoint.bcfv"));
}

#[test]
fn backslash_separators_are_normalised_and_recorded() {
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("2.1")),
        ("t1\\markup.bcf", MARKUP_2_1.as_bytes()),
        ("t1\\viewpoint.bcfv", b"<VisualizationInfo/>"),
        ("t1\\snapshot.png", b"\x89PNG"),
    ]);
    let archive = openbim_bcf::read_slice(&bytes).unwrap();

    assert!(archive.entries().iter().any(|e| e == "t1/markup.bcf"));
    assert!(archive.diagnostics().iter().any(|d| matches!(
        &d.tolerance,
        Tolerance::BackslashSeparator { name } if name.contains('\\')
    )));
    // Normalisation must actually resolve the references, not just rename.
    assert!(!archive
        .diagnostics()
        .iter()
        .any(|d| matches!(d.tolerance, Tolerance::DanglingReference { .. })));
}

#[test]
fn a_traversal_entry_is_refused_outright() {
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("2.1")),
        ("../../escape.bcf", MARKUP_2_1.as_bytes()),
    ]);
    match openbim_bcf::read_slice(&bytes).unwrap_err() {
        BcfError::UnsafeEntry { name } => assert!(name.contains(".."), "{name}"),
        other => panic!("expected UnsafeEntry, got {other:?}"),
    }
}

#[test]
fn an_oversized_entry_is_refused_rather_than_buffered() {
    let big = vec![b' '; 4096];
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("2.1")),
        ("t1/markup.bcf", &big),
    ]);
    let limits = Limits {
        max_entry_uncompressed: 1024,
        ..Limits::default()
    };
    match openbim_bcf::read_slice_with(&bytes, limits) {
        Err(BcfError::NoTopics | BcfError::LimitExceeded { .. }) => {}
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_total_size_bomb_is_refused_before_decompression() {
    let chunk = vec![b'0'; 2 * 1024 * 1024];
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("2.1")),
        ("t1/markup.bcf", MARKUP_2_1.as_bytes()),
        ("payload.bin", &chunk),
    ]);
    let limits = Limits {
        max_total_uncompressed: 64 * 1024,
        ..Limits::default()
    };
    match openbim_bcf::read_slice_with(&bytes, limits) {
        Err(BcfError::LimitExceeded { limit, .. }) => assert_eq!(limit, "uncompressed size"),
        other => panic!("expected LimitExceeded, got {other:?}"),
    }
}

#[test]
fn the_2_0_shape_is_read_with_status_from_child_elements() {
    let bytes = zip_of(&[
        ("bcf.version", &version_entry("2.0")),
        ("t1/markup.bcf", MARKUP_2_0.as_bytes()),
    ]);
    let archive = openbim_bcf::read_slice(&bytes).unwrap();

    assert_eq!(*archive.version(), Detected::Declared(BcfVersion::V2_0));
    let topic = archive.topics().next().unwrap();
    assert_eq!(topic.status(), Some("Open"));
    assert_eq!(topic.topic.topic_type.as_deref(), Some("Error"));
    assert_eq!(topic.comments[0].comment.as_deref(), Some("back-reference"));
}
