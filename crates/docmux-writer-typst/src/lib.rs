//! # docmux-writer-typst
//!
//! Typst writer for docmux. Converts the docmux AST into Typst markup.

use docmux_ast::*;
use docmux_core::{Result, WriteOptions, Writer};

/// A Typst writer.
#[derive(Debug, Default)]
pub struct TypstWriter;

impl TypstWriter {
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
                self.write_inlines(content, opts, out);
                out.push_str("\n\n");
            }
            Block::Heading {
                level, id, content, ..
            } => {
                for _ in 0..*level {
                    out.push('=');
                }
                out.push(' ');
                self.write_inlines(content, opts, out);
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
                        self.write_inlines(cap, opts, out);
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
            _ => {}
        }
    }

    fn write_inlines(&self, inlines: &[Inline], opts: &WriteOptions, out: &mut String) {
        for inline in inlines {
            self.write_inline(inline, opts, out);
        }
    }

    fn write_inline(&self, inline: &Inline, opts: &WriteOptions, out: &mut String) {
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
                self.write_inlines(content, opts, out);
                out.push('_');
            }
            Inline::Strong { content } => {
                out.push('*');
                self.write_inlines(content, opts, out);
                out.push('*');
            }
            Inline::Strikethrough { content } => {
                out.push_str("#strike[");
                self.write_inlines(content, opts, out);
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
                    self.write_inlines(content, opts, out);
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
                self.write_inlines(content, opts, out);
                out.push(']');
            }
            Inline::Subscript { content } => {
                out.push_str("#sub[");
                self.write_inlines(content, opts, out);
                out.push(']');
            }
            Inline::SmallCaps { content } => {
                out.push_str("#smallcaps[");
                self.write_inlines(content, opts, out);
                out.push(']');
            }
            Inline::Underline { content } => {
                out.push_str("#underline[");
                self.write_inlines(content, opts, out);
                out.push(']');
            }
            Inline::Span { content, .. } => {
                self.write_inlines(content, opts, out);
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
                // Placeholder — real footnote expansion comes in Task 4
                out.push_str(&format!("#footnote[See footnote {}]", escape_typst(id)));
            }
        }
    }

    fn wrap_standalone(&self, body: &str, _doc: &Document) -> String {
        body.to_string()
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
        let mut body = String::with_capacity(4096);
        self.write_blocks(&doc.content, opts, &mut body);

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
}
