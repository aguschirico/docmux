use docmux_ast::{
    Author, Block, Citation, CitationMode, CrossRef, Document, Inline, MetaValue, Metadata,
    ParseWarning, RefForm,
};

use crate::lexer::Token;
use crate::unescape::unescape_latex;

/// Commands that should be silently consumed (with their arguments) without
/// producing any output or warning.
const SILENTLY_IGNORED: &[&str] = &[
    "documentclass",
    "usepackage",
    "newcommand",
    "renewcommand",
    "maketitle",
    "tableofcontents",
    "bibliographystyle",
    "pagestyle",
    "thispagestyle",
    "setlength",
    "setcounter",
];

/// Core recursive-descent parser for LaTeX documents.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    warnings: Vec<ParseWarning>,
    footnote_defs: Vec<Block>,
    footnote_counter: usize,
}

impl Parser {
    /// Create a new parser from a token stream produced by the lexer.
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            warnings: Vec::new(),
            footnote_defs: Vec::new(),
            footnote_counter: 0,
        }
    }

    /// Entry point: detect preamble, parse body, and return a [`Document`].
    pub fn parse(mut self) -> Document {
        // Check if there is a \begin{document} — if so, split into preamble + body.
        let has_document_env = self
            .tokens
            .iter()
            .any(|t| matches!(t, Token::BeginEnv { name, .. } if name == "document"));

        let (metadata, _raw_preamble) = if has_document_env {
            self.parse_preamble()
        } else {
            (Metadata::default(), None)
        };

        let content = self.parse_body();

        // Append footnote definitions at the end of the content.
        let mut all_content = content;
        all_content.append(&mut self.footnote_defs);

        Document {
            metadata,
            content: all_content,
            bibliography: None,
            warnings: self.warnings,
        }
    }

    // ── Navigation helpers ──────────────────────────────────────────────────

    /// Peek at the current token without consuming it.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// Consume and return the current token, advancing the position.
    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    // ── Argument consumption ────────────────────────────────────────────────

    /// Consume a balanced brace argument `{...}` and return the inner tokens.
    ///
    /// Expects the next token to be `BraceOpen`. If not, returns an empty vec.
    fn parse_brace_argument(&mut self) -> Vec<Token> {
        // Skip optional whitespace/newlines before the brace.
        while matches!(self.peek(), Some(Token::Newline)) {
            self.advance();
        }

        if !matches!(self.peek(), Some(Token::BraceOpen)) {
            return Vec::new();
        }
        self.advance(); // consume BraceOpen

        let mut depth: u32 = 1;
        let mut inner = Vec::new();

        while let Some(tok) = self.advance() {
            match tok {
                Token::BraceOpen => {
                    depth += 1;
                    inner.push(tok);
                }
                Token::BraceClose => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    inner.push(tok);
                }
                _ => inner.push(tok),
            }
        }

        inner
    }

    /// Convenience: consume a brace argument and collect all its text content
    /// into a single `String`.
    fn parse_brace_text(&mut self) -> String {
        let tokens = self.parse_brace_argument();
        let mut s = String::new();
        for tok in tokens {
            match tok {
                Token::Text { value } => s.push_str(&value),
                Token::Newline => s.push(' '),
                Token::Tilde => s.push('\u{00A0}'),
                Token::Command { name, .. } => {
                    // Handle \and for author splitting — just add it literally
                    // so callers can split on it.
                    s.push('\\');
                    s.push_str(&name);
                }
                _ => {}
            }
        }
        s
    }

    /// If the next token is `BracketOpen`, consume `[...]` and return the
    /// contained text. Otherwise return `None`.
    fn parse_optional_argument(&mut self) -> Option<String> {
        if !matches!(self.peek(), Some(Token::BracketOpen)) {
            return None;
        }
        self.advance(); // consume BracketOpen

        let mut s = String::new();
        let mut depth: u32 = 1;

        while let Some(tok) = self.advance() {
            match tok {
                Token::BracketOpen => {
                    depth += 1;
                    s.push('[');
                }
                Token::BracketClose => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    s.push(']');
                }
                Token::Text { value } => s.push_str(&value),
                Token::Newline => s.push(' '),
                _ => {}
            }
        }

        Some(s)
    }

    // ── Preamble parsing ────────────────────────────────────────────────────

    /// Parse everything before `\begin{document}`, extracting metadata.
    ///
    /// Returns `(Metadata, Option<raw_preamble_text>)`.
    fn parse_preamble(&mut self) -> (Metadata, Option<String>) {
        let mut metadata = Metadata::default();
        let mut raw_parts: Vec<String> = Vec::new();

        while let Some(tok) = self.peek() {
            // Stop at \begin{document}.
            if matches!(tok, Token::BeginEnv { name, .. } if name == "document") {
                self.advance(); // consume BeginEnv{document}
                break;
            }

            let tok = self.advance().unwrap();

            match tok {
                Token::Command { ref name, .. } => {
                    let name_str = name.clone();
                    match name_str.as_str() {
                        "title" => {
                            let title = self.parse_brace_text();
                            raw_parts.push(format!("\\title{{{}}}", title));
                            metadata.title = Some(title);
                        }
                        "author" => {
                            let author_text = self.parse_brace_text();
                            raw_parts.push(format!("\\author{{{}}}", author_text));
                            // Split by \and
                            let authors: Vec<Author> = author_text
                                .split("\\and")
                                .map(|s| Author {
                                    name: s.trim().to_string(),
                                    affiliation: None,
                                    email: None,
                                    orcid: None,
                                })
                                .collect();
                            metadata.authors = authors;
                        }
                        "date" => {
                            let date = self.parse_brace_text();
                            raw_parts.push(format!("\\date{{{}}}", date));
                            metadata.date = Some(date);
                        }
                        cmd if SILENTLY_IGNORED.contains(&cmd) => {
                            // Consume optional argument and brace argument.
                            self.parse_optional_argument();
                            self.parse_brace_argument();
                            raw_parts.push(format!("\\{}", cmd));
                        }
                        _ => {
                            // Unknown preamble command — consume any
                            // arguments and store raw.
                            self.parse_optional_argument();
                            self.parse_brace_argument();
                            raw_parts.push(format!("\\{}", name_str));
                        }
                    }
                }
                Token::Text { ref value } => {
                    raw_parts.push(value.clone());
                }
                Token::Newline | Token::BlankLine => {
                    raw_parts.push("\n".to_string());
                }
                Token::Comment { ref value } => {
                    raw_parts.push(format!("%{}", value));
                }
                _ => {}
            }
        }

        let raw = raw_parts.join("");
        let raw_trimmed = raw.trim().to_string();
        if !raw_trimmed.is_empty() {
            metadata.custom.insert(
                "latex_preamble".to_string(),
                MetaValue::String(raw_trimmed.clone()),
            );
        }

        let raw_opt = if raw_trimmed.is_empty() {
            None
        } else {
            Some(raw_trimmed)
        };

        (metadata, raw_opt)
    }

    // ── Body parsing ────────────────────────────────────────────────────────

    /// Parse between `\begin{document}` and `\end{document}` (or all tokens
    /// in snippet mode).
    fn parse_body(&mut self) -> Vec<Block> {
        self.parse_blocks(Some("document"))
    }

    /// Collect blocks until `\end{stop_env}` or EOF.
    fn parse_blocks(&mut self, stop_env: Option<&str>) -> Vec<Block> {
        let mut blocks = Vec::new();

        while let Some(tok) = self.peek() {
            // Check for stop condition: \end{stop_env}.
            if let Some(env) = stop_env {
                if matches!(tok, Token::EndEnv { name, .. } if name == env) {
                    self.advance(); // consume EndEnv
                    break;
                }
            }

            // Skip blank lines between blocks (paragraph separators consumed
            // during inline collection will also end up here).
            if matches!(tok, Token::BlankLine) {
                self.advance();
                continue;
            }

            // Skip comments at block level.
            if matches!(tok, Token::Comment { .. }) {
                self.advance();
                continue;
            }

            // Skip bare newlines at block level.
            if matches!(tok, Token::Newline) {
                self.advance();
                continue;
            }

            // Handle block-level commands.
            if let Token::Command { ref name, .. } = tok {
                let name = name.clone();

                if Self::is_block_command(&name) {
                    match name.as_str() {
                        "section" | "section*" | "subsection" | "subsection*" | "subsubsection"
                        | "subsubsection*" | "paragraph" | "paragraph*" | "subparagraph"
                        | "subparagraph*" => {
                            let level = match name.trim_end_matches('*') {
                                "section" => 1,
                                "subsection" => 2,
                                "subsubsection" => 3,
                                "paragraph" => 4,
                                "subparagraph" => 5,
                                _ => 1,
                            };
                            self.advance(); // consume the command
                            let content = self.parse_inline_content();
                            blocks.push(Block::Heading {
                                level,
                                id: None,
                                content,
                            });
                            continue;
                        }
                        "hrule" => {
                            self.advance();
                            blocks.push(Block::ThematicBreak);
                            continue;
                        }
                        "begin" => {
                            // This is handled below as BeginEnv.
                            // The lexer emits BeginEnv directly, so this
                            // branch should not be reached in normal flow.
                            self.advance();
                            continue;
                        }
                        "noindent" => {
                            // Only block-level when followed by \rule.
                            // For now, consume and continue.
                            self.advance();
                            continue;
                        }
                        "footnotetext" => {
                            self.advance();
                            let content_inlines = self.parse_inline_content();
                            self.footnote_counter += 1;
                            let id = format!("fn{}", self.footnote_counter);
                            self.footnote_defs.push(Block::FootnoteDef {
                                id,
                                content: vec![Block::Paragraph {
                                    content: content_inlines,
                                }],
                            });
                            continue;
                        }
                        _ => {}
                    }
                }

                // Check silently ignored commands.
                if SILENTLY_IGNORED.contains(&name.as_str()) {
                    self.advance();
                    self.parse_optional_argument();
                    self.parse_brace_argument();
                    continue;
                }
            }

            // Handle BeginEnv at block level (environments).
            if let Token::BeginEnv { ref name, .. } = tok {
                let env_name = name.clone();
                self.advance(); // consume BeginEnv

                // For now, collect everything until matching EndEnv as a raw
                // block. Task 5 will add proper environment parsing.
                let mut raw_content = String::new();
                let mut depth: u32 = 1;

                while let Some(inner_tok) = self.advance() {
                    match inner_tok {
                        Token::BeginEnv { ref name, .. } => {
                            depth += 1;
                            raw_content.push_str(&format!("\\begin{{{}}}", name));
                        }
                        Token::EndEnv { ref name, .. } => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            raw_content.push_str(&format!("\\end{{{}}}", name));
                        }
                        Token::Text { ref value } => raw_content.push_str(value),
                        Token::Command { ref name, .. } => {
                            raw_content.push('\\');
                            raw_content.push_str(name);
                        }
                        Token::BraceOpen => raw_content.push('{'),
                        Token::BraceClose => raw_content.push('}'),
                        Token::BracketOpen => raw_content.push('['),
                        Token::BracketClose => raw_content.push(']'),
                        Token::Newline => raw_content.push('\n'),
                        Token::BlankLine => raw_content.push_str("\n\n"),
                        Token::Tilde => raw_content.push('~'),
                        Token::Ampersand => raw_content.push('&'),
                        Token::MathInline { ref value } => {
                            raw_content.push('$');
                            raw_content.push_str(value);
                            raw_content.push('$');
                        }
                        Token::MathDisplay { ref value } => {
                            raw_content.push_str("$$");
                            raw_content.push_str(value);
                            raw_content.push_str("$$");
                        }
                        Token::DoubleBackslash { .. } => {
                            raw_content.push_str("\\\\");
                        }
                        Token::Comment { ref value } => {
                            raw_content.push('%');
                            raw_content.push_str(value);
                        }
                    }
                }

                blocks.push(Block::RawBlock {
                    format: "latex".to_string(),
                    content: format!(
                        "\\begin{{{}}}{}\\end{{{}}}",
                        env_name, raw_content, env_name
                    ),
                });
                continue;
            }

            // Handle display math at block level.
            if matches!(tok, Token::MathDisplay { .. }) {
                if let Some(Token::MathDisplay { value }) = self.advance() {
                    blocks.push(Block::MathBlock {
                        content: value,
                        label: None,
                    });
                }
                continue;
            }

            // Otherwise, collect a paragraph of inlines.
            let inlines = self.collect_paragraph_inlines();
            if !inlines.is_empty() {
                blocks.push(Block::Paragraph { content: inlines });
            }
        }

        blocks
    }

    /// Gather inline tokens until a blank line or block-level command is
    /// encountered.
    ///
    /// If a `MathDisplay` token is found mid-paragraph, we stop the current
    /// paragraph before it (the caller will pick it up as a block).
    fn collect_paragraph_inlines(&mut self) -> Vec<Inline> {
        let mut inlines = Vec::new();

        while let Some(tok) = self.peek() {
            // Stop on blank line (paragraph separator).
            if matches!(tok, Token::BlankLine) {
                break;
            }

            // Stop on EndEnv (the caller's parse_blocks will handle it).
            if matches!(tok, Token::EndEnv { .. }) {
                break;
            }

            // Stop on BeginEnv (block-level environment).
            if matches!(tok, Token::BeginEnv { .. }) {
                break;
            }

            // Stop on display math — it becomes a block.
            if matches!(tok, Token::MathDisplay { .. }) {
                break;
            }

            // Stop on block-level commands.
            if let Token::Command { ref name, .. } = tok {
                let name = name.clone();
                if Self::is_block_command(&name) {
                    break;
                }
                // Check silently ignored commands at paragraph level —
                // they should stop the paragraph so parse_blocks handles them.
                if SILENTLY_IGNORED.contains(&name.as_str()) {
                    break;
                }
            }

            let tok = self.advance().unwrap();
            let inline = self.token_to_inline(tok);
            if let Some(i) = inline {
                inlines.push(i);
            }
        }

        inlines
    }

    /// Convert a single token to an inline element.
    ///
    /// Returns `None` for tokens that should be silently skipped (comments).
    fn token_to_inline(&mut self, token: Token) -> Option<Inline> {
        match token {
            Token::Text { value } => Some(Inline::Text {
                value: unescape_latex(&value),
            }),
            Token::Tilde => Some(Inline::Text {
                value: "\u{00A0}".to_string(),
            }),
            Token::MathInline { value } => Some(Inline::MathInline { value }),
            Token::DoubleBackslash { .. } => Some(Inline::HardBreak),
            Token::Newline => Some(Inline::SoftBreak),
            Token::Comment { .. } => None, // silently skip
            Token::Command { name, line } => self.dispatch_inline_command(&name, line),
            Token::BraceOpen => {
                // Bare group: parse contents as inlines until BraceClose.
                let mut group_inlines = Vec::new();
                while let Some(tok) = self.peek() {
                    if matches!(tok, Token::BraceClose) {
                        self.advance();
                        break;
                    }
                    let tok = self.advance().unwrap();
                    if let Some(i) = self.token_to_inline(tok) {
                        group_inlines.push(i);
                    }
                }
                // Flatten: if single inline, return it; otherwise wrap in a
                // Span or just return the first. For simplicity, return all
                // as a sequence — we'll add the first one and extend.
                if group_inlines.len() == 1 {
                    Some(group_inlines.into_iter().next().unwrap())
                } else if group_inlines.is_empty() {
                    None
                } else {
                    // Return first, push rest back… Actually just return
                    // them all. We need to return a single inline, so wrap
                    // in Emphasis-less Span (no attributes).
                    Some(Inline::Span {
                        content: group_inlines,
                        attrs: Default::default(),
                    })
                }
            }
            Token::BraceClose => None, // stray close brace, ignore
            Token::BracketOpen => {
                // Literal bracket text.
                Some(Inline::Text {
                    value: "[".to_string(),
                })
            }
            Token::BracketClose => Some(Inline::Text {
                value: "]".to_string(),
            }),
            Token::Ampersand => Some(Inline::Text {
                value: "&".to_string(),
            }),
            // MathDisplay and BlankLine should not reach here (handled by
            // collect_paragraph_inlines), but handle gracefully.
            Token::MathDisplay { value } => Some(Inline::MathInline { value }),
            Token::BlankLine => None,
            Token::BeginEnv { .. } | Token::EndEnv { .. } => None,
        }
    }

    /// Dispatch a `\command` to the appropriate inline constructor.
    fn dispatch_inline_command(&mut self, name: &str, line: usize) -> Option<Inline> {
        match name {
            "emph" | "textit" => {
                let content = self.parse_inline_content();
                Some(Inline::Emphasis { content })
            }
            "textbf" => {
                let content = self.parse_inline_content();
                Some(Inline::Strong { content })
            }
            "sout" => {
                let content = self.parse_inline_content();
                Some(Inline::Strikethrough { content })
            }
            "texttt" => {
                let value = self.parse_brace_text();
                Some(Inline::Code { value })
            }
            "textsc" => {
                let content = self.parse_inline_content();
                Some(Inline::SmallCaps { content })
            }
            "textsuperscript" => {
                let content = self.parse_inline_content();
                Some(Inline::Superscript { content })
            }
            "textsubscript" => {
                let content = self.parse_inline_content();
                Some(Inline::Subscript { content })
            }
            "href" => {
                let url = self.parse_brace_text();
                let content = self.parse_inline_content();
                Some(Inline::Link {
                    url,
                    title: None,
                    content,
                })
            }
            "url" => {
                let url = self.parse_brace_text();
                Some(Inline::Link {
                    url: url.clone(),
                    title: None,
                    content: vec![Inline::Text { value: url }],
                })
            }
            "cite" => {
                let keys_text = self.parse_brace_text();
                let keys: Vec<String> = keys_text
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                Some(Inline::Citation(Citation {
                    keys,
                    prefix: None,
                    suffix: None,
                    mode: CitationMode::Normal,
                }))
            }
            "citet" => {
                let keys_text = self.parse_brace_text();
                let keys: Vec<String> = keys_text
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                Some(Inline::Citation(Citation {
                    keys,
                    prefix: None,
                    suffix: None,
                    mode: CitationMode::AuthorOnly,
                }))
            }
            "citeyear" => {
                let keys_text = self.parse_brace_text();
                let keys: Vec<String> = keys_text
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                Some(Inline::Citation(Citation {
                    keys,
                    prefix: None,
                    suffix: None,
                    mode: CitationMode::SuppressAuthor,
                }))
            }
            "ref" => {
                let target = self.parse_brace_text();
                Some(Inline::CrossRef(CrossRef {
                    target,
                    form: RefForm::Number,
                }))
            }
            "autoref" => {
                let target = self.parse_brace_text();
                Some(Inline::CrossRef(CrossRef {
                    target,
                    form: RefForm::NumberWithType,
                }))
            }
            "pageref" => {
                let target = self.parse_brace_text();
                Some(Inline::CrossRef(CrossRef {
                    target,
                    form: RefForm::Page,
                }))
            }
            "label" => {
                // Consume the argument but skip — consumed by parent block
                // handler in Task 5.
                self.parse_brace_argument();
                None
            }
            "footnote" => {
                self.footnote_counter += 1;
                let id = format!("fn{}", self.footnote_counter);
                let content_inlines = self.parse_inline_content();
                self.footnote_defs.push(Block::FootnoteDef {
                    id: id.clone(),
                    content: vec![Block::Paragraph {
                        content: content_inlines,
                    }],
                });
                Some(Inline::FootnoteRef { id })
            }
            _ => {
                // Unknown command — emit RawInline and warning.
                // Try to consume a brace argument if present.
                let arg_text = if matches!(self.peek(), Some(Token::BraceOpen)) {
                    let t = self.parse_brace_text();
                    format!("\\{}{{{}}}", name, t)
                } else {
                    format!("\\{}", name)
                };

                self.warn(line, format!("Unknown command: \\{}", name));

                Some(Inline::RawInline {
                    format: "latex".to_string(),
                    content: arg_text,
                })
            }
        }
    }

    /// Parse a brace argument as a sequence of inlines.
    fn parse_inline_content(&mut self) -> Vec<Inline> {
        let tokens = self.parse_brace_argument();
        // Create a sub-parser to process these tokens as inlines.
        let mut sub = Parser {
            tokens,
            pos: 0,
            warnings: Vec::new(),
            footnote_defs: Vec::new(),
            footnote_counter: self.footnote_counter,
        };

        let mut inlines = Vec::new();
        while sub.peek().is_some() {
            let tok = sub.advance().unwrap();
            if let Some(i) = sub.token_to_inline(tok) {
                inlines.push(i);
            }
        }

        // Merge any warnings and footnotes back.
        self.warnings.append(&mut sub.warnings);
        self.footnote_defs.append(&mut sub.footnote_defs);
        self.footnote_counter = sub.footnote_counter;

        inlines
    }

    /// Returns `true` if the command name is block-level.
    fn is_block_command(name: &str) -> bool {
        matches!(
            name,
            "section"
                | "section*"
                | "subsection"
                | "subsection*"
                | "subsubsection"
                | "subsubsection*"
                | "paragraph"
                | "paragraph*"
                | "subparagraph"
                | "subparagraph*"
                | "hrule"
                | "begin"
                | "noindent"
                | "footnotetext"
        )
    }

    /// Record a parse warning.
    fn warn(&mut self, line: usize, message: impl Into<String>) {
        self.warnings.push(ParseWarning {
            line,
            message: message.into(),
        });
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    #[test]
    fn parse_simple_paragraph() {
        let tokens = tokenize("Hello world.");
        let doc = Parser::new(tokens).parse();
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::Paragraph { .. }));
    }

    #[test]
    fn parse_two_paragraphs() {
        let tokens = tokenize("First paragraph.\n\nSecond paragraph.");
        let doc = Parser::new(tokens).parse();
        assert_eq!(doc.content.len(), 2);
    }

    #[test]
    fn parse_preamble_metadata() {
        let input = r"\documentclass{article}
\title{My Paper}
\author{Jane Doe \and John Smith}
\date{2026-03-23}
\begin{document}
Hello.
\end{document}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        assert_eq!(doc.metadata.title.as_deref(), Some("My Paper"));
        assert_eq!(doc.metadata.authors.len(), 2);
        assert_eq!(doc.metadata.authors[0].name, "Jane Doe");
        assert_eq!(doc.metadata.authors[1].name, "John Smith");
        assert_eq!(doc.metadata.date.as_deref(), Some("2026-03-23"));
    }

    #[test]
    fn parse_preamble_preserved_raw() {
        let input = r"\documentclass{article}
\usepackage{amsmath}
\title{Test}
\begin{document}
Body.
\end{document}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        assert!(doc.metadata.custom.contains_key("latex_preamble"));
    }

    #[test]
    fn parse_snippet_mode_no_document_env() {
        let tokens = tokenize("Just a snippet.");
        let doc = Parser::new(tokens).parse();
        assert_eq!(doc.content.len(), 1);
        assert!(doc.warnings.is_empty());
    }

    #[test]
    fn parse_inline_emphasis() {
        let tokens = tokenize(r"\emph{hello}");
        let doc = Parser::new(tokens).parse();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[0], Inline::Emphasis { .. }));
        } else {
            panic!("Expected paragraph");
        }
    }

    #[test]
    fn parse_inline_strong() {
        let tokens = tokenize(r"\textbf{bold}");
        let doc = Parser::new(tokens).parse();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[0], Inline::Strong { .. }));
        } else {
            panic!("Expected paragraph");
        }
    }

    #[test]
    fn parse_inline_math() {
        let tokens = tokenize(r"The formula $x^2$ is nice.");
        let doc = Parser::new(tokens).parse();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(content
                .iter()
                .any(|i| matches!(i, Inline::MathInline { .. })));
        } else {
            panic!("Expected paragraph");
        }
    }

    #[test]
    fn parse_tilde_as_nbsp() {
        let tokens = tokenize(r"Dr.~Smith");
        let doc = Parser::new(tokens).parse();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(content
                .iter()
                .any(|i| matches!(i, Inline::Text { value } if value == "\u{00A0}")));
        } else {
            panic!("Expected paragraph");
        }
    }

    #[test]
    fn parse_unknown_command_produces_raw_inline_and_warning() {
        let tokens = tokenize(r"\weirdcommand{arg}");
        let doc = Parser::new(tokens).parse();
        assert!(!doc.warnings.is_empty());
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(content
                .iter()
                .any(|i| matches!(i, Inline::RawInline { format, .. } if format == "latex")));
        }
    }

    #[test]
    fn parse_silently_ignored_commands_no_warning() {
        let tokens = tokenize(
            r"\maketitle
\tableofcontents
\bibliographystyle{plain}

Some text.",
        );
        let doc = Parser::new(tokens).parse();
        assert!(
            doc.warnings.is_empty(),
            "Expected no warnings for ignored commands, got: {:?}",
            doc.warnings
        );
    }
}
