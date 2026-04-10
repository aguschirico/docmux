//! LaTeX math tokenizer.
//!
//! Breaks a LaTeX math string into a stream of [`Token`]s suitable for
//! further transformation into the docmux AST.

/// A single token produced by the LaTeX math tokenizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// A LaTeX command such as `\frac` or `\alpha`. The name excludes the
    /// leading backslash.
    Command(String),
    /// Content enclosed in `{…}`, recursively tokenized.
    BraceGroup(Vec<Token>),
    /// Content inside `[…]` that immediately follows a [`Command`](Token::Command).
    OptionalArg(String),
    /// The subscript operator `_`.
    SubScript,
    /// The superscript operator `^`.
    SuperScript,
    /// A `\begin{name}…\end{name}` environment. The body is stored as a raw
    /// string (not tokenized).
    Environment {
        /// Environment name, e.g. `pmatrix`.
        name: String,
        /// Raw body between `\begin{name}` and `\end{name}`.
        body: String,
    },
    /// Plain text — letters, digits, operators, whitespace, etc.
    Text(String),
}

/// Tokenize a LaTeX math string into a flat list of [`Token`]s.
///
/// Brace groups are recursively tokenized. Environments are captured with
/// their raw body. Optional arguments (`[…]`) are only recognised
/// immediately after a command token.
pub fn tokenize_latex(input: &str) -> Vec<Token> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    tokenize_inner(&chars, &mut pos, false)
}

/// Core recursive tokenizer. When `inside_brace` is true we stop at a
/// matching `}`.
fn tokenize_inner(chars: &[char], pos: &mut usize, inside_brace: bool) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut text_buf = String::new();

    while *pos < chars.len() {
        let ch = chars[*pos];

        match ch {
            // End of a brace group — return to caller.
            '}' if inside_brace => {
                flush_text(&mut text_buf, &mut tokens);
                *pos += 1;
                return tokens;
            }

            // Backslash — command or environment.
            '\\' => {
                flush_text(&mut text_buf, &mut tokens);
                *pos += 1; // skip '\'
                let name = consume_command_name(chars, pos);

                if name == "begin" {
                    let env_name = consume_brace_raw(chars, pos);
                    let body = consume_env_body(chars, pos, &env_name);
                    tokens.push(Token::Environment {
                        name: env_name,
                        body,
                    });
                } else {
                    tokens.push(Token::Command(name));
                    // Check for optional argument immediately after command.
                    maybe_consume_optional_arg(chars, pos, &mut tokens);
                }
            }

            // Opening brace — recursive group.
            '{' => {
                flush_text(&mut text_buf, &mut tokens);
                *pos += 1; // skip '{'
                let inner = tokenize_inner(chars, pos, true);
                tokens.push(Token::BraceGroup(inner));
            }

            '_' => {
                flush_text(&mut text_buf, &mut tokens);
                tokens.push(Token::SubScript);
                *pos += 1;
            }

            '^' => {
                flush_text(&mut text_buf, &mut tokens);
                tokens.push(Token::SuperScript);
                *pos += 1;
            }

            // Everything else is plain text.
            _ => {
                text_buf.push(ch);
                *pos += 1;
            }
        }
    }

    flush_text(&mut text_buf, &mut tokens);
    tokens
}

/// If `text_buf` is non-empty, push it as a [`Token::Text`] and clear the
/// buffer.
fn flush_text(buf: &mut String, tokens: &mut Vec<Token>) {
    if !buf.is_empty() {
        tokens.push(Token::Text(buf.clone()));
        buf.clear();
    }
}

/// Consume a command name (letters only) starting at `pos`.
fn consume_command_name(chars: &[char], pos: &mut usize) -> String {
    let mut name = String::new();
    while *pos < chars.len() && chars[*pos].is_ascii_alphabetic() {
        name.push(chars[*pos]);
        *pos += 1;
    }
    name
}

/// Consume the raw content of a `{…}` group as a plain string (no recursive
/// tokenization). Used for environment names and `\end{…}` markers.
fn consume_brace_raw(chars: &[char], pos: &mut usize) -> String {
    // Skip the opening '{'.
    if *pos < chars.len() && chars[*pos] == '{' {
        *pos += 1;
    }
    let mut content = String::new();
    while *pos < chars.len() && chars[*pos] != '}' {
        content.push(chars[*pos]);
        *pos += 1;
    }
    // Skip the closing '}'.
    if *pos < chars.len() && chars[*pos] == '}' {
        *pos += 1;
    }
    content
}

/// Consume everything between `\begin{name}` and `\end{name}`, returning
/// the raw body. The `\begin{name}` has already been consumed; we also
/// consume the `\end{name}`.
fn consume_env_body(chars: &[char], pos: &mut usize, name: &str) -> String {
    let end_marker = format!("\\end{{{name}}}");
    let mut body = String::new();

    while *pos < chars.len() {
        // Check whether we are at the end marker.
        let remaining: String = chars[*pos..].iter().collect();
        if remaining.starts_with(&end_marker) {
            *pos += end_marker.len();
            return body;
        }
        body.push(chars[*pos]);
        *pos += 1;
    }

    body
}

/// If the character at `pos` is `[`, consume the content up to the matching
/// `]` and push an [`Token::OptionalArg`].
fn maybe_consume_optional_arg(chars: &[char], pos: &mut usize, tokens: &mut Vec<Token>) {
    if *pos < chars.len() && chars[*pos] == '[' {
        *pos += 1; // skip '['
        let mut content = String::new();
        while *pos < chars.len() && chars[*pos] != ']' {
            content.push(chars[*pos]);
            *pos += 1;
        }
        if *pos < chars.len() && chars[*pos] == ']' {
            *pos += 1; // skip ']'
        }
        tokens.push(Token::OptionalArg(content));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple_variable() {
        let tokens = tokenize_latex("x");
        assert_eq!(tokens, vec![Token::Text("x".into())]);
    }

    #[test]
    fn tokenize_command() {
        let tokens = tokenize_latex(r"\alpha");
        assert_eq!(tokens, vec![Token::Command("alpha".into())]);
    }

    #[test]
    fn tokenize_command_with_brace_arg() {
        let tokens = tokenize_latex(r"\frac{a}{b}");
        assert_eq!(
            tokens,
            vec![
                Token::Command("frac".into()),
                Token::BraceGroup(vec![Token::Text("a".into())]),
                Token::BraceGroup(vec![Token::Text("b".into())]),
            ]
        );
    }

    #[test]
    fn tokenize_subscript_superscript() {
        let tokens = tokenize_latex("x^2_i");
        assert_eq!(
            tokens,
            vec![
                Token::Text("x".into()),
                Token::SuperScript,
                Token::Text("2".into()),
                Token::SubScript,
                Token::Text("i".into()),
            ]
        );
    }

    #[test]
    fn tokenize_environment() {
        let tokens = tokenize_latex(r"\begin{pmatrix}a &amp; b\end{pmatrix}");
        assert_eq!(
            tokens,
            vec![Token::Environment {
                name: "pmatrix".into(),
                body: r"a &amp; b".into(),
            }]
        );
    }

    #[test]
    fn tokenize_nested_braces() {
        let tokens = tokenize_latex(r"\frac{a+{b}}{c}");
        assert_eq!(
            tokens,
            vec![
                Token::Command("frac".into()),
                Token::BraceGroup(vec![
                    Token::Text("a+".into()),
                    Token::BraceGroup(vec![Token::Text("b".into())]),
                ]),
                Token::BraceGroup(vec![Token::Text("c".into())]),
            ]
        );
    }

    #[test]
    fn tokenize_optional_arg() {
        let tokens = tokenize_latex(r"\sqrt[3]{x}");
        assert_eq!(
            tokens,
            vec![
                Token::Command("sqrt".into()),
                Token::OptionalArg("3".into()),
                Token::BraceGroup(vec![Token::Text("x".into())]),
            ]
        );
    }

    #[test]
    fn tokenize_whitespace_preserved() {
        let tokens = tokenize_latex(r"a + b");
        assert_eq!(tokens, vec![Token::Text("a + b".into())]);
    }
}
