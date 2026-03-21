//! # docmux-writer-html
//!
//! HTML5 writer for docmux. Converts the docmux AST into semantic HTML.

use docmux_ast::*;
use docmux_core::{MathEngine, Result, WriteOptions, Writer};

/// An HTML5 writer.
#[derive(Debug, Default)]
pub struct HtmlWriter;

impl HtmlWriter {
    pub fn new() -> Self {
        Self
    }

    fn write_blocks(&self, blocks: &[Block], opts: &WriteOptions, out: &mut String) {
        for block in blocks {
            self.write_block(block, opts, out);
        }
    }

    fn write_block(&self, block: &Block, opts: &WriteOptions, out: &mut String) {
        match block {
            Block::Paragraph { content } => {
                out.push_str("<p>");
                self.write_inlines(content, opts, out);
                out.push_str("</p>\n");
            }
            Block::Heading { level, id, content } => {
                let tag = format!("h{}", level.min(&6));
                if let Some(id) = id {
                    out.push_str(&format!("<{tag} id=\"{}\">", escape_attr(id)));
                } else {
                    out.push_str(&format!("<{tag}>"));
                }
                self.write_inlines(content, opts, out);
                out.push_str(&format!("</{tag}>\n"));
            }
            Block::CodeBlock {
                language, content, ..
            } => {
                if let Some(lang) = language {
                    out.push_str(&format!(
                        "<pre><code class=\"language-{}\">",
                        escape_html(lang)
                    ));
                } else {
                    out.push_str("<pre><code>");
                }
                out.push_str(&escape_html(content));
                out.push_str("</code></pre>\n");
            }
            Block::MathBlock { content, label } => {
                let class = match opts.math_engine {
                    MathEngine::KaTeX => "math math-display",
                    MathEngine::MathJax => "math math-display",
                    MathEngine::Raw => "math",
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
            Block::BlockQuote { content } => {
                out.push_str("<blockquote>\n");
                self.write_blocks(content, opts, out);
                out.push_str("</blockquote>\n");
            }
            Block::List {
                ordered,
                start,
                items,
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
                    self.write_blocks(&item.content, opts, out);
                    out.push_str("</li>\n");
                }
                if *ordered {
                    out.push_str("</ol>\n");
                } else {
                    out.push_str("</ul>\n");
                }
            }
            Block::Table(table) => {
                self.write_table(table, opts, out);
            }
            Block::Figure {
                image,
                caption,
                label,
            } => {
                if let Some(label) = label {
                    out.push_str(&format!("<figure id=\"{}\">", escape_attr(label)));
                } else {
                    out.push_str("<figure>");
                }
                out.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\">",
                    escape_attr(&image.url),
                    escape_attr(&image.alt)
                ));
                if let Some(cap) = caption {
                    out.push_str("<figcaption>");
                    self.write_inlines(cap, opts, out);
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
                            self.write_inlines(t, opts, out);
                            out.push_str("</p>");
                        }
                        self.write_blocks(content, opts, out);
                        out.push_str("</aside>\n");
                        return;
                    }
                };
                out.push_str(&format!("<aside class=\"{class}\">"));
                if let Some(t) = title {
                    out.push_str("<p class=\"admonition-title\">");
                    self.write_inlines(t, opts, out);
                    out.push_str("</p>");
                }
                self.write_blocks(content, opts, out);
                out.push_str("</aside>\n");
            }
            Block::DefinitionList { items } => {
                out.push_str("<dl>\n");
                for item in items {
                    out.push_str("<dt>");
                    self.write_inlines(&item.term, opts, out);
                    out.push_str("</dt>\n");
                    for def in &item.definitions {
                        out.push_str("<dd>");
                        self.write_blocks(def, opts, out);
                        out.push_str("</dd>\n");
                    }
                }
                out.push_str("</dl>\n");
            }
            Block::FootnoteDef { id, content } => {
                out.push_str(&format!(
                    "<aside id=\"fn-{}\" class=\"footnote\" role=\"note\">\n",
                    escape_attr(id)
                ));
                self.write_blocks(content, opts, out);
                out.push_str("</aside>\n");
            }
        }
    }

    fn write_table(&self, table: &Table, opts: &WriteOptions, out: &mut String) {
        if let Some(label) = &table.label {
            out.push_str(&format!("<table id=\"{}\">\n", escape_attr(label)));
        } else {
            out.push_str("<table>\n");
        }

        if let Some(cap) = &table.caption {
            out.push_str("<caption>");
            self.write_inlines(cap, opts, out);
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
                self.write_th(cell, align, opts, out);
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
                self.write_td(cell, align, opts, out);
            }
            out.push_str("</tr>\n");
        }
        out.push_str("</tbody>\n</table>\n");
    }

    fn write_th(&self, cell: &TableCell, align: &Alignment, opts: &WriteOptions, out: &mut String) {
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
        self.write_blocks(&cell.content, opts, out);
        out.push_str("</th>");
    }

    fn write_td(&self, cell: &TableCell, align: &Alignment, opts: &WriteOptions, out: &mut String) {
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
        self.write_blocks(&cell.content, opts, out);
        out.push_str("</td>");
    }

    fn write_inlines(&self, inlines: &[Inline], opts: &WriteOptions, out: &mut String) {
        for inline in inlines {
            self.write_inline(inline, opts, out);
        }
    }

    fn write_inline(&self, inline: &Inline, opts: &WriteOptions, out: &mut String) {
        match inline {
            Inline::Text { value } => {
                out.push_str(&escape_html(value));
            }
            Inline::Emphasis { content } => {
                out.push_str("<em>");
                self.write_inlines(content, opts, out);
                out.push_str("</em>");
            }
            Inline::Strong { content } => {
                out.push_str("<strong>");
                self.write_inlines(content, opts, out);
                out.push_str("</strong>");
            }
            Inline::Strikethrough { content } => {
                out.push_str("<del>");
                self.write_inlines(content, opts, out);
                out.push_str("</del>");
            }
            Inline::Code { value } => {
                out.push_str("<code>");
                out.push_str(&escape_html(value));
                out.push_str("</code>");
            }
            Inline::MathInline { value } => {
                let class = match opts.math_engine {
                    MathEngine::KaTeX => "math math-inline",
                    MathEngine::MathJax => "math math-inline",
                    MathEngine::Raw => "math",
                };
                out.push_str(&format!("<span class=\"{class}\">"));
                out.push_str(&escape_html(value));
                out.push_str("</span>");
            }
            Inline::Link {
                url,
                title,
                content,
            } => {
                out.push_str(&format!("<a href=\"{}\"", escape_attr(url)));
                if let Some(t) = title {
                    out.push_str(&format!(" title=\"{}\"", escape_attr(t)));
                }
                out.push('>');
                self.write_inlines(content, opts, out);
                out.push_str("</a>");
            }
            Inline::Image(img) => {
                out.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\"",
                    escape_attr(&img.url),
                    escape_attr(&img.alt)
                ));
                if let Some(t) = &img.title {
                    out.push_str(&format!(" title=\"{}\"", escape_attr(t)));
                }
                out.push_str(">");
            }
            Inline::Citation(cite) => {
                // Placeholder rendering — transforms should resolve this first
                out.push_str("<cite>");
                out.push_str(&cite.keys.join("; "));
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
                self.write_inlines(content, opts, out);
                out.push_str("</sup>");
            }
            Inline::Subscript { content } => {
                out.push_str("<sub>");
                self.write_inlines(content, opts, out);
                out.push_str("</sub>");
            }
            Inline::SmallCaps { content } => {
                out.push_str("<span style=\"font-variant: small-caps\">");
                self.write_inlines(content, opts, out);
                out.push_str("</span>");
            }
            Inline::SoftBreak => {
                out.push('\n');
            }
            Inline::HardBreak => {
                out.push_str("<br>\n");
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
                self.write_inlines(content, opts, out);
                out.push_str("</span>");
            }
        }
    }

    fn wrap_standalone(&self, body: &str, doc: &Document, opts: &WriteOptions) -> String {
        let title = doc
            .metadata
            .title
            .as_deref()
            .unwrap_or("Untitled Document");

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
            MathEngine::Raw => "",
        };

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
{math_head}
</head>
<body>
{body}</body>
</html>
"#,
            title = escape_html(title),
        )
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
        self.write_blocks(&doc.content, opts, &mut body);

        if opts.standalone {
            Ok(self.wrap_standalone(&body, doc, opts))
        } else {
            Ok(body)
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

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

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
}
