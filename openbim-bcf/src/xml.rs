//! A minimal pull-based XML tree, tuned for BCF markup.
//!
//! # Why not serde
//!
//! BCF's two hard problems are that the *same* element name means different
//! things across versions (`TopicStatus` is a child in 2.0 and an attribute in
//! 2.1) and that real files omit fields the schema requires. A derive-based
//! mapping either needs one struct set per version or `Option` everywhere plus
//! bespoke visitors — and it cannot tell "absent" from "present but empty",
//! which is precisely the distinction the tolerance policy rests on.
//!
//! So markup is parsed into a small generic tree and interpreted afterwards by
//! version-aware code that can report what it tolerated.

use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::Event;
use quick_xml::{Reader, XmlVersion};

/// A parsed XML element: name, attributes, text, and children, all verbatim.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Node {
    pub(crate) name: String,
    pub(crate) attrs: Vec<(String, String)>,
    pub(crate) text: String,
    pub(crate) children: Vec<Node>,
}

impl Node {
    /// The first child with this local name.
    pub(crate) fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    /// Every child with this local name, in document order.
    pub(crate) fn children_named<'a>(
        &'a self,
        name: &'a str,
    ) -> impl Iterator<Item = &'a Node> + 'a {
        self.children.iter().filter(move |c| c.name == name)
    }

    /// An attribute value.
    pub(crate) fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// Trimmed text of the first child with this name, if that text is not
    /// empty.
    ///
    /// Empty is treated as absent deliberately: BCF 3.0's own schema types
    /// several fields as `NonEmptyOrBlankString`, so a blank element carries
    /// no more information than a missing one.
    pub(crate) fn child_text(&self, name: &str) -> Option<&str> {
        self.child(name)
            .map(|c| c.text.trim())
            .filter(|t| !t.is_empty())
    }

    /// An attribute, or failing that a child element, with the same name.
    ///
    /// This is the single place the 2.0-vs-2.1 relocation of `TopicStatus` and
    /// `TopicType` is absorbed. Attribute wins: when a writer emits both, the
    /// attribute is the newer, authoritative form.
    pub(crate) fn attr_or_child_text(&self, name: &str) -> Option<&str> {
        self.attr(name)
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .or_else(|| self.child_text(name))
    }
}

/// Strip any namespace prefix. BCF markup is unprefixed in every published
/// schema, but writers in the corpus declare `xsd`/`xsi` prefixes and a few
/// prefix their own elements; the local name is what carries meaning.
fn local_name(raw: &str) -> String {
    match raw.rsplit_once(':') {
        Some((_, local)) => local.to_string(),
        None => raw.to_string(),
    }
}

/// Parse a markup document into a tree.
///
/// Accepts a leading UTF-8 BOM and CRLF line endings, both of which appear in
/// the corpus. Text and attribute values are normalised as the XML
/// specification requires of every parser: line endings become `\n`, and
/// whitespace in attribute values becomes spaces — except where a character
/// reference such as `&#xD;` spells it, which is how a writer keeps it.
pub(crate) fn parse(bytes: &[u8]) -> Result<Node, String> {
    let mut reader = Reader::from_reader(bytes);
    let config = reader.config_mut();
    config.trim_text(false);
    config.check_end_names = false;

    let mut stack: Vec<Node> = Vec::new();
    let mut root: Option<Node> = None;
    let mut buf = Vec::new();
    // Normalisation rules differ between XML 1.0 and 1.1; the declaration
    // says which, and its absence means 1.0.
    let mut version = XmlVersion::Implicit1_0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let node = build_node(&e, version).map_err(|e| e.to_string())?;
                stack.push(node);
            }
            Ok(Event::Empty(e)) => {
                let node = build_node(&e, version).map_err(|e| e.to_string())?;
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Ok(Event::End(_)) => {
                let Some(node) = stack.pop() else { continue };
                match stack.last_mut() {
                    Some(parent) => parent.children.push(node),
                    None => root = Some(node),
                }
            }
            Ok(Event::Decl(d)) => {
                version = d.xml_version().map_err(|e| e.to_string())?;
            }
            Ok(Event::Text(t)) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t.xml_content(version));
                }
            }
            // References arrive as their own events, between the text around
            // them. Only the five predefined entities and character references
            // exist without a DTD; any other name is an error, as it is for
            // every conforming parser.
            Ok(Event::GeneralRef(r)) => {
                if let Some(top) = stack.last_mut() {
                    match r.resolve_char_ref().map_err(|e| e.to_string())? {
                        Some(ch) => top.text.push(ch),
                        None => match resolve_predefined_entity(&r) {
                            Some(s) => top.text.push_str(s),
                            None => {
                                return Err(format!(
                                    "unknown entity &{}; at byte {}",
                                    &*r,
                                    reader.buffer_position()
                                ))
                            }
                        },
                    }
                }
            }
            Ok(Event::CData(t)) => {
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&t.xml_content(version));
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(format!("{e} at byte {}", reader.buffer_position())),
        }
        buf.clear();
    }

    // An unclosed document still yields whatever was on the stack rather than
    // nothing: truncated markup is worth reporting with its partial content.
    if root.is_none() {
        while let Some(node) = stack.pop() {
            match stack.last_mut() {
                Some(parent) => parent.children.push(node),
                None => root = Some(node),
            }
        }
    }

    root.ok_or_else(|| "document contains no elements".to_string())
}

fn build_node(
    e: &quick_xml::events::BytesStart<'_>,
    version: XmlVersion,
) -> Result<Node, quick_xml::Error> {
    let mut node = Node {
        name: local_name(e.name().as_ref()),
        ..Node::default()
    };
    // `with_checks(false)`: a repeated attribute is tolerated (the first one
    // wins in `Node::attr`), not a reason to lose the document. It also skips
    // the duplicate-name scan that RUSTSEC-2026-0194 made quadratic.
    for attr in e.attributes().with_checks(false) {
        let attr = attr.map_err(quick_xml::Error::from)?;
        let key = local_name(attr.key.as_ref());
        let value = attr.normalized_value(version)?.into_owned();
        node.attrs.push((key, value));
    }
    Ok(node)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attributes_text_and_nesting() {
        let doc = parse(br#"<Markup><Topic Guid="g"><Title>T</Title></Topic></Markup>"#).unwrap();
        assert_eq!(doc.name, "Markup");
        let topic = doc.child("Topic").unwrap();
        assert_eq!(topic.attr("Guid"), Some("g"));
        assert_eq!(topic.child_text("Title"), Some("T"));
    }

    #[test]
    fn strips_namespace_prefixes_from_names_and_attributes() {
        let doc = parse(br#"<b:Markup xmlns:b="urn:x"><b:Topic b:Guid="g"/></b:Markup>"#).unwrap();
        assert_eq!(doc.name, "Markup");
        assert_eq!(doc.child("Topic").unwrap().attr("Guid"), Some("g"));
    }

    #[test]
    fn tolerates_bom_and_crlf() {
        let doc =
            parse("\u{feff}<Markup>\r\n  <Topic Guid=\"g\"/>\r\n</Markup>".as_bytes()).unwrap();
        assert_eq!(doc.name, "Markup");
        assert!(doc.child("Topic").is_some());
    }

    #[test]
    fn attribute_wins_over_element_of_the_same_name() {
        let doc = parse(br#"<Topic TopicStatus="Open"><TopicStatus>Closed</TopicStatus></Topic>"#)
            .unwrap();
        assert_eq!(doc.attr_or_child_text("TopicStatus"), Some("Open"));
    }

    #[test]
    fn falls_back_to_child_element_for_2_0_shape() {
        let doc = parse(br"<Topic><TopicStatus>Offen</TopicStatus></Topic>").unwrap();
        assert_eq!(doc.attr_or_child_text("TopicStatus"), Some("Offen"));
    }

    #[test]
    fn blank_values_count_as_absent() {
        let doc = parse(br#"<Topic TopicStatus="   "><TopicType>  </TopicType></Topic>"#).unwrap();
        assert_eq!(doc.attr_or_child_text("TopicStatus"), None);
        assert_eq!(doc.child_text("TopicType"), None);
    }

    #[test]
    fn repeated_children_are_all_kept_in_order() {
        let doc = parse(br"<M><C>1</C><C>2</C><C>3</C></M>").unwrap();
        let seen: Vec<_> = doc.children_named("C").map(|c| c.text.as_str()).collect();
        assert_eq!(seen, ["1", "2", "3"]);
    }

    #[test]
    fn entities_and_cdata_are_unescaped() {
        let doc = parse(br"<M><A>a &amp; b</A><B><![CDATA[<raw>]]></B></M>").unwrap();
        assert_eq!(doc.child_text("A"), Some("a & b"));
        assert_eq!(doc.child_text("B"), Some("<raw>"));
    }

    #[test]
    fn truncated_documents_yield_partial_content_not_nothing() {
        let doc = parse(br#"<Markup><Topic Guid="g"><Title>T</Title>"#).unwrap();
        assert_eq!(doc.name, "Markup");
        assert_eq!(doc.child("Topic").unwrap().attr("Guid"), Some("g"));
    }

    /// XML 1.0 section 2.11: a parser delivers line breaks as `\n`. A CR the
    /// writer means to keep is spelled `&#xD;`, which survives.
    #[test]
    fn line_endings_are_normalised_but_character_references_are_kept() {
        let doc = parse(b"<M><A>one\r\ntwo\rthree</A><B>one&#xD;\ntwo</B></M>").unwrap();
        assert_eq!(doc.child("A").unwrap().text, "one\ntwo\nthree");
        assert_eq!(doc.child("B").unwrap().text, "one\r\ntwo");
    }

    /// Attribute-value normalisation turns literal whitespace into spaces;
    /// character references are exempt, which is how the writer keeps a tab
    /// or line break in an attribute.
    #[test]
    fn attribute_whitespace_is_normalised_except_character_references() {
        let doc =
            parse(b"<M A=\"a\tb\nc\" B=\"a&#x9;b&#xA;c&#xD;\" C=\"&lt;&amp;&gt;&quot;&apos;\"/>")
                .unwrap();
        assert_eq!(doc.attr("A"), Some("a b c"));
        assert_eq!(doc.attr("B"), Some("a\tb\nc\r"));
        assert_eq!(doc.attr("C"), Some("<&>\"'"));
    }

    #[test]
    fn predefined_entities_and_character_references_resolve_in_text() {
        let doc = parse("<M>&lt;&amp;&gt;&quot;&apos; &#228;&#x1F600;</M>".as_bytes()).unwrap();
        assert_eq!(doc.text, "<&>\"' ä😀");
    }

    /// Without a DTD only the five predefined entities exist. An unknown one
    /// fails the document, which the archive reports as `UnreadableMarkup`
    /// while reading the rest — as with quick-xml 0.37.
    #[test]
    fn an_unknown_entity_is_an_error_not_silently_dropped() {
        let err = parse(b"<M>a &nbsp; b</M>").unwrap_err();
        assert!(err.contains("nbsp"), "{err}");
    }

    /// Repeated attributes are tolerated rather than failing the document;
    /// the first one wins.
    #[test]
    fn a_repeated_attribute_keeps_the_first_value() {
        let doc = parse(br#"<Topic Guid="a" Guid="b"/>"#).unwrap();
        assert_eq!(doc.attr("Guid"), Some("a"));
    }

    #[test]
    fn an_xml_1_1_declaration_is_accepted() {
        let doc = parse(b"<?xml version=\"1.1\"?><M><A>x\r\ny</A></M>").unwrap();
        assert_eq!(doc.child("A").unwrap().text, "x\ny");
    }

    #[test]
    fn a_document_without_elements_is_an_error() {
        assert!(parse(b"not xml at all").is_err());
        assert!(parse(b"").is_err());
    }
}
