use crate::lexer::Token;
use docmux_ast::Document;

/// Recursive descent parser for Typst documents.
#[allow(dead_code)]
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    raw_input: String,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, raw_input: &str) -> Self {
        Self {
            tokens,
            pos: 0,
            raw_input: raw_input.to_string(),
        }
    }

    pub fn parse(self) -> Document {
        Document::default()
    }
}
