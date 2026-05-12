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

// `depth` is wired through but not yet read — the actual depth guard is added
// in Task 5. Until then, silence the lint just for that parameter.
#[allow(clippy::only_used_in_recursion)]
fn flatten_inner(
    tokens: Vec<Token>,
    files: &HashMap<String, String>,
    warnings: &mut Vec<ParseWarning>,
    visited: &mut Vec<String>,
    depth: usize,
) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut verbatim_depth: u32 = 0;
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::BeginEnv { name, .. } if is_verbatim_env(name) => {
                verbatim_depth += 1;
                out.push(tokens[i].clone());
                i += 1;
            }
            Token::EndEnv { name, .. } if is_verbatim_env(name) => {
                verbatim_depth = verbatim_depth.saturating_sub(1);
                out.push(tokens[i].clone());
                i += 1;
            }
            Token::Command { name, line }
                if verbatim_depth == 0 && (name == "input" || name == "include") =>
            {
                let line = *line;
                let cmd = name.clone();
                match read_brace_arg(&tokens, i + 1) {
                    Some((arg, consumed)) => {
                        i += 1 + consumed;
                        match resolve_target(&arg, files) {
                            Some((key, content)) => {
                                if visited.iter().any(|v| v == &key) {
                                    warnings.push(ParseWarning {
                                        line,
                                        message: format!(
                                            "Circular \\{cmd}{{{arg}}} (already including {key})"
                                        ),
                                    });
                                } else {
                                    let sub = lexer::tokenize(content);
                                    visited.push(key);
                                    let mut flat =
                                        flatten_inner(sub, files, warnings, visited, depth + 1);
                                    visited.pop();
                                    out.append(&mut flat);
                                }
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

fn is_verbatim_env(name: &str) -> bool {
    matches!(
        name,
        "verbatim" | "verbatim*" | "Verbatim" | "lstlisting" | "minted"
    )
}

/// Resolves the `\input` argument against the file map. Strips leading `./`
/// and tries the bare key, then `<key>.tex`, then `<key>.ltx`.
fn resolve_target<'a>(arg: &str, files: &'a HashMap<String, String>) -> Option<(String, &'a str)> {
    let cleaned = arg.trim_start_matches("./");
    for candidate in [
        cleaned.to_string(),
        format!("{cleaned}.tex"),
        format!("{cleaned}.ltx"),
    ] {
        if let Some(content) = files.get(&candidate) {
            return Some((candidate, content.as_str()));
        }
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

    #[test]
    fn flatten_resolves_with_tex_extension() {
        let main = "\\input{intro}";
        let mut files = HashMap::new();
        // Note: only the .tex-suffixed key exists.
        files.insert("intro.tex".to_string(), "hello".to_string());

        let tokens = lexer::tokenize(main);
        let mut warnings = Vec::new();
        let flat = flatten_includes(tokens, &files, &mut warnings);

        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert!(text_concat(&flat).contains("hello"));
    }

    #[test]
    fn flatten_accepts_explicit_extension() {
        let main = "\\input{intro.tex}";
        let mut files = HashMap::new();
        files.insert("intro.tex".to_string(), "hello".to_string());

        let tokens = lexer::tokenize(main);
        let mut warnings = Vec::new();
        let flat = flatten_includes(tokens, &files, &mut warnings);

        assert!(warnings.is_empty());
        assert!(text_concat(&flat).contains("hello"));
    }

    #[test]
    fn flatten_strips_leading_dot_slash() {
        let main = "\\input{./sec/intro}";
        let mut files = HashMap::new();
        files.insert("sec/intro.tex".to_string(), "deep".to_string());

        let tokens = lexer::tokenize(main);
        let mut warnings = Vec::new();
        let flat = flatten_includes(tokens, &files, &mut warnings);

        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        assert!(text_concat(&flat).contains("deep"));
    }

    #[test]
    fn flatten_recurses_through_nested_includes() {
        let main = "\\input{intro}";
        let mut files = HashMap::new();
        files.insert("intro".to_string(), "A \\input{deeper} Z".to_string());
        files.insert("deeper".to_string(), "MIDDLE".to_string());

        let tokens = lexer::tokenize(main);
        let mut warnings = Vec::new();
        let flat = flatten_includes(tokens, &files, &mut warnings);

        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        let concat = text_concat(&flat);
        assert!(
            concat.contains("MIDDLE"),
            "expected nested content; got {concat:?}"
        );
    }

    #[test]
    fn flatten_detects_cycles_and_warns() {
        let main = "\\input{a}";
        let mut files = HashMap::new();
        files.insert("a".to_string(), "from-a \\input{b}".to_string());
        files.insert("b".to_string(), "from-b \\input{a}".to_string());

        let tokens = lexer::tokenize(main);
        let mut warnings = Vec::new();
        let flat = flatten_includes(tokens, &files, &mut warnings);

        // Both bodies should appear once. The cycle should be cut, not infinite.
        let concat = text_concat(&flat);
        assert!(concat.contains("from-a"));
        assert!(concat.contains("from-b"));
        assert!(
            warnings.iter().any(|w| w.message.contains("Circular")),
            "expected a Circular warning, got: {warnings:?}"
        );
    }

    #[test]
    fn flatten_skips_inside_verbatim_env() {
        let main = "\\begin{verbatim}\\input{ghost}\\end{verbatim}";
        let files = HashMap::new(); // ghost is not in the map

        let tokens = lexer::tokenize(main);
        let mut warnings = Vec::new();
        let flat = flatten_includes(tokens, &files, &mut warnings);

        // No warning: the \input wasn't even considered (inside verbatim).
        assert!(
            warnings.is_empty(),
            "should not warn for \\input inside verbatim; got {warnings:?}"
        );
        // The \input command must still be present in the output stream.
        let has_input_cmd = flat
            .iter()
            .any(|t| matches!(t, Token::Command { name, .. } if name == "input"));
        assert!(
            has_input_cmd,
            "\\input inside verbatim must be preserved literally"
        );
    }

    #[test]
    fn flatten_include_behaves_like_input() {
        let main = "\\include{intro}";
        let mut files = HashMap::new();
        files.insert("intro".to_string(), "INC".to_string());

        let tokens = lexer::tokenize(main);
        let mut warnings = Vec::new();
        let flat = flatten_includes(tokens, &files, &mut warnings);

        assert!(warnings.is_empty());
        assert!(text_concat(&flat).contains("INC"));
    }

    #[test]
    fn flatten_missing_file_emits_warning() {
        let main = "alpha \\input{ghost} omega";
        let files = HashMap::new();

        let tokens = lexer::tokenize(main);
        let mut warnings = Vec::new();
        let flat = flatten_includes(tokens, &files, &mut warnings);

        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].message.contains("ghost"));
        assert!(warnings[0].message.contains("file not found"));
        // \input directive and its arg should be dropped — surrounding text remains.
        let concat = text_concat(&flat);
        assert!(concat.contains("alpha"));
        assert!(concat.contains("omega"));
        assert!(!concat.contains("ghost"));
    }

    #[test]
    fn flatten_no_brace_argument_keeps_command() {
        // Malformed: \input with no following brace at all.
        let main = "\\input";
        let files = HashMap::new();

        let tokens = lexer::tokenize(main);
        let mut warnings = Vec::new();
        let flat = flatten_includes(tokens, &files, &mut warnings);

        let has_input_cmd = flat
            .iter()
            .any(|t| matches!(t, Token::Command { name, .. } if name == "input"));
        assert!(has_input_cmd);
        assert!(warnings.is_empty());
    }
}
