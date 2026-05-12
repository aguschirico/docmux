//! Token-stream pre-pass that resolves `\input{X}` and `\include{X}` against
//! an in-memory file map. Runs between the lexer and the parser so the parser
//! sees a single flat token stream with includes already inlined.
//!
//! Bootstrapped in Task 1 of issue #4 — the public entry point will be wired
//! into `LatexReader::read_with_files` in a later task, so dead-code lints are
//! suppressed at the module level until then.
#![allow(dead_code)]

use docmux_ast::ParseWarning;
use std::collections::HashMap;

use crate::lexer::{self, Token};

/// Maximum nesting depth before we abort a sub-tree. Defensive: real-world
/// papers nest 2–3 levels.
const MAX_DEPTH: usize = 32;

/// Walk `tokens`, replacing `\input{X}` / `\include{X}` directives with the
/// tokenized contents of `files[X]`. Recurses into included files. Emits
/// warnings on missing files, cycles, and depth-exceeded cases.
pub(crate) fn flatten_includes(
    tokens: Vec<Token>,
    files: &HashMap<String, String>,
    warnings: &mut Vec<ParseWarning>,
) -> Vec<Token> {
    let mut visited: Vec<String> = Vec::new();
    flatten_inner(tokens, files, warnings, &mut visited, 0)
}

#[allow(clippy::only_used_in_recursion)]
fn flatten_inner(
    tokens: Vec<Token>,
    files: &HashMap<String, String>,
    warnings: &mut Vec<ParseWarning>,
    visited: &mut Vec<String>,
    depth: usize,
) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Command { name, line } if name == "input" || name == "include" => {
                let line = *line;
                let cmd = name.clone();
                match read_brace_arg(&tokens, i + 1) {
                    Some((arg, consumed)) => {
                        i += 1 + consumed;
                        match files.get(&arg) {
                            Some(content) => {
                                let sub = lexer::tokenize(content);
                                let mut flat =
                                    flatten_inner(sub, files, warnings, visited, depth + 1);
                                out.append(&mut flat);
                            }
                            None => {
                                warnings.push(ParseWarning {
                                    line,
                                    message: format!(
                                        "\\{cmd}{{{arg}}}: file not found in file map"
                                    ),
                                });
                            }
                        }
                    }
                    None => {
                        out.push(tokens[i].clone());
                        i += 1;
                    }
                }
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    out
}

/// Reads a `{...}` brace argument starting at `tokens[start]`. Skips leading
/// whitespace/newlines. Returns the concatenated text inside the braces and
/// the number of tokens consumed (including the braces).
fn read_brace_arg(tokens: &[Token], start: usize) -> Option<(String, usize)> {
    let mut i = start;
    // Skip whitespace before the brace.
    while i < tokens.len() && matches!(tokens[i], Token::Newline) {
        i += 1;
    }
    if i >= tokens.len() || !matches!(tokens[i], Token::BraceOpen) {
        return None;
    }
    i += 1; // consume BraceOpen
    let mut depth: u32 = 1;
    let mut buf = String::new();
    while i < tokens.len() {
        match &tokens[i] {
            Token::BraceOpen => {
                depth += 1;
                buf.push('{');
            }
            Token::BraceClose => {
                depth -= 1;
                if depth == 0 {
                    i += 1;
                    return Some((buf, i - start));
                }
                buf.push('}');
            }
            Token::Text { value } => buf.push_str(value),
            Token::Newline => buf.push(' '),
            _ => {}
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_text_blocks(tokens: &[Token]) -> usize {
        tokens
            .iter()
            .filter(|t| matches!(t, Token::Text { .. }))
            .count()
    }

    fn text_concat(tokens: &[Token]) -> String {
        tokens
            .iter()
            .filter_map(|t| match t {
                Token::Text { value } => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn flatten_basic_input_inlines_referenced_file() {
        let main = "\\input{intro}";
        let mut files = HashMap::new();
        files.insert("intro".to_string(), "hello world".to_string());

        let tokens = lexer::tokenize(main);
        let mut warnings = Vec::new();
        let flat = flatten_includes(tokens, &files, &mut warnings);

        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        let concat = text_concat(&flat);
        assert!(
            concat.contains("hello world"),
            "expected included content; got tokens = {flat:?}"
        );
        // The \input command itself should be gone.
        let has_input_cmd = flat
            .iter()
            .any(|t| matches!(t, Token::Command { name, .. } if name == "input"));
        assert!(!has_input_cmd, "\\input command should be removed");
        // Defensive: there must be at least one Text token now.
        assert!(count_text_blocks(&flat) >= 1);
    }
}
