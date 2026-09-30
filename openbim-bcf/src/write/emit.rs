//! Turning a checked [`Document`] into archive entries, and entries into ZIP
//! bytes.
//!
//! Validation and emission walk the document together, so no field can be
//! written without having been checked. All entries are built in memory
//! before the ZIP encoder sees any of them: an invalid document writes
//! nothing.

use super::{
    check, Camera, Comment, Compression, Document, Extensions, Invalid, Options, Projection,
    TargetVersion, Topic, Vector3, Viewpoint, Visibility, WriteError,
};
use crate::markup::Component;
use std::collections::HashSet;
use std::io::{Seek, Write};

/// `(entry name, bytes)` in archive order.
pub(super) type Entries = Vec<(String, Vec<u8>)>;

fn bad(at: impl Into<String>, problem: Invalid) -> WriteError {
    WriteError::Invalid {
        at: at.into(),
        problem,
    }
}

/// Check `doc` and render every archive entry.
pub(super) fn entries(doc: &Document) -> Result<Entries, WriteError> {
    if doc.topics.is_empty() {
        return Err(bad("topics", Invalid::NoTopics));
    }
    if let Some(ext) = &doc.extensions {
        check_extensions(ext)?;
    }

    let mut ctx = Ctx {
        version: doc.version,
        extensions: doc.extensions.as_ref(),
        seen: HashSet::new(),
    };
    let mut topic_entries = Vec::new();
    for (i, topic) in doc.topics.iter().enumerate() {
        ctx.topic(&format!("topics[{i}]"), topic, &mut topic_entries)?;
    }

    let mut out = vec![("bcf.version".to_string(), version_xml(doc.version))];
    if doc.version == TargetVersion::V3_0 {
        let derived;
        let ext = if let Some(ext) = &doc.extensions {
            ext
        } else {
            derived = derive_extensions(&doc.topics);
            &derived
        };
        out.push(("extensions.xml".to_string(), extensions_xml(ext)));
    }
    out.extend(topic_entries);
    Ok(out)
}

/// Refuse options the encoder would reject or silently reinterpret.
pub(super) fn check_options(options: Options) -> Result<(), WriteError> {
    match options.compression {
        Compression::Stored => Ok(()),
        // 1..=9 is flate2's range. The encoder would also accept 10..=264,
        // but routes those to Zopfli: a different codec, not a higher level.
        Compression::Deflated { level } if (1..=9).contains(&level) => Ok(()),
        Compression::Deflated { level } => Err(bad(
            "options.compression.level",
            Invalid::Number {
                value: f64::from(level),
                expected: "a deflate level within 1..=9",
            },
        )),
    }
}

/// Encode entries as a ZIP with fixed metadata.
pub(super) fn zip<W: Write + Seek>(
    entries: &Entries,
    sink: W,
    options: Options,
) -> Result<W, WriteError> {
    // Every field that could vary between runs or machines is pinned:
    // timestamp (the crate's `time` feature would otherwise default it to
    // now), permissions, compression method, and level. See the module docs
    // for why stored is the default.
    let (method, level) = match options.compression {
        Compression::Stored => (zip::CompressionMethod::Stored, None),
        Compression::Deflated { level } => {
            (zip::CompressionMethod::Deflated, Some(i64::from(level)))
        }
    };
    let file_options = zip::write::SimpleFileOptions::default()
        .compression_method(method)
        .compression_level(level)
        .last_modified_time(zip::DateTime::default())
        .unix_permissions(0o644)
        .large_file(false);
    let mut writer = zip::ZipWriter::new(sink);
    for (name, bytes) in entries {
        writer
            .start_file(name.as_str(), file_options)
            .map_err(zip_err)?;
        writer.write_all(bytes)?;
    }
    writer.finish().map_err(zip_err)
}

fn zip_err(e: zip::result::ZipError) -> WriteError {
    match e {
        zip::result::ZipError::Io(source) => WriteError::Io(source),
        other => WriteError::Zip {
            detail: other.to_string(),
        },
    }
}

struct Ctx<'d> {
    version: TargetVersion,
    extensions: Option<&'d Extensions>,
    /// Lowercased GUIDs seen so far, across the whole document.
    seen: HashSet<String>,
}

impl Ctx<'_> {
    fn guid(&mut self, at: &str, value: &str) -> Result<(), WriteError> {
        check::guid(value, self.version).map_err(|p| bad(at, p))?;
        if !self.seen.insert(value.to_ascii_lowercase()) {
            return Err(bad(
                at,
                Invalid::DuplicateGuid {
                    value: value.to_string(),
                },
            ));
        }
        Ok(())
    }

    /// A value that, when extensions are supplied, must be listed in them.
    fn listed(
        &self,
        at: &str,
        value: &str,
        list: impl Fn(&Extensions) -> &Vec<String>,
    ) -> Result<(), WriteError> {
        check::text(value).map_err(|p| bad(at, p))?;
        match self.extensions {
            Some(ext) if !list(ext).iter().any(|v| v == value) => Err(bad(
                at,
                Invalid::NotInExtensions {
                    value: value.to_string(),
                },
            )),
            _ => Ok(()),
        }
    }

    fn topic(&mut self, at: &str, t: &Topic, out: &mut Entries) -> Result<(), WriteError> {
        self.guid(&format!("{at}.guid"), &t.guid)?;
        text(&format!("{at}.title"), &t.title)?;
        for (field, value, list) in [
            ("topic_type", &t.topic_type, list_types as ListFn),
            ("topic_status", &t.topic_status, list_statuses),
        ] {
            match value {
                Some(v) => self.listed(&format!("{at}.{field}"), v, list)?,
                // Optional attributes in 2.1; `use="required"` in 3.0.
                None if self.version == TargetVersion::V3_0 => {
                    return Err(bad(format!("{at}.{field}"), Invalid::Missing));
                }
                None => {}
            }
        }
        if let Some(p) = &t.priority {
            self.listed(&format!("{at}.priority"), p, |e| &e.priorities)?;
        }
        for (i, label) in t.labels.iter().enumerate() {
            self.listed(&format!("{at}.labels[{i}]"), label, |e| &e.topic_labels)?;
        }
        date(&format!("{at}.creation_date"), &t.creation_date)?;
        text(&format!("{at}.creation_author"), &t.creation_author)?;
        if let Some(d) = &t.due_date {
            date(&format!("{at}.due_date"), d)?;
        }
        if let Some(a) = &t.assigned_to {
            self.listed(&format!("{at}.assigned_to"), a, |e| &e.users)?;
        }
        if let Some(d) = &t.description {
            text(&format!("{at}.description"), d)?;
        }

        // Viewpoints first, so comment anchors can be checked against them.
        let mut viewpoint_entries = Vec::new();
        for (i, vp) in t.viewpoints.iter().enumerate() {
            let bytes = self.viewpoint(&format!("{at}.viewpoints[{i}]"), vp)?;
            viewpoint_entries.push((format!("{}/{}", t.guid, viewpoint_file(vp)), bytes));
            // Directly after its .bcfv, verbatim.
            if let Some(s) = &vp.snapshot {
                viewpoint_entries
                    .push((format!("{}/{}", t.guid, snapshot_file(vp)), s.png.clone()));
            }
        }
        for (i, c) in t.comments.iter().enumerate() {
            self.comment(&format!("{at}.comments[{i}]"), c, &t.viewpoints)?;
        }

        out.push((
            format!("{}/markup.bcf", t.guid),
            markup_xml(self.version, t),
        ));
        out.extend(viewpoint_entries);
        Ok(())
    }

    fn comment(
        &mut self,
        at: &str,
        c: &Comment,
        viewpoints: &[Viewpoint],
    ) -> Result<(), WriteError> {
        self.guid(&format!("{at}.guid"), &c.guid)?;
        date(&format!("{at}.date"), &c.date)?;
        text(&format!("{at}.author"), &c.author)?;
        text(&format!("{at}.comment"), &c.comment)?;
        if let Some(anchor) = &c.viewpoint {
            if !viewpoints.iter().any(|v| &v.guid == anchor) {
                return Err(bad(
                    format!("{at}.viewpoint"),
                    Invalid::UnknownViewpoint {
                        guid: anchor.clone(),
                    },
                ));
            }
        }
        Ok(())
    }

    fn viewpoint(&mut self, at: &str, vp: &Viewpoint) -> Result<Vec<u8>, WriteError> {
        self.guid(&format!("{at}.guid"), &vp.guid)?;
        if let Some(s) = &vp.snapshot {
            if !s.png.starts_with(PNG_SIGNATURE) {
                return Err(bad(format!("{at}.snapshot"), Invalid::Snapshot));
            }
        }
        for (i, c) in vp.selection.iter().enumerate() {
            component(&format!("{at}.selection[{i}]"), c)?;
        }
        if let Some(v) = &vp.visibility {
            for (i, c) in v.exceptions.iter().enumerate() {
                component(&format!("{at}.visibility.exceptions[{i}]"), c)?;
            }
        }
        for (i, coloring) in vp.coloring.iter().enumerate() {
            let at = format!("{at}.coloring[{i}]");
            check::color(&coloring.color, self.version)
                .map_err(|p| bad(format!("{at}.color"), p))?;
            // 2.1 `Color` and 3.0 `Color/Components` both need a Component.
            if coloring.components.is_empty() {
                return Err(bad(format!("{at}.components"), Invalid::NoComponents));
            }
            for (j, c) in coloring.components.iter().enumerate() {
                component(&format!("{at}.components[{j}]"), c)?;
            }
        }
        for (i, plane) in vp.clipping_planes.iter().enumerate() {
            let at = format!("{at}.clipping_planes[{i}]");
            vector(&format!("{at}.location"), plane.location, false)?;
            vector(&format!("{at}.direction"), plane.direction, true)?;
        }
        match &vp.camera {
            Some(camera) => self.camera(&format!("{at}.camera"), camera)?,
            // 3.0's VisualizationInfo makes the camera a mandatory choice.
            None if self.version == TargetVersion::V3_0 => {
                return Err(bad(format!("{at}.camera"), Invalid::Missing));
            }
            None => {}
        }
        Ok(visinfo_xml(self.version, vp))
    }

    fn camera(&self, at: &str, c: &Camera) -> Result<(), WriteError> {
        vector(&format!("{at}.view_point"), c.view_point, false)?;
        vector(&format!("{at}.direction"), c.direction, true)?;
        vector(&format!("{at}.up_vector"), c.up_vector, true)?;
        match c.projection {
            Projection::Perspective { field_of_view } => {
                let at = format!("{at}.projection.field_of_view");
                check::finite(field_of_view).map_err(|p| bad(&at, p))?;
                let (ok, expected) = match self.version {
                    TargetVersion::V2_1 => (
                        (45.0..=60.0).contains(&field_of_view),
                        "within 45..=60 degrees",
                    ),
                    TargetVersion::V3_0 => (
                        field_of_view > 0.0 && field_of_view < 180.0,
                        "strictly between 0 and 180 degrees",
                    ),
                };
                if !ok {
                    return Err(bad(
                        at,
                        Invalid::Number {
                            value: field_of_view,
                            expected,
                        },
                    ));
                }
            }
            Projection::Orthogonal {
                view_to_world_scale,
            } => {
                positive(
                    &format!("{at}.projection.view_to_world_scale"),
                    view_to_world_scale,
                )?;
            }
        }
        let at = format!("{at}.aspect_ratio");
        match (self.version, c.aspect_ratio) {
            (TargetVersion::V3_0, Some(r)) => positive(&at, r),
            (TargetVersion::V3_0, None) => Err(bad(at, Invalid::Missing)),
            (TargetVersion::V2_1, Some(_)) => Err(bad(
                at,
                Invalid::NotInVersion {
                    version: TargetVersion::V2_1,
                },
            )),
            (TargetVersion::V2_1, None) => Ok(()),
        }
    }
}

type ListFn = fn(&Extensions) -> &Vec<String>;

fn list_types(e: &Extensions) -> &Vec<String> {
    &e.topic_types
}

fn list_statuses(e: &Extensions) -> &Vec<String> {
    &e.topic_statuses
}

fn text(at: &str, value: &str) -> Result<(), WriteError> {
    check::text(value).map_err(|p| bad(at, p))
}

fn date(at: &str, value: &str) -> Result<(), WriteError> {
    check::date_time(value).map_err(|p| bad(at, p))
}

fn positive(at: &str, value: f64) -> Result<(), WriteError> {
    check::finite(value).map_err(|p| bad(at, p))?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(bad(
            at,
            Invalid::Number {
                value,
                expected: "positive",
            },
        ))
    }
}

fn vector(at: &str, v: Vector3, non_zero: bool) -> Result<(), WriteError> {
    for (axis, value) in [("x", v.x), ("y", v.y), ("z", v.z)] {
        check::finite(value).map_err(|p| bad(format!("{at}.{axis}"), p))?;
    }
    if non_zero && v.x == 0.0 && v.y == 0.0 && v.z == 0.0 {
        return Err(bad(
            at,
            Invalid::Number {
                value: 0.0,
                expected: "part of a non-zero vector",
            },
        ));
    }
    Ok(())
}

fn component(at: &str, c: &Component) -> Result<(), WriteError> {
    if c.ifc_guid.is_none() && c.authoring_tool_id.is_none() {
        return Err(bad(at, Invalid::UnidentifiedComponent));
    }
    if let Some(g) = &c.ifc_guid {
        check::ifc_guid(g).map_err(|p| bad(format!("{at}.ifc_guid"), p))?;
    }
    if let Some(s) = &c.originating_system {
        text(&format!("{at}.originating_system"), s)?;
    }
    if let Some(id) = &c.authoring_tool_id {
        text(&format!("{at}.authoring_tool_id"), id)?;
    }
    Ok(())
}

fn check_extensions(ext: &Extensions) -> Result<(), WriteError> {
    for (field, list) in extension_lists(ext) {
        for (i, value) in list.iter().enumerate() {
            text(&format!("extensions.{field}[{i}]"), value)?;
        }
    }
    Ok(())
}

fn extension_lists(ext: &Extensions) -> [(&'static str, &Vec<String>); 7] {
    [
        ("topic_types", &ext.topic_types),
        ("topic_statuses", &ext.topic_statuses),
        ("priorities", &ext.priorities),
        ("topic_labels", &ext.topic_labels),
        ("users", &ext.users),
        ("snippet_types", &ext.snippet_types),
        ("stages", &ext.stages),
    ]
}

/// The vocabulary a 3.0 archive's topics actually use, in order of first use.
fn derive_extensions(topics: &[Topic]) -> Extensions {
    fn push(list: &mut Vec<String>, value: &str) {
        if !list.iter().any(|v| v == value) {
            list.push(value.to_string());
        }
    }
    let mut ext = Extensions::default();
    for t in topics {
        if let Some(v) = &t.topic_type {
            push(&mut ext.topic_types, v);
        }
        if let Some(v) = &t.topic_status {
            push(&mut ext.topic_statuses, v);
        }
        if let Some(v) = &t.priority {
            push(&mut ext.priorities, v);
        }
        for v in &t.labels {
            push(&mut ext.topic_labels, v);
        }
        if let Some(v) = &t.assigned_to {
            push(&mut ext.users, v);
        }
    }
    ext
}

/// The eight bytes every PNG file starts with (PNG specification, 5.2).
const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

fn snapshot_file(vp: &Viewpoint) -> String {
    // The naming the official 3.0 test archives use.
    format!("Snapshot_{}.png", vp.guid)
}

fn viewpoint_file(vp: &Viewpoint) -> String {
    // The naming the official 3.0 test archives use.
    format!("Viewpoint_{}.bcfv", vp.guid)
}

// --- XML ------------------------------------------------------------------

/// A minimal indenting XML emitter. Text goes inline in its element, so the
/// indentation never becomes part of a value.
struct Xml {
    out: String,
    depth: usize,
}

impl Xml {
    fn new() -> Self {
        Self {
            out: String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"),
            depth: 0,
        }
    }

    fn start_tag(&mut self, name: &str, attrs: &[(&str, &str)]) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
        self.out.push('<');
        self.out.push_str(name);
        for (k, v) in attrs {
            self.out.push(' ');
            self.out.push_str(k);
            self.out.push_str("=\"");
            escape_attr(&mut self.out, v);
            self.out.push('"');
        }
    }

    fn open(&mut self, name: &str, attrs: &[(&str, &str)]) {
        self.start_tag(name, attrs);
        self.out.push_str(">\n");
        self.depth += 1;
    }

    fn close(&mut self, name: &str) {
        self.depth -= 1;
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push_str(">\n");
    }

    fn empty(&mut self, name: &str, attrs: &[(&str, &str)]) {
        self.start_tag(name, attrs);
        self.out.push_str("/>\n");
    }

    fn leaf(&mut self, name: &str, text: &str) {
        self.start_tag(name, &[]);
        self.out.push('>');
        escape_text(&mut self.out, text);
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push_str(">\n");
    }

    fn number(&mut self, name: &str, value: f64) {
        // Rust's `Display` for f64 is the shortest string that round-trips,
        // never uses an exponent, and is a valid `xs:double` lexeme.
        self.leaf(name, &value.to_string());
    }

    fn vector(&mut self, name: &str, v: Vector3) {
        self.open(name, &[]);
        self.number("X", v.x);
        self.number("Y", v.y);
        self.number("Z", v.z);
        self.close(name);
    }

    fn finish(self) -> Vec<u8> {
        self.out.into_bytes()
    }
}

/// Escape character data. `\r` becomes a character reference because XML
/// parsers normalise a literal CR away, which would break the round trip.
fn escape_text(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#xD;"),
            c => out.push(c),
        }
    }
}

/// Escape an attribute value. Tab and line breaks are character references:
/// attribute-value normalisation would otherwise turn them into spaces.
fn escape_attr(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\t' => out.push_str("&#x9;"),
            '\n' => out.push_str("&#xA;"),
            '\r' => out.push_str("&#xD;"),
            c => out.push(c),
        }
    }
}

fn version_xml(version: TargetVersion) -> Vec<u8> {
    let mut x = Xml::new();
    match version {
        TargetVersion::V2_1 => {
            x.open("Version", &[("VersionId", "2.1")]);
            x.leaf("DetailedVersion", "2.1");
            x.close("Version");
        }
        // 3.0's version.xsd dropped DetailedVersion.
        TargetVersion::V3_0 => x.empty("Version", &[("VersionId", "3.0")]),
    }
    x.finish()
}

fn extensions_xml(ext: &Extensions) -> Vec<u8> {
    let mut x = Xml::new();
    x.open("Extensions", &[]);
    for ((_, list), (wrapper, item)) in extension_lists(ext).into_iter().zip([
        ("TopicTypes", "TopicType"),
        ("TopicStatuses", "TopicStatus"),
        ("Priorities", "Priority"),
        ("TopicLabels", "TopicLabel"),
        ("Users", "User"),
        ("SnippetTypes", "SnippetType"),
        ("Stages", "Stage"),
    ]) {
        if list.is_empty() {
            x.empty(wrapper, &[]);
        } else {
            x.open(wrapper, &[]);
            for v in list {
                x.leaf(item, v);
            }
            x.close(wrapper);
        }
    }
    x.close("Extensions");
    x.finish()
}

fn markup_xml(version: TargetVersion, t: &Topic) -> Vec<u8> {
    let v3 = version == TargetVersion::V3_0;
    let mut x = Xml::new();
    x.open("Markup", &[]);

    let mut attrs = vec![("Guid", t.guid.as_str())];
    if let Some(v) = &t.topic_type {
        attrs.push(("TopicType", v));
    }
    if let Some(v) = &t.topic_status {
        attrs.push(("TopicStatus", v));
    }
    x.open("Topic", &attrs);
    // Element order is the xs:sequence order of the Topic type.
    x.leaf("Title", &t.title);
    if let Some(p) = &t.priority {
        x.leaf("Priority", p);
    }
    if v3 {
        if !t.labels.is_empty() {
            x.open("Labels", &[]);
            for l in &t.labels {
                x.leaf("Label", l);
            }
            x.close("Labels");
        }
    } else {
        for l in &t.labels {
            x.leaf("Labels", l);
        }
    }
    x.leaf("CreationDate", &t.creation_date);
    x.leaf("CreationAuthor", &t.creation_author);
    // Topic sequence: … CreationAuthor, ModifiedDate?, ModifiedAuthor?,
    // DueDate?, AssignedTo?, Stage?, Description? … in 2.1 and 3.0 alike.
    if let Some(d) = &t.due_date {
        x.leaf("DueDate", d);
    }
    if let Some(a) = &t.assigned_to {
        x.leaf("AssignedTo", a);
    }
    if let Some(d) = &t.description {
        x.leaf("Description", d);
    }

    // 3.0 nests comments and viewpoints inside Topic; 2.1 lists them after it.
    if v3 {
        if !t.comments.is_empty() {
            x.open("Comments", &[]);
            comments_xml(&mut x, &t.comments);
            x.close("Comments");
        }
        if !t.viewpoints.is_empty() {
            x.open("Viewpoints", &[]);
            viewpoint_refs_xml(&mut x, "ViewPoint", &t.viewpoints);
            x.close("Viewpoints");
        }
        x.close("Topic");
    } else {
        x.close("Topic");
        comments_xml(&mut x, &t.comments);
        // In 2.1 each <Viewpoints> element *is* one viewpoint reference.
        viewpoint_refs_xml(&mut x, "Viewpoints", &t.viewpoints);
    }

    x.close("Markup");
    x.finish()
}

fn comments_xml(x: &mut Xml, comments: &[Comment]) {
    for c in comments {
        x.open("Comment", &[("Guid", &c.guid)]);
        x.leaf("Date", &c.date);
        x.leaf("Author", &c.author);
        x.leaf("Comment", &c.comment);
        if let Some(v) = &c.viewpoint {
            x.empty("Viewpoint", &[("Guid", v)]);
        }
        x.close("Comment");
    }
}

fn viewpoint_refs_xml(x: &mut Xml, element: &str, viewpoints: &[Viewpoint]) {
    for vp in viewpoints {
        x.open(element, &[("Guid", &vp.guid)]);
        // ViewPoint sequence: Viewpoint?, Snapshot?, Index? (2.1 and 3.0).
        x.leaf("Viewpoint", &viewpoint_file(vp));
        if vp.snapshot.is_some() {
            x.leaf("Snapshot", &snapshot_file(vp));
        }
        x.close(element);
    }
}

fn visinfo_xml(version: TargetVersion, vp: &Viewpoint) -> Vec<u8> {
    let v3 = version == TargetVersion::V3_0;
    let mut x = Xml::new();
    x.open("VisualizationInfo", &[("Guid", &vp.guid)]);
    // The Components sequence is Selection?, Visibility, Coloring? in both
    // versions (2.1 also allows a leading ViewSetupHints, not written here).
    if !vp.selection.is_empty() || vp.visibility.is_some() || !vp.coloring.is_empty() {
        x.open("Components", &[]);
        if !vp.selection.is_empty() {
            x.open("Selection", &[]);
            components_xml(&mut x, &vp.selection);
            x.close("Selection");
        }
        visibility_xml(&mut x, vp.visibility.as_ref());
        if !vp.coloring.is_empty() {
            x.open("Coloring", &[]);
            for coloring in &vp.coloring {
                x.open("Color", &[("Color", &coloring.color)]);
                // 3.0 wraps the coloured components; 2.1 lists them bare.
                if v3 {
                    x.open("Components", &[]);
                    components_xml(&mut x, &coloring.components);
                    x.close("Components");
                } else {
                    components_xml(&mut x, &coloring.components);
                }
                x.close("Color");
            }
            x.close("Coloring");
        }
        x.close("Components");
    }
    if let Some(c) = &vp.camera {
        let element = match c.projection {
            Projection::Perspective { .. } => "PerspectiveCamera",
            Projection::Orthogonal { .. } => "OrthogonalCamera",
        };
        x.open(element, &[]);
        x.vector("CameraViewPoint", c.view_point);
        x.vector("CameraDirection", c.direction);
        x.vector("CameraUpVector", c.up_vector);
        match c.projection {
            Projection::Perspective { field_of_view } => x.number("FieldOfView", field_of_view),
            Projection::Orthogonal {
                view_to_world_scale,
            } => {
                x.number("ViewToWorldScale", view_to_world_scale);
            }
        }
        if v3 {
            if let Some(r) = c.aspect_ratio {
                x.number("AspectRatio", r);
            }
        }
        x.close(element);
    }
    // After the camera and (unwritten) Lines, in both versions.
    if !vp.clipping_planes.is_empty() {
        x.open("ClippingPlanes", &[]);
        for plane in &vp.clipping_planes {
            x.open("ClippingPlane", &[]);
            x.vector("Location", plane.location);
            x.vector("Direction", plane.direction);
            x.close("ClippingPlane");
        }
        x.close("ClippingPlanes");
    }
    x.close("VisualizationInfo");
    x.finish()
}

/// `Components/Visibility`: required in 2.1 whenever `Components` is
/// written, and written in 3.0 too, explicitly — 3.0's `DefaultVisibility`
/// defaults to false, so omitting it would hide the model.
fn visibility_xml(x: &mut Xml, visibility: Option<&Visibility>) {
    let Some(v) = visibility else {
        x.empty("Visibility", &[("DefaultVisibility", "true")]);
        return;
    };
    let default = if v.default_visibility {
        "true"
    } else {
        "false"
    };
    // 2.1's Exceptions needs at least one Component, so an empty list is
    // omitted rather than written empty; the meaning is the same.
    if v.exceptions.is_empty() {
        x.empty("Visibility", &[("DefaultVisibility", default)]);
        return;
    }
    x.open("Visibility", &[("DefaultVisibility", default)]);
    x.open("Exceptions", &[]);
    components_xml(x, &v.exceptions);
    x.close("Exceptions");
    x.close("Visibility");
}

fn components_xml(x: &mut Xml, components: &[Component]) {
    for c in components {
        let attrs: Vec<(&str, &str)> = c.ifc_guid.iter().map(|g| ("IfcGuid", g.as_str())).collect();
        if c.originating_system.is_none() && c.authoring_tool_id.is_none() {
            x.empty("Component", &attrs);
            continue;
        }
        x.open("Component", &attrs);
        if let Some(s) = &c.originating_system {
            x.leaf("OriginatingSystem", s);
        }
        if let Some(id) = &c.authoring_tool_id {
            x.leaf("AuthoringToolId", id);
        }
        x.close("Component");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_and_attributes_escape_what_xml_would_alter() {
        let mut t = String::new();
        escape_text(&mut t, "a & <b> \"c\"\r\n\td");
        assert_eq!(t, "a &amp; &lt;b&gt; \"c\"&#xD;\n\td");
        let mut a = String::new();
        escape_attr(&mut a, "a & <b> \"c\"\r\n\td");
        assert_eq!(a, "a &amp; &lt;b&gt; &quot;c&quot;&#xD;&#xA;&#x9;d");
    }

    #[test]
    fn numbers_are_plain_decimal_xs_doubles() {
        let mut x = Xml::new();
        x.number("A", 1.0);
        x.number("B", -0.000_000_1);
        x.number("C", 26.002_615_225_590_86);
        let s = String::from_utf8(x.finish()).unwrap();
        assert!(s.contains("<A>1</A>"), "{s}");
        assert!(s.contains("<B>-0.0000001</B>"), "{s}");
        assert!(s.contains("<C>26.00261522559086</C>"), "{s}");
    }
}
