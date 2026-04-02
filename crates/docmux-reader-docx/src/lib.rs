//! # docmux-reader-docx
//!
//! DOCX reader for docmux — parses a DOCX ZIP archive into the docmux AST.

mod archive;
pub(crate) mod document;
pub(crate) mod footnotes;
pub(crate) mod metadata;
pub(crate) mod numbering;
pub(crate) mod relationships;
pub(crate) mod styles;

use archive::DocxArchive;
use docmux_ast::{Block, Document, ResourceData};
use docmux_core::{BinaryReader, Result as CoreResult};
use std::collections::HashMap;

// ─── Error type ──────────────────────────────────────────────────────────────

/// Errors specific to DOCX parsing.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DocxError {
    #[error("zip error: {0}")]
    Zip(String),
    #[error("utf-8 error: {0}")]
    Utf8(String),
    #[error("xml error: {0}")]
    Xml(String),
    #[error("missing part: {0}")]
    MissingPart(String),
}

impl From<DocxError> for docmux_core::ConvertError {
    fn from(e: DocxError) -> Self {
        docmux_core::ConvertError::Other(e.to_string())
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Infer MIME type from file extension.
fn mime_from_path(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "tiff" | "tif" => "image/tiff",
        "wmf" => "image/x-wmf",
        "emf" => "image/x-emf",
        _ => "application/octet-stream",
    }
}

// ─── DocxReader ──────────────────────────────────────────────────────────────

/// A reader that parses DOCX binary input into a [`Document`].
pub struct DocxReader;

impl DocxReader {
    /// Create a new `DocxReader`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DocxReader {
    fn default() -> Self {
        Self::new()
    }
}

impl BinaryReader for DocxReader {
    fn format(&self) -> &str {
        "docx"
    }

    fn extensions(&self) -> &[&str] {
        &["docx"]
    }

    fn read_bytes(&self, input: &[u8]) -> CoreResult<Document> {
        let archive = DocxArchive::from_bytes(input)?;

        // 1. Parse relationships (optional)
        let rels = if let Some(xml_result) = archive.get_xml("word/_rels/document.xml.rels") {
            let xml = xml_result?;
            relationships::parse_relationships(&xml)?
        } else {
            relationships::RelMap::new()
        };

        // 2. Parse styles (optional)
        let styles = if let Some(xml_result) = archive.get_xml("word/styles.xml") {
            let xml = xml_result?;
            styles::parse_styles(&xml)?
        } else {
            styles::StyleMap::new()
        };

        // 3. Parse numbering (optional)
        let numbering = if let Some(xml_result) = archive.get_xml("word/numbering.xml") {
            let xml = xml_result?;
            numbering::parse_numbering(&xml)?
        } else {
            numbering::NumberingMap::new()
        };

        // 4. Parse footnotes (optional)
        let footnote_map = if let Some(xml_result) = archive.get_xml("word/footnotes.xml") {
            let xml = xml_result?;
            footnotes::parse_footnotes(&xml)?
        } else {
            footnotes::FootnoteMap::new()
        };

        // 5. Parse metadata from Dublin Core (optional)
        let metadata_result = if let Some(xml_result) = archive.get_xml("docProps/core.xml") {
            let xml = xml_result?;
            metadata::parse_core_properties(&xml)?
        } else {
            docmux_ast::Metadata::default()
        };

        // 5b. Load embedded media resources
        let mut resources: HashMap<String, ResourceData> = HashMap::new();
        for full_path in archive.media_paths() {
            if let Some(bytes) = archive.get_bytes(full_path) {
                // Strip "word/" prefix → "media/image1.png"
                let key = full_path.strip_prefix("word/").unwrap_or(full_path);
                resources.insert(
                    key.to_string(),
                    ResourceData {
                        mime_type: mime_from_path(full_path).to_string(),
                        data: bytes.to_vec(),
                    },
                );
            }
        }

        // 6. Parse document body (required)
        let doc_xml = archive
            .get_xml("word/document.xml")
            .ok_or(DocxError::MissingPart("word/document.xml".to_string()))?
            .map_err(|e| DocxError::Utf8(e.to_string()))?;

        let ctx = document::ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
        };

        let mut content = document::parse_body(&doc_xml, &ctx)?;

        // 7. Append footnote definitions
        for (id, blocks) in &footnote_map {
            content.push(Block::FootnoteDef {
                id: id.clone(),
                content: blocks.clone(),
            });
        }

        Ok(Document {
            metadata: metadata_result,
            content,
            bibliography: None,
            warnings: vec![],
            resources,
        })
    }
}

// ─── Integration tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use zip::write::{FileOptions, ZipWriter};

    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zw = ZipWriter::new(cursor);
        let opts = FileOptions::<()>::default();
        for (name, data) in entries {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(data).unwrap();
        }
        zw.finish().unwrap();
        buf
    }

    #[test]
    fn read_empty_docx() {
        let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body/>
</w:document>"#;

        let zip_bytes = make_zip(&[("word/document.xml", doc_xml.as_bytes())]);
        let reader = DocxReader;
        let doc = reader.read_bytes(&zip_bytes).unwrap();
        assert!(doc.content.is_empty());
    }

    #[test]
    fn reject_invalid_bytes() {
        let reader = DocxReader;
        let result = reader.read_bytes(b"not a zip file at all");
        assert!(result.is_err());
    }

    #[test]
    fn read_docx_with_paragraph() {
        let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello from DOCX</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let zip_bytes = make_zip(&[("word/document.xml", doc_xml.as_bytes())]);
        let reader = DocxReader;
        let doc = reader.read_bytes(&zip_bytes).unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::Paragraph { content } = &doc.content[0] {
            assert_eq!(content.len(), 1);
            if let docmux_ast::Inline::Text { value } = &content[0] {
                assert_eq!(value, "Hello from DOCX");
            } else {
                panic!("expected Text inline");
            }
        } else {
            panic!("expected Paragraph block");
        }
    }

    #[test]
    fn read_docx_with_metadata() {
        let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body/>
</w:document>"#;

        let core_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties
    xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
    xmlns:dc="http://purl.org/dc/elements/1.1/"
    xmlns:dcterms="http://purl.org/dc/terms/">
  <dc:title>Test Title</dc:title>
  <dc:creator>Jane Doe</dc:creator>
</cp:coreProperties>"#;

        let zip_bytes = make_zip(&[
            ("word/document.xml", doc_xml.as_bytes()),
            ("docProps/core.xml", core_xml.as_bytes()),
        ]);
        let reader = DocxReader;
        let doc = reader.read_bytes(&zip_bytes).unwrap();
        assert_eq!(doc.metadata.title.as_deref(), Some("Test Title"));
        assert_eq!(doc.metadata.authors.len(), 1);
        assert_eq!(doc.metadata.authors[0].name, "Jane Doe");
    }

    #[test]
    fn read_docx_with_footnotes() {
        let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Text</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

        let footnotes_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:footnote w:type="separator" w:id="-1">
    <w:p><w:r><w:t>separator</w:t></w:r></w:p>
  </w:footnote>
  <w:footnote w:id="1">
    <w:p><w:r><w:t>A footnote.</w:t></w:r></w:p>
  </w:footnote>
</w:footnotes>"#;

        let zip_bytes = make_zip(&[
            ("word/document.xml", doc_xml.as_bytes()),
            ("word/footnotes.xml", footnotes_xml.as_bytes()),
        ]);
        let reader = DocxReader;
        let doc = reader.read_bytes(&zip_bytes).unwrap();
        // 1 paragraph + 1 footnote def
        assert_eq!(doc.content.len(), 2);
        let has_footnote_def = doc
            .content
            .iter()
            .any(|b| matches!(b, Block::FootnoteDef { .. }));
        assert!(has_footnote_def);
    }

    #[test]
    fn read_docx_loads_media_resources() {
        let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>Has image</w:t></w:r></w:p></w:body>
</w:document>"#;

        let fake_png = b"\x89PNG\r\n\x1a\nfake image data";
        let zip_bytes = make_zip(&[
            ("word/document.xml", doc_xml.as_bytes()),
            ("word/media/image1.png", fake_png),
        ]);

        let reader = DocxReader;
        let doc = reader.read_bytes(&zip_bytes).unwrap();
        assert_eq!(doc.resources.len(), 1);

        let res = doc
            .resources
            .get("media/image1.png")
            .expect("resource should exist");
        assert_eq!(res.mime_type, "image/png");
        assert_eq!(res.data, fake_png);
    }

    #[test]
    fn missing_document_xml_errors() {
        let zip_bytes = make_zip(&[("word/styles.xml", b"<styles/>")]);
        let reader = DocxReader;
        let result = reader.read_bytes(&zip_bytes);
        assert!(result.is_err());
    }

    // ─── Roundtrip tests (markdown → DOCX bytes → AST) ──────────────────────

    fn roundtrip(markdown: &str) -> Document {
        use docmux_core::{BinaryReader as _, Reader as _, WriteOptions, Writer as _};
        use docmux_reader_markdown::MarkdownReader;
        use docmux_writer_docx::DocxWriter;

        let md_reader = MarkdownReader::new();
        let doc = md_reader.read(markdown).unwrap();

        let docx_writer = DocxWriter::new();
        let bytes = docx_writer
            .write_bytes(&doc, &WriteOptions::default())
            .unwrap();

        DocxReader::new().read_bytes(&bytes).unwrap()
    }

    #[test]
    fn roundtrip_markdown_basic() {
        let recovered = roundtrip("# Hello\n\nBold paragraph.\n\n## Sub\n\nAnother.");
        let has_heading = recovered
            .content
            .iter()
            .any(|b| matches!(b, Block::Heading { .. }));
        let has_paragraph = recovered
            .content
            .iter()
            .any(|b| matches!(b, Block::Paragraph { .. }));
        assert!(
            has_heading,
            "expected at least one Heading in recovered AST"
        );
        assert!(
            has_paragraph,
            "expected at least one Paragraph in recovered AST"
        );
    }

    #[test]
    fn roundtrip_code_block() {
        let recovered = roundtrip("```rust\nfn main() {}\n```");
        let has_code = recovered
            .content
            .iter()
            .any(|b| matches!(b, Block::CodeBlock { .. }));
        assert!(has_code, "expected a CodeBlock in recovered AST");
    }

    #[test]
    fn roundtrip_table() {
        let recovered = roundtrip("| A | B |\n|---|---|\n| 1 | 2 |");
        let has_table = recovered
            .content
            .iter()
            .any(|b| matches!(b, Block::Table(_)));
        assert!(has_table, "expected a Table in recovered AST");
    }
}
