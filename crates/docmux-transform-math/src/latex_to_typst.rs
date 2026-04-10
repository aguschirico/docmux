//! LaTeX → Typst math notation converter.
//!
//! Tokenizes a LaTeX math string and walks the token list, converting each
//! token into its Typst equivalent using the mapping tables.

use crate::tables::{
    LATEX_ENV_TO_TYPST, LATEX_TO_TYPST_COMMANDS, LATEX_TO_TYPST_FUNCTIONS, MATHBB_TO_TYPST,
};
use crate::tokenizer::{tokenize_latex, Token};

/// Convert a LaTeX math string to Typst math notation.
pub fn latex_to_typst(input: &str) -> String {
    let tokens = tokenize_latex(input);
    tokens_to_typst(&tokens)
}

/// Walk a slice of tokens and produce the Typst string.
fn tokens_to_typst(tokens: &[Token]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Command(name) => {
                out.push_str(&convert_command(name, tokens, &mut i));
            }
            Token::Text(t) => {
                out.push_str(t);
                i += 1;
            }
            Token::SubScript => {
                out.push('_');
                i += 1;
            }
            Token::SuperScript => {
                out.push('^');
                i += 1;
            }
            Token::BraceGroup(inner) => {
                out.push_str(&tokens_to_typst(inner));
                i += 1;
            }
            Token::OptionalArg(arg) => {
                // Standalone optional arg (shouldn't normally appear without a command).
                out.push('[');
                out.push_str(arg);
                out.push(']');
                i += 1;
            }
            Token::Environment { name, body } => {
                out.push_str(&convert_environment(name, body));
                i += 1;
            }
        }
    }
    out
}

/// Handle a command token, consuming additional brace-group arguments as needed.
///
/// `i` points at the command token on entry and is advanced past all consumed
/// tokens on exit.
fn convert_command(name: &str, tokens: &[Token], i: &mut usize) -> String {
    *i += 1; // skip the Command token itself

    match name {
        "frac" => convert_frac(tokens, i),
        "sqrt" => convert_sqrt(tokens, i),
        "text" => convert_text(tokens, i),
        "mathbb" => convert_mathbb(tokens, i),
        _ if LATEX_TO_TYPST_FUNCTIONS.contains_key(name) => convert_function(name, tokens, i),
        _ if LATEX_TO_TYPST_COMMANDS.contains_key(name) => {
            let typst = LATEX_TO_TYPST_COMMANDS[name];
            typst.to_string()
        }
        _ => format!("\\{name}"),
    }
}

/// `\frac{num}{den}` → `(num)/(den)`
fn convert_frac(tokens: &[Token], i: &mut usize) -> String {
    let num = consume_brace_arg(tokens, i);
    let den = consume_brace_arg(tokens, i);
    format!("({num})/({den})")
}

/// `\sqrt{x}` → `sqrt(x)`, `\sqrt[n]{x}` → `root(n, x)`
fn convert_sqrt(tokens: &[Token], i: &mut usize) -> String {
    let opt = consume_optional_arg(tokens, i);
    let body = consume_brace_arg(tokens, i);
    match opt {
        Some(n) => format!("root({n}, {body})"),
        None => format!("sqrt({body})"),
    }
}

/// `\text{hello}` → `"hello"`
fn convert_text(tokens: &[Token], i: &mut usize) -> String {
    let raw = consume_brace_arg_raw(tokens, i);
    format!("\"{raw}\"")
}

/// `\mathbb{R}` → `RR` (via `MATHBB_TO_TYPST`), fallback to `bb(X)`
fn convert_mathbb(tokens: &[Token], i: &mut usize) -> String {
    let raw = consume_brace_arg_raw(tokens, i);
    MATHBB_TO_TYPST
        .get(raw.as_str())
        .map(|s| (*s).to_string())
        .unwrap_or_else(|| format!("bb({raw})"))
}

/// Generic decoration / font command: `\hat{x}` → `hat(x)`
fn convert_function(name: &str, tokens: &[Token], i: &mut usize) -> String {
    let typst_fn = LATEX_TO_TYPST_FUNCTIONS[name];
    let arg = consume_brace_arg(tokens, i);
    format!("{typst_fn}({arg})")
}

/// Consume the next token if it is a `BraceGroup` and return the recursively
/// converted Typst string. Returns an empty string if the next token is not a
/// brace group (graceful fallback).
fn consume_brace_arg(tokens: &[Token], i: &mut usize) -> String {
    if *i < tokens.len() {
        if let Token::BraceGroup(inner) = &tokens[*i] {
            let result = tokens_to_typst(inner);
            *i += 1;
            return result;
        }
    }
    String::new()
}

/// Consume the next token if it is a `BraceGroup` and return the raw text
/// content (without recursive Typst conversion). Useful for `\text` and
/// `\mathbb` where the content is literal.
fn consume_brace_arg_raw(tokens: &[Token], i: &mut usize) -> String {
    if *i < tokens.len() {
        if let Token::BraceGroup(inner) = &tokens[*i] {
            let result = raw_text(inner);
            *i += 1;
            return result;
        }
    }
    String::new()
}

/// Consume the next token if it is an `OptionalArg`, returning `Some(content)`.
fn consume_optional_arg(tokens: &[Token], i: &mut usize) -> Option<String> {
    if *i < tokens.len() {
        if let Token::OptionalArg(content) = &tokens[*i] {
            let result = content.clone();
            *i += 1;
            return Some(result);
        }
    }
    None
}

/// Extract the raw text from a slice of tokens (concatenate all text content).
fn raw_text(tokens: &[Token]) -> String {
    let mut out = String::new();
    for t in tokens {
        match t {
            Token::Text(s) => out.push_str(s),
            Token::Command(name) => {
                out.push('\\');
                out.push_str(name);
            }
            Token::BraceGroup(inner) => {
                out.push('{');
                out.push_str(&raw_text(inner));
                out.push('}');
            }
            Token::SubScript => out.push('_'),
            Token::SuperScript => out.push('^'),
            Token::OptionalArg(arg) => {
                out.push('[');
                out.push_str(arg);
                out.push(']');
            }
            Token::Environment { name, body } => {
                out.push_str(&format!("\\begin{{{name}}}{body}\\end{{{name}}}"));
            }
        }
    }
    out
}

/// Convert a `\begin{env}…\end{env}` environment to Typst.
fn convert_environment(name: &str, body: &str) -> String {
    let (typst_fn, delim) = LATEX_ENV_TO_TYPST.get(name).copied().unwrap_or(("", ""));

    if typst_fn.is_empty() {
        // Unknown environment — pass through raw.
        return format!("\\begin{{{name}}}{body}\\end{{{name}}}");
    }

    if typst_fn == "cases" {
        return convert_cases(typst_fn, body);
    }

    convert_matrix(typst_fn, delim, body)
}

/// Convert matrix-like environments: rows split by `\\`, columns by `&`.
fn convert_matrix(func: &str, delim: &str, body: &str) -> String {
    let rows: Vec<&str> = body.split("\\\\").collect();
    let mut cells: Vec<String> = Vec::new();
    let mut row_strs: Vec<String> = Vec::new();

    for row in &rows {
        let trimmed = row.trim();
        if trimmed.is_empty() {
            continue;
        }
        let cols: Vec<String> = trimmed
            .split('&')
            .map(|c| latex_to_typst(c.trim()))
            .collect();
        row_strs.push(cols.join(", "));
    }

    cells.extend(row_strs);
    let body_str = cells.join("; ");

    if delim.is_empty() {
        format!("{func}({body_str})")
    } else {
        format!("{func}(delim: {delim}, {body_str})")
    }
}

/// Convert `cases` environments: rows split by `\\`, joined with `, `.
fn convert_cases(func: &str, body: &str) -> String {
    let rows: Vec<&str> = body.split("\\\\").collect();
    let mut parts: Vec<String> = Vec::new();

    for row in &rows {
        let trimmed = row.trim();
        if trimmed.is_empty() {
            continue;
        }
        // For cases, keep `&` as-is in the output.
        let converted = latex_to_typst(trimmed);
        parts.push(converted);
    }

    format!("{func}({})", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_variable() {
        assert_eq!(latex_to_typst("x"), "x");
    }

    #[test]
    fn greek_letter() {
        assert_eq!(latex_to_typst(r"\alpha"), "alpha");
    }

    #[test]
    fn fraction() {
        assert_eq!(latex_to_typst(r"\frac{a}{b}"), "(a)/(b)");
    }

    #[test]
    fn sqrt_basic() {
        assert_eq!(latex_to_typst(r"\sqrt{x}"), "sqrt(x)");
    }

    #[test]
    fn sqrt_nth() {
        assert_eq!(latex_to_typst(r"\sqrt[3]{x}"), "root(3, x)");
    }

    #[test]
    fn subscript_superscript() {
        assert_eq!(latex_to_typst("x^2_i"), "x^2_i");
    }

    #[test]
    fn operator() {
        assert_eq!(latex_to_typst(r"\int_0^1"), "integral_0^1");
    }

    #[test]
    fn decoration() {
        assert_eq!(latex_to_typst(r"\hat{x}"), "hat(x)");
        assert_eq!(latex_to_typst(r"\bar{x}"), "overline(x)");
    }

    #[test]
    fn mathbb() {
        assert_eq!(latex_to_typst(r"\mathbb{R}"), "RR");
        assert_eq!(latex_to_typst(r"\mathbb{N}"), "NN");
    }

    #[test]
    fn environment_pmatrix() {
        assert_eq!(
            latex_to_typst(r"\begin{pmatrix}a & b \\ c & d\end{pmatrix}"),
            "mat(delim: \"(\", a, b; c, d)"
        );
    }

    #[test]
    fn environment_cases() {
        assert_eq!(
            latex_to_typst(r"\begin{cases}x & y \\ a & b\end{cases}"),
            "cases(x & y, a & b)"
        );
    }

    #[test]
    fn nested_fraction() {
        assert_eq!(latex_to_typst(r"\frac{\frac{a}{b}}{c}"), "((a)/(b))/(c)");
    }

    #[test]
    fn unrecognized_command_passthrough() {
        assert_eq!(latex_to_typst(r"\mycommand"), r"\mycommand");
    }

    #[test]
    fn spacing() {
        assert_eq!(latex_to_typst(r"\quad"), "quad");
    }

    #[test]
    fn complex_expression() {
        assert_eq!(
            latex_to_typst(r"\frac{\partial f}{\partial x}"),
            "(partial f)/(partial x)"
        );
    }
}
