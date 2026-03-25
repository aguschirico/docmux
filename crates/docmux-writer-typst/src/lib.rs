//! # docmux-writer-typst
//!
//! Typst writer for docmux. Converts the docmux AST into Typst markup.

use std::collections::HashMap;

use docmux_ast::*;
use docmux_core::{Result, WriteOptions, Writer};

/// Collect all footnote definitions from a block list into a lookup map.
fn collect_footnotes(blocks: &[Block]) -> HashMap<String, Vec<Block>> {
    let mut map = HashMap::new();
    for block in blocks {
        if let Block::FootnoteDef { id, content } = block {
            map.insert(id.clone(), content.clone());
        }
    }
    map
}

/// A Typst writer.
#[derive(Debug, Default)]
pub struct TypstWriter;

impl TypstWriter {
    pub fn new() -> Self {
        Self
    }

    fn write_blocks_impl(
        &self,
        blocks: &[Block],
        opts: &WriteOptions,
        out: &mut String,
        footnotes: &HashMap<String, Vec<Block>>,
    ) {
        for block in blocks {
            self.write_block_impl(block, opts, out, footnotes);
        }
    }

    fn write_block_impl(
        &self,
        block: &Block,
        opts: &WriteOptions,
        out: &mut String,
        footnotes: &HashMap<String, Vec<Block>>,
    ) {
        match block {
            Block::Paragraph { content } => {
                self.write_inlines_impl(content, opts, out, footnotes);
                out.push_str("\n\n");
            }
            Block::Heading {
                level, id, content, ..
            } => {
                for _ in 0..*level {
                    out.push('=');
                }
                out.push(' ');
                self.write_inlines_impl(content, opts, out, footnotes);
                if let Some(id) = id {
                    out.push_str(&format!(" <{}>", id));
                }
                out.push('\n');
            }
            Block::CodeBlock {
                language,
                content,
                caption,
                label,
                ..
            } => {
                if caption.is_some() || label.is_some() {
                    out.push_str("#figure(\n");
                    if let Some(cap) = caption {
                        out.push_str("  caption: [");
                        self.write_inlines_impl(cap, opts, out, footnotes);
                        out.push_str("],\n");
                    }
                    out.push_str(")[\n");
                }
                out.push_str("```");
                if let Some(lang) = language {
                    out.push_str(lang);
                }
                out.push('\n');
                out.push_str(content);
                if !content.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str("```\n");
                if caption.is_some() || label.is_some() {
                    out.push(']');
                    if let Some(label) = label {
                        out.push_str(&format!(" <{}>", label));
                    }
                    out.push('\n');
                }
            }
            Block::MathBlock { content, label } => {
                out.push_str("$\n");
                out.push_str(content.trim());
                out.push_str("\n$");
                if let Some(label) = label {
                    out.push_str(&format!(" <{}>", label));
                }
                out.push('\n');
            }
            Block::BlockQuote { content } => {
                out.push_str("#quote(block: true)[\n");
                self.write_blocks_impl(content, opts, out, footnotes);
                out.push_str("]\n");
            }
            Block::List {
                ordered,
                start,
                items,
                ..
            } => {
                if *ordered {
                    if let Some(s) = start {
                        if *s != 1 {
                            out.push_str(&format!("#set enum(start: {})\n", s));
                        }
                    }
                }
                let marker = if *ordered { "+ " } else { "- " };
                for item in items {
                    if let Some(checked) = item.checked {
                        let checkbox = if checked { "\u{2611} " } else { "\u{2610} " };
                        out.push_str(marker);
                        out.push_str(checkbox);
                    } else {
                        out.push_str(marker);
                    }
                    let mut item_content = String::new();
                    self.write_blocks_impl(&item.content, opts, &mut item_content, footnotes);
                    out.push_str(item_content.trim());
                    out.push('\n');
                }
            }
            Block::Table(table) => {
                self.write_table(table, opts, out, footnotes);
            }
            Block::Figure {
                image,
                caption,
                label,
                ..
            } => {
                out.push_str("#figure(\n");
                out.push_str(&format!("  image(\"{}\"),\n", escape_typst_url(&image.url)));
                if let Some(cap) = caption {
                    out.push_str("  caption: [");
                    self.write_inlines_impl(cap, opts, out, footnotes);
                    out.push_str("],\n");
                }
                out.push(')');
                if let Some(label) = label {
                    out.push_str(&format!(" <{}>", label));
                }
                out.push('\n');
            }
            Block::ThematicBreak => {
                out.push_str("#line(length: 100%)\n");
            }
            Block::RawBlock { format, content } => {
                if format == "typst" || format == "typ" {
                    out.push_str(content);
                    if !content.ends_with('\n') {
                        out.push('\n');
                    }
                }
            }
            Block::Admonition {
                kind,
                title,
                content,
            } => {
                let label = match kind {
                    AdmonitionKind::Note => "Note",
                    AdmonitionKind::Warning => "Warning",
                    AdmonitionKind::Tip => "Tip",
                    AdmonitionKind::Important => "Important",
                    AdmonitionKind::Caution => "Caution",
                    AdmonitionKind::Custom(c) => c.as_str(),
                };
                out.push_str("#block(inset: 1em, stroke: 0.5pt)[\n");
                if let Some(t) = title {
                    out.push('*');
                    self.write_inlines_impl(t, opts, out, footnotes);
                    out.push_str("*\n\n");
                } else {
                    out.push_str(&format!("*{}*\n\n", label));
                }
                self.write_blocks_impl(content, opts, out, footnotes);
                out.push_str("]\n");
            }
            Block::DefinitionList { items } => {
                for item in items {
                    for def in &item.definitions {
                        out.push_str("/ ");
                        self.write_inlines_impl(&item.term, opts, out, footnotes);
                        out.push_str(": ");
                        let mut def_content = String::new();
                        self.write_blocks_impl(def, opts, &mut def_content, footnotes);
                        out.push_str(def_content.trim());
                        out.push('\n');
                    }
                }
            }
            Block::Div { content, .. } => {
                self.write_blocks_impl(content, opts, out, footnotes);
            }
            Block::FootnoteDef { .. } => {
                // Consumed by footnote pre-pass; skip
            }
        }
    }

    fn write_inlines_impl(
        &self,
        inlines: &[Inline],
        opts: &WriteOptions,
        out: &mut String,
        footnotes: &HashMap<String, Vec<Block>>,
    ) {
        for inline in inlines {
            self.write_inline_impl(inline, opts, out, footnotes);
        }
    }

    fn write_inline_impl(
        &self,
        inline: &Inline,
        opts: &WriteOptions,
        out: &mut String,
        footnotes: &HashMap<String, Vec<Block>>,
    ) {
        match inline {
            Inline::Text { value } => {
                out.push_str(&escape_typst(value));
            }
            Inline::SoftBreak => {
                out.push('\n');
            }
            Inline::HardBreak => {
                out.push_str("\\ \n");
            }
            Inline::Emphasis { content } => {
                out.push('_');
                self.write_inlines_impl(content, opts, out, footnotes);
                out.push('_');
            }
            Inline::Strong { content } => {
                out.push('*');
                self.write_inlines_impl(content, opts, out, footnotes);
                out.push('*');
            }
            Inline::Strikethrough { content } => {
                out.push_str("#strike[");
                self.write_inlines_impl(content, opts, out, footnotes);
                out.push(']');
            }
            Inline::Code { value } => {
                out.push('`');
                out.push_str(value);
                out.push('`');
            }
            Inline::MathInline { value } => {
                out.push('$');
                out.push_str(value);
                out.push('$');
            }
            Inline::Link { url, content, .. } => {
                out.push_str(&format!("#link(\"{}\")", escape_typst_url(url)));
                if !content.is_empty() {
                    out.push('[');
                    self.write_inlines_impl(content, opts, out, footnotes);
                    out.push(']');
                }
            }
            Inline::Image(img) => {
                out.push_str(&format!("#image(\"{}\"", escape_typst_url(&img.url)));
                if !img.alt.is_empty() {
                    out.push_str(&format!(", alt: \"{}\"", escape_typst_url(&img.alt)));
                }
                out.push(')');
            }
            Inline::Superscript { content } => {
                out.push_str("#super[");
                self.write_inlines_impl(content, opts, out, footnotes);
                out.push(']');
            }
            Inline::Subscript { content } => {
                out.push_str("#sub[");
                self.write_inlines_impl(content, opts, out, footnotes);
                out.push(']');
            }
            Inline::SmallCaps { content } => {
                out.push_str("#smallcaps[");
                self.write_inlines_impl(content, opts, out, footnotes);
                out.push(']');
            }
            Inline::Underline { content } => {
                out.push_str("#underline[");
                self.write_inlines_impl(content, opts, out, footnotes);
                out.push(']');
            }
            Inline::Span { content, .. } => {
                self.write_inlines_impl(content, opts, out, footnotes);
            }
            Inline::RawInline { format, content } => {
                if format == "typst" || format == "typ" {
                    out.push_str(content);
                }
            }
            Inline::Citation(cite) => {
                if let Some(prefix) = &cite.prefix {
                    out.push_str(&escape_typst(prefix));
                    out.push(' ');
                }
                for (i, key) in cite.keys.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    out.push('@');
                    out.push_str(key);
                }
                if let Some(suffix) = &cite.suffix {
                    out.push(' ');
                    out.push_str(&escape_typst(suffix));
                }
            }
            Inline::CrossRef(cr) => {
                out.push('@');
                out.push_str(&cr.target);
            }
            Inline::FootnoteRef { id } => {
                if let Some(content) = footnotes.get(id) {
                    out.push_str("#footnote[");
                    let mut inner = String::new();
                    self.write_blocks_impl(content, opts, &mut inner, footnotes);
                    out.push_str(inner.trim());
                    out.push(']');
                }
            }
        }
    }

    // ── Table helper ─────────────────────────────────────────────────────

    fn write_table(
        &self,
        table: &Table,
        opts: &WriteOptions,
        out: &mut String,
        footnotes: &HashMap<String, Vec<Block>>,
    ) {
        let has_wrapper = table.caption.is_some() || table.label.is_some();
        if has_wrapper {
            out.push_str("#figure(\n");
            if let Some(cap) = &table.caption {
                out.push_str("  caption: [");
                self.write_inlines_impl(cap, opts, out, footnotes);
                out.push_str("],\n");
            }
        }
        let ncols = table.columns.len().max(
            table
                .header
                .as_ref()
                .map(|h| h.len())
                .or_else(|| table.rows.first().map(|r| r.len()))
                .unwrap_or(1),
        );
        out.push_str(&format!("#table(\n  columns: {},\n", ncols));

        if table
            .columns
            .iter()
            .any(|c| !matches!(c.alignment, Alignment::Default | Alignment::Left))
        {
            out.push_str("  align: (");
            for (i, col) in table.columns.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                match col.alignment {
                    Alignment::Left | Alignment::Default => out.push_str("left"),
                    Alignment::Center => out.push_str("center"),
                    Alignment::Right => out.push_str("right"),
                }
            }
            out.push_str("),\n");
        }

        if let Some(header) = &table.header {
            out.push_str("  table.header(\n");
            for cell in header {
                out.push_str("    [");
                let mut cell_content = String::new();
                self.write_blocks_impl(&cell.content, opts, &mut cell_content, footnotes);
                out.push_str(cell_content.trim());
                out.push_str("],\n");
            }
            out.push_str("  ),\n");
        }

        for row in &table.rows {
            for cell in row {
                out.push_str("  [");
                let mut cell_content = String::new();
                self.write_blocks_impl(&cell.content, opts, &mut cell_content, footnotes);
                out.push_str(cell_content.trim());
                out.push_str("],\n");
            }
        }

        out.push(')');
        if has_wrapper {
            out.push(')');
            if let Some(label) = &table.label {
                out.push_str(&format!(" <{}>", label));
            }
        }
        out.push('\n');
    }

    fn wrap_standalone(&self, body: &str, doc: &Document) -> String {
        let mut preamble = String::with_capacity(512);

        let has_meta = doc.metadata.title.is_some()
            || !doc.metadata.authors.is_empty()
            || doc.metadata.date.is_some();

        if has_meta {
            preamble.push_str("#set document(\n");
            if let Some(title) = &doc.metadata.title {
                preamble.push_str(&format!("  title: \"{}\",\n", escape_typst_url(title)));
            }
            if !doc.metadata.authors.is_empty() {
                let names: Vec<String> = doc
                    .metadata
                    .authors
                    .iter()
                    .map(|a| format!("\"{}\"", escape_typst_url(&a.name)))
                    .collect();
                if names.len() == 1 {
                    preamble.push_str(&format!("  author: {},\n", names[0]));
                } else {
                    preamble.push_str(&format!("  author: ({}),\n", names.join(", ")));
                }
            }
            if let Some(date) = &doc.metadata.date {
                let parts: Vec<&str> = date.split('-').collect();
                if parts.len() == 3
                    && parts[0].parse::<u32>().is_ok()
                    && parts[1].parse::<u32>().is_ok()
                    && parts[2].parse::<u32>().is_ok()
                {
                    preamble.push_str(&format!(
                        "  date: datetime(year: {}, month: {}, day: {}),\n",
                        parts[0], parts[1], parts[2]
                    ));
                } else {
                    preamble.push_str(&format!("  date: \"{}\",\n", escape_typst_url(date)));
                }
            }
            preamble.push_str(")\n\n");
        }

        if let Some(abstract_text) = &doc.metadata.abstract_text {
            preamble.push_str("#quote(block: true)[\n");
            preamble.push_str(&escape_typst(abstract_text));
            preamble.push_str("\n]\n\n");
        }

        preamble.push_str(body);
        preamble
    }
}

impl Writer for TypstWriter {
    fn format(&self) -> &str {
        "typst"
    }

    fn default_extension(&self) -> &str {
        "typ"
    }

    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
        let footnotes = collect_footnotes(&doc.content);
        let mut body = String::with_capacity(4096);
        self.write_blocks_impl(&doc.content, opts, &mut body, &footnotes);

        if opts.standalone {
            Ok(self.wrap_standalone(&body, doc))
        } else {
            Ok(body)
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Escape Typst special characters: \ * _ ` $ # @ < >
pub fn escape_typst(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '*' | '_' | '`' | '$' | '#' | '@' | '<' | '>' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Escape characters inside a Typst URL string literal: \ and "
pub fn escape_typst_url(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn write_typst(doc: &Document) -> String {
        let writer = TypstWriter::new();
        writer.write(doc, &WriteOptions::default()).unwrap()
    }

    #[test]
    fn paragraph() {
        let doc = Document {
            content: vec![Block::text("Hello, world!")],
            ..Default::default()
        };
        let out = write_typst(&doc);
        assert_eq!(out.trim(), "Hello, world!");
    }

    #[test]
    fn escaping() {
        assert_eq!(escape_typst("hello"), "hello");
        assert_eq!(escape_typst("a*b"), "a\\*b");
        assert_eq!(escape_typst("a_b"), "a\\_b");
        assert_eq!(escape_typst("a`b"), "a\\`b");
        assert_eq!(escape_typst("a$b"), "a\\$b");
        assert_eq!(escape_typst("a#b"), "a\\#b");
        assert_eq!(escape_typst("a@b"), "a\\@b");
        assert_eq!(escape_typst("a<b"), "a\\<b");
        assert_eq!(escape_typst("a>b"), "a\\>b");
        assert_eq!(escape_typst("a\\b"), "a\\\\b");
    }

    #[test]
    fn url_escaping() {
        assert_eq!(
            escape_typst_url("https://example.com"),
            "https://example.com"
        );
        assert_eq!(escape_typst_url("a\\b"), "a\\\\b");
        assert_eq!(escape_typst_url("say \"hi\""), "say \\\"hi\\\"");
        assert_eq!(
            escape_typst_url("path\\to\\\"file\""),
            "path\\\\to\\\\\\\"file\\\""
        );
    }

    #[test]
    fn writer_trait_metadata() {
        let writer = TypstWriter::new();
        assert_eq!(writer.format(), "typst");
        assert_eq!(writer.default_extension(), "typ");
    }

    #[test]
    fn heading_with_label() {
        let doc = Document {
            content: vec![Block::Heading {
                level: 2,
                id: Some("intro".into()),
                content: vec![Inline::text("Introduction")],
                attrs: None,
            }],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert_eq!(typ.trim(), "== Introduction <intro>");
    }

    #[test]
    fn code_block() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "print('hello')".into(),
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains("```python\nprint('hello')\n```"));
    }

    #[test]
    fn math_block_with_label() {
        let doc = Document {
            content: vec![Block::MathBlock {
                content: "E = m c^2".into(),
                label: Some("eq:einstein".into()),
            }],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains("$\nE = m c^2\n$"));
        assert!(typ.contains("<eq:einstein>"));
    }

    #[test]
    fn emphasis_and_strong() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::Emphasis {
                        content: vec![Inline::text("italic")],
                    },
                    Inline::text(" and "),
                    Inline::Strong {
                        content: vec![Inline::text("bold")],
                    },
                ],
            }],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains("_italic_"));
        assert!(typ.contains("*bold*"));
    }

    #[test]
    fn inline_code_and_math() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::Code {
                        value: "x + 1".into(),
                    },
                    Inline::text(" and "),
                    Inline::MathInline {
                        value: "x^2".into(),
                    },
                ],
            }],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains("`x + 1`"));
        assert!(typ.contains("$x^2$"));
    }

    #[test]
    fn link_and_image() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::Link {
                        url: "https://example.com".into(),
                        title: None,
                        content: vec![Inline::text("Example")],
                    },
                    Inline::text(" "),
                    Inline::Image(Image {
                        url: "photo.png".into(),
                        alt: "A photo".into(),
                        title: None,
                    }),
                ],
            }],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains(r#"#link("https://example.com")[Example]"#));
        assert!(typ.contains(r#"#image("photo.png", alt: "A photo")"#));
    }

    #[test]
    fn lists_ordered_unordered() {
        let doc = Document {
            content: vec![
                Block::List {
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
                },
                Block::List {
                    ordered: true,
                    start: None,
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
                },
            ],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains("- Alpha"));
        assert!(typ.contains("- Beta"));
        assert!(typ.contains("+ First"));
        assert!(typ.contains("+ Second"));
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
        let typ = write_typst(&doc);
        assert!(typ.contains("/ Rust: A systems programming language."));
    }

    #[test]
    fn footnote_expansion() {
        let doc = Document {
            content: vec![
                Block::Paragraph {
                    content: vec![
                        Inline::text("See note"),
                        Inline::FootnoteRef { id: "fn1".into() },
                        Inline::text("."),
                    ],
                },
                Block::FootnoteDef {
                    id: "fn1".into(),
                    content: vec![Block::text("This is the footnote.")],
                },
            ],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains("#footnote[This is the footnote.]"));
        // FootnoteDef should not appear as a separate block
        assert!(!typ.contains("fn1"));
    }

    #[test]
    fn standalone_mode() {
        let doc = Document {
            metadata: Metadata {
                title: Some("My Paper".into()),
                authors: vec![Author {
                    name: "Jane Doe".into(),
                    affiliation: Some("MIT".into()),
                    email: None,
                    orcid: None,
                }],
                date: Some("2026-03-25".into()),
                abstract_text: Some("This paper is about things.".into()),
                ..Default::default()
            },
            content: vec![Block::text("Body text.")],
            ..Default::default()
        };
        let writer = TypstWriter::new();
        let opts = WriteOptions {
            standalone: true,
            ..Default::default()
        };
        let typ = writer.write(&doc, &opts).unwrap();
        assert!(typ.contains("#set document("));
        assert!(typ.contains("title: \"My Paper\""));
        assert!(typ.contains("Jane Doe"));
        assert!(typ.contains("Body text."));
    }
}
