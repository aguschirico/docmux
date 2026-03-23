//! # docmux-reader-latex
//!
//! LaTeX reader for docmux. Parses a practical subset of LaTeX into the
//! docmux AST using a hand-written recursive descent parser.

pub(crate) mod lexer;
pub(crate) mod unescape;

pub use lexer::{tokenize, Token};
pub use unescape::unescape_latex;
