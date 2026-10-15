//! LaTeX math → plain Unicode text, for places that only hold a string
//! (citation notes, bookmarks). Best effort: symbols become their Unicode
//! character, font and text wrappers keep their content, scripts keep their
//! `_` and `^`, and unknown commands keep their name.

use crate::tables::LATEX_TO_UNICODE;
use crate::tokenizer::{tokenize_latex, Token};

/// Convert a LaTeX math string to plain text: `y_\mathrm{DP}` → `y_DP`,
/// `4.4\,\sigma` → `4.4 σ`.
pub fn latex_to_text(input: &str) -> String {
    let mut out = String::new();
    write(&tokenize_latex(input), &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn write(tokens: &[Token], out: &mut String) {
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Text(t) => out.push_str(t),
            Token::BraceGroup(inner) => write(inner, out),
            Token::OptionalArg(_) => {}
            Token::SubScript => out.push('_'),
            Token::SuperScript => out.push('^'),
            Token::Environment { body, .. } => write(&tokenize_latex(body), out),
            Token::Command(name) => match name.as_str() {
                "," | ";" | ":" | ">" | " " | "thinspace" | "quad" | "qquad" | "enspace" => {
                    out.push(' ')
                }
                "!" | "left" | "right" | "big" | "Big" | "bigg" | "Bigg" | "displaystyle"
                | "textstyle" | "nonumber" | "notag" => {}
                // {TeX version}{PDF version}: the second is already plain text.
                "texorpdfstring" => {
                    if let Some(Token::BraceGroup(pdf)) = tokens.get(i + 2) {
                        write(pdf, out);
                        i += 2;
                    }
                }
                _ => match LATEX_TO_UNICODE.get(name.as_str()) {
                    Some(symbol) => out.push_str(symbol),
                    // Wrappers (\mathrm, \text, \mathbf) leave their argument.
                    None if matches!(tokens.get(i + 1), Some(Token::BraceGroup(_))) => {}
                    None => out.push_str(name),
                },
            },
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_wrappers_and_spaces() {
        assert_eq!(latex_to_text(r"4.4\thinspace\sigma"), "4.4 σ");
        assert_eq!(latex_to_text(r"y_\mathrm{DP}"), "y_DP");
        assert_eq!(latex_to_text(r"\texorpdfstring{\oldell}{ℓ}"), "ℓ");
        assert_eq!(latex_to_text(r"\alpha \leq 1"), "α ≤ 1");
    }
}
