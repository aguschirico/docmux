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
            if i > 0 {
                if !out.ends_with("\n\n") {
                    if out.ends_with('\n') {
                        out.push('\n');
                    } else {
                        out.push_str("\n\n");
                    }
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
            _ => {} // remaining blocks added in subsequent tasks
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
            _ => {} // remaining inlines added in subsequent tasks
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
}
