//! Parses `word/numbering.xml` to resolve list styles for numbered/bulleted paragraphs.

// These items will be used once the document body parser is implemented.
#![allow(dead_code)]

use crate::DocxError;
use docmux_ast::ListStyle;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

// ─── Types ───────────────────────────────────────────────────────────────────

/// Resolved numbering information for a (numId, ilvl) pair.
#[derive(Debug, Clone)]
pub(crate) struct NumberingInfo {
    pub(crate) ordered: bool,
    pub(crate) style: Option<ListStyle>,
}

/// Map from (numId, ilvl) → [`NumberingInfo`].
pub(crate) type NumberingMap = HashMap<(u32, u32), NumberingInfo>;

// ─── num_fmt_to_list_style ────────────────────────────────────────────────────

/// Convert a `w:numFmt` value to `(ordered, Option<ListStyle>)`.
pub(crate) fn num_fmt_to_list_style(fmt: &str) -> (bool, Option<ListStyle>) {
    match fmt {
        "bullet" => (false, None),
        "decimal" => (true, Some(ListStyle::Decimal)),
        "lowerLetter" => (true, Some(ListStyle::LowerAlpha)),
        "upperLetter" => (true, Some(ListStyle::UpperAlpha)),
        "lowerRoman" => (true, Some(ListStyle::LowerRoman)),
        "upperRoman" => (true, Some(ListStyle::UpperRoman)),
        // Treat everything else as an unordered/normal bullet
        _ => (false, None),
    }
}

// ─── parse_numbering ─────────────────────────────────────────────────────────

/// Parse `word/numbering.xml` into a [`NumberingMap`].
///
/// Two-pass strategy:
/// 1. Collect `abstractNum` definitions: abstractNumId → HashMap<ilvl, numFmt>
/// 2. Resolve `num` entries: numId → abstractNumId, then expand per ilvl
pub(crate) fn parse_numbering(xml: &str) -> Result<NumberingMap, DocxError> {
    // ── Pass 1: collect abstractNum ──────────────────────────────────────────
    // abstractNumId → (ilvl → numFmt string)
    let mut abstract_defs: HashMap<u32, HashMap<u32, String>> = HashMap::new();

    {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();

        let mut cur_abstract_id: Option<u32> = None;
        let mut cur_ilvl: Option<u32> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    match e.local_name().as_ref() {
                        b"abstractNum" => {
                            cur_abstract_id = None;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"abstractNumId" {
                                    if let Ok(n) =
                                        String::from_utf8_lossy(&attr.value).parse::<u32>()
                                    {
                                        cur_abstract_id = Some(n);
                                        abstract_defs.entry(n).or_default();
                                    }
                                }
                            }
                        }
                        b"lvl" => {
                            cur_ilvl = None;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"ilvl" {
                                    if let Ok(n) =
                                        String::from_utf8_lossy(&attr.value).parse::<u32>()
                                    {
                                        cur_ilvl = Some(n);
                                    }
                                }
                            }
                        }
                        b"numFmt" => {
                            if let (Some(abs_id), Some(ilvl)) = (cur_abstract_id, cur_ilvl) {
                                for attr in e.attributes().flatten() {
                                    if attr.key.local_name().as_ref() == b"val" {
                                        let fmt = String::from_utf8_lossy(&attr.value).into_owned();
                                        abstract_defs.entry(abs_id).or_default().insert(ilvl, fmt);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => match e.local_name().as_ref() {
                    b"abstractNum" => cur_abstract_id = None,
                    b"lvl" => cur_ilvl = None,
                    _ => {}
                },
                Ok(Event::Eof) => break,
                Err(e) => return Err(DocxError::Xml(e.to_string())),
                _ => {}
            }
            buf.clear();
        }
    }

    // ── Pass 2: resolve num → abstractNum ────────────────────────────────────
    let mut map = NumberingMap::new();

    {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();

        let mut cur_num_id: Option<u32> = None;
        let mut cur_abstract_num_id: Option<u32> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    match e.local_name().as_ref() {
                        b"num" => {
                            cur_num_id = None;
                            cur_abstract_num_id = None;
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"numId" {
                                    if let Ok(n) =
                                        String::from_utf8_lossy(&attr.value).parse::<u32>()
                                    {
                                        cur_num_id = Some(n);
                                    }
                                }
                            }
                        }
                        b"abstractNumId" => {
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"val" {
                                    if let Ok(n) =
                                        String::from_utf8_lossy(&attr.value).parse::<u32>()
                                    {
                                        cur_abstract_num_id = Some(n);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) if e.local_name().as_ref() == b"num" => {
                    if let (Some(num_id), Some(abs_id)) = (cur_num_id, cur_abstract_num_id) {
                        if let Some(levels) = abstract_defs.get(&abs_id) {
                            for (&ilvl, fmt) in levels {
                                let (ordered, style) = num_fmt_to_list_style(fmt);
                                map.insert((num_id, ilvl), NumberingInfo { ordered, style });
                            }
                        }
                    }
                    cur_num_id = None;
                    cur_abstract_num_id = None;
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(DocxError::Xml(e.to_string())),
                _ => {}
            }
            buf.clear();
        }
    }

    Ok(map)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn num_fmt_mapping() {
        let (ordered, style) = num_fmt_to_list_style("bullet");
        assert!(!ordered);
        assert!(style.is_none());

        let (ordered, style) = num_fmt_to_list_style("decimal");
        assert!(ordered);
        assert!(matches!(style, Some(ListStyle::Decimal)));

        let (ordered, style) = num_fmt_to_list_style("lowerLetter");
        assert!(ordered);
        assert!(matches!(style, Some(ListStyle::LowerAlpha)));

        let (ordered, style) = num_fmt_to_list_style("upperLetter");
        assert!(ordered);
        assert!(matches!(style, Some(ListStyle::UpperAlpha)));

        let (ordered, style) = num_fmt_to_list_style("lowerRoman");
        assert!(ordered);
        assert!(matches!(style, Some(ListStyle::LowerRoman)));

        let (ordered, style) = num_fmt_to_list_style("upperRoman");
        assert!(ordered);
        assert!(matches!(style, Some(ListStyle::UpperRoman)));

        // Unknown format → unordered
        let (ordered, style) = num_fmt_to_list_style("chicago");
        assert!(!ordered);
        assert!(style.is_none());
    }

    #[test]
    fn parse_numbering_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:abstractNum w:abstractNumId="0">
    <w:lvl w:ilvl="0"><w:numFmt w:val="bullet"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="bullet"/></w:lvl>
  </w:abstractNum>
  <w:abstractNum w:abstractNumId="1">
    <w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl>
    <w:lvl w:ilvl="1"><w:numFmt w:val="lowerLetter"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1">
    <w:abstractNumId w:val="0"/>
  </w:num>
  <w:num w:numId="2">
    <w:abstractNumId w:val="1"/>
  </w:num>
</w:numbering>"#;

        let map = parse_numbering(xml).unwrap();

        // numId=1 (bullet list)
        let info = map.get(&(1, 0)).unwrap();
        assert!(!info.ordered);
        assert!(info.style.is_none());

        // numId=2 (decimal ordered list)
        let info = map.get(&(2, 0)).unwrap();
        assert!(info.ordered);
        assert!(matches!(info.style, Some(ListStyle::Decimal)));

        // numId=2 ilvl=1 (lower-alpha)
        let info = map.get(&(2, 1)).unwrap();
        assert!(info.ordered);
        assert!(matches!(info.style, Some(ListStyle::LowerAlpha)));
    }

    #[test]
    fn parse_empty_numbering() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
</w:numbering>"#;

        let map = parse_numbering(xml).unwrap();
        assert!(map.is_empty());
    }
}
