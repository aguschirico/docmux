//! # docmux-reader-latex
//!
//! LaTeX reader for docmux. Parses a practical subset of LaTeX into the
//! docmux AST using a hand-written recursive descent parser.
//!
//! Unrecognized commands and environments are emitted as `RawBlock`/`RawInline`
//! with warnings accumulated in `Document.warnings`.

pub(crate) mod lexer;
pub(crate) mod parser;
pub(crate) mod unescape;

use docmux_ast::Document;
use docmux_core::{Reader, Result};

/// A LaTeX reader.
#[derive(Debug, Default)]
pub struct LatexReader;

impl LatexReader {
    pub fn new() -> Self {
        Self
    }
}

impl Reader for LatexReader {
    fn format(&self) -> &str {
        "latex"
    }

    fn extensions(&self) -> &[&str] {
        &["tex", "latex"]
    }

    fn read(&self, input: &str) -> Result<Document> {
        let tokens = lexer::tokenize(input);
        let doc = parser::Parser::new(tokens).parse();
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_ast::Block;

    #[test]
    fn reader_trait_metadata() {
        let reader = LatexReader::new();
        assert_eq!(reader.format(), "latex");
        assert!(reader.extensions().contains(&"tex"));
    }

    #[test]
    fn read_simple_document() {
        let reader = LatexReader::new();
        let doc = reader
            .read(
                r"\section{Hello}

Some text.",
            )
            .unwrap();
        assert_eq!(doc.content.len(), 2);
        assert!(matches!(&doc.content[0], Block::Heading { level: 1, .. }));
    }
}
