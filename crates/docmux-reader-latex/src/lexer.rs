/// Tokens produced by the LaTeX lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Command { name: String, line: usize },
    BeginEnv { name: String, line: usize },
    EndEnv { name: String, line: usize },
    BraceOpen,
    BraceClose,
    BracketOpen,
    BracketClose,
    Text { value: String },
    MathInline { value: String },
    MathDisplay { value: String },
    Comment { value: String },
    BlankLine,
    Tilde,
    Ampersand,
    DoubleBackslash { line: usize },
    Newline,
}

/// Tokenize a LaTeX source string into a flat sequence of tokens.
///
/// The tokenizer is a character-by-character scanner that tracks line numbers
/// (1-based). It recognises structural LaTeX constructs such as commands,
/// environments, math modes, comments, and whitespace, accumulating everything
/// else into `Text` tokens.
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut text_buf = String::new();
    let mut line: usize = 1;

    // Flush accumulated plain text as a Text token.
    macro_rules! flush_text {
        () => {
            if !text_buf.is_empty() {
                tokens.push(Token::Text {
                    value: std::mem::take(&mut text_buf),
                });
            }
        };
    }

    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        match c {
            // ----------------------------------------------------------------
            // Newline handling
            // ----------------------------------------------------------------
            '\n' => {
                // Peek ahead to see if the next non-empty line is also blank,
                // which would make this a paragraph separator (BlankLine).
                // A BlankLine is two consecutive newlines, or a newline
                // followed by a line that contains only whitespace then '\n'.
                let mut j = i + 1;
                // Skip whitespace (but not the next newline) on the next line.
                while j < len && chars[j] != '\n' && chars[j].is_whitespace() {
                    j += 1;
                }
                if j < len && chars[j] == '\n' {
                    // Blank line: consume everything up to and including the
                    // second newline.
                    flush_text!();
                    line += 1; // the '\n' at i
                    line += 1; // the '\n' at j
                    i = j + 1;
                    tokens.push(Token::BlankLine);
                } else {
                    // Single newline — emit Newline.
                    flush_text!();
                    line += 1;
                    i += 1;
                    tokens.push(Token::Newline);
                }
            }

            // ----------------------------------------------------------------
            // Comment
            // ----------------------------------------------------------------
            '%' => {
                flush_text!();
                i += 1;
                let mut comment = String::new();
                while i < len && chars[i] != '\n' {
                    comment.push(chars[i]);
                    i += 1;
                }
                // Don't consume the newline — that will be handled in the
                // next iteration.
                tokens.push(Token::Comment { value: comment });
            }

            // ----------------------------------------------------------------
            // Dollar signs: $$ (display) or $ (inline)
            // ----------------------------------------------------------------
            '$' => {
                flush_text!();
                if i + 1 < len && chars[i + 1] == '$' {
                    // Display math: $$...$$
                    i += 2;
                    let mut math = String::new();
                    while i < len {
                        if chars[i] == '$' && i + 1 < len && chars[i + 1] == '$' {
                            i += 2;
                            break;
                        }
                        if chars[i] == '\n' {
                            line += 1;
                        }
                        math.push(chars[i]);
                        i += 1;
                    }
                    tokens.push(Token::MathDisplay { value: math });
                } else {
                    // Inline math: $...$
                    i += 1;
                    let mut math = String::new();
                    while i < len {
                        if chars[i] == '$' {
                            i += 1;
                            break;
                        }
                        if chars[i] == '\n' {
                            line += 1;
                        }
                        math.push(chars[i]);
                        i += 1;
                    }
                    tokens.push(Token::MathInline { value: math });
                }
            }

            // ----------------------------------------------------------------
            // Tilde / Ampersand / Braces / Brackets
            // ----------------------------------------------------------------
            '~' => {
                flush_text!();
                i += 1;
                tokens.push(Token::Tilde);
            }
            '&' => {
                flush_text!();
                i += 1;
                tokens.push(Token::Ampersand);
            }
            '{' => {
                flush_text!();
                i += 1;
                tokens.push(Token::BraceOpen);
            }
            '}' => {
                flush_text!();
                i += 1;
                tokens.push(Token::BraceClose);
            }
            '[' => {
                flush_text!();
                i += 1;
                tokens.push(Token::BracketOpen);
            }
            ']' => {
                flush_text!();
                i += 1;
                tokens.push(Token::BracketClose);
            }

            // ----------------------------------------------------------------
            // Backslash — commands, environments, display math, double-backslash
            // ----------------------------------------------------------------
            '\\' => {
                // 1. Double backslash: \\
                if i + 1 < len && chars[i + 1] == '\\' {
                    flush_text!();
                    i += 2;
                    tokens.push(Token::DoubleBackslash { line });
                    continue;
                }

                // 2. Display math: \[...\]
                if i + 1 < len && chars[i + 1] == '[' {
                    flush_text!();
                    i += 2; // skip \[
                    let mut math = String::new();
                    while i < len {
                        if chars[i] == '\\' && i + 1 < len && chars[i + 1] == ']' {
                            i += 2;
                            break;
                        }
                        if chars[i] == '\n' {
                            line += 1;
                        }
                        math.push(chars[i]);
                        i += 1;
                    }
                    tokens.push(Token::MathDisplay { value: math });
                    continue;
                }

                // 3. \begin{name} or \end{name}
                let begin_prefix = "begin{";
                let end_prefix = "end{";

                let remaining: String = chars[i + 1..].iter().collect();

                if remaining.starts_with(begin_prefix) {
                    flush_text!();
                    i += 1 + begin_prefix.len(); // skip \begin{
                    let mut name = String::new();
                    while i < len && chars[i] != '}' {
                        name.push(chars[i]);
                        i += 1;
                    }
                    if i < len {
                        i += 1; // skip closing }
                    }
                    tokens.push(Token::BeginEnv { name, line });
                    continue;
                }

                if remaining.starts_with(end_prefix) {
                    flush_text!();
                    i += 1 + end_prefix.len(); // skip \end{
                    let mut name = String::new();
                    while i < len && chars[i] != '}' {
                        name.push(chars[i]);
                        i += 1;
                    }
                    if i < len {
                        i += 1; // skip closing }
                    }
                    tokens.push(Token::EndEnv { name, line });
                    continue;
                }

                // 4. Command: \letters (optionally followed by *)
                i += 1; // skip the backslash
                if i < len && chars[i].is_ascii_alphabetic() {
                    flush_text!();
                    let mut name = String::new();
                    while i < len && chars[i].is_ascii_alphabetic() {
                        name.push(chars[i]);
                        i += 1;
                    }
                    // Include trailing '*' in the command name.
                    if i < len && chars[i] == '*' {
                        name.push('*');
                        i += 1;
                    }
                    tokens.push(Token::Command { name, line });
                    continue;
                }

                // 5. Lone backslash — treat as text.
                text_buf.push('\\');
            }

            // ----------------------------------------------------------------
            // Everything else accumulates into a Text token
            // ----------------------------------------------------------------
            other => {
                text_buf.push(other);
                i += 1;
            }
        }
    }

    flush_text!();
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lex_plain_text() {
        let tokens = tokenize("Hello world");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Text { value } if value == "Hello world"));
    }

    #[test]
    fn lex_command() {
        let tokens = tokenize(r"\textbf");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Command { name, .. } if name == "textbf"));
    }

    #[test]
    fn lex_begin_end_env() {
        let tokens = tokenize(r"\begin{itemize}\end{itemize}");
        assert!(matches!(&tokens[0], Token::BeginEnv { name, .. } if name == "itemize"));
        assert!(matches!(&tokens[1], Token::EndEnv { name, .. } if name == "itemize"));
    }

    #[test]
    fn lex_braces() {
        let tokens = tokenize("{hello}");
        assert!(matches!(&tokens[0], Token::BraceOpen));
        assert!(matches!(&tokens[1], Token::Text { .. }));
        assert!(matches!(&tokens[2], Token::BraceClose));
    }

    #[test]
    fn lex_math_inline() {
        let tokens = tokenize("$x^2$");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::MathInline { value } if value == "x^2"));
    }

    #[test]
    fn lex_math_display_dollars() {
        let tokens = tokenize("$$E = mc^2$$");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::MathDisplay { value } if value == "E = mc^2"));
    }

    #[test]
    fn lex_math_display_brackets() {
        let tokens = tokenize(r"\[E = mc^2\]");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::MathDisplay { value } if value == "E = mc^2"));
    }

    #[test]
    fn lex_comment() {
        let tokens = tokenize("text % comment\nnext line");
        assert!(tokens.iter().any(|t| matches!(t, Token::Comment { .. })));
    }

    #[test]
    fn lex_blank_line() {
        let tokens = tokenize("para one\n\npara two");
        assert!(tokens.iter().any(|t| matches!(t, Token::BlankLine)));
    }

    #[test]
    fn lex_tilde() {
        let tokens = tokenize("word~word");
        assert!(tokens.iter().any(|t| matches!(t, Token::Tilde)));
    }

    #[test]
    fn lex_ampersand() {
        let tokens = tokenize("a & b");
        assert!(tokens.iter().any(|t| matches!(t, Token::Ampersand)));
    }

    #[test]
    fn lex_double_backslash() {
        let tokens = tokenize(r"line\\next");
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::DoubleBackslash { .. })));
    }

    #[test]
    fn lex_brackets() {
        let tokens = tokenize("[option]");
        assert!(matches!(&tokens[0], Token::BracketOpen));
        assert!(matches!(&tokens[2], Token::BracketClose));
    }

    #[test]
    fn lex_starred_command() {
        let tokens = tokenize(r"\section*");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Command { name, .. } if name == "section*"));
    }

    #[test]
    fn lex_newline_within_text() {
        let tokens = tokenize("line one\nline two");
        assert!(tokens.iter().any(|t| matches!(t, Token::Newline)));
    }
}
