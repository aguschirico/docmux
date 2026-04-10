//! LaTeX → MathML converter.
//!
//! Tokenizes a LaTeX math string and walks the token list, converting each
//! token into its MathML equivalent. Uses the shared `LATEX_TO_UNICODE` table
//! for Greek letters and symbols.

use crate::tables::LATEX_TO_UNICODE;
use crate::tokenizer::{tokenize_latex, Token};

/// Convert a LaTeX math string to MathML markup (without the outer `<math>` wrapper).
pub fn latex_to_mathml(input: &str) -> String {
    let tokens = tokenize_latex(input);
    let elements = tokens_to_elements(&tokens);
    elements.join("")
}

/// Wrap MathML output in `<math display="...">` tags.
///
/// When `display` is true the wrapper uses `display="block"` (display math);
/// otherwise it uses `display="inline"`.
pub fn wrap_mathml(input: &str, display: bool) -> String {
    let inner = latex_to_mathml(input);
    let mode = if display { "block" } else { "inline" };
    format!("<math display=\"{mode}\">{inner}</math>")
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Accent command → Unicode combining character.
fn accent_char(name: &str) -> Option<char> {
    match name {
        "hat" => Some('\u{0302}'),
        "bar" | "overline" => Some('\u{0304}'),
        "vec" => Some('\u{20D7}'),
        "tilde" => Some('\u{0303}'),
        "dot" => Some('\u{0307}'),
        "ddot" => Some('\u{0308}'),
        _ => None,
    }
}

/// Font command → MathML `mathvariant` attribute value.
fn font_variant(name: &str) -> Option<&'static str> {
    match name {
        "mathbb" => Some("double-struck"),
        "mathbf" => Some("bold"),
        "mathit" => Some("italic"),
        "mathrm" => Some("normal"),
        "mathcal" => Some("script"),
        _ => None,
    }
}

/// Characters that should be rendered as MathML `<mo>` operators.
fn is_operator(ch: char) -> bool {
    matches!(
        ch,
        '+' | '-'
            | '='
            | '<'
            | '>'
            | '('
            | ')'
            | '['
            | ']'
            | ','
            | ';'
            | '!'
            | '|'
            | '/'
            | '*'
            | '.'
    )
}

/// Convert a text fragment (letters, digits, operators) into a list of MathML
/// elements, grouping consecutive digits into a single `<mn>`.
fn text_to_elements(text: &str) -> Vec<String> {
    let mut elems: Vec<String> = Vec::new();
    let mut digit_buf = String::new();

    for ch in text.chars() {
        if ch.is_ascii_digit() {
            digit_buf.push(ch);
            continue;
        }
        flush_digits(&mut digit_buf, &mut elems);
        if ch.is_whitespace() {
            continue; // MathML handles spacing
        }
        if is_operator(ch) {
            elems.push(format!("<mo>{ch}</mo>"));
        } else if ch.is_alphabetic() {
            elems.push(format!("<mi>{ch}</mi>"));
        } else {
            elems.push(format!("<mo>{ch}</mo>"));
        }
    }
    flush_digits(&mut digit_buf, &mut elems);
    elems
}

/// If the digit buffer is non-empty, push an `<mn>` element and clear it.
fn flush_digits(buf: &mut String, elems: &mut Vec<String>) {
    if !buf.is_empty() {
        elems.push(format!("<mn>{buf}</mn>"));
        buf.clear();
    }
}

/// Convert a token list into a `Vec<String>` of MathML elements. Each entry
/// is a complete, self-contained MathML fragment (e.g. `<mi>x</mi>`).
///
/// Sub/superscripts are handled by popping the previous element and wrapping
/// it together with the next element.
fn tokens_to_elements(tokens: &[Token]) -> Vec<String> {
    let mut elems: Vec<String> = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        match &tokens[i] {
            Token::Text(t) => {
                elems.extend(text_to_elements(t));
                i += 1;
            }
            Token::Command(name) => {
                i += 1;
                let cmd_elems = convert_command(name, tokens, &mut i);
                elems.extend(cmd_elems);
            }
            Token::BraceGroup(inner) => {
                elems.extend(tokens_to_elements(inner));
                i += 1;
            }
            Token::SuperScript => {
                i += 1;
                let base = pop_last_element(&mut elems);
                let sup = consume_next_element(tokens, &mut i);
                elems.push(format!("<msup>{base}{sup}</msup>"));
            }
            Token::SubScript => {
                i += 1;
                let base = pop_last_element(&mut elems);
                let sub = consume_next_element(tokens, &mut i);
                elems.push(format!("<msub>{base}{sub}</msub>"));
            }
            Token::OptionalArg(arg) => {
                // Standalone optional arg — render as text.
                elems.push(format!("<mtext>[{arg}]</mtext>"));
                i += 1;
            }
            Token::Environment { body, .. } => {
                elems.push(format!("<mrow>{}</mrow>", latex_to_mathml(body)));
                i += 1;
            }
        }
    }
    elems
}

/// Pop the last element from the output list, falling back to `<mrow/>`.
fn pop_last_element(elems: &mut Vec<String>) -> String {
    elems.pop().unwrap_or_else(|| "<mrow/>".to_string())
}

/// Consume the next token and return it as a single MathML element string.
fn consume_next_element(tokens: &[Token], i: &mut usize) -> String {
    if *i >= tokens.len() {
        return "<mrow/>".to_string();
    }
    match &tokens[*i] {
        Token::Text(t) => {
            *i += 1;
            let parts = text_to_elements(t);
            parts.into_iter().next().unwrap_or_default()
        }
        Token::Command(name) => {
            let name = name.clone();
            *i += 1;
            let parts = convert_command(&name, tokens, i);
            join_or_wrap(parts)
        }
        Token::BraceGroup(inner) => {
            *i += 1;
            let parts = tokens_to_elements(inner);
            join_or_wrap(parts)
        }
        _ => {
            *i += 1;
            "<mrow/>".to_string()
        }
    }
}

/// Join multiple elements — if there is exactly one, return it directly;
/// otherwise wrap in `<mrow>`.
fn join_or_wrap(parts: Vec<String>) -> String {
    if parts.len() == 1 {
        return parts.into_iter().next().unwrap_or_default();
    }
    if parts.is_empty() {
        return "<mrow/>".to_string();
    }
    format!("<mrow>{}</mrow>", parts.join(""))
}

/// Handle a command token, consuming additional arguments as needed.
/// Returns a `Vec` of MathML elements produced by this command.
fn convert_command(name: &str, tokens: &[Token], i: &mut usize) -> Vec<String> {
    match name {
        "frac" => vec![convert_frac(tokens, i)],
        "sqrt" => vec![convert_sqrt(tokens, i)],
        "text" => vec![convert_text(tokens, i)],
        n if accent_char(n).is_some() => vec![convert_accent(n, tokens, i)],
        n if font_variant(n).is_some() => vec![convert_font(n, tokens, i)],
        _ if LATEX_TO_UNICODE.contains_key(name) => {
            let ch = LATEX_TO_UNICODE[name];
            vec![format!("<mi>{ch}</mi>")]
        }
        _ => vec![format!("<mtext>\\{name}</mtext>")],
    }
}

/// `\frac{a}{b}` → `<mfrac><mi>a</mi><mi>b</mi></mfrac>`
fn convert_frac(tokens: &[Token], i: &mut usize) -> String {
    let num = consume_brace_group_mathml(tokens, i);
    let den = consume_brace_group_mathml(tokens, i);
    format!("<mfrac>{num}{den}</mfrac>")
}

/// `\sqrt{x}` → `<msqrt>…</msqrt>`,  `\sqrt[n]{x}` → `<mroot>…</mroot>`
fn convert_sqrt(tokens: &[Token], i: &mut usize) -> String {
    let opt = consume_optional_arg(tokens, i);
    let body = consume_brace_group_mathml(tokens, i);
    match opt {
        Some(n) => {
            let index = text_to_elements(&n).join("");
            format!("<mroot>{body}{index}</mroot>")
        }
        None => format!("<msqrt>{body}</msqrt>"),
    }
}

/// `\text{hello}` → `<mtext>hello</mtext>`
fn convert_text(tokens: &[Token], i: &mut usize) -> String {
    let raw = consume_brace_arg_raw(tokens, i);
    format!("<mtext>{raw}</mtext>")
}

/// `\hat{x}` → `<mover accent="true"><mi>x</mi><mo>̂</mo></mover>`
fn convert_accent(name: &str, tokens: &[Token], i: &mut usize) -> String {
    let body = consume_brace_group_mathml(tokens, i);
    let ch = accent_char(name).unwrap_or('\u{0302}');
    format!("<mover accent=\"true\">{body}<mo>{ch}</mo></mover>")
}

/// `\mathbb{R}` → `<mstyle mathvariant="double-struck"><mi>R</mi></mstyle>`
fn convert_font(name: &str, tokens: &[Token], i: &mut usize) -> String {
    let body = consume_brace_group_mathml(tokens, i);
    let variant = font_variant(name).unwrap_or("normal");
    format!("<mstyle mathvariant=\"{variant}\">{body}</mstyle>")
}

/// Consume the next brace group and return its MathML rendering.
fn consume_brace_group_mathml(tokens: &[Token], i: &mut usize) -> String {
    if *i < tokens.len() {
        if let Token::BraceGroup(inner) = &tokens[*i] {
            let parts = tokens_to_elements(inner);
            *i += 1;
            return parts.join("");
        }
    }
    String::new()
}

/// Consume the next optional arg (`[…]`), if present.
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

/// Consume the next brace group and return its raw text content.
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

/// Extract raw text from a slice of tokens.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_variable() {
        assert_eq!(latex_to_mathml("x"), "<mi>x</mi>");
    }

    #[test]
    fn number() {
        assert_eq!(latex_to_mathml("42"), "<mn>42</mn>");
    }

    #[test]
    fn greek_letter() {
        assert_eq!(latex_to_mathml(r"\alpha"), "<mi>\u{03B1}</mi>");
    }

    #[test]
    fn operator_symbol() {
        assert_eq!(latex_to_mathml(r"\infty"), "<mi>\u{221E}</mi>");
    }

    #[test]
    fn superscript() {
        assert_eq!(latex_to_mathml("x^2"), "<msup><mi>x</mi><mn>2</mn></msup>");
    }

    #[test]
    fn subscript() {
        assert_eq!(latex_to_mathml("x_i"), "<msub><mi>x</mi><mi>i</mi></msub>");
    }

    #[test]
    fn fraction() {
        assert_eq!(
            latex_to_mathml(r"\frac{a}{b}"),
            "<mfrac><mi>a</mi><mi>b</mi></mfrac>"
        );
    }

    #[test]
    fn sqrt_basic() {
        assert_eq!(latex_to_mathml(r"\sqrt{x}"), "<msqrt><mi>x</mi></msqrt>");
    }

    #[test]
    fn sqrt_nth() {
        assert_eq!(
            latex_to_mathml(r"\sqrt[3]{x}"),
            "<mroot><mi>x</mi><mn>3</mn></mroot>"
        );
    }

    #[test]
    fn plus_operator() {
        let result = latex_to_mathml("a + b");
        assert_eq!(result, "<mi>a</mi><mo>+</mo><mi>b</mi>");
    }

    #[test]
    fn unrecognized_command() {
        assert_eq!(latex_to_mathml(r"\mycommand"), "<mtext>\\mycommand</mtext>");
    }

    #[test]
    fn wrap_display() {
        assert_eq!(
            wrap_mathml("x", true),
            "<math display=\"block\"><mi>x</mi></math>"
        );
    }

    #[test]
    fn wrap_inline() {
        assert_eq!(
            wrap_mathml("x", false),
            "<math display=\"inline\"><mi>x</mi></math>"
        );
    }
}
