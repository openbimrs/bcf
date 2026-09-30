//! The writer: golden bytes, round trips through the reader, and refusals.
//!
//! Schema validity of the same sample documents is proven separately by
//! `scripts/validate-written.py` against the official XSDs, which cannot be
//! vendored here (CC BY-ND).

#[path = "../examples/write-samples/fixture.rs"]
mod fixture;

use openbim_bcf::write::{
    self, Camera, ClippingPlane, Coloring, Comment, Compression, Document, Extensions, Invalid,
    Options, Projection, TargetVersion, Topic, Vector3, Viewpoint, Visibility, WriteError,
};
use openbim_bcf::{BcfVersion, Component, Markup};
use openbim_core::Detected;
use std::path::PathBuf;

fn golden_path(stem: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{stem}.bcfzip"))
}

/// Identical input must give identical bytes — not merely equivalent XML —
/// so reports built on this crate can be diffed and cached.
///
/// Regenerate after an intended format change with
/// `BCF_BLESS=1 cargo test --test write`, then inspect the diff.
#[test]
fn output_is_byte_identical_to_the_golden_files() {
    let bless = std::env::var_os("BCF_BLESS").is_some();
    for (stem, doc) in fixture::samples() {
        let bytes = write::to_vec(&doc).unwrap();
        let path = golden_path(stem);
        if bless {
            std::fs::write(&path, &bytes).unwrap();
            continue;
        }
        let golden = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{}: {e}; run with BCF_BLESS=1", path.display()));
        assert!(
            bytes == golden,
            "{stem}: output differs from {}",
            path.display()
        );
    }
}

#[test]
fn every_sink_receives_the_same_bytes() {
    let dir = std::env::temp_dir().join(format!("openbim-bcf-write-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (stem, doc) in fixture::samples() {
        let in_memory = write::to_vec(&doc).unwrap();
        assert_eq!(in_memory, write::to_vec(&doc.clone()).unwrap(), "{stem}");
        let path = dir.join(format!("{stem}.bcfzip"));
        write::to_path(&doc, &path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), in_memory, "{stem}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn zip_metadata_is_fixed() {
    for (stem, doc) in fixture::samples() {
        let bytes = write::to_vec(&doc).unwrap();
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        for i in 0..zip.len() {
            let f = zip.by_index(i).unwrap();
            assert_eq!(f.compression(), zip::CompressionMethod::Stored, "{stem}");
            assert_eq!(
                f.last_modified(),
                Some(zip::DateTime::default()),
                "{stem}: {}",
                f.name()
            );
            assert_eq!(f.unix_mode(), Some(0o100_644), "{stem}: {}", f.name());
        }
    }
}

/// Every written archive reads back with zero diagnostics and a declared
/// version, and what was written is what is read.
#[test]
fn written_archives_round_trip_through_the_reader() {
    for (stem, doc) in fixture::samples() {
        let archive = openbim_bcf::read_slice(&write::to_vec(&doc).unwrap()).unwrap();
        assert!(
            archive.diagnostics().is_empty(),
            "{stem}: {:?}",
            archive.diagnostics()
        );
        assert_eq!(
            *archive.version(),
            Detected::Declared(BcfVersion::from(doc.version)),
            "{stem}"
        );
        assert_read_back(stem, &doc, &archive.topics().collect::<Vec<_>>());
    }
}

fn assert_read_back(stem: &str, doc: &Document, read: &[&Markup]) {
    assert_eq!(read.len(), doc.topics.len(), "{stem}");
    for (w, r) in doc.topics.iter().zip(read) {
        let t = &r.topic;
        assert_eq!(t.guid.as_deref(), Some(w.guid.as_str()), "{stem}");
        assert_eq!(t.title.as_deref(), Some(w.title.as_str()), "{stem}");
        assert_eq!(t.description, w.description, "{stem}");
        assert_eq!(t.topic_type, w.topic_type, "{stem}");
        assert_eq!(t.topic_status, w.topic_status, "{stem}");
        assert_eq!(t.priority, w.priority, "{stem}");
        assert_eq!(t.labels, w.labels, "{stem}");
        assert_eq!(t.creation_date.as_deref(), Some(w.creation_date.as_str()));
        assert_eq!(
            t.creation_author.as_deref(),
            Some(w.creation_author.as_str())
        );
        assert_eq!(t.assigned_to, w.assigned_to, "{stem}");
        assert_eq!(t.due_date, w.due_date, "{stem}");

        assert_eq!(r.comments.len(), w.comments.len(), "{stem}");
        for (wc, rc) in w.comments.iter().zip(&r.comments) {
            assert_eq!(rc.guid.as_deref(), Some(wc.guid.as_str()), "{stem}");
            assert_eq!(rc.date.as_deref(), Some(wc.date.as_str()), "{stem}");
            assert_eq!(rc.author.as_deref(), Some(wc.author.as_str()), "{stem}");
            assert_eq!(rc.comment.as_deref(), Some(wc.comment.as_str()), "{stem}");
            assert_eq!(rc.viewpoint, wc.viewpoint, "{stem}");
        }

        assert_eq!(r.viewpoints.len(), w.viewpoints.len(), "{stem}");
        for (wv, rv) in w.viewpoints.iter().zip(&r.viewpoints) {
            assert_eq!(rv.guid.as_deref(), Some(wv.guid.as_str()), "{stem}");
            let vis = rv
                .visualization
                .as_ref()
                .unwrap_or_else(|| panic!("{stem}: viewpoint {} unread", wv.guid));
            assert_eq!(vis.guid.as_deref(), Some(wv.guid.as_str()), "{stem}");
            assert_eq!(vis.selection, wv.selection, "{stem}");
        }
    }
}

/// The derived 3.0 vocabulary lists exactly the values in use, in first-use
/// order, so the archive is self-consistent without the caller supplying one.
#[test]
fn a_3_0_archive_without_extensions_gets_the_vocabulary_it_uses() {
    let (_, doc) = fixture::samples()
        .into_iter()
        .find(|(s, _)| *s == "sample-3.0-derived-extensions")
        .unwrap();
    let bytes = write::to_vec(&doc).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("extensions.xml").unwrap(), &mut xml).unwrap();
    let pos = |s: &str| xml.find(s).unwrap_or_else(|| panic!("{s} missing:\n{xml}"));
    assert!(pos("<TopicType>Clash</TopicType>") < pos("<TopicType>formale Prüfung</TopicType>"));
    assert!(pos("<TopicStatus>Open</TopicStatus>") < pos("<TopicStatus>Offen</TopicStatus>"));
    pos("<Priority>High</Priority>");
    assert!(pos("<TopicLabel>MEP</TopicLabel>") < pos("<TopicLabel>Struktur</TopicLabel>"));
    assert!(
        !xml.contains("Info"),
        "unused values are not invented:\n{xml}"
    );
    assert!(xml.contains("<Users/>"), "{xml}");
}

/// A 2.1 archive carries no extensions file: 2.1 expresses its vocabulary
/// as an `extensions.xsd`, which is out of scope.
#[test]
fn a_2_1_archive_has_no_extensions_xml() {
    let (_, doc) = fixture::samples().into_iter().next().unwrap();
    assert_eq!(doc.version, TargetVersion::V2_1);
    let archive = openbim_bcf::read_slice(&write::to_vec(&doc).unwrap()).unwrap();
    assert!(!archive.entries().iter().any(|e| e.contains("extensions")));
}

// --- refusals ---------------------------------------------------------------

const T: &str = "3f2504e0-4f89-41d3-9a0c-0305e82c3301";
const C: &str = "0b7c3c1e-9d0a-4d2b-8f55-1a2b3c4d5e6f";
const V: &str = "7e1d2c3b-4a59-4687-9a1b-2c3d4e5f6a7b";

fn camera() -> Camera {
    Camera {
        projection: Projection::Perspective {
            field_of_view: 50.0,
        },
        view_point: Vector3::new(1.0, 2.0, 3.0),
        direction: Vector3::new(0.0, 1.0, 0.0),
        up_vector: Vector3::new(0.0, 0.0, 1.0),
        aspect_ratio: None,
    }
}

/// A minimal valid document for `version`, for mutation by the refusal cases.
fn minimal(version: TargetVersion) -> Document {
    let v3 = version == TargetVersion::V3_0;
    Document {
        version,
        extensions: None,
        topics: vec![Topic {
            guid: T.into(),
            title: "Title".into(),
            topic_type: Some("Clash".into()),
            topic_status: Some("Open".into()),
            creation_date: "2026-09-26T10:00:00Z".into(),
            creation_author: "a@example.com".into(),
            comments: vec![Comment {
                guid: C.into(),
                date: "2026-09-26T10:00:00Z".into(),
                author: "a@example.com".into(),
                comment: "text".into(),
                viewpoint: Some(V.into()),
            }],
            viewpoints: vec![Viewpoint {
                guid: V.into(),
                selection: vec![Component::ifc("0fXw$sQh19ixbI4tZgfkXu")],
                camera: Some(Camera {
                    aspect_ratio: v3.then_some(1.5),
                    ..camera()
                }),
                ..Viewpoint::default()
            }],
            ..Topic::default()
        }],
    }
}

fn coloring(color: &str) -> Coloring {
    Coloring {
        color: color.into(),
        components: vec![Component::ifc("0fXw$sQh19ixbI4tZgfkXu")],
    }
}

fn refusal(doc: &Document) -> (String, Invalid) {
    match write::to_vec(doc) {
        Err(WriteError::Invalid { at, problem }) => (at, problem),
        Err(other) => panic!("expected Invalid, got {other:?}"),
        Ok(_) => panic!("document was accepted"),
    }
}

#[test]
fn the_minimal_documents_are_themselves_valid() {
    for v in [TargetVersion::V2_1, TargetVersion::V3_0] {
        let archive = openbim_bcf::read_slice(&write::to_vec(&minimal(v)).unwrap()).unwrap();
        assert!(
            archive.diagnostics().is_empty(),
            "{:?}",
            archive.diagnostics()
        );
    }
}

/// `(label, version, mutation, expected location, expected problem)`.
type RefusalCase = (
    &'static str,
    TargetVersion,
    fn(&mut Document),
    &'static str,
    fn(&Invalid) -> bool,
);

// One table of cases reads better than twenty near-identical test functions.
#[allow(clippy::too_many_lines)]
#[test]
fn malformed_values_are_refused_with_their_location() {
    let cases: Vec<RefusalCase> = vec![
        (
            "topic GUID without dashes",
            TargetVersion::V2_1,
            |d| d.topics[0].guid = "3f2504e04f8941d39a0c0305e82c3301".into(),
            "topics[0].guid",
            |p| matches!(p, Invalid::Guid { .. }),
        ),
        (
            "uppercase GUID in 3.0",
            TargetVersion::V3_0,
            |d| d.topics[0].comments[0].guid = C.to_uppercase(),
            "topics[0].comments[0].guid",
            |p| matches!(p, Invalid::Guid { .. }),
        ),
        (
            "IfcGuid of 21 characters",
            TargetVersion::V2_1,
            |d| d.topics[0].viewpoints[0].selection[0] = Component::ifc("0fXw$sQh19ixbI4tZgfkX"),
            "topics[0].viewpoints[0].selection[0].ifc_guid",
            |p| matches!(p, Invalid::IfcGuid { .. }),
        ),
        (
            "IfcGuid outside the IFC alphabet",
            TargetVersion::V3_0,
            |d| d.topics[0].viewpoints[0].selection[0] = Component::ifc("0fXw-sQh19ixbI4tZgfkXu"),
            "topics[0].viewpoints[0].selection[0].ifc_guid",
            |p| matches!(p, Invalid::IfcGuid { .. }),
        ),
        (
            "component identifying nothing",
            TargetVersion::V2_1,
            |d| {
                d.topics[0].viewpoints[0].selection[0] = Component {
                    originating_system: Some("Revit".into()),
                    ..Component::default()
                };
            },
            "topics[0].viewpoints[0].selection[0]",
            |p| matches!(p, Invalid::UnidentifiedComponent),
        ),
        (
            "date without time",
            TargetVersion::V2_1,
            |d| d.topics[0].creation_date = "2026-09-26".into(),
            "topics[0].creation_date",
            |p| matches!(p, Invalid::DateTime { .. }),
        ),
        (
            "impossible comment date",
            TargetVersion::V3_0,
            |d| d.topics[0].comments[0].date = "2026-02-30T00:00:00Z".into(),
            "topics[0].comments[0].date",
            |p| matches!(p, Invalid::DateTime { .. }),
        ),
        (
            "blank title",
            TargetVersion::V2_1,
            |d| d.topics[0].title = "  ".into(),
            "topics[0].title",
            |p| matches!(p, Invalid::Blank),
        ),
        (
            "trailing whitespace that the reader would trim",
            TargetVersion::V2_1,
            |d| d.topics[0].description = Some("text\n".into()),
            "topics[0].description",
            |p| matches!(p, Invalid::SurroundingWhitespace { .. }),
        ),
        (
            "control character XML cannot carry",
            TargetVersion::V3_0,
            |d| d.topics[0].comments[0].comment = "bell\u{7}".into(),
            "topics[0].comments[0].comment",
            |p| matches!(p, Invalid::ForbiddenCharacter { ch: '\u{7}' }),
        ),
        (
            "3.0 topic without a type",
            TargetVersion::V3_0,
            |d| d.topics[0].topic_type = None,
            "topics[0].topic_type",
            |p| matches!(p, Invalid::Missing),
        ),
        (
            "3.0 topic without a status",
            TargetVersion::V3_0,
            |d| d.topics[0].topic_status = None,
            "topics[0].topic_status",
            |p| matches!(p, Invalid::Missing),
        ),
        (
            "3.0 viewpoint without a camera",
            TargetVersion::V3_0,
            |d| d.topics[0].viewpoints[0].camera = None,
            "topics[0].viewpoints[0].camera",
            |p| matches!(p, Invalid::Missing),
        ),
        (
            "3.0 camera without an aspect ratio",
            TargetVersion::V3_0,
            |d| d.topics[0].viewpoints[0].camera = Some(camera()),
            "topics[0].viewpoints[0].camera.aspect_ratio",
            |p| matches!(p, Invalid::Missing),
        ),
        (
            "2.1 camera with an aspect ratio it cannot hold",
            TargetVersion::V2_1,
            |d| {
                d.topics[0].viewpoints[0].camera = Some(Camera {
                    aspect_ratio: Some(1.5),
                    ..camera()
                });
            },
            "topics[0].viewpoints[0].camera.aspect_ratio",
            |p| {
                matches!(
                    p,
                    Invalid::NotInVersion {
                        version: TargetVersion::V2_1
                    }
                )
            },
        ),
        (
            "2.1 field of view outside 45..=60",
            TargetVersion::V2_1,
            |d| {
                d.topics[0].viewpoints[0].camera = Some(Camera {
                    projection: Projection::Perspective {
                        field_of_view: 90.0,
                    },
                    ..camera()
                });
            },
            "topics[0].viewpoints[0].camera.projection.field_of_view",
            |p| matches!(p, Invalid::Number { .. }),
        ),
        (
            "non-finite camera coordinate",
            TargetVersion::V2_1,
            |d| {
                d.topics[0].viewpoints[0].camera = Some(Camera {
                    view_point: Vector3::new(f64::NAN, 0.0, 0.0),
                    ..camera()
                });
            },
            "topics[0].viewpoints[0].camera.view_point.x",
            |p| matches!(p, Invalid::Number { .. }),
        ),
        (
            "zero viewing direction",
            TargetVersion::V2_1,
            |d| {
                d.topics[0].viewpoints[0].camera = Some(Camera {
                    direction: Vector3::default(),
                    ..camera()
                });
            },
            "topics[0].viewpoints[0].camera.direction",
            |p| matches!(p, Invalid::Number { .. }),
        ),
        (
            "non-positive orthogonal scale",
            TargetVersion::V2_1,
            |d| {
                d.topics[0].viewpoints[0].camera = Some(Camera {
                    projection: Projection::Orthogonal {
                        view_to_world_scale: 0.0,
                    },
                    ..camera()
                });
            },
            "topics[0].viewpoints[0].camera.projection.view_to_world_scale",
            |p| matches!(p, Invalid::Number { .. }),
        ),
        (
            "GUID reused across topic and viewpoint",
            TargetVersion::V2_1,
            |d| d.topics[0].viewpoints[0].guid = T.to_uppercase(),
            "topics[0].viewpoints[0].guid",
            |p| matches!(p, Invalid::DuplicateGuid { .. }),
        ),
        (
            "two topics with one GUID",
            TargetVersion::V3_0,
            |d| {
                let mut twin = d.topics[0].clone();
                twin.comments.clear();
                twin.viewpoints.clear();
                d.topics.push(twin);
            },
            "topics[1].guid",
            |p| matches!(p, Invalid::DuplicateGuid { .. }),
        ),
        (
            "comment anchored to a missing viewpoint",
            TargetVersion::V2_1,
            |d| {
                d.topics[0].comments[0].viewpoint =
                    Some("8f2e3d4c-5b6a-4798-8b2c-3d4e5f6a7b8c".into());
            },
            "topics[0].comments[0].viewpoint",
            |p| matches!(p, Invalid::UnknownViewpoint { .. }),
        ),
        (
            "visibility exception identifying nothing",
            TargetVersion::V3_0,
            |d| {
                d.topics[0].viewpoints[0].visibility = Some(Visibility {
                    default_visibility: false,
                    exceptions: vec![Component::default()],
                });
            },
            "topics[0].viewpoints[0].visibility.exceptions[0]",
            |p| matches!(p, Invalid::UnidentifiedComponent),
        ),
        (
            "visibility exception with a malformed IfcGuid",
            TargetVersion::V2_1,
            |d| {
                d.topics[0].viewpoints[0].visibility = Some(Visibility {
                    default_visibility: true,
                    exceptions: vec![Component::ifc("not-an-ifc-guid")],
                });
            },
            "topics[0].viewpoints[0].visibility.exceptions[0].ifc_guid",
            |p| matches!(p, Invalid::IfcGuid { .. }),
        ),
        (
            "colour of 7 digits",
            TargetVersion::V3_0,
            |d| d.topics[0].viewpoints[0].coloring = vec![coloring("FF00000")],
            "topics[0].viewpoints[0].coloring[0].color",
            |p| matches!(p, Invalid::Color { .. }),
        ),
        (
            "colour with a leading hash",
            TargetVersion::V3_0,
            |d| d.topics[0].viewpoints[0].coloring = vec![coloring("#FF0000")],
            "topics[0].viewpoints[0].coloring[0].color",
            |p| matches!(p, Invalid::Color { .. }),
        ),
        (
            "lowercase colour in 2.1",
            TargetVersion::V2_1,
            |d| d.topics[0].viewpoints[0].coloring = vec![coloring("ff0000")],
            "topics[0].viewpoints[0].coloring[0].color",
            |p| matches!(p, Invalid::Color { .. }),
        ),
        (
            "colouring without components",
            TargetVersion::V2_1,
            |d| {
                d.topics[0].viewpoints[0].coloring = vec![Coloring {
                    color: "FF0000".into(),
                    components: Vec::new(),
                }];
            },
            "topics[0].viewpoints[0].coloring[0].components",
            |p| matches!(p, Invalid::NoComponents),
        ),
        (
            "coloured component identifying nothing",
            TargetVersion::V3_0,
            |d| {
                d.topics[0].viewpoints[0].coloring = vec![Coloring {
                    color: "FF0000".into(),
                    components: vec![
                        Component::ifc("0fXw$sQh19ixbI4tZgfkXu"),
                        Component::default(),
                    ],
                }];
            },
            "topics[0].viewpoints[0].coloring[0].components[1]",
            |p| matches!(p, Invalid::UnidentifiedComponent),
        ),
        (
            "clipping plane with a zero direction",
            TargetVersion::V2_1,
            |d| {
                d.topics[0].viewpoints[0].clipping_planes = vec![ClippingPlane {
                    location: Vector3::new(1.0, 2.0, 3.0),
                    direction: Vector3::default(),
                }];
            },
            "topics[0].viewpoints[0].clipping_planes[0].direction",
            |p| matches!(p, Invalid::Number { .. }),
        ),
        (
            "clipping plane at a non-finite location",
            TargetVersion::V3_0,
            |d| {
                d.topics[0].viewpoints[0].clipping_planes = vec![ClippingPlane {
                    location: Vector3::new(0.0, f64::INFINITY, 0.0),
                    direction: Vector3::new(0.0, 0.0, 1.0),
                }];
            },
            "topics[0].viewpoints[0].clipping_planes[0].location.y",
            |p| matches!(p, Invalid::Number { .. }),
        ),
        (
            "due date without a time",
            TargetVersion::V2_1,
            |d| d.topics[0].due_date = Some("2026-10-15".into()),
            "topics[0].due_date",
            |p| matches!(p, Invalid::DateTime { .. }),
        ),
        (
            "blank assignee",
            TargetVersion::V3_0,
            |d| d.topics[0].assigned_to = Some("   ".into()),
            "topics[0].assigned_to",
            |p| matches!(p, Invalid::Blank),
        ),
        (
            "assignee with surrounding whitespace",
            TargetVersion::V2_1,
            |d| d.topics[0].assigned_to = Some("a@example.com ".into()),
            "topics[0].assigned_to",
            |p| matches!(p, Invalid::SurroundingWhitespace { .. }),
        ),
        (
            "no topics at all",
            TargetVersion::V3_0,
            |d| d.topics.clear(),
            "topics",
            |p| matches!(p, Invalid::NoTopics),
        ),
    ];

    for (label, version, mutate, at, expected) in cases {
        let mut doc = minimal(version);
        mutate(&mut doc);
        let (got_at, problem) = refusal(&doc);
        assert_eq!(got_at, at, "{label}: {problem}");
        assert!(expected(&problem), "{label}: unexpected {problem:?}");
    }
}

fn vocabulary() -> Extensions {
    Extensions {
        topic_types: vec!["Clash".into()],
        topic_statuses: vec!["Open".into()],
        priorities: vec!["High".into()],
        topic_labels: vec!["MEP".into()],
        users: vec!["a@example.com".into()],
        ..Extensions::default()
    }
}

/// Supplied extensions are enforced verbatim: `"open"` is not `"Open"`.
#[test]
fn values_missing_from_supplied_extensions_are_refused() {
    type Mutation = fn(&mut Topic);
    let cases: [(&str, Mutation, &str); 5] = [
        (
            "type",
            |t| t.topic_type = Some("Error".into()),
            "topics[0].topic_type",
        ),
        (
            "status",
            |t| t.topic_status = Some("open".into()),
            "topics[0].topic_status",
        ),
        (
            "priority",
            |t| t.priority = Some("Low".into()),
            "topics[0].priority",
        ),
        (
            "label",
            |t| t.labels = vec!["MEP".into(), "ARC".into()],
            "topics[0].labels[1]",
        ),
        (
            "assignee",
            |t| t.assigned_to = Some("stranger@example.com".into()),
            "topics[0].assigned_to",
        ),
    ];
    for version in [TargetVersion::V2_1, TargetVersion::V3_0] {
        let mut ok = minimal(version);
        ok.extensions = Some(vocabulary());
        ok.topics[0].priority = Some("High".into());
        ok.topics[0].labels = vec!["MEP".into()];
        ok.topics[0].assigned_to = Some("a@example.com".into());
        write::to_vec(&ok).unwrap();

        for (label, mutate, at) in cases {
            let mut doc = ok.clone();
            mutate(&mut doc.topics[0]);
            let (got_at, problem) = refusal(&doc);
            assert_eq!(got_at, at, "{label}");
            assert!(
                matches!(problem, Invalid::NotInExtensions { .. }),
                "{label}: {problem:?}"
            );
        }
    }
}

#[test]
fn blank_extension_values_are_refused() {
    let mut doc = minimal(TargetVersion::V3_0);
    let mut ext = vocabulary();
    ext.users = vec!["ok@example.com".into(), String::new()];
    doc.extensions = Some(ext);
    assert_eq!(
        refusal(&doc),
        ("extensions.users[1]".into(), Invalid::Blank)
    );
}

#[test]
fn an_invalid_document_leaves_the_filesystem_untouched() {
    let path =
        std::env::temp_dir().join(format!("openbim-bcf-refused-{}.bcfzip", std::process::id()));
    let mut doc = minimal(TargetVersion::V2_1);
    doc.topics[0].guid = "nope".into();
    assert!(write::to_path(&doc, &path).is_err());
    assert!(!path.exists(), "{} was created", path.display());
}

/// Errors name the field and the value, so a producer can fix its input.
#[test]
fn refusal_messages_name_the_location_and_value() {
    let mut doc = minimal(TargetVersion::V2_1);
    doc.topics[0].creation_date = "yesterday".into();
    let msg = write::to_vec(&doc).unwrap_err().to_string();
    assert!(msg.contains("topics[0].creation_date"), "{msg}");
    assert!(msg.contains("\"yesterday\""), "{msg}");
}

// --- compression options ----------------------------------------------------

#[test]
fn deflated_archives_round_trip_and_are_deterministic_within_a_build() {
    let options = Options::default().compression(Compression::Deflated { level: 9 });
    for (stem, doc) in fixture::samples() {
        let stored = write::to_vec(&doc).unwrap();
        let deflated = write::to_vec_with(&doc, options).unwrap();
        assert_eq!(
            deflated,
            write::to_vec_with(&doc, options).unwrap(),
            "{stem}"
        );
        assert!(
            deflated.len() < stored.len(),
            "{stem}: deflate did not shrink"
        );

        let archive = openbim_bcf::read_slice(&deflated).unwrap();
        assert!(
            archive.diagnostics().is_empty(),
            "{stem}: {:?}",
            archive.diagnostics()
        );
        assert_read_back(stem, &doc, &archive.topics().collect::<Vec<_>>());

        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(deflated)).unwrap();
        for i in 0..zip.len() {
            let f = zip.by_index(i).unwrap();
            assert_eq!(f.compression(), zip::CompressionMethod::Deflated, "{stem}");
            assert_eq!(f.last_modified(), Some(zip::DateTime::default()), "{stem}");
        }
    }
}

/// The level is part of reproducibility, so it is honoured, not ignored.
#[test]
fn the_deflate_level_changes_the_output() {
    let (_, doc) = fixture::samples().into_iter().next().unwrap();
    let fast = write::to_vec_with(
        &doc,
        Options::default().compression(Compression::Deflated { level: 1 }),
    );
    let small = write::to_vec_with(
        &doc,
        Options::default().compression(Compression::Deflated { level: 9 }),
    );
    assert_ne!(fast.unwrap(), small.unwrap());
}

#[test]
fn the_default_options_are_the_stored_golden_output() {
    for (stem, doc) in fixture::samples() {
        assert_eq!(
            write::to_vec_with(&doc, Options::default()).unwrap(),
            write::to_vec(&doc).unwrap(),
            "{stem}"
        );
    }
}

#[test]
fn out_of_range_deflate_levels_are_refused_before_anything_is_written() {
    let doc = minimal(TargetVersion::V2_1);
    for level in [0, 10, 255] {
        let options = Options::default().compression(Compression::Deflated { level });
        match write::to_vec_with(&doc, options) {
            Err(WriteError::Invalid { at, problem }) => {
                assert_eq!(at, "options.compression.level", "{level}");
                assert!(
                    matches!(problem, Invalid::Number { .. }),
                    "{level}: {problem:?}"
                );
            }
            other => panic!("level {level}: expected Invalid, got {other:?}"),
        }
        let path = std::env::temp_dir().join(format!(
            "openbim-bcf-level-{level}-{}.bcfzip",
            std::process::id()
        ));
        assert!(write::to_path_with(&doc, &path, options).is_err());
        assert!(!path.exists(), "{} was created", path.display());
    }
}

// --- visibility, colouring, clipping planes ---------------------------------

/// The `.bcfv` of the first viewpoint of the first topic.
fn first_visinfo(doc: &Document) -> String {
    let bytes = write::to_vec(doc).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let name = format!("{T}/Viewpoint_{V}.bcfv");
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name(&name).unwrap(), &mut xml).unwrap();
    xml
}

/// Without `visibility`, output is exactly what 0.3.0 wrote: the golden
/// files pin it byte for byte, and this names the element they rely on.
#[test]
fn no_visibility_means_everything_visible_as_before() {
    for v in [TargetVersion::V2_1, TargetVersion::V3_0] {
        let xml = first_visinfo(&minimal(v));
        assert!(
            xml.contains("    <Visibility DefaultVisibility=\"true\"/>\n"),
            "{xml}"
        );
        assert!(
            !xml.contains("Exceptions")
                && !xml.contains("Coloring")
                && !xml.contains("ClippingPlanes"),
            "{xml}"
        );
    }
}

#[test]
fn visibility_exceptions_are_written_in_order() {
    for v in [TargetVersion::V2_1, TargetVersion::V3_0] {
        let mut doc = minimal(v);
        doc.topics[0].viewpoints[0].visibility = Some(Visibility {
            default_visibility: false,
            exceptions: vec![
                Component::ifc("1Qj3z6Wtb5WwCLdW5ctWxe"),
                Component::ifc("0fXw$sQh19ixbI4tZgfkXu"),
            ],
        });
        let xml = first_visinfo(&doc);
        let expected = "    <Visibility DefaultVisibility=\"false\">\n      <Exceptions>\n        <Component IfcGuid=\"1Qj3z6Wtb5WwCLdW5ctWxe\"/>\n        <Component IfcGuid=\"0fXw$sQh19ixbI4tZgfkXu\"/>\n      </Exceptions>\n    </Visibility>\n";
        assert!(xml.contains(expected), "{v:?}:\n{xml}");
        assert!(
            xml.find("</Selection>") < xml.find("<Visibility"),
            "Selection precedes Visibility in the schema sequence:\n{xml}"
        );
    }
}

/// 2.1 lists coloured components under `Color`; 3.0 wraps them in
/// `Color/Components`. Both follow their own `visinfo.xsd`.
#[test]
fn colouring_follows_each_versions_shape() {
    for (v, expected) in [
        (
            TargetVersion::V2_1,
            "    <Coloring>\n      <Color Color=\"80FF0000\">\n        <Component IfcGuid=\"0fXw$sQh19ixbI4tZgfkXu\"/>\n      </Color>\n    </Coloring>\n",
        ),
        (
            TargetVersion::V3_0,
            "    <Coloring>\n      <Color Color=\"80FF0000\">\n        <Components>\n          <Component IfcGuid=\"0fXw$sQh19ixbI4tZgfkXu\"/>\n        </Components>\n      </Color>\n    </Coloring>\n",
        ),
    ] {
        let mut doc = minimal(v);
        doc.topics[0].viewpoints[0].coloring = vec![coloring("80FF0000")];
        let xml = first_visinfo(&doc);
        assert!(xml.contains(expected), "{v:?}:\n{xml}");
        assert!(xml.find("</Visibility>").or(xml.find("<Visibility")) < xml.find("<Coloring>"), "{xml}");
    }
}

/// A viewpoint with only colouring still gets `Components`, and the
/// `Visibility` 2.1 requires inside it.
#[test]
fn colouring_alone_writes_components_with_visibility() {
    let mut doc = minimal(TargetVersion::V2_1);
    doc.topics[0].viewpoints[0].selection.clear();
    doc.topics[0].viewpoints[0].coloring = vec![coloring("00AA00")];
    let xml = first_visinfo(&doc);
    assert!(!xml.contains("<Selection>"), "{xml}");
    assert!(
        xml.contains("<Visibility DefaultVisibility=\"true\"/>"),
        "{xml}"
    );
    assert!(xml.contains("<Color Color=\"00AA00\">"), "{xml}");
}

#[test]
fn clipping_planes_follow_the_camera() {
    for v in [TargetVersion::V2_1, TargetVersion::V3_0] {
        let mut doc = minimal(v);
        doc.topics[0].viewpoints[0].clipping_planes = vec![ClippingPlane {
            location: Vector3::new(0.0, 0.0, 3.5),
            direction: Vector3::new(0.0, 0.0, -1.0),
        }];
        let xml = first_visinfo(&doc);
        let expected = "  <ClippingPlanes>\n    <ClippingPlane>\n      <Location>\n        <X>0</X>\n        <Y>0</Y>\n        <Z>3.5</Z>\n      </Location>\n      <Direction>\n        <X>0</X>\n        <Y>0</Y>\n        <Z>-1</Z>\n      </Direction>\n    </ClippingPlane>\n  </ClippingPlanes>\n</VisualizationInfo>\n";
        assert!(xml.ends_with(expected), "{v:?}:\n{xml}");
        assert!(
            xml.find("</PerspectiveCamera>") < xml.find("<ClippingPlanes>"),
            "{xml}"
        );
    }
}

/// The styled samples still read back cleanly: the reader ignores what it
/// does not model rather than tripping over it.
#[test]
fn styled_viewpoints_read_back_without_diagnostics() {
    for (stem, doc) in fixture::samples() {
        let archive = openbim_bcf::read_slice(&write::to_vec(&doc).unwrap()).unwrap();
        assert!(
            archive.diagnostics().is_empty(),
            "{stem}: {:?}",
            archive.diagnostics()
        );
    }
}

// --- assignee and due date ---------------------------------------------------

fn entry_text(doc: &Document, name: &str) -> String {
    let bytes = write::to_vec(doc).unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut zip.by_name(name).unwrap(), &mut xml).unwrap();
    xml
}

/// Derived 3.0 `Users` list every assignee in first-use order, and nobody
/// else: creation and comment authors are not added.
#[test]
fn derived_3_0_users_are_the_assignees_in_first_use_order() {
    let (_, doc) = fixture::samples()
        .into_iter()
        .find(|(s, _)| *s == "sample-3.0-review-derived-extensions")
        .unwrap();
    let xml = entry_text(&doc, "extensions.xml");
    assert!(
        xml.contains("  <Users>\n    <User>reviewer@example.com</User>\n    <User>checker@example.com</User>\n  </Users>\n"),
        "{xml}"
    );
}

/// `DueDate` precedes `AssignedTo`, both between `CreationAuthor` and
/// `Description`, as the Topic sequence orders them in both schemas.
#[test]
fn due_date_and_assignee_sit_at_their_schema_position() {
    for v in [TargetVersion::V2_1, TargetVersion::V3_0] {
        let mut doc = minimal(v);
        doc.topics[0].description = Some("d".into());
        doc.topics[0].due_date = Some("2026-10-15T17:00:00Z".into());
        doc.topics[0].assigned_to = Some("a@example.com".into());
        let xml = entry_text(&doc, &format!("{T}/markup.bcf"));
        assert!(
            xml.contains("    <CreationAuthor>a@example.com</CreationAuthor>\n    <DueDate>2026-10-15T17:00:00Z</DueDate>\n    <AssignedTo>a@example.com</AssignedTo>\n    <Description>d</Description>\n"),
            "{v:?}:\n{xml}"
        );
    }
}
