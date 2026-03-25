//! # docmux-writer-markdown
//!
//! CommonMark / GFM Markdown writer for docmux. Converts the docmux AST
//! back into Markdown, enabling roundtrip and normalization workflows.

use docmux_ast::*;
use docmux_core::{Result, WriteOptions, Writer};

/// A Markdown writer producing CommonMark with GFM extensions.
#[derive(Debug, Default)]
pub struct MarkdownWriter;

impl MarkdownWriter {
    pub fn new() -> Self {
        Self
    }

    fn write_blocks(&self, blocks: &[Block], out: &mut String, ctx: &Ctx) {
        for (i, block) in blocks.iter().enumerate() {
            if i > 0 {
                // Separate blocks with a blank line (unless the previous
                // block already ends with two newlines).
                if !out.ends_with("\n\n") {
                    if out.ends_with('\n') {
                        out.push('\n');
                    } else {
                        out.push_str("\n\n");
                    }
                }
            }
            self.write_block(block, out, ctx);
        }
    }

    fn write_block(&self, block: &Block, out: &mut String, ctx: &Ctx) {
        match block {
            Block::Paragraph { content } => {
                self.write_inlines(content, out);
                out.push('\n');
            }
            Block::Heading {
                level,
                id,
                content,
                attrs,
            } => {
                for _ in 0..*level {
                    out.push('#');
                }
                out.push(' ');
                self.write_inlines(content, out);
                // Emit pandoc-style attributes if present
                let has_explicit_attrs = id.is_some()
                    || attrs
                        .as_ref()
                        .is_some_and(|a| !a.classes.is_empty() || !a.key_values.is_empty());
                if has_explicit_attrs {
                    out.push_str(" {");
                    if let Some(id) = id {
                        out.push_str(&format!("#{id}"));
                    }
                    if let Some(a) = attrs {
                        for class in &a.classes {
                            out.push_str(&format!(" .{class}"));
                        }
                        for (k, v) in &a.key_values {
                            out.push_str(&format!(" {k}={v}"));
                        }
                    }
                    out.push('}');
                }
                out.push('\n');
            }
            Block::CodeBlock {
                language,
                content,
                attrs,
                ..
            } => {
                // Determine the minimum fence length needed
                let fence_len = longest_backtick_run(content).max(2) + 1;
                let fence = "`".repeat(fence_len);

                out.push_str(&fence);
                // Emit pandoc-style attributes or simple info string
                let has_attrs = attrs
                    .as_ref()
                    .is_some_and(|a| !a.classes.is_empty() || !a.key_values.is_empty());
                if has_attrs {
                    out.push('{');
                    if let Some(lang) = language {
                        out.push_str(&format!(".{lang}"));
                    }
                    if let Some(a) = attrs {
                        for class in &a.classes {
                            if language.as_deref() == Some(class) {
                                continue; // already emitted as language
                            }
                            out.push_str(&format!(" .{class}"));
                        }
                        for (k, v) in &a.key_values {
                            out.push_str(&format!(" {k}={v}"));
                        }
                    }
                    out.push('}');
                } else if let Some(lang) = language {
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
            Block::MathBlock { content, label } => {
                out.push_str("$$\n");
                out.push_str(content.trim());
                out.push('\n');
                out.push_str("$$\n");
                if let Some(label) = label {
                    // Emit label as an HTML comment (common convention)
                    out.push_str(&format!("<!-- label: {label} -->\n"));
                }
            }
            Block::BlockQuote { content } => {
                let mut inner = String::new();
                self.write_blocks(content, &mut inner, ctx);
                for line in inner.lines() {
                    if line.is_empty() {
                        out.push_str(">\n");
                    } else {
                        out.push_str(&format!("> {line}\n"));
                    }
                }
            }
            Block::List {
                ordered,
                start,
                items,
                tight,
                ..
            } => {
                let start_num = start.unwrap_or(1);
                for (i, item) in items.iter().enumerate() {
                    let marker = if *ordered {
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
                    if *tight {
                        // Tight: render content inline
                        self.write_blocks_tight(&item.content, &mut item_out, ctx);
                    } else {
                        self.write_blocks(&item.content, &mut item_out, ctx);
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
            Block::Table(table) => {
                self.write_table(table, out);
            }
            Block::Figure {
                image,
                caption,
                label,
                ..
            } => {
                out.push_str("![");
                if let Some(cap) = caption {
                    self.write_inlines(cap, out);
                } else {
                    let alt = image.alt_text();
                    out.push_str(&alt);
                }
                out.push_str(&format!("]({})", &image.url));
                if let Some(title) = &image.title {
                    // Rewrite to include title
                    let url = &image.url;
                    let last = out.len();
                    let remove_from = last - format!("]({})", url).len();
                    out.truncate(remove_from);
                    out.push_str(&format!("]({}  \"{}\")", url, title));
                }
                out.push('\n');
                if let Some(label) = label {
                    out.push_str(&format!("<!-- label: {label} -->\n"));
                }
            }
            Block::ThematicBreak => {
                out.push_str("---\n");
            }
            Block::RawBlock { format, content } => {
                if format == "html" {
                    out.push_str(content);
                    if !content.ends_with('\n') {
                        out.push('\n');
                    }
                } else {
                    // Wrap non-html raw blocks in a code fence
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
                // Use GFM-style admonition syntax: > [!NOTE]
                let tag = match kind {
                    AdmonitionKind::Note => "NOTE",
                    AdmonitionKind::Warning => "WARNING",
                    AdmonitionKind::Tip => "TIP",
                    AdmonitionKind::Important => "IMPORTANT",
                    AdmonitionKind::Caution => "CAUTION",
                    AdmonitionKind::Custom(c) => c.as_str(),
                };
                out.push_str(&format!("> [!{tag}]"));
                if let Some(t) = title {
                    out.push_str(" **");
                    self.write_inlines(t, out);
                    out.push_str("**");
                }
                out.push('\n');
                let mut inner = String::new();
                self.write_blocks(content, &mut inner, ctx);
                for line in inner.lines() {
                    if line.is_empty() {
                        out.push_str(">\n");
                    } else {
                        out.push_str(&format!("> {line}\n"));
                    }
                }
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
                        self.write_blocks(def, &mut def_out, ctx);
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
            Block::Div { attrs, content } => {
                // Fenced div syntax: ::: {#id .class}
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
                self.write_blocks(content, out, ctx);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push_str(":::\n");
            }
            Block::FootnoteDef { id, content } => {
                out.push_str(&format!("[^{id}]: "));
                let mut inner = String::new();
                self.write_blocks(content, &mut inner, ctx);
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
        }
    }

    /// Write blocks in tight-list mode (no blank lines between paragraphs).
    fn write_blocks_tight(&self, blocks: &[Block], out: &mut String, ctx: &Ctx) {
        for block in blocks {
            match block {
                Block::Paragraph { content } => {
                    self.write_inlines(content, out);
                    out.push('\n');
                }
                _ => self.write_block(block, out, ctx),
            }
        }
    }

    fn write_table(&self, table: &Table, out: &mut String) {
        // Collect all rows for width calculation
        let ncols = table.columns.len().max(
            table
                .header
                .as_ref()
                .map(|h| h.len())
                .or_else(|| table.rows.first().map(|r| r.len()))
                .unwrap_or(1),
        );

        // Render header
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
            // Empty header row
            out.push('|');
            for _ in 0..ncols {
                out.push_str("  |");
            }
            out.push('\n');
        }

        // Separator row with alignment
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

        // Body rows
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

        // Footer rows (rendered as regular rows — GFM has no tfoot)
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

    /// Write table cell content as inline text (flatten paragraphs).
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
            Inline::Text { value } => {
                out.push_str(value);
            }
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
                // Check for autolink
                let mut link_text = String::new();
                self.write_inlines(content, &mut link_text);
                if link_text == *url && title.is_none() {
                    out.push_str(&format!("<{url}>"));
                } else {
                    out.push('[');
                    self.write_inlines(content, out);
                    out.push(']');
                    out.push('(');
                    out.push_str(url);
                    if let Some(t) = title {
                        out.push_str(&format!(" \"{t}\""));
                    }
                    out.push(')');
                }
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
            Inline::Citation(cite) => {
                // Pandoc-style: [@key1; @key2]
                out.push('[');
                for (i, item) in cite.items.iter().enumerate() {
                    if i > 0 {
                        out.push_str("; ");
                    }
                    if let Some(prefix) = &item.prefix {
                        out.push_str(prefix);
                        out.push(' ');
                    }
                    out.push('@');
                    out.push_str(&item.key);
                    if let Some(suffix) = &item.suffix {
                        out.push(' ');
                        out.push_str(suffix);
                    }
                }
                out.push(']');
            }
            Inline::FootnoteRef { id } => {
                out.push_str(&format!("[^{id}]"));
            }
            Inline::CrossRef(cr) => {
                // Emit as a link to the label
                out.push_str(&format!("[{target}](#{target})", target = cr.target));
            }
            Inline::RawInline { format, content } => {
                if format == "html" {
                    out.push_str(content);
                } else {
                    // Wrap in backticks with format tag
                    out.push_str(&format!("`{content}`{{={format}}}"));
                }
            }
            Inline::Superscript { content } => {
                out.push('^');
                self.write_inlines(content, out);
                out.push('^');
            }
            Inline::Subscript { content } => {
                out.push('~');
                self.write_inlines(content, out);
                out.push('~');
            }
            Inline::SmallCaps { content } => {
                // No native markdown — use a span
                out.push('[');
                self.write_inlines(content, out);
                out.push_str("]{.smallcaps}");
            }
            Inline::SoftBreak => {
                out.push('\n');
            }
            Inline::HardBreak => {
                out.push_str("  \n");
            }
            Inline::Underline { content } => {
                // No native markdown — use HTML
                out.push_str("<u>");
                self.write_inlines(content, out);
                out.push_str("</u>");
            }
            Inline::Span { content, attrs } => {
                // Pandoc bracketed span: [content]{#id .class key=val}
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
        }
    }

    fn wrap_standalone(&self, body: &str, doc: &Document) -> String {
        let mut out = String::with_capacity(body.len() + 256);

        // YAML frontmatter
        let meta = &doc.metadata;
        let has_meta = meta.title.is_some()
            || !meta.authors.is_empty()
            || meta.date.is_some()
            || meta.abstract_text.is_some()
            || !meta.keywords.is_empty()
            || !meta.custom.is_empty();

        if has_meta {
            out.push_str("---\n");
            if let Some(title) = &meta.title {
                out.push_str(&format!("title: \"{}\"\n", yaml_escape(title)));
            }
            if meta.authors.len() == 1 {
                out.push_str(&format!(
                    "author: \"{}\"\n",
                    yaml_escape(&meta.authors[0].name)
                ));
            } else if meta.authors.len() > 1 {
                out.push_str("author:\n");
                for a in &meta.authors {
                    if a.affiliation.is_some() || a.email.is_some() || a.orcid.is_some() {
                        out.push_str(&format!("  - name: \"{}\"\n", yaml_escape(&a.name)));
                        if let Some(aff) = &a.affiliation {
                            out.push_str(&format!("    affiliation: \"{}\"\n", yaml_escape(aff)));
                        }
                        if let Some(email) = &a.email {
                            out.push_str(&format!("    email: \"{}\"\n", yaml_escape(email)));
                        }
                        if let Some(orcid) = &a.orcid {
                            out.push_str(&format!("    orcid: \"{}\"\n", yaml_escape(orcid)));
                        }
                    } else {
                        out.push_str(&format!("  - \"{}\"\n", yaml_escape(&a.name)));
                    }
                }
            }
            if let Some(date) = &meta.date {
                out.push_str(&format!("date: \"{}\"\n", yaml_escape(date)));
            }
            if let Some(abstract_blocks) = &meta.abstract_text {
                // Flatten abstract blocks to plain text for YAML
                let abstract_text = blocks_to_plain_text(abstract_blocks);
                out.push_str(&format!("abstract: \"{}\"\n", yaml_escape(&abstract_text)));
            }
            if !meta.keywords.is_empty() {
                out.push_str(&format!(
                    "keywords: [{}]\n",
                    meta.keywords
                        .iter()
                        .map(|k| format!("\"{}\"", yaml_escape(k)))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            for (k, v) in &meta.custom {
                write_meta_value(&mut out, k, v, 0);
            }
            out.push_str("---\n\n");
        }

        out.push_str(body);
        out
    }
}

impl Writer for MarkdownWriter {
    fn format(&self) -> &str {
        "markdown"
    }

    fn default_extension(&self) -> &str {
        "md"
    }

    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
        let ctx = Ctx {};
        let mut body = String::with_capacity(4096);
        self.write_blocks(&doc.content, &mut body, &ctx);

        if opts.standalone {
            Ok(self.wrap_standalone(&body, doc))
        } else {
            Ok(body)
        }
    }
}

/// Internal context (reserved for future use, e.g. footnote collection).
struct Ctx {}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Find the backtick delimiter pair for inline code that avoids clashing
/// with backticks inside the code.
fn code_delimiters(code: &str) -> (String, String) {
    let max_run = longest_backtick_run(code);
    let n = max_run + 1;
    let ticks = "`".repeat(n);
    if n > 1 {
        // Add space padding when using multiple backticks
        (format!("{ticks} "), format!(" {ticks}"))
    } else {
        (ticks.clone(), ticks)
    }
}

/// Return the length of the longest consecutive backtick run in `s`.
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

/// Escape a string for YAML double-quoted values.
fn yaml_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Flatten blocks to plain text (for YAML abstract serialization).
fn blocks_to_plain_text(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        if let Block::Paragraph { content } = block {
            inlines_to_plain_text(content, &mut out);
        }
    }
    out
}

fn inlines_to_plain_text(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text { value } => out.push_str(value),
            Inline::Code { value, .. } => out.push_str(value),
            Inline::MathInline { value } => out.push_str(value),
            Inline::SoftBreak | Inline::HardBreak => out.push(' '),
            Inline::Emphasis { content }
            | Inline::Strong { content }
            | Inline::Strikethrough { content }
            | Inline::Underline { content }
            | Inline::Superscript { content }
            | Inline::Subscript { content }
            | Inline::SmallCaps { content }
            | Inline::Span { content, .. }
            | Inline::Quoted { content, .. } => inlines_to_plain_text(content, out),
            Inline::Link { content, .. } => inlines_to_plain_text(content, out),
            _ => {}
        }
    }
}

/// Write a MetaValue to YAML.
fn write_meta_value(out: &mut String, key: &str, val: &MetaValue, indent: usize) {
    let pad: String = " ".repeat(indent);
    match val {
        MetaValue::String(s) => {
            out.push_str(&format!("{pad}{key}: \"{}\"\n", yaml_escape(s)));
        }
        MetaValue::Bool(b) => {
            out.push_str(&format!("{pad}{key}: {b}\n"));
        }
        MetaValue::Number(n) => {
            out.push_str(&format!("{pad}{key}: {n}\n"));
        }
        MetaValue::List(items) => {
            out.push_str(&format!("{pad}{key}:\n"));
            for item in items {
                match item {
                    MetaValue::String(s) => {
                        out.push_str(&format!("{pad}  - \"{}\"\n", yaml_escape(s)));
                    }
                    _ => {
                        out.push_str(&format!("{pad}  - "));
                        // Simplified: just serialize as string
                        out.push_str(&format!("{item:?}\n"));
                    }
                }
            }
        }
        MetaValue::Map(map) => {
            out.push_str(&format!("{pad}{key}:\n"));
            for (k, v) in map {
                write_meta_value(out, k, v, indent + 2);
            }
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn write_md(doc: &Document) -> String {
        let writer = MarkdownWriter::new();
        writer.write(doc, &WriteOptions::default()).unwrap()
    }

    #[test]
    fn paragraph() {
        let doc = Document {
            content: vec![Block::text("Hello, world!")],
            ..Default::default()
        };
        assert_eq!(write_md(&doc).trim(), "Hello, world!");
    }

    #[test]
    fn headings() {
        let doc = Document {
            content: vec![
                Block::heading(1, "Title"),
                Block::heading(2, "Subtitle"),
                Block::heading(3, "Sub-subtitle"),
            ],
            ..Default::default()
        };
        let md = write_md(&doc);
        assert!(md.contains("# Title\n"));
        assert!(md.contains("## Subtitle\n"));
        assert!(md.contains("### Sub-subtitle\n"));
    }

    #[test]
    fn heading_with_id_and_attrs() {
        let doc = Document {
            content: vec![Block::Heading {
                level: 2,
                id: Some("intro".into()),
                content: vec![Inline::text("Introduction")],
                attrs: Some(Attributes {
                    id: None,
                    classes: vec!["unnumbered".into()],
                    key_values: HashMap::new(),
                }),
            }],
            ..Default::default()
        };
        let md = write_md(&doc);
        assert!(md.contains("## Introduction {#intro .unnumbered}"));
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
        let md = write_md(&doc);
        assert!(md.contains("*italic*"));
        assert!(md.contains("**bold**"));
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
                    Inline::text(" please."),
                ],
            }],
            ..Default::default()
        };
        let md = write_md(&doc);
        assert!(md.contains("`fn main()`"));
    }

    #[test]
    fn inline_code_with_backticks() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Code {
                    value: "x = `a`".into(),
                    attrs: None,
                }],
            }],
            ..Default::default()
        };
        let md = write_md(&doc);
        assert!(md.contains("`` x = `a` ``"));
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
        let md = write_md(&doc);
        assert!(md.contains("```python\nprint('hello')\n```"));
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
        let md = write_md(&doc);
        assert!(md.contains("$E = mc^2$"));
    }

    #[test]
    fn display_math() {
        let doc = Document {
            content: vec![Block::MathBlock {
                content: "x^2 + y^2 = z^2".into(),
                label: None,
            }],
            ..Default::default()
        };
        let md = write_md(&doc);
        assert!(md.contains("$$\nx^2 + y^2 = z^2\n$$"));
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
        let md = write_md(&doc);
        assert!(md.contains("- Alpha\n"));
        assert!(md.contains("- Beta\n"));
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
        let md = write_md(&doc);
        assert!(md.contains("3. Third\n"));
        assert!(md.contains("4. Fourth\n"));
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
        let md = write_md(&doc);
        assert!(md.contains("- [x] Done\n"));
        assert!(md.contains("- [ ] Todo\n"));
    }

    #[test]
    fn blockquote() {
        let doc = Document {
            content: vec![Block::BlockQuote {
                content: vec![Block::text("Quoted text.")],
            }],
            ..Default::default()
        };
        let md = write_md(&doc);
        assert!(md.contains("> Quoted text.\n"));
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
        let md = write_md(&doc);
        assert!(md.contains("| Name | Value |"));
        assert!(md.contains("| :--- | ---: |"));
        assert!(md.contains("| Pi | 3.14 |"));
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
        let md = write_md(&doc);
        assert!(md.contains("[Example](https://example.com)"));
    }

    #[test]
    fn autolink() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Link {
                    url: "https://example.com".into(),
                    title: None,
                    content: vec![Inline::text("https://example.com")],
                    attrs: None,
                }],
            }],
            ..Default::default()
        };
        let md = write_md(&doc);
        assert!(md.contains("<https://example.com>"));
    }

    #[test]
    fn image() {
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
        let md = write_md(&doc);
        assert!(md.contains("![A photo](photo.png)"));
    }

    #[test]
    fn footnote_ref_and_def() {
        let doc = Document {
            content: vec![
                Block::Paragraph {
                    content: vec![Inline::text("See"), Inline::FootnoteRef { id: "1".into() }],
                },
                Block::FootnoteDef {
                    id: "1".into(),
                    content: vec![Block::text("The footnote.")],
                },
            ],
            ..Default::default()
        };
        let md = write_md(&doc);
        assert!(md.contains("[^1]"));
        assert!(md.contains("[^1]: The footnote."));
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
        let md = write_md(&doc);
        assert!(md.contains("~~deleted~~"));
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
        let md = write_md(&doc);
        assert!(md.contains("---\n"));
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
        let md = write_md(&doc);
        assert!(md.contains("Line one  \nLine two"));
    }

    #[test]
    fn standalone_with_frontmatter() {
        let doc = Document {
            metadata: Metadata {
                title: Some("My Doc".into()),
                authors: vec![Author {
                    name: "Jane Doe".into(),
                    affiliation: None,
                    email: None,
                    orcid: None,
                }],
                date: Some("2026-03-25".into()),
                abstract_text: Some(vec![Block::text("This is the abstract.")]),
                ..Default::default()
            },
            content: vec![Block::text("Body.")],
            ..Default::default()
        };
        let writer = MarkdownWriter::new();
        let opts = WriteOptions {
            standalone: true,
            ..Default::default()
        };
        let md = writer.write(&doc, &opts).unwrap();
        assert!(md.starts_with("---\n"));
        assert!(md.contains("title: \"My Doc\""));
        assert!(md.contains("author: \"Jane Doe\""));
        assert!(md.contains("date: \"2026-03-25\""));
        assert!(md.contains("abstract: \"This is the abstract.\""));
        assert!(md.contains("---\n\nBody."));
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
        let md = write_md(&doc);
        assert!(md.contains("\u{201C}hello\u{201D}"));
        assert!(md.contains("\u{2018}world\u{2019}"));
    }

    #[test]
    fn citation() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Citation(Citation {
                    items: vec![
                        CiteItem {
                            key: "smith2020".into(),
                            prefix: Some("see".into()),
                            suffix: None,
                        },
                        CiteItem {
                            key: "jones2021".into(),
                            prefix: None,
                            suffix: Some("p. 42".into()),
                        },
                    ],
                    mode: CitationMode::Normal,
                })],
            }],
            ..Default::default()
        };
        let md = write_md(&doc);
        assert!(md.contains("[see @smith2020; @jones2021 p. 42]"));
    }

    #[test]
    fn writer_trait_metadata() {
        let writer = MarkdownWriter::new();
        assert_eq!(writer.format(), "markdown");
        assert_eq!(writer.default_extension(), "md");
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
        let md = write_md(&doc);
        assert!(md.contains("Rust\n:   A systems programming language."));
    }

    #[test]
    fn admonition() {
        let doc = Document {
            content: vec![Block::Admonition {
                kind: AdmonitionKind::Warning,
                title: None,
                content: vec![Block::text("Be careful!")],
            }],
            ..Default::default()
        };
        let md = write_md(&doc);
        assert!(md.contains("> [!WARNING]"));
        assert!(md.contains("> Be careful!"));
    }

    #[test]
    fn fenced_div() {
        let doc = Document {
            content: vec![Block::Div {
                attrs: Attributes {
                    id: Some("note1".into()),
                    classes: vec!["warning".into()],
                    key_values: HashMap::new(),
                },
                content: vec![Block::text("Content here.")],
            }],
            ..Default::default()
        };
        let md = write_md(&doc);
        assert!(md.contains("::: {#note1 .warning}"));
        assert!(md.contains("Content here."));
        assert!(md.contains(":::"));
    }
}
