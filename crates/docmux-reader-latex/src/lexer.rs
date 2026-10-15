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

                // Compare in place: collecting the rest of the file for every
                // backslash made lexing quadratic on long papers.
                let starts_with = |prefix: &str| {
                    let mut rest = chars[i + 1..].iter();
                    prefix.chars().all(|c| rest.next() == Some(&c))
                };

                if starts_with(begin_prefix) {
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

                if starts_with(end_prefix) {
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

                // 4. Accents spelled with a letter: \v{c}, \c c, \H{o}. Checked
                //    before commands, so `\v{c}` is not read as the command \v.
                if let Some((accented, consumed)) = letter_accent(&chars[i + 1..]) {
                    text_buf.push_str(&accented);
                    i += 1 + consumed;
                    continue;
                }

                // 4b. Command: \letters (optionally followed by *)
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
                    // \verb|text|: any delimiter, verbatim content. Emitted as
                    // \verb{text} so the parser reads one brace argument.
                    if name.starts_with("verb") && name.len() <= 5 && i < len {
                        let delim = chars[i];
                        if let Some(close) = chars[i + 1..].iter().position(|&c| c == delim) {
                            let value: String = chars[i + 1..i + 1 + close].iter().collect();
                            i += close + 2;
                            tokens.push(Token::Command { name, line });
                            tokens.push(Token::BraceOpen);
                            tokens.push(Token::Text { value });
                            tokens.push(Token::BraceClose);
                            continue;
                        }
                    }
                    tokens.push(Token::Command { name, line });
                    continue;
                }

                // 5. Inline math: \(...\)
                if i < len && chars[i] == '(' {
                    flush_text!();
                    i += 1;
                    let mut math = String::new();
                    while i < len && !(chars[i] == '\\' && chars.get(i + 1) == Some(&')')) {
                        if chars[i] == '\n' {
                            line += 1;
                        }
                        math.push(chars[i]);
                        i += 1;
                    }
                    i = (i + 2).min(len);
                    tokens.push(Token::MathInline { value: math });
                    continue;
                }

                // 6. Accents: \'e, \"{o}, \~n → the accented character.
                if let Some((accented, consumed)) = accent(&chars[i..]) {
                    text_buf.push_str(&accented);
                    i += consumed;
                    continue;
                }

                // 7. Control symbols (\%, \$, \&, \#, \_, \{, \}, \,, ...): keep
                //    the escape in the text so `%` and `$` are not read as a
                //    comment or math; `unescape_latex` resolves it later.
                text_buf.push('\\');
                if i < len {
                    text_buf.push(chars[i]);
                    i += 1;
                }
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

/// Resolve an accent command named by a letter, starting right after the
/// backslash: `v{c}`, `c{\i}`, or `v c` with a space. `\vspace` and `\cite`
/// do not match, since the letter must be followed by a brace or a space.
fn letter_accent(rest: &[char]) -> Option<(String, usize)> {
    let mark = match rest.first()? {
        'c' => '\u{0327}',
        'v' => '\u{030C}',
        'u' => '\u{0306}',
        'H' => '\u{030B}',
        'k' => '\u{0328}',
        'r' => '\u{030A}',
        'd' => '\u{0323}',
        _ => return None,
    };
    let (base, consumed) = match rest.get(1..) {
        Some(['{', b, '}', ..]) if b.is_alphabetic() => (*b, 4),
        Some(['{', '\\', 'i', '}', ..]) => ('i', 5),
        Some([' ', b, ..]) if b.is_alphabetic() => (*b, 3),
        _ => return None,
    };
    Some((compose(base, mark), consumed))
}

/// Resolve a text-mode accent command starting right after the backslash:
/// `'e`, `"{o}`, `` `a ``, `^\i`. Returns the accented text and the number of
/// characters consumed.
fn accent(rest: &[char]) -> Option<(String, usize)> {
    let mark = match rest.first()? {
        '\'' => '\u{0301}',
        '`' => '\u{0300}',
        '^' => '\u{0302}',
        '"' => '\u{0308}',
        '~' => '\u{0303}',
        '=' => '\u{0304}',
        '.' => '\u{0307}',
        _ => return None,
    };
    let (base, consumed) = match rest.get(1..) {
        Some(['{', b, '}', ..]) if b.is_alphabetic() => (*b, 4),
        Some(['{', '\\', 'i', '}', ..]) => ('i', 5),
        // `\i` is a command: a following space only terminates it.
        Some(['\\', 'i', ..]) => ('i', if rest.get(3) == Some(&' ') { 4 } else { 3 }),
        Some([b, ..]) if b.is_alphabetic() => (*b, 2),
        _ => return None,
    };
    Some((compose(base, mark), consumed))
}

/// Precomposed form for common Latin letters, else base + combining mark.
fn compose(base: char, mark: char) -> String {
    const TABLE: &[(char, &str, &str)] = &[
        (
            '\u{0301}',
            "aeiouyAEIOUYncsznNCSZ",
            "áéíóúýÁÉÍÓÚÝńćśźňŃĆŚŹŇ",
        ),
        ('\u{0300}', "aeiouyAEIOUY", "àèìòùỳÀÈÌÒÙỲ"),
        ('\u{0302}', "aeiouAEIOU", "âêîôûÂÊÎÔÛ"),
        ('\u{0308}', "aeiouyAEIOU", "äëïöüÿÄËÏÖÜ"),
        ('\u{0303}', "anoANO", "ãñõÃÑÕ"),
        ('\u{0304}', "aeiouAEIOU", "āēīōūĀĒĪŌŪ"),
        ('\u{0307}', "zegcZEGCI", "żėġċŻĖĠĊİ"),
        ('\u{0327}', "cstCST", "çşţÇŞŢ"),
        ('\u{030C}', "cszrnedtCSZRNEDT", "čšžřňěďťČŠŽŘŇĚĎŤ"),
        ('\u{0306}', "aguAGU", "ăğŭĂĞŬ"),
        ('\u{030B}', "ouOU", "őűŐŰ"),
        ('\u{0328}', "aeAE", "ąęĄĘ"),
        ('\u{030A}', "auAU", "åůÅŮ"),
    ];
    TABLE
        .iter()
        .find(|(m, _, _)| *m == mark)
        .and_then(|(_, from, to)| {
            from.chars()
                .position(|c| c == base)
                .and_then(|k| to.chars().nth(k))
        })
        .map(String::from)
        .unwrap_or_else(|| format!("{base}{mark}"))
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
    fn escaped_percent_is_not_a_comment() {
        let tokens = tokenize(r"has 3.57\% error");
        assert_eq!(
            tokens,
            vec![Token::Text {
                value: r"has 3.57\% error".into()
            }]
        );
    }

    #[test]
    fn escaped_dollar_is_not_math() {
        let tokens = tokenize(r"costs \$10 or \$20");
        assert_eq!(
            tokens,
            vec![Token::Text {
                value: r"costs \$10 or \$20".into()
            }]
        );
    }

    #[test]
    fn paren_inline_math() {
        let tokens = tokenize(r"a \(x^2\) b");
        assert_eq!(
            tokens[1],
            Token::MathInline {
                value: "x^2".into()
            }
        );
    }

    #[test]
    fn letter_accents_compose() {
        let text = |src: &str| match &tokenize(src)[..] {
            [Token::Text { value }] => value.clone(),
            other => panic!("expected one text token, got {other:?}"),
        };
        assert_eq!(text(r"Kone\v{c}n\`y"), "Konečnỳ");
        assert_eq!(text(r#"G\"ul\c{c}ehre"#), "Gülçehre");
        assert_eq!(text(r"Erd\H{o}s"), "Erdős");
        assert_eq!(text(r"Pi\k{e}kos"), "Piękos");
        assert_eq!(text(r"\v Sedivy"), "Šedivy");
        assert!(
            matches!(&tokenize(r"\vspace{1em}")[0], Token::Command { name, .. } if name == "vspace")
        );
        assert!(matches!(&tokenize(r"\cite{x}")[0], Token::Command { name, .. } if name == "cite"));
    }

    #[test]
    fn accents_compose() {
        let tokens = tokenize(r#"Pe\~na, G\"{o}del, Erd\H{o}s, na\"\i ve, \'{e}t\'e"#);
        assert!(
            matches!(&tokens[0], Token::Text { value } if value.starts_with("Peña, Gödel, Erd"))
        );
        let all: String = tokens
            .iter()
            .filter_map(|t| match t {
                Token::Text { value } => Some(value.as_str()),
                _ => None,
            })
            .collect();
        assert!(all.contains("naïve"));
        assert!(all.contains("été"));
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
