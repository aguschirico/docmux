//! # docmux-writer-typst
//!
//! Typst writer for docmux. Converts the docmux AST into Typst markup.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

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

/// What the writer needs to know about the whole document while writing
/// any part of it.
#[derive(Default)]
struct Ctx {
    footnotes: HashMap<String, Vec<Block>>,
    /// Every label defined in the document. A Typst `@ref` to anything else
    /// is a compile error, so dangling references are written as text.
    labels: HashSet<String>,
    /// Titles of unnumbered headings by label. Typst cannot `@ref` them, so
    /// a reference links to the heading with its title instead.
    unnumbered: HashMap<String, Vec<Inline>>,
    /// Labels already attached. Typst rejects a label that occurs twice, and
    /// papers do reuse them, so only the first occurrence keeps it.
    attached: RefCell<HashSet<String>>,
}

impl Ctx {
    /// ` <label>` to attach after an element, or nothing if an earlier
    /// element already has this label.
    fn attach(&self, label: &str) -> String {
        if self.attached.borrow_mut().insert(label.to_string()) {
            format!(" <{}>", typst_label(label))
        } else {
            String::new()
        }
    }
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
        ctx: &Ctx,
    ) {
        for block in blocks {
            self.write_block_impl(block, opts, out, ctx);
        }
    }

    fn write_block_impl(&self, block: &Block, opts: &WriteOptions, out: &mut String, ctx: &Ctx) {
        match block {
            Block::Paragraph { content } => {
                self.write_inlines_impl(content, opts, out, ctx);
                out.push_str("\n\n");
            }
            Block::Heading {
                level,
                id,
                content,
                attrs,
            } => {
                let unnumbered = attrs
                    .as_ref()
                    .is_some_and(|a| a.classes.iter().any(|c| c == "unnumbered"));
                // A `=` heading ends at the end of its line, so a line break
                // in the title would leave the rest, and the label, outside it.
                let mut title = String::new();
                self.write_inlines_impl(content, opts, &mut title, ctx);
                let title = title.replace('\n', " ");
                if unnumbered {
                    out.push_str(&format!(
                        "#heading(level: {level}, numbering: none)[{title}]"
                    ));
                } else {
                    out.push_str(&"=".repeat(*level as usize));
                    out.push(' ');
                    out.push_str(&title);
                }
                if let Some(id) = id {
                    out.push_str(&ctx.attach(id));
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
                        self.write_inlines_impl(cap, opts, out, ctx);
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
                        out.push_str(&ctx.attach(label));
                    }
                    out.push('\n');
                }
            }
            Block::MathBlock { content, label } => {
                out.push_str("$\n");
                out.push_str(content.trim());
                out.push_str("\n$");
                if let Some(label) = label {
                    out.push_str(&ctx.attach(label));
                }
                out.push('\n');
            }
            Block::BlockQuote { content } => {
                out.push_str("#quote(block: true)[\n");
                self.write_blocks_impl(content, opts, out, ctx);
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
                    self.write_blocks_impl(&item.content, opts, &mut item_content, ctx);
                    out.push_str(trim_markup(&item_content));
                    out.push('\n');
                }
            }
            Block::Table(table) => {
                self.write_table(table, opts, out, ctx);
            }
            Block::Figure {
                image,
                caption,
                label,
                ..
            } => {
                out.push_str("#figure(\n");
                if image.url.is_empty() {
                    // Figures drawn in the source (TikZ, picture) have no file.
                    out.push_str("  [],\n");
                } else {
                    out.push_str(&format!("  image(\"{}\"),\n", escape_typst_url(&image.url)));
                }
                if let Some(cap) = caption {
                    out.push_str("  caption: [");
                    self.write_inlines_impl(cap, opts, out, ctx);
                    out.push_str("],\n");
                }
                out.push(')');
                if let Some(label) = label {
                    out.push_str(&ctx.attach(label));
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
                    self.write_inlines_impl(t, opts, out, ctx);
                    out.push_str("*\n\n");
                } else {
                    out.push_str(&format!("*{}*\n\n", label));
                }
                self.write_blocks_impl(content, opts, out, ctx);
                out.push_str("]\n");
            }
            Block::DefinitionList { items } => {
                for item in items {
                    for def in &item.definitions {
                        out.push_str("/ ");
                        self.write_inlines_impl(&item.term, opts, out, ctx);
                        out.push_str(": ");
                        let mut def_content = String::new();
                        self.write_blocks_impl(def, opts, &mut def_content, ctx);
                        out.push_str(trim_markup(&def_content));
                        out.push('\n');
                    }
                }
            }
            Block::Div { attrs, content } => match &attrs.id {
                // A labelled block, so in-document links can point at it.
                Some(id) => {
                    out.push_str("#block[\n");
                    self.write_blocks_impl(content, opts, out, ctx);
                    out.push_str(&format!("]{}\n\n", ctx.attach(id)));
                }
                None => self.write_blocks_impl(content, opts, out, ctx),
            },
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
        ctx: &Ctx,
    ) {
        for (k, inline) in inlines.iter().enumerate() {
            // `*` and `_` only delimit at word boundaries in Typst: `*Lo*w`
            // does not parse. Use the function form when glued to a word.
            let glued = out.chars().last().is_some_and(char::is_alphanumeric)
                || inlines.get(k + 1).is_some_and(starts_alphanumeric);
            match inline {
                Inline::Strong { content } if glued => {
                    out.push_str("#strong[");
                    self.write_inlines_impl(content, opts, out, ctx);
                    out.push(']');
                }
                Inline::Emphasis { content } if glued => {
                    out.push_str("#emph[");
                    self.write_inlines_impl(content, opts, out, ctx);
                    out.push(']');
                }
                // `@a-b` would read the `-b` as part of the label.
                Inline::CrossRef(cr)
                    if ctx.labels.contains(&cr.target)
                        && !ctx.unnumbered.contains_key(&cr.target)
                        && matches!(cr.form, RefForm::NumberWithType)
                        && inlines.get(k + 1).is_some_and(starts_label_char) =>
                {
                    out.push_str(&format!("#ref(<{}>)", typst_label(&cr.target)));
                }
                _ => self.write_inline_impl(inline, opts, out, ctx),
            }
        }
    }

    /// `_text_` or `*text*`. Typst only reads the marks next to non-space
    /// text, so spaces at either end of the content move outside them.
    fn write_delimited(
        &self,
        mark: char,
        content: &[Inline],
        opts: &WriteOptions,
        out: &mut String,
        ctx: &Ctx,
    ) {
        let mut inner = String::new();
        self.write_inlines_impl(content, opts, &mut inner, ctx);
        let trimmed = trim_markup(&inner);
        let start = inner.len() - inner.trim_start().len();
        let end = start + trimmed.len();
        if start >= end {
            if !inner.is_empty() {
                out.push(' ');
            }
            return;
        }
        if start > 0 {
            out.push(' ');
        }
        out.push(mark);
        out.push_str(&inner[start..end]);
        out.push(mark);
        if end < inner.len() {
            out.push(' ');
        }
    }

    fn write_inline_impl(&self, inline: &Inline, opts: &WriteOptions, out: &mut String, ctx: &Ctx) {
        match inline {
            Inline::Text { value } => {
                push_text(out, value);
            }
            Inline::SoftBreak => {
                // Two newlines in a row would end the paragraph, and with it
                // any `*` or `_` still open.
                out.push(if out.ends_with('\n') { ' ' } else { '\n' });
            }
            Inline::HardBreak => {
                out.push_str("\\ \n");
            }
            Inline::Emphasis { content } => self.write_delimited('_', content, opts, out, ctx),
            Inline::Strong { content } => self.write_delimited('*', content, opts, out, ctx),
            Inline::Strikethrough { content } => {
                out.push_str("#strike[");
                self.write_inlines_impl(content, opts, out, ctx);
                out.push(']');
            }
            Inline::Code { value, .. } => {
                if value.contains('`') || value.contains('\n') {
                    // A backtick inside would close the raw span early.
                    out.push_str(&format!("#raw(\"{}\")", escape_typst_url(value)));
                } else {
                    out.push('`');
                    out.push_str(value);
                    out.push('`');
                }
            }
            Inline::MathInline { value } => {
                out.push('$');
                out.push_str(value);
                out.push('$');
            }
            Inline::Link { url, content, .. } if url.trim().is_empty() => {
                // Typst rejects an empty URL; keep the text.
                self.write_inlines_impl(content, opts, out, ctx);
            }
            Inline::Link { url, content, .. } if url.starts_with('#') => {
                // In-document link: a label, or plain text if nothing has it.
                let target = &url[1..];
                if ctx.labels.contains(target) {
                    out.push_str(&format!("#link(<{}>)[", typst_label(target)));
                    self.write_inlines_impl(content, opts, out, ctx);
                    out.push(']');
                } else {
                    self.write_inlines_impl(content, opts, out, ctx);
                }
            }
            Inline::Link { url, content, .. } => {
                out.push_str(&format!("#link(\"{}\")", escape_typst_url(url)));
                if !content.is_empty() {
                    out.push('[');
                    self.write_inlines_impl(content, opts, out, ctx);
                    out.push(']');
                }
            }
            Inline::Image(img) => {
                out.push_str(&format!("#image(\"{}\"", escape_typst_url(&img.url)));
                let alt_text = img.alt_text();
                if !alt_text.is_empty() {
                    out.push_str(&format!(", alt: \"{}\"", escape_typst_url(&alt_text)));
                }
                out.push(')');
            }
            Inline::Superscript { content } => {
                out.push_str("#super[");
                self.write_inlines_impl(content, opts, out, ctx);
                out.push(']');
            }
            Inline::Subscript { content } => {
                out.push_str("#sub[");
                self.write_inlines_impl(content, opts, out, ctx);
                out.push(']');
            }
            Inline::SmallCaps { content } => {
                out.push_str("#smallcaps[");
                self.write_inlines_impl(content, opts, out, ctx);
                out.push(']');
            }
            Inline::Underline { content } => {
                out.push_str("#underline[");
                self.write_inlines_impl(content, opts, out, ctx);
                out.push(']');
            }
            Inline::Span { content, .. } => {
                self.write_inlines_impl(content, opts, out, ctx);
            }
            Inline::RawInline { format, content } => {
                if format == "typst" || format == "typ" {
                    out.push_str(content);
                }
            }
            Inline::Citation(cite) => {
                if let Some(prefix) = cite.items.first().and_then(|i| i.prefix.as_deref()) {
                    out.push_str(&escape_typst(prefix));
                    out.push(' ');
                }
                for (i, item) in cite.items.iter().enumerate() {
                    if i > 0 {
                        out.push(' ');
                    }
                    if ctx.labels.contains(&item.key) {
                        out.push('@');
                        out.push_str(&typst_label(&item.key));
                    } else {
                        // No bibliography entry to point at: keep the key visible.
                        out.push_str(&escape_typst(&format!("[{}]", item.key)));
                    }
                    if let Some(suffix) = &item.suffix {
                        out.push(' ');
                        out.push_str(&escape_typst(suffix));
                    }
                }
            }
            Inline::CrossRef(cr) => {
                let label = typst_label(&cr.target);
                if !ctx.labels.contains(&cr.target) {
                    // What LaTeX prints for an undefined reference.
                    out.push_str("??");
                    return;
                }
                if let Some(title) = ctx.unnumbered.get(&cr.target) {
                    out.push_str(&format!("#link(<{label}>)["));
                    self.write_inlines_impl(title, opts, out, ctx);
                    out.push(']');
                    return;
                }
                match &cr.form {
                    // `Table~\ref{t}`: the source already names the kind.
                    RefForm::Number | RefForm::Page => {
                        out.push_str(&format!("#ref(<{label}>, supplement: none)"))
                    }
                    RefForm::Custom(text) => out.push_str(&format!(
                        "#ref(<{label}>, supplement: [{}])",
                        escape_typst(text)
                    )),
                    RefForm::NumberWithType => {
                        out.push('@');
                        out.push_str(&label);
                    }
                }
            }
            Inline::FootnoteRef { id } => {
                if let Some(content) = ctx.footnotes.get(id) {
                    out.push_str("#footnote[");
                    let mut inner = String::new();
                    self.write_blocks_impl(content, opts, &mut inner, ctx);
                    out.push_str(trim_markup(&inner));
                    out.push(']');
                }
            }
            Inline::Quoted {
                quote_type,
                content,
            } => {
                let (open, close) = match quote_type {
                    QuoteType::SingleQuote => ('\u{2018}', '\u{2019}'),
                    QuoteType::DoubleQuote => ('\u{201C}', '\u{201D}'),
                };
                out.push(open);
                self.write_inlines_impl(content, opts, out, ctx);
                out.push(close);
            }
        }
    }

    // ── Table helper ─────────────────────────────────────────────────────

    fn write_table(&self, table: &Table, opts: &WriteOptions, out: &mut String, ctx: &Ctx) {
        let has_wrapper = table.caption.is_some() || table.label.is_some();
        if has_wrapper {
            out.push_str("#figure(\n");
            if let Some(cap) = &table.caption {
                out.push_str("  caption: [");
                self.write_inlines_impl(cap, opts, out, ctx);
                out.push_str("],\n");
            }
        }
        let widths = row_widths(table);
        let ncols = widths
            .iter()
            .copied()
            .chain([table.columns.len(), 1])
            .max()
            .unwrap_or(1);
        // Empty cells that complete each row, in the order rows are written.
        let mut padding = widths.iter().map(|w| ncols - w);
        let hash = if has_wrapper { "" } else { "#" };
        out.push_str(&format!("{hash}table(\n  columns: {ncols},\n"));

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

        let mut write_row = |row: &[TableCell], indent: &str, out: &mut String| {
            for cell in row {
                out.push_str(&cell_open(cell, indent));
                let mut cell_content = String::new();
                self.write_blocks_impl(&cell.content, opts, &mut cell_content, ctx);
                out.push_str(trim_markup(&cell_content));
                out.push_str("],\n");
            }
            for _ in 0..padding.next().unwrap_or(0) {
                out.push_str(indent);
                out.push_str("[],\n");
            }
        };

        if let Some(header) = &table.header {
            out.push_str("  table.header(\n");
            write_row(header, "    ", out);
            out.push_str("  ),\n");
        }

        for row in &table.rows {
            write_row(row, "  ", out);
        }

        if let Some(foot) = &table.foot {
            out.push_str("  table.footer(\n");
            write_row(foot, "    ", out);
            out.push_str("  ),\n");
        }

        out.push(')');
        if has_wrapper {
            out.push(')');
            if let Some(label) = &table.label {
                out.push_str(&ctx.attach(label));
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
                }
            }
            preamble.push_str(")\n\n");
            preamble.push_str(&title_block(doc));
        }

        if let Some(abstract_blocks) = &doc.metadata.abstract_text {
            preamble.push_str("#quote(block: true)[\n");
            let opts_default = WriteOptions::default();
            self.write_blocks_impl(
                abstract_blocks,
                &opts_default,
                &mut preamble,
                &Ctx::default(),
            );
            preamble.push_str("]\n\n");
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
        let mut labels = Vec::new();
        collect_labels(&doc.content, &mut labels);
        let ctx = Ctx {
            footnotes: collect_footnotes(&doc.content),
            labels: labels.iter().map(|(_, l)| l.clone()).collect(),
            unnumbered: unnumbered_headings(&doc.content),
            attached: RefCell::default(),
        };
        let mut body = String::with_capacity(4096);
        body.push_str(&numbering_rules(&doc.content, &labels));
        self.write_blocks_impl(&doc.content, opts, &mut body, &ctx);

        if opts.standalone {
            Ok(self.wrap_standalone(&body, doc))
        } else {
            Ok(body)
        }
    }
}

/// How many columns each row fills once column and row spans are laid out,
/// in order: header, body rows, footer. Typst places cells one after another
/// and wraps on its own, so short rows must be padded and the column count
/// must cover the widest row: papers often declare fewer columns than they use.
fn row_widths(table: &Table) -> Vec<usize> {
    let rows = table
        .header
        .iter()
        .chain(table.rows.iter())
        .chain(table.foot.iter());
    // Rows still covered by a rowspan from above, per column.
    let mut covered: Vec<u32> = Vec::new();
    let mut widths = Vec::new();
    for row in rows {
        let mut used = covered.iter().filter(|&&n| n > 0).count();
        let mut col = 0;
        for cell in row {
            while covered.get(col).is_some_and(|&n| n > 0) {
                col += 1;
            }
            let span = cell.colspan.max(1) as usize;
            if covered.len() < col + span {
                covered.resize(col + span, 0);
            }
            for c in &mut covered[col..col + span] {
                // Counted down below, so this row's own cells end at zero.
                *c = cell.rowspan.max(1);
            }
            col += span;
            used += span;
        }
        widths.push(used.max(col));
        for c in &mut covered {
            *c = c.saturating_sub(1);
        }
    }
    widths
}

/// Opening of a table cell: `[` or `table.cell(colspan: 2)[`.
fn cell_open(cell: &TableCell, indent: &str) -> String {
    let mut spans = Vec::new();
    if cell.colspan > 1 {
        spans.push(format!("colspan: {}", cell.colspan));
    }
    if cell.rowspan > 1 {
        spans.push(format!("rowspan: {}", cell.rowspan));
    }
    if spans.is_empty() {
        format!("{indent}[")
    } else {
        format!("{indent}table.cell({})[", spans.join(", "))
    }
}

/// The visible title, authors and date, centered like LaTeX's \\maketitle.
/// (`#set document` only fills the PDF metadata.)
fn title_block(doc: &Document) -> String {
    let meta = &doc.metadata;
    let mut lines = Vec::new();
    if let Some(title) = &meta.title {
        lines.push(format!(
            "#text(size: 1.6em, weight: \"bold\")[{}]",
            escape_typst(title)
        ));
    }
    if !meta.authors.is_empty() {
        let names: Vec<String> = meta.authors.iter().map(|a| escape_typst(&a.name)).collect();
        lines.push(names.join(", "));
    }
    if let Some(date) = &meta.date {
        lines.push(escape_typst(date));
    }
    if lines.is_empty() {
        return String::new();
    }
    format!("#align(center)[\n{}\n]\n\n", lines.join(" \\\n"))
}

/// Written markup without surrounding whitespace, except the space of a
/// trailing `\ ` line break: without it the `\` would escape whatever
/// closes the content next (`]`, `*`, `_`).
fn trim_markup(s: &str) -> &str {
    let trimmed = s.trim();
    let backslashes = trimmed.chars().rev().take_while(|&c| c == '\\').count();
    if backslashes % 2 == 1 {
        let start = s.len() - s.trim_start().len();
        &s[start..start + trimmed.len() + 1]
    } else {
        trimmed
    }
}

fn starts_label_char(inline: &Inline) -> bool {
    matches!(inline, Inline::Text { value } if value.starts_with(|c: char| c.is_alphanumeric() || c == '_' || c == '-'))
}

fn starts_alphanumeric(inline: &Inline) -> bool {
    match inline {
        Inline::Text { value } => value.chars().next().is_some_and(char::is_alphanumeric),
        // A span is written as its content, with nothing in front.
        Inline::Span { content, .. } => content.first().is_some_and(starts_alphanumeric),
        _ => false,
    }
}

// ─── Numbering ───────────────────────────────────────────────────────────────

/// Typst refuses `@label` on headings and equations unless they are numbered.
/// Turn numbering on for the kinds of element the document cross-references.
fn numbering_rules(blocks: &[Block], labels: &[(LabelKind, String)]) -> String {
    let mut targets = Vec::new();
    collect_ref_targets(blocks, &mut targets);
    let referenced = |kind: LabelKind| {
        labels
            .iter()
            .any(|(k, l)| *k == kind && targets.contains(l))
    };
    let mut rules = String::new();
    if referenced(LabelKind::Heading) {
        // Number three levels like LaTeX's article; `\\paragraph` stays unnumbered.
        rules.push_str(
            "#set heading(numbering: (..n) => if n.pos().len() <= 3 { numbering(\"1.1\", ..n) })\n",
        );
    }
    if referenced(LabelKind::Equation) {
        rules.push_str("#set math.equation(numbering: \"(1)\")\n");
    }
    if !rules.is_empty() {
        rules.push('\n');
    }
    rules
}

#[derive(Clone, Copy, PartialEq)]
enum LabelKind {
    Heading,
    Equation,
    Other,
}

fn unnumbered_headings(blocks: &[Block]) -> HashMap<String, Vec<Inline>> {
    fn walk(blocks: &[Block], out: &mut HashMap<String, Vec<Inline>>) {
        for block in blocks {
            match block {
                Block::Heading {
                    id: Some(id),
                    content,
                    attrs: Some(attrs),
                    ..
                } if attrs.classes.iter().any(|c| c == "unnumbered") => {
                    out.insert(id.clone(), content.clone());
                }
                Block::Div { content, .. }
                | Block::BlockQuote { content }
                | Block::Admonition { content, .. } => walk(content, out),
                Block::List { items, .. } => {
                    for item in items {
                        walk(&item.content, out);
                    }
                }
                _ => {}
            }
        }
    }
    let mut out = HashMap::new();
    walk(blocks, &mut out);
    out
}

/// Every label the writer emits, with the kind of element it is attached to.
fn collect_labels(blocks: &[Block], out: &mut Vec<(LabelKind, String)>) {
    for block in blocks {
        match block {
            Block::Heading { id: Some(id), .. } => out.push((LabelKind::Heading, id.clone())),
            Block::MathBlock {
                label: Some(label), ..
            } => out.push((LabelKind::Equation, label.clone())),
            Block::Figure {
                label: Some(label), ..
            }
            | Block::CodeBlock {
                label: Some(label), ..
            } => out.push((LabelKind::Other, label.clone())),
            Block::Table(table) => {
                if let Some(label) = &table.label {
                    out.push((LabelKind::Other, label.clone()));
                }
            }
            Block::Div { attrs, content } => {
                if let Some(id) = &attrs.id {
                    out.push((LabelKind::Other, id.clone()));
                }
                collect_labels(content, out);
            }
            Block::BlockQuote { content } | Block::Admonition { content, .. } => {
                collect_labels(content, out)
            }
            Block::List { items, .. } => {
                for item in items {
                    collect_labels(&item.content, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_ref_targets(blocks: &[Block], out: &mut Vec<String>) {
    for block in blocks {
        match block {
            Block::Paragraph { content } | Block::Heading { content, .. } => {
                collect_inline_targets(content, out)
            }
            Block::BlockQuote { content }
            | Block::Div { content, .. }
            | Block::Admonition { content, .. }
            | Block::FootnoteDef { content, .. } => collect_ref_targets(content, out),
            Block::List { items, .. } => {
                for item in items {
                    collect_ref_targets(&item.content, out);
                }
            }
            Block::Figure {
                caption: Some(caption),
                ..
            } => collect_inline_targets(caption, out),
            Block::Table(table) => {
                let cells = table
                    .header
                    .iter()
                    .flatten()
                    .chain(table.rows.iter().flatten());
                for cell in cells {
                    collect_ref_targets(&cell.content, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_inline_targets(inlines: &[Inline], out: &mut Vec<String>) {
    for inline in inlines {
        match inline {
            Inline::CrossRef(cr) => out.push(cr.target.clone()),
            Inline::Emphasis { content }
            | Inline::Strong { content }
            | Inline::Strikethrough { content }
            | Inline::Underline { content }
            | Inline::Superscript { content }
            | Inline::Subscript { content }
            | Inline::SmallCaps { content }
            | Inline::Link { content, .. }
            | Inline::Span { content, .. }
            | Inline::Quoted { content, .. } => collect_inline_targets(content, out),
            _ => {}
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Escape Typst special characters: \\ * _ ` $ # @ < > [ ] and the `//` / `/*`
/// comment openers.
pub fn escape_typst(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' | '*' | '_' | '`' | '$' | '#' | '@' | '<' | '>' | '[' | ']' => {
                out.push('\\');
                out.push(c);
            }
            '/' if matches!(chars.peek(), Some('/' | '*')) => out.push_str("\\/"),
            _ => out.push(c),
        }
    }
    out
}

/// Append escaped text, also escaping markers that Typst only interprets at
/// the start of a line (`- `, `+ `, `= `, `/ `, `1. `).
fn push_text(out: &mut String, value: &str) {
    let mut escaped = escape_typst(value);
    // `/` next to the `*` of strong text, or to another `/`, opens or closes
    // a comment: `*a*/*b*`. Within one text node `escape_typst` handles it.
    if escaped.starts_with('/') && (out.ends_with('*') || out.ends_with('/')) {
        escaped.insert(0, '\\');
    }
    if escaped.ends_with('/') && !escaped.ends_with("\\/") {
        escaped.insert(escaped.len() - 1, '\\');
    }
    if out.ends_with(']') || out.ends_with(')') {
        // Right after an embedded expression (`#link(..)[..]`), `;` ends the
        // expression, `(` and `[` continue the call, `.x` is a field access
        // (the `x` may come in the next text node).
        let mut chars = escaped.chars();
        let continues = match (chars.next(), chars.next()) {
            (Some(';' | '(' | '['), _) => true,
            (Some('.'), None) => true,
            (Some('.'), Some(c)) => c.is_alphanumeric(),
            _ => false,
        };
        if continues {
            out.push('\\');
        }
    }
    if !(out.is_empty() || out.ends_with('\n')) {
        out.push_str(&escaped);
        return;
    }
    match line_marker_pos(&escaped) {
        Some(pos) => {
            out.push_str(&escaped[..pos]);
            out.push('\\');
            out.push_str(&escaped[pos..]);
        }
        None => out.push_str(&escaped),
    }
}

/// Byte offset of the character that would turn this line into a list item,
/// heading or term (the `-`, `+`, `=`, `/` or the `.` after digits).
fn line_marker_pos(s: &str) -> Option<usize> {
    let indent = s.len() - s.trim_start_matches(' ').len();
    let rest = &s[indent..];
    let marker_end = |i: usize| matches!(rest[i..].chars().next(), None | Some(' ' | '\n'));
    if rest.starts_with(['-', '+', '=', '/']) && marker_end(1) {
        return Some(indent);
    }
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    (digits > 0 && rest[digits..].starts_with('.') && marker_end(digits + 1))
        .then_some(indent + digits)
}

/// Turn an arbitrary identifier into a valid Typst label / reference name.
/// Typst labels allow letters, digits, `_`, `-`, `:` and `.`; anything else
/// becomes `-`. Applied to both definitions and references so they match.
pub fn typst_label(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '_' | '-' | ':' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect()
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
                        attrs: None,
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
                        attrs: None,
                    },
                    Inline::text(" "),
                    Inline::Image(Image {
                        url: "photo.png".into(),
                        alt: vec![Inline::text("A photo")],
                        title: None,
                        attrs: None,
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
    fn table() {
        let doc = Document {
            content: vec![Block::Table(Table {
                caption: Some(vec![Inline::text("Results")]),
                label: Some("tab:results".into()),
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
        let typ = write_typst(&doc);
        // Inside `#figure(` we are in code mode: `table(` must not carry `#`.
        assert!(typ.contains("#figure(\n  caption: [Results],\ntable("));
        assert!(!typ.contains("#table("));
        assert!(typ.contains("columns: 2"));
        assert!(typ.contains("[Name]"));
        assert!(typ.contains("[Pi]"));
        assert!(typ.contains("<tab:results>"));
    }

    #[test]
    fn figure_with_caption() {
        let doc = Document {
            content: vec![Block::Figure {
                image: Image {
                    url: "diagram.png".into(),
                    alt: vec![Inline::text("Architecture")],
                    title: None,
                    attrs: None,
                },
                caption: Some(vec![Inline::text("System architecture")]),
                label: Some("fig:arch".into()),
                attrs: None,
            }],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains("#figure("));
        assert!(typ.contains("image(\"diagram.png\")"));
        assert!(typ.contains("caption: [System architecture]"));
        assert!(typ.contains("<fig:arch>"));
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
                abstract_text: Some(vec![Block::text("This paper is about things.")]),
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

    fn cell(text: &str, colspan: u32, rowspan: u32) -> TableCell {
        TableCell {
            content: vec![Block::text(text)],
            colspan,
            rowspan,
        }
    }

    #[test]
    fn table_is_as_wide_as_its_spanned_cells() {
        // Two declared columns, but the second row needs three.
        let table = Table {
            caption: None,
            label: None,
            columns: vec![],
            header: Some(vec![cell("a", 1, 2), cell("b", 1, 1)]),
            rows: vec![vec![cell("c", 2, 1)]],
            foot: None,
            attrs: None,
        };
        assert_eq!(row_widths(&table), vec![2, 3]);
    }

    #[test]
    fn short_rows_are_padded_so_later_cells_stay_in_place() {
        let table = Table {
            caption: None,
            label: None,
            columns: vec![],
            header: None,
            rows: vec![
                vec![cell("a", 1, 1), cell("b", 1, 1), cell("c", 1, 1)],
                vec![cell("d", 1, 1)],
                vec![cell("e", 3, 1)],
            ],
            foot: None,
            attrs: None,
        };
        let doc = Document {
            content: vec![Block::Table(table)],
            ..Default::default()
        };
        let out = write_typst(&doc);
        assert!(
            out.contains("[d],\n  [],\n  [],\n  table.cell(colspan: 3)[e]"),
            "{out}"
        );
    }

    #[test]
    fn a_cell_ending_in_a_line_break_still_closes() {
        let table = Table {
            caption: None,
            label: None,
            columns: vec![],
            header: None,
            rows: vec![vec![TableCell {
                content: vec![Block::Paragraph {
                    content: vec![Inline::text("Buy"), Inline::HardBreak],
                }],
                colspan: 1,
                rowspan: 1,
            }]],
            foot: None,
            attrs: None,
        };
        let doc = Document {
            content: vec![Block::Table(table)],
            ..Default::default()
        };
        assert!(write_typst(&doc).contains("[Buy\\ ],"));
    }

    #[test]
    fn a_repeated_label_is_attached_once() {
        let eq = |content: &str| Block::MathBlock {
            content: content.into(),
            label: Some("eq:x".into()),
        };
        let doc = Document {
            content: vec![eq("a"), eq("b")],
            ..Default::default()
        };
        assert_eq!(write_typst(&doc).matches("<eq:x>").count(), 1);
    }

    #[test]
    fn heading_with_a_line_break_keeps_its_label() {
        let doc = Document {
            content: vec![Block::Heading {
                level: 1,
                id: Some("sec:a".into()),
                content: vec![
                    Inline::text("First"),
                    Inline::HardBreak,
                    Inline::text("second"),
                ],
                attrs: None,
            }],
            ..Default::default()
        };
        let out = write_typst(&doc);
        assert!(
            out.lines()
                .any(|l| l.starts_with("= First") && l.ends_with("second <sec:a>")),
            "{out}"
        );
    }

    #[test]
    fn slash_between_strong_runs_is_not_a_comment() {
        let strong = |t: &str| Inline::Strong {
            content: vec![Inline::text(t)],
        };
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![strong("1"), Inline::text("/"), strong("2")],
            }],
            ..Default::default()
        };
        assert_eq!(write_typst(&doc).trim(), r"*1*\/*2*");
    }

    #[test]
    fn reference_to_an_unnumbered_heading_links_to_its_title() {
        let doc = Document {
            content: vec![
                Block::Heading {
                    level: 1,
                    id: Some("sec:safety".into()),
                    content: vec![Inline::text("Safety")],
                    attrs: Some(Attributes {
                        id: None,
                        classes: vec!["unnumbered".into()],
                        key_values: HashMap::new(),
                    }),
                },
                Block::Paragraph {
                    content: vec![
                        Inline::text("See "),
                        Inline::CrossRef(CrossRef {
                            target: "sec:safety".into(),
                            form: RefForm::NumberWithType,
                        }),
                    ],
                },
            ],
            ..Default::default()
        };
        assert!(write_typst(&doc).contains("See #link(<sec:safety>)[Safety]"));
    }

    #[test]
    fn emphasis_marks_hug_the_text() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("A "),
                    Inline::Emphasis {
                        content: vec![Inline::text(" graph ")],
                    },
                    Inline::text(", or"),
                ],
            }],
            ..Default::default()
        };
        assert!(write_typst(&doc).contains(" _graph_ "));
    }

    #[test]
    fn strong_text_never_contains_a_blank_line() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Strong {
                    content: vec![
                        Inline::text("Dear"),
                        Inline::SoftBreak,
                        Inline::SoftBreak,
                        Inline::text("recruiter"),
                    ],
                }],
            }],
            ..Default::default()
        };
        assert!(!write_typst(&doc).trim().contains("\n\n"));
    }

    #[test]
    fn bare_table_keeps_hash() {
        let doc = Document {
            content: vec![Block::Table(Table {
                caption: None,
                label: None,
                columns: vec![],
                header: None,
                rows: vec![vec![TableCell {
                    content: vec![Block::text("x")],
                    colspan: 1,
                    rowspan: 1,
                }]],
                foot: None,
                attrs: None,
            })],
            ..Default::default()
        };
        assert!(write_typst(&doc).starts_with("#table("));
    }

    #[test]
    fn labels_with_spaces_are_sanitized_on_both_ends() {
        let doc = Document {
            content: vec![
                Block::Figure {
                    image: Image {
                        url: "a.png".into(),
                        alt: vec![],
                        title: None,
                        attrs: None,
                    },
                    caption: None,
                    label: Some("figure:eval strategies".into()),
                    attrs: None,
                },
                Block::Paragraph {
                    content: vec![Inline::CrossRef(CrossRef {
                        target: "figure:eval strategies".into(),
                        form: RefForm::NumberWithType,
                    })],
                },
            ],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains("<figure:eval-strategies>"));
        assert!(typ.contains("@figure:eval-strategies"));
    }

    #[test]
    fn dangling_references_are_written_as_text() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::CrossRef(CrossRef {
                        target: "sec:missing".into(),
                        form: RefForm::Number,
                    }),
                    Inline::text(" "),
                    Inline::Citation(Citation {
                        items: vec![CiteItem {
                            key: "smith2020".into(),
                            prefix: None,
                            suffix: None,
                        }],
                        mode: CitationMode::Normal,
                    }),
                ],
            }],
            ..Default::default()
        };
        assert_eq!(write_typst(&doc).trim(), "?? \\[smith2020\\]");
    }

    #[test]
    fn brackets_and_comment_openers_are_escaped() {
        assert_eq!(escape_typst("a ] b [c"), "a \\] b \\[c");
        assert_eq!(escape_typst("x // y /* z"), "x \\// y \\/\\* z");
        assert_eq!(escape_typst("http:/x a/b"), "http:/x a/b");
    }

    #[test]
    fn line_start_markers_are_escaped() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("intro"),
                    Inline::SoftBreak,
                    Inline::text("- not a list, 2. not an enum"),
                    Inline::SoftBreak,
                    Inline::text("12. also not"),
                    Inline::SoftBreak,
                    Inline::text("= not a heading"),
                    Inline::SoftBreak,
                    Inline::text("-5 degrees is fine"),
                ],
            }],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains("\n\\- not a list, 2. not an enum\n"));
        assert!(typ.contains("\n12\\. also not\n"));
        assert!(typ.contains("\n\\= not a heading\n"));
        assert!(typ.contains("\n-5 degrees is fine"));
    }

    #[test]
    fn referenced_headings_and_equations_get_numbering() {
        let doc = Document {
            content: vec![
                Block::Heading {
                    level: 1,
                    id: Some("sec:intro".into()),
                    content: vec![Inline::text("Intro")],
                    attrs: None,
                },
                Block::MathBlock {
                    content: "x".into(),
                    label: Some("eq:x".into()),
                },
                Block::Paragraph {
                    content: vec![
                        Inline::CrossRef(CrossRef {
                            target: "sec:intro".into(),
                            form: RefForm::Number,
                        }),
                        Inline::CrossRef(CrossRef {
                            target: "eq:x".into(),
                            form: RefForm::Number,
                        }),
                    ],
                },
            ],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.starts_with(
            "#set heading(numbering: (..n) => if n.pos().len() <= 3 { numbering(\"1.1\", ..n) })\n#set math.equation(numbering: \"(1)\")\n"
        ));
    }

    #[test]
    fn unreferenced_document_has_no_numbering_rules() {
        let doc = Document {
            content: vec![Block::Heading {
                level: 1,
                id: Some("sec:intro".into()),
                content: vec![Inline::text("Intro")],
                attrs: None,
            }],
            ..Default::default()
        };
        assert!(write_typst(&doc).starts_with("= Intro"));
    }

    #[test]
    fn figure_without_image_file() {
        let doc = Document {
            content: vec![Block::Figure {
                image: Image {
                    url: String::new(),
                    alt: vec![],
                    title: None,
                    attrs: None,
                },
                caption: Some(vec![Inline::text("A TikZ drawing")]),
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains("#figure(\n  [],\n"));
        assert!(!typ.contains("image(\"\")"));
    }

    #[test]
    fn inline_code_with_backtick_uses_raw_function() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Code {
                    value: "`She' refers".into(),
                    attrs: None,
                }],
            }],
            ..Default::default()
        };
        assert_eq!(write_typst(&doc).trim(), "#raw(\"`She' refers\")");
    }

    #[test]
    fn strong_glued_to_a_word_uses_function_form() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::Strong {
                        content: vec![Inline::text("Lo")],
                    },
                    Inline::text("w-"),
                    Inline::Strong {
                        content: vec![Inline::text("R")],
                    },
                    Inline::text("ank and "),
                    Inline::Strong {
                        content: vec![Inline::text("free")],
                    },
                    Inline::text(" text"),
                ],
            }],
            ..Default::default()
        };
        assert_eq!(
            write_typst(&doc).trim(),
            "#strong[Lo]w-#strong[R]ank and *free* text"
        );
    }

    #[test]
    fn reference_followed_by_label_char_uses_function_form() {
        let doc = Document {
            content: vec![
                Block::Heading {
                    level: 2,
                    id: Some("sec:a".into()),
                    content: vec![Inline::text("A")],
                    attrs: None,
                },
                Block::Paragraph {
                    content: vec![
                        Inline::CrossRef(CrossRef {
                            target: "sec:a".into(),
                            form: RefForm::NumberWithType,
                        }),
                        Inline::text("-"),
                        Inline::CrossRef(CrossRef {
                            target: "sec:a".into(),
                            form: RefForm::NumberWithType,
                        }),
                        Inline::text(" below."),
                    ],
                },
            ],
            ..Default::default()
        };
        assert!(write_typst(&doc).contains("#ref(<sec:a>)-@sec:a below."));
    }

    #[test]
    fn references_section_with_internal_links() {
        let entry = Block::Div {
            attrs: Attributes {
                id: Some("ref-kingma".into()),
                classes: vec!["csl-entry".into()],
                key_values: HashMap::new(),
            },
            content: vec![Block::text("Kingma and Ba. Adam.")],
        };
        let doc = Document {
            content: vec![
                Block::Paragraph {
                    content: vec![
                        Inline::Link {
                            url: "#ref-kingma".into(),
                            title: None,
                            content: vec![Inline::text("[1]")],
                            attrs: None,
                        },
                        Inline::text(" "),
                        Inline::Link {
                            url: "#nowhere".into(),
                            title: None,
                            content: vec![Inline::text("x")],
                            attrs: None,
                        },
                    ],
                },
                Block::Heading {
                    level: 1,
                    id: Some("references".into()),
                    content: vec![Inline::text("References")],
                    attrs: Some(Attributes {
                        id: None,
                        classes: vec!["unnumbered".into()],
                        key_values: HashMap::new(),
                    }),
                },
                entry,
            ],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert!(typ.contains("#link(<ref-kingma>)[\\[1\\]] x"));
        assert!(typ.contains("#heading(level: 1, numbering: none)[References] <references>"));
        assert!(typ.contains("#block[\nKingma and Ba. Adam.\n\n] <ref-kingma>"));
    }

    #[test]
    fn text_after_embedded_expression_does_not_continue_it() {
        let link = |t: &str| Inline::Link {
            url: "https://a.org".into(),
            title: None,
            content: vec![Inline::text(t)],
            attrs: None,
        };
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    link("a"),
                    Inline::text("; "),
                    link("b"),
                    Inline::text("(2018). Next"),
                ],
            }],
            ..Default::default()
        };
        assert_eq!(
            write_typst(&doc).trim(),
            "#link(\"https://a.org\")[a]\\; #link(\"https://a.org\")[b]\\(2018). Next"
        );
    }

    #[test]
    fn plain_reference_omits_the_kind() {
        let doc = Document {
            content: vec![
                Block::Heading {
                    level: 1,
                    id: Some("sec:a".into()),
                    content: vec![Inline::text("A")],
                    attrs: None,
                },
                Block::Paragraph {
                    content: vec![
                        Inline::text("Section "),
                        Inline::CrossRef(CrossRef {
                            target: "sec:a".into(),
                            form: RefForm::Number,
                        }),
                    ],
                },
            ],
            ..Default::default()
        };
        assert!(write_typst(&doc).contains("Section #ref(<sec:a>, supplement: none)"));
    }
}
