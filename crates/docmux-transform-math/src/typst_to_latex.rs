//! Typst → LaTeX math notation converter.
//!
//! A text-level scanner (no tokenizer needed) that converts Typst math strings
//! to their LaTeX equivalents using the shared mapping tables.

use crate::tables::{MATHBB_TO_TYPST, TYPST_TO_LATEX_COMMANDS};

/// Convert a Typst math string to LaTeX math notation.
pub fn typst_to_latex(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut pos = 0;
    while pos < chars.len() {
        if chars[pos] == '(' {
            if let Some((num, den, end)) = try_parse_fraction(&chars, pos) {
                let converted_num = typst_to_latex(&num);
                let converted_den = typst_to_latex(&den);
                out.push_str(&format!("\\frac{{{converted_num}}}{{{converted_den}}}"));
                pos = end;
                continue;
            }
            out.push(chars[pos]);
            pos += 1;
        } else if chars[pos].is_alphabetic() {
            let ident = read_identifier(&chars, &mut pos);
            if let Some(latex) = try_double_letter_shorthand(&ident) {
                out.push_str(&latex);
            } else if pos < chars.len() && chars[pos] == '(' {
                out.push_str(&convert_function_call(&ident, &chars, &mut pos));
            } else {
                out.push_str(&convert_identifier(&ident));
            }
        } else {
            out.push(chars[pos]);
            pos += 1;
        }
    }
    out
}

/// Read consecutive alphabetic (or `.`) characters to form an identifier.
fn read_identifier(chars: &[char], pos: &mut usize) -> String {
    let mut ident = String::new();
    while *pos < chars.len() && (chars[*pos].is_alphabetic() || chars[*pos] == '.') {
        ident.push(chars[*pos]);
        *pos += 1;
    }
    ident
}

/// Try to parse `(num)/(den)` starting at a `(` character.
///
/// Returns `Some((numerator, denominator, end_position))` if the pattern
/// matches, `None` otherwise.
fn try_parse_fraction(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    let num = read_balanced_paren(chars, start)?;
    let after_num = start + num.len() + 2; // +2 for the outer parens
    if after_num >= chars.len() || chars[after_num] != '/' {
        return None;
    }
    let slash_next = after_num + 1;
    if slash_next >= chars.len() || chars[slash_next] != '(' {
        return None;
    }
    let den = read_balanced_paren(chars, slash_next)?;
    let end = slash_next + den.len() + 2; // +2 for the outer parens
    Some((num, den, end))
}

/// Read the content inside balanced parentheses starting at `start`.
///
/// `chars[start]` must be `(`. Returns the content between the outer parens
/// (not including them), or `None` if unbalanced.
fn read_balanced_paren(chars: &[char], start: usize) -> Option<String> {
    if start >= chars.len() || chars[start] != '(' {
        return None;
    }
    let mut depth = 1;
    let mut pos = start + 1;
    while pos < chars.len() && depth > 0 {
        match chars[pos] {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        pos += 1;
    }
    if depth != 0 {
        return None;
    }
    // Content is between start+1 and pos-1 (the closing paren).
    let content: String = chars[start + 1..pos - 1].iter().collect();
    Some(content)
}

/// Read the content of a parenthesized argument for a function call.
///
/// `chars[*pos]` must be `(`. Advances `pos` past the closing `)`.
fn read_paren_content(chars: &[char], pos: &mut usize) -> String {
    // Skip the opening paren.
    *pos += 1;
    let mut depth = 1;
    let mut content = String::new();
    while *pos < chars.len() && depth > 0 {
        match chars[*pos] {
            '(' => {
                depth += 1;
                content.push('(');
            }
            ')' => {
                depth -= 1;
                if depth > 0 {
                    content.push(')');
                }
            }
            ch => content.push(ch),
        }
        *pos += 1;
    }
    content
}

/// Try to map a double-letter shorthand like `RR` → `\mathbb{R}`.
fn try_double_letter_shorthand(ident: &str) -> Option<String> {
    // Build reverse: iterate MATHBB_TO_TYPST looking for the ident.
    for (letter, typst_short) in MATHBB_TO_TYPST.iter() {
        if *typst_short == ident {
            return Some(format!("\\mathbb{{{letter}}}"));
        }
    }
    None
}

/// Convert a Typst function call like `sqrt(x)` → `\sqrt{x}` or
/// `root(n, x)` → `\sqrt[n]{x}`.
fn convert_function_call(name: &str, chars: &[char], pos: &mut usize) -> String {
    let raw_args = read_paren_content(chars, pos);

    if name == "root" {
        return convert_root_call(&raw_args);
    }

    let latex_name = TYPST_TO_LATEX_COMMANDS.get(name).copied().unwrap_or(name);
    let converted_arg = typst_to_latex(raw_args.trim());
    format!("\\{latex_name}{{{converted_arg}}}")
}

/// Convert `root(n, x)` → `\sqrt[n]{x}`.
fn convert_root_call(raw_args: &str) -> String {
    // Split on the first comma only.
    if let Some((index_part, body_part)) = raw_args.split_once(',') {
        let index = typst_to_latex(index_part.trim());
        let body = typst_to_latex(body_part.trim());
        format!("\\sqrt[{index}]{{{body}}}")
    } else {
        // Fallback: treat as single argument.
        let body = typst_to_latex(raw_args.trim());
        format!("\\sqrt{{{body}}}")
    }
}

/// Map a plain Typst identifier to its LaTeX equivalent.
fn convert_identifier(ident: &str) -> String {
    if let Some(latex_cmd) = TYPST_TO_LATEX_COMMANDS.get(ident) {
        format!("\\{latex_cmd}")
    } else {
        ident.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_variable() {
        assert_eq!(typst_to_latex("x"), "x");
    }

    #[test]
    fn greek_letter() {
        assert_eq!(typst_to_latex("alpha"), r"\alpha");
    }

    #[test]
    fn operator_rename() {
        assert_eq!(typst_to_latex("integral"), r"\int");
        assert_eq!(typst_to_latex("product"), r"\prod");
    }

    #[test]
    fn subscript_superscript() {
        assert_eq!(typst_to_latex("x^2_i"), "x^2_i");
    }

    #[test]
    fn typst_function_call() {
        assert_eq!(typst_to_latex("sqrt(x)"), r"\sqrt{x}");
        assert_eq!(typst_to_latex("hat(x)"), r"\hat{x}");
    }

    #[test]
    fn typst_fraction_syntax() {
        assert_eq!(typst_to_latex("(a)/(b)"), r"\frac{a}{b}");
    }

    #[test]
    fn double_letter_shorthand() {
        assert_eq!(typst_to_latex("RR"), r"\mathbb{R}");
        assert_eq!(typst_to_latex("NN"), r"\mathbb{N}");
    }

    #[test]
    fn infinity() {
        assert_eq!(typst_to_latex("infinity"), r"\infty");
    }

    #[test]
    fn unrecognized_passthrough() {
        assert_eq!(typst_to_latex("myvar"), "myvar");
    }
}
