//! # docmux-writer-docx
//!
//! DOCX (Office Open XML) writer for docmux. Produces `.docx` files as byte
//! vectors — the text-based [`Writer::write`] method returns an error because
//! DOCX is a binary (ZIP) format.

use docmux_ast::{Block, Document, Inline, QuoteType};
use docmux_core::{ConvertError, Result, WriteOptions, Writer};
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

// ─── Static assets ──────────────────────────────────────────────────────────

static STYLES_XML: &str = include_str!("styles.xml");

// ─── XML helpers ────────────────────────────────────────────────────────────

/// Escape XML special characters in text content.
fn xml_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
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

// ─── Relationship ───────────────────────────────────────────────────────────

/// An OOXML relationship entry.
struct Relationship {
    id: String,
    rel_type: String,
    target: String,
}

// ─── DocxBuilder ────────────────────────────────────────────────────────────

/// Builds the OOXML parts and assembles them into a ZIP archive.
#[allow(dead_code)]
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
            next_footnote_id: 2,
            next_image_id: 1,
        }
    }

    /// Register a relationship and return its rId.
    #[allow(dead_code)]
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

    // ── Block / inline rendering ──────────────────────────────────────

    fn write_blocks(&mut self, blocks: &[Block]) {
        for block in blocks {
            self.write_block(block);
        }
    }

    fn write_block(&mut self, block: &Block) {
        match block {
            Block::Paragraph { content } => {
                self.body_xml.push_str("<w:p>");
                self.write_inlines(content, &[]);
                self.body_xml.push_str("</w:p>\n");
            }
            Block::Heading { level, content, .. } => {
                let style = format!("Heading{}", level.min(&6));
                self.body_xml.push_str("<w:p><w:pPr><w:pStyle w:val=\"");
                self.body_xml.push_str(&style);
                self.body_xml.push_str("\"/></w:pPr>");
                self.write_inlines(content, &[]);
                self.body_xml.push_str("</w:p>\n");
            }
            _ => {}
        }
    }

    /// Render metadata (title, authors, date, abstract) as styled paragraphs
    /// before the main body content.
    fn write_metadata(&mut self, metadata: &docmux_ast::Metadata) {
        if let Some(ref title) = metadata.title {
            self.write_styled_paragraph("Title", title);
        }
        for author in &metadata.authors {
            self.write_styled_paragraph("Author", &author.name);
        }
        if let Some(ref date) = metadata.date {
            self.write_styled_paragraph("Date", date);
        }
        if let Some(ref abstract_blocks) = metadata.abstract_text {
            // Render abstract blocks, overriding their paragraph style to "Abstract"
            for block in abstract_blocks {
                match block {
                    Block::Paragraph { content } => {
                        self.body_xml
                            .push_str("<w:p><w:pPr><w:pStyle w:val=\"Abstract\"/></w:pPr>");
                        self.write_inlines(content, &[]);
                        self.body_xml.push_str("</w:p>\n");
                    }
                    other => self.write_block(other),
                }
            }
        }
    }

    /// Write a single paragraph with a named style and plain text content.
    fn write_styled_paragraph(&mut self, style: &str, text: &str) {
        self.body_xml.push_str("<w:p><w:pPr><w:pStyle w:val=\"");
        self.body_xml.push_str(style);
        self.body_xml.push_str("\"/></w:pPr>");
        self.body_xml.push_str("<w:r><w:t>");
        self.body_xml.push_str(&xml_escape(text));
        self.body_xml.push_str("</w:t></w:r></w:p>\n");
    }

    fn write_inlines(&mut self, inlines: &[Inline], run_props: &[&str]) {
        for inline in inlines {
            self.write_inline(inline, run_props);
        }
    }

    fn write_inline(&mut self, inline: &Inline, run_props: &[&str]) {
        match inline {
            Inline::Text { value } => {
                self.body_xml.push_str("<w:r>");
                self.write_run_props(run_props);
                let needs_preserve = value.starts_with(' ') || value.ends_with(' ');
                if needs_preserve {
                    self.body_xml.push_str("<w:t xml:space=\"preserve\">");
                } else {
                    self.body_xml.push_str("<w:t>");
                }
                self.body_xml.push_str(&xml_escape(value));
                self.body_xml.push_str("</w:t></w:r>");
            }
            Inline::Strong { content } => {
                let mut props: Vec<&str> = run_props.to_vec();
                props.push("<w:b/>");
                self.write_inlines(content, &props);
            }
            Inline::Emphasis { content } => {
                let mut props: Vec<&str> = run_props.to_vec();
                props.push("<w:i/>");
                self.write_inlines(content, &props);
            }
            Inline::Strikethrough { content } => {
                let mut props: Vec<&str> = run_props.to_vec();
                props.push("<w:strike/>");
                self.write_inlines(content, &props);
            }
            Inline::Underline { content } => {
                let mut props: Vec<&str> = run_props.to_vec();
                props.push("<w:u w:val=\"single\"/>");
                self.write_inlines(content, &props);
            }
            Inline::Superscript { content } => {
                let mut props: Vec<&str> = run_props.to_vec();
                props.push("<w:vertAlign w:val=\"superscript\"/>");
                self.write_inlines(content, &props);
            }
            Inline::Subscript { content } => {
                let mut props: Vec<&str> = run_props.to_vec();
                props.push("<w:vertAlign w:val=\"subscript\"/>");
                self.write_inlines(content, &props);
            }
            Inline::SmallCaps { content } => {
                let mut props: Vec<&str> = run_props.to_vec();
                props.push("<w:smallCaps/>");
                self.write_inlines(content, &props);
            }
            Inline::Code { value, .. } => {
                self.body_xml.push_str("<w:r><w:rPr>");
                for prop in run_props {
                    self.body_xml.push_str(prop);
                }
                self.body_xml
                    .push_str("<w:rFonts w:ascii=\"Courier New\" w:hAnsi=\"Courier New\"/>");
                self.body_xml.push_str("<w:sz w:val=\"20\"/>");
                self.body_xml.push_str("</w:rPr><w:t>");
                self.body_xml.push_str(&xml_escape(value));
                self.body_xml.push_str("</w:t></w:r>");
            }
            Inline::MathInline { value } => {
                self.body_xml.push_str("<w:r>");
                self.write_run_props(run_props);
                self.body_xml.push_str("<w:t>");
                self.body_xml.push_str(&xml_escape(value));
                self.body_xml.push_str("</w:t></w:r>");
            }
            Inline::SoftBreak => {
                self.body_xml
                    .push_str("<w:r><w:t xml:space=\"preserve\"> </w:t></w:r>");
            }
            Inline::HardBreak => {
                self.body_xml.push_str("<w:r><w:br/></w:r>");
            }
            Inline::Quoted {
                quote_type,
                content,
            } => {
                let (open, close) = match quote_type {
                    QuoteType::SingleQuote => ("\u{2018}", "\u{2019}"),
                    QuoteType::DoubleQuote => ("\u{201C}", "\u{201D}"),
                };
                // Opening quote
                self.body_xml.push_str("<w:r>");
                self.write_run_props(run_props);
                self.body_xml.push_str("<w:t>");
                self.body_xml.push_str(open);
                self.body_xml.push_str("</w:t></w:r>");
                // Content
                self.write_inlines(content, run_props);
                // Closing quote
                self.body_xml.push_str("<w:r>");
                self.write_run_props(run_props);
                self.body_xml.push_str("<w:t>");
                self.body_xml.push_str(close);
                self.body_xml.push_str("</w:t></w:r>");
            }
            Inline::Span { content, .. } => {
                self.write_inlines(content, run_props);
            }
            Inline::RawInline { format, content } => {
                if format == "docx" || format == "openxml" {
                    self.body_xml.push_str(content);
                }
                // Skip other formats
            }
            Inline::Citation(citation) => {
                let keys: Vec<&str> = citation.items.iter().map(|i| i.key.as_str()).collect();
                let text = format!("[{}]", keys.join("; "));
                self.body_xml.push_str("<w:r>");
                self.write_run_props(run_props);
                self.body_xml.push_str("<w:t>");
                self.body_xml.push_str(&xml_escape(&text));
                self.body_xml.push_str("</w:t></w:r>");
            }
            Inline::CrossRef(cross_ref) => {
                self.body_xml.push_str("<w:r>");
                self.write_run_props(run_props);
                self.body_xml.push_str("<w:t>");
                self.body_xml.push_str(&xml_escape(&cross_ref.target));
                self.body_xml.push_str("</w:t></w:r>");
            }
            // Skip for now (handled in later tasks)
            Inline::FootnoteRef { .. } | Inline::Link { .. } | Inline::Image(_) => {}
        }
    }

    /// Write `<w:rPr>…</w:rPr>` if there are any run properties.
    fn write_run_props(&mut self, run_props: &[&str]) {
        if !run_props.is_empty() {
            self.body_xml.push_str("<w:rPr>");
            for prop in run_props {
                self.body_xml.push_str(prop);
            }
            self.body_xml.push_str("</w:rPr>");
        }
    }

    // ── Part builders ───────────────────────────────────────────────────

    fn build_content_types(&self) -> String {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\n\
             <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\n\
             <Default Extension=\"xml\" ContentType=\"application/xml\"/>\n\
             <Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\n\
             <Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\n",
        );

        if !self.footnotes.is_empty() {
            xml.push_str("<Override PartName=\"/word/footnotes.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml\"/>\n");
        }

        if self.numbering_xml.is_some() {
            xml.push_str("<Override PartName=\"/word/numbering.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml\"/>\n");
        }

        for (name, _) in &self.media {
            if name.ends_with(".png") {
                xml.push_str("<Default Extension=\"png\" ContentType=\"image/png\"/>\n");
                break;
            }
        }
        for (name, _) in &self.media {
            if name.ends_with(".jpeg") || name.ends_with(".jpg") {
                xml.push_str("<Default Extension=\"jpeg\" ContentType=\"image/jpeg\"/>\n");
                break;
            }
        }

        xml.push_str("</Types>");
        xml
    }

    fn build_root_rels(&self) -> String {
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n\
         <Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\n\
         </Relationships>"
            .to_string()
    }

    fn build_document_xml(&self) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <w:document\n  \
               xmlns:wpc=\"http://schemas.microsoft.com/office/word/2010/wordprocessingCanvas\"\n  \
               xmlns:mo=\"http://schemas.microsoft.com/office/mac/office/2008/main\"\n  \
               xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\"\n  \
               xmlns:mv=\"urn:schemas-microsoft-com:mac:vml\"\n  \
               xmlns:o=\"urn:schemas-microsoft-com:office:office\"\n  \
               xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\"\n  \
               xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"\n  \
               xmlns:v=\"urn:schemas-microsoft-com:vml\"\n  \
               xmlns:wp14=\"http://schemas.microsoft.com/office/word/2010/wordprocessingDrawing\"\n  \
               xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\"\n  \
               xmlns:w10=\"urn:schemas-microsoft-com:office:word\"\n  \
               xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"\n  \
               xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\"\n  \
               xmlns:wpg=\"http://schemas.microsoft.com/office/word/2010/wordprocessingGroup\"\n  \
               xmlns:wpi=\"http://schemas.microsoft.com/office/word/2010/wordprocessingInk\"\n  \
               xmlns:wne=\"http://schemas.microsoft.com/office/word/2006/wordml\"\n  \
               xmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\"\n  \
               xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">\n\
             <w:body>\n\
             {body}\
             <w:sectPr>\n\
               <w:pgSz w:w=\"12240\" w:h=\"15840\"/>\n\
               <w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/>\n\
             </w:sectPr>\n\
             </w:body>\n\
             </w:document>",
            body = self.body_xml,
        )
    }

    fn build_document_rels(&self) -> String {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\n\
             <Relationship Id=\"rIdStyles\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\n",
        );

        if !self.footnotes.is_empty() {
            xml.push_str("<Relationship Id=\"rIdFootnotes\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes\" Target=\"footnotes.xml\"/>\n");
        }

        if self.numbering_xml.is_some() {
            xml.push_str("<Relationship Id=\"rIdNumbering\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering\" Target=\"numbering.xml\"/>\n");
        }

        for rel in &self.relationships {
            xml.push_str(&format!(
                "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"/>\n",
                xml_escape(&rel.id),
                xml_escape(&rel.rel_type),
                xml_escape(&rel.target),
            ));
        }

        xml.push_str("</Relationships>");
        xml
    }

    fn build_styles_xml(&self) -> &'static str {
        STYLES_XML
    }

    fn build_footnotes_xml(&self) -> String {
        let mut xml = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <w:footnotes xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"\n  \
               xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\">\n",
        );

        // Separator and continuation-separator (required by Word)
        xml.push_str(
            "<w:footnote w:type=\"separator\" w:id=\"-1\">\n\
               <w:p><w:r><w:separator/></w:r></w:p>\n\
             </w:footnote>\n\
             <w:footnote w:type=\"continuationSeparator\" w:id=\"0\">\n\
               <w:p><w:r><w:continuationSeparator/></w:r></w:p>\n\
             </w:footnote>\n",
        );

        for (id, content) in &self.footnotes {
            xml.push_str(&format!(
                "<w:footnote w:id=\"{id}\">\n\
                   <w:p>\n\
                     <w:pPr><w:pStyle w:val=\"FootnoteText\"/></w:pPr>\n\
                     <w:r>\n\
                       <w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr>\n\
                       <w:footnoteRef/>\n\
                     </w:r>\n\
                     <w:r><w:t xml:space=\"preserve\"> </w:t></w:r>\n\
                     {content}\n\
                   </w:p>\n\
                 </w:footnote>\n"
            ));
        }

        xml.push_str("</w:footnotes>");
        xml
    }

    // ── ZIP assembly ────────────────────────────────────────────────────

    fn assemble_zip(self) -> Result<Vec<u8>> {
        let buf = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        // [Content_Types].xml
        zip.start_file("[Content_Types].xml", opts)
            .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
        zip.write_all(self.build_content_types().as_bytes())
            .map_err(ConvertError::Io)?;

        // _rels/.rels
        zip.start_file("_rels/.rels", opts)
            .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
        zip.write_all(self.build_root_rels().as_bytes())
            .map_err(ConvertError::Io)?;

        // word/document.xml
        zip.start_file("word/document.xml", opts)
            .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
        zip.write_all(self.build_document_xml().as_bytes())
            .map_err(ConvertError::Io)?;

        // word/_rels/document.xml.rels
        zip.start_file("word/_rels/document.xml.rels", opts)
            .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
        zip.write_all(self.build_document_rels().as_bytes())
            .map_err(ConvertError::Io)?;

        // word/styles.xml
        zip.start_file("word/styles.xml", opts)
            .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
        zip.write_all(self.build_styles_xml().as_bytes())
            .map_err(ConvertError::Io)?;

        // word/footnotes.xml (only if there are footnotes)
        if !self.footnotes.is_empty() {
            zip.start_file("word/footnotes.xml", opts)
                .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
            zip.write_all(self.build_footnotes_xml().as_bytes())
                .map_err(ConvertError::Io)?;
        }

        // word/numbering.xml (only if lists are present)
        if let Some(ref numbering) = self.numbering_xml {
            zip.start_file("word/numbering.xml", opts)
                .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
            zip.write_all(numbering.as_bytes())
                .map_err(ConvertError::Io)?;
        }

        // Media files
        for (name, data) in &self.media {
            zip.start_file(format!("word/media/{name}"), opts)
                .map_err(|e| ConvertError::Other(format!("zip error: {e}")))?;
            zip.write_all(data).map_err(ConvertError::Io)?;
        }

        let cursor = zip
            .finish()
            .map_err(|e| ConvertError::Other(format!("zip finish error: {e}")))?;
        Ok(cursor.into_inner())
    }
}

// ─── DocxWriter ─────────────────────────────────────────────────────────────

/// A DOCX writer that produces Office Open XML `.docx` files.
#[derive(Debug, Default)]
pub struct DocxWriter;

impl DocxWriter {
    pub fn new() -> Self {
        Self
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
            "DOCX is a binary format \u{2014} use write_bytes() instead".into(),
        ))
    }

    fn write_bytes(&self, doc: &Document, _opts: &WriteOptions) -> Result<Vec<u8>> {
        let mut builder = DocxBuilder::new();
        builder.write_metadata(&doc.metadata);
        builder.write_blocks(&doc.content);
        builder.assemble_zip()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read as _};

    /// Extract word/document.xml from a DOCX byte buffer.
    fn extract_document_xml(bytes: &[u8]) -> String {
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut file = archive.by_name("word/document.xml").unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        contents
    }

    #[test]
    fn paragraph_with_inlines() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::Text {
                        value: "Hello ".into(),
                    },
                    Inline::Strong {
                        content: vec![Inline::Text {
                            value: "bold".into(),
                        }],
                    },
                    Inline::Text {
                        value: " and ".into(),
                    },
                    Inline::Emphasis {
                        content: vec![Inline::Text {
                            value: "italic".into(),
                        }],
                    },
                ],
            }],
            ..Default::default()
        };

        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
        let xml = extract_document_xml(&bytes);

        assert!(
            xml.contains("<w:t xml:space=\"preserve\">Hello </w:t>"),
            "missing preserved-space text run, got:\n{xml}"
        );
        assert!(xml.contains("<w:b/>"), "missing bold run prop, got:\n{xml}");
        assert!(
            xml.contains("<w:t>bold</w:t>"),
            "missing bold text, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:i/>"),
            "missing italic run prop, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>italic</w:t>"),
            "missing italic text, got:\n{xml}"
        );
    }

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
        let err = w.write(&doc, &opts).unwrap_err();
        assert!(
            matches!(err, ConvertError::Unsupported(_)),
            "expected Unsupported, got {err:?}"
        );
    }

    #[test]
    fn empty_doc_produces_valid_zip() {
        let w = DocxWriter::new();
        let doc = Document::default();
        let opts = WriteOptions::default();
        let bytes = w.write_bytes(&doc, &opts).unwrap();

        // Verify it's a valid ZIP
        let reader = Cursor::new(&bytes);
        let mut archive = zip::ZipArchive::new(reader).unwrap();

        // Collect file names
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();

        // Required OOXML parts
        assert!(
            names.contains(&"[Content_Types].xml".to_string()),
            "missing [Content_Types].xml, got: {names:?}"
        );
        assert!(
            names.contains(&"_rels/.rels".to_string()),
            "missing _rels/.rels, got: {names:?}"
        );
        assert!(
            names.contains(&"word/document.xml".to_string()),
            "missing word/document.xml, got: {names:?}"
        );
        assert!(
            names.contains(&"word/styles.xml".to_string()),
            "missing word/styles.xml, got: {names:?}"
        );

        // Verify document.xml has expected content
        let mut doc_xml = String::new();
        archive
            .by_name("word/document.xml")
            .unwrap()
            .read_to_string(&mut doc_xml)
            .unwrap();
        assert!(
            doc_xml.contains("w:document"),
            "document.xml missing w:document element"
        );
        assert!(
            doc_xml.contains("w:body"),
            "document.xml missing w:body element"
        );
        assert!(
            doc_xml.contains("w:sectPr"),
            "document.xml missing w:sectPr element"
        );
        // US Letter size: 12240x15840 twips
        assert!(
            doc_xml.contains("w:w=\"12240\""),
            "document.xml missing US Letter width"
        );
        assert!(
            doc_xml.contains("w:h=\"15840\""),
            "document.xml missing US Letter height"
        );
        // 1-inch margins: 1440 twips
        assert!(
            doc_xml.contains("w:top=\"1440\""),
            "document.xml missing 1-inch top margin"
        );

        // Verify styles.xml has expected styles
        let mut styles_xml = String::new();
        archive
            .by_name("word/styles.xml")
            .unwrap()
            .read_to_string(&mut styles_xml)
            .unwrap();
        assert!(
            styles_xml.contains("w:styleId=\"Normal\""),
            "styles.xml missing Normal style"
        );
        assert!(
            styles_xml.contains("w:styleId=\"Heading1\""),
            "styles.xml missing Heading1 style"
        );
        assert!(
            styles_xml.contains("w:styleId=\"CodeBlock\""),
            "styles.xml missing CodeBlock style"
        );
        assert!(
            styles_xml.contains("w:styleId=\"Hyperlink\""),
            "styles.xml missing Hyperlink style"
        );
    }

    #[test]
    fn headings_use_heading_styles() {
        use docmux_ast::{Block, Inline};
        let doc = Document {
            content: vec![
                Block::Heading {
                    level: 1,
                    id: Some("intro".into()),
                    content: vec![Inline::Text {
                        value: "Introduction".into(),
                    }],
                    attrs: None,
                },
                Block::Heading {
                    level: 2,
                    id: None,
                    content: vec![Inline::Text {
                        value: "Sub".into(),
                    }],
                    attrs: None,
                },
            ],
            ..Default::default()
        };
        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
        let xml = extract_document_xml(&bytes);
        assert!(
            xml.contains(r#"<w:pStyle w:val="Heading1"/>"#),
            "missing Heading1 style, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>Introduction</w:t>"),
            "missing Introduction text, got:\n{xml}"
        );
        assert!(
            xml.contains(r#"<w:pStyle w:val="Heading2"/>"#),
            "missing Heading2 style, got:\n{xml}"
        );
    }

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
        assert!(
            xml.contains(r#"<w:pStyle w:val="Title"/>"#),
            "missing Title style, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>My Paper</w:t>"),
            "missing title text, got:\n{xml}"
        );
        assert!(
            xml.contains(r#"<w:pStyle w:val="Author"/>"#),
            "missing Author style, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>Jane Doe</w:t>"),
            "missing author text, got:\n{xml}"
        );
        assert!(
            xml.contains(r#"<w:pStyle w:val="Date"/>"#),
            "missing Date style, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>2026-01-01</w:t>"),
            "missing date text, got:\n{xml}"
        );
    }

    #[test]
    fn xml_escape_special_chars() {
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(xml_escape("plain"), "plain");
    }
}
