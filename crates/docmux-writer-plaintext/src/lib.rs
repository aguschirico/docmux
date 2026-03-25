//! # docmux-writer-plaintext
//!
//! Plain text writer for docmux. Strips all formatting and outputs clean,
//! readable plain text — no markdown syntax, no HTML, no LaTeX.

use docmux_ast::*;
use docmux_core::{Result, WriteOptions, Writer};

/// A plain text writer that strips all formatting.
#[derive(Debug, Default)]
pub struct PlaintextWriter;

impl PlaintextWriter {
    pub fn new() -> Self {
        Self
    }

    fn write_blocks(&self, blocks: &[Block], out: &mut String, indent: &str) {
        for block in blocks {
            self.write_block(block, out, indent);
        }
    }

    fn write_block(&self, block: &Block, out: &mut String, indent: &str) {
        match block {
            Block::Paragraph { content } => {
                out.push_str(indent);
                let mut para = String::new();
                self.write_inlines(content, &mut para);
                // Re-indent continuation lines of the paragraph
                let indented = para.replace('\n', &format!("\n{indent}"));
                out.push_str(&indented);
                out.push_str("\n\n");
            }
            Block::Heading { level, content, .. } => {
                let mut text = String::new();
                self.write_inlines(content, &mut text);
                out.push_str(indent);
                out.push_str(&text);
                out.push('\n');
                // Underline h1 with '=', h2 with '-'
                if *level == 1 {
                    out.push_str(indent);
                    for _ in 0..text.len() {
                        out.push('=');
                    }
                    out.push('\n');
                } else if *level == 2 {
                    out.push_str(indent);
                    for _ in 0..text.len() {
                        out.push('-');
                    }
                    out.push('\n');
                }
                out.push('\n');
            }
            Block::CodeBlock { content, .. } => {
                for line in content.lines() {
                    out.push_str(indent);
                    out.push_str("    ");
                    out.push_str(line);
                    out.push('\n');
                }
                out.push('\n');
            }
            Block::MathBlock { content, .. } => {
                out.push_str(indent);
                out.push_str(content);
                out.push_str("\n\n");
            }
            Block::BlockQuote { content } => {
                // Write the inner content into a temporary buffer, then
                // prefix every non-empty line with "> "
                let mut inner = String::new();
                self.write_blocks(content, &mut inner, "");
                for line in inner.lines() {
                    out.push_str(indent);
                    out.push('>');
                    if !line.is_empty() {
                        out.push(' ');
                        out.push_str(line);
                    }
                    out.push('\n');
                }
                out.push('\n');
            }
            Block::List {
                ordered,
                start,
                items,
                ..
            } => {
                let first_num = start.unwrap_or(1);
                for (i, item) in items.iter().enumerate() {
                    let marker = if *ordered {
                        format!("{}. ", first_num as usize + i)
                    } else {
                        "- ".to_string()
                    };
                    // Render item content
                    let mut item_buf = String::new();
                    // For task-list items, prepend the checkbox marker
                    if let Some(checked) = item.checked {
                        let checkbox = if checked { "[x] " } else { "[ ] " };
                        item_buf.push_str(checkbox);
                    }
                    // Render the item blocks into a temporary buffer with no
                    // extra indent, then we'll indent them below.
                    let mut content_buf = String::new();
                    self.write_blocks(&item.content, &mut content_buf, "");
                    item_buf.push_str(content_buf.trim_end_matches('\n'));

                    // First line gets the marker, continuation lines are
                    // indented by marker length.
                    let continuation_indent = " ".repeat(marker.len());
                    let mut lines = item_buf.lines();
                    if let Some(first) = lines.next() {
                        out.push_str(indent);
                        out.push_str(&marker);
                        out.push_str(first);
                        out.push('\n');
                    }
                    for line in lines {
                        out.push_str(indent);
                        out.push_str(&continuation_indent);
                        out.push_str(line);
                        out.push('\n');
                    }
                }
                out.push('\n');
            }
            Block::Table(table) => {
                self.write_table(table, out, indent);
            }
            Block::Figure { image, caption, .. } => {
                out.push_str(indent);
                out.push_str("[Image: ");
                out.push_str(&image.alt_text());
                out.push(']');
                if let Some(cap) = caption {
                    out.push_str(" — ");
                    self.write_inlines(cap, out);
                }
                out.push_str("\n\n");
            }
            Block::ThematicBreak => {
                out.push_str(indent);
                out.push_str("────────────────────────────────────────\n\n");
            }
            Block::RawBlock { format, content } => {
                // Only pass through content explicitly marked as plain/text.
                if format == "plain" || format == "text" {
                    out.push_str(indent);
                    out.push_str(content);
                    if !content.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push('\n');
                }
                // All other raw formats are silently dropped.
            }
            Block::Admonition {
                kind,
                title,
                content,
            } => {
                let kind_label = match kind {
                    AdmonitionKind::Note => "NOTE",
                    AdmonitionKind::Warning => "WARNING",
                    AdmonitionKind::Tip => "TIP",
                    AdmonitionKind::Important => "IMPORTANT",
                    AdmonitionKind::Caution => "CAUTION",
                    AdmonitionKind::Custom(s) => s.as_str(),
                };
                out.push_str(indent);
                out.push('[');
                out.push_str(kind_label);
                out.push(']');
                if let Some(t) = title {
                    out.push(' ');
                    self.write_inlines(t, out);
                }
                out.push('\n');
                let body_indent = format!("{indent}    ");
                self.write_blocks(content, out, &body_indent);
            }
            Block::DefinitionList { items } => {
                for item in items {
                    out.push_str(indent);
                    self.write_inlines(&item.term, out);
                    out.push('\n');
                    for definition in &item.definitions {
                        let def_indent = format!("{indent}    ");
                        self.write_blocks(definition, out, &def_indent);
                    }
                }
                out.push('\n');
            }
            Block::FootnoteDef { id, content } => {
                // Footnote definitions are rendered inline when referenced;
                // emit them at the end as "[^id]: content"
                out.push_str(indent);
                out.push_str(&format!("[^{id}]: "));
                let mut fn_buf = String::new();
                self.write_blocks(content, &mut fn_buf, "");
                out.push_str(fn_buf.trim());
                out.push_str("\n\n");
            }
            Block::Div { content, .. } => {
                // Transparent container — just render the inner blocks
                self.write_blocks(content, out, indent);
            }
        }
    }

    fn write_table(&self, table: &Table, out: &mut String, indent: &str) {
        // Collect all cell texts so we can compute column widths.
        let ncols = table.columns.len().max(
            table
                .header
                .as_ref()
                .map(|h| h.len())
                .unwrap_or(0)
                .max(table.rows.iter().map(|r| r.len()).max().unwrap_or(0)),
        );

        if ncols == 0 {
            return;
        }

        // Render every cell to plain text.
        let render_cell = |cell: &TableCell| -> String {
            let mut buf = String::new();
            for block in &cell.content {
                self.write_block(block, &mut buf, "");
            }
            buf.trim().replace('\n', " ")
        };

        let header_texts: Vec<String> = table
            .header
            .as_ref()
            .map(|row| {
                let mut texts: Vec<String> = row.iter().map(render_cell).collect();
                while texts.len() < ncols {
                    texts.push(String::new());
                }
                texts
            })
            .unwrap_or_else(|| vec![String::new(); ncols]);

        let body_texts: Vec<Vec<String>> = table
            .rows
            .iter()
            .map(|row| {
                let mut texts: Vec<String> = row.iter().map(render_cell).collect();
                while texts.len() < ncols {
                    texts.push(String::new());
                }
                texts
            })
            .collect();

        let foot_texts: Option<Vec<String>> = table.foot.as_ref().map(|row| {
            let mut texts: Vec<String> = row.iter().map(render_cell).collect();
            while texts.len() < ncols {
                texts.push(String::new());
            }
            texts
        });

        // Compute column widths.
        let mut col_widths: Vec<usize> = (0..ncols)
            .map(|i| {
                let header_w = header_texts.get(i).map(|s| s.len()).unwrap_or(0);
                let body_w = body_texts
                    .iter()
                    .map(|row| row.get(i).map(|s| s.len()).unwrap_or(0))
                    .max()
                    .unwrap_or(0);
                let foot_w = foot_texts
                    .as_ref()
                    .and_then(|f| f.get(i))
                    .map(|s| s.len())
                    .unwrap_or(0);
                header_w.max(body_w).max(foot_w).max(1)
            })
            .collect();

        // Ensure minimum width of 3 for aesthetics.
        for w in &mut col_widths {
            *w = (*w).max(3);
        }

        let separator = |dash: char| -> String {
            let mut s = indent.to_string();
            s.push('+');
            for w in &col_widths {
                for _ in 0..*w + 2 {
                    s.push(dash);
                }
                s.push('+');
            }
            s.push('\n');
            s
        };

        let render_row = |texts: &[String]| -> String {
            let mut s = indent.to_string();
            s.push('|');
            for (i, w) in col_widths.iter().enumerate() {
                let cell = texts.get(i).map(|t| t.as_str()).unwrap_or("");
                s.push(' ');
                s.push_str(cell);
                for _ in cell.len()..*w {
                    s.push(' ');
                }
                s.push(' ');
                s.push('|');
            }
            s.push('\n');
            s
        };

        if let Some(cap) = &table.caption {
            out.push_str(indent);
            self.write_inlines(cap, out);
            out.push('\n');
        }

        out.push_str(&separator('-'));

        if table.header.is_some() {
            out.push_str(&render_row(&header_texts));
            out.push_str(&separator('='));
        }

        for row_texts in &body_texts {
            out.push_str(&render_row(row_texts));
            out.push_str(&separator('-'));
        }

        if let Some(foot) = &foot_texts {
            out.push_str(&render_row(foot));
            out.push_str(&separator('='));
        }

        out.push('\n');
    }

    fn write_inlines(&self, inlines: &[Inline], out: &mut String) {
        for inline in inlines {
            self.write_inline(inline, out);
        }
    }

    fn write_inline(&self, inline: &Inline, out: &mut String) {
        match inline {
            Inline::Text { value } => {
                out.push_str(value);
            }
            // All formatting containers: emit content only, drop markup.
            Inline::Emphasis { content }
            | Inline::Strong { content }
            | Inline::Strikethrough { content }
            | Inline::Underline { content }
            | Inline::Superscript { content }
            | Inline::Subscript { content }
            | Inline::SmallCaps { content }
            | Inline::Span { content, .. } => {
                self.write_inlines(content, out);
            }
            Inline::Code { value, .. } => {
                out.push_str(value);
            }
            Inline::MathInline { value } => {
                out.push_str(value);
            }
            Inline::Link { url, content, .. } => {
                let mut link_text = String::new();
                self.write_inlines(content, &mut link_text);
                if link_text.is_empty() || link_text == *url {
                    out.push_str(url);
                } else {
                    out.push_str(&link_text);
                    out.push_str(" (");
                    out.push_str(url);
                    out.push(')');
                }
            }
            Inline::Image(img) => {
                out.push_str("[Image: ");
                out.push_str(&img.alt_text());
                out.push(']');
            }
            Inline::Citation(cite) => {
                out.push_str("[cite: ");
                out.push_str(&cite.keys().join(", "));
                out.push(']');
            }
            Inline::FootnoteRef { id } => {
                out.push_str(&format!("[^{id}]"));
            }
            Inline::CrossRef(cr) => {
                out.push_str(&format!("[ref: {}]", cr.target));
            }
            Inline::RawInline { format, content } => {
                if format == "plain" || format == "text" {
                    out.push_str(content);
                }
                // Other formats are silently dropped.
            }
            Inline::SoftBreak => {
                out.push(' ');
            }
            Inline::HardBreak => {
                out.push('\n');
            }
            Inline::Quoted {
                quote_type,
                content,
            } => {
                let (open, close) = match quote_type {
                    QuoteType::SingleQuote => ('\u{2018}', '\u{2019}'), // ' '
                    QuoteType::DoubleQuote => ('\u{201C}', '\u{201D}'), // " "
                };
                out.push(open);
                self.write_inlines(content, out);
                out.push(close);
            }
        }
    }

    fn write_standalone_header(&self, doc: &Document, out: &mut String) {
        if let Some(title) = &doc.metadata.title {
            out.push_str(title);
            out.push('\n');
            for _ in 0..title.len() {
                out.push('=');
            }
            out.push_str("\n\n");
        }

        if !doc.metadata.authors.is_empty() {
            let names: Vec<&str> = doc
                .metadata
                .authors
                .iter()
                .map(|a| a.name.as_str())
                .collect();
            out.push_str(&names.join(", "));
            out.push_str("\n\n");
        }

        if let Some(date) = &doc.metadata.date {
            out.push_str(date);
            out.push_str("\n\n");
        }

        if let Some(abstract_blocks) = &doc.metadata.abstract_text {
            out.push_str("Abstract\n--------\n\n");
            self.write_blocks(abstract_blocks, out, "");
        }
    }
}

impl Writer for PlaintextWriter {
    fn format(&self) -> &str {
        "plain"
    }

    fn default_extension(&self) -> &str {
        "txt"
    }

    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
        let mut out = String::with_capacity(4096);

        if opts.standalone {
            self.write_standalone_header(doc, &mut out);
        }

        self.write_blocks(&doc.content, &mut out, "");

        // Trim trailing whitespace/newlines from the final output.
        let trimmed = out.trim_end().to_string();
        Ok(if trimmed.is_empty() {
            trimmed
        } else {
            let mut result = trimmed;
            result.push('\n');
            result
        })
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn write_plain(doc: &Document) -> String {
        let writer = PlaintextWriter::new();
        writer.write(doc, &WriteOptions::default()).unwrap()
    }

    fn write_plain_standalone(doc: &Document) -> String {
        let writer = PlaintextWriter::new();
        let opts = WriteOptions {
            standalone: true,
            ..Default::default()
        };
        writer.write(doc, &opts).unwrap()
    }

    // ── Block types ──────────────────────────────────────────────────────────

    #[test]
    fn paragraph() {
        let doc = Document {
            content: vec![Block::text("Hello, world!")],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert_eq!(out, "Hello, world!\n");
    }

    #[test]
    fn heading_h1_underlined() {
        let doc = Document {
            content: vec![Block::heading(1, "Introduction")],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("Introduction"));
        assert!(out.contains("============"));
    }

    #[test]
    fn heading_h2_underlined() {
        let doc = Document {
            content: vec![Block::heading(2, "Background")],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("Background"));
        assert!(out.contains("----------"));
    }

    #[test]
    fn heading_h3_no_underline() {
        let doc = Document {
            content: vec![Block::heading(3, "Details")],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("Details"));
        // No underline characters for h3
        assert!(!out.contains("==="));
        assert!(!out.contains("---"));
    }

    #[test]
    fn code_block() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("rust".into()),
                content: "fn main() {\n    println!(\"hi\");\n}".into(),
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("    fn main() {"));
        assert!(out.contains("        println!(\"hi\");"));
        assert!(out.contains("    }"));
    }

    #[test]
    fn inline_math() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("Formula: "),
                    Inline::MathInline {
                        value: "x^2 + y^2".into(),
                    },
                ],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("Formula: x^2 + y^2"));
    }

    #[test]
    fn display_math() {
        let doc = Document {
            content: vec![Block::MathBlock {
                content: "E = mc^2".into(),
                label: None,
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("E = mc^2"));
    }

    #[test]
    fn emphasis_stripped() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("This is "),
                    Inline::Emphasis {
                        content: vec![Inline::text("important")],
                    },
                    Inline::text("."),
                ],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert_eq!(out, "This is important.\n");
    }

    #[test]
    fn strong_stripped() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("This is "),
                    Inline::Strong {
                        content: vec![Inline::text("bold")],
                    },
                    Inline::text("."),
                ],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert_eq!(out, "This is bold.\n");
    }

    #[test]
    fn link_with_url() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Link {
                    url: "https://example.com".into(),
                    title: None,
                    content: vec![Inline::text("Example")],
                    attrs: None,
                }],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("Example (https://example.com)"));
    }

    #[test]
    fn link_url_only_when_text_equals_url() {
        let url = "https://example.com";
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Link {
                    url: url.into(),
                    title: None,
                    content: vec![Inline::text(url)],
                    attrs: None,
                }],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        // Should not duplicate the URL
        assert!(!out.contains("https://example.com (https://example.com)"));
        assert!(out.contains("https://example.com"));
    }

    #[test]
    fn unordered_list() {
        let doc = Document {
            content: vec![Block::List {
                ordered: false,
                start: None,
                items: vec![
                    ListItem {
                        checked: None,
                        content: vec![Block::text("Alpha")],
                    },
                    ListItem {
                        checked: None,
                        content: vec![Block::text("Beta")],
                    },
                ],
                tight: true,
                style: None,
                delimiter: None,
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("- Alpha"));
        assert!(out.contains("- Beta"));
    }

    #[test]
    fn ordered_list() {
        let doc = Document {
            content: vec![Block::List {
                ordered: true,
                start: Some(1),
                items: vec![
                    ListItem {
                        checked: None,
                        content: vec![Block::text("First")],
                    },
                    ListItem {
                        checked: None,
                        content: vec![Block::text("Second")],
                    },
                ],
                tight: true,
                style: None,
                delimiter: None,
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("1. First"));
        assert!(out.contains("2. Second"));
    }

    #[test]
    fn table() {
        let table = Table {
            caption: None,
            label: None,
            columns: vec![
                ColumnSpec {
                    alignment: Alignment::Left,
                    width: None,
                },
                ColumnSpec {
                    alignment: Alignment::Left,
                    width: None,
                },
            ],
            header: Some(vec![
                TableCell {
                    content: vec![Block::text("Name")],
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: vec![Block::text("Score")],
                    colspan: 1,
                    rowspan: 1,
                },
            ]),
            rows: vec![vec![
                TableCell {
                    content: vec![Block::text("Alice")],
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: vec![Block::text("95")],
                    colspan: 1,
                    rowspan: 1,
                },
            ]],
            foot: None,
            attrs: None,
        };
        let doc = Document {
            content: vec![Block::Table(table)],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("Name"));
        assert!(out.contains("Score"));
        assert!(out.contains("Alice"));
        assert!(out.contains("95"));
        assert!(out.contains('|'));
        assert!(out.contains('+'));
    }

    #[test]
    fn blockquote() {
        let doc = Document {
            content: vec![Block::BlockQuote {
                content: vec![Block::text("To be or not to be.")],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("> To be or not to be."));
    }

    #[test]
    fn image() {
        let doc = Document {
            content: vec![Block::Figure {
                image: Image {
                    url: "photo.jpg".into(),
                    alt: vec![Inline::text("A sunset")],
                    title: None,
                    attrs: None,
                },
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("[Image: A sunset]"));
    }

    #[test]
    fn inline_image() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("See "),
                    Inline::Image(Image {
                        url: "icon.png".into(),
                        alt: vec![Inline::text("icon")],
                        title: None,
                        attrs: None,
                    }),
                    Inline::text(" here."),
                ],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("See [Image: icon] here."));
    }

    #[test]
    fn standalone_mode() {
        let doc = Document {
            metadata: Metadata {
                title: Some("My Paper".into()),
                authors: vec![Author {
                    name: "Jane Doe".into(),
                    affiliation: None,
                    email: None,
                    orcid: None,
                }],
                date: Some("2024-01-15".into()),
                ..Default::default()
            },
            content: vec![Block::text("Body text.")],
            ..Default::default()
        };
        let out = write_plain_standalone(&doc);
        assert!(out.contains("My Paper"));
        // "My Paper" is 8 chars, so the underline is 8 '=' characters
        assert!(out.contains("========"));
        assert!(out.contains("Jane Doe"));
        assert!(out.contains("2024-01-15"));
        assert!(out.contains("Body text."));
    }

    #[test]
    fn writer_trait_metadata() {
        let writer = PlaintextWriter::new();
        assert_eq!(writer.format(), "plain");
        assert_eq!(writer.default_extension(), "txt");
    }

    #[test]
    fn admonition() {
        let doc = Document {
            content: vec![Block::Admonition {
                kind: AdmonitionKind::Warning,
                title: Some(vec![Inline::text("Watch out")]),
                content: vec![Block::text("This is dangerous.")],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("[WARNING] Watch out"));
        assert!(out.contains("This is dangerous."));
    }

    #[test]
    fn definition_list() {
        let doc = Document {
            content: vec![Block::DefinitionList {
                items: vec![DefinitionItem {
                    term: vec![Inline::text("Rust")],
                    definitions: vec![vec![Block::text("A systems programming language.")]],
                }],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("Rust"));
        assert!(out.contains("A systems programming language."));
    }

    #[test]
    fn citation() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("See "),
                    Inline::Citation(Citation {
                        items: vec![
                            CiteItem {
                                key: "smith2020".into(),
                                prefix: None,
                                suffix: None,
                            },
                            CiteItem {
                                key: "jones2021".into(),
                                prefix: None,
                                suffix: None,
                            },
                        ],
                        mode: CitationMode::Normal,
                    }),
                    Inline::text("."),
                ],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("[cite: smith2020, jones2021]"));
    }

    #[test]
    fn cross_ref() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("See "),
                    Inline::CrossRef(CrossRef {
                        target: "fig:result".into(),
                        form: RefForm::Number,
                    }),
                    Inline::text("."),
                ],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("[ref: fig:result]"));
    }

    #[test]
    fn thematic_break() {
        let doc = Document {
            content: vec![
                Block::text("Before."),
                Block::ThematicBreak,
                Block::text("After."),
            ],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("────"));
        assert!(out.contains("Before."));
        assert!(out.contains("After."));
    }

    #[test]
    fn raw_block_plain_passthrough() {
        let doc = Document {
            content: vec![Block::RawBlock {
                format: "plain".into(),
                content: "raw content here".into(),
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("raw content here"));
    }

    #[test]
    fn raw_block_html_dropped() {
        let doc = Document {
            content: vec![Block::RawBlock {
                format: "html".into(),
                content: "<b>should be dropped</b>".into(),
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(!out.contains("<b>"));
        assert!(!out.contains("should be dropped"));
    }

    #[test]
    fn quoted_smart_quotes() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Quoted {
                    quote_type: QuoteType::DoubleQuote,
                    content: vec![Inline::text("hello")],
                }],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains('\u{201C}')); // "
        assert!(out.contains('\u{201D}')); // "
        assert!(out.contains("hello"));
    }

    #[test]
    fn soft_break_becomes_space() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("line one"),
                    Inline::SoftBreak,
                    Inline::text("line two"),
                ],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("line one line two"));
    }

    #[test]
    fn hard_break_becomes_newline() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("line one"),
                    Inline::HardBreak,
                    Inline::text("line two"),
                ],
            }],
            ..Default::default()
        };
        let out = write_plain(&doc);
        assert!(out.contains("line one\nline two"));
    }
}
