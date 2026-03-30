# DOCX Writer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a DOCX writer that produces valid Office Open XML files from the docmux AST, using raw XML generation + the `zip` crate.

**Architecture:** Single `DocxWriter` struct implementing the `Writer` trait. A `DocxBuilder` accumulates relationships, footnotes, media, and numbering definitions during AST traversal, then assembles the ZIP. All XML is generated with `format!()` / `write!()` — no XML builder crate.

**Tech Stack:** Rust, `zip` crate (v2, deflate), Office Open XML (ECMA-376)

**Spec:** `docs/superpowers/specs/2026-03-27-docx-writer-design.md`

---

### Task 1: Add `zip` dependency and scaffold `DocxWriter` struct

**Files:**
- Modify: `Cargo.toml` (workspace root, line 65 — add `zip` to workspace deps)
- Modify: `crates/docmux-writer-docx/Cargo.toml` (line 12 — add `zip`)
- Modify: `crates/docmux-writer-docx/src/lib.rs` (replace placeholder)

- [ ] **Step 1: Add `zip` to workspace dependencies**

In root `Cargo.toml`, add after the `syntect` line (line 65):

```toml
zip = { version = "2", default-features = false, features = ["deflate"] }
```

- [ ] **Step 2: Add `zip` to docx crate dependencies**

In `crates/docmux-writer-docx/Cargo.toml`, add:

```toml
zip = { workspace = true }
```

- [ ] **Step 3: Write the failing test — DocxWriter implements Writer trait**

Replace `crates/docmux-writer-docx/src/lib.rs` with:

```rust
//! # docmux-writer-docx
//!
//! DOCX (Office Open XML) writer for docmux.
//! Generates .docx files as ZIP archives containing WordprocessingML XML.

use docmux_ast::Document;
use docmux_core::{ConvertError, Result, WriteOptions, Writer};

pub struct DocxWriter;

impl DocxWriter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DocxWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer for DocxWriter {
    fn format(&self) -> &str {
        "docx"
    }

    fn default_extension(&self) -> &str {
        "docx"
    }

    fn write(&self, _doc: &Document, _opts: &WriteOptions) -> Result<String> {
        Err(ConvertError::Unsupported(
            "DOCX is a binary format — use write_bytes() instead".into(),
        ))
    }

    fn write_bytes(&self, _doc: &Document, _opts: &WriteOptions) -> Result<Vec<u8>> {
        todo!("DOCX generation not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_metadata() {
        let w = DocxWriter::new();
        assert_eq!(w.format(), "docx");
        assert_eq!(w.default_extension(), "docx");
    }

    #[test]
    fn write_returns_unsupported() {
        let w = DocxWriter::new();
        let doc = Document::default();
        let opts = WriteOptions::default();
        let result = w.write(&doc, &opts);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-docx`
Expected: 2 tests pass (trait_metadata, write_returns_unsupported)

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/docmux-writer-docx/
git commit -m "feat(docx): scaffold DocxWriter with zip dependency and Writer trait"
```

---

### Task 2: Minimal ZIP — empty document

**Files:**
- Modify: `crates/docmux-writer-docx/src/lib.rs`

- [ ] **Step 1: Write the failing test — empty doc produces valid ZIP**

Add to the `tests` module:

```rust
#[test]
fn empty_doc_produces_valid_zip() {
    let w = DocxWriter::new();
    let doc = Document::default();
    let opts = WriteOptions::default();
    let bytes = w.write_bytes(&doc, &opts).expect("write_bytes failed");

    // Verify it's a valid ZIP
    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).expect("not a valid ZIP");

    // Check required OOXML parts exist
    let mut names = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i).unwrap();
        names.push(file.name().to_string());
    }

    assert!(names.contains(&"[Content_Types].xml".to_string()));
    assert!(names.contains(&"_rels/.rels".to_string()));
    assert!(names.contains(&"word/document.xml".to_string()));
    assert!(names.contains(&"word/styles.xml".to_string()));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p docmux-writer-docx empty_doc_produces_valid_zip`
Expected: FAIL (panics on `todo!()`)

- [ ] **Step 3: Implement minimal ZIP generation**

Add the XML escaping helper and `DocxBuilder` struct. Replace `write_bytes` implementation:

```rust
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Escape text for XML content.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
    out
}

struct Relationship {
    id: String,
    rel_type: String,
    target: String,
}

struct DocxBuilder {
    body_xml: String,
    relationships: Vec<Relationship>,
    footnotes: Vec<(u32, String)>,
    media: Vec<(String, Vec<u8>)>,
    numbering_xml: Option<String>,
    next_rel_id: u32,
    next_footnote_id: u32,
    next_image_id: u32,
}

impl DocxBuilder {
    fn new() -> Self {
        Self {
            body_xml: String::new(),
            relationships: Vec::new(),
            footnotes: Vec::new(),
            media: Vec::new(),
            numbering_xml: None,
            next_rel_id: 1,
            next_footnote_id: 2, // 0 and 1 reserved by Word
            next_image_id: 1,
        }
    }

    fn add_relationship(&mut self, rel_type: &str, target: &str) -> String {
        let id = format!("rId{}", self.next_rel_id);
        self.next_rel_id += 1;
        self.relationships.push(Relationship {
            id: id.clone(),
            rel_type: rel_type.to_string(),
            target: target.to_string(),
        });
        id
    }

    fn build_content_types(&self) -> String {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>"#,
        );
        if !self.footnotes.is_empty() {
            xml.push_str(
                r#"
  <Override PartName="/word/footnotes.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml"/>"#,
            );
        }
        if self.numbering_xml.is_some() {
            xml.push_str(
                r#"
  <Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>"#,
            );
        }
        for (name, _) in &self.media {
            let ext = name.rsplit('.').next().unwrap_or("bin");
            let content_type = match ext {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                _ => "application/octet-stream",
            };
            write!(
                xml,
                r#"
  <Default Extension="{ext}" ContentType="{content_type}"/>"#,
            )
            .unwrap();
        }
        xml.push_str(
            r#"
</Types>"#,
        );
        xml
    }

    fn build_root_rels(&self) -> String {
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#
            .to_string()
    }

    fn build_document_xml(&self) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:wpc="http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"
            xmlns:o="urn:schemas-microsoft-com:office:office"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"
            xmlns:v="urn:schemas-microsoft-com:vml"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            xmlns:w10="urn:schemas-microsoft-com:office:word"
            xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"
            xmlns:wpg="http://schemas.microsoft.com/office/word/2010/wordprocessingGroup"
            xmlns:wpi="http://schemas.microsoft.com/office/word/2010/wordprocessingInk"
            xmlns:wne="http://schemas.microsoft.com/office/word/2006/wordml"
            xmlns:wps="http://schemas.microsoft.com/office/word/2010/wordprocessingShape"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            mc:Ignorable="w14 wp14">
  <w:body>
{body}    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/>
    </w:sectPr>
  </w:body>
</w:document>"#,
            body = self.body_xml,
        )
    }

    fn build_document_rels(&self) -> String {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
        );
        if !self.footnotes.is_empty() {
            xml.push_str(
                r#"
  <Relationship Id="rIdFootnotes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes" Target="footnotes.xml"/>"#,
            );
        }
        if self.numbering_xml.is_some() {
            xml.push_str(
                r#"
  <Relationship Id="rIdNumbering" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>"#,
            );
        }
        for rel in &self.relationships {
            write!(
                xml,
                r#"
  <Relationship Id="{}" Type="{}" Target="{}"/>"#,
                rel.id, rel.rel_type, rel.target,
            )
            .unwrap();
        }
        xml.push_str(
            r#"
</Relationships>"#,
        );
        xml
    }

    fn build_styles_xml(&self) -> String {
        include_str!("styles.xml").to_string()
    }

    fn assemble_zip(&self) -> Result<Vec<u8>> {
        let buf = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);
        let options = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);

        zip.start_file("[Content_Types].xml", options)
            .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
        zip.write_all(self.build_content_types().as_bytes())
            .map_err(ConvertError::Io)?;

        zip.start_file("_rels/.rels", options)
            .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
        zip.write_all(self.build_root_rels().as_bytes())
            .map_err(ConvertError::Io)?;

        zip.start_file("word/document.xml", options)
            .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
        zip.write_all(self.build_document_xml().as_bytes())
            .map_err(ConvertError::Io)?;

        zip.start_file("word/styles.xml", options)
            .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
        zip.write_all(self.build_styles_xml().as_bytes())
            .map_err(ConvertError::Io)?;

        zip.start_file("word/_rels/document.xml.rels", options)
            .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
        zip.write_all(self.build_document_rels().as_bytes())
            .map_err(ConvertError::Io)?;

        if !self.footnotes.is_empty() {
            zip.start_file("word/footnotes.xml", options)
                .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
            zip.write_all(self.build_footnotes_xml().as_bytes())
                .map_err(ConvertError::Io)?;
        }

        if let Some(numbering) = &self.numbering_xml {
            zip.start_file("word/numbering.xml", options)
                .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
            zip.write_all(numbering.as_bytes())
                .map_err(ConvertError::Io)?;
        }

        for (name, data) in &self.media {
            zip.start_file(format!("word/media/{name}"), options)
                .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
            zip.write_all(data).map_err(ConvertError::Io)?;
        }

        let cursor = zip
            .finish()
            .map_err(|e| ConvertError::Other(format!("zip finish error: {e}")))?;
        Ok(cursor.into_inner())
    }

    fn build_footnotes_xml(&self) -> String {
        let mut xml = String::from(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:footnote w:type="separator" w:id="0">
    <w:p><w:r><w:separator/></w:r></w:p>
  </w:footnote>
  <w:footnote w:type="continuationSeparator" w:id="1">
    <w:p><w:r><w:continuationSeparator/></w:r></w:p>
  </w:footnote>"#,
        );
        for (id, content) in &self.footnotes {
            write!(xml, r#"
  <w:footnote w:id="{id}">{content}
  </w:footnote>"#).unwrap();
        }
        xml.push_str(
            r#"
</w:footnotes>"#,
        );
        xml
    }
}
```

Update `write_bytes` in the `Writer` impl:

```rust
fn write_bytes(&self, doc: &Document, _opts: &WriteOptions) -> Result<Vec<u8>> {
    let mut builder = DocxBuilder::new();
    // For now, empty body — blocks will be added in later tasks
    let _ = doc;
    builder.assemble_zip()
}
```

- [ ] **Step 4: Create `styles.xml` static file**

Create `crates/docmux-writer-docx/src/styles.xml` with base styles (Normal, Heading1-6, Title, Author, Date, CodeBlock, BlockQuote, Caption, Abstract, FootnoteText, FootnoteReference, Hyperlink, CodeChar, MathBlock):

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:docDefaults>
    <w:rPrDefault>
      <w:rPr>
        <w:rFonts w:ascii="Calibri" w:hAnsi="Calibri" w:cs="Calibri"/>
        <w:sz w:val="22"/>
        <w:szCs w:val="22"/>
        <w:lang w:val="en-US"/>
      </w:rPr>
    </w:rPrDefault>
    <w:pPrDefault>
      <w:pPr>
        <w:spacing w:after="160" w:line="259" w:lineRule="auto"/>
      </w:pPr>
    </w:pPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:keepNext/><w:spacing w:before="240" w:after="120"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="32"/><w:szCs w:val="32"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading2">
    <w:name w:val="heading 2"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:keepNext/><w:spacing w:before="200" w:after="100"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="28"/><w:szCs w:val="28"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading3">
    <w:name w:val="heading 3"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:keepNext/><w:spacing w:before="160" w:after="80"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="24"/><w:szCs w:val="24"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading4">
    <w:name w:val="heading 4"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:keepNext/><w:spacing w:before="120" w:after="60"/></w:pPr>
    <w:rPr><w:b/><w:i/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading5">
    <w:name w:val="heading 5"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:keepNext/><w:spacing w:before="80" w:after="40"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading6">
    <w:name w:val="heading 6"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:keepNext/><w:spacing w:before="80" w:after="40"/></w:pPr>
    <w:rPr><w:b/><w:i/><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Title">
    <w:name w:val="Title"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:spacing w:after="200"/><w:jc w:val="center"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="56"/><w:szCs w:val="56"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Author">
    <w:name w:val="Author"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:jc w:val="center"/></w:pPr>
    <w:rPr><w:sz w:val="24"/><w:szCs w:val="24"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Date">
    <w:name w:val="Date"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:jc w:val="center"/><w:spacing w:after="400"/></w:pPr>
    <w:rPr><w:color w:val="666666"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Abstract">
    <w:name w:val="Abstract"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:ind w:left="720" w:right="720"/></w:pPr>
    <w:rPr><w:i/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="CodeBlock">
    <w:name w:val="Code Block"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr>
      <w:shd w:val="clear" w:color="auto" w:fill="F5F5F5"/>
      <w:spacing w:before="120" w:after="120" w:line="240" w:lineRule="auto"/>
    </w:pPr>
    <w:rPr><w:rFonts w:ascii="Courier New" w:hAnsi="Courier New" w:cs="Courier New"/><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="BlockQuote">
    <w:name w:val="Block Quote"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr>
      <w:ind w:left="720"/>
      <w:pBdr><w:left w:val="single" w:sz="12" w:space="8" w:color="CCCCCC"/></w:pBdr>
    </w:pPr>
    <w:rPr><w:i/><w:color w:val="555555"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Caption">
    <w:name w:val="Caption"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:jc w:val="center"/></w:pPr>
    <w:rPr><w:i/><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="MathBlock">
    <w:name w:val="Math Block"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:jc w:val="center"/><w:spacing w:before="120" w:after="120"/></w:pPr>
    <w:rPr><w:rFonts w:ascii="Cambria Math" w:hAnsi="Cambria Math"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="FootnoteText">
    <w:name w:val="footnote text"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:spacing w:after="0" w:line="240" w:lineRule="auto"/></w:pPr>
    <w:rPr><w:sz w:val="18"/><w:szCs w:val="18"/></w:rPr>
  </w:style>
  <w:style w:type="character" w:styleId="FootnoteReference">
    <w:name w:val="footnote reference"/>
    <w:rPr><w:vertAlign w:val="superscript"/></w:rPr>
  </w:style>
  <w:style w:type="character" w:styleId="Hyperlink">
    <w:name w:val="Hyperlink"/>
    <w:rPr><w:color w:val="0563C1"/><w:u w:val="single"/></w:rPr>
  </w:style>
  <w:style w:type="character" w:styleId="CodeChar">
    <w:name w:val="Code Char"/>
    <w:rPr>
      <w:rFonts w:ascii="Courier New" w:hAnsi="Courier New" w:cs="Courier New"/>
      <w:sz w:val="20"/><w:szCs w:val="20"/>
      <w:shd w:val="clear" w:color="auto" w:fill="F5F5F5"/>
    </w:rPr>
  </w:style>
</w:styles>
```

- [ ] **Step 5: Run test**

Run: `cargo test -p docmux-writer-docx empty_doc_produces_valid_zip`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-writer-docx/
git commit -m "feat(docx): minimal ZIP generation with OOXML structure and styles"
```

---

### Task 3: Paragraphs and inline text rendering

**Files:**
- Modify: `crates/docmux-writer-docx/src/lib.rs`

- [ ] **Step 1: Write the failing test — paragraph with inline formatting**

```rust
#[test]
fn paragraph_with_inlines() {
    use docmux_ast::{Block, Inline};

    let doc = Document {
        content: vec![Block::Paragraph {
            content: vec![
                Inline::Text { value: "Hello ".into() },
                Inline::Strong { content: vec![Inline::Text { value: "bold".into() }] },
                Inline::Text { value: " and ".into() },
                Inline::Emphasis { content: vec![Inline::Text { value: "italic".into() }] },
            ],
        }],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains("<w:t xml:space=\"preserve\">Hello </w:t>"));
    assert!(xml.contains("<w:b/>"));
    assert!(xml.contains("<w:t>bold</w:t>"));
    assert!(xml.contains("<w:i/>"));
    assert!(xml.contains("<w:t>italic</w:t>"));
}

/// Helper: extract word/document.xml from DOCX bytes.
fn extract_document_xml(bytes: &[u8]) -> String {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut file = archive.by_name("word/document.xml").unwrap();
    let mut contents = String::new();
    std::io::Read::read_to_string(&mut file, &mut contents).unwrap();
    contents
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p docmux-writer-docx paragraph_with_inlines`
Expected: FAIL

- [ ] **Step 3: Implement `write_blocks`, `write_block`, `write_inlines`, `write_inline`**

Add methods to `DocxBuilder`:

```rust
impl DocxBuilder {
    fn write_blocks(&mut self, blocks: &[Block]) {
        for block in blocks {
            self.write_block(block);
        }
    }

    fn write_block(&mut self, block: &Block) {
        match block {
            Block::Paragraph { content } => {
                self.body_xml.push_str("    <w:p>\n");
                self.write_inlines(content, &[]);
                self.body_xml.push_str("    </w:p>\n");
            }
            // Other block types will be added in later tasks
            _ => {}
        }
    }

    fn write_inlines(&mut self, inlines: &[Inline], run_props: &[&str]) {
        for inline in inlines {
            self.write_inline(inline, run_props);
        }
    }

    fn write_inline(&mut self, inline: &Inline, run_props: &[&str]) {
        match inline {
            Inline::Text { value } => {
                self.body_xml.push_str("      <w:r>");
                if !run_props.is_empty() {
                    self.body_xml.push_str("<w:rPr>");
                    for prop in run_props {
                        self.body_xml.push_str(prop);
                    }
                    self.body_xml.push_str("</w:rPr>");
                }
                let escaped = xml_escape(value);
                // Preserve leading/trailing spaces
                if escaped.starts_with(' ') || escaped.ends_with(' ') {
                    write!(self.body_xml, "<w:t xml:space=\"preserve\">{escaped}</w:t>")
                        .unwrap();
                } else {
                    write!(self.body_xml, "<w:t>{escaped}</w:t>").unwrap();
                }
                self.body_xml.push_str("</w:r>\n");
            }
            Inline::Strong { content } => {
                let mut props = run_props.to_vec();
                props.push("<w:b/>");
                self.write_inlines(content, &props);
            }
            Inline::Emphasis { content } => {
                let mut props = run_props.to_vec();
                props.push("<w:i/>");
                self.write_inlines(content, &props);
            }
            Inline::Strikethrough { content } => {
                let mut props = run_props.to_vec();
                props.push("<w:strike/>");
                self.write_inlines(content, &props);
            }
            Inline::Underline { content } => {
                let mut props = run_props.to_vec();
                props.push(r#"<w:u w:val="single"/>"#);
                self.write_inlines(content, &props);
            }
            Inline::Superscript { content } => {
                let mut props = run_props.to_vec();
                props.push(r#"<w:vertAlign w:val="superscript"/>"#);
                self.write_inlines(content, &props);
            }
            Inline::Subscript { content } => {
                let mut props = run_props.to_vec();
                props.push(r#"<w:vertAlign w:val="subscript"/>"#);
                self.write_inlines(content, &props);
            }
            Inline::SmallCaps { content } => {
                let mut props = run_props.to_vec();
                props.push("<w:smallCaps/>");
                self.write_inlines(content, &props);
            }
            Inline::Code { value, .. } => {
                self.body_xml.push_str("      <w:r><w:rPr>");
                for prop in run_props {
                    self.body_xml.push_str(prop);
                }
                self.body_xml.push_str(
                    r#"<w:rFonts w:ascii="Courier New" w:hAnsi="Courier New" w:cs="Courier New"/><w:sz w:val="20"/><w:szCs w:val="20"/>"#,
                );
                self.body_xml.push_str("</w:rPr>");
                write!(self.body_xml, "<w:t>{}</w:t>", xml_escape(value)).unwrap();
                self.body_xml.push_str("</w:r>\n");
            }
            Inline::MathInline { value } => {
                // Plain text rendering for now (OMML future)
                self.body_xml.push_str("      <w:r>");
                if !run_props.is_empty() {
                    self.body_xml.push_str("<w:rPr>");
                    for prop in run_props {
                        self.body_xml.push_str(prop);
                    }
                    self.body_xml.push_str("</w:rPr>");
                }
                write!(self.body_xml, "<w:t>{}</w:t>", xml_escape(value)).unwrap();
                self.body_xml.push_str("</w:r>\n");
            }
            Inline::SoftBreak => {
                self.body_xml.push_str(
                    "      <w:r><w:t xml:space=\"preserve\"> </w:t></w:r>\n",
                );
            }
            Inline::HardBreak => {
                self.body_xml.push_str("      <w:r><w:br/></w:r>\n");
            }
            Inline::Quoted { quote_type, content } => {
                let (open, close) = match quote_type {
                    docmux_ast::QuoteType::SingleQuote => ("\u{2018}", "\u{2019}"),
                    docmux_ast::QuoteType::DoubleQuote => ("\u{201C}", "\u{201D}"),
                };
                self.body_xml.push_str("      <w:r>");
                if !run_props.is_empty() {
                    self.body_xml.push_str("<w:rPr>");
                    for prop in run_props {
                        self.body_xml.push_str(prop);
                    }
                    self.body_xml.push_str("</w:rPr>");
                }
                write!(self.body_xml, "<w:t>{open}</w:t>").unwrap();
                self.body_xml.push_str("</w:r>\n");
                self.write_inlines(content, run_props);
                self.body_xml.push_str("      <w:r>");
                if !run_props.is_empty() {
                    self.body_xml.push_str("<w:rPr>");
                    for prop in run_props {
                        self.body_xml.push_str(prop);
                    }
                    self.body_xml.push_str("</w:rPr>");
                }
                write!(self.body_xml, "<w:t>{close}</w:t>").unwrap();
                self.body_xml.push_str("</w:r>\n");
            }
            Inline::Span { content, .. } => {
                self.write_inlines(content, run_props);
            }
            Inline::RawInline { format, content } => {
                if format == "docx" || format == "openxml" {
                    self.body_xml.push_str(content);
                }
                // Other formats silently skipped
            }
            Inline::Citation(cite) => {
                // Plain text rendering: [key1; key2]
                let keys: Vec<&str> = cite.keys();
                let text = format!("[{}]", keys.join("; "));
                self.body_xml.push_str("      <w:r>");
                if !run_props.is_empty() {
                    self.body_xml.push_str("<w:rPr>");
                    for prop in run_props {
                        self.body_xml.push_str(prop);
                    }
                    self.body_xml.push_str("</w:rPr>");
                }
                write!(self.body_xml, "<w:t>{}</w:t>", xml_escape(&text)).unwrap();
                self.body_xml.push_str("</w:r>\n");
            }
            Inline::CrossRef(xref) => {
                // Render as the target label (crossref transform resolves these before writing)
                self.body_xml.push_str("      <w:r>");
                if !run_props.is_empty() {
                    self.body_xml.push_str("<w:rPr>");
                    for prop in run_props {
                        self.body_xml.push_str(prop);
                    }
                    self.body_xml.push_str("</w:rPr>");
                }
                write!(self.body_xml, "<w:t>{}</w:t>", xml_escape(&xref.target)).unwrap();
                self.body_xml.push_str("</w:r>\n");
            }
            // FootnoteRef and Link/Image handled in later tasks
            _ => {}
        }
    }
}
```

Update `write_bytes`:

```rust
fn write_bytes(&self, doc: &Document, _opts: &WriteOptions) -> Result<Vec<u8>> {
    let mut builder = DocxBuilder::new();
    builder.write_blocks(&doc.content);
    builder.assemble_zip()
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p docmux-writer-docx`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-docx/
git commit -m "feat(docx): paragraph and inline text rendering with formatting"
```

---

### Task 4: Headings and metadata

**Files:**
- Modify: `crates/docmux-writer-docx/src/lib.rs`

- [ ] **Step 1: Write the failing test — headings**

```rust
#[test]
fn headings_use_heading_styles() {
    use docmux_ast::{Block, Inline};

    let doc = Document {
        content: vec![
            Block::Heading {
                level: 1,
                id: Some("intro".into()),
                content: vec![Inline::Text { value: "Introduction".into() }],
                attrs: None,
            },
            Block::Heading {
                level: 2,
                id: None,
                content: vec![Inline::Text { value: "Sub".into() }],
                attrs: None,
            },
        ],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains(r#"<w:pStyle w:val="Heading1"/>"#));
    assert!(xml.contains("<w:t>Introduction</w:t>"));
    assert!(xml.contains(r#"<w:pStyle w:val="Heading2"/>"#));
}
```

- [ ] **Step 2: Write the failing test — metadata**

```rust
#[test]
fn metadata_renders_title_author_date() {
    use docmux_ast::{Author, Metadata};

    let doc = Document {
        metadata: Metadata {
            title: Some("My Paper".into()),
            authors: vec![Author {
                name: "Jane Doe".into(),
                affiliation: Some("MIT".into()),
                email: None,
                orcid: None,
            }],
            date: Some("2026-01-01".into()),
            ..Default::default()
        },
        content: vec![],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains(r#"<w:pStyle w:val="Title"/>"#));
    assert!(xml.contains("<w:t>My Paper</w:t>"));
    assert!(xml.contains(r#"<w:pStyle w:val="Author"/>"#));
    assert!(xml.contains("<w:t>Jane Doe</w:t>"));
    assert!(xml.contains(r#"<w:pStyle w:val="Date"/>"#));
    assert!(xml.contains("<w:t>2026-01-01</w:t>"));
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-docx headings_use_heading_styles metadata_renders`
Expected: FAIL

- [ ] **Step 4: Implement headings in `write_block`**

Add to the `match block` in `write_block`:

```rust
Block::Heading { level, content, .. } => {
    let style = format!("Heading{}", level.min(6));
    write!(
        self.body_xml,
        "    <w:p><w:pPr><w:pStyle w:val=\"{style}\"/></w:pPr>\n"
    )
    .unwrap();
    self.write_inlines(content, &[]);
    self.body_xml.push_str("    </w:p>\n");
}
```

- [ ] **Step 5: Implement metadata rendering**

Add a `write_metadata` method to `DocxBuilder` and call it before `write_blocks` in `write_bytes`:

```rust
fn write_metadata(&mut self, meta: &docmux_ast::Metadata) {
    if let Some(title) = &meta.title {
        write!(
            self.body_xml,
            "    <w:p><w:pPr><w:pStyle w:val=\"Title\"/></w:pPr>\
             <w:r><w:t>{}</w:t></w:r></w:p>\n",
            xml_escape(title)
        )
        .unwrap();
    }
    for author in &meta.authors {
        write!(
            self.body_xml,
            "    <w:p><w:pPr><w:pStyle w:val=\"Author\"/></w:pPr>\
             <w:r><w:t>{}</w:t></w:r></w:p>\n",
            xml_escape(&author.name)
        )
        .unwrap();
    }
    if let Some(date) = &meta.date {
        write!(
            self.body_xml,
            "    <w:p><w:pPr><w:pStyle w:val=\"Date\"/></w:pPr>\
             <w:r><w:t>{}</w:t></w:r></w:p>\n",
            xml_escape(date)
        )
        .unwrap();
    }
    if let Some(abstract_blocks) = &meta.abstract_text {
        for block in abstract_blocks {
            // Wrap abstract blocks with Abstract style
            match block {
                Block::Paragraph { content } => {
                    self.body_xml.push_str(
                        "    <w:p><w:pPr><w:pStyle w:val=\"Abstract\"/></w:pPr>\n",
                    );
                    self.write_inlines(content, &[]);
                    self.body_xml.push_str("    </w:p>\n");
                }
                other => self.write_block(other),
            }
        }
    }
}
```

Update `write_bytes`:

```rust
fn write_bytes(&self, doc: &Document, _opts: &WriteOptions) -> Result<Vec<u8>> {
    let mut builder = DocxBuilder::new();
    builder.write_metadata(&doc.metadata);
    builder.write_blocks(&doc.content);
    builder.assemble_zip()
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p docmux-writer-docx`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-writer-docx/
git commit -m "feat(docx): headings with Word styles and metadata (title/author/date/abstract)"
```

---

### Task 5: Code blocks, math blocks, block quotes, thematic breaks

**Files:**
- Modify: `crates/docmux-writer-docx/src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn code_block_uses_code_style() {
    let doc = Document {
        content: vec![Block::CodeBlock {
            language: Some("rust".into()),
            content: "fn main() {}".into(),
            caption: None,
            label: None,
            attrs: None,
        }],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains(r#"<w:pStyle w:val="CodeBlock"/>"#));
    assert!(xml.contains("<w:t>fn main() {}</w:t>"));
}

#[test]
fn math_block_renders_plain_text() {
    let doc = Document {
        content: vec![Block::MathBlock {
            content: "E = mc^2".into(),
            label: None,
        }],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains(r#"<w:pStyle w:val="MathBlock"/>"#));
    assert!(xml.contains("<w:t>E = mc^2</w:t>"));
}

#[test]
fn blockquote_uses_blockquote_style() {
    let doc = Document {
        content: vec![Block::BlockQuote {
            content: vec![Block::Paragraph {
                content: vec![Inline::Text { value: "A quote".into() }],
            }],
        }],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains(r#"<w:pStyle w:val="BlockQuote"/>"#));
    assert!(xml.contains("<w:t>A quote</w:t>"));
}

#[test]
fn thematic_break_renders_border() {
    let doc = Document {
        content: vec![Block::ThematicBreak],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains("<w:pBdr>"));
    assert!(xml.contains(r#"<w:bottom w:val="single""#));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-docx code_block math_block blockquote thematic`
Expected: FAIL

- [ ] **Step 3: Implement block types in `write_block`**

Add cases to the `match block`:

```rust
Block::CodeBlock { content, .. } => {
    // Split by lines, each line is a paragraph with CodeBlock style
    for (i, line) in content.lines().enumerate() {
        self.body_xml.push_str(
            "    <w:p><w:pPr><w:pStyle w:val=\"CodeBlock\"/></w:pPr>",
        );
        if line.is_empty() && i < content.lines().count() - 1 {
            // Empty line — still need a run for spacing
            self.body_xml.push_str("<w:r><w:t></w:t></w:r>");
        } else {
            write!(
                self.body_xml,
                "<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>",
                xml_escape(line)
            )
            .unwrap();
        }
        self.body_xml.push_str("</w:p>\n");
    }
}
Block::MathBlock { content, .. } => {
    self.body_xml.push_str(
        "    <w:p><w:pPr><w:pStyle w:val=\"MathBlock\"/></w:pPr>",
    );
    write!(
        self.body_xml,
        "<w:r><w:t>{}</w:t></w:r>",
        xml_escape(content)
    )
    .unwrap();
    self.body_xml.push_str("</w:p>\n");
}
Block::BlockQuote { content } => {
    // Render each child block with BlockQuote style override
    for child in content {
        match child {
            Block::Paragraph { content: inlines } => {
                self.body_xml.push_str(
                    "    <w:p><w:pPr><w:pStyle w:val=\"BlockQuote\"/></w:pPr>\n",
                );
                self.write_inlines(inlines, &[]);
                self.body_xml.push_str("    </w:p>\n");
            }
            other => self.write_block(other),
        }
    }
}
Block::ThematicBreak => {
    self.body_xml.push_str(
        "    <w:p><w:pPr><w:pBdr>\
         <w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"auto\"/>\
         </w:pBdr></w:pPr></w:p>\n",
    );
}
Block::RawBlock { format, content } => {
    if format == "docx" || format == "openxml" {
        self.body_xml.push_str(content);
        self.body_xml.push('\n');
    }
}
Block::Div { content, .. } => {
    self.write_blocks(content);
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p docmux-writer-docx`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-docx/
git commit -m "feat(docx): code blocks, math blocks, blockquotes, thematic breaks, raw blocks, divs"
```

---

### Task 6: Tables

**Files:**
- Modify: `crates/docmux-writer-docx/src/lib.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn table_renders_with_header_and_rows() {
    use docmux_ast::{Alignment, ColumnSpec, Table, TableCell};

    let doc = Document {
        content: vec![Block::Table(Table {
            caption: Some(vec![Inline::Text { value: "Results".into() }]),
            label: None,
            columns: vec![
                ColumnSpec { alignment: Alignment::Left, width: None },
                ColumnSpec { alignment: Alignment::Right, width: None },
            ],
            header: Some(vec![
                TableCell { content: vec![Block::Paragraph { content: vec![Inline::Text { value: "Name".into() }] }], colspan: 1, rowspan: 1 },
                TableCell { content: vec![Block::Paragraph { content: vec![Inline::Text { value: "Score".into() }] }], colspan: 1, rowspan: 1 },
            ]),
            rows: vec![vec![
                TableCell { content: vec![Block::Paragraph { content: vec![Inline::Text { value: "Alice".into() }] }], colspan: 1, rowspan: 1 },
                TableCell { content: vec![Block::Paragraph { content: vec![Inline::Text { value: "95".into() }] }], colspan: 1, rowspan: 1 },
            ]],
            foot: None,
            attrs: None,
        })],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains("<w:tbl>"));
    assert!(xml.contains("<w:tr>"));
    assert!(xml.contains("<w:tc>"));
    assert!(xml.contains("<w:t>Name</w:t>"));
    assert!(xml.contains("<w:t>Alice</w:t>"));
    // Caption rendered as a paragraph
    assert!(xml.contains("<w:t>Results</w:t>"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p docmux-writer-docx table_renders`
Expected: FAIL

- [ ] **Step 3: Implement table rendering**

Add a `write_table` method and the `Table` case in `write_block`:

```rust
Block::Table(table) => {
    self.write_table(table);
}
```

```rust
fn write_table(&mut self, table: &docmux_ast::Table) {
    // Caption before table
    if let Some(caption) = &table.caption {
        self.body_xml.push_str(
            "    <w:p><w:pPr><w:pStyle w:val=\"Caption\"/></w:pPr>\n",
        );
        self.write_inlines(caption, &[]);
        self.body_xml.push_str("    </w:p>\n");
    }

    self.body_xml.push_str("    <w:tbl>\n");

    // Table properties
    self.body_xml.push_str(
        "      <w:tblPr>\
         <w:tblStyle w:val=\"TableGrid\"/>\
         <w:tblW w:w=\"0\" w:type=\"auto\"/>\
         <w:tblBorders>\
         <w:top w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\
         <w:left w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\
         <w:bottom w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\
         <w:right w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\
         <w:insideH w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\
         <w:insideV w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"/>\
         </w:tblBorders>\
         <w:tblLook w:val=\"04A0\"/>\
         </w:tblPr>\n",
    );

    // Grid columns
    self.body_xml.push_str("      <w:tblGrid>");
    for _ in &table.columns {
        self.body_xml.push_str("<w:gridCol/>");
    }
    self.body_xml.push_str("</w:tblGrid>\n");

    // Header row
    if let Some(header) = &table.header {
        self.write_table_row(header, &table.columns, true);
    }

    // Body rows
    for row in &table.rows {
        self.write_table_row(row, &table.columns, false);
    }

    // Footer row
    if let Some(foot) = &table.foot {
        self.write_table_row(foot, &table.columns, false);
    }

    self.body_xml.push_str("    </w:tbl>\n");
}

fn write_table_row(
    &mut self,
    cells: &[docmux_ast::TableCell],
    columns: &[docmux_ast::ColumnSpec],
    is_header: bool,
) {
    self.body_xml.push_str("      <w:tr>");
    if is_header {
        self.body_xml.push_str("<w:trPr><w:tblHeader/></w:trPr>");
    }
    self.body_xml.push('\n');

    for (i, cell) in cells.iter().enumerate() {
        self.body_xml.push_str("        <w:tc><w:tcPr>");

        if cell.colspan > 1 {
            write!(self.body_xml, "<w:gridSpan w:val=\"{}\"/>", cell.colspan).unwrap();
        }
        if cell.rowspan > 1 {
            self.body_xml.push_str("<w:vMerge w:val=\"restart\"/>");
        }

        // Alignment from column spec
        if let Some(col) = columns.get(i) {
            let jc = match col.alignment {
                docmux_ast::Alignment::Left => "left",
                docmux_ast::Alignment::Center => "center",
                docmux_ast::Alignment::Right => "right",
                docmux_ast::Alignment::Default => "left",
            };
            write!(self.body_xml, "<w:tcW w:w=\"0\" w:type=\"auto\"/>").unwrap();
            // We'll set paragraph alignment inside
            let _ = jc; // used below
        }

        self.body_xml.push_str("</w:tcPr>\n");

        // Cell content — write blocks, but if empty, add empty paragraph (required by OOXML)
        if cell.content.is_empty() {
            self.body_xml.push_str("          <w:p/>\n");
        } else {
            for block in &cell.content {
                self.write_block(block);
            }
        }

        self.body_xml.push_str("        </w:tc>\n");
    }

    self.body_xml.push_str("      </w:tr>\n");
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p docmux-writer-docx`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-docx/
git commit -m "feat(docx): table rendering with header, body, footer, colspan"
```

---

### Task 7: Lists with numbering.xml

**Files:**
- Modify: `crates/docmux-writer-docx/src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn unordered_list_renders_bullets() {
    use docmux_ast::ListItem;

    let doc = Document {
        content: vec![Block::List {
            ordered: false,
            start: None,
            items: vec![
                ListItem { checked: None, content: vec![Block::Paragraph { content: vec![Inline::Text { value: "First".into() }] }] },
                ListItem { checked: None, content: vec![Block::Paragraph { content: vec![Inline::Text { value: "Second".into() }] }] },
            ],
            tight: true,
            style: None,
            delimiter: None,
        }],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains("<w:numPr>"));
    assert!(xml.contains("<w:t>First</w:t>"));
    assert!(xml.contains("<w:t>Second</w:t>"));

    // Also verify numbering.xml exists
    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    assert!(archive.by_name("word/numbering.xml").is_ok());
}

#[test]
fn ordered_list_renders_numbers() {
    use docmux_ast::ListItem;

    let doc = Document {
        content: vec![Block::List {
            ordered: true,
            start: Some(1),
            items: vec![
                ListItem { checked: None, content: vec![Block::Paragraph { content: vec![Inline::Text { value: "Alpha".into() }] }] },
            ],
            tight: true,
            style: None,
            delimiter: None,
        }],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains("<w:numPr>"));
    assert!(xml.contains("<w:t>Alpha</w:t>"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-docx list`
Expected: FAIL

- [ ] **Step 3: Implement list rendering with numbering**

Add numbering tracking to `DocxBuilder`:

```rust
struct NumberingDef {
    abstract_num_id: u32,
    num_id: u32,
    is_ordered: bool,
    num_fmt: String,
}
```

Add field to `DocxBuilder`:

```rust
numbering_defs: Vec<NumberingDef>,
next_num_id: u32, // starts at 1
```

Initialize in `new()`:

```rust
numbering_defs: Vec::new(),
next_num_id: 1,
```

Add list methods:

```rust
fn get_or_create_numbering(&mut self, ordered: bool, style: Option<&docmux_ast::ListStyle>) -> u32 {
    let num_fmt = if ordered {
        match style {
            Some(docmux_ast::ListStyle::LowerAlpha) => "lowerLetter",
            Some(docmux_ast::ListStyle::UpperAlpha) => "upperLetter",
            Some(docmux_ast::ListStyle::LowerRoman) => "lowerRoman",
            Some(docmux_ast::ListStyle::UpperRoman) => "upperRoman",
            _ => "decimal",
        }
    } else {
        "bullet"
    };

    // Reuse existing def if same format
    for def in &self.numbering_defs {
        if def.is_ordered == ordered && def.num_fmt == num_fmt {
            return def.num_id;
        }
    }

    let id = self.next_num_id;
    self.next_num_id += 1;
    self.numbering_defs.push(NumberingDef {
        abstract_num_id: id,
        num_id: id,
        is_ordered: ordered,
        num_fmt: num_fmt.to_string(),
    });
    id
}

fn write_list(&mut self, list: &Block, depth: u32) {
    if let Block::List { ordered, items, style, .. } = list {
        let num_id = self.get_or_create_numbering(*ordered, style.as_ref());

        for item in items {
            for block in &item.content {
                match block {
                    Block::Paragraph { content } => {
                        self.body_xml.push_str("    <w:p><w:pPr>");
                        write!(
                            self.body_xml,
                            "<w:numPr><w:ilvl w:val=\"{depth}\"/><w:numId w:val=\"{num_id}\"/></w:numPr>"
                        )
                        .unwrap();
                        self.body_xml.push_str("</w:pPr>\n");
                        self.write_inlines(content, &[]);
                        self.body_xml.push_str("    </w:p>\n");
                    }
                    nested @ Block::List { .. } => {
                        self.write_list(nested, depth + 1);
                    }
                    other => self.write_block(other),
                }
            }
        }
    }
}

fn build_numbering_xml(&self) -> Option<String> {
    if self.numbering_defs.is_empty() {
        return None;
    }

    let mut xml = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
    );

    for def in &self.numbering_defs {
        write!(
            xml,
            r#"
  <w:abstractNum w:abstractNumId="{id}">
    <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/>{lvl_text}<w:pPr><w:ind w:left="720" w:hanging="360"/></w:pPr></w:lvl>
    <w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/>{lvl_text}<w:pPr><w:ind w:left="1440" w:hanging="360"/></w:pPr></w:lvl>
    <w:lvl w:ilvl="2"><w:start w:val="1"/><w:numFmt w:val="{fmt}"/>{lvl_text}<w:pPr><w:ind w:left="2160" w:hanging="360"/></w:pPr></w:lvl>
  </w:abstractNum>"#,
            id = def.abstract_num_id,
            fmt = def.num_fmt,
            lvl_text = if def.num_fmt == "bullet" {
                "<w:lvlText w:val=\"\u{2022}\"/>"
            } else {
                "<w:lvlText w:val=\"%1.\"/>"
            },
        )
        .unwrap();

        write!(
            xml,
            r#"
  <w:num w:numId="{}"><w:abstractNumId w:val="{}"/></w:num>"#,
            def.num_id, def.abstract_num_id
        )
        .unwrap();
    }

    xml.push_str("\n</w:numbering>");
    Some(xml)
}
```

Add `List` case to `write_block`:

```rust
list @ Block::List { .. } => {
    self.write_list(list, 0);
}
```

Update `write_bytes` to build numbering XML before assembling:

```rust
fn write_bytes(&self, doc: &Document, _opts: &WriteOptions) -> Result<Vec<u8>> {
    let mut builder = DocxBuilder::new();
    builder.write_metadata(&doc.metadata);
    builder.write_blocks(&doc.content);
    builder.numbering_xml = builder.build_numbering_xml();
    builder.assemble_zip()
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p docmux-writer-docx`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-docx/
git commit -m "feat(docx): ordered and unordered lists with numbering.xml"
```

---

### Task 8: Hyperlinks and footnotes

**Files:**
- Modify: `crates/docmux-writer-docx/src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn hyperlink_creates_relationship() {
    let doc = Document {
        content: vec![Block::Paragraph {
            content: vec![Inline::Link {
                url: "https://example.com".into(),
                title: None,
                content: vec![Inline::Text { value: "click".into() }],
                attrs: None,
            }],
        }],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains("<w:hyperlink"));
    assert!(xml.contains("<w:t>click</w:t>"));

    // Check relationship exists
    let rels = extract_zip_file(&bytes, "word/_rels/document.xml.rels");
    assert!(rels.contains("https://example.com"));
}

#[test]
fn footnotes_create_footnotes_xml() {
    let doc = Document {
        content: vec![
            Block::Paragraph {
                content: vec![
                    Inline::Text { value: "Text".into() },
                    Inline::FootnoteRef { id: "1".into() },
                ],
            },
            Block::FootnoteDef {
                id: "1".into(),
                content: vec![Block::Paragraph {
                    content: vec![Inline::Text { value: "A footnote".into() }],
                }],
            },
        ],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();

    let footnotes_xml = extract_zip_file(&bytes, "word/footnotes.xml");
    assert!(footnotes_xml.contains("<w:t>A footnote</w:t>"));

    let doc_xml = extract_document_xml(&bytes);
    assert!(doc_xml.contains("<w:footnoteReference"));
}

/// Helper: extract any file from DOCX bytes.
fn extract_zip_file(bytes: &[u8], name: &str) -> String {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut file = archive.by_name(name).unwrap();
    let mut contents = String::new();
    std::io::Read::read_to_string(&mut file, &mut contents).unwrap();
    contents
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-docx hyperlink footnotes`
Expected: FAIL

- [ ] **Step 3: Implement hyperlinks in `write_inline`**

Replace the `_ => {}` catch-all with:

```rust
Inline::Link { url, content, .. } => {
    let rel_id = self.add_relationship(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink",
        url,
    );
    // Mark as external relationship
    // (handled by updating build_document_rels to add TargetMode="External" for hyperlinks)
    write!(self.body_xml, "      <w:hyperlink r:id=\"{rel_id}\">").unwrap();
    // Apply hyperlink character style
    let mut props = run_props.to_vec();
    props.push(r#"<w:rStyle w:val="Hyperlink"/>"#);
    self.write_inlines(content, &props);
    self.body_xml.push_str("</w:hyperlink>\n");
}
Inline::FootnoteRef { id } => {
    if let Some(&footnote_id) = self.footnote_id_map.get(id.as_str()) {
        self.body_xml.push_str(
            "      <w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr>",
        );
        write!(self.body_xml, "<w:footnoteReference w:id=\"{footnote_id}\"/>").unwrap();
        self.body_xml.push_str("</w:r>\n");
    }
}
Inline::Image(_) => {
    // Handled in Task 9
}
```

- [ ] **Step 4: Implement footnote collection**

Add `footnote_id_map: HashMap<String, u32>` to `DocxBuilder` (maps source ID → DOCX footnote ID).

Add a first pass to collect footnotes before rendering blocks. In `write_bytes`:

```rust
fn write_bytes(&self, doc: &Document, _opts: &WriteOptions) -> Result<Vec<u8>> {
    let mut builder = DocxBuilder::new();

    // First pass: collect footnote definitions
    builder.collect_footnotes(&doc.content);

    builder.write_metadata(&doc.metadata);
    builder.write_blocks(&doc.content);
    builder.numbering_xml = builder.build_numbering_xml();
    builder.assemble_zip()
}
```

```rust
fn collect_footnotes(&mut self, blocks: &[Block]) {
    for block in blocks {
        if let Block::FootnoteDef { id, content } = block {
            let footnote_id = self.next_footnote_id;
            self.next_footnote_id += 1;
            self.footnote_id_map.insert(id.clone(), footnote_id);

            // Render footnote content to XML
            let mut footnote_body = String::new();
            std::mem::swap(&mut self.body_xml, &mut footnote_body);
            for child in content {
                self.write_block(child);
            }
            std::mem::swap(&mut self.body_xml, &mut footnote_body);

            self.footnotes.push((footnote_id, footnote_body));
        }
    }
}
```

Skip `FootnoteDef` in `write_block` (already rendered in first pass):

```rust
Block::FootnoteDef { .. } => {
    // Rendered during collect_footnotes pass
}
```

- [ ] **Step 5: Update `build_document_rels` to mark hyperlinks as external**

Update the relationship rendering to add `TargetMode="External"` for hyperlink relationships:

```rust
for rel in &self.relationships {
    let external = if rel.rel_type.contains("hyperlink") {
        " TargetMode=\"External\""
    } else {
        ""
    };
    write!(
        xml,
        r#"
  <Relationship Id="{}" Type="{}" Target="{}"{}/>"#,
        rel.id, rel.rel_type, xml_escape(&rel.target), external,
    )
    .unwrap();
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p docmux-writer-docx`
Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-writer-docx/
git commit -m "feat(docx): hyperlinks with relationships and footnotes"
```

---

### Task 9: Images

**Files:**
- Modify: `crates/docmux-writer-docx/src/lib.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn image_embeds_local_file() {
    use std::io::Write;

    // Create a tiny 1x1 PNG for testing
    let tmp_dir = std::env::temp_dir().join("docmux-docx-test");
    std::fs::create_dir_all(&tmp_dir).ok();
    let img_path = tmp_dir.join("test.png");

    // Minimal valid PNG (1x1 red pixel)
    let png_bytes: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, // RGB, etc.
        0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, // IDAT chunk
        0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00,
        0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC, 0x33,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, // IEND chunk
        0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(&img_path, png_bytes).unwrap();

    let doc = Document {
        content: vec![Block::Figure {
            image: docmux_ast::Image {
                url: img_path.to_str().unwrap().into(),
                alt: vec![Inline::Text { value: "A test image".into() }],
                title: None,
                attrs: None,
            },
            caption: Some(vec![Inline::Text { value: "Figure 1".into() }]),
            label: None,
            attrs: None,
        }],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();

    // Verify image is in ZIP
    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut found_image = false;
    for i in 0..archive.len() {
        let file = archive.by_index(i).unwrap();
        if file.name().starts_with("word/media/") {
            found_image = true;
        }
    }
    assert!(found_image, "Image should be embedded in word/media/");

    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("<w:drawing>") || xml.contains("<wp:inline"));
    assert!(xml.contains("<w:t>Figure 1</w:t>"));

    // Cleanup
    let _ = std::fs::remove_file(&img_path);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p docmux-writer-docx image_embeds`
Expected: FAIL

- [ ] **Step 3: Implement image embedding**

Add image handling to `DocxBuilder`:

```rust
fn embed_image(&mut self, url: &str) -> Option<(String, u32, u32)> {
    // Only handle local files
    let path = std::path::Path::new(url);
    if !path.exists() {
        return None;
    }

    let data = std::fs::read(path).ok()?;
    let ext = path.extension()?.to_str()?;

    let filename = format!("image{}.{}", self.next_image_id, ext);
    self.next_image_id += 1;

    let rel_id = self.add_relationship(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
        &format!("media/{filename}"),
    );

    self.media.push((filename, data));

    // Default size: 4 inches wide × 3 inches tall (in EMUs: 1 inch = 914400 EMU)
    let cx: u32 = 3657600; // 4 inches
    let cy: u32 = 2743200; // 3 inches

    Some((rel_id, cx, cy))
}
```

Add `Figure` case to `write_block`:

```rust
Block::Figure { image, caption, .. } => {
    if let Some((rel_id, cx, cy)) = self.embed_image(&image.url) {
        let alt_text = image.alt_text();
        let alt_escaped = xml_escape(&alt_text);
        self.body_xml.push_str("    <w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr>\n");
        write!(
            self.body_xml,
            r#"      <w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">
        <wp:extent cx="{cx}" cy="{cy}"/>
        <wp:docPr id="{img_id}" name="{alt_escaped}"/>
        <a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
            <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
              <pic:nvPicPr><pic:cNvPr id="{img_id}" name="{alt_escaped}"/><pic:cNvPicPr/></pic:nvPicPr>
              <pic:blipFill><a:blip r:embed="{rel_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>
              <pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr>
            </pic:pic>
          </a:graphicData>
        </a:graphic>
      </wp:inline></w:drawing></w:r>"#,
            img_id = self.next_image_id - 1,
        )
        .unwrap();
        self.body_xml.push_str("\n    </w:p>\n");
    } else {
        // Fallback: render as text link
        self.body_xml.push_str("    <w:p>\n");
        write!(
            self.body_xml,
            "      <w:r><w:t>[Image: {}]</w:t></w:r>\n",
            xml_escape(&image.url)
        )
        .unwrap();
        self.body_xml.push_str("    </w:p>\n");
    }

    // Caption
    if let Some(caption) = caption {
        self.body_xml.push_str(
            "    <w:p><w:pPr><w:pStyle w:val=\"Caption\"/></w:pPr>\n",
        );
        self.write_inlines(caption, &[]);
        self.body_xml.push_str("    </w:p>\n");
    }
}
```

Also handle inline `Image`:

```rust
Inline::Image(image) => {
    if let Some((rel_id, cx, cy)) = self.embed_image(&image.url) {
        let alt_text = image.alt_text();
        let alt_escaped = xml_escape(&alt_text);
        write!(
            self.body_xml,
            r#"      <w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0">
        <wp:extent cx="{cx}" cy="{cy}"/>
        <wp:docPr id="{img_id}" name="{alt_escaped}"/>
        <a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
            <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
              <pic:nvPicPr><pic:cNvPr id="{img_id}" name="{alt_escaped}"/><pic:cNvPicPr/></pic:nvPicPr>
              <pic:blipFill><a:blip r:embed="{rel_id}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>
              <pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr>
            </pic:pic>
          </a:graphicData>
        </a:graphic>
      </wp:inline></w:drawing></w:r>"#,
            img_id = self.next_image_id - 1,
        )
        .unwrap();
        self.body_xml.push('\n');
    } else {
        write!(
            self.body_xml,
            "      <w:r><w:t>[Image: {}]</w:t></w:r>\n",
            xml_escape(&image.url)
        )
        .unwrap();
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p docmux-writer-docx`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-docx/
git commit -m "feat(docx): image embedding from local files with drawing XML"
```

---

### Task 10: Admonitions, definition lists, remaining block types

**Files:**
- Modify: `crates/docmux-writer-docx/src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn admonition_renders_with_border() {
    let doc = Document {
        content: vec![Block::Admonition {
            kind: docmux_ast::AdmonitionKind::Note,
            title: Some(vec![Inline::Text { value: "Note".into() }]),
            content: vec![Block::Paragraph {
                content: vec![Inline::Text { value: "Important info".into() }],
            }],
        }],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains("<w:b/>"));
    assert!(xml.contains("<w:t>Note</w:t>"));
    assert!(xml.contains("<w:t>Important info</w:t>"));
    assert!(xml.contains("<w:pBdr>"));
}

#[test]
fn definition_list_renders() {
    let doc = Document {
        content: vec![Block::DefinitionList {
            items: vec![docmux_ast::DefinitionItem {
                term: vec![Inline::Text { value: "Term".into() }],
                definitions: vec![vec![Block::Paragraph {
                    content: vec![Inline::Text { value: "Definition".into() }],
                }]],
            }],
        }],
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    assert!(xml.contains("<w:b/>"));
    assert!(xml.contains("<w:t>Term</w:t>"));
    assert!(xml.contains("<w:t>Definition</w:t>"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-docx admonition definition`
Expected: FAIL

- [ ] **Step 3: Implement remaining block types**

Add to `write_block`:

```rust
Block::Admonition { kind, title, content } => {
    // Title paragraph with left border and bold
    let kind_label = match kind {
        docmux_ast::AdmonitionKind::Note => "Note",
        docmux_ast::AdmonitionKind::Warning => "Warning",
        docmux_ast::AdmonitionKind::Tip => "Tip",
        docmux_ast::AdmonitionKind::Important => "Important",
        docmux_ast::AdmonitionKind::Caution => "Caution",
        docmux_ast::AdmonitionKind::Custom(s) => s.as_str(),
    };

    let title_text = if let Some(t) = title {
        let mut s = String::new();
        // Collect plain text from inlines
        for inline in t {
            if let Inline::Text { value } = inline {
                s.push_str(value);
            }
        }
        s
    } else {
        kind_label.to_string()
    };

    self.body_xml.push_str(
        "    <w:p><w:pPr><w:pBdr>\
         <w:left w:val=\"single\" w:sz=\"12\" w:space=\"8\" w:color=\"4472C4\"/>\
         </w:pBdr></w:pPr>\n",
    );
    write!(
        self.body_xml,
        "      <w:r><w:rPr><w:b/></w:rPr><w:t>{}</w:t></w:r>\n",
        xml_escape(&title_text)
    )
    .unwrap();
    self.body_xml.push_str("    </w:p>\n");

    // Content paragraphs with same border
    for child in content {
        match child {
            Block::Paragraph { content: inlines } => {
                self.body_xml.push_str(
                    "    <w:p><w:pPr><w:pBdr>\
                     <w:left w:val=\"single\" w:sz=\"12\" w:space=\"8\" w:color=\"4472C4\"/>\
                     </w:pBdr></w:pPr>\n",
                );
                self.write_inlines(inlines, &[]);
                self.body_xml.push_str("    </w:p>\n");
            }
            other => self.write_block(other),
        }
    }
}
Block::DefinitionList { items } => {
    for item in items {
        // Term in bold
        self.body_xml.push_str("    <w:p>\n");
        self.write_inlines(&item.term, &["<w:b/>"]);
        self.body_xml.push_str("    </w:p>\n");

        // Definitions indented
        for def_blocks in &item.definitions {
            for block in def_blocks {
                match block {
                    Block::Paragraph { content } => {
                        self.body_xml.push_str(
                            "    <w:p><w:pPr><w:ind w:left=\"720\"/></w:pPr>\n",
                        );
                        self.write_inlines(content, &[]);
                        self.body_xml.push_str("    </w:p>\n");
                    }
                    other => self.write_block(other),
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p docmux-writer-docx`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-docx/
git commit -m "feat(docx): admonitions with colored borders and definition lists"
```

---

### Task 11: CLI integration — binary output path

**Files:**
- Modify: `crates/docmux-cli/src/main.rs` (lines 129-141, 354-365)
- Modify: `crates/docmux-cli/Cargo.toml` (add docx dependency)

- [ ] **Step 1: Add docx writer dependency to CLI**

In `crates/docmux-cli/Cargo.toml`, add:

```toml
docmux-writer-docx = { workspace = true }
```

- [ ] **Step 2: Register DocxWriter in `build_registry()`**

At line ~140 in `main.rs`, add:

```rust
use docmux_writer_docx::DocxWriter;
// ...in build_registry():
reg.add_writer(Box::new(DocxWriter::new()));
```

- [ ] **Step 3: Add binary output path**

The CLI currently calls `writer.write()` at line 355 and passes the result to `write_output` as `&str`. Add a check for binary formats before that. Replace lines 354-364 with:

```rust
// Binary formats (e.g. DOCX) use write_bytes
if writer.default_extension() == "docx" {
    let bytes = match writer.write_bytes(&doc, &opts) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("docmux: write error: {e}");
            std::process::exit(1);
        }
    };
    match &cli.output {
        Some(path) if path.to_str() != Some("-") => {
            if let Err(e) = std::fs::write(path, &bytes) {
                eprintln!("docmux: error writing {}: {e}", path.display());
                std::process::exit(1);
            }
            if !cli.quiet {
                let first_input = cli
                    .input
                    .first()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".into());
                eprintln!(
                    "docmux: {} -> {} ({} bytes)",
                    first_input,
                    path.display(),
                    bytes.len()
                );
            }
        }
        _ => {
            eprintln!("docmux: DOCX output requires -o FILE (binary format cannot be written to stdout)");
            std::process::exit(1);
        }
    }
    return;
}

let output = match writer.write(&doc, &opts) {
    // ... existing code continues
```

- [ ] **Step 4: Write CLI smoke test**

Add to `crates/docmux-cli/tests/cli_smoke.rs`:

```rust
#[test]
fn converts_markdown_to_docx_file() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let tmp_dir = std::env::temp_dir().join("docmux-test");
    std::fs::create_dir_all(&tmp_dir).ok();
    let output_file = tmp_dir.join("paragraph.docx");
    let _ = std::fs::remove_file(&output_file);

    let output = Command::new(docmux_bin())
        .args([
            input.to_str().unwrap(),
            "-o",
            output_file.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run docmux");

    assert!(
        output.status.success(),
        "docmux exited with error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_file.exists(), "output file should exist");

    // Verify it's a valid ZIP
    let bytes = std::fs::read(&output_file).unwrap();
    let cursor = std::io::Cursor::new(&bytes);
    let archive = zip::ZipArchive::new(cursor);
    assert!(archive.is_ok(), "output should be a valid ZIP file");

    let _ = std::fs::remove_file(&output_file);
}

#[test]
fn docx_to_stdout_errors() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let output = Command::new(docmux_bin())
        .args([input.to_str().unwrap(), "-t", "docx"])
        .output()
        .expect("failed to run docmux");

    assert!(
        !output.status.success(),
        "docx to stdout should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("requires -o FILE") || stderr.contains("binary format"));
}
```

- [ ] **Step 5: Add `zip` to CLI dev-dependencies** (for the test)

In `crates/docmux-cli/Cargo.toml`:

```toml
[dev-dependencies]
zip = { workspace = true }
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p docmux-cli`
Expected: All tests pass (existing + 2 new)

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-cli/ crates/docmux-writer-docx/
git commit -m "feat(cli): register DOCX writer with binary output path"
```

---

### Task 12: WASM integration

**Files:**
- Modify: `crates/docmux-wasm/Cargo.toml`
- Modify: `crates/docmux-wasm/src/lib.rs`

- [ ] **Step 1: Add docx dependency to WASM crate**

In `crates/docmux-wasm/Cargo.toml`, add:

```toml
docmux-writer-docx = { workspace = true }
```

- [ ] **Step 2: Register DocxWriter in WASM `build_registry()`**

In `crates/docmux-wasm/src/lib.rs`, add:

```rust
use docmux_writer_docx::DocxWriter;
// ...in build_registry():
reg.add_writer(Box::new(DocxWriter::new()));
```

Note: The existing `convert()` function calls `writer.write()`, which returns an error for DOCX ("binary format"). For WASM, DOCX conversion would need a separate `convert_bytes()` export — that's a future enhancement. For now, registering makes `docx` appear in format listings.

- [ ] **Step 3: Verify WASM builds**

Run: `cargo build --target wasm32-unknown-unknown -p docmux-wasm`
Expected: Build succeeds. Note: the `zip` crate must support `wasm32-unknown-unknown` (it does, since it uses `std::io::Write` which is available).

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-wasm/
git commit -m "feat(wasm): register DOCX writer in WASM crate"
```

---

### Task 13: Integration test — full document roundtrip

**Files:**
- Create: `crates/docmux-writer-docx/tests/integration.rs`

- [ ] **Step 1: Write the integration test**

```rust
//! Integration tests: parse Markdown → write DOCX → verify ZIP structure and XML content.

use docmux_core::WriteOptions;
use docmux_reader_markdown::MarkdownReader;
use docmux_core::Reader;
use docmux_writer_docx::DocxWriter;
use docmux_core::Writer;

fn write_docx(markdown: &str) -> Vec<u8> {
    let reader = MarkdownReader::new();
    let doc = reader.read(markdown).expect("parse failed");
    let writer = DocxWriter::new();
    writer.write_bytes(&doc, &WriteOptions::default()).expect("write failed")
}

fn extract_xml(bytes: &[u8], name: &str) -> String {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut file = archive.by_name(name).unwrap();
    let mut contents = String::new();
    std::io::Read::read_to_string(&mut file, &mut contents).unwrap();
    contents
}

#[test]
fn full_document_structure() {
    let md = r#"---
title: Test Document
author: Alice
date: 2026-01-01
---

# Introduction

This is a paragraph with **bold** and *italic* text.

## Lists

- Item one
- Item two

1. First
2. Second

## Code

```rust
fn main() {
    println!("hello");
}
```

## Table

| Name  | Score |
|-------|-------|
| Alice | 95    |
| Bob   | 87    |

---

> A blockquote with *emphasis*.

Inline math: $E = mc^2$ and a footnote[^1].

[^1]: This is a footnote.
"#;

    let bytes = write_docx(md);

    // Verify ZIP structure
    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut names = Vec::new();
    for i in 0..archive.len() {
        names.push(archive.by_index(i).unwrap().name().to_string());
    }

    assert!(names.contains(&"[Content_Types].xml".to_string()));
    assert!(names.contains(&"word/document.xml".to_string()));
    assert!(names.contains(&"word/styles.xml".to_string()));
    assert!(names.contains(&"word/numbering.xml".to_string()));
    assert!(names.contains(&"word/footnotes.xml".to_string()));

    // Verify document.xml content
    let doc_xml = extract_xml(&bytes, "word/document.xml");

    // Metadata
    assert!(doc_xml.contains("<w:t>Test Document</w:t>"));
    assert!(doc_xml.contains("<w:t>Alice</w:t>"));

    // Headings
    assert!(doc_xml.contains(r#"<w:pStyle w:val="Heading1"/>"#));
    assert!(doc_xml.contains(r#"<w:pStyle w:val="Heading2"/>"#));
    assert!(doc_xml.contains("<w:t>Introduction</w:t>"));

    // Inline formatting
    assert!(doc_xml.contains("<w:b/>"));
    assert!(doc_xml.contains("<w:i/>"));

    // Lists
    assert!(doc_xml.contains("<w:numPr>"));

    // Code block
    assert!(doc_xml.contains(r#"<w:pStyle w:val="CodeBlock"/>"#));

    // Table
    assert!(doc_xml.contains("<w:tbl>"));
    assert!(doc_xml.contains("<w:t>Alice</w:t>"));

    // Blockquote
    assert!(doc_xml.contains(r#"<w:pStyle w:val="BlockQuote"/>"#));

    // Thematic break
    assert!(doc_xml.contains("<w:pBdr>"));

    // Footnote reference
    assert!(doc_xml.contains("<w:footnoteReference"));

    // Footnotes file
    let fn_xml = extract_xml(&bytes, "word/footnotes.xml");
    assert!(fn_xml.contains("This is a footnote"));
}
```

- [ ] **Step 2: Add dev-dependencies to docx writer Cargo.toml**

```toml
[dev-dependencies]
zip = { workspace = true }
docmux-reader-markdown = { workspace = true }
```

- [ ] **Step 3: Run the integration test**

Run: `cargo test -p docmux-writer-docx --test integration`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-writer-docx/
git commit -m "test(docx): add full document roundtrip integration test"
```

---

### Task 14: Run full workspace tests and clippy

**Files:** None (verification only)

- [ ] **Step 1: Run cargo clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: No warnings

- [ ] **Step 2: Run cargo fmt check**

Run: `cargo fmt --all -- --check`
Expected: No formatting issues

- [ ] **Step 3: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests pass (existing + ~15 new DOCX tests)

- [ ] **Step 4: Build WASM target**

Run: `cargo build --target wasm32-unknown-unknown -p docmux-wasm`
Expected: Build succeeds

- [ ] **Step 5: Fix any issues found, commit if needed**

If any issues found in steps 1-4, fix them and commit:

```bash
git add -A
git commit -m "fix(docx): address clippy warnings and test issues"
```

---

### Task 15: Update ROADMAP.md

**Files:**
- Modify: `ROADMAP.md`

- [ ] **Step 1: Mark DOCX writer as complete**

Change line 101 from:
```markdown
- [ ] DOCX writer — OOXML output via zip + XML generation
```
To:
```markdown
- [x] DOCX writer — OOXML output via zip + XML generation (unit + integration tests)
```

- [ ] **Step 2: Commit**

```bash
git add ROADMAP.md
git commit -m "docs: mark DOCX writer as complete in roadmap"
```
