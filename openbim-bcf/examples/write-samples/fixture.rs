//! Sample documents shared by the `write-samples` example (whose output
//! `scripts/validate-written.py` checks against the official XSDs) and the
//! golden-file test in `tests/write.rs`. Sharing them means the bytes pinned
//! as golden are exactly the bytes proven schema-valid.
//!
//! The content is deliberately awkward: non-ASCII text, markup characters,
//! CRLF and tabs, uppercase 2.1 GUIDs, components identified only by an
//! authoring-tool id, and both camera kinds.

use openbim_bcf::write::{
    Camera, ClippingPlane, Coloring, Comment, Document, Extensions, Projection, TargetVersion,
    Topic, Vector3, Viewpoint, Visibility,
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
        ("sample-2.1-styled", styled(TargetVersion::V2_1)),
        ("sample-3.0-styled", styled(TargetVersion::V3_0)),
        ("sample-2.1-review", review(TargetVersion::V2_1, None)),
        (
            "sample-3.0-review",
            review(TargetVersion::V3_0, Some(extensions())),
        ),
        (
            "sample-3.0-review-derived-extensions",
            review(TargetVersion::V3_0, None),
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
                assigned_to: None,
                due_date: None,
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
                        ..Viewpoint::default()
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
                        ..Viewpoint::default()
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

/// Viewpoints exercising visibility exceptions, colouring, and clipping
/// planes, alone and combined, so every optional branch of `Components` is
/// XSD-validated.
fn styled(version: TargetVersion) -> Document {
    let v3 = version == TargetVersion::V3_0;
    let camera = Some(Camera {
        projection: Projection::Perspective {
            field_of_view: 45.0,
        },
        view_point: Vector3::new(10.0, -10.0, 8.0),
        direction: Vector3::new(-0.5, 0.5, -0.4),
        up_vector: Vector3::new(0.0, 0.0, 1.0),
        aspect_ratio: v3.then_some(1.25),
    });
    let subject = Component {
        ifc_guid: Some("0fXw$sQh19ixbI4tZgfkXu".into()),
        originating_system: Some("Revit 2026".into()),
        authoring_tool_id: None,
    };
    let related = Component::ifc("1Qj3z6Wtb5WwCLdW5ctWxe");
    let by_tool = Component {
        authoring_tool_id: Some("ARCHICAD:77".into()),
        ..Component::default()
    };
    Document {
        version,
        extensions: None,
        topics: vec![Topic {
            guid: "5a6b7c8d-9e0f-4a1b-8c2d-3e4f5a6b7c8d".into(),
            title: "Brandschutz: Durchbruch ohne Abschottung".into(),
            topic_type: Some("Clash".into()),
            topic_status: Some("Open".into()),
            creation_date: "2026-09-28T09:00:00Z".into(),
            creation_author: "checker@example.com".into(),
            viewpoints: vec![
                // Everything at once: isolate the involved objects, colour the
                // subject red and the related objects half-transparent blue,
                // and cut the view twice.
                Viewpoint {
                    guid: "6b7c8d9e-0f1a-4b2c-9d3e-4f5a6b7c8d9e".into(),
                    selection: vec![subject.clone()],
                    camera,
                    visibility: Some(Visibility {
                        default_visibility: false,
                        exceptions: vec![subject.clone(), related.clone(), by_tool.clone()],
                    }),
                    coloring: vec![
                        Coloring {
                            color: "FF0000".into(),
                            components: vec![subject.clone()],
                        },
                        Coloring {
                            // AARRGGBB, alpha first. 3.0 also accepts lowercase.
                            color: if v3 { "800000ff" } else { "800000FF" }.into(),
                            components: vec![related.clone(), by_tool],
                        },
                    ],
                    clipping_planes: vec![
                        ClippingPlane {
                            location: Vector3::new(0.0, 0.0, 3.5),
                            direction: Vector3::new(0.0, 0.0, 1.0),
                        },
                        ClippingPlane {
                            location: Vector3::new(12.25, 0.0, 0.0),
                            direction: Vector3::new(-1.0, 0.0, 0.0),
                        },
                    ],
                },
                // Hide only the listed component; no selection.
                Viewpoint {
                    guid: "7c8d9e0f-1a2b-4c3d-8e4f-5a6b7c8d9e0f".into(),
                    camera,
                    visibility: Some(Visibility {
                        default_visibility: true,
                        exceptions: vec![related],
                    }),
                    ..Viewpoint::default()
                },
                // Colouring alone still writes the Visibility 2.1 requires.
                Viewpoint {
                    guid: "8d9e0f1a-2b3c-4d4e-9f5a-6b7c8d9e0f1a".into(),
                    camera,
                    coloring: vec![Coloring {
                        color: "00AA00".into(),
                        components: vec![subject],
                    }],
                    ..Viewpoint::default()
                },
                // Nothing visible and no exceptions; one clipping plane.
                Viewpoint {
                    guid: "9e0f1a2b-3c4d-4e5f-8a6b-7c8d9e0f1a2b".into(),
                    camera,
                    visibility: Some(Visibility {
                        default_visibility: false,
                        exceptions: Vec::new(),
                    }),
                    clipping_planes: vec![ClippingPlane {
                        location: Vector3::default(),
                        direction: Vector3::new(0.0, 1.0, 0.0),
                    }],
                    ..Viewpoint::default()
                },
            ],
            ..Topic::default()
        }],
    }
}

/// Review metadata: assignees and due dates, including one topic assigned to
/// a user no other topic uses, so the derived 3.0 `Users` list is exercised.
fn review(version: TargetVersion, extensions: Option<Extensions>) -> Document {
    let v3 = version == TargetVersion::V3_0;
    let topic =
        |guid: &str, title: &str, assigned_to: Option<&str>, due_date: Option<&str>| Topic {
            guid: guid.into(),
            title: title.into(),
            description: Some("Raised in design review.".into()),
            topic_type: Some("Info".into()),
            topic_status: Some("Open".into()),
            creation_date: "2026-09-30T08:00:00Z".into(),
            creation_author: "checker@example.com".into(),
            assigned_to: assigned_to.map(Into::into),
            due_date: due_date.map(Into::into),
            ..Topic::default()
        };
    Document {
        version,
        extensions,
        topics: vec![
            topic(
                "b1c2d3e4-f5a6-4b7c-8d9e-0f1a2b3c4d5e",
                "Stützenraster prüfen",
                Some("reviewer@example.com"),
                Some("2026-10-15T17:00:00+02:00"),
            ),
            topic(
                "c2d3e4f5-a6b7-4c8d-9e0f-1a2b3c4d5e6f",
                "Due, not yet assigned",
                None,
                Some("2026-10-31T12:00:00Z"),
            ),
            topic(
                "d3e4f5a6-b7c8-4d9e-8f1a-2b3c4d5e6f7a",
                "Assigned, no deadline",
                Some(if v3 {
                    "checker@example.com"
                } else {
                    "Planungsbüro Müller"
                }),
                None,
            ),
        ],
    }
}
