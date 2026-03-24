/// Tokens produced by the Typst lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Heading { level: u8, line: usize },
    Text { value: String },
    Star,
    Underscore,
    Backtick { count: u8 },
    FuncCall { name: String, line: usize },
    ParenOpen,
    ParenClose,
    BracketOpen,
    BracketClose,
    BraceOpen,
    BraceClose,
    Dollar,
    Dash { count: u8 },
    Label { name: String },
    AtRef { name: String },
    RawFrontmatter { value: String },
    Colon,
    Comma,
    TermMarker { line: usize },
    Comment { value: String },
    BlockComment { value: String },
    BlankLine,
    Newline,
    Backslash,
    Escape { ch: char },
    StringLit { value: String },
    Plus { line: usize },
}

/// Returns true if the scanner is at the logical start of a line.
///
/// This requires both that the text buffer is empty (no unflushed content on
/// the current line) and that the last emitted token is a line-boundary token
/// (or there are no tokens yet, i.e. the very start of file).
fn at_line_start(tokens: &[Token], text_buf: &str) -> bool {
    if !text_buf.is_empty() {
        return false;
    }
    matches!(
        tokens.last(),
        None | Some(Token::Newline) | Some(Token::BlankLine) | Some(Token::RawFrontmatter { .. })
    )
}

/// Returns true for characters that have special meaning in Typst markup.
fn is_special_char(c: char) -> bool {
    matches!(
        c,
        '*' | '_' | '#' | '$' | '@' | '<' | '\\' | '`' | '/' | '[' | ']'
    )
}

/// Returns true if `c` is valid inside an identifier / label / ref name.
fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || c == '.'
}

/// Tokenize a Typst source string into a flat sequence of tokens.
///
/// The tokenizer is a character-by-character scanner that tracks line numbers
/// (1-based) and mode (code-block / math) to suppress special character
/// handling inside those regions.
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut text_buf = String::new();
    let mut line: usize = 1;
    let mut in_code_block = false;
    let mut in_math = false;

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

    // ----------------------------------------------------------------
    // YAML-style frontmatter: `---\n...\n---` at the very start of the file.
    // ----------------------------------------------------------------
    if len >= 3 && chars[0] == '-' && chars[1] == '-' && chars[2] == '-' {
        // Check that the `---` is followed by a newline (or end-of-file).
        let mut j = 3;
        if j < len && chars[j] == '\n' {
            j += 1; // skip opening `---\n`
            line += 1;
            let fm_start = j;
            // Scan for closing `---`.
            while j + 2 < len {
                if chars[j] == '-' && chars[j + 1] == '-' && chars[j + 2] == '-' {
                    // Closing delimiter found.
                    let fm_value: String = chars[fm_start..j].iter().collect();
                    // Consume the closing `---` and optional trailing newline.
                    j += 3;
                    if j < len && chars[j] == '\n' {
                        j += 1;
                        line += 1;
                    }
                    tokens.push(Token::RawFrontmatter { value: fm_value });
                    i = j;
                    break;
                }
                if chars[j] == '\n' {
                    line += 1;
                }
                j += 1;
            }
        }
    }

    while i < len {
        let c = chars[i];

        // ----------------------------------------------------------------
        // Inside a code block: only ``` closes it; everything else is text.
        // ----------------------------------------------------------------
        if in_code_block {
            if c == '`' && i + 2 < len && chars[i + 1] == '`' && chars[i + 2] == '`' {
                flush_text!();
                in_code_block = false;
                i += 3;
                tokens.push(Token::Backtick { count: 3 });
                continue;
            }
            text_buf.push(c);
            if c == '\n' {
                line += 1;
            }
            i += 1;
            continue;
        }

        // ----------------------------------------------------------------
        // Inside math mode: only $ closes it; everything else is text.
        // ----------------------------------------------------------------
        if in_math {
            if c == '$' {
                flush_text!();
                in_math = false;
                i += 1;
                tokens.push(Token::Dollar);
                continue;
            }
            text_buf.push(c);
            if c == '\n' {
                line += 1;
            }
            i += 1;
            continue;
        }

        match c {
            // ----------------------------------------------------------------
            // Newline / blank line
            // ----------------------------------------------------------------
            '\n' => {
                let mut j = i + 1;
                // Skip spaces/tabs on the next line.
                while j < len && chars[j] != '\n' && chars[j].is_whitespace() {
                    j += 1;
                }
                if j < len && chars[j] == '\n' {
                    // Two newlines → blank line (paragraph separator).
                    flush_text!();
                    line += 1; // the '\n' at i
                    line += 1; // the '\n' at j
                    i = j + 1;
                    tokens.push(Token::BlankLine);
                } else {
                    flush_text!();
                    line += 1;
                    i += 1;
                    tokens.push(Token::Newline);
                }
            }

            // ----------------------------------------------------------------
            // Heading: `=` at the start of a line
            // ----------------------------------------------------------------
            '=' if at_line_start(&tokens, &text_buf) => {
                flush_text!();
                let mut level: u8 = 0;
                while i < len && chars[i] == '=' {
                    level += 1;
                    i += 1;
                }
                // Consume optional single space after `===`.
                if i < len && chars[i] == ' ' {
                    i += 1;
                }
                tokens.push(Token::Heading { level, line });
            }

            // ----------------------------------------------------------------
            // Star
            // ----------------------------------------------------------------
            '*' => {
                flush_text!();
                i += 1;
                tokens.push(Token::Star);
            }

            // ----------------------------------------------------------------
            // Underscore
            // ----------------------------------------------------------------
            '_' => {
                flush_text!();
                i += 1;
                tokens.push(Token::Underscore);
            }

            // ----------------------------------------------------------------
            // Backtick(s)
            // ----------------------------------------------------------------
            '`' => {
                flush_text!();
                let mut count: u8 = 0;
                while i < len && chars[i] == '`' {
                    count += 1;
                    i += 1;
                }
                if count == 3 {
                    in_code_block = true;
                }
                tokens.push(Token::Backtick { count });
            }

            // ----------------------------------------------------------------
            // Dollar → math mode toggle
            // ----------------------------------------------------------------
            '$' => {
                flush_text!();
                in_math = true;
                i += 1;
                tokens.push(Token::Dollar);
            }

            // ----------------------------------------------------------------
            // Hash: either a function call (#name) or plain text
            // ----------------------------------------------------------------
            '#' => {
                if i + 1 < len && chars[i + 1].is_ascii_alphabetic() {
                    flush_text!();
                    i += 1; // skip `#`
                    let mut name = String::new();
                    while i < len
                        && (chars[i].is_alphanumeric() || chars[i] == '-' || chars[i] == '_')
                    {
                        name.push(chars[i]);
                        i += 1;
                    }
                    tokens.push(Token::FuncCall { name, line });
                } else {
                    text_buf.push('#');
                    i += 1;
                }
            }

            // ----------------------------------------------------------------
            // At-reference: @identifier
            // ----------------------------------------------------------------
            '@' => {
                if i + 1 < len && (chars[i + 1].is_alphanumeric() || chars[i + 1] == '_') {
                    flush_text!();
                    i += 1; // skip `@`
                    let mut name = String::new();
                    while i < len && is_ident_char(chars[i]) {
                        name.push(chars[i]);
                        i += 1;
                    }
                    tokens.push(Token::AtRef { name });
                } else {
                    text_buf.push('@');
                    i += 1;
                }
            }

            // ----------------------------------------------------------------
            // Angle-bracket label: <identifier>
            // ----------------------------------------------------------------
            '<' => {
                // Peek ahead to see if this is <identifier>.
                let mut j = i + 1;
                while j < len && is_ident_char(chars[j]) {
                    j += 1;
                }
                if j > i + 1 && j < len && chars[j] == '>' {
                    flush_text!();
                    let name: String = chars[i + 1..j].iter().collect();
                    i = j + 1; // skip closing `>`
                    tokens.push(Token::Label { name });
                } else {
                    text_buf.push('<');
                    i += 1;
                }
            }

            // ----------------------------------------------------------------
            // Forward slash: line comment `//`, block comment `/* */`,
            // or term marker `/ ` at line start
            // ----------------------------------------------------------------
            '/' => {
                if i + 1 < len && chars[i + 1] == '/' {
                    // Line comment
                    flush_text!();
                    i += 2; // skip `//`
                    let mut comment = String::new();
                    while i < len && chars[i] != '\n' {
                        comment.push(chars[i]);
                        i += 1;
                    }
                    tokens.push(Token::Comment { value: comment });
                } else if i + 1 < len && chars[i + 1] == '*' {
                    // Block comment
                    flush_text!();
                    i += 2; // skip `/*`
                    let mut comment = String::new();
                    while i < len {
                        if chars[i] == '*' && i + 1 < len && chars[i + 1] == '/' {
                            i += 2;
                            break;
                        }
                        if chars[i] == '\n' {
                            line += 1;
                        }
                        comment.push(chars[i]);
                        i += 1;
                    }
                    tokens.push(Token::BlockComment { value: comment });
                } else if i + 1 < len && chars[i + 1] == ' ' && at_line_start(&tokens, &text_buf) {
                    // Term marker at line start: `/ `
                    flush_text!();
                    i += 2; // skip `/ `
                    tokens.push(Token::TermMarker { line });
                } else {
                    text_buf.push('/');
                    i += 1;
                }
            }

            // ----------------------------------------------------------------
            // Backslash: escape or bare line-break
            // ----------------------------------------------------------------
            '\\' => {
                flush_text!();
                i += 1; // skip `\`
                if i < len && is_special_char(chars[i]) {
                    let ch = chars[i];
                    i += 1;
                    tokens.push(Token::Escape { ch });
                } else {
                    tokens.push(Token::Backslash);
                }
            }

            // ----------------------------------------------------------------
            // Dash(es)
            // ----------------------------------------------------------------
            '-' => {
                let mut count: u8 = 1;
                i += 1;
                while i < len && chars[i] == '-' {
                    count += 1;
                    i += 1;
                }
                if count == 1 && !at_line_start(&tokens, &text_buf) {
                    // Single dash not at line start → plain hyphen in text.
                    text_buf.push('-');
                } else {
                    flush_text!();
                    tokens.push(Token::Dash { count });
                }
            }

            // ----------------------------------------------------------------
            // Plus: ordered list item marker at line start
            // ----------------------------------------------------------------
            '+' if at_line_start(&tokens, &text_buf) => {
                flush_text!();
                i += 1;
                // Consume optional single space after `+`.
                if i < len && chars[i] == ' ' {
                    i += 1;
                }
                tokens.push(Token::Plus { line });
            }

            // ----------------------------------------------------------------
            // Brackets and parentheses
            // ----------------------------------------------------------------
            '(' => {
                flush_text!();
                i += 1;
                tokens.push(Token::ParenOpen);
            }
            ')' => {
                flush_text!();
                i += 1;
                tokens.push(Token::ParenClose);
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

            // ----------------------------------------------------------------
            // Colon and Comma
            // ----------------------------------------------------------------
            ':' => {
                flush_text!();
                i += 1;
                tokens.push(Token::Colon);
            }
            ',' => {
                flush_text!();
                i += 1;
                tokens.push(Token::Comma);
            }

            // ----------------------------------------------------------------
            // String literal: "..."
            // ----------------------------------------------------------------
            '"' => {
                flush_text!();
                i += 1; // skip opening `"`
                let mut value = String::new();
                while i < len && chars[i] != '"' {
                    if chars[i] == '\\' && i + 1 < len {
                        // Simple escape inside string literals.
                        i += 1;
                        value.push(chars[i]);
                    } else {
                        value.push(chars[i]);
                    }
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
                if i < len {
                    i += 1; // skip closing `"`
                }
                tokens.push(Token::StringLit { value });
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

    // ------------------------------------------------------------------
    // Basic text and whitespace
    // ------------------------------------------------------------------

    #[test]
    fn lex_plain_text() {
        let tokens = tokenize("Hello world");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Text { value } if value == "Hello world"));
    }

    #[test]
    fn lex_newline() {
        let tokens = tokenize("line one\nline two");
        assert!(tokens.iter().any(|t| matches!(t, Token::Newline)));
    }

    #[test]
    fn lex_blank_line() {
        let tokens = tokenize("para one\n\npara two");
        assert!(tokens.iter().any(|t| matches!(t, Token::BlankLine)));
    }

    // ------------------------------------------------------------------
    // Headings
    // ------------------------------------------------------------------

    #[test]
    fn lex_heading_level1() {
        let tokens = tokenize("= Introduction\n");
        assert!(matches!(&tokens[0], Token::Heading { level: 1, .. }));
        assert!(matches!(&tokens[1], Token::Text { value } if value == "Introduction"));
    }

    #[test]
    fn lex_heading_level3() {
        let tokens = tokenize("=== Deep section\n");
        assert!(matches!(&tokens[0], Token::Heading { level: 3, .. }));
    }

    #[test]
    fn lex_equals_not_heading() {
        // `=` not at line start should be plain text.
        let tokens = tokenize("a = b");
        assert!(tokens.iter().all(|t| !matches!(t, Token::Heading { .. })));
        let combined: String = tokens
            .iter()
            .filter_map(|t| {
                if let Token::Text { value } = t {
                    Some(value.as_str())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");
        assert!(combined.contains('='));
    }

    // ------------------------------------------------------------------
    // Inline emphasis
    // ------------------------------------------------------------------

    #[test]
    fn lex_star() {
        let tokens = tokenize("*bold*");
        assert!(tokens.iter().any(|t| matches!(t, Token::Star)));
    }

    #[test]
    fn lex_underscore() {
        let tokens = tokenize("_italic_");
        assert!(tokens.iter().any(|t| matches!(t, Token::Underscore)));
    }

    // ------------------------------------------------------------------
    // Backticks
    // ------------------------------------------------------------------

    #[test]
    fn lex_backtick_single() {
        let tokens = tokenize("`code`");
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::Backtick { count: 1 })));
    }

    #[test]
    fn lex_backtick_triple() {
        let tokens = tokenize("```\ncode block\n```");
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::Backtick { count: 3 })));
    }

    // ------------------------------------------------------------------
    // Dollar / math mode
    // ------------------------------------------------------------------

    #[test]
    fn lex_dollar() {
        let tokens = tokenize("$x^2$");
        assert_eq!(tokens[0], Token::Dollar);
        assert!(matches!(&tokens[1], Token::Text { value } if value == "x^2"));
        assert_eq!(tokens[2], Token::Dollar);
    }

    // ------------------------------------------------------------------
    // Brackets and parentheses
    // ------------------------------------------------------------------

    #[test]
    fn lex_brackets_and_parens() {
        let tokens = tokenize("([]){}");
        assert_eq!(
            tokens,
            vec![
                Token::ParenOpen,
                Token::BracketOpen,
                Token::BracketClose,
                Token::ParenClose,
                Token::BraceOpen,
                Token::BraceClose,
            ]
        );
    }

    // ------------------------------------------------------------------
    // Function calls and references
    // ------------------------------------------------------------------

    #[test]
    fn lex_func_call() {
        let tokens = tokenize("#pagebreak");
        assert!(matches!(&tokens[0], Token::FuncCall { name, .. } if name == "pagebreak"));
    }

    #[test]
    fn lex_func_call_with_args() {
        let tokens = tokenize("#figure(image(\"fig.png\"))");
        assert!(matches!(&tokens[0], Token::FuncCall { name, .. } if name == "figure"));
        assert!(tokens.iter().any(|t| matches!(t, Token::ParenOpen)));
    }

    #[test]
    fn lex_at_ref() {
        let tokens = tokenize("@fig-1");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::AtRef { name } if name == "fig-1"));
    }

    #[test]
    fn lex_label() {
        let tokens = tokenize("<my-label>");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Label { name } if name == "my-label"));
    }

    #[test]
    fn lex_not_label_in_text() {
        // `< ` with a space should not become a Label.
        let tokens = tokenize("a < b");
        assert!(tokens.iter().all(|t| !matches!(t, Token::Label { .. })));
    }

    // ------------------------------------------------------------------
    // Comments
    // ------------------------------------------------------------------

    #[test]
    fn lex_line_comment() {
        let tokens = tokenize("text // comment\nnext");
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::Comment { value } if value.trim() == "comment")));
    }

    #[test]
    fn lex_block_comment() {
        let tokens = tokenize("a /* block */ b");
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::BlockComment { value } if value.trim() == "block")));
    }

    // ------------------------------------------------------------------
    // Escape and backslash
    // ------------------------------------------------------------------

    #[test]
    fn lex_escape() {
        let tokens = tokenize(r"\*");
        assert!(matches!(&tokens[0], Token::Escape { ch: '*' }));
    }

    #[test]
    fn lex_backslash_line_break() {
        // `\` followed by a non-special char → Backslash token.
        let tokens = tokenize("a\\b");
        // `b` is not a special char, so we get Backslash then Text "b".
        assert!(tokens.iter().any(|t| matches!(t, Token::Backslash)));
    }

    // ------------------------------------------------------------------
    // Dashes
    // ------------------------------------------------------------------

    #[test]
    fn lex_dashes() {
        // `---` at line start → Dash { count: 3 }.
        let tokens = tokenize("---");
        assert!(matches!(&tokens[0], Token::Dash { count: 3 }));
    }

    #[test]
    fn lex_dash_in_word() {
        // Single `-` not at line start must be accumulated as plain text.
        let tokens = tokenize("self-referential");
        assert!(tokens.iter().all(|t| !matches!(t, Token::Dash { .. })));
        // Should be a single Text token containing the hyphen.
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::Text { value } if value == "self-referential"));
    }

    // ------------------------------------------------------------------
    // String literal
    // ------------------------------------------------------------------

    #[test]
    fn lex_string_literal() {
        let tokens = tokenize("\"hello world\"");
        assert_eq!(tokens.len(), 1);
        assert!(matches!(&tokens[0], Token::StringLit { value } if value == "hello world"));
    }

    // ------------------------------------------------------------------
    // Term marker and plus
    // ------------------------------------------------------------------

    #[test]
    fn lex_term_marker() {
        let tokens = tokenize("/ Term: definition");
        assert!(matches!(&tokens[0], Token::TermMarker { .. }));
    }

    #[test]
    fn lex_plus_ordered_list() {
        let tokens = tokenize("+ First item");
        assert!(matches!(&tokens[0], Token::Plus { .. }));
    }

    // ------------------------------------------------------------------
    // Frontmatter
    // ------------------------------------------------------------------

    #[test]
    fn lex_yaml_frontmatter() {
        let input = "---\ntitle: My Doc\nauthor: Alice\n---\n= Heading\n";
        let tokens = tokenize(input);
        assert!(matches!(
            &tokens[0],
            Token::RawFrontmatter { value } if value.contains("title: My Doc")
        ));
        assert!(tokens.iter().any(|t| matches!(t, Token::Heading { .. })));
    }

    // ------------------------------------------------------------------
    // Code / math mode suppression (critical fixes)
    // ------------------------------------------------------------------

    #[test]
    fn lex_star_in_math_not_special() {
        // `*` inside `$...$` must be plain text, not Token::Star.
        let tokens = tokenize("$a * b$");
        // Expect: Dollar, Text("a * b"), Dollar
        assert_eq!(tokens[0], Token::Dollar);
        assert!(matches!(&tokens[1], Token::Text { value } if value == "a * b"));
        assert_eq!(tokens[2], Token::Dollar);
        assert!(tokens.iter().all(|t| !matches!(t, Token::Star)));
    }

    #[test]
    fn lex_hash_in_code_not_func() {
        // `#` inside ``` ... ``` must be plain text, not FuncCall.
        let tokens = tokenize("```\n#pagebreak\n```");
        // Should contain no FuncCall token.
        assert!(tokens.iter().all(|t| !matches!(t, Token::FuncCall { .. })));
        // The text buffer inside the code block should include the hash.
        assert!(tokens
            .iter()
            .any(|t| matches!(t, Token::Text { value } if value.contains('#'))));
    }

    // ------------------------------------------------------------------
    // Colon, Comma
    // ------------------------------------------------------------------

    #[test]
    fn lex_colon_and_comma() {
        let tokens = tokenize("a: b, c");
        assert!(tokens.iter().any(|t| matches!(t, Token::Colon)));
        assert!(tokens.iter().any(|t| matches!(t, Token::Comma)));
    }
}
