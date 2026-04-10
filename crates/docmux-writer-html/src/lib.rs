//! # docmux-writer-html
//!
//! HTML5 writer for docmux. Converts the docmux AST into semantic HTML.

use base64::Engine;
use docmux_ast::*;
use docmux_core::{MathEngine, Result, WriteOptions, Writer};
use docmux_highlight::LineOptions;

/// An HTML5 writer.
#[derive(Debug, Default)]
pub struct HtmlWriter;

impl HtmlWriter {
    pub fn new() -> Self {
        Self
    }

    fn write_blocks(
        &self,
        blocks: &[Block],
        opts: &WriteOptions,
        doc: &Document,
        out: &mut String,
    ) {
        for block in blocks {
            self.write_block(block, opts, doc, out);
        }
    }

    fn write_block(&self, block: &Block, opts: &WriteOptions, doc: &Document, out: &mut String) {
        match block {
            Block::Paragraph { content } => {
                out.push_str("<p>");
                self.write_inlines(content, opts, doc, out);
                out.push_str("</p>\n");
            }
            Block::Heading {
                level, id, content, ..
            } => {
                let tag = format!("h{}", level.min(&6));
                if let Some(id) = id {
                    out.push_str(&format!("<{tag} id=\"{}\">", escape_attr(id)));
                } else {
                    out.push_str(&format!("<{tag}>"));
                }
                self.write_inlines(content, opts, doc, out);
                out.push_str(&format!("</{tag}>\n"));
            }
            Block::CodeBlock {
                language,
                content,
                attrs,
                ..
            } => {
                let line_opts = LineOptions::from_attrs(attrs.as_ref());
                if let (Some(lang), Some(theme)) =
                    (language.as_deref(), opts.highlight_style.as_deref())
                {
                    if let Ok(lines) = docmux_highlight::highlight(content, lang, theme) {
                        out.push_str(&format!(
                            "<pre><code class=\"language-{}\">",
                            escape_html(lang)
                        ));
                        write_highlighted_lines(out, &lines, &line_opts);
                        out.push_str("</code></pre>\n");
                    } else {
                        // Highlight failed (unknown lang, etc.) — fall back to plain
                        out.push_str(&format!(
                            "<pre><code class=\"language-{}\">",
                            escape_html(lang)
                        ));
                        write_plain_lines(out, content, &line_opts);
                        out.push_str("</code></pre>\n");
                    }
                } else if let Some(lang) = language {
                    out.push_str(&format!(
                        "<pre><code class=\"language-{}\">",
                        escape_html(lang)
                    ));
                    write_plain_lines(out, content, &line_opts);
                    out.push_str("</code></pre>\n");
                } else {
                    out.push_str("<pre><code>");
                    write_plain_lines(out, content, &line_opts);
                    out.push_str("</code></pre>\n");
                }
            }
            Block::MathBlock { content, label } => match opts.math_engine {
                MathEngine::MathML => {
                    if let Some(label) = label {
                        out.push_str(&format!("<div id=\"{}\">", escape_attr(label)));
                    }
                    out.push_str(content); // already MathML from transform
                    if label.is_some() {
                        out.push_str("</div>");
                    }
                    out.push('\n');
                }
                _ => {
                    let class = match opts.math_engine {
                        MathEngine::KaTeX => "math math-display",
                        MathEngine::MathJax => "math math-display",
                        MathEngine::MathML | MathEngine::Raw => "math",
                    };
                    if let Some(label) = label {
                        out.push_str(&format!(
                            "<div class=\"{class}\" id=\"{}\">",
                            escape_attr(label)
                        ));
                    } else {
                        out.push_str(&format!("<div class=\"{class}\">"));
                    }
                    out.push_str(&escape_html(content));
                    out.push_str("</div>\n");
                }
            },
            Block::BlockQuote { content } => {
                out.push_str("<blockquote>\n");
                self.write_blocks(content, opts, doc, out);
                out.push_str("</blockquote>\n");
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
                            out.push_str(&format!("<ol start=\"{s}\">\n"));
                        } else {
                            out.push_str("<ol>\n");
                        }
                    } else {
                        out.push_str("<ol>\n");
                    }
                } else {
                    out.push_str("<ul>\n");
                }
                for item in items {
                    if let Some(checked) = item.checked {
                        let check = if checked {
                            "<input type=\"checkbox\" checked disabled> "
                        } else {
                            "<input type=\"checkbox\" disabled> "
                        };
                        out.push_str(&format!("<li class=\"task-list-item\">{check}"));
                    } else {
                        out.push_str("<li>");
                    }
                    self.write_blocks(&item.content, opts, doc, out);
                    out.push_str("</li>\n");
                }
                if *ordered {
                    out.push_str("</ol>\n");
                } else {
                    out.push_str("</ul>\n");
                }
            }
            Block::Table(table) => {
                self.write_table(table, opts, doc, out);
            }
            Block::Figure {
                image,
                caption,
                label,
                ..
            } => {
                if let Some(label) = label {
                    out.push_str(&format!("<figure id=\"{}\">", escape_attr(label)));
                } else {
                    out.push_str("<figure>");
                }
                let src = image_src(&image.url, doc);
                out.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\">",
                    escape_attr(&src),
                    escape_attr(&image.alt_text())
                ));
                if let Some(cap) = caption {
                    out.push_str("<figcaption>");
                    self.write_inlines(cap, opts, doc, out);
                    out.push_str("</figcaption>");
                }
                out.push_str("</figure>\n");
            }
            Block::ThematicBreak => {
                out.push_str("<hr>\n");
            }
            Block::RawBlock { format, content } => {
                if format == "html" {
                    out.push_str(content);
                    out.push('\n');
                }
                // Non-HTML raw blocks are silently dropped
            }
            Block::Admonition {
                kind,
                title,
                content,
            } => {
                let class = match kind {
                    AdmonitionKind::Note => "admonition note",
                    AdmonitionKind::Warning => "admonition warning",
                    AdmonitionKind::Tip => "admonition tip",
                    AdmonitionKind::Important => "admonition important",
                    AdmonitionKind::Caution => "admonition caution",
                    AdmonitionKind::Custom(c) => {
                        out.push_str(&format!("<aside class=\"admonition {}\">", escape_attr(c)));
                        if let Some(t) = title {
                            out.push_str("<p class=\"admonition-title\">");
                            self.write_inlines(t, opts, doc, out);
                            out.push_str("</p>");
                        }
                        self.write_blocks(content, opts, doc, out);
                        out.push_str("</aside>\n");
                        return;
                    }
                };
                out.push_str(&format!("<aside class=\"{class}\">"));
                if let Some(t) = title {
                    out.push_str("<p class=\"admonition-title\">");
                    self.write_inlines(t, opts, doc, out);
                    out.push_str("</p>");
                }
                self.write_blocks(content, opts, doc, out);
                out.push_str("</aside>\n");
            }
            Block::DefinitionList { items } => {
                out.push_str("<dl>\n");
                for item in items {
                    out.push_str("<dt>");
                    self.write_inlines(&item.term, opts, doc, out);
                    out.push_str("</dt>\n");
                    for def in &item.definitions {
                        out.push_str("<dd>");
                        self.write_blocks(def, opts, doc, out);
                        out.push_str("</dd>\n");
                    }
                }
                out.push_str("</dl>\n");
            }
            Block::Div { attrs, content } => {
                let mut attr_str = String::new();
                if let Some(id) = &attrs.id {
                    attr_str.push_str(&format!(" id=\"{}\"", escape_attr(id)));
                }
                if !attrs.classes.is_empty() {
                    attr_str.push_str(&format!(" class=\"{}\"", attrs.classes.join(" ")));
                }
                for (k, v) in &attrs.key_values {
                    attr_str.push_str(&format!(" data-{}=\"{}\"", escape_attr(k), escape_attr(v)));
                }
                out.push_str(&format!("<div{attr_str}>\n"));
                self.write_blocks(content, opts, doc, out);
                out.push_str("</div>\n");
            }
            Block::FootnoteDef { id, content } => {
                out.push_str(&format!(
                    "<aside id=\"fn-{}\" class=\"footnote\" role=\"note\">\n",
                    escape_attr(id)
                ));
                self.write_blocks(content, opts, doc, out);
                out.push_str("</aside>\n");
            }
        }
    }

    fn write_table(&self, table: &Table, opts: &WriteOptions, doc: &Document, out: &mut String) {
        if let Some(label) = &table.label {
            out.push_str(&format!("<table id=\"{}\">\n", escape_attr(label)));
        } else {
            out.push_str("<table>\n");
        }

        if let Some(cap) = &table.caption {
            out.push_str("<caption>");
            self.write_inlines(cap, opts, doc, out);
            out.push_str("</caption>\n");
        }

        if let Some(header) = &table.header {
            out.push_str("<thead>\n<tr>");
            for (i, cell) in header.iter().enumerate() {
                let align = table
                    .columns
                    .get(i)
                    .map(|c| &c.alignment)
                    .unwrap_or(&Alignment::Default);
                self.write_th(cell, align, opts, doc, out);
            }
            out.push_str("</tr>\n</thead>\n");
        }

        out.push_str("<tbody>\n");
        for row in &table.rows {
            out.push_str("<tr>");
            for (i, cell) in row.iter().enumerate() {
                let align = table
                    .columns
                    .get(i)
                    .map(|c| &c.alignment)
                    .unwrap_or(&Alignment::Default);
                self.write_td(cell, align, opts, doc, out);
            }
            out.push_str("</tr>\n");
        }
        out.push_str("</tbody>\n");

        if let Some(foot) = &table.foot {
            out.push_str("<tfoot>\n<tr>");
            for (i, cell) in foot.iter().enumerate() {
                let align = table
                    .columns
                    .get(i)
                    .map(|c| &c.alignment)
                    .unwrap_or(&Alignment::Default);
                self.write_td(cell, align, opts, doc, out);
            }
            out.push_str("</tr>\n</tfoot>\n");
        }

        out.push_str("</table>\n");
    }

    fn write_th(
        &self,
        cell: &TableCell,
        align: &Alignment,
        opts: &WriteOptions,
        doc: &Document,
        out: &mut String,
    ) {
        let mut attrs = String::new();
        if cell.colspan > 1 {
            attrs.push_str(&format!(" colspan=\"{}\"", cell.colspan));
        }
        if cell.rowspan > 1 {
            attrs.push_str(&format!(" rowspan=\"{}\"", cell.rowspan));
        }
        if !matches!(align, Alignment::Default) {
            attrs.push_str(&format!(" style=\"text-align: {}\"", alignment_css(align)));
        }
        out.push_str(&format!("<th{attrs}>"));
        self.write_blocks(&cell.content, opts, doc, out);
        out.push_str("</th>");
    }

    fn write_td(
        &self,
        cell: &TableCell,
        align: &Alignment,
        opts: &WriteOptions,
        doc: &Document,
        out: &mut String,
    ) {
        let mut attrs = String::new();
        if cell.colspan > 1 {
            attrs.push_str(&format!(" colspan=\"{}\"", cell.colspan));
        }
        if cell.rowspan > 1 {
            attrs.push_str(&format!(" rowspan=\"{}\"", cell.rowspan));
        }
        if !matches!(align, Alignment::Default) {
            attrs.push_str(&format!(" style=\"text-align: {}\"", alignment_css(align)));
        }
        out.push_str(&format!("<td{attrs}>"));
        self.write_blocks(&cell.content, opts, doc, out);
        out.push_str("</td>");
    }

    fn write_inlines(
        &self,
        inlines: &[Inline],
        opts: &WriteOptions,
        doc: &Document,
        out: &mut String,
    ) {
        for inline in inlines {
            self.write_inline(inline, opts, doc, out);
        }
    }

    fn write_inline(&self, inline: &Inline, opts: &WriteOptions, doc: &Document, out: &mut String) {
        match inline {
            Inline::Text { value } => {
                out.push_str(&escape_html(value));
            }
            Inline::Emphasis { content } => {
                out.push_str("<em>");
                self.write_inlines(content, opts, doc, out);
                out.push_str("</em>");
            }
            Inline::Strong { content } => {
                out.push_str("<strong>");
                self.write_inlines(content, opts, doc, out);
                out.push_str("</strong>");
            }
            Inline::Strikethrough { content } => {
                out.push_str("<del>");
                self.write_inlines(content, opts, doc, out);
                out.push_str("</del>");
            }
            Inline::Code { value, .. } => {
                out.push_str("<code>");
                out.push_str(&escape_html(value));
                out.push_str("</code>");
            }
            Inline::MathInline { value } => match opts.math_engine {
                MathEngine::MathML => {
                    out.push_str(value); // already MathML from transform
                }
                _ => {
                    let class = match opts.math_engine {
                        MathEngine::KaTeX => "math math-inline",
                        MathEngine::MathJax => "math math-inline",
                        MathEngine::MathML | MathEngine::Raw => "math",
                    };
                    out.push_str(&format!("<span class=\"{class}\">"));
                    out.push_str(&escape_html(value));
                    out.push_str("</span>");
                }
            },
            Inline::Link {
                url,
                title,
                content,
                ..
            } => {
                out.push_str(&format!("<a href=\"{}\"", escape_attr(url)));
                if let Some(t) = title {
                    out.push_str(&format!(" title=\"{}\"", escape_attr(t)));
                }
                out.push('>');
                self.write_inlines(content, opts, doc, out);
                out.push_str("</a>");
            }
            Inline::Image(img) => {
                let src = image_src(&img.url, doc);
                out.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\"",
                    escape_attr(&src),
                    escape_attr(&img.alt_text())
                ));
                if let Some(t) = &img.title {
                    out.push_str(&format!(" title=\"{}\"", escape_attr(t)));
                }
                out.push('>');
            }
            Inline::Citation(cite) => {
                // Placeholder rendering — transforms should resolve this first
                out.push_str("<cite>");
                out.push_str(&cite.keys().join("; "));
                out.push_str("</cite>");
            }
            Inline::FootnoteRef { id } => {
                out.push_str(&format!(
                    "<sup class=\"footnote-ref\"><a href=\"#fn-{id}\">[{id}]</a></sup>"
                ));
            }
            Inline::CrossRef(cr) => {
                // Placeholder rendering — CrossRefResolver transform should
                // replace these before the writer runs.
                out.push_str(&format!(
                    "<a href=\"#{}\" class=\"crossref\">[{}]</a>",
                    escape_attr(&cr.target),
                    escape_html(&cr.target)
                ));
            }
            Inline::RawInline { format, content } => {
                if format == "html" {
                    out.push_str(content);
                }
            }
            Inline::Superscript { content } => {
                out.push_str("<sup>");
                self.write_inlines(content, opts, doc, out);
                out.push_str("</sup>");
            }
            Inline::Subscript { content } => {
                out.push_str("<sub>");
                self.write_inlines(content, opts, doc, out);
                out.push_str("</sub>");
            }
            Inline::SmallCaps { content } => {
                out.push_str("<span style=\"font-variant: small-caps\">");
                self.write_inlines(content, opts, doc, out);
                out.push_str("</span>");
            }
            Inline::SoftBreak => {
                out.push('\n');
            }
            Inline::HardBreak => {
                out.push_str("<br>\n");
            }
            Inline::Underline { content } => {
                out.push_str("<u>");
                self.write_inlines(content, opts, doc, out);
                out.push_str("</u>");
            }
            Inline::Quoted {
                quote_type,
                content,
            } => {
                let (open, close) = match quote_type {
                    QuoteType::SingleQuote => ("&lsquo;", "&rsquo;"),
                    QuoteType::DoubleQuote => ("&ldquo;", "&rdquo;"),
                };
                out.push_str(open);
                self.write_inlines(content, opts, doc, out);
                out.push_str(close);
            }
            Inline::Span { content, attrs } => {
                let mut attr_str = String::new();
                if let Some(id) = &attrs.id {
                    attr_str.push_str(&format!(" id=\"{}\"", escape_attr(id)));
                }
                if !attrs.classes.is_empty() {
                    attr_str.push_str(&format!(" class=\"{}\"", attrs.classes.join(" ")));
                }
                for (k, v) in &attrs.key_values {
                    attr_str.push_str(&format!(" data-{}=\"{}\"", escape_attr(k), escape_attr(v)));
                }
                out.push_str(&format!("<span{attr_str}>"));
                self.write_inlines(content, opts, doc, out);
                out.push_str("</span>");
            }
        }
    }

    fn wrap_standalone(
        &self,
        body: &str,
        doc: &Document,
        opts: &WriteOptions,
    ) -> docmux_core::Result<String> {
        let template_src = match &opts.template {
            Some(path) => std::fs::read_to_string(path)?,
            None => docmux_template::DEFAULT_HTML.to_string(),
        };
        let ctx = self.build_template_context(body, doc, opts);
        docmux_template::render(&template_src, &ctx).map_err(docmux_core::ConvertError::from)
    }

    fn build_template_context(
        &self,
        body: &str,
        doc: &Document,
        opts: &WriteOptions,
    ) -> docmux_template::TemplateContext {
        use docmux_template::TemplateValue;
        let mut ctx = docmux_template::TemplateContext::new();

        // Body
        ctx.insert("body".into(), TemplateValue::Str(body.to_string()));

        // Title (HTML-escaped)
        if let Some(title) = &doc.metadata.title {
            ctx.insert("title".into(), TemplateValue::Str(escape_html(title)));
        }

        // Authors
        if !doc.metadata.authors.is_empty() {
            let author_list: Vec<TemplateValue> = doc
                .metadata
                .authors
                .iter()
                .map(|a| {
                    let mut map = std::collections::HashMap::new();
                    map.insert("name".into(), TemplateValue::Str(escape_html(&a.name)));
                    if let Some(email) = &a.email {
                        map.insert("email".into(), TemplateValue::Str(escape_html(email)));
                    }
                    if let Some(aff) = &a.affiliation {
                        map.insert("affiliation".into(), TemplateValue::Str(escape_html(aff)));
                    }
                    if let Some(orcid) = &a.orcid {
                        map.insert("orcid".into(), TemplateValue::Str(escape_html(orcid)));
                    }
                    TemplateValue::Map(map)
                })
                .collect();
            ctx.insert("author".into(), TemplateValue::List(author_list));
        }

        // Date
        if let Some(date) = &doc.metadata.date {
            ctx.insert("date".into(), TemplateValue::Str(escape_html(date)));
        }

        // Abstract (rendered as HTML)
        if let Some(blocks) = &doc.metadata.abstract_text {
            let mut abs_html = String::new();
            self.write_blocks(blocks, opts, doc, &mut abs_html);
            ctx.insert("abstract".into(), TemplateValue::Str(abs_html));
        }

        // Math engine head
        let math_head = match opts.math_engine {
            MathEngine::KaTeX => {
                r#"<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css">
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js"></script>
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/contrib/auto-render.min.js"
  onload="renderMathInElement(document.body, {delimiters: [{left: '$$', right: '$$', display: true}, {left: '$', right: '$', display: false}]})"></script>"#
            }
            MathEngine::MathJax => {
                r#"<script src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js" async></script>"#
            }
            MathEngine::MathML | MathEngine::Raw => "",
        };
        if !math_head.is_empty() {
            ctx.insert("math".into(), TemplateValue::Str(math_head.to_string()));
        }

        // Code block line-feature CSS (harmless if unused)
        ctx.insert(
            "highlighting-css".into(),
            TemplateValue::Str(CODE_LINE_CSS.to_string()),
        );

        // CSS URLs from variables
        let mut css_urls: Vec<TemplateValue> = Vec::new();
        // "css" is the first URL, "css1", "css2" etc are subsequent
        if let Some(url) = opts.variables.get("css") {
            css_urls.push(TemplateValue::Str(url.clone()));
        }
        for i in 1.. {
            let key = format!("css{i}");
            if let Some(url) = opts.variables.get(&key) {
                css_urls.push(TemplateValue::Str(url.clone()));
            } else {
                break;
            }
        }
        if !css_urls.is_empty() {
            ctx.insert("css".into(), TemplateValue::List(css_urls));
        }

        // Merge user variables (these override metadata)
        for (k, v) in &opts.variables {
            // Skip css variables (already handled above)
            if k == "css" || (k.starts_with("css") && k[3..].parse::<u32>().is_ok()) {
                continue;
            }
            ctx.insert(k.clone(), TemplateValue::Str(v.clone()));
        }

        ctx
    }
}

impl Writer for HtmlWriter {
    fn format(&self) -> &str {
        "html"
    }

    fn default_extension(&self) -> &str {
        "html"
    }

    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
        let mut body = String::with_capacity(4096);
        self.write_blocks(&doc.content, opts, doc, &mut body);

        if opts.standalone {
            self.wrap_standalone(&body, doc, opts)
        } else {
            Ok(body)
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn image_src(url: &str, doc: &Document) -> String {
    if let Some(res) = doc.resources.get(url) {
        if !res.data.is_empty() {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&res.data);
            return format!("data:{};base64,{}", res.mime_type, b64);
        }
    }
    url.to_string()
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    escape_html(s).replace('"', "&quot;")
}

fn alignment_css(a: &Alignment) -> &'static str {
    match a {
        Alignment::Left => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
        Alignment::Default => "left",
    }
}

/// CSS for line numbers and line highlighting in code blocks.
const CODE_LINE_CSS: &str = "\
.line-number { color: #6e7781; padding-right: 1em; user-select: none; display: inline-block; text-align: right; min-width: 2em; }\n\
.highlight-line { background-color: rgba(255, 255, 0, 0.15); display: block; }";

/// Write a line-number `<span>` if `line_opts.number_lines` is true.
fn write_code_line_prefix(out: &mut String, line_num: u32, line_opts: &LineOptions) {
    if line_opts.number_lines {
        out.push_str(&format!("<span class=\"line-number\">{line_num}</span>"));
    }
}

/// Write syntax-highlighted tokens for a single line.
fn write_highlighted_tokens(out: &mut String, tokens: &[docmux_highlight::HighlightToken]) {
    for token in tokens {
        let c = token.style.foreground;
        let mut style = format!("color:#{:02x}{:02x}{:02x}", c.r, c.g, c.b);
        if token.style.bold {
            style.push_str(";font-weight:bold");
        }
        if token.style.italic {
            style.push_str(";font-style:italic");
        }
        if token.style.underline {
            style.push_str(";text-decoration:underline");
        }
        out.push_str(&format!(
            "<span style=\"{}\">{}</span>",
            style,
            escape_html(&token.text)
        ));
    }
}

/// Render syntax-highlighted lines with optional line numbers and highlighting.
fn write_highlighted_lines(
    out: &mut String,
    lines: &[Vec<docmux_highlight::HighlightToken>],
    line_opts: &LineOptions,
) {
    for (i, line) in lines.iter().enumerate() {
        let line_num = line_opts.start_from + i as u32;
        write_code_line_prefix(out, line_num, line_opts);
        if line_opts.is_highlighted(line_num) {
            out.push_str("<span class=\"highlight-line\">");
            write_highlighted_tokens(out, line);
            out.push_str("</span>");
        } else {
            write_highlighted_tokens(out, line);
        }
    }
}

/// Render plain-text code lines with optional line numbers and highlighting.
fn write_plain_lines(out: &mut String, content: &str, line_opts: &LineOptions) {
    let has_line_features = line_opts.number_lines || !line_opts.highlighted_lines.is_empty();
    if !has_line_features {
        out.push_str(&escape_html(content));
        return;
    }
    for (i, line) in content.split('\n').enumerate() {
        let line_num = line_opts.start_from + i as u32;
        write_code_line_prefix(out, line_num, line_opts);
        if line_opts.is_highlighted(line_num) {
            out.push_str("<span class=\"highlight-line\">");
            out.push_str(&escape_html(line));
            out.push_str("</span>");
        } else {
            out.push_str(&escape_html(line));
        }
        // Preserve newlines between lines (split removes them)
        if i < content.split('\n').count() - 1 {
            out.push('\n');
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn write_html(doc: &Document) -> String {
        let writer = HtmlWriter::new();
        writer.write(doc, &WriteOptions::default()).unwrap()
    }

    #[test]
    fn paragraph() {
        let doc = Document {
            content: vec![Block::text("Hello!")],
            ..Default::default()
        };
        let html = write_html(&doc);
        assert_eq!(html.trim(), "<p>Hello!</p>");
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
        let html = write_html(&doc);
        assert!(html.contains("<h2 id=\"intro\">Introduction</h2>"));
    }

    #[test]
    fn inline_math() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("The formula "),
                    Inline::MathInline {
                        value: "E = mc^2".into(),
                    },
                    Inline::text(" is famous."),
                ],
            }],
            ..Default::default()
        };
        let html = write_html(&doc);
        assert!(html.contains("class=\"math math-inline\""));
        assert!(html.contains("E = mc^2"));
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
        let html = write_html(&doc);
        assert!(html.contains("class=\"language-python\""));
        assert!(html.contains("print(&#x27;hello&#x27;)") || html.contains("print('hello')"));
    }

    #[test]
    fn standalone_mode() {
        let doc = Document {
            metadata: Metadata {
                title: Some("My Doc".into()),
                ..Default::default()
            },
            content: vec![Block::text("Body")],
            ..Default::default()
        };
        let writer = HtmlWriter::new();
        let opts = WriteOptions {
            standalone: true,
            ..Default::default()
        };
        let html = writer.write(&doc, &opts).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<title>My Doc</title>"));
        assert!(html.contains("katex"));
    }

    #[test]
    fn writer_trait_metadata() {
        let writer = HtmlWriter::new();
        assert_eq!(writer.format(), "html");
        assert_eq!(writer.default_extension(), "html");
    }

    #[test]
    fn code_block_with_highlighting() {
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
        let writer = HtmlWriter::new();
        let opts = WriteOptions {
            highlight_style: Some("InspiredGitHub".into()),
            ..Default::default()
        };
        let html = writer.write(&doc, &opts).unwrap();
        assert!(
            html.contains("<span style=\""),
            "expected colored spans, got: {html}"
        );
        assert!(html.contains("fn"), "expected 'fn' in output, got: {html}");
        assert!(
            html.contains("<pre"),
            "expected <pre in output, got: {html}"
        );
    }

    #[test]
    fn code_block_highlighting_unknown_lang_falls_back() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("nonexistent-xyz".into()),
                content: "some code".into(),
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let writer = HtmlWriter::new();
        let opts = WriteOptions {
            highlight_style: Some("InspiredGitHub".into()),
            ..Default::default()
        };
        let html = writer.write(&doc, &opts).unwrap();
        assert!(
            html.contains("some code"),
            "expected 'some code' in output, got: {html}"
        );
        assert!(
            html.contains("<pre><code"),
            "expected plain fallback with <pre><code, got: {html}"
        );
    }

    #[test]
    fn image_with_resource_renders_data_uri() {
        use std::collections::HashMap;

        let png_bytes = b"\x89PNG\r\n\x1a\nfake";
        let expected_b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);

        let doc = Document {
            resources: HashMap::from([(
                "media/image1.png".to_string(),
                ResourceData {
                    mime_type: "image/png".to_string(),
                    data: png_bytes.to_vec(),
                },
            )]),
            content: vec![Block::Paragraph {
                content: vec![Inline::Image(docmux_ast::Image {
                    url: "media/image1.png".to_string(),
                    alt: vec![Inline::Text {
                        value: "A logo".to_string(),
                    }],
                    title: None,
                    attrs: None,
                })],
            }],
            ..Default::default()
        };

        let writer = HtmlWriter::new();
        let output = writer.write(&doc, &WriteOptions::default()).unwrap();
        let expected_src = format!("data:image/png;base64,{expected_b64}");
        assert!(
            output.contains(&expected_src),
            "output should contain data URI, got: {output}"
        );
        assert!(output.contains("alt=\"A logo\""));
    }

    #[test]
    fn image_without_resource_renders_path() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Image(docmux_ast::Image {
                    url: "images/photo.jpg".to_string(),
                    alt: vec![],
                    title: None,
                    attrs: None,
                })],
            }],
            ..Default::default()
        };

        let writer = HtmlWriter::new();
        let output = writer.write(&doc, &WriteOptions::default()).unwrap();
        assert!(
            output.contains("src=\"images/photo.jpg\""),
            "should use path as-is, got: {output}"
        );
    }

    #[test]
    fn code_block_with_line_numbers() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "def hello():\n    print(\"world\")".into(),
                caption: None,
                label: None,
                attrs: Some(Attributes {
                    id: None,
                    classes: vec!["numberLines".into()],
                    key_values: HashMap::new(),
                }),
            }],
            ..Default::default()
        };
        let writer = HtmlWriter::new();
        let opts = WriteOptions::default();
        let html = writer.write(&doc, &opts).unwrap();
        assert!(html.contains("line-number"), "should have line numbers");
    }

    #[test]
    fn code_block_with_line_highlight() {
        let mut kvs = HashMap::new();
        kvs.insert("highlight".into(), "2".into());
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "line1\nline2\nline3".into(),
                caption: None,
                label: None,
                attrs: Some(Attributes {
                    id: None,
                    classes: vec![],
                    key_values: kvs,
                }),
            }],
            ..Default::default()
        };
        let writer = HtmlWriter::new();
        let opts = WriteOptions::default();
        let html = writer.write(&doc, &opts).unwrap();
        assert!(
            html.contains("highlight-line"),
            "should have highlight class"
        );
    }

    #[test]
    fn code_block_with_start_from() {
        let mut kvs = HashMap::new();
        kvs.insert("startFrom".into(), "10".into());
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "a\nb".into(),
                caption: None,
                label: None,
                attrs: Some(Attributes {
                    id: None,
                    classes: vec!["numberLines".into()],
                    key_values: kvs,
                }),
            }],
            ..Default::default()
        };
        let writer = HtmlWriter::new();
        let opts = WriteOptions::default();
        let html = writer.write(&doc, &opts).unwrap();
        assert!(html.contains(">10<"), "should start at 10");
        assert!(html.contains(">11<"), "should have line 11");
    }
}
