//! Parses `word/footnotes.xml` into a map of footnote id → block content.

// These items will be used once the document body parser is implemented.
#![allow(dead_code)]

use crate::DocxError;
use docmux_ast::{Block, Inline};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

/// Map from footnote id (as string) to its block content.
pub(crate) type FootnoteMap = HashMap<String, Vec<Block>>;

/// Parse `word/footnotes.xml` into a [`FootnoteMap`].
///
/// Skips separator and continuation-separator footnotes (ids -1 and 0).
/// Extracts plain text from `<w:t>` elements and wraps them as Paragraph blocks.
pub(crate) fn parse_footnotes(xml: &str) -> Result<FootnoteMap, DocxError> {
    let mut map = FootnoteMap::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();

    let mut cur_id: Option<String> = None;
    let mut cur_type: Option<String> = None;
    let mut cur_text: Vec<Inline> = Vec::new();
    let mut in_footnote = false;
    let mut in_t = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => match e.local_name().as_ref() {
                b"footnote" => {
                    cur_id = None;
                    cur_type = None;
                    cur_text.clear();
                    in_footnote = true;

                    for attr in e.attributes().flatten() {
                        match attr.key.local_name().as_ref() {
                            b"id" => {
                                cur_id = Some(String::from_utf8_lossy(&attr.value).into_owned());
                            }
                            b"type" => {
                                cur_type = Some(String::from_utf8_lossy(&attr.value).into_owned());
                            }
                            _ => {}
                        }
                    }
                }
                b"t" if in_footnote => {
                    in_t = true;
                }
                _ => {}
            },
            Ok(Event::Text(ref e)) if in_t => {
                let text = e
                    .unescape()
                    .map_err(|err| DocxError::Xml(err.to_string()))?;
                if !text.is_empty() {
                    cur_text.push(Inline::Text {
                        value: text.into_owned(),
                    });
                }
            }
            Ok(Event::End(ref e)) => match e.local_name().as_ref() {
                b"t" => {
                    in_t = false;
                }
                b"footnote" if in_footnote => {
                    if let Some(id) = cur_id.take() {
                        // Skip separator / continuation-separator (ids -1 and 0)
                        let skip = cur_type
                            .as_deref()
                            .map(|t| {
                                t == "separator"
                                    || t == "continuationSeparator"
                                    || t == "continuationNotice"
                            })
                            .unwrap_or(false)
                            || id == "-1"
                            || id == "0";

                        if !skip {
                            let content = if cur_text.is_empty() {
                                vec![]
                            } else {
                                vec![Block::Paragraph {
                                    content: cur_text.clone(),
                                }]
                            };
                            map.insert(id, content);
                        }
                    }
                    cur_text.clear();
                    cur_type = None;
                    in_footnote = false;
                }
                _ => {}
            },
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
    fn parse_footnotes_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:footnote w:type="separator" w:id="-1">
    <w:p><w:r><w:t>separator</w:t></w:r></w:p>
  </w:footnote>
  <w:footnote w:type="continuationSeparator" w:id="0">
    <w:p><w:r><w:t>continuation</w:t></w:r></w:p>
  </w:footnote>
  <w:footnote w:id="1">
    <w:p><w:r><w:t>First footnote text.</w:t></w:r></w:p>
  </w:footnote>
  <w:footnote w:id="2">
    <w:p><w:r><w:t>Second footnote text.</w:t></w:r></w:p>
  </w:footnote>
</w:footnotes>"#;

        let map = parse_footnotes(xml).unwrap();

        // Separators should be filtered out
        assert!(!map.contains_key("-1"));
        assert!(!map.contains_key("0"));

        // Real footnotes should be present
        assert!(map.contains_key("1"));
        assert!(map.contains_key("2"));

        let fn1 = &map["1"];
        assert_eq!(fn1.len(), 1);
        if let Block::Paragraph { content } = &fn1[0] {
            assert_eq!(content.len(), 1);
            if let Inline::Text { value } = &content[0] {
                assert_eq!(value, "First footnote text.");
            } else {
                panic!("expected Text inline");
            }
        } else {
            panic!("expected Paragraph block");
        }
    }

    #[test]
    fn parse_empty_footnotes() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
</w:footnotes>"#;

        let map = parse_footnotes(xml).unwrap();
        assert!(map.is_empty());
    }
}
