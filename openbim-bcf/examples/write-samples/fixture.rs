//! Sample documents shared by the `write-samples` example (whose output
//! `scripts/validate-written.py` checks against the official XSDs) and the
//! golden-file test in `tests/write.rs`. Sharing them means the bytes pinned
//! as golden are exactly the bytes proven schema-valid.
//!
//! The content is deliberately awkward: non-ASCII text, markup characters,
//! CRLF and tabs, uppercase 2.1 GUIDs, components identified only by an
//! authoring-tool id, and both camera kinds.

use openbim_bcf::write::{
    Camera, Comment, Document, Extensions, Projection, TargetVersion, Topic, Vector3, Viewpoint,
};
use openbim_bcf::Component;

/// `(file stem, document)` for every sample.
pub fn samples() -> Vec<(&'static str, Document)> {
    vec![
        ("sample-2.1", sample(TargetVersion::V2_1, None)),
        (
            "sample-3.0",
            sample(TargetVersion::V3_0, Some(extensions())),
        ),
        (
            "sample-3.0-derived-extensions",
            sample(TargetVersion::V3_0, None),
        ),
    ]
}

fn extensions() -> Extensions {
    Extensions {
        topic_types: vec!["Clash".into(), "formale Prüfung".into(), "Info".into()],
        topic_statuses: vec!["Open".into(), "Offen".into(), "Closed".into()],
        priorities: vec!["High".into(), "Normal".into()],
        topic_labels: vec!["Architektur".into(), "MEP".into(), "Struktur".into()],
        users: vec!["checker@example.com".into(), "reviewer@example.com".into()],
        snippet_types: Vec::new(),
        stages: vec!["Entwurf".into()],
    }
}

fn sample(version: TargetVersion, extensions: Option<Extensions>) -> Document {
    let v3 = version == TargetVersion::V3_0;
    let aspect_ratio = v3.then_some(1.777_777_777_777_777_7);
    Document {
        version,
        extensions,
        topics: vec![
            Topic {
                guid: "3f2504e0-4f89-41d3-9a0c-0305e82c3301".into(),
                title: "Kollision Lüftung & Träger <UG>".into(),
                description: Some(
                    "Duct \"L-12\" intersects beam B7.\r\nFound by rule C-12:\tclearance < 50 mm."
                        .into(),
                ),
                topic_type: Some("Clash".into()),
                topic_status: Some("Open".into()),
                priority: Some("High".into()),
                labels: vec!["MEP".into(), "Struktur".into()],
                creation_date: "2026-09-26T10:00:00Z".into(),
                creation_author: "checker@example.com".into(),
                comments: vec![
                    Comment {
                        guid: "0b7c3c1e-9d0a-4d2b-8f55-1a2b3c4d5e6f".into(),
                        date: "2026-09-26T10:00:00Z".into(),
                        author: "checker@example.com".into(),
                        comment: "Clearance is 12 mm; required ≥ 50 mm.".into(),
                        viewpoint: Some("7e1d2c3b-4a59-4687-9a1b-2c3d4e5f6a7b".into()),
                    },
                    Comment {
                        guid: "1c8d4d2f-ae1b-4e3c-9066-2b3c4d5e6f70".into(),
                        date: "2026-09-27T08:30:15.250+02:00".into(),
                        author: "reviewer@example.com".into(),
                        comment: "Confirmed. Rerouting above the beam.".into(),
                        viewpoint: None,
                    },
                ],
                viewpoints: vec![
                    Viewpoint {
                        guid: "7e1d2c3b-4a59-4687-9a1b-2c3d4e5f6a7b".into(),
                        selection: vec![
                            Component {
                                ifc_guid: Some("0fXw$sQh19ixbI4tZgfkXu".into()),
                                originating_system: Some("Revit 2026".into()),
                                authoring_tool_id: Some("412233".into()),
                            },
                            Component::ifc("1Qj3z6Wtb5WwCLdW5ctWxe"),
                            Component {
                                authoring_tool_id: Some("ARCHICAD:77".into()),
                                ..Component::default()
                            },
                        ],
                        camera: Some(Camera {
                            projection: Projection::Perspective {
                                field_of_view: 60.0,
                            },
                            view_point: Vector3::new(
                                26.002_615_225_590_86,
                                -26.152_546_069_409_244,
                                20.25,
                            ),
                            direction: Vector3::new(
                                -0.665_854_744_985_119_4,
                                0.484_255_386_946_369_66,
                                -0.567_568_655_577_652,
                            ),
                            up_vector: Vector3::new(0.0, 0.0, 1.0),
                            aspect_ratio,
                        }),
                    },
                    Viewpoint {
                        guid: "8f2e3d4c-5b6a-4798-8b2c-3d4e5f6a7b8c".into(),
                        selection: Vec::new(),
                        camera: Some(Camera {
                            projection: Projection::Orthogonal {
                                view_to_world_scale: 12.5,
                            },
                            view_point: Vector3::new(0.0, 0.0, 100.0),
                            direction: Vector3::new(0.0, 0.0, -1.0),
                            up_vector: Vector3::new(0.0, 1.0, 0.0),
                            aspect_ratio,
                        }),
                    },
                ],
            },
            Topic {
                // 2.1 accepts uppercase GUIDs; 3.0 does not.
                guid: if v3 {
                    "a1b2c3d4-e5f6-4a7b-8c9d-0e1f2a3b4c5d".into()
                } else {
                    "A1B2C3D4-E5F6-4A7B-8C9D-0E1F2A3B4C5D".into()
                },
                title: "Formale Prüfung: Raumnummer fehlt".into(),
                topic_type: Some("formale Prüfung".into()),
                topic_status: Some("Offen".into()),
                creation_date: "2026-09-26T10:05:00+02:00".into(),
                creation_author: "checker@example.com".into(),
                ..Topic::default()
            },
        ],
    }
}
