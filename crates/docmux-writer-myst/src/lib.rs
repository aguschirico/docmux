//! # docmux-writer-myst
//!
//! MyST Markdown writer for docmux. Converts the docmux AST into
//! [MyST (Markedly Structured Text)](https://mystmd.org/) output,
//! using directives and roles for features beyond CommonMark.

use docmux_ast::*;
use docmux_core::{Result, WriteOptions, Writer};

/// A MyST Markdown writer.
#[derive(Debug, Default)]
pub struct MystWriter;

impl MystWriter {
    pub fn new() -> Self {
        Self
    }

    fn write_blocks(&self, blocks: &[Block], out: &mut String) {
        for (i, block) in blocks.iter().enumerate() {
            if i > 0 && !out.ends_with("\n\n") {
                if out.ends_with('\n') {
                    out.push('\n');
                } else {
                    out.push_str("\n\n");
                }
            }
            self.write_block(block, out);
        }
    }

    fn write_block(&self, block: &Block, out: &mut String) {
        match block {
            Block::Paragraph { content } => {
                self.write_inlines(content, out);
                out.push('\n');
            }
            Block::Heading {
                level, id, content, ..
            } => {
                if let Some(id) = id {
                    out.push_str(&format!("({id})=\n"));
                }
                for _ in 0..*level {
                    out.push('#');
                }
                out.push(' ');
                self.write_inlines(content, out);
                out.push('\n');
            }
            Block::CodeBlock {
                language,
                content,
                caption,
                label,
                ..
            } => {
                let has_directive = caption.is_some() || label.is_some();
                if has_directive {
                    self.write_code_block_directive(language, content, caption, label, out);
                } else {
                    self.write_code_block_fenced(language, content, out);
                }
            }
            Block::MathBlock { content, label } => {
                if let Some(label) = label {
                    out.push_str(&format!("({label})=\n"));
                }
                out.push_str("$$\n");
                out.push_str(content.trim());
                out.push_str("\n$$\n");
            }
            Block::BlockQuote { content } => {
                let mut inner = String::new();
                self.write_blocks(content, &mut inner);
                for line in inner.lines() {
                    if line.is_empty() {
                        out.push_str(">\n");
                    } else {
                        out.push_str(&format!("> {line}\n"));
                    }
                }
            }
            Block::ThematicBreak => {
                out.push_str("---\n");
            }
            Block::List {
                ordered,
                start,
                items,
                tight,
                ..
            } => {
                self.write_list(*ordered, *start, items, *tight, out);
            }
            Block::Table(table) => {
                self.write_table(table, out);
            }
            Block::DefinitionList { items } => {
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push('\n');
                    }
                    self.write_inlines(&item.term, out);
                    out.push('\n');
                    for def in &item.definitions {
                        out.push_str(":   ");
                        let mut def_out = String::new();
                        self.write_blocks(def, &mut def_out);
                        let trimmed = def_out.trim_end();
                        let mut first = true;
                        for line in trimmed.lines() {
                            if first {
                                out.push_str(line);
                                out.push('\n');
                                first = false;
                            } else if line.is_empty() {
                                out.push('\n');
                            } else {
                                out.push_str(&format!("    {line}\n"));
                            }
                        }
                    }
                }
            }
            Block::RawBlock { format, content } => {
                if format == "html" {
                    out.push_str(content);
                    if !content.ends_with('\n') {
                        out.push('\n');
                    }
                } else {
                    out.push_str(&format!("```{format}\n"));
                    out.push_str(content);
                    if !content.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("```\n");
                }
            }
            Block::Admonition {
                kind,
                title,
                content,
            } => {
                let tag = match kind {
                    AdmonitionKind::Note => "note",
                    AdmonitionKind::Warning => "warning",
                    AdmonitionKind::Tip => "tip",
                    AdmonitionKind::Important => "important",
                    AdmonitionKind::Caution => "caution",
                    AdmonitionKind::Custom(c) => c.as_str(),
                };
                out.push_str(&format!(":::{{{tag}}}"));
                if let Some(t) = title {
                    out.push(' ');
                    self.write_inlines(t, out);
                }
                out.push('\n');
                self.write_blocks(content, out);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(":::\n");
            }
            Block::Figure {
                image,
                caption,
                label,
                ..
            } => {
                out.push_str(&format!(":::{{figure}} {}\n", image.url));
                let alt = image.alt_text();
                if !alt.is_empty() {
                    out.push_str(&format!(":alt: {alt}\n"));
                }
                if let Some(label) = label {
                    out.push_str(&format!(":name: {label}\n"));
                }
                if let Some(cap) = caption {
                    out.push('\n');
                    self.write_inlines(cap, out);
                    out.push('\n');
                }
                out.push_str(":::\n");
            }
            Block::FootnoteDef { id, content } => {
                out.push_str(&format!("[^{id}]: "));
                let mut inner = String::new();
                self.write_blocks(content, &mut inner);
                let trimmed = inner.trim_end();
                let mut first = true;
                for line in trimmed.lines() {
                    if first {
                        out.push_str(line);
                        out.push('\n');
                        first = false;
                    } else if line.is_empty() {
                        out.push('\n');
                    } else {
                        out.push_str(&format!("    {line}\n"));
                    }
                }
            }
            Block::Div { attrs, content } => {
                out.push_str(":::");
                let has_attrs =
                    attrs.id.is_some() || !attrs.classes.is_empty() || !attrs.key_values.is_empty();
                if has_attrs {
                    out.push_str(" {");
                    if let Some(id) = &attrs.id {
                        out.push_str(&format!("#{id}"));
                    }
                    for class in &attrs.classes {
                        out.push_str(&format!(" .{class}"));
                    }
                    for (k, v) in &attrs.key_values {
                        out.push_str(&format!(" {k}={v}"));
                    }
                    out.push('}');
                }
                out.push('\n');
                self.write_blocks(content, out);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(":::\n");
            }
        }
    }

    fn write_code_block_fenced(&self, language: &Option<String>, content: &str, out: &mut String) {
        let fence_len = longest_backtick_run(content).max(2) + 1;
        let fence = "`".repeat(fence_len);
        out.push_str(&fence);
        if let Some(lang) = language {
            out.push_str(lang);
        }
        out.push('\n');
        out.push_str(content);
        if !content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&fence);
        out.push('\n');
    }

    fn write_code_block_directive(
        &self,
        language: &Option<String>,
        content: &str,
        caption: &Option<Vec<Inline>>,
        label: &Option<String>,
        out: &mut String,
    ) {
        out.push_str(":::{code-block}");
        if let Some(lang) = language {
            out.push_str(&format!(" {lang}"));
        }
        out.push('\n');
        if let Some(cap) = caption {
            out.push_str(":caption: ");
            self.write_inlines(cap, out);
            out.push('\n');
        }
        if let Some(label) = label {
            out.push_str(&format!(":name: {label}\n"));
        }
        out.push('\n');
        out.push_str(content);
        if !content.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(":::\n");
    }

    fn write_list(
        &self,
        ordered: bool,
        start: Option<u32>,
        items: &[ListItem],
        tight: bool,
        out: &mut String,
    ) {
        let start_num = start.unwrap_or(1);
        for (i, item) in items.iter().enumerate() {
            let marker = if ordered {
                format!("{}. ", start_num + i as u32)
            } else {
                "- ".to_string()
            };
            if let Some(checked) = item.checked {
                let checkbox = if checked { "[x] " } else { "[ ] " };
                out.push_str(&marker);
                out.push_str(checkbox);
            } else {
                out.push_str(&marker);
            }
            let indent: String = " ".repeat(marker.len());
            let mut item_out = String::new();
            if tight {
                self.write_blocks_tight(&item.content, &mut item_out);
            } else {
                self.write_blocks(&item.content, &mut item_out);
            }
            let item_str = item_out.trim_end();
            let mut first = true;
            for line in item_str.lines() {
                if first {
                    out.push_str(line);
                    out.push('\n');
                    first = false;
                } else if line.is_empty() {
                    out.push('\n');
                } else {
                    out.push_str(&indent);
                    out.push_str(line);
                    out.push('\n');
                }
            }
        }
    }

    fn write_blocks_tight(&self, blocks: &[Block], out: &mut String) {
        for block in blocks {
            match block {
                Block::Paragraph { content } => {
                    self.write_inlines(content, out);
                    out.push('\n');
                }
                _ => self.write_block(block, out),
            }
        }
    }

    fn write_table(&self, table: &Table, out: &mut String) {
        if let Some(cap) = &table.caption {
            out.push_str("Table: ");
            self.write_inlines(cap, out);
            out.push_str("\n\n");
        }
        let ncols = table.columns.len().max(
            table
                .header
                .as_ref()
                .map(|h| h.len())
                .or_else(|| table.rows.first().map(|r| r.len()))
                .unwrap_or(1),
        );
        if let Some(header) = &table.header {
            out.push('|');
            for cell in header {
                out.push(' ');
                let mut cell_out = String::new();
                self.write_cell_inline(&cell.content, &mut cell_out);
                out.push_str(cell_out.trim());
                out.push_str(" |");
            }
            out.push('\n');
        } else {
            out.push('|');
            for _ in 0..ncols {
                out.push_str("  |");
            }
            out.push('\n');
        }
        out.push('|');
        for i in 0..ncols {
            let align = table
                .columns
                .get(i)
                .map(|c| &c.alignment)
                .unwrap_or(&Alignment::Default);
            match align {
                Alignment::Left => out.push_str(" :--- |"),
                Alignment::Center => out.push_str(" :---: |"),
                Alignment::Right => out.push_str(" ---: |"),
                Alignment::Default => out.push_str(" --- |"),
            }
        }
        out.push('\n');
        for row in &table.rows {
            out.push('|');
            for cell in row {
                out.push(' ');
                let mut cell_out = String::new();
                self.write_cell_inline(&cell.content, &mut cell_out);
                out.push_str(cell_out.trim());
                out.push_str(" |");
            }
            out.push('\n');
        }
        if let Some(foot) = &table.foot {
            out.push('|');
            for cell in foot {
                out.push(' ');
                let mut cell_out = String::new();
                self.write_cell_inline(&cell.content, &mut cell_out);
                out.push_str(cell_out.trim());
                out.push_str(" |");
            }
            out.push('\n');
        }
    }

    fn write_cell_inline(&self, blocks: &[Block], out: &mut String) {
        for block in blocks {
            if let Block::Paragraph { content } = block {
                self.write_inlines(content, out);
            }
        }
    }

    fn write_inlines(&self, inlines: &[Inline], out: &mut String) {
        for inline in inlines {
            self.write_inline(inline, out);
        }
    }

    fn write_inline(&self, inline: &Inline, out: &mut String) {
        match inline {
            Inline::Text { value } => out.push_str(value),
            Inline::Emphasis { content } => {
                out.push('*');
                self.write_inlines(content, out);
                out.push('*');
            }
            Inline::Strong { content } => {
                out.push_str("**");
                self.write_inlines(content, out);
                out.push_str("**");
            }
            Inline::Strikethrough { content } => {
                out.push_str("~~");
                self.write_inlines(content, out);
                out.push_str("~~");
            }
            Inline::Code { value, .. } => {
                let (open, close) = code_delimiters(value);
                out.push_str(&open);
                out.push_str(value);
                out.push_str(&close);
            }
            Inline::MathInline { value } => {
                out.push('$');
                out.push_str(value);
                out.push('$');
            }
            Inline::Link {
                url,
                title,
                content,
                ..
            } => {
                out.push('[');
                self.write_inlines(content, out);
                out.push_str("](");
                out.push_str(url);
                if let Some(t) = title {
                    out.push_str(&format!(" \"{t}\""));
                }
                out.push(')');
            }
            Inline::Image(img) => {
                out.push_str("![");
                let alt = img.alt_text();
                out.push_str(&alt);
                out.push_str("](");
                out.push_str(&img.url);
                if let Some(t) = &img.title {
                    out.push_str(&format!(" \"{t}\""));
                }
                out.push(')');
            }
            Inline::FootnoteRef { id } => {
                out.push_str(&format!("[^{id}]"));
            }
            Inline::RawInline { format, content } => {
                if format == "html" {
                    out.push_str(content);
                } else {
                    out.push_str(&format!("`{content}`{{={format}}}"));
                }
            }
            Inline::SmallCaps { content } => {
                out.push('[');
                self.write_inlines(content, out);
                out.push_str("]{.smallcaps}");
            }
            Inline::SoftBreak => out.push('\n'),
            Inline::HardBreak => out.push_str("  \n"),
            Inline::Span { content, attrs } => {
                out.push('[');
                self.write_inlines(content, out);
                out.push_str("]{");
                if let Some(id) = &attrs.id {
                    out.push_str(&format!("#{id}"));
                }
                for class in &attrs.classes {
                    out.push_str(&format!(" .{class}"));
                }
                for (k, v) in &attrs.key_values {
                    out.push_str(&format!(" {k}={v}"));
                }
                out.push('}');
            }
            Inline::Quoted {
                quote_type,
                content,
            } => {
                let (open, close) = match quote_type {
                    QuoteType::SingleQuote => ("\u{2018}", "\u{2019}"),
                    QuoteType::DoubleQuote => ("\u{201C}", "\u{201D}"),
                };
                out.push_str(open);
                self.write_inlines(content, out);
                out.push_str(close);
            }
            Inline::Citation(cite) => {
                let role = match cite.mode {
                    CitationMode::AuthorOnly => "cite:t",
                    CitationMode::Normal | CitationMode::SuppressAuthor => "cite:p",
                };
                let keys: Vec<&str> = cite.items.iter().map(|i| i.key.as_str()).collect();
                out.push_str(&format!("{{{role}}}`{}`", keys.join(",")));
            }
            Inline::CrossRef(cr) => {
                let role = match &cr.form {
                    RefForm::Number | RefForm::NumberWithType => "numref",
                    RefForm::Page | RefForm::Custom(_) => "ref",
                };
                out.push_str(&format!("{{{role}}}`{}`", cr.target));
            }
            Inline::Superscript { content } => {
                out.push_str("{sup}`");
                self.write_inlines(content, out);
                out.push('`');
            }
            Inline::Subscript { content } => {
                out.push_str("{sub}`");
                self.write_inlines(content, out);
                out.push('`');
            }
            Inline::Underline { content } => {
                out.push_str("{u}`");
                self.write_inlines(content, out);
                out.push('`');
            }
        }
    }
}

impl Writer for MystWriter {
    fn format(&self) -> &str {
        "myst"
    }

    fn default_extension(&self) -> &str {
        "md"
    }

    fn write(&self, doc: &Document, _opts: &WriteOptions) -> Result<String> {
        let mut body = String::with_capacity(4096);
        self.write_blocks(&doc.content, &mut body);
        Ok(body)
    }
}

fn longest_backtick_run(s: &str) -> usize {
    let mut max = 0;
    let mut current = 0;
    for c in s.chars() {
        if c == '`' {
            current += 1;
            max = max.max(current);
        } else {
            current = 0;
        }
    }
    max
}

fn code_delimiters(code: &str) -> (String, String) {
    let max_run = longest_backtick_run(code);
    let n = max_run + 1;
    let ticks = "`".repeat(n);
    if n > 1 {
        (format!("{ticks} "), format!(" {ticks}"))
    } else {
        (ticks.clone(), ticks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_myst(doc: &Document) -> String {
        let writer = MystWriter::new();
        writer.write(doc, &WriteOptions::default()).unwrap()
    }

    #[test]
    fn paragraph() {
        let doc = Document {
            content: vec![Block::text("Hello, world!")],
            ..Default::default()
        };
        assert_eq!(write_myst(&doc).trim(), "Hello, world!");
    }

    #[test]
    fn writer_trait_metadata() {
        let writer = MystWriter::new();
        assert_eq!(writer.format(), "myst");
        assert_eq!(writer.default_extension(), "md");
    }

    #[test]
    fn heading_no_id() {
        let doc = Document {
            content: vec![Block::heading(1, "Title"), Block::heading(2, "Subtitle")],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("# Title\n"));
        assert!(myst.contains("## Subtitle\n"));
    }

    #[test]
    fn heading_with_id() {
        let doc = Document {
            content: vec![Block::Heading {
                level: 2,
                id: Some("intro".into()),
                content: vec![Inline::text("Introduction")],
                attrs: None,
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("(intro)=\n## Introduction\n"));
    }

    #[test]
    fn code_block_simple() {
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
        let myst = write_myst(&doc);
        assert!(myst.contains("```python\nprint('hello')\n```"));
    }

    #[test]
    fn code_block_with_caption() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "print('hello')".into(),
                caption: Some(vec![Inline::text("Hello example")]),
                label: Some("code-hello".into()),
                attrs: None,
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains(":::{code-block} python\n"));
        assert!(myst.contains(":caption: Hello example\n"));
        assert!(myst.contains(":name: code-hello\n"));
        assert!(myst.contains("print('hello')\n"));
        assert!(myst.contains(":::\n"));
    }

    #[test]
    fn math_block_no_label() {
        let doc = Document {
            content: vec![Block::MathBlock {
                content: "E = mc^2".into(),
                label: None,
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("$$\nE = mc^2\n$$"));
    }

    #[test]
    fn math_block_with_label() {
        let doc = Document {
            content: vec![Block::MathBlock {
                content: "E = mc^2".into(),
                label: Some("eq:einstein".into()),
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("(eq:einstein)=\n$$\nE = mc^2\n$$"));
    }

    #[test]
    fn blockquote() {
        let doc = Document {
            content: vec![Block::BlockQuote {
                content: vec![Block::text("Quoted text.")],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("> Quoted text.\n"));
    }

    #[test]
    fn thematic_break() {
        let doc = Document {
            content: vec![
                Block::text("Above"),
                Block::ThematicBreak,
                Block::text("Below"),
            ],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("---\n"));
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
        let myst = write_myst(&doc);
        assert!(myst.contains("- Alpha\n"));
        assert!(myst.contains("- Beta\n"));
    }

    #[test]
    fn ordered_list() {
        let doc = Document {
            content: vec![Block::List {
                ordered: true,
                start: Some(3),
                items: vec![
                    ListItem {
                        checked: None,
                        content: vec![Block::text("Third")],
                    },
                    ListItem {
                        checked: None,
                        content: vec![Block::text("Fourth")],
                    },
                ],
                tight: true,
                style: None,
                delimiter: None,
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("3. Third\n"));
        assert!(myst.contains("4. Fourth\n"));
    }

    #[test]
    fn task_list() {
        let doc = Document {
            content: vec![Block::List {
                ordered: false,
                start: None,
                items: vec![
                    ListItem {
                        checked: Some(true),
                        content: vec![Block::text("Done")],
                    },
                    ListItem {
                        checked: Some(false),
                        content: vec![Block::text("Todo")],
                    },
                ],
                tight: true,
                style: None,
                delimiter: None,
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("- [x] Done\n"));
        assert!(myst.contains("- [ ] Todo\n"));
    }

    #[test]
    fn table() {
        let doc = Document {
            content: vec![Block::Table(Table {
                caption: None,
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
                        content: vec![Block::text("Name")],
                        colspan: 1,
                        rowspan: 1,
                    },
                    TableCell {
                        content: vec![Block::text("Value")],
                        colspan: 1,
                        rowspan: 1,
                    },
                ]),
                rows: vec![vec![
                    TableCell {
                        content: vec![Block::text("Pi")],
                        colspan: 1,
                        rowspan: 1,
                    },
                    TableCell {
                        content: vec![Block::text("3.14")],
                        colspan: 1,
                        rowspan: 1,
                    },
                ]],
                foot: None,
                attrs: None,
            })],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("| Name | Value |"));
        assert!(myst.contains("| :--- | ---: |"));
        assert!(myst.contains("| Pi | 3.14 |"));
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
        let myst = write_myst(&doc);
        assert!(myst.contains("Rust\n:   A systems programming language."));
    }

    #[test]
    fn raw_block_html() {
        let doc = Document {
            content: vec![Block::RawBlock {
                format: "html".into(),
                content: "<div>raw</div>\n".into(),
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("<div>raw</div>\n"));
    }

    #[test]
    fn admonition_note() {
        let doc = Document {
            content: vec![Block::Admonition {
                kind: AdmonitionKind::Note,
                title: None,
                content: vec![Block::text("Take note.")],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains(":::{note}\nTake note.\n:::\n"));
    }

    #[test]
    fn admonition_with_title() {
        let doc = Document {
            content: vec![Block::Admonition {
                kind: AdmonitionKind::Warning,
                title: Some(vec![Inline::text("Be careful")]),
                content: vec![Block::text("Danger ahead.")],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains(":::{warning} Be careful\n"));
        assert!(myst.contains("Danger ahead.\n"));
        assert!(myst.contains(":::\n"));
    }

    #[test]
    fn admonition_custom() {
        let doc = Document {
            content: vec![Block::Admonition {
                kind: AdmonitionKind::Custom("danger".into()),
                title: None,
                content: vec![Block::text("Very dangerous.")],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains(":::{danger}\n"));
    }

    #[test]
    fn figure_with_caption_and_label() {
        let doc = Document {
            content: vec![Block::Figure {
                image: Image {
                    url: "photo.png".into(),
                    alt: vec![Inline::text("A photo")],
                    title: None,
                    attrs: None,
                },
                caption: Some(vec![Inline::text("My photo caption")]),
                label: Some("fig-photo".into()),
                attrs: None,
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains(":::{figure} photo.png\n"));
        assert!(myst.contains(":alt: A photo\n"));
        assert!(myst.contains(":name: fig-photo\n"));
        assert!(myst.contains("\nMy photo caption\n"));
        assert!(myst.contains(":::\n"));
    }

    #[test]
    fn figure_no_caption() {
        let doc = Document {
            content: vec![Block::Figure {
                image: Image {
                    url: "img.jpg".into(),
                    alt: vec![Inline::text("Alt text")],
                    title: None,
                    attrs: None,
                },
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains(":::{figure} img.jpg\n"));
        assert!(myst.contains(":alt: Alt text\n"));
        assert!(myst.contains(":::\n"));
    }

    #[test]
    fn footnote_def() {
        let doc = Document {
            content: vec![Block::FootnoteDef {
                id: "1".into(),
                content: vec![Block::text("The footnote.")],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("[^1]: The footnote."));
    }

    #[test]
    fn div_block() {
        use std::collections::HashMap;
        let doc = Document {
            content: vec![Block::Div {
                attrs: Attributes {
                    id: Some("note1".into()),
                    classes: vec!["special".into()],
                    key_values: HashMap::new(),
                },
                content: vec![Block::text("Div content.")],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains(":::"));
        assert!(myst.contains("Div content."));
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
        let myst = write_myst(&doc);
        assert!(myst.contains("*italic*"));
        assert!(myst.contains("**bold**"));
    }

    #[test]
    fn inline_code() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("Use "),
                    Inline::Code {
                        value: "fn main()".into(),
                        attrs: None,
                    },
                ],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("`fn main()`"));
    }

    #[test]
    fn inline_math() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("Energy: "),
                    Inline::MathInline {
                        value: "E = mc^2".into(),
                    },
                ],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("$E = mc^2$"));
    }

    #[test]
    fn link() {
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
        let myst = write_myst(&doc);
        assert!(myst.contains("[Example](https://example.com)"));
    }

    #[test]
    fn image_inline() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Image(Image {
                    url: "photo.png".into(),
                    alt: vec![Inline::text("A photo")],
                    title: None,
                    attrs: None,
                })],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("![A photo](photo.png)"));
    }

    #[test]
    fn strikethrough() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Strikethrough {
                    content: vec![Inline::text("deleted")],
                }],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("~~deleted~~"));
    }

    #[test]
    fn footnote_ref() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::text("See"), Inline::FootnoteRef { id: "1".into() }],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("[^1]"));
    }

    #[test]
    fn hard_break() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("Line one"),
                    Inline::HardBreak,
                    Inline::text("Line two"),
                ],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("Line one  \nLine two"));
    }

    #[test]
    fn raw_inline_html() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::RawInline {
                    format: "html".into(),
                    content: "<br>".into(),
                }],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("<br>"));
    }

    #[test]
    fn quoted_inlines() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::Quoted {
                        quote_type: QuoteType::DoubleQuote,
                        content: vec![Inline::text("hello")],
                    },
                    Inline::text(" and "),
                    Inline::Quoted {
                        quote_type: QuoteType::SingleQuote,
                        content: vec![Inline::text("world")],
                    },
                ],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("\u{201C}hello\u{201D}"));
        assert!(myst.contains("\u{2018}world\u{2019}"));
    }

    #[test]
    fn span_with_attrs() {
        use std::collections::HashMap;
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Span {
                    content: vec![Inline::text("styled")],
                    attrs: Attributes {
                        id: Some("s1".into()),
                        classes: vec!["highlight".into()],
                        key_values: HashMap::new(),
                    },
                }],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("[styled]{#s1 .highlight}"));
    }

    #[test]
    fn smallcaps() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::SmallCaps {
                    content: vec![Inline::text("Title")],
                }],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("[Title]{.smallcaps}"));
    }

    #[test]
    fn citation_normal() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Citation(Citation {
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
                })],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert_eq!(myst.trim(), "{cite:p}`smith2020,jones2021`");
    }

    #[test]
    fn citation_author_only() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Citation(Citation {
                    items: vec![CiteItem {
                        key: "smith2020".into(),
                        prefix: None,
                        suffix: None,
                    }],
                    mode: CitationMode::AuthorOnly,
                })],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert_eq!(myst.trim(), "{cite:t}`smith2020`");
    }

    #[test]
    fn crossref_numref() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::CrossRef(CrossRef {
                    target: "fig-photo".into(),
                    form: RefForm::Number,
                })],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert_eq!(myst.trim(), "{numref}`fig-photo`");
    }

    #[test]
    fn crossref_ref() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::CrossRef(CrossRef {
                    target: "my-section".into(),
                    form: RefForm::Custom("see".into()),
                })],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert_eq!(myst.trim(), "{ref}`my-section`");
    }

    #[test]
    fn superscript_role() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("x"),
                    Inline::Superscript {
                        content: vec![Inline::text("2")],
                    },
                ],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("{sup}`2`"));
    }

    #[test]
    fn subscript_role() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("H"),
                    Inline::Subscript {
                        content: vec![Inline::text("2")],
                    },
                    Inline::text("O"),
                ],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("{sub}`2`"));
    }

    #[test]
    fn underline_role() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Underline {
                    content: vec![Inline::text("underlined")],
                }],
            }],
            ..Default::default()
        };
        let myst = write_myst(&doc);
        assert!(myst.contains("{u}`underlined`"));
    }
}
