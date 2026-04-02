//! Parses OOXML `.rels` XML files into a map of relationship id → relationship.

// These items will be used once the document body parser is implemented.
#![allow(dead_code)]

use crate::DocxError;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

/// A single OOXML relationship entry.
#[derive(Debug, Clone)]
pub(crate) struct Relationship {
    pub(crate) rel_type: String,
    pub(crate) target: String,
    pub(crate) target_mode: Option<String>,
}

/// Map from rId to [`Relationship`].
pub(crate) type RelMap = HashMap<String, Relationship>;

/// Parse a `.rels` XML string into a [`RelMap`].
pub(crate) fn parse_relationships(xml: &str) -> Result<RelMap, DocxError> {
    let mut map = RelMap::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e))
                if e.local_name().as_ref() == b"Relationship" =>
            {
                let mut id = String::new();
                let mut rel_type = String::new();
                let mut target = String::new();
                let mut target_mode: Option<String> = None;

                for attr in e.attributes().flatten() {
                    match attr.key.local_name().as_ref() {
                        b"Id" => {
                            id = String::from_utf8_lossy(&attr.value).into_owned();
                        }
                        b"Type" => {
                            rel_type = String::from_utf8_lossy(&attr.value).into_owned();
                        }
                        b"Target" => {
                            target = String::from_utf8_lossy(&attr.value).into_owned();
                        }
                        b"TargetMode" => {
                            target_mode = Some(String::from_utf8_lossy(&attr.value).into_owned());
                        }
                        _ => {}
                    }
                }

                if !id.is_empty() {
                    map.insert(
                        id,
                        Relationship {
                            rel_type,
                            target,
                            target_mode,
                        },
                    );
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocxError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(map)
}

/// Returns `true` if the relationship type is a hyperlink.
pub(crate) fn is_hyperlink(rel_type: &str) -> bool {
    rel_type.contains("hyperlink")
}

/// Returns `true` if the relationship type is an image.
pub(crate) fn is_image(rel_type: &str) -> bool {
    rel_type.contains("image")
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_document_rels() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
</Relationships>"#;

        let map = parse_relationships(xml).unwrap();
        assert_eq!(map.len(), 3);

        let r1 = map.get("rId1").unwrap();
        assert!(r1.rel_type.contains("styles"));
        assert_eq!(r1.target, "styles.xml");
        assert!(r1.target_mode.is_none());

        let r2 = map.get("rId2").unwrap();
        assert!(is_hyperlink(&r2.rel_type));
        assert_eq!(r2.target, "https://example.com");
        assert_eq!(r2.target_mode.as_deref(), Some("External"));

        let r3 = map.get("rId3").unwrap();
        assert!(is_image(&r3.rel_type));
        assert_eq!(r3.target, "media/image1.png");
    }

    #[test]
    fn parse_empty_rels() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#;

        let map = parse_relationships(xml).unwrap();
        assert!(map.is_empty());
    }
}
