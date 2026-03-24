//! # docmux-reader-typst
//!
//! Typst reader for docmux. Parses a practical subset of Typst markup into
//! the docmux AST using a hand-written recursive descent parser.
//!
//! Unrecognized function calls are emitted as `RawBlock`/`RawInline`
//! with warnings accumulated in `Document.warnings`.

pub(crate) mod lexer;
pub(crate) mod parser;

use docmux_ast::Document;
use docmux_core::{Reader, Result};

/// A Typst reader.
#[derive(Debug, Default)]
pub struct TypstReader;

impl TypstReader {
    pub fn new() -> Self {
        Self
    }
}

impl Reader for TypstReader {
    fn format(&self) -> &str {
        "typst"
    }

    fn extensions(&self) -> &[&str] {
        &["typ"]
    }

    fn read(&self, input: &str) -> Result<Document> {
        let tokens = lexer::tokenize(input);
        let doc = parser::Parser::new(tokens, input).parse();
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_ast::Block;

    #[test]
    fn reader_trait_metadata() {
        let reader = TypstReader::new();
        assert_eq!(reader.format(), "typst");
        assert!(reader.extensions().contains(&"typ"));
    }

    #[test]
    fn read_simple_document() {
        let reader = TypstReader::new();
        let doc = reader.read("= Hello\n\nSome text.").unwrap();
        assert_eq!(doc.content.len(), 2);
        assert!(matches!(&doc.content[0], Block::Heading { level: 1, .. }));
    }
}
