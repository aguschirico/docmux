//! # docmux-reader-latex
//!
//! LaTeX reader for docmux. Parses a practical subset of LaTeX into the
//! docmux AST using a hand-written recursive descent parser.
//!
//! Unrecognized commands and environments are emitted as `RawBlock`/`RawInline`
//! with warnings accumulated in `Document.warnings`.

pub(crate) mod commands;
pub(crate) mod flatten;
pub(crate) mod lexer;
pub(crate) mod macros;
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
        // `\bibliography{refs}` stands for the `.bbl` BibTeX produced.
        let input = inline_bbl(input, files);

        // User macros may be defined in any file and used in any other.
        let mut defs = macros::Macros::default();
        let main = macros::collect(&input, &mut defs);
        let files: HashMap<String, String> = files
            .iter()
            .map(|(name, source)| (name.clone(), macros::collect(source, &mut defs)))
            .collect();
        let main = macros::expand(&main, &defs);
        let files: HashMap<String, String> = files
            .into_iter()
            .map(|(name, source)| (name, macros::expand(&source, &defs)))
            .collect();

        let tokens = lexer::tokenize(&main);
        let mut warnings = Vec::new();
        let flat = flatten::flatten_includes(tokens, &files, &mut warnings);
        let mut doc = parser::Parser::new(flat).parse();
        // Surface flatten warnings before parser warnings.
        warnings.extend(std::mem::take(&mut doc.warnings));
        doc.warnings = warnings;
        Ok(doc)
    }
}

/// Replace `\bibliography{a,b}` with the `thebibliography` environment from
/// `a.bbl` (or `b.bbl`), or from the only `.bbl` in `files`. arXiv sources ship
/// the `.bbl`, rarely the `.bib`. Sources that already contain the
/// environment are left alone.
fn inline_bbl(source: &str, files: &HashMap<String, String>) -> String {
    const COMMAND: &str = "\\bibliography{";
    if source.contains("\\begin{thebibliography}") {
        return source.to_string();
    }
    let Some(start) = source.find(COMMAND) else {
        return source.to_string();
    };
    let args_start = start + COMMAND.len();
    let Some(len) = source[args_start..].find('}') else {
        return source.to_string();
    };
    let named = source[args_start..args_start + len]
        .split(',')
        .map(|name| format!("{}.bbl", name.trim().trim_end_matches(".bib")))
        .find_map(|name| files.get(&name));
    let mut bbl_keys: Vec<&String> = files.keys().filter(|k| k.ends_with(".bbl")).collect();
    bbl_keys.sort();
    let Some(bbl) = named.or_else(|| bbl_keys.first().and_then(|k| files.get(*k))) else {
        return source.to_string();
    };
    // biblatex `.bbl` files are a data format, not LaTeX to typeset.
    if !bbl.contains("\\begin{thebibliography}") {
        return source.to_string();
    }
    format!(
        "{}{}{}",
        &source[..start],
        bbl,
        &source[args_start + len + 1..]
    )
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

    #[test]
    fn bibliography_is_read_from_bbl() {
        let mut files = HashMap::new();
        files.insert(
            "refs.bbl".to_string(),
            r"\begin{thebibliography}{1}
\expandafter\ifx\csname natexlab\endcsname\relax\def\natexlab#1{#1}\fi

\bibitem[{Kingma and Ba(2015)}]{kingma2014adam}
Diederik~P. Kingma and Jimmy Ba. 2015.
\newblock Adam: A method for stochastic optimization.
\newblock In \emph{ICLR}.

\bibitem[Vaswani et~al.(2017)]{vaswani}
A.~Vaswani.
\newblock Attention.
\end{thebibliography}"
                .to_string(),
        );
        let main = r"\documentclass{article}
\begin{document}
We use Adam \citep{kingma2014adam}.
\bibliography{refs}
\end{document}";
        let doc = LatexReader::new().read_with_files(main, &files).unwrap();
        let refs = doc
            .content
            .iter()
            .find_map(|b| match b {
                Block::Div { attrs, content } if attrs.id.as_deref() == Some("refs") => {
                    Some(content)
                }
                _ => None,
            })
            .expect("references div");
        assert!(matches!(&refs[0], Block::Heading { .. }));
        let Block::Div { attrs, content } = &refs[1] else {
            panic!("expected entry div, got {:?}", refs[1]);
        };
        assert_eq!(attrs.id.as_deref(), Some("ref-kingma2014adam"));
        assert_eq!(
            attrs.key_values.get("label").map(String::as_str),
            Some("Kingma and Ba(2015)")
        );
        let text = format!("{content:?}");
        assert!(text.contains("Adam: A method"));
        assert!(!text.contains("natexlab"));
        assert!(
            matches!(&refs[2], Block::Div { attrs, .. } if attrs.key_values["label"] == "Vaswani et al.(2017)")
        );
    }
}
