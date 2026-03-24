/// Tokens produced by the Typst lexer.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Token {
    Text { value: String },
}

/// Tokenize a Typst source string into a flat sequence of tokens.
pub fn tokenize(_input: &str) -> Vec<Token> {
    Vec::new()
}
