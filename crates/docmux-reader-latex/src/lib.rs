//! # docmux-reader-latex
//!
//! LaTeX reader for docmux. Parses a practical subset of LaTeX into the
//! docmux AST using a hand-written recursive descent parser.
//!
//! Unrecognized commands and environments are emitted as `RawBlock`/`RawInline`
//! with warnings accumulated in `Document.warnings`.

pub(crate) mod flatten;
pub(crate) mod lexer;
pub(crate) mod parser;
pub(crate) mod unescape;

use docmux_ast::Document;
use docmux_core::{Reader, Result};
use std::collections::HashMap;

/// A LaTeX reader.
#[derive(Debug, Default)]
pub struct LatexReader;

impl LatexReader {
    pub fn new() -> Self {
        Self
    }

    /// Parse a LaTeX document, resolving `\input{}` and `\include{}` directives
    /// against the given file map. Keys are filenames as referenced by the
    /// directive (with or without `.tex` extension).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use docmux_reader_latex::LatexReader;
    /// use std::collections::HashMap;
    ///
    /// let mut files = HashMap::new();
    /// files.insert("intro.tex".to_string(), "Hello!".to_string());
    /// let doc = LatexReader::new()
    ///     .read_with_files("\\input{intro}", &files)
    ///     .unwrap();
    /// ```
    pub fn read_with_files(
        &self,
        input: &str,
        files: &HashMap<String, String>,
    ) -> Result<Document> {
        let tokens = lexer::tokenize(input);
        let mut warnings = Vec::new();
        let flat = flatten::flatten_includes(tokens, files, &mut warnings);
        let mut doc = parser::Parser::new(flat).parse();
        // Surface flatten warnings before parser warnings.
        warnings.extend(std::mem::take(&mut doc.warnings));
        doc.warnings = warnings;
        Ok(doc)
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
        self.read_with_files(input, &HashMap::new())
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

    #[test]
    fn read_with_files_inlines_input_directive() {
        let reader = LatexReader::new();
        let mut files = HashMap::new();
        files.insert("body.tex".to_string(), "\\section{Body}\nText.".to_string());

        let main = r"\documentclass{article}
\begin{document}
\input{body}
\end{document}";

        let doc = reader.read_with_files(main, &files).unwrap();

        // After flattening, the body must yield a heading and a paragraph.
        let heading_count = doc
            .content
            .iter()
            .filter(|b| matches!(b, Block::Heading { .. }))
            .count();
        assert!(
            heading_count >= 1,
            "expected included \\section, got {:?}",
            doc.content
        );
        assert!(
            doc.warnings.is_empty(),
            "unexpected warnings: {:?}",
            doc.warnings
        );
    }

    #[test]
    fn read_unchanged_when_no_files_given() {
        // Regression: existing read() callers should see identical behaviour.
        let reader = LatexReader::new();
        let source = "\\section{Title}\n\nHello world.";
        let doc_via_read = reader.read(source).unwrap();
        let doc_via_with_files = reader.read_with_files(source, &HashMap::new()).unwrap();
        assert_eq!(doc_via_read.content.len(), doc_via_with_files.content.len());
        assert_eq!(doc_via_read.warnings, doc_via_with_files.warnings);
    }
}
