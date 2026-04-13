//! # docmux-writer-docx
//!
//! DOCX (Office Open XML) writer for docmux. Produces `.docx` files as byte
//! vectors — the text-based [`Writer::write`] method returns an error because
//! DOCX is a binary (ZIP) format.

use docmux_ast::{Alignment, Block, Document, Inline, QuoteType};
use docmux_core::{ConvertError, Result, WriteOptions, Writer};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::CompressionMethod;
use zip::ZipWriter;

// ─── Static assets ──────────────────────────────────────────────────────────

static STYLES_XML: &str = include_str!("styles.xml");

/// Paragraph prefix for admonition boxes (blue left border).
const ADMONITION_PARA_PREFIX: &str = "<w:p><w:pPr><w:pBdr>\
    <w:left w:val=\"single\" w:sz=\"12\" w:space=\"8\" w:color=\"4472C4\"/>\
    </w:pBdr></w:pPr>";

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

// ─── Image helpers ──────────────────────────────────────────────────────────

/// Parse image dimensions (width, height) in pixels from PNG or JPEG headers.
fn image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    // PNG: magic bytes + IHDR chunk at offset 16 (width) and 20 (height)
    if data.len() >= 24 && data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        return Some((w, h));
    }
    // JPEG: scan for SOF0 (FF C0) or SOF2 (FF C2) marker
    if data.len() >= 4 && data[0..2] == [0xFF, 0xD8] {
        let mut i = 2;
        while i + 8 < data.len() {
            if data[i] == 0xFF && (data[i + 1] == 0xC0 || data[i + 1] == 0xC2) {
                // SOF layout: FF Cx | length(2) | precision(1) | height(2) | width(2)
                let h = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                let w = u16::from_be_bytes([data[i + 7], data[i + 8]]) as u32;
                return Some((w, h));
            }
            if data[i] == 0xFF && i + 3 < data.len() {
                let seg_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
                i += 2 + seg_len;
            } else {
                i += 1;
            }
        }
    }
    None
}

/// Detect MIME type from magic bytes.
fn detect_mime(data: &[u8]) -> Option<&'static str> {
    if data.len() >= 4 && data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        Some("image/png")
    } else if data.len() >= 3 && data[0..3] == [0xFF, 0xD8, 0xFF] {
        Some("image/jpeg")
    } else {
        None
    }
}

// ─── Relationship ───────────────────────────────────────────────────────────

/// An OOXML relationship entry.
struct Relationship {
    id: String,
    rel_type: String,
    target: String,
}

// ─── DocxBuilder ────────────────────────────────────────────────────────────

/// A numbering definition for lists (maps to abstractNum + num in numbering.xml).
struct NumberingDef {
    abstract_num_id: u32,
    num_id: u32,
    is_ordered: bool,
    num_fmt: String,
}

/// Builds the OOXML parts and assembles them into a ZIP archive.
struct DocxBuilder {
    body_xml: String,
    relationships: Vec<Relationship>,
    footnotes: Vec<(u32, String)>,
    media: Vec<(String, Vec<u8>)>,
    numbering_xml: Option<String>,
    numbering_defs: Vec<NumberingDef>,
    footnote_id_map: HashMap<String, u32>,
    resources: HashMap<String, docmux_ast::ResourceData>,
    next_rel_id: u32,
    next_footnote_id: u32,
    next_image_id: u32,
    next_num_id: u32,
}

impl DocxBuilder {
    fn new() -> Self {
        Self {
            body_xml: String::new(),
            relationships: Vec::new(),
            footnotes: Vec::new(),
            media: Vec::new(),
            numbering_xml: None,
            numbering_defs: Vec::new(),
            footnote_id_map: HashMap::new(),
            resources: HashMap::new(),
            next_rel_id: 1,
            next_footnote_id: 2,
            next_image_id: 1,
            next_num_id: 1,
        }
    }

    /// Register a relationship and return its rId.
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

    /// First pass: collect FootnoteDef blocks, assign IDs, and render their content.
    fn collect_footnotes(&mut self, blocks: &[Block]) {
        for block in blocks {
            if let Block::FootnoteDef { id, content } = block {
                let footnote_id = self.next_footnote_id;
                self.next_footnote_id += 1;
                self.footnote_id_map.insert(id.clone(), footnote_id);

                // Render footnote content into a temporary buffer
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
            Block::CodeBlock { content, .. } => {
                for line in content.lines() {
                    self.body_xml
                        .push_str("<w:p><w:pPr><w:pStyle w:val=\"CodeBlock\"/></w:pPr>");
                    self.body_xml.push_str("<w:r><w:t xml:space=\"preserve\">");
                    self.body_xml.push_str(&xml_escape(line));
                    self.body_xml.push_str("</w:t></w:r></w:p>\n");
                }
            }
            Block::MathBlock { content, .. } => {
                self.body_xml
                    .push_str("<w:p><w:pPr><w:pStyle w:val=\"MathBlock\"/></w:pPr>");
                self.body_xml.push_str("<w:r><w:t xml:space=\"preserve\">");
                self.body_xml.push_str(&xml_escape(content));
                self.body_xml.push_str("</w:t></w:r></w:p>\n");
            }
            Block::BlockQuote { content } => {
                for child in content {
                    match child {
                        Block::Paragraph { content: inlines } => {
                            self.body_xml
                                .push_str("<w:p><w:pPr><w:pStyle w:val=\"BlockQuote\"/></w:pPr>");
                            self.write_inlines(inlines, &[]);
                            self.body_xml.push_str("</w:p>\n");
                        }
                        other => self.write_block(other),
                    }
                }
            }
            Block::ThematicBreak => {
                self.body_xml.push_str(
                    "<w:p><w:pPr><w:pBdr>\
                     <w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"auto\"/>\
                     </w:pBdr></w:pPr></w:p>\n",
                );
            }
            Block::RawBlock { format, content } => {
                if format == "docx" || format == "openxml" {
                    self.body_xml.push_str(content);
                }
                // Skip other formats
            }
            Block::Div { content, .. } => {
                self.write_blocks(content);
            }
            Block::Table(table) => {
                self.write_table(table);
            }
            list @ Block::List { .. } => {
                self.write_list(list, 0);
            }
            Block::Figure { image, caption, .. } => {
                if let Some((rel_id, cx, cy)) = self.embed_image(&image.url) {
                    let img_id = self.next_image_id - 1;
                    self.body_xml
                        .push_str("<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr>");
                    self.write_image_drawing(&rel_id, &image.alt_text(), img_id, cx, cy);
                    self.body_xml.push_str("</w:p>\n");
                } else {
                    self.body_xml.push_str("<w:p>");
                    self.write_image_fallback(&image.url);
                    self.body_xml.push_str("</w:p>\n");
                }
                // Caption
                if let Some(cap) = caption {
                    self.body_xml
                        .push_str("<w:p><w:pPr><w:pStyle w:val=\"Caption\"/></w:pPr>");
                    self.write_inlines(cap, &[]);
                    self.body_xml.push_str("</w:p>\n");
                }
            }
            Block::Admonition {
                kind,
                title,
                content,
            } => {
                let kind_label = match kind {
                    docmux_ast::AdmonitionKind::Note => "Note",
                    docmux_ast::AdmonitionKind::Warning => "Warning",
                    docmux_ast::AdmonitionKind::Tip => "Tip",
                    docmux_ast::AdmonitionKind::Important => "Important",
                    docmux_ast::AdmonitionKind::Caution => "Caution",
                    docmux_ast::AdmonitionKind::Custom(s) => s.as_str(),
                };

                let title_text = title
                    .as_ref()
                    .map(|t| {
                        t.iter()
                            .filter_map(|i| {
                                if let Inline::Text { value } = i {
                                    Some(value.as_str())
                                } else {
                                    None
                                }
                            })
                            .collect::<String>()
                    })
                    .unwrap_or_else(|| kind_label.to_string());

                // Title with left border and bold
                self.body_xml.push_str(ADMONITION_PARA_PREFIX);
                write!(
                    self.body_xml,
                    "<w:r><w:rPr><w:b/></w:rPr><w:t>{}</w:t></w:r>",
                    xml_escape(&title_text)
                )
                .unwrap();
                self.body_xml.push_str("</w:p>\n");

                // Content with same border
                for child in content {
                    match child {
                        Block::Paragraph { content: inlines } => {
                            self.body_xml.push_str(ADMONITION_PARA_PREFIX);
                            self.write_inlines(inlines, &[]);
                            self.body_xml.push_str("</w:p>\n");
                        }
                        other => self.write_block(other),
                    }
                }
            }
            Block::DefinitionList { items } => {
                for item in items {
                    // Term in bold
                    self.body_xml.push_str("<w:p>");
                    self.write_inlines(&item.term, &["<w:b/>"]);
                    self.body_xml.push_str("</w:p>\n");

                    // Definitions indented
                    for def_blocks in &item.definitions {
                        for block in def_blocks {
                            match block {
                                Block::Paragraph { content } => {
                                    self.body_xml
                                        .push_str("<w:p><w:pPr><w:ind w:left=\"720\"/></w:pPr>");
                                    self.write_inlines(content, &[]);
                                    self.body_xml.push_str("</w:p>\n");
                                }
                                other => self.write_block(other),
                            }
                        }
                    }
                }
            }
            Block::FootnoteDef { .. } => {
                // Rendered during collect_footnotes pass — skip here
            }
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

    fn write_table(&mut self, table: &docmux_ast::Table) {
        // Caption before table
        if let Some(caption) = &table.caption {
            self.body_xml
                .push_str("<w:p><w:pPr><w:pStyle w:val=\"Caption\"/></w:pPr>");
            self.write_inlines(caption, &[]);
            self.body_xml.push_str("</w:p>\n");
        }

        self.body_xml.push_str("<w:tbl>\n");

        // Table properties: auto-width, grid borders
        self.body_xml.push_str(
            "<w:tblPr>\
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
        self.body_xml.push_str("<w:tblGrid>");
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

        self.body_xml.push_str("</w:tbl>\n");
    }

    fn write_table_row(
        &mut self,
        cells: &[docmux_ast::TableCell],
        columns: &[docmux_ast::ColumnSpec],
        is_header: bool,
    ) {
        self.body_xml.push_str("<w:tr>");
        if is_header {
            self.body_xml.push_str("<w:trPr><w:tblHeader/></w:trPr>");
        }
        self.body_xml.push('\n');

        for (i, cell) in cells.iter().enumerate() {
            self.body_xml.push_str("<w:tc><w:tcPr>");
            self.body_xml.push_str("<w:tcW w:w=\"0\" w:type=\"auto\"/>");

            if cell.colspan > 1 {
                write!(self.body_xml, "<w:gridSpan w:val=\"{}\"/>", cell.colspan).unwrap();
            }
            if cell.rowspan > 1 {
                self.body_xml.push_str("<w:vMerge w:val=\"restart\"/>");
            }

            // Column alignment as paragraph justification
            if let Some(col) = columns.get(i) {
                match col.alignment {
                    Alignment::Center => {
                        self.body_xml.push_str("<w:vAlign w:val=\"center\"/>");
                    }
                    Alignment::Right => {
                        self.body_xml.push_str("<w:vAlign w:val=\"bottom\"/>");
                    }
                    Alignment::Left | Alignment::Default => {}
                }
            }

            self.body_xml.push_str("</w:tcPr>\n");

            // Cell content — OOXML requires at least one <w:p> per cell
            if cell.content.is_empty() {
                self.body_xml.push_str("<w:p/>\n");
            } else {
                for block in &cell.content {
                    self.write_block(block);
                }
            }

            self.body_xml.push_str("</w:tc>\n");
        }

        self.body_xml.push_str("</w:tr>\n");
    }

    /// Write an inline drawing element referencing an embedded image.
    fn write_image_drawing(&mut self, rel_id: &str, alt: &str, img_id: u32, cx: u32, cy: u32) {
        let alt_escaped = xml_escape(alt);
        write!(
            self.body_xml,
            "<w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">\
             <wp:extent cx=\"{cx}\" cy=\"{cy}\"/>\
             <wp:docPr id=\"{img_id}\" name=\"{alt_escaped}\"/>\
             <a:graphic xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\">\
             <a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
             <pic:pic xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
             <pic:nvPicPr><pic:cNvPr id=\"{img_id}\" name=\"{alt_escaped}\"/><pic:cNvPicPr/></pic:nvPicPr>\
             <pic:blipFill><a:blip r:embed=\"{rel_id}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
             <pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>\
             <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr>\
             </pic:pic></a:graphicData></a:graphic>\
             </wp:inline></w:drawing></w:r>",
        )
        .unwrap();
    }

    /// Write a fallback text run for an image that couldn't be embedded.
    fn write_image_fallback(&mut self, url: &str) {
        write!(
            self.body_xml,
            "<w:r><w:t>[Image: {}]</w:t></w:r>",
            xml_escape(url)
        )
        .unwrap();
    }

    /// Embed an image and return (rel_id, width_emu, height_emu).
    /// Resolution order: doc.resources → filesystem → None (fallback).
    fn embed_image(&mut self, url: &str) -> Option<(String, u32, u32)> {
        // 1. Check doc.resources
        let data = if let Some(res) = self.resources.get(url) {
            res.data.clone()
        } else {
            // 2. Filesystem fallback
            let path = std::path::Path::new(url);
            if path.exists() {
                std::fs::read(path).ok()?
            } else {
                return None;
            }
        };

        let mime = detect_mime(&data);
        let ext = match mime {
            Some("image/png") => "png",
            Some("image/jpeg") => "jpeg",
            _ => std::path::Path::new(url).extension()?.to_str()?,
        };

        let filename = format!("image{}.{}", self.next_image_id, ext);
        self.next_image_id += 1;

        let rel_id = self.add_relationship(
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
            &format!("media/{filename}"),
        );

        // Compute dimensions: real size capped at 6 inches wide
        let (cx, cy) = match image_dimensions(&data) {
            Some((w, h)) if w > 0 && h > 0 => {
                let emu_w = w * 914400 / 96;
                let emu_h = h * 914400 / 96;
                let max_width: u32 = 5_486_400; // 6 inches
                if emu_w > max_width {
                    let scale = max_width as f64 / emu_w as f64;
                    (max_width, (emu_h as f64 * scale) as u32)
                } else {
                    (emu_w, emu_h)
                }
            }
            _ => (3_657_600, 2_743_200), // fallback 4"×3"
        };

        self.media.push((filename, data));
        Some((rel_id, cx, cy))
    }

    fn get_or_create_numbering(
        &mut self,
        ordered: bool,
        style: Option<&docmux_ast::ListStyle>,
    ) -> u32 {
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
        if let Block::List {
            ordered,
            items,
            style,
            ..
        } = list
        {
            let num_id = self.get_or_create_numbering(*ordered, style.as_ref());

            for item in items {
                for block in &item.content {
                    match block {
                        Block::Paragraph { content } => {
                            self.body_xml.push_str("<w:p><w:pPr>");
                            write!(
                                self.body_xml,
                                "<w:numPr><w:ilvl w:val=\"{depth}\"/>\
                                 <w:numId w:val=\"{num_id}\"/></w:numPr>"
                            )
                            .unwrap();
                            self.body_xml.push_str("</w:pPr>");
                            self.write_inlines(content, &[]);
                            self.body_xml.push_str("</w:p>\n");
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
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n\
             <w:numbering xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\n",
        );

        for def in &self.numbering_defs {
            let lvl_text = if def.num_fmt == "bullet" {
                "<w:lvlText w:val=\"\u{2022}\"/>"
            } else {
                "<w:lvlText w:val=\"%1.\"/>"
            };
            writeln!(
                xml,
                "<w:abstractNum w:abstractNumId=\"{id}\">\
                 <w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/>\
                 <w:numFmt w:val=\"{fmt}\"/>{lvl_text}\
                 <w:pPr><w:ind w:left=\"720\" w:hanging=\"360\"/></w:pPr></w:lvl>\
                 <w:lvl w:ilvl=\"1\"><w:start w:val=\"1\"/>\
                 <w:numFmt w:val=\"{fmt}\"/>{lvl_text}\
                 <w:pPr><w:ind w:left=\"1440\" w:hanging=\"360\"/></w:pPr></w:lvl>\
                 <w:lvl w:ilvl=\"2\"><w:start w:val=\"1\"/>\
                 <w:numFmt w:val=\"{fmt}\"/>{lvl_text}\
                 <w:pPr><w:ind w:left=\"2160\" w:hanging=\"360\"/></w:pPr></w:lvl>\
                 </w:abstractNum>",
                id = def.abstract_num_id,
                fmt = def.num_fmt,
            )
            .unwrap();

            writeln!(
                xml,
                "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"{}\"/></w:num>",
                def.num_id, def.abstract_num_id
            )
            .unwrap();
        }

        xml.push_str("</w:numbering>");
        Some(xml)
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
            Inline::Link { url, content, .. } => {
                let rel_id = self.add_relationship(
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink",
                    url,
                );
                write!(self.body_xml, "<w:hyperlink r:id=\"{rel_id}\">").unwrap();
                let mut props = run_props.to_vec();
                props.push("<w:rStyle w:val=\"Hyperlink\"/>");
                self.write_inlines(content, &props);
                self.body_xml.push_str("</w:hyperlink>");
            }
            Inline::FootnoteRef { id } => {
                if let Some(&footnote_id) = self.footnote_id_map.get(id.as_str()) {
                    self.body_xml
                        .push_str("<w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr>");
                    write!(
                        self.body_xml,
                        "<w:footnoteReference w:id=\"{footnote_id}\"/>"
                    )
                    .unwrap();
                    self.body_xml.push_str("</w:r>");
                }
            }
            Inline::Image(image) => {
                if let Some((rel_id, cx, cy)) = self.embed_image(&image.url) {
                    let img_id = self.next_image_id - 1;
                    self.write_image_drawing(&rel_id, &image.alt_text(), img_id, cx, cy);
                } else {
                    self.write_image_fallback(&image.url);
                }
            }
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
            let external = if rel.rel_type.contains("hyperlink") {
                " TargetMode=\"External\""
            } else {
                ""
            };
            writeln!(
                xml,
                "<Relationship Id=\"{}\" Type=\"{}\" Target=\"{}\"{}/>",
                xml_escape(&rel.id),
                xml_escape(&rel.rel_type),
                xml_escape(&rel.target),
                external,
            )
            .unwrap();
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

    fn write_bytes(&self, doc: &Document, opts: &WriteOptions) -> Result<Vec<u8>> {
        let mut builder = DocxBuilder::new();
        builder.resources = doc.resources.clone();
        // First pass: collect footnote definitions
        builder.collect_footnotes(&doc.content);
        if opts.standalone {
            builder.write_metadata(&doc.metadata);
        }
        builder.write_blocks(&doc.content);
        builder.numbering_xml = builder.build_numbering_xml();
        builder.assemble_zip()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read as _};

    /// Extract any file from DOCX bytes by path within the ZIP.
    fn extract_zip_file(bytes: &[u8], name: &str) -> String {
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut file = archive.by_name(name).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        contents
    }

    /// Extract word/document.xml from a DOCX byte buffer.
    fn extract_document_xml(bytes: &[u8]) -> String {
        extract_zip_file(bytes, "word/document.xml")
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
        let opts = WriteOptions {
            standalone: true,
            ..Default::default()
        };
        let bytes = w.write_bytes(&doc, &opts).unwrap();
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
        assert!(
            xml.contains(r#"<w:pStyle w:val="CodeBlock"/>"#),
            "missing CodeBlock style, got:\n{xml}"
        );
        assert!(
            xml.contains("fn main() {}"),
            "missing code content, got:\n{xml}"
        );
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
        assert!(
            xml.contains(r#"<w:pStyle w:val="MathBlock"/>"#),
            "missing MathBlock style, got:\n{xml}"
        );
        assert!(
            xml.contains("E = mc^2"),
            "missing math content, got:\n{xml}"
        );
    }

    #[test]
    fn blockquote_uses_blockquote_style() {
        let doc = Document {
            content: vec![Block::BlockQuote {
                content: vec![Block::Paragraph {
                    content: vec![Inline::Text {
                        value: "A quote".into(),
                    }],
                }],
            }],
            ..Default::default()
        };
        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
        let xml = extract_document_xml(&bytes);
        assert!(
            xml.contains(r#"<w:pStyle w:val="BlockQuote"/>"#),
            "missing BlockQuote style, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>A quote</w:t>"),
            "missing quote text, got:\n{xml}"
        );
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
        assert!(
            xml.contains("<w:pBdr>"),
            "missing pBdr element, got:\n{xml}"
        );
        assert!(
            xml.contains(r#"<w:bottom w:val="single""#),
            "missing bottom border, got:\n{xml}"
        );
    }

    #[test]
    fn table_renders_with_header_and_rows() {
        use docmux_ast::{Alignment, ColumnSpec, Table, TableCell};

        let doc = Document {
            content: vec![Block::Table(Table {
                caption: Some(vec![Inline::Text {
                    value: "Results".into(),
                }]),
                label: None,
                columns: vec![
                    ColumnSpec {
                        alignment: Alignment::Left,
                        width: None,
                    },
                    ColumnSpec {
                        alignment: Alignment::Right,
                        width: None,
                    },
                ],
                header: Some(vec![
                    TableCell {
                        content: vec![Block::Paragraph {
                            content: vec![Inline::Text {
                                value: "Name".into(),
                            }],
                        }],
                        colspan: 1,
                        rowspan: 1,
                    },
                    TableCell {
                        content: vec![Block::Paragraph {
                            content: vec![Inline::Text {
                                value: "Score".into(),
                            }],
                        }],
                        colspan: 1,
                        rowspan: 1,
                    },
                ]),
                rows: vec![vec![
                    TableCell {
                        content: vec![Block::Paragraph {
                            content: vec![Inline::Text {
                                value: "Alice".into(),
                            }],
                        }],
                        colspan: 1,
                        rowspan: 1,
                    },
                    TableCell {
                        content: vec![Block::Paragraph {
                            content: vec![Inline::Text { value: "95".into() }],
                        }],
                        colspan: 1,
                        rowspan: 1,
                    },
                ]],
                foot: None,
                attrs: None,
            })],
            ..Default::default()
        };

        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
        let xml = extract_document_xml(&bytes);

        assert!(
            xml.contains("<w:tbl>"),
            "missing w:tbl element, got:\n{xml}"
        );
        assert!(xml.contains("<w:tr>"), "missing w:tr element, got:\n{xml}");
        assert!(xml.contains("<w:tc>"), "missing w:tc element, got:\n{xml}");
        assert!(
            xml.contains("<w:t>Name</w:t>"),
            "missing header cell Name, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>Alice</w:t>"),
            "missing body cell Alice, got:\n{xml}"
        );
        // Caption rendered as a paragraph
        assert!(
            xml.contains("<w:t>Results</w:t>"),
            "missing caption text, got:\n{xml}"
        );
    }

    #[test]
    fn unordered_list_renders_bullets() {
        use docmux_ast::ListItem;

        let doc = Document {
            content: vec![Block::List {
                ordered: false,
                start: None,
                items: vec![
                    ListItem {
                        checked: None,
                        content: vec![Block::Paragraph {
                            content: vec![Inline::Text {
                                value: "First".into(),
                            }],
                        }],
                    },
                    ListItem {
                        checked: None,
                        content: vec![Block::Paragraph {
                            content: vec![Inline::Text {
                                value: "Second".into(),
                            }],
                        }],
                    },
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

        assert!(
            xml.contains("<w:numPr>"),
            "missing numPr element, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>First</w:t>"),
            "missing First text, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>Second</w:t>"),
            "missing Second text, got:\n{xml}"
        );

        // Verify numbering.xml exists in the ZIP
        let cursor = std::io::Cursor::new(&bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        assert!(
            archive.by_name("word/numbering.xml").is_ok(),
            "missing word/numbering.xml in ZIP"
        );
    }

    #[test]
    fn ordered_list_renders_numbers() {
        use docmux_ast::ListItem;

        let doc = Document {
            content: vec![Block::List {
                ordered: true,
                start: Some(1),
                items: vec![ListItem {
                    checked: None,
                    content: vec![Block::Paragraph {
                        content: vec![Inline::Text {
                            value: "Alpha".into(),
                        }],
                    }],
                }],
                tight: true,
                style: None,
                delimiter: None,
            }],
            ..Default::default()
        };

        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
        let xml = extract_document_xml(&bytes);

        assert!(
            xml.contains("<w:numPr>"),
            "missing numPr element, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>Alpha</w:t>"),
            "missing Alpha text, got:\n{xml}"
        );
    }

    #[test]
    fn hyperlink_creates_relationship() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Link {
                    url: "https://example.com".into(),
                    title: None,
                    content: vec![Inline::Text {
                        value: "click".into(),
                    }],
                    attrs: None,
                }],
            }],
            ..Default::default()
        };

        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
        let xml = extract_document_xml(&bytes);

        assert!(
            xml.contains("<w:hyperlink"),
            "missing w:hyperlink element, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>click</w:t>"),
            "missing hyperlink text, got:\n{xml}"
        );

        let rels = extract_zip_file(&bytes, "word/_rels/document.xml.rels");
        assert!(
            rels.contains("https://example.com"),
            "missing hyperlink in rels, got:\n{rels}"
        );
    }

    #[test]
    fn footnotes_create_footnotes_xml() {
        let doc = Document {
            content: vec![
                Block::Paragraph {
                    content: vec![
                        Inline::Text {
                            value: "Text".into(),
                        },
                        Inline::FootnoteRef { id: "1".into() },
                    ],
                },
                Block::FootnoteDef {
                    id: "1".into(),
                    content: vec![Block::Paragraph {
                        content: vec![Inline::Text {
                            value: "A footnote".into(),
                        }],
                    }],
                },
            ],
            ..Default::default()
        };

        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();

        let footnotes_xml = extract_zip_file(&bytes, "word/footnotes.xml");
        assert!(
            footnotes_xml.contains("<w:t>A footnote</w:t>"),
            "missing footnote content, got:\n{footnotes_xml}"
        );

        let doc_xml = extract_document_xml(&bytes);
        assert!(
            doc_xml.contains("<w:footnoteReference"),
            "missing footnoteReference, got:\n{doc_xml}"
        );
    }

    #[test]
    fn admonition_renders_with_border() {
        let doc = Document {
            content: vec![Block::Admonition {
                kind: docmux_ast::AdmonitionKind::Note,
                title: Some(vec![Inline::Text {
                    value: "Note".into(),
                }]),
                content: vec![Block::Paragraph {
                    content: vec![Inline::Text {
                        value: "Important info".into(),
                    }],
                }],
            }],
            ..Default::default()
        };

        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
        let xml = extract_document_xml(&bytes);

        assert!(xml.contains("<w:b/>"), "missing bold title, got:\n{xml}");
        assert!(
            xml.contains("<w:t>Note</w:t>"),
            "missing Note title, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>Important info</w:t>"),
            "missing content, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:pBdr>"),
            "missing border element, got:\n{xml}"
        );
    }

    #[test]
    fn definition_list_renders() {
        let doc = Document {
            content: vec![Block::DefinitionList {
                items: vec![docmux_ast::DefinitionItem {
                    term: vec![Inline::Text {
                        value: "Term".into(),
                    }],
                    definitions: vec![vec![Block::Paragraph {
                        content: vec![Inline::Text {
                            value: "Definition".into(),
                        }],
                    }]],
                }],
            }],
            ..Default::default()
        };

        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
        let xml = extract_document_xml(&bytes);

        assert!(xml.contains("<w:b/>"), "missing bold term, got:\n{xml}");
        assert!(
            xml.contains("<w:t>Term</w:t>"),
            "missing term text, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>Definition</w:t>"),
            "missing definition text, got:\n{xml}"
        );
    }

    #[test]
    fn image_embeds_local_file() {
        // Create a tiny 1x1 PNG for testing
        let tmp_dir = std::env::temp_dir().join("docmux-docx-test");
        std::fs::create_dir_all(&tmp_dir).ok();
        let img_path = tmp_dir.join("test.png");

        // Minimal valid PNG (1x1 red pixel)
        let png_bytes: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, // RGB
            0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, // IDAT chunk
            0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21,
            0xBC, 0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, // IEND chunk
            0xAE, 0x42, 0x60, 0x82,
        ];
        std::fs::write(&img_path, png_bytes).unwrap();

        let doc = Document {
            content: vec![Block::Figure {
                image: docmux_ast::Image {
                    url: img_path.to_str().unwrap().into(),
                    alt: vec![Inline::Text {
                        value: "A test image".into(),
                    }],
                    title: None,
                    attrs: None,
                },
                caption: Some(vec![Inline::Text {
                    value: "Figure 1".into(),
                }]),
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };

        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();

        // Verify image is in ZIP
        let cursor = Cursor::new(&bytes);
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
        assert!(
            xml.contains("<w:drawing>") || xml.contains("<wp:inline"),
            "missing drawing element, got:\n{xml}"
        );
        assert!(
            xml.contains("<w:t>Figure 1</w:t>"),
            "missing caption, got:\n{xml}"
        );

        let _ = std::fs::remove_file(&img_path);
    }

    #[test]
    fn xml_escape_special_chars() {
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(xml_escape("plain"), "plain");
    }

    #[test]
    fn png_dimensions_parsed() {
        let png: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE,
        ];
        assert_eq!(image_dimensions(png), Some((1, 1)));
    }

    #[test]
    fn png_dimensions_large_image() {
        let mut png = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52,
        ];
        png.extend_from_slice(&800_u32.to_be_bytes());
        png.extend_from_slice(&600_u32.to_be_bytes());
        assert_eq!(image_dimensions(&png), Some((800, 600)));
    }

    #[test]
    fn jpeg_dimensions_parsed() {
        let jpeg: &[u8] = &[
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x02, 0xFF, 0xC0, 0x00, 0x0B, 0x08, 0x01,
            0xE0, // height = 480
            0x02, 0x80, // width = 640
        ];
        assert_eq!(image_dimensions(jpeg), Some((640, 480)));
    }

    #[test]
    fn unknown_format_returns_none() {
        assert_eq!(image_dimensions(&[0x00, 0x01, 0x02]), None);
    }

    #[test]
    fn mime_from_magic_bytes_png() {
        let png = [0x89, 0x50, 0x4E, 0x47];
        assert_eq!(detect_mime(&png), Some("image/png"));
    }

    #[test]
    fn mime_from_magic_bytes_jpeg() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(detect_mime(&jpeg), Some("image/jpeg"));
    }

    #[test]
    fn mime_from_unknown_bytes() {
        assert_eq!(detect_mime(&[0x00, 0x01]), None);
    }

    #[test]
    fn image_embeds_from_resources() {
        let png_bytes: Vec<u8> = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC,
            0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ];
        let mut resources = HashMap::new();
        resources.insert(
            "photo.png".to_string(),
            docmux_ast::ResourceData {
                mime_type: "image/png".to_string(),
                data: png_bytes,
            },
        );
        let doc = Document {
            content: vec![Block::Figure {
                image: docmux_ast::Image {
                    url: "photo.png".into(),
                    alt: vec![Inline::Text {
                        value: "A photo".into(),
                    }],
                    title: None,
                    attrs: None,
                },
                caption: None,
                label: None,
                attrs: None,
            }],
            resources,
            ..Default::default()
        };

        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();

        let cursor = Cursor::new(&bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let mut found = false;
        for i in 0..archive.len() {
            if archive
                .by_index(i)
                .unwrap()
                .name()
                .starts_with("word/media/")
            {
                found = true;
            }
        }
        assert!(
            found,
            "Image from resources should be embedded in word/media/"
        );

        let xml = extract_document_xml(&bytes);
        assert!(xml.contains("<w:drawing>") || xml.contains("<wp:inline"));
    }

    #[test]
    fn image_dimensions_capped_at_six_inches() {
        // 1200x600 PNG header → 12.5"x6.25" at 96 DPI → capped to 6"x3"
        let mut png = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52,
        ];
        png.extend_from_slice(&1200_u32.to_be_bytes());
        png.extend_from_slice(&600_u32.to_be_bytes());
        png.extend_from_slice(&[0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE]);
        png.extend_from_slice(&[
            0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0,
            0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC, 0x33, 0x00, 0x00, 0x00, 0x00,
            0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]);

        let mut resources = HashMap::new();
        resources.insert(
            "wide.png".to_string(),
            docmux_ast::ResourceData {
                mime_type: "image/png".to_string(),
                data: png,
            },
        );
        let doc = Document {
            content: vec![Block::Figure {
                image: docmux_ast::Image {
                    url: "wide.png".into(),
                    alt: vec![],
                    title: None,
                    attrs: None,
                },
                caption: None,
                label: None,
                attrs: None,
            }],
            resources,
            ..Default::default()
        };

        let w = DocxWriter::new();
        let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
        let xml = extract_document_xml(&bytes);

        // Max width = 6 inches = 5486400 EMU, height scales proportionally
        assert!(
            xml.contains("cx=\"5486400\""),
            "Width should be capped at 6 inches (5486400 EMU), got:\n{xml}"
        );
        assert!(
            xml.contains("cy=\"2743200\""),
            "Height should scale proportionally to 3 inches (2743200 EMU), got:\n{xml}"
        );
    }
}
