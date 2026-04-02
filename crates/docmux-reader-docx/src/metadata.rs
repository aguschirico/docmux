//! Parses Dublin Core metadata from `docProps/core.xml`.

// These items will be used once the document body parser is implemented.
#![allow(dead_code)]

use crate::DocxError;
use docmux_ast::{Author, Metadata};
use quick_xml::events::Event;
use quick_xml::Reader;

/// Parse `docProps/core.xml` (Dublin Core / OPC core properties) into [`Metadata`].
///
/// Fields parsed:
/// - `dc:title` → `metadata.title`
/// - `dc:creator` → `metadata.authors` (split by `;`)
/// - `dcterms:created` → `metadata.date`
/// - `dc:subject` + `cp:keywords` → `metadata.keywords` (split by `,` or `;`)
/// - `dc:description` → `metadata.abstract_text` (plain text paragraph)
pub(crate) fn parse_core_properties(xml: &str) -> Result<Metadata, DocxError> {
    use docmux_ast::{Block, Inline};

    let mut metadata = Metadata::default();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current_element: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                current_element =
                    Some(String::from_utf8_lossy(e.local_name().as_ref()).into_owned());
            }
            Ok(Event::Text(ref e)) => {
                if let Some(ref el) = current_element {
                    let text = e
                        .unescape()
                        .map_err(|err| DocxError::Xml(err.to_string()))?;
                    let text = text.trim().to_string();
                    if text.is_empty() {
                        continue;
                    }

                    match el.as_str() {
                        "title" => {
                            metadata.title = Some(text);
                        }
                        "creator" => {
                            // Split by ";" for multiple authors
                            let authors: Vec<Author> = text
                                .split(';')
                                .map(|s| s.trim())
                                .filter(|s| !s.is_empty())
                                .map(|name| Author {
                                    name: name.to_string(),
                                    affiliation: None,
                                    email: None,
                                    orcid: None,
                                })
                                .collect();
                            metadata.authors = authors;
                        }
                        "created" => {
                            // ISO 8601 date — keep as-is, trim trailing 'Z' or 'T' content
                            // e.g. "2023-01-15T00:00:00Z" → "2023-01-15T00:00:00Z"
                            metadata.date = Some(text);
                        }
                        "subject" | "keywords" => {
                            // Split by "," or ";" — add to existing keywords (union)
                            let separator = if text.contains(';') { ';' } else { ',' };
                            let kws: Vec<String> = text
                                .split(separator)
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            for kw in kws {
                                if !metadata.keywords.contains(&kw) {
                                    metadata.keywords.push(kw);
                                }
                            }
                        }
                        "description" => {
                            metadata.abstract_text = Some(vec![Block::Paragraph {
                                content: vec![Inline::Text { value: text }],
                            }]);
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::End(_)) => {
                current_element = None;
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocxError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(metadata)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_core_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties
    xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:dcterms="http://purl.org/dc/terms/"
    xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <dc:title>My Document Title</dc:title>
  <dc:creator>Alice Smith; Bob Jones</dc:creator>
  <dc:description>This is the abstract of the document.</dc:description>
  <dc:subject>Rust, WASM</dc:subject>
  <dcterms:created xsi:type="dcterms:W3CDTF">2023-01-15T00:00:00Z</dcterms:created>
  <cp:keywords>docmux, converter</cp:keywords>
</cp:coreProperties>"#;

        let meta = parse_core_properties(xml).unwrap();

        assert_eq!(meta.title.as_deref(), Some("My Document Title"));

        assert_eq!(meta.authors.len(), 2);
        assert_eq!(meta.authors[0].name, "Alice Smith");
        assert_eq!(meta.authors[1].name, "Bob Jones");

        assert_eq!(meta.date.as_deref(), Some("2023-01-15T00:00:00Z"));

        // subject + keywords merged
        assert!(meta.keywords.contains(&"Rust".to_string()));
        assert!(meta.keywords.contains(&"WASM".to_string()));
        assert!(meta.keywords.contains(&"docmux".to_string()));
        assert!(meta.keywords.contains(&"converter".to_string()));

        assert!(meta.abstract_text.is_some());
        if let Some(ref blocks) = meta.abstract_text {
            if let docmux_ast::Block::Paragraph { content } = &blocks[0] {
                if let docmux_ast::Inline::Text { value } = &content[0] {
                    assert_eq!(value, "This is the abstract of the document.");
                }
            }
        }
    }

    #[test]
    fn parse_empty_core_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties
    xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
    xmlns:dc="http://purl.org/dc/elements/1.1/">
</cp:coreProperties>"#;

        let meta = parse_core_properties(xml).unwrap();
        assert!(meta.title.is_none());
        assert!(meta.authors.is_empty());
        assert!(meta.date.is_none());
        assert!(meta.keywords.is_empty());
        assert!(meta.abstract_text.is_none());
    }

    #[test]
    fn parse_keywords_with_semicolons() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties
    xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
    xmlns:dc="http://purl.org/dc/elements/1.1/">
  <cp:keywords>alpha; beta; gamma</cp:keywords>
</cp:coreProperties>"#;

        let meta = parse_core_properties(xml).unwrap();
        assert_eq!(meta.keywords.len(), 3);
        assert!(meta.keywords.contains(&"alpha".to_string()));
        assert!(meta.keywords.contains(&"beta".to_string()));
        assert!(meta.keywords.contains(&"gamma".to_string()));
    }
}
