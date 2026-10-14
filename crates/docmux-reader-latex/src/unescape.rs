/// Revert LaTeX special character escaping.
///
/// Handles the 10 escape sequences produced by the LaTeX writer's
/// `escape_latex()` function, converting them back to plain characters.
pub fn unescape_latex(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            let rest: String = chars.clone().collect();
            if rest.starts_with("textbackslash{}") {
                out.push('\\');
                for _ in 0.."textbackslash{}".len() {
                    chars.next();
                }
            } else if rest.starts_with("textasciitilde{}") {
                out.push('~');
                for _ in 0.."textasciitilde{}".len() {
                    chars.next();
                }
            } else if rest.starts_with("textasciicircum{}") {
                out.push('^');
                for _ in 0.."textasciicircum{}".len() {
                    chars.next();
                }
            } else if let Some(&next) = chars.peek() {
                match next {
                    '{' => {
                        out.push('{');
                        chars.next();
                    }
                    '}' => {
                        out.push('}');
                        chars.next();
                    }
                    '#' => {
                        out.push('#');
                        chars.next();
                    }
                    '$' => {
                        out.push('$');
                        chars.next();
                    }
                    '%' => {
                        out.push('%');
                        chars.next();
                    }
                    '&' => {
                        out.push('&');
                        chars.next();
                    }
                    '_' => {
                        out.push('_');
                        chars.next();
                    }
                    ',' | ' ' => {
                        out.push(' ');
                        chars.next();
                    }
                    // Discretionary hyphen, italic correction, end-of-sentence spacing.
                    '-' | '/' | '@' => {
                        chars.next();
                    }
                    _ => out.push('\\'),
                }
            } else {
                out.push('\\');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Apply TeX's text ligatures: ``` `` ``` and `''` quotes, `--` and `---`
/// dashes.
pub fn typographic(input: &str) -> String {
    input
        .replace("---", "\u{2014}")
        .replace("--", "\u{2013}")
        .replace("``", "\u{201C}")
        .replace("''", "\u{201D}")
        .replace('`', "\u{2018}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescape_backslash() {
        assert_eq!(unescape_latex(r"\textbackslash{}"), "\\");
    }

    #[test]
    fn unescape_braces() {
        assert_eq!(unescape_latex(r"\{hello\}"), "{hello}");
    }

    #[test]
    fn unescape_special_chars() {
        assert_eq!(unescape_latex(r"\#"), "#");
        assert_eq!(unescape_latex(r"\$"), "$");
        assert_eq!(unescape_latex(r"\%"), "%");
        assert_eq!(unescape_latex(r"\&"), "&");
        assert_eq!(unescape_latex(r"\_"), "_");
    }

    #[test]
    fn unescape_tilde_and_caret() {
        assert_eq!(unescape_latex(r"\textasciitilde{}"), "~");
        assert_eq!(unescape_latex(r"\textasciicircum{}"), "^");
    }

    #[test]
    fn unescape_mixed_text() {
        assert_eq!(
            unescape_latex(r"Price is \$10 \& tax is 5\%"),
            "Price is $10 & tax is 5%"
        );
    }

    #[test]
    fn unescape_spacing_and_hints() {
        assert_eq!(
            unescape_latex(r"e.g.\ this, hy\-phen, i.e.\@ ok, a\,b"),
            "e.g. this, hyphen, i.e. ok, a b"
        );
    }

    #[test]
    fn typographic_ligatures() {
        assert_eq!(
            typographic("a ``rank'' b `x' 1--2 a---b"),
            "a \u{201C}rank\u{201D} b \u{2018}x' 1\u{2013}2 a\u{2014}b"
        );
    }

    #[test]
    fn unescape_no_special_chars() {
        assert_eq!(unescape_latex("plain text"), "plain text");
    }
}
