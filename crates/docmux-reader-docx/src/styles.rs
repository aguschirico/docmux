//! Parses the DOCX `word/styles.xml` and classifies styles into AST-relevant kinds.

// These items will be used once the document body parser is implemented.
#![allow(dead_code)]

use crate::DocxError;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

// ─── StyleKind ───────────────────────────────────────────────────────────────

/// The semantic kind of a Word style.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StyleKind {
    Heading(u8),
    CodeBlock,
    BlockQuote,
    MathBlock,
    Caption,
    Title,
    Author,
    Date,
    Abstract,
    Normal,
}

// ─── StyleInfo ────────────────────────────────────────────────────────────────

/// Metadata about a single Word style.
#[derive(Debug, Clone)]
pub(crate) struct StyleInfo {
    pub(crate) style_id: String,
    pub(crate) name: String,
    pub(crate) based_on: Option<String>,
    pub(crate) style_type: String,
    pub(crate) kind: StyleKind,
}

/// Map from styleId → [`StyleInfo`].
pub(crate) type StyleMap = HashMap<String, StyleInfo>;

// ─── classify_style ──────────────────────────────────────────────────────────

/// Classify a style by ID, name, and optional outline level.
///
/// Priority:
/// 1. Exact ID match for well-known docmux/Word styles
/// 2. Heading by ID pattern (heading1–6, titre1–6, überschrift1–6, título1–6)
/// 3. Heading by outline level (0–5 → levels 1–6)
/// 4. Name pattern match
/// 5. Fallback: Normal
pub(crate) fn classify_style(id: &str, name: &str, outline_lvl: Option<u8>) -> StyleKind {
    let id_lc = id.to_lowercase();
    let name_lc = name.to_lowercase();

    // ── 1. Exact ID match ────────────────────────────────────────────────────
    match id_lc.as_str() {
        "title" => return StyleKind::Title,
        "author" => return StyleKind::Author,
        "date" => return StyleKind::Date,
        "abstract" => return StyleKind::Abstract,
        "codeblock" | "code block" | "sourcecode" | "source code" | "verbatim" | "preformatted" => {
            return StyleKind::CodeBlock
        }
        "blockquote" | "block quote" | "blocktext" | "block text" => return StyleKind::BlockQuote,
        "mathblock" | "math block" | "displaymath" | "display math" => return StyleKind::MathBlock,
        "caption" => return StyleKind::Caption,
        _ => {}
    }

    // ── 2. Heading by ID pattern ─────────────────────────────────────────────
    let heading_prefixes = [
        "heading",
        "titre",
        "überschrift",
        "titulo",
        "título",
        "overskrift",
        "rubrik",
        "kap",
    ];
    for prefix in &heading_prefixes {
        if let Some(suffix) = id_lc.strip_prefix(prefix) {
            if let Ok(n) = suffix.parse::<u8>() {
                if (1..=6).contains(&n) {
                    return StyleKind::Heading(n);
                }
            }
        }
    }

    // ── 3. Heading by outline level ──────────────────────────────────────────
    if let Some(lvl) = outline_lvl {
        if lvl <= 5 {
            return StyleKind::Heading(lvl + 1);
        }
    }

    // ── 4. Name pattern match ────────────────────────────────────────────────
    if name_lc.contains("heading")
        || name_lc.starts_with("h1")
        || name_lc.starts_with("h2")
        || name_lc.starts_with("h3")
    {
        // Try to extract level from name (e.g. "Heading 2", "heading3")
        for n in 1u8..=6 {
            let ns = n.to_string();
            if name_lc.ends_with(&ns) || name_lc.contains(&format!(" {ns}")) {
                return StyleKind::Heading(n);
            }
        }
        return StyleKind::Heading(1);
    }
    if name_lc.contains("code")
        || name_lc.contains("source code")
        || name_lc.contains("preformatted")
        || name_lc.contains("verbatim")
    {
        return StyleKind::CodeBlock;
    }
    if name_lc.contains("quote") || name_lc.contains("block text") {
        return StyleKind::BlockQuote;
    }
    if name_lc.contains("abstract") {
        return StyleKind::Abstract;
    }
    if name_lc.contains("caption") {
        return StyleKind::Caption;
    }

    StyleKind::Normal
}

// ─── parse_styles ────────────────────────────────────────────────────────────

/// Parse `word/styles.xml` into a [`StyleMap`].
pub(crate) fn parse_styles(xml: &str) -> Result<StyleMap, DocxError> {
    let mut map = StyleMap::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();

    // Per-style state
    let mut style_id = String::new();
    let mut style_type = String::new();
    let mut style_name = String::new();
    let mut based_on: Option<String> = None;
    let mut outline_lvl: Option<u8> = None;
    let mut in_style = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                match e.local_name().as_ref() {
                    b"style" => {
                        // Start a new style — reset state
                        style_id.clear();
                        style_type.clear();
                        style_name.clear();
                        based_on = None;
                        outline_lvl = None;
                        in_style = true;

                        for attr in e.attributes().flatten() {
                            match attr.key.local_name().as_ref() {
                                b"styleId" => {
                                    style_id = String::from_utf8_lossy(&attr.value).into_owned();
                                }
                                b"type" => {
                                    style_type = String::from_utf8_lossy(&attr.value).into_owned();
                                }
                                _ => {}
                            }
                        }
                    }
                    b"name" if in_style => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                style_name = String::from_utf8_lossy(&attr.value).into_owned();
                            }
                        }
                    }
                    b"basedOn" if in_style => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                based_on = Some(String::from_utf8_lossy(&attr.value).into_owned());
                            }
                        }
                    }
                    b"outlineLvl" if in_style => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                if let Ok(n) = String::from_utf8_lossy(&attr.value).parse::<u8>() {
                                    outline_lvl = Some(n);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if e.local_name().as_ref() == b"style" => {
                if in_style && !style_id.is_empty() {
                    let kind = classify_style(&style_id, &style_name, outline_lvl);
                    map.insert(
                        style_id.clone(),
                        StyleInfo {
                            style_id: style_id.clone(),
                            name: style_name.clone(),
                            based_on: based_on.clone(),
                            style_type: style_type.clone(),
                            kind,
                        },
                    );
                }
                in_style = false;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocxError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(map)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_docmux_styles() {
        assert_eq!(classify_style("Title", "Title", None), StyleKind::Title);
        assert_eq!(classify_style("Author", "Author", None), StyleKind::Author);
        assert_eq!(classify_style("Date", "Date", None), StyleKind::Date);
        assert_eq!(
            classify_style("Abstract", "Abstract", None),
            StyleKind::Abstract
        );
        assert_eq!(
            classify_style("CodeBlock", "Code Block", None),
            StyleKind::CodeBlock
        );
        assert_eq!(
            classify_style("BlockQuote", "Block Quote", None),
            StyleKind::BlockQuote
        );
        assert_eq!(
            classify_style("MathBlock", "Math Block", None),
            StyleKind::MathBlock
        );
        assert_eq!(
            classify_style("Caption", "Caption", None),
            StyleKind::Caption
        );
    }

    #[test]
    fn classify_by_outline_level() {
        // outlineLvl 0 → Heading 1, outlineLvl 5 → Heading 6
        assert_eq!(
            classify_style("MyStyle", "My Style", Some(0)),
            StyleKind::Heading(1)
        );
        assert_eq!(
            classify_style("MyStyle", "My Style", Some(2)),
            StyleKind::Heading(3)
        );
        assert_eq!(
            classify_style("MyStyle", "My Style", Some(5)),
            StyleKind::Heading(6)
        );
        // outlineLvl 6+ → Normal (not a visible heading)
        assert_eq!(
            classify_style("MyStyle", "My Style", Some(6)),
            StyleKind::Normal
        );
    }

    #[test]
    fn classify_by_name_pattern() {
        assert_eq!(
            classify_style("xyz1", "Heading 1", None),
            StyleKind::Heading(1)
        );
        assert_eq!(
            classify_style("xyz2", "Heading 2", None),
            StyleKind::Heading(2)
        );
        assert_eq!(
            classify_style("xyz", "Source Code", None),
            StyleKind::CodeBlock
        );
        assert_eq!(
            classify_style("xyz", "Block Quote", None),
            StyleKind::BlockQuote
        );
        assert_eq!(
            classify_style("xyz", "Abstract Text", None),
            StyleKind::Abstract
        );
        assert_eq!(
            classify_style("xyz", "Figure Caption", None),
            StyleKind::Caption
        );
    }

    #[test]
    fn classify_i18n_styles() {
        // French
        assert_eq!(
            classify_style("titre1", "Titre 1", None),
            StyleKind::Heading(1)
        );
        assert_eq!(
            classify_style("titre3", "Titre 3", None),
            StyleKind::Heading(3)
        );
        // German
        assert_eq!(
            classify_style("überschrift2", "Überschrift 2", None),
            StyleKind::Heading(2)
        );
        // Spanish
        assert_eq!(
            classify_style("título1", "Título 1", None),
            StyleKind::Heading(1)
        );
    }

    #[test]
    fn classify_unknown_as_normal() {
        assert_eq!(
            classify_style("FooterText", "Footer Text", None),
            StyleKind::Normal
        );
        assert_eq!(classify_style("Normal", "Normal", None), StyleKind::Normal);
    }

    #[test]
    fn parse_styles_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Normal">
    <w:name w:val="Normal"/>
  </w:style>
  <w:style w:type="paragraph" w:styleId="CodeBlock">
    <w:name w:val="Code Block"/>
  </w:style>
</w:styles>"#;

        let map = parse_styles(xml).unwrap();
        assert!(map.contains_key("Heading1"));
        assert!(map.contains_key("Normal"));
        assert!(map.contains_key("CodeBlock"));

        let h1 = &map["Heading1"];
        assert_eq!(h1.kind, StyleKind::Heading(1));
        assert_eq!(h1.based_on.as_deref(), Some("Normal"));

        let code = &map["CodeBlock"];
        assert_eq!(code.kind, StyleKind::CodeBlock);
    }
}
