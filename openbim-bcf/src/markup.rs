//! The BCF markup model: topics, comments, and viewpoint references.
//!
//! # Verbatim by default
//!
//! Status, type, priority, stage, and user identifiers are `String`, not
//! enums. The corpus settles it: `TopicStatus` occurs as `Open`, `OPEN`,
//! `Offen`, `Active`, `ReOpened`, and `In Progress`; `TopicType` as `Error`,
//! `ERROR`, `formale Prüfung`, `Sichprüfung`, and `Clash`. Those are not typos
//! — BCF 2.x defines the vocabulary in a per-project `extensions.xsd`, so the
//! valid set is a property of the *project*, not of the format. Mapping them
//! onto a fixed enum would either reject valid files or silently rewrite them.
//!
//! Dates are also kept as written. A caller that needs `chrono`/`time` can
//! parse them; the reader reports non-conforming values as
//! [`Tolerance::UnparseableDateTime`] instead of dropping the field.

use crate::diagnostic::{Diagnostic, Tolerance};
use crate::version::BcfVersion;
use crate::xml::Node;

/// A file referenced by a topic header — the model the issue is about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderFile {
    /// `IfcProject` GUID of the referenced project, when given.
    pub ifc_project: Option<String>,
    /// `IfcSpatialStructureElement` GUID, when given.
    pub ifc_spatial_structure_element: Option<String>,
    /// The file name as written.
    pub filename: Option<String>,
    /// The file's date, verbatim.
    pub date: Option<String>,
    /// A URL or path locating the file, when given.
    pub reference: Option<String>,
    /// Whether the file lives outside the BCF archive. Absent means the
    /// schema default, `true`.
    pub is_external: Option<bool>,
}

/// A viewpoint attached to a topic: the camera and visibility state, plus its
/// snapshot.
///
/// This is the *reference*, not the parsed `.bcfv` contents. Viewpoint geometry
/// is not implemented; see the crate-level status section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewPointRef {
    /// The viewpoint's GUID.
    pub guid: Option<String>,
    /// Archive entry name of the `.bcfv` document.
    pub viewpoint: Option<String>,
    /// Archive entry name of the snapshot image.
    pub snapshot: Option<String>,
    /// Sort order among a topic's viewpoints, when given (BCF 3.0).
    pub index: Option<i32>,
}

/// One comment on a topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// The comment's GUID.
    pub guid: Option<String>,
    /// Creation timestamp, verbatim.
    pub date: Option<String>,
    /// Author identifier, verbatim.
    pub author: Option<String>,
    /// The comment text.
    pub comment: Option<String>,
    /// GUID of the viewpoint this comment is anchored to.
    pub viewpoint: Option<String>,
    /// Last-modified timestamp, verbatim.
    pub modified_date: Option<String>,
    /// Last-modifying author, verbatim.
    pub modified_author: Option<String>,
}

/// One BCF topic — a single issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topic {
    /// The topic GUID. Required by the schema; `None` is recorded as
    /// [`Tolerance::TopicWithoutGuid`].
    pub guid: Option<String>,
    /// Topic type, verbatim and project-defined.
    pub topic_type: Option<String>,
    /// Topic status, verbatim and project-defined.
    pub topic_status: Option<String>,
    /// An identifier assigned by a BCF server, when present (BCF 3.0).
    pub server_assigned_id: Option<String>,
    /// The topic title.
    pub title: Option<String>,
    /// Priority, verbatim and project-defined.
    pub priority: Option<String>,
    /// Project-defined index for ordering.
    pub index: Option<i32>,
    /// Free-form labels.
    pub labels: Vec<String>,
    /// Creation date, verbatim.
    pub creation_date: Option<String>,
    /// Creation author, verbatim.
    pub creation_author: Option<String>,
    /// Last-modified date, verbatim.
    pub modified_date: Option<String>,
    /// Last-modifying author, verbatim.
    pub modified_author: Option<String>,
    /// Due date, verbatim.
    pub due_date: Option<String>,
    /// Assignee, verbatim.
    pub assigned_to: Option<String>,
    /// Free-form description.
    pub description: Option<String>,
    /// Project-defined stage.
    pub stage: Option<String>,
    /// GUIDs of related topics.
    pub related_topics: Vec<String>,
    /// External reference links (BCF 3.0).
    pub reference_links: Vec<String>,
}

/// A parsed BCF markup document: one topic with its comments and viewpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Markup {
    /// The archive entry this markup was read from.
    pub entry: String,
    /// Files the topic refers to.
    pub header_files: Vec<HeaderFile>,
    /// The topic itself.
    pub topic: Topic,
    /// Comments, in document order.
    pub comments: Vec<Comment>,
    /// Viewpoint references, in document order.
    pub viewpoints: Vec<ViewPointRef>,
}

impl Markup {
    /// The topic title, or an empty string when the file omits it.
    #[must_use]
    pub fn title(&self) -> &str {
        self.topic.title.as_deref().unwrap_or("")
    }

    /// The topic status exactly as the file spells it.
    #[must_use]
    pub fn status(&self) -> Option<&str> {
        self.topic.topic_status.as_deref()
    }
}

/// Which BCF version's shape a markup tree looks like, from evidence only.
///
/// Only markers verified against the official `markup.xsd` of each version are
/// used. In particular `TopicStatus`/`TopicType` are **attributes in 2.0 too**
/// — a widely repeated belief that they moved in 2.1 is wrong, and keying
/// detection on it manufactures conflicts on buildingSMART's own test cases.
///
/// Returns `None` when nothing distinguishes the versions: a topic with no
/// comments and no viewpoints is valid in all three, and inventing a version
/// for it would fabricate evidence.
pub(crate) fn observe_version(markup: &Node) -> Option<BcfVersion> {
    let topic = markup.child("Topic")?;

    // --- 3.0 markers -------------------------------------------------------
    // 3.0 nests collections inside Topic and wraps their members:
    //   Topic/Comments/Comment, Topic/Viewpoints/ViewPoint,
    //   Topic/Labels/Label, Topic/DocumentReferences/DocumentReference,
    //   Topic/ReferenceLinks/ReferenceLink.
    // In 2.x, Comment and Viewpoints are siblings of Topic; Labels is a
    // repeated string; ReferenceLink is a bare element. DocumentReferences
    // exists in 2.0 too — but there it directly holds ReferencedDocument, with
    // no DocumentReference child. So the *wrapper-with-typed-member* shape is
    // the discriminator, never the container name alone.
    let nests_collections = topic.child("Comments").is_some()
        || topic.child("Viewpoints").is_some()
        || topic
            .child("Labels")
            .is_some_and(|l| l.child("Label").is_some())
        || topic
            .child("DocumentReferences")
            .is_some_and(|d| d.child("DocumentReference").is_some())
        || topic
            .child("ReferenceLinks")
            .is_some_and(|r| r.child("ReferenceLink").is_some());
    if nests_collections || topic.attr("ServerAssignedId").is_some() {
        return Some(BcfVersion::V3_0);
    }
    // A root-level Viewpoints element wrapping ViewPoint children is 3.0
    // shaped too. A *bare* Markup/Viewpoints is not: in 2.x that element **is**
    // the viewpoint, so the name alone proves nothing.
    if markup
        .children_named("Viewpoints")
        .any(|v| v.child("ViewPoint").is_some())
    {
        return Some(BcfVersion::V3_0);
    }

    // --- 2.0 markers -------------------------------------------------------
    // 2.0's Comment type carries Status, VerbalStatus, ReplyToComment, and a
    // mandatory back-reference Topic. 2.1 removed all four. Any one of them
    // inside a Comment is decisive; nothing else distinguishes 2.0 from 2.1.
    let comments_are_2_0 = markup.children_named("Comment").any(|c| {
        c.child("Topic").is_some()
            || c.child("Status").is_some()
            || c.child("VerbalStatus").is_some()
            || c.child("ReplyToComment").is_some()
    });
    if comments_are_2_0 {
        return Some(BcfVersion::V2_0);
    }

    // --- 2.x, version indeterminate ---------------------------------------
    // A 2.x file with no comments carries no evidence separating 2.0 from 2.1:
    // both spell Topic, its attributes, and Viewpoints identically. Claiming
    // either would be a guess dressed as an observation, so observe nothing and
    // let a declared bcf.version stand unchallenged.
    None
}

/// Interpret a markup tree, appending a diagnostic for each tolerated
/// deviation.
pub(crate) fn interpret(
    entry: &str,
    root: &Node,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Markup> {
    let Some(topic_node) = root.child("Topic") else {
        diagnostics.push(Diagnostic::in_entry(entry, Tolerance::MarkupWithoutTopic));
        return None;
    };

    let topic = read_topic(entry, topic_node, diagnostics);

    let header_files = root
        .child("Header")
        .into_iter()
        .flat_map(|h| h.children_named("File"))
        .map(read_header_file)
        .collect();

    // Viewpoint references live in three shapes across versions:
    //   2.x   Markup/Viewpoints            (the element *is* the ViewPoint)
    //   3.0   Topic/Viewpoints/ViewPoint   (wrapper inside Topic)
    //   ---   Markup/Viewpoints/ViewPoint  (wrapper at root; emitted by some
    //         writers migrating to 3.0)
    // All three are read, so a hybrid file still yields all its viewpoints.
    let mut viewpoints: Vec<ViewPointRef> = Vec::new();
    for wrapper in root
        .children_named("Viewpoints")
        .chain(topic_node.children_named("Viewpoints"))
    {
        if wrapper.child("ViewPoint").is_some() {
            viewpoints.extend(wrapper.children_named("ViewPoint").map(read_viewpoint));
        } else if !wrapper.children.is_empty() || wrapper.attr("Guid").is_some() {
            // An empty <Viewpoints/> in 3.0 means "no viewpoints", not "one
            // blank viewpoint"; only a populated 2.x element is a reference.
            viewpoints.push(read_viewpoint(wrapper));
        }
    }

    // Comments: 2.x lists them under Markup, 3.0 nests them in Topic/Comments.
    let comments = root
        .children_named("Comment")
        .chain(
            topic_node
                .child("Comments")
                .into_iter()
                .flat_map(|c| c.children_named("Comment")),
        )
        .map(|c| read_comment(entry, c, diagnostics))
        .collect();

    Some(Markup {
        entry: entry.to_string(),
        header_files,
        topic,
        comments,
        viewpoints,
    })
}

fn owned(v: Option<&str>) -> Option<String> {
    v.map(str::to_string)
}

fn read_topic(entry: &str, node: &Node, diagnostics: &mut Vec<Diagnostic>) -> Topic {
    let guid = owned(node.attr("Guid").map(str::trim).filter(|g| !g.is_empty()));
    if guid.is_none() {
        diagnostics.push(Diagnostic::in_entry(entry, Tolerance::TopicWithoutGuid));
    }
    let title = owned(node.child_text("Title"));
    if title.is_none() {
        diagnostics.push(Diagnostic::in_entry(entry, Tolerance::TopicWithoutTitle));
    }

    Topic {
        guid,
        topic_type: owned(node.attr_or_child_text("TopicType")),
        topic_status: owned(node.attr_or_child_text("TopicStatus")),
        server_assigned_id: owned(node.attr_or_child_text("ServerAssignedId")),
        title,
        priority: owned(node.child_text("Priority")),
        index: node.child_text("Index").and_then(|i| i.parse().ok()),
        // 2.x: repeated <Labels>text</Labels>. 3.0: <Labels><Label>text</Label></Labels>.
        labels: node
            .children_named("Labels")
            .flat_map(|l| {
                let nested: Vec<&str> = l
                    .children_named("Label")
                    .map(|n| n.text.trim())
                    .filter(|t| !t.is_empty())
                    .collect();
                if nested.is_empty() {
                    let own = l.text.trim();
                    if own.is_empty() {
                        Vec::new()
                    } else {
                        vec![own]
                    }
                } else {
                    nested
                }
            })
            .map(str::to_string)
            .collect(),
        creation_date: owned(node.child_text("CreationDate")),
        creation_author: owned(node.child_text("CreationAuthor")),
        modified_date: owned(node.child_text("ModifiedDate")),
        modified_author: owned(node.child_text("ModifiedAuthor")),
        due_date: owned(node.child_text("DueDate")),
        assigned_to: owned(node.child_text("AssignedTo")),
        description: owned(node.child_text("Description")),
        stage: owned(node.child_text("Stage")),
        related_topics: node
            .children_named("RelatedTopic")
            .chain(
                node.child("RelatedTopics")
                    .into_iter()
                    .flat_map(|r| r.children_named("RelatedTopic")),
            )
            .filter_map(|r| owned(r.attr("Guid")))
            .collect(),
        reference_links: node
            .children_named("ReferenceLink")
            .chain(
                node.child("ReferenceLinks")
                    .into_iter()
                    .flat_map(|r| r.children_named("ReferenceLink")),
            )
            .filter_map(|r| {
                let t = r.text.trim();
                (!t.is_empty()).then(|| t.to_string())
            })
            .collect(),
    }
}

fn read_header_file(node: &Node) -> HeaderFile {
    HeaderFile {
        ifc_project: owned(node.attr("IfcProject")),
        ifc_spatial_structure_element: owned(node.attr("IfcSpatialStructureElement")),
        filename: owned(node.child_text("Filename")),
        date: owned(node.child_text("Date")),
        reference: owned(node.child_text("Reference")),
        is_external: node.attr("IsExternal").and_then(parse_bool),
    }
}

fn read_viewpoint(node: &Node) -> ViewPointRef {
    ViewPointRef {
        guid: owned(node.attr("Guid")),
        viewpoint: owned(node.child_text("Viewpoint")),
        snapshot: owned(node.child_text("Snapshot")),
        index: node.child_text("Index").and_then(|i| i.parse().ok()),
    }
}

fn read_comment(entry: &str, node: &Node, diagnostics: &mut Vec<Diagnostic>) -> Comment {
    let guid = owned(node.attr("Guid").map(str::trim).filter(|g| !g.is_empty()));
    if guid.is_none() {
        diagnostics.push(Diagnostic::in_entry(entry, Tolerance::CommentWithoutGuid));
    }
    Comment {
        guid,
        date: owned(node.child_text("Date")),
        author: owned(node.child_text("Author")),
        comment: owned(node.child_text("Comment")),
        // 2.0 writes <Viewpoint Guid="..."/>; 2.1+ keeps the same shape.
        viewpoint: node
            .child("Viewpoint")
            .and_then(|v| owned(v.attr("Guid")))
            .or_else(|| owned(node.child_text("Viewpoint"))),
        modified_date: owned(node.child_text("ModifiedDate")),
        modified_author: owned(node.child_text("ModifiedAuthor")),
    }
}

/// `xs:boolean` accepts `true`/`false`/`1`/`0`. Writers in the field also emit
/// `True`/`False`, so matching is case-insensitive.
fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xml::parse;

    fn tree(src: &str) -> Node {
        parse(src.as_bytes()).unwrap()
    }

    #[test]
    fn detects_2_0_from_a_comment_back_reference() {
        let t = tree(
            r#"<Markup><Topic Guid="a"/><Comment Guid="c"><Topic Guid="a"/></Comment></Markup>"#,
        );
        assert_eq!(observe_version(&t), Some(BcfVersion::V2_0));
    }

    /// The other three 2.0-only `Comment` members are equally decisive.
    #[test]
    fn detects_2_0_from_comment_only_members() {
        for child in [
            "<Status>Open</Status>",
            "<VerbalStatus>Open</VerbalStatus>",
            r#"<ReplyToComment Guid="x"/>"#,
        ] {
            let t = tree(&format!(
                r#"<Markup><Topic Guid="a" TopicStatus="Open"/><Comment Guid="c">{child}</Comment></Markup>"#
            ));
            assert_eq!(observe_version(&t), Some(BcfVersion::V2_0), "for {child}");
        }
    }

    /// Regression against a myth: `TopicStatus`/`TopicType` are `Topic`
    /// **attributes** in BCF 2.0 and 2.1 alike — verified against both official
    /// `markup.xsd` files. Treating the attribute form as 2.1-only reported
    /// three of buildingSMART's own v2.0 test cases as version conflicts.
    #[test]
    fn attribute_status_is_not_evidence_against_2_0() {
        let t = tree(
            r#"<Markup><Topic Guid="a" TopicType="Information" TopicStatus="Open"><Title>T</Title></Topic></Markup>"#,
        );
        assert_eq!(observe_version(&t), None);
        // A file declaring 2.0 must therefore stay Declared, not Conflict.
        assert_eq!(
            BcfVersion::reconcile(Some(BcfVersion::V2_0), observe_version(&t)),
            openbim_core::Detected::Declared(BcfVersion::V2_0)
        );
    }

    /// `DocumentReferences` exists in 2.0 as well; only the 3.0 wrapper shape
    /// (a `DocumentReference` child) is evidence.
    #[test]
    fn a_2_0_document_references_element_is_not_a_3_0_wrapper() {
        let t = tree(
            r#"<Markup><Topic Guid="a" TopicStatus="Open"><DocumentReferences Guid="d"><ReferencedDocument>a.pdf</ReferencedDocument></DocumentReferences></Topic></Markup>"#,
        );
        assert_eq!(observe_version(&t), None);
    }

    #[test]
    fn detects_3_0_from_viewpoints_wrapper_or_server_id() {
        let wrapped = tree(
            r#"<Markup><Topic Guid="a" TopicStatus="Open"/><Viewpoints><ViewPoint Guid="v"/></Viewpoints></Markup>"#,
        );
        assert_eq!(observe_version(&wrapped), Some(BcfVersion::V3_0));

        let server = tree(r#"<Markup><Topic Guid="a" ServerAssignedId="7"/></Markup>"#);
        assert_eq!(observe_version(&server), Some(BcfVersion::V3_0));
    }

    /// Regression: BCF 2.1's `<Viewpoints>` *is* the viewpoint, while 3.0's is
    /// a wrapper around `<ViewPoint>`. Keying detection on the element name
    /// alone misread every 2.1 file with a viewpoint as 3.0 — which then
    /// manufactured a spurious version conflict.
    #[test]
    fn a_2_x_viewpoints_element_is_not_a_3_0_wrapper() {
        let t = tree(
            r#"<Markup><Topic Guid="a" TopicStatus="Open"/><Viewpoints Guid="v"><Viewpoint>v.bcfv</Viewpoint><Snapshot>s.png</Snapshot></Viewpoints></Markup>"#,
        );
        // 2.0 and 2.1 spell this identically, so no version is observable.
        assert_eq!(observe_version(&t), None);
    }

    /// Regression, caught on 4 official buildingSMART 3.0 test cases: 3.0 moves
    /// `Comments`, `Viewpoints`, `Labels`, and `DocumentReferences` *inside*
    /// `Topic`. Looking only at `Markup` children read those files as 2.1 and
    /// reported a false conflict against their own `bcf.version`.
    #[test]
    fn detects_3_0_from_collections_nested_inside_topic() {
        for child in [
            "<Comments/>",
            "<Comments><Comment Guid=\"c\"/></Comments>",
            "<Viewpoints/>",
            "<Viewpoints><ViewPoint Guid=\"v\"/></Viewpoints>",
            "<Labels><Label>x</Label></Labels>",
            "<DocumentReferences><DocumentReference Guid=\"d\"/></DocumentReferences>",
            "<ReferenceLinks><ReferenceLink>http://x</ReferenceLink></ReferenceLinks>",
        ] {
            let t = tree(&format!(
                r#"<Markup><Topic Guid="a" TopicStatus="OPEN" TopicType="ERROR"><Title>T</Title>{child}</Topic></Markup>"#
            ));
            assert_eq!(observe_version(&t), Some(BcfVersion::V3_0), "for {child}");
        }
    }

    /// The 2.x `<Labels>text</Labels>` shape must not be mistaken for 3.0.
    #[test]
    fn bare_2_x_labels_are_not_evidence_of_3_0() {
        let t = tree(
            r#"<Markup><Topic Guid="a" TopicStatus="Open"><Labels>Architecture</Labels></Topic></Markup>"#,
        );
        assert_eq!(observe_version(&t), None);
    }

    #[test]
    fn reads_comments_and_viewpoints_nested_in_a_3_0_topic() {
        let mut d = Vec::new();
        let m = interpret(
            "e",
            &tree(
                r#"<Markup><Topic Guid="a"><Title>T</Title><Labels><Label>L1</Label><Label>L2</Label></Labels><Comments><Comment Guid="c1"><Comment>hello</Comment></Comment></Comments><Viewpoints><ViewPoint Guid="v1"><Viewpoint>v.bcfv</Viewpoint></ViewPoint></Viewpoints></Topic></Markup>"#,
            ),
            &mut d,
        )
        .unwrap();
        assert_eq!(m.comments.len(), 1);
        assert_eq!(m.comments[0].comment.as_deref(), Some("hello"));
        assert_eq!(m.viewpoints.len(), 1);
        assert_eq!(m.viewpoints[0].viewpoint.as_deref(), Some("v.bcfv"));
        assert_eq!(m.topic.labels, ["L1", "L2"]);
        assert!(d.is_empty(), "{d:?}");
    }

    /// An empty `<Viewpoints/>` in 3.0 means "none", not one blank viewpoint.
    #[test]
    fn an_empty_viewpoints_wrapper_yields_no_viewpoints() {
        let mut d = Vec::new();
        let m = interpret(
            "e",
            &tree(r#"<Markup><Topic Guid="a"><Title>T</Title><Viewpoints/><Comments/></Topic></Markup>"#),
            &mut d,
        )
        .unwrap();
        assert!(m.viewpoints.is_empty(), "{:?}", m.viewpoints);
        assert!(m.comments.is_empty());
    }

    #[test]
    fn indistinguishable_markup_observes_nothing() {
        let t = tree(r#"<Markup><Topic Guid="a"><Title>x</Title></Topic></Markup>"#);
        assert_eq!(observe_version(&t), None);
        assert_eq!(observe_version(&tree("<Markup/>")), None);
    }

    #[test]
    fn status_and_type_are_kept_verbatim_not_normalised() {
        let mut d = Vec::new();
        let t = tree(
            r#"<Markup><Topic Guid="a" TopicStatus="Offen" TopicType="formale Prüfung"><Title>T</Title></Topic></Markup>"#,
        );
        let m = interpret("x/markup.bcf", &t, &mut d).unwrap();
        assert_eq!(m.status(), Some("Offen"));
        assert_eq!(m.topic.topic_type.as_deref(), Some("formale Prüfung"));
        assert!(d.is_empty(), "{d:?}");
    }

    #[test]
    fn missing_guid_and_title_are_diagnosed_but_not_fatal() {
        let mut d = Vec::new();
        let m = interpret("x/markup.bcf", &tree("<Markup><Topic/></Markup>"), &mut d).unwrap();
        assert_eq!(m.topic.guid, None);
        assert_eq!(m.title(), "");
        let kinds: Vec<_> = d.iter().map(|x| x.tolerance.clone()).collect();
        assert!(kinds.contains(&Tolerance::TopicWithoutGuid), "{kinds:?}");
        assert!(kinds.contains(&Tolerance::TopicWithoutTitle), "{kinds:?}");
    }

    #[test]
    fn markup_without_a_topic_yields_nothing_and_says_so() {
        let mut d = Vec::new();
        assert!(interpret("x", &tree("<Markup/>"), &mut d).is_none());
        assert_eq!(d[0].tolerance, Tolerance::MarkupWithoutTopic);
    }

    #[test]
    fn reads_both_wrapped_and_unwrapped_viewpoint_references() {
        let mut d = Vec::new();
        let v3 = interpret(
            "e",
            &tree(
                r#"<Markup><Topic Guid="a"/><Viewpoints><ViewPoint Guid="v1"><Viewpoint>v1.bcfv</Viewpoint><Snapshot>s1.png</Snapshot><Index>2</Index></ViewPoint></Viewpoints></Markup>"#,
            ),
            &mut d,
        )
        .unwrap();
        assert_eq!(v3.viewpoints.len(), 1);
        assert_eq!(v3.viewpoints[0].guid.as_deref(), Some("v1"));
        assert_eq!(v3.viewpoints[0].snapshot.as_deref(), Some("s1.png"));
        assert_eq!(v3.viewpoints[0].index, Some(2));

        let v2 = interpret(
            "e",
            &tree(
                r#"<Markup><Topic Guid="a"/><Viewpoints Guid="v1"><Viewpoint>v1.bcfv</Viewpoint></Viewpoints><Viewpoints Guid="v2"><Viewpoint>v2.bcfv</Viewpoint></Viewpoints></Markup>"#,
            ),
            &mut d,
        )
        .unwrap();
        assert_eq!(v2.viewpoints.len(), 2);
        assert_eq!(v2.viewpoints[1].viewpoint.as_deref(), Some("v2.bcfv"));
    }

    #[test]
    fn comments_keep_order_and_anchor_guid() {
        let mut d = Vec::new();
        let m = interpret(
            "e",
            &tree(
                r#"<Markup><Topic Guid="a"/><Comment Guid="c1"><Comment>first</Comment><Viewpoint Guid="v1"/></Comment><Comment Guid="c2"><Comment>second</Comment></Comment></Markup>"#,
            ),
            &mut d,
        )
        .unwrap();
        assert_eq!(m.comments.len(), 2);
        assert_eq!(m.comments[0].comment.as_deref(), Some("first"));
        assert_eq!(m.comments[0].viewpoint.as_deref(), Some("v1"));
        assert_eq!(m.comments[1].comment.as_deref(), Some("second"));
    }

    #[test]
    fn header_files_read_attributes_and_boolean_spellings() {
        let mut d = Vec::new();
        let m = interpret(
            "e",
            &tree(
                r#"<Markup><Header><File IfcProject="p1" IsExternal="False"><Filename>a.ifc</Filename><Date>2015-04-14T15:51:25Z</Date></File><File IsExternal="1"><Filename>b.ifc</Filename></File></Header><Topic Guid="a"/></Markup>"#,
            ),
            &mut d,
        )
        .unwrap();
        assert_eq!(m.header_files.len(), 2);
        assert_eq!(m.header_files[0].ifc_project.as_deref(), Some("p1"));
        assert_eq!(m.header_files[0].is_external, Some(false));
        assert_eq!(
            m.header_files[0].date.as_deref(),
            Some("2015-04-14T15:51:25Z")
        );
        assert_eq!(m.header_files[1].is_external, Some(true));
    }

    #[test]
    fn related_topics_and_labels_read_in_both_2_x_and_3_0_shapes() {
        let mut d = Vec::new();
        let m = interpret(
            "e",
            &tree(
                r#"<Markup><Topic Guid="a"><Labels>L1</Labels><Labels>L2</Labels><RelatedTopic Guid="r1"/><RelatedTopics><RelatedTopic Guid="r2"/></RelatedTopics><ReferenceLinks><ReferenceLink>http://x/1</ReferenceLink></ReferenceLinks></Topic></Markup>"#,
            ),
            &mut d,
        )
        .unwrap();
        assert_eq!(m.topic.labels, ["L1", "L2"]);
        assert_eq!(m.topic.related_topics, ["r1", "r2"]);
        assert_eq!(m.topic.reference_links, ["http://x/1"]);
    }

    #[test]
    fn boolean_parsing_rejects_nonsense_rather_than_defaulting() {
        assert_eq!(parse_bool("TRUE"), Some(true));
        assert_eq!(parse_bool(" 0 "), Some(false));
        assert_eq!(parse_bool("yes"), None);
        assert_eq!(parse_bool(""), None);
    }
}
