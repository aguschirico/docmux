//! Parses the DOCX `word/document.xml` body into AST blocks.

use crate::numbering::NumberingMap;
use crate::relationships::RelMap;
use crate::styles::{StyleKind, StyleMap};
use crate::DocxError;
use docmux_ast::{Alignment, Block, ColumnSpec, Image, Inline, Table, TableCell};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

// ─── ParseContext ───────────────────────────────────────────────────────────

/// Context needed while parsing the document body.
pub(crate) struct ParseContext<'a> {
    pub(crate) styles: &'a StyleMap,
    /// Numbering definitions — used for future list assembly.
    #[allow(dead_code)]
    pub(crate) numbering: &'a NumberingMap,
    pub(crate) rels: &'a RelMap,
    #[allow(dead_code)]
    pub(crate) archive: &'a crate::archive::DocxArchive,
}

// ─── XML reconstruction helpers ─────────────────────────────────────────────

/// Append a start tag (with attributes) reconstructed from a `BytesStart`.
fn append_start_tag(buf: &mut String, e: &BytesStart<'_>) {
    buf.push('<');
    buf.push_str(&String::from_utf8_lossy(e.name().as_ref()));
    for attr in e.attributes().flatten() {
        buf.push(' ');
        buf.push_str(&String::from_utf8_lossy(attr.key.as_ref()));
        buf.push_str("=\"");
        buf.push_str(&quick_xml_escape(&String::from_utf8_lossy(&attr.value)));
        buf.push('"');
    }
    buf.push('>');
}

/// Append an end tag.
fn append_end_tag(buf: &mut String, name: &[u8]) {
    buf.push_str("</");
    buf.push_str(&String::from_utf8_lossy(name));
    buf.push('>');
}

/// Append a self-closing empty tag.
fn append_empty_tag(buf: &mut String, e: &BytesStart<'_>) {
    buf.push('<');
    buf.push_str(&String::from_utf8_lossy(e.name().as_ref()));
    for attr in e.attributes().flatten() {
        buf.push(' ');
        buf.push_str(&String::from_utf8_lossy(attr.key.as_ref()));
        buf.push_str("=\"");
        buf.push_str(&quick_xml_escape(&String::from_utf8_lossy(&attr.value)));
        buf.push('"');
    }
    buf.push_str("/>");
}

/// Minimal XML escaping for attribute values.
fn quick_xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ─── parse_body ─────────────────────────────────────────────────────────────

/// Parse `<w:body>` from the document XML into a list of blocks.
pub(crate) fn parse_body(xml: &str, ctx: &ParseContext<'_>) -> Result<Vec<Block>, DocxError> {
    let mut blocks = Vec::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();
    let mut in_body = false;
    let mut depth: u32 = 0;
    let mut element_xml = String::new();
    let mut element_kind: Option<ElementKind> = None;
    let mut element_depth: u32 = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();

                if name == b"body" && !in_body {
                    in_body = true;
                    depth = 0;
                    continue;
                }

                if in_body {
                    if depth == 0 {
                        // Top-level element inside <w:body>
                        element_xml.clear();
                        if name == b"p" {
                            element_kind = Some(ElementKind::Paragraph);
                        } else if name == b"tbl" {
                            element_kind = Some(ElementKind::Table);
                        } else {
                            element_kind = Some(ElementKind::Other);
                        }
                        element_depth = 1;
                        append_start_tag(&mut element_xml, e);
                    } else if element_kind.is_some() {
                        element_depth += 1;
                        append_start_tag(&mut element_xml, e);
                    }
                    depth += 1;
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();

                if name == b"body" && in_body {
                    in_body = false;
                    continue;
                }

                if in_body {
                    if let Some(ref kind) = element_kind {
                        if element_depth > 0 {
                            element_depth -= 1;
                            append_end_tag(&mut element_xml, e.name().as_ref());
                        }
                        if element_depth == 0 {
                            match kind {
                                ElementKind::Paragraph => {
                                    if let Some(block) = parse_paragraph(&element_xml, ctx)? {
                                        blocks.push(block);
                                    }
                                }
                                ElementKind::Table => {
                                    blocks.push(parse_table(&element_xml, ctx)?);
                                }
                                ElementKind::Other => {}
                            }
                            element_kind = None;
                            element_xml.clear();
                        }
                    }
                    depth = depth.saturating_sub(1);
                }
            }
            Ok(Event::Empty(ref e)) if in_body && element_kind.is_some() => {
                append_empty_tag(&mut element_xml, e);
            }
            Ok(Event::Text(ref e)) if in_body && element_kind.is_some() => {
                let text = e
                    .unescape()
                    .map_err(|err| DocxError::Xml(err.to_string()))?;
                element_xml.push_str(&quick_xml_escape(&text));
            }
            Ok(Event::CData(ref e)) if in_body && element_kind.is_some() => {
                element_xml.push_str(&String::from_utf8_lossy(e.as_ref()));
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocxError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(blocks)
}

#[derive(Debug)]
enum ElementKind {
    Paragraph,
    Table,
    Other,
}

// ─── parse_drawing ──────────────────────────────────────────────────────────

/// Extract an `Inline::Image` from a `<w:drawing>` element.
///
/// Handles both `<wp:inline>` and `<wp:anchor>` drawing types.
/// Returns `None` if no image relationship can be resolved.
fn parse_drawing(xml: &str, ctx: &ParseContext<'_>) -> Result<Option<Inline>, DocxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut alt_text = String::new();
    let mut embed_rid: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"docPr" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"name" {
                                alt_text = String::from_utf8_lossy(&attr.value).into_owned();
                            }
                        }
                    }
                    b"blip" => {
                        for attr in e.attributes().flatten() {
                            let key = attr.key.as_ref();
                            if key == b"r:embed" || key.ends_with(b":embed") {
                                embed_rid = Some(String::from_utf8_lossy(&attr.value).into_owned());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocxError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let rid = match embed_rid {
        Some(r) => r,
        None => return Ok(None),
    };

    let rel = match ctx.rels.get(&rid) {
        Some(r) => r,
        None => return Ok(None),
    };

    let url = rel.target.clone();

    let alt = if alt_text.is_empty() {
        vec![]
    } else {
        vec![Inline::Text { value: alt_text }]
    };

    Ok(Some(Inline::Image(Image {
        url,
        alt,
        title: None,
        attrs: None,
    })))
}

// ─── parse_runs (inline runs) ───────────────────────────────────────────────

/// Parse inline runs from paragraph XML content.
///
/// This extracts `<w:r>`, `<w:hyperlink>`, and bare content
/// from the XML of a `<w:p>` element.
pub(crate) fn parse_runs(xml: &str, ctx: &ParseContext<'_>) -> Result<Vec<Inline>, DocxError> {
    let mut inlines = Vec::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    let mut buf = Vec::new();

    // Run properties state
    let mut in_run = false;
    let mut in_rpr = false;
    let mut in_t = false;
    let mut fmt = RunFormat::default();
    let mut code_font = false;

    // Hyperlink state
    let mut in_hyperlink = false;
    let mut hyperlink_url = String::new();
    let mut hyperlink_inlines: Vec<Inline> = Vec::new();

    // Drawing state
    let mut in_drawing = false;
    let mut drawing_xml = String::new();
    let mut drawing_depth: u32 = 0;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();

                if in_drawing {
                    drawing_depth += 1;
                    append_start_tag(&mut drawing_xml, e);
                } else {
                    match name {
                        b"hyperlink" => {
                            in_hyperlink = true;
                            hyperlink_url.clear();
                            hyperlink_inlines.clear();

                            // Look for r:id attribute
                            for attr in e.attributes().flatten() {
                                let key = attr.key.as_ref();
                                // r:id or w:id — the relationship id
                                if key == b"r:id"
                                    || key.ends_with(b":id") && !key.ends_with(b"w:id")
                                {
                                    let rid = String::from_utf8_lossy(&attr.value).into_owned();
                                    if let Some(rel) = ctx.rels.get(&rid) {
                                        hyperlink_url = rel.target.clone();
                                    }
                                }
                            }
                        }
                        b"r" => {
                            in_run = true;
                            // Reset run properties
                            fmt = RunFormat::default();
                            code_font = false;
                        }
                        b"drawing" if in_run => {
                            in_drawing = true;
                            drawing_xml.clear();
                            drawing_depth = 0;
                            append_start_tag(&mut drawing_xml, e);
                        }
                        b"rPr" if in_run => {
                            in_rpr = true;
                        }
                        b"b" if in_rpr => {
                            fmt.bold = !is_val_false(e);
                        }
                        b"i" if in_rpr => {
                            fmt.italic = !is_val_false(e);
                        }
                        b"strike" if in_rpr => {
                            fmt.strike = !is_val_false(e);
                        }
                        b"u" if in_rpr => {
                            let val = get_val_attr(e);
                            fmt.underline = val.as_deref() != Some("none");
                        }
                        b"vertAlign" if in_rpr => {
                            let val = get_val_attr(e);
                            match val.as_deref() {
                                Some("superscript") => fmt.superscript = true,
                                Some("subscript") => fmt.subscript = true,
                                _ => {}
                            }
                        }
                        b"smallCaps" if in_rpr => {
                            fmt.small_caps = !is_val_false(e);
                        }
                        b"rFonts" if in_rpr => {
                            // Check if font is monospace
                            for attr in e.attributes().flatten() {
                                let key_local = attr.key.local_name();
                                if key_local.as_ref() == b"ascii"
                                    || key_local.as_ref() == b"hAnsi"
                                    || key_local.as_ref() == b"cs"
                                {
                                    let font_name = String::from_utf8_lossy(&attr.value);
                                    if is_monospace_font(&font_name) {
                                        code_font = true;
                                    }
                                }
                            }
                        }
                        b"t" if in_run => {
                            in_t = true;
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();

                if in_drawing {
                    append_empty_tag(&mut drawing_xml, e);
                } else {
                    match name {
                        // Inline formatting attributes — always empty elements in OOXML
                        b"rPr" if in_run => {
                            in_rpr = true;
                        }
                        b"b" if in_rpr => {
                            fmt.bold = !is_val_false(e);
                        }
                        b"i" if in_rpr => {
                            fmt.italic = !is_val_false(e);
                        }
                        b"strike" if in_rpr => {
                            fmt.strike = !is_val_false(e);
                        }
                        b"u" if in_rpr => {
                            let val = get_val_attr(e);
                            fmt.underline = val.as_deref() != Some("none");
                        }
                        b"vertAlign" if in_rpr => {
                            let val = get_val_attr(e);
                            match val.as_deref() {
                                Some("superscript") => fmt.superscript = true,
                                Some("subscript") => fmt.subscript = true,
                                _ => {}
                            }
                        }
                        b"smallCaps" if in_rpr => {
                            fmt.small_caps = !is_val_false(e);
                        }
                        b"rFonts" if in_rpr => {
                            for attr in e.attributes().flatten() {
                                let key_local = attr.key.local_name();
                                if key_local.as_ref() == b"ascii"
                                    || key_local.as_ref() == b"hAnsi"
                                    || key_local.as_ref() == b"cs"
                                {
                                    let font_name = String::from_utf8_lossy(&attr.value);
                                    if is_monospace_font(&font_name) {
                                        code_font = true;
                                    }
                                }
                            }
                        }
                        b"br" if in_run => {
                            let inline = Inline::HardBreak;
                            if in_hyperlink {
                                hyperlink_inlines.push(inline);
                            } else {
                                inlines.push(inline);
                            }
                        }
                        b"footnoteReference" if in_run => {
                            // <w:footnoteReference w:id="N"/>
                            for attr in e.attributes().flatten() {
                                if attr.key.local_name().as_ref() == b"id" {
                                    let id = String::from_utf8_lossy(&attr.value).into_owned();
                                    let inline = Inline::FootnoteRef { id };
                                    if in_hyperlink {
                                        hyperlink_inlines.push(inline);
                                    } else {
                                        inlines.push(inline);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_drawing {
                    let text = e
                        .unescape()
                        .map_err(|err| DocxError::Xml(err.to_string()))?;
                    drawing_xml.push_str(&text);
                } else if in_t && in_run {
                    let text = e
                        .unescape()
                        .map_err(|err| DocxError::Xml(err.to_string()))?;
                    let text = text.into_owned();
                    if !text.is_empty() {
                        let inline = if code_font {
                            Inline::Code {
                                value: text,
                                attrs: None,
                            }
                        } else {
                            let base = Inline::Text { value: text };
                            wrap_inline(base, &fmt)
                        };
                        if in_hyperlink {
                            hyperlink_inlines.push(inline);
                        } else {
                            inlines.push(inline);
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();

                if in_drawing {
                    if drawing_depth == 0 {
                        // Closing the <w:drawing> element itself
                        append_end_tag(&mut drawing_xml, e.name().as_ref());
                        in_drawing = false;
                        if let Some(inline) = parse_drawing(&drawing_xml, ctx)? {
                            if in_hyperlink {
                                hyperlink_inlines.push(inline);
                            } else {
                                inlines.push(inline);
                            }
                        }
                    } else {
                        drawing_depth -= 1;
                        append_end_tag(&mut drawing_xml, e.name().as_ref());
                    }
                } else {
                    match name {
                        b"hyperlink" => {
                            if in_hyperlink && !hyperlink_inlines.is_empty() {
                                inlines.push(Inline::Link {
                                    url: hyperlink_url.clone(),
                                    title: None,
                                    content: std::mem::take(&mut hyperlink_inlines),
                                    attrs: None,
                                });
                            }
                            in_hyperlink = false;
                        }
                        b"r" => {
                            in_run = false;
                            in_rpr = false;
                            in_t = false;
                        }
                        b"rPr" => {
                            in_rpr = false;
                        }
                        b"t" => {
                            in_t = false;
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocxError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(inlines)
}

/// Check if a `w:val` attribute is explicitly "false" or "0".
fn is_val_false(e: &BytesStart<'_>) -> bool {
    for attr in e.attributes().flatten() {
        if attr.key.local_name().as_ref() == b"val" {
            let val = String::from_utf8_lossy(&attr.value);
            return val == "false" || val == "0";
        }
    }
    false
}

/// Get the `w:val` attribute value from an element.
fn get_val_attr(e: &BytesStart<'_>) -> Option<String> {
    for attr in e.attributes().flatten() {
        if attr.key.local_name().as_ref() == b"val" {
            return Some(String::from_utf8_lossy(&attr.value).into_owned());
        }
    }
    None
}

/// Detect common monospace font names.
pub(crate) fn is_monospace_font(name: &str) -> bool {
    let lower = name.to_lowercase();
    matches!(
        lower.as_str(),
        "courier new"
            | "courier"
            | "consolas"
            | "monospace"
            | "menlo"
            | "monaco"
            | "dejavu sans mono"
            | "liberation mono"
            | "lucida console"
            | "andale mono"
            | "source code pro"
            | "fira code"
            | "fira mono"
            | "jetbrains mono"
            | "ibm plex mono"
            | "sf mono"
            | "cascadia code"
            | "cascadia mono"
            | "ubuntu mono"
            | "roboto mono"
            | "droid sans mono"
            | "inconsolata"
            | "hack"
    )
}

/// Inline formatting flags parsed from `<w:rPr>`.
#[derive(Default)]
struct RunFormat {
    bold: bool,
    italic: bool,
    strike: bool,
    underline: bool,
    superscript: bool,
    subscript: bool,
    small_caps: bool,
}

/// Wrap a base inline in formatting layers (outermost first).
fn wrap_inline(base: Inline, fmt: &RunFormat) -> Inline {
    let mut result = base;

    // Apply innermost first, so the outermost wrapping is applied last.
    // Order: smallCaps → subscript → superscript → underline → strike → italic → bold
    if fmt.small_caps {
        result = Inline::SmallCaps {
            content: vec![result],
        };
    }
    if fmt.subscript {
        result = Inline::Subscript {
            content: vec![result],
        };
    }
    if fmt.superscript {
        result = Inline::Superscript {
            content: vec![result],
        };
    }
    if fmt.underline {
        result = Inline::Underline {
            content: vec![result],
        };
    }
    if fmt.strike {
        result = Inline::Strikethrough {
            content: vec![result],
        };
    }
    if fmt.italic {
        result = Inline::Emphasis {
            content: vec![result],
        };
    }
    if fmt.bold {
        result = Inline::Strong {
            content: vec![result],
        };
    }
    result
}

// ─── extract_plain_text ─────────────────────────────────────────────────────

/// Extract concatenated plain text from a list of inlines.
pub(crate) fn extract_plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            Inline::Text { value } | Inline::Code { value, .. } => out.push_str(value),
            Inline::MathInline { value } => out.push_str(value),
            Inline::Strong { content }
            | Inline::Emphasis { content }
            | Inline::Strikethrough { content }
            | Inline::Underline { content }
            | Inline::Superscript { content }
            | Inline::Subscript { content }
            | Inline::SmallCaps { content }
            | Inline::Span { content, .. }
            | Inline::Quoted { content, .. }
            | Inline::Link { content, .. } => {
                out.push_str(&extract_plain_text(content));
            }
            Inline::SoftBreak => out.push(' '),
            Inline::HardBreak => out.push('\n'),
            _ => {}
        }
    }
    out
}

// ─── parse_paragraph ────────────────────────────────────────────────────────

/// Parse a `<w:p>` element's XML into a Block.
///
/// Returns `None` if the paragraph is empty and should be skipped.
fn parse_paragraph(xml: &str, ctx: &ParseContext<'_>) -> Result<Option<Block>, DocxError> {
    // Extract paragraph properties
    let p_style = extract_p_style(xml);
    let (num_id, _ilvl) = extract_num_pr(xml);
    let has_bottom_border = has_bottom_border(xml);
    let has_admonition_border = has_admonition_border(xml);

    // Parse inline runs
    let inlines = parse_runs(xml, ctx)?;

    // Classify by style
    if let Some(ref style_id) = p_style {
        if let Some(style_info) = ctx.styles.get(style_id) {
            match &style_info.kind {
                StyleKind::Heading(level) => {
                    return Ok(Some(Block::Heading {
                        level: *level,
                        id: None,
                        content: inlines,
                        attrs: None,
                    }));
                }
                StyleKind::Title => {
                    return Ok(Some(Block::Heading {
                        level: 1,
                        id: None,
                        content: inlines,
                        attrs: None,
                    }));
                }
                StyleKind::CodeBlock => {
                    let text = extract_plain_text(&inlines);
                    return Ok(Some(Block::CodeBlock {
                        language: None,
                        content: text,
                        caption: None,
                        label: None,
                        attrs: None,
                    }));
                }
                StyleKind::MathBlock => {
                    let text = extract_plain_text(&inlines);
                    return Ok(Some(Block::MathBlock {
                        content: text,
                        label: None,
                    }));
                }
                StyleKind::BlockQuote => {
                    if inlines.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(Block::BlockQuote {
                        content: vec![Block::Paragraph { content: inlines }],
                    }));
                }
                _ => {}
            }
        }
    }

    // ThematicBreak: bottom border + empty inlines
    if has_bottom_border && inlines.is_empty() {
        return Ok(Some(Block::ThematicBreak));
    }

    // Admonition: left border with specific color
    if has_admonition_border && !inlines.is_empty() {
        return Ok(Some(Block::Admonition {
            kind: docmux_ast::AdmonitionKind::Note,
            title: None,
            content: vec![Block::Paragraph { content: inlines }],
        }));
    }

    // Numbered/bulleted list item: if numId > 0, keep as a paragraph
    // (list assembly is deferred to a later stage)
    if let Some(nid) = num_id {
        if nid > 0 {
            // Paragraph with numbering info — will be assembled into lists later
            return Ok(Some(Block::Paragraph { content: inlines }));
        }
    }

    // Default: paragraph (skip if empty)
    if inlines.is_empty() {
        return Ok(None);
    }

    Ok(Some(Block::Paragraph { content: inlines }))
}

/// Extract pStyle value from paragraph XML.
fn extract_p_style(xml: &str) -> Option<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_ppr = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();
                if name == b"pPr" {
                    in_ppr = true;
                } else if in_ppr && name == b"pStyle" {
                    return get_val_attr(e);
                }
            }
            Ok(Event::End(ref e)) if in_ppr && e.local_name().as_ref() == b"pPr" => {
                return None;
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    None
}

/// Extract numId and ilvl from paragraph XML.
fn extract_num_pr(xml: &str) -> (Option<u32>, Option<u32>) {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_num_pr = false;
    let mut num_id: Option<u32> = None;
    let mut ilvl: Option<u32> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();
                if name == b"numPr" {
                    in_num_pr = true;
                } else if in_num_pr {
                    if name == b"numId" {
                        if let Some(val) = get_val_attr(e) {
                            num_id = val.parse().ok();
                        }
                    } else if name == b"ilvl" {
                        if let Some(val) = get_val_attr(e) {
                            ilvl = val.parse().ok();
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) if in_num_pr && e.local_name().as_ref() == b"numPr" => {
                return (num_id, ilvl);
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    (num_id, ilvl)
}

/// Check if paragraph has a bottom border (thematic break indicator).
fn has_bottom_border(xml: &str) -> bool {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_pbdr = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();
                if name == b"pBdr" {
                    in_pbdr = true;
                } else if in_pbdr && name == b"bottom" {
                    return true;
                }
            }
            Ok(Event::End(ref e)) if in_pbdr && e.local_name().as_ref() == b"pBdr" => {
                return false;
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    false
}

/// Check if paragraph has an admonition-style left border (blue, color 4472C4).
fn has_admonition_border(xml: &str) -> bool {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_pbdr = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();
                if name == b"pBdr" {
                    in_pbdr = true;
                } else if in_pbdr && name == b"left" {
                    for attr in e.attributes().flatten() {
                        if attr.key.local_name().as_ref() == b"color" {
                            let color = String::from_utf8_lossy(&attr.value).to_uppercase();
                            if color == "4472C4" {
                                return true;
                            }
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) if in_pbdr && e.local_name().as_ref() == b"pBdr" => {
                return false;
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }
    false
}

// ─── parse_table ────────────────────────────────────────────────────────────

/// Parse the inner content of a `<w:tc>` cell into blocks.
///
/// Splits the raw cell XML into top-level `<w:p>` elements and parses each
/// through `parse_paragraph`, preserving paragraph boundaries.
fn parse_cell_blocks(xml: &str, ctx: &ParseContext<'_>) -> Result<Vec<Block>, DocxError> {
    let mut blocks = Vec::new();
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut depth: u32 = 0;
    let mut element_xml = String::new();
    let mut in_paragraph = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = e.local_name();
                if depth == 0 && name.as_ref() == b"p" {
                    in_paragraph = true;
                    element_xml.clear();
                    append_start_tag(&mut element_xml, e);
                    depth = 1;
                } else if in_paragraph {
                    depth += 1;
                    append_start_tag(&mut element_xml, e);
                } else {
                    depth += 1;
                }
            }
            Ok(Event::End(ref e)) => {
                if in_paragraph {
                    depth -= 1;
                    append_end_tag(&mut element_xml, e.name().as_ref());
                    if depth == 0 {
                        if let Some(block) = parse_paragraph(&element_xml, ctx)? {
                            blocks.push(block);
                        }
                        in_paragraph = false;
                        element_xml.clear();
                    }
                } else {
                    depth = depth.saturating_sub(1);
                }
            }
            Ok(Event::Empty(ref e)) if in_paragraph => {
                append_empty_tag(&mut element_xml, e);
            }
            Ok(Event::Text(ref e)) if in_paragraph => {
                let text = e
                    .unescape()
                    .map_err(|err| DocxError::Xml(err.to_string()))?;
                element_xml.push_str(&quick_xml_escape(&text));
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocxError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(blocks)
}

/// Parse a `<w:tbl>` element's XML into a `Block::Table`.
fn parse_table(xml: &str, ctx: &ParseContext<'_>) -> Result<Block, DocxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut col_count: usize = 0;
    let mut rows: Vec<Vec<TableCell>> = Vec::new();
    let mut header_rows: Vec<Vec<TableCell>> = Vec::new();

    // Row state
    let mut in_row = false;
    let mut is_header_row = false;
    let mut current_cells: Vec<TableCell> = Vec::new();

    // Cell state
    let mut in_cell = false;
    let mut cell_depth: u32 = 0;
    let mut cell_xml = String::new();
    let mut cell_colspan: u32 = 1;
    let mut cell_vmerge_continue = false;

    // Grid state
    let mut in_tbl_grid = false;

    // Table properties state
    let mut in_tc_pr = false;
    let mut in_tr_pr = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();

                match name {
                    b"tblGrid" => {
                        in_tbl_grid = true;
                    }
                    b"tr" => {
                        in_row = true;
                        is_header_row = false;
                        current_cells.clear();
                    }
                    b"trPr" if in_row => {
                        in_tr_pr = true;
                    }
                    b"tc" if in_row => {
                        in_cell = true;
                        cell_depth = 1;
                        cell_xml.clear();
                        cell_colspan = 1;
                        cell_vmerge_continue = false;
                        in_tc_pr = false;
                    }
                    b"tcPr" if in_cell => {
                        in_tc_pr = true;
                    }
                    _ => {
                        if in_cell && !in_tc_pr {
                            cell_depth += 1;
                            append_start_tag(&mut cell_xml, e);
                        }
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();

                match name {
                    b"gridCol" if in_tbl_grid => {
                        col_count += 1;
                    }
                    b"tblHeader" if in_tr_pr => {
                        is_header_row = true;
                    }
                    b"gridSpan" if in_tc_pr => {
                        if let Some(val) = get_val_attr(e) {
                            cell_colspan = val.parse().unwrap_or(1);
                        }
                    }
                    b"vMerge" if in_tc_pr => {
                        // <w:vMerge w:val="restart"/> or <w:vMerge/> (continue)
                        let val = get_val_attr(e);
                        if val.as_deref() != Some("restart") {
                            cell_vmerge_continue = true;
                        }
                    }
                    _ => {
                        if in_cell && !in_tc_pr {
                            append_empty_tag(&mut cell_xml, e);
                        }
                    }
                }
            }
            Ok(Event::Text(ref e)) if in_cell && !in_tc_pr => {
                let text = e
                    .unescape()
                    .map_err(|err| DocxError::Xml(err.to_string()))?;
                cell_xml.push_str(&quick_xml_escape(&text));
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                let name = local.as_ref();

                match name {
                    b"tblGrid" => {
                        in_tbl_grid = false;
                    }
                    b"trPr" => {
                        in_tr_pr = false;
                    }
                    b"tcPr" => {
                        in_tc_pr = false;
                    }
                    b"tc" if in_cell => {
                        // Parse cell content: split into individual paragraphs
                        let content = parse_cell_blocks(&cell_xml, ctx)?;

                        // Handle vMerge (vertical merge)
                        let rowspan = if cell_vmerge_continue {
                            0 // Marker: this cell is merged with the one above
                        } else {
                            1
                        };

                        current_cells.push(TableCell {
                            content,
                            colspan: cell_colspan,
                            rowspan,
                        });

                        in_cell = false;
                    }
                    b"tr" if in_row => {
                        if is_header_row {
                            header_rows.push(std::mem::take(&mut current_cells));
                        } else {
                            rows.push(std::mem::take(&mut current_cells));
                        }
                        in_row = false;
                    }
                    _ => {
                        if in_cell && !in_tc_pr {
                            cell_depth = cell_depth.saturating_sub(1);
                            append_end_tag(&mut cell_xml, e.name().as_ref());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocxError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    // Resolve vMerge: count rowspan for "restart" cells
    resolve_vmerge(&mut header_rows, &mut rows);

    // Build column specs
    let columns: Vec<ColumnSpec> = (0..col_count)
        .map(|_| ColumnSpec {
            alignment: Alignment::Default,
            width: None,
        })
        .collect();

    // Combine header rows (use first row as header if available)
    let header = if !header_rows.is_empty() {
        Some(header_rows.remove(0))
    } else {
        None
    };

    // If there were multiple header rows, prepend extras to body
    if !header_rows.is_empty() {
        let mut combined = header_rows;
        combined.append(&mut rows);
        rows = combined;
    }

    Ok(Block::Table(Table {
        caption: None,
        label: None,
        columns,
        header,
        rows,
        foot: None,
        attrs: None,
    }))
}

/// Resolve vertical merges: for cells with vMerge="restart", count how many
/// continuation cells follow below and set the rowspan accordingly.
/// Remove continuation cells (rowspan=0) from the rows.
fn resolve_vmerge(header_rows: &mut [Vec<TableCell>], body_rows: &mut [Vec<TableCell>]) {
    // Combine all rows for analysis
    let total_headers = header_rows.len();
    let total_rows = total_headers + body_rows.len();

    if total_rows < 2 {
        return;
    }

    // For each column position, walk down and resolve merges
    let max_cols = header_rows
        .iter()
        .chain(body_rows.iter())
        .map(|r| r.len())
        .max()
        .unwrap_or(0);

    for col in 0..max_cols {
        let mut restart_row: Option<usize> = None;

        for row_idx in 0..total_rows {
            let cells = if row_idx < total_headers {
                &header_rows[row_idx]
            } else {
                &body_rows[row_idx - total_headers]
            };

            if col >= cells.len() {
                continue;
            }

            if cells[col].rowspan == 0 {
                // Continuation cell
                if let Some(start) = restart_row {
                    // Increment the restart cell's rowspan
                    let start_cells = if start < total_headers {
                        &mut header_rows[start]
                    } else {
                        &mut body_rows[start - total_headers]
                    };
                    if col < start_cells.len() {
                        start_cells[col].rowspan += 1;
                    }
                }
            } else {
                restart_row = Some(row_idx);
            }
        }
    }

    // Remove continuation cells (rowspan == 0)
    for row in header_rows.iter_mut() {
        row.retain(|c| c.rowspan > 0);
    }
    for row in body_rows.iter_mut() {
        row.retain(|c| c.rowspan > 0);
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_archive() -> crate::archive::DocxArchive {
        let zip = crate::tests::make_zip(&[("word/document.xml", b"<doc/>")]);
        crate::archive::DocxArchive::from_bytes(&zip).unwrap()
    }

    fn empty_ctx() -> ParseContext<'static> {
        // Leak to get 'static — fine in tests
        let styles: &'static StyleMap = Box::leak(Box::new(StyleMap::new()));
        let numbering: &'static NumberingMap = Box::leak(Box::new(NumberingMap::new()));
        let rels: &'static RelMap = Box::leak(Box::new(RelMap::new()));
        let archive: &'static crate::archive::DocxArchive = Box::leak(Box::new(empty_archive()));
        ParseContext {
            styles,
            numbering,
            rels,
            archive,
        }
    }

    fn wrap_p(inner: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><w:p xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">{inner}</w:p>"#
        )
    }

    // ── Inline run tests ────────────────────────────────────────────────────

    #[test]
    fn parse_plain_text_run() {
        let ctx = empty_ctx();
        let xml = wrap_p(r#"<w:r><w:t>Hello</w:t></w:r>"#);
        let inlines = parse_runs(&xml, &ctx).unwrap();
        assert_eq!(inlines.len(), 1);
        if let Inline::Text { value } = &inlines[0] {
            assert_eq!(value, "Hello");
        } else {
            panic!("expected Text, got {:?}", inlines[0]);
        }
    }

    #[test]
    fn parse_bold_run() {
        let ctx = empty_ctx();
        let xml = wrap_p(r#"<w:r><w:rPr><w:b/></w:rPr><w:t>Bold</w:t></w:r>"#);
        let inlines = parse_runs(&xml, &ctx).unwrap();
        assert_eq!(inlines.len(), 1);
        if let Inline::Strong { content } = &inlines[0] {
            assert_eq!(content.len(), 1);
            if let Inline::Text { value } = &content[0] {
                assert_eq!(value, "Bold");
            } else {
                panic!("expected Text inside Strong");
            }
        } else {
            panic!("expected Strong, got {:?}", inlines[0]);
        }
    }

    #[test]
    fn parse_bold_italic_run() {
        let ctx = empty_ctx();
        let xml = wrap_p(r#"<w:r><w:rPr><w:b/><w:i/></w:rPr><w:t>Both</w:t></w:r>"#);
        let inlines = parse_runs(&xml, &ctx).unwrap();
        assert_eq!(inlines.len(), 1);
        // Should be Strong(Emphasis(Text))
        if let Inline::Strong { content } = &inlines[0] {
            assert_eq!(content.len(), 1);
            if let Inline::Emphasis { content } = &content[0] {
                assert_eq!(content.len(), 1);
                if let Inline::Text { value } = &content[0] {
                    assert_eq!(value, "Both");
                } else {
                    panic!("expected Text inside Emphasis");
                }
            } else {
                panic!("expected Emphasis inside Strong");
            }
        } else {
            panic!("expected Strong, got {:?}", inlines[0]);
        }
    }

    #[test]
    fn parse_code_font_run() {
        let ctx = empty_ctx();
        let xml = wrap_p(
            r#"<w:r><w:rPr><w:rFonts w:ascii="Courier New"/></w:rPr><w:t>code()</w:t></w:r>"#,
        );
        let inlines = parse_runs(&xml, &ctx).unwrap();
        assert_eq!(inlines.len(), 1);
        if let Inline::Code { value, .. } = &inlines[0] {
            assert_eq!(value, "code()");
        } else {
            panic!("expected Code, got {:?}", inlines[0]);
        }
    }

    #[test]
    fn parse_hyperlink() {
        use crate::relationships::Relationship;

        let mut rels = RelMap::new();
        rels.insert(
            "rId1".to_string(),
            Relationship {
                rel_type:
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink"
                        .to_string(),
                target: "https://example.com".to_string(),
                target_mode: Some("External".to_string()),
            },
        );

        let styles = StyleMap::new();
        let numbering = NumberingMap::new();
        let archive = empty_archive();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            archive: &archive,
        };

        let xml =
            wrap_p(r#"<w:hyperlink r:id="rId1"><w:r><w:t>Click here</w:t></w:r></w:hyperlink>"#);
        let inlines = parse_runs(&xml, &ctx).unwrap();
        assert_eq!(inlines.len(), 1);
        if let Inline::Link { url, content, .. } = &inlines[0] {
            assert_eq!(url, "https://example.com");
            assert_eq!(content.len(), 1);
            if let Inline::Text { value } = &content[0] {
                assert_eq!(value, "Click here");
            } else {
                panic!("expected Text inside Link");
            }
        } else {
            panic!("expected Link, got {:?}", inlines[0]);
        }
    }

    #[test]
    fn parse_hard_break() {
        let ctx = empty_ctx();
        let xml = wrap_p(r#"<w:r><w:t>before</w:t><w:br/><w:t>after</w:t></w:r>"#);
        let inlines = parse_runs(&xml, &ctx).unwrap();
        assert_eq!(inlines.len(), 3);
        assert!(matches!(&inlines[0], Inline::Text { value } if value == "before"));
        assert!(matches!(&inlines[1], Inline::HardBreak));
        assert!(matches!(&inlines[2], Inline::Text { value } if value == "after"));
    }

    #[test]
    fn parse_footnote_reference() {
        let ctx = empty_ctx();
        let xml = wrap_p(r#"<w:r><w:footnoteReference w:id="2"/></w:r>"#);
        let inlines = parse_runs(&xml, &ctx).unwrap();
        assert_eq!(inlines.len(), 1);
        if let Inline::FootnoteRef { id } = &inlines[0] {
            assert_eq!(id, "2");
        } else {
            panic!("expected FootnoteRef, got {:?}", inlines[0]);
        }
    }

    #[test]
    fn is_monospace_detects_common_fonts() {
        assert!(is_monospace_font("Courier New"));
        assert!(is_monospace_font("courier new")); // case insensitive
        assert!(is_monospace_font("Consolas"));
        assert!(is_monospace_font("Menlo"));
        assert!(is_monospace_font("Monaco"));
        assert!(is_monospace_font("Fira Code"));
        assert!(is_monospace_font("JetBrains Mono"));
        assert!(!is_monospace_font("Arial"));
        assert!(!is_monospace_font("Times New Roman"));
        assert!(!is_monospace_font("Calibri"));
    }

    // ── Block tests ─────────────────────────────────────────────────────────

    #[test]
    fn parse_heading_paragraph() {
        use crate::styles::{StyleInfo, StyleKind};

        let mut styles = StyleMap::new();
        styles.insert(
            "Heading1".to_string(),
            StyleInfo {
                style_id: "Heading1".to_string(),
                name: "heading 1".to_string(),
                based_on: None,
                style_type: "paragraph".to_string(),
                kind: StyleKind::Heading(1),
            },
        );
        let numbering = NumberingMap::new();
        let rels = RelMap::new();
        let archive = empty_archive();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            archive: &archive,
        };

        let xml = wrap_p(
            r#"<w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Introduction</w:t></w:r>"#,
        );
        let block = parse_paragraph(&xml, &ctx).unwrap().unwrap();
        if let Block::Heading { level, content, .. } = &block {
            assert_eq!(*level, 1);
            assert_eq!(content.len(), 1);
        } else {
            panic!("expected Heading, got {:?}", block);
        }
    }

    #[test]
    fn parse_thematic_break() {
        let styles = StyleMap::new();
        let numbering = NumberingMap::new();
        let rels = RelMap::new();
        let archive = empty_archive();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            archive: &archive,
        };

        let xml = wrap_p(r#"<w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6"/></w:pBdr></w:pPr>"#);
        let block = parse_paragraph(&xml, &ctx).unwrap().unwrap();
        assert!(matches!(block, Block::ThematicBreak));
    }

    #[test]
    fn parse_code_block_paragraph() {
        use crate::styles::{StyleInfo, StyleKind};

        let mut styles = StyleMap::new();
        styles.insert(
            "CodeBlock".to_string(),
            StyleInfo {
                style_id: "CodeBlock".to_string(),
                name: "Code Block".to_string(),
                based_on: None,
                style_type: "paragraph".to_string(),
                kind: StyleKind::CodeBlock,
            },
        );
        let numbering = NumberingMap::new();
        let rels = RelMap::new();
        let archive = empty_archive();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            archive: &archive,
        };

        let xml = wrap_p(
            r#"<w:pPr><w:pStyle w:val="CodeBlock"/></w:pPr><w:r><w:t>fn main() {}</w:t></w:r>"#,
        );
        let block = parse_paragraph(&xml, &ctx).unwrap().unwrap();
        if let Block::CodeBlock { content, .. } = &block {
            assert_eq!(content, "fn main() {}");
        } else {
            panic!("expected CodeBlock, got {:?}", block);
        }
    }

    #[test]
    fn parse_simple_table() {
        let styles = StyleMap::new();
        let numbering = NumberingMap::new();
        let rels = RelMap::new();
        let archive = empty_archive();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            archive: &archive,
        };

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:tbl xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
               xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
            <w:tblGrid>
                <w:gridCol w:w="4000"/>
                <w:gridCol w:w="4000"/>
            </w:tblGrid>
            <w:tr>
                <w:trPr><w:tblHeader/></w:trPr>
                <w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>Value</w:t></w:r></w:p></w:tc>
            </w:tr>
            <w:tr>
                <w:tc><w:p><w:r><w:t>Pi</w:t></w:r></w:p></w:tc>
                <w:tc><w:p><w:r><w:t>3.14</w:t></w:r></w:p></w:tc>
            </w:tr>
        </w:tbl>"#;

        let block = parse_table(xml, &ctx).unwrap();
        if let Block::Table(table) = &block {
            assert_eq!(table.columns.len(), 2);
            assert!(table.header.is_some());
            let header = table.header.as_ref().unwrap();
            assert_eq!(header.len(), 2);
            assert_eq!(table.rows.len(), 1);
            assert_eq!(table.rows[0].len(), 2);
        } else {
            panic!("expected Table, got {:?}", block);
        }
    }

    #[test]
    fn parse_table_multi_paragraph_cell() {
        let styles = StyleMap::new();
        let numbering = NumberingMap::new();
        let rels = RelMap::new();
        let archive = empty_archive();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            archive: &archive,
        };

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:tbl xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
               xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
            <w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid>
            <w:tr>
                <w:tc>
                    <w:p><w:r><w:t>First paragraph</w:t></w:r></w:p>
                    <w:p><w:r><w:t>Second paragraph</w:t></w:r></w:p>
                    <w:p><w:r><w:t>Third paragraph</w:t></w:r></w:p>
                </w:tc>
            </w:tr>
        </w:tbl>"#;

        let block = parse_table(xml, &ctx).unwrap();
        if let Block::Table(table) = &block {
            assert_eq!(table.rows.len(), 1);
            let cell = &table.rows[0][0];
            assert_eq!(
                cell.content.len(),
                3,
                "cell should have 3 separate paragraphs"
            );
            for block in &cell.content {
                assert!(
                    matches!(block, Block::Paragraph { .. }),
                    "each block should be a Paragraph"
                );
            }
        } else {
            panic!("expected Table, got {:?}", block);
        }
    }

    #[test]
    fn parse_table_empty_paragraph_cell() {
        let styles = StyleMap::new();
        let numbering = NumberingMap::new();
        let rels = RelMap::new();
        let archive = empty_archive();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            archive: &archive,
        };

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:tbl xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
               xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
            <w:tblGrid><w:gridCol w:w="4000"/></w:tblGrid>
            <w:tr>
                <w:tc>
                    <w:p></w:p>
                </w:tc>
            </w:tr>
        </w:tbl>"#;

        let block = parse_table(xml, &ctx).unwrap();
        if let Block::Table(table) = &block {
            let cell = &table.rows[0][0];
            assert!(
                cell.content.is_empty(),
                "empty paragraph should produce no blocks"
            );
        } else {
            panic!("expected Table, got {:?}", block);
        }
    }

    #[test]
    fn parse_body_with_paragraph() {
        let styles = StyleMap::new();
        let numbering = NumberingMap::new();
        let rels = RelMap::new();
        let archive = empty_archive();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            archive: &archive,
        };

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                    xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
            <w:body>
                <w:p><w:r><w:t>Hello world</w:t></w:r></w:p>
            </w:body>
        </w:document>"#;

        let blocks = parse_body(xml, &ctx).unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(matches!(&blocks[0], Block::Paragraph { .. }));
    }

    #[test]
    fn parse_empty_body() {
        let styles = StyleMap::new();
        let numbering = NumberingMap::new();
        let rels = RelMap::new();
        let archive = empty_archive();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            archive: &archive,
        };

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
            <w:body/>
        </w:document>"#;

        let blocks = parse_body(xml, &ctx).unwrap();
        assert!(blocks.is_empty());
    }

    #[test]
    fn extract_plain_text_nested() {
        let inlines = vec![
            Inline::Strong {
                content: vec![Inline::Text {
                    value: "bold ".to_string(),
                }],
            },
            Inline::Text {
                value: "normal".to_string(),
            },
        ];
        assert_eq!(extract_plain_text(&inlines), "bold normal");
    }

    // ── Drawing tests ───────────────────────────────────────────────────────

    #[test]
    fn parse_drawing_inline() {
        use crate::relationships::Relationship;

        let mut rels = RelMap::new();
        rels.insert(
            "rId5".to_string(),
            Relationship {
                rel_type:
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
                        .to_string(),
                target: "media/image1.png".to_string(),
                target_mode: None,
            },
        );

        let styles = StyleMap::new();
        let numbering = NumberingMap::new();
        let archive = empty_archive();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            archive: &archive,
        };

        let xml = wrap_p(
            r#"<w:r><w:drawing>
  <wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
             xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
    <wp:docPr id="1" name="My Logo"/>
    <a:graphic>
      <a:graphicData>
        <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
          <pic:blipFill>
            <a:blip r:embed="rId5"/>
          </pic:blipFill>
        </pic:pic>
      </a:graphicData>
    </a:graphic>
  </wp:inline>
</w:drawing></w:r>"#,
        );

        let inlines = parse_runs(&xml, &ctx).unwrap();
        assert_eq!(inlines.len(), 1, "expected exactly one inline");
        if let Inline::Image(img) = &inlines[0] {
            assert_eq!(img.url, "media/image1.png");
            assert_eq!(img.alt_text(), "My Logo");
        } else {
            panic!("expected Image inline, got {:?}", inlines[0]);
        }
    }

    #[test]
    fn parse_drawing_anchor() {
        use crate::relationships::Relationship;

        let mut rels = RelMap::new();
        rels.insert(
            "rId5".to_string(),
            Relationship {
                rel_type:
                    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
                        .to_string(),
                target: "media/image1.png".to_string(),
                target_mode: None,
            },
        );

        let styles = StyleMap::new();
        let numbering = NumberingMap::new();
        let archive = empty_archive();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
            archive: &archive,
        };

        let xml = wrap_p(
            r#"<w:r><w:drawing>
  <wp:anchor xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
             xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
             xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
    <wp:docPr id="2" name="My Logo"/>
    <a:graphic>
      <a:graphicData>
        <pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
          <pic:blipFill>
            <a:blip r:embed="rId5"/>
          </pic:blipFill>
        </pic:pic>
      </a:graphicData>
    </a:graphic>
  </wp:anchor>
</w:drawing></w:r>"#,
        );

        let inlines = parse_runs(&xml, &ctx).unwrap();
        assert_eq!(inlines.len(), 1, "expected exactly one inline");
        if let Inline::Image(img) = &inlines[0] {
            assert_eq!(img.url, "media/image1.png");
            assert_eq!(img.alt_text(), "My Logo");
        } else {
            panic!("expected Image inline, got {:?}", inlines[0]);
        }
    }
}
