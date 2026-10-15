use docmux_ast::{
    Alignment, Attributes, Author, Block, Citation, CitationMode, CiteItem, ColumnSpec, CrossRef,
    DefinitionItem, Document, Image, Inline, ListItem, MetaValue, Metadata, ParseWarning, RefForm,
    Table, TableCell,
};
use std::collections::HashMap;

use crate::commands::{self, GroupFormat};
use crate::lexer::Token;
use crate::unescape::{typographic, unescape_latex};

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

/// A `\\bibitem[label]` label as plain text: `ADG$^{+}$16` → `ADG+16`,
/// `Akbik et~al.(2018)Akbik, …` keeps its natbib shape.
fn clean_bib_label(label: &str) -> String {
    let text: String = label
        .chars()
        .filter(|c| !matches!(c, '{' | '}' | '$' | '^'))
        .map(|c| if c == '~' || c == '\u{00A0}' { ' ' } else { c })
        .collect();
    unescape_latex(&text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Core recursive-descent parser for LaTeX documents.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    warnings: Vec<ParseWarning>,
    footnote_defs: Vec<Block>,
    footnote_counter: usize,
    abstract_text: Option<Vec<Block>>,
    /// Environments being parsed, outermost first, so a mismatched `\end`
    /// can tell an enclosing environment from a stray one.
    open_envs: Vec<String>,
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
            abstract_text: None,
            open_envs: Vec::new(),
        }
    }

    /// Entry point: detect preamble, parse body, and return a [`Document`].
    pub fn parse(mut self) -> Document {
        // Check if there is a \begin{document} — if so, split into preamble + body.
        let has_document_env = self
            .tokens
            .iter()
            .any(|t| matches!(t, Token::BeginEnv { name, .. } if name == "document"));

        let (mut metadata, _raw_preamble) = if has_document_env {
            self.parse_preamble()
        } else {
            (Metadata::default(), None)
        };

        let content = self.parse_body();

        // Merge abstract text into metadata if found.
        if let Some(abs) = self.abstract_text.take() {
            metadata.abstract_text = Some(abs);
        }

        // Append footnote definitions at the end of the content.
        let mut all_content = content;
        all_content.append(&mut self.footnote_defs);

        Document {
            metadata,
            content: all_content,
            bibliography: None,
            warnings: self.warnings,
            resources: HashMap::new(),
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
        // Skip whitespace before the brace: `\\cmd {arg}` is `\\cmd{arg}`.
        while matches!(self.peek(), Some(Token::Newline))
            || matches!(self.peek(), Some(Token::Text { value }) if value.trim().is_empty())
        {
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

    /// Commands without a dedicated handler: keep their text, drop their
    /// layout. Truly unknown commands keep their brace argument's content.
    fn fallback_inline_command(&mut self, name: &str, line: usize) -> Option<Inline> {
        if let Some(text) = commands::abbreviation(name) {
            return Some(Inline::text(text));
        }
        if commands::is_switch(name) {
            return None;
        }
        if let Some((optional, braces)) = commands::dropped(name) {
            self.skip_arguments(optional, braces);
            return None;
        }
        if let Some((optional, braces)) = commands::wrapper(name) {
            self.skip_arguments(optional, braces);
            return Self::span_of(self.parse_inline_content());
        }
        self.warn(line, format!("Unknown command: \\{}", name));
        if matches!(self.peek(), Some(Token::BraceOpen)) {
            let content = self.parse_inline_content();
            return Self::span_of(content);
        }
        None
    }

    fn skip_arguments(&mut self, optional: usize, braces: usize) {
        for _ in 0..optional {
            self.parse_optional_argument();
        }
        for _ in 0..braces {
            self.parse_brace_argument();
        }
    }

    fn span_of(content: Vec<Inline>) -> Option<Inline> {
        match content.len() {
            0 => None,
            1 => content.into_iter().next(),
            _ => Some(Inline::Span {
                content,
                attrs: Default::default(),
            }),
        }
    }

    /// `\\citep[prefix][suffix]{key1,key2}` and the other natbib/biblatex
    /// citation commands. With a single optional argument it is the suffix.
    fn parse_citation(&mut self, command: &str) -> Inline {
        let first = self.parse_optional_text();
        let second = self.parse_optional_text();
        let (prefix, suffix) = match (first, second) {
            (Some(pre), Some(post)) => (Some(pre), Some(post)),
            (Some(post), None) => (None, Some(post)),
            _ => (None, None),
        };
        let non_empty =
            |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
        let (prefix, suffix) = (non_empty(prefix), non_empty(suffix));
        let keys: Vec<String> = self
            .parse_brace_text()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let last = keys.len().saturating_sub(1);
        let items = keys
            .into_iter()
            .enumerate()
            .map(|(k, key)| CiteItem {
                key,
                prefix: if k == 0 { prefix.clone() } else { None },
                suffix: if k == last { suffix.clone() } else { None },
            })
            .collect();
        let mode = match command {
            "citet" | "citealt" | "textcite" | "Citet" | "Citealt" | "citeauthor"
            | "Citeauthor" | "newcite" => CitationMode::AuthorOnly,
            "citeyear" | "citeyearpar" => CitationMode::SuppressAuthor,
            _ => CitationMode::Normal,
        };
        Inline::Citation(Citation { items, mode })
    }

    /// If the next token is `BracketOpen`, consume `[...]` and return the
    /// tokens inside it. Brackets nest, and a `]` inside braces does not close.
    fn parse_optional_tokens(&mut self) -> Option<Vec<Token>> {
        if !matches!(self.peek(), Some(Token::BracketOpen)) {
            return None;
        }
        self.advance(); // consume BracketOpen

        let mut tokens = Vec::new();
        let (mut brackets, mut braces) = (1u32, 0u32);
        while let Some(tok) = self.advance() {
            match tok {
                Token::BracketOpen if braces == 0 => brackets += 1,
                Token::BracketClose if braces == 0 => {
                    brackets -= 1;
                    if brackets == 0 {
                        break;
                    }
                }
                Token::BraceOpen => braces += 1,
                Token::BraceClose => braces = braces.saturating_sub(1),
                _ => {}
            }
            tokens.push(tok);
        }
        Some(tokens)
    }

    /// The `[...]` argument read as text, with its commands resolved
    /// (`[\textit{inter alia}]`, `[Kone{\v{c}}n{\'y}]`) and its formatting
    /// dropped.
    fn parse_optional_text(&mut self) -> Option<String> {
        let tokens = self.parse_optional_tokens()?;
        Some(docmux_ast::inlines_to_plain_text(
            &self.inlines_from_tokens(tokens),
        ))
    }

    /// If the next token is `BracketOpen`, consume `[...]` and return the
    /// contained text. Otherwise return `None`.
    fn parse_optional_argument(&mut self) -> Option<String> {
        let tokens = self.parse_optional_tokens()?;
        let mut s = String::new();
        for tok in tokens {
            match tok {
                Token::BracketOpen => s.push('['),
                Token::BracketClose => s.push(']'),
                Token::Text { value } => s.push_str(&value),
                Token::Newline => s.push(' '),
                Token::MathInline { value } => {
                    s.push('$');
                    s.push_str(&value);
                    s.push('$');
                }
                Token::Tilde => s.push('~'),
                Token::Command { name, .. } => {
                    s.push('\\');
                    s.push_str(&name);
                }
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
        self.open_envs.push("document".into());
        let blocks = self.parse_blocks(Some("document"));
        self.open_envs.pop();
        blocks
    }

    /// For a token that ends the current environment but is not its own
    /// `\end`: true when it closes an enclosing environment, which then ends
    /// this one too, as LaTeX's error recovery does. A stray `\end` that closes
    /// nothing is consumed with a warning and gives false.
    fn ends_enclosing_env(&mut self) -> bool {
        let Some(Token::EndEnv { name, line }) = self.peek() else {
            return false;
        };
        if self.open_envs.iter().any(|open| open == name) {
            return true;
        }
        let (name, line) = (name.clone(), *line);
        self.advance();
        self.warn(line, format!("\\end{{{name}}} without a matching \\begin"));
        false
    }

    /// Skips a token that no rule consumed, so a parsing loop always moves on.
    fn skip_unparsed_token(&mut self) {
        if let Some(tok) = self.advance() {
            let line = match tok {
                Token::Command { line, .. }
                | Token::BeginEnv { line, .. }
                | Token::EndEnv { line, .. } => line,
                _ => 0,
            };
            self.warn(line, format!("Skipped unexpected token: {tok:?}"));
        }
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
            if matches!(tok, Token::EndEnv { .. }) {
                if self.ends_enclosing_env() {
                    break;
                }
                continue;
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
                            self.parse_optional_argument(); // short title for the ToC
                            let content = self.parse_inline_content();

                            // Peek for \label{...} after the heading.
                            let id = self.peek_label();

                            // Like LaTeX's article class: starred sections and
                            // \paragraph levels carry no number.
                            let unnumbered = name.ends_with('*') || level >= 4;
                            let attrs = unnumbered.then(|| Attributes {
                                id: None,
                                classes: vec!["unnumbered".into()],
                                key_values: HashMap::new(),
                            });
                            blocks.push(Block::Heading {
                                level,
                                id,
                                content,
                                attrs,
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
                        "item" => {
                            // An \item outside itemize/enumerate (in a custom
                            // list environment): its text becomes a paragraph.
                            self.advance();
                            self.parse_optional_argument();
                            continue;
                        }
                        "footnotetext" => {
                            self.advance();
                            let opt_n = self.parse_optional_argument();
                            let content_inlines = self.parse_inline_content();
                            let id = if let Some(n) = opt_n {
                                n
                            } else {
                                self.footnote_counter += 1;
                                format!("fn{}", self.footnote_counter)
                            };
                            blocks.push(Block::FootnoteDef {
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
            if let Token::BeginEnv { ref name, line, .. } = tok {
                let env_name = name.clone();
                let env_line = *line;
                self.advance(); // consume BeginEnv

                if let Some(block) = self.parse_environment(&env_name, env_line) {
                    blocks.push(block);
                }
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
            let start = self.pos;
            let inlines = self.collect_paragraph_inlines();
            if !inlines.is_empty() {
                blocks.push(Block::Paragraph { content: inlines });
            }
            if self.pos == start {
                self.skip_unparsed_token();
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
                value: typographic(&unescape_latex(&value)),
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
                // A leading font switch (`{\\bf …}`, `{\\em …}`) formats the group.
                let format = match self.peek() {
                    Some(Token::Command { name, .. }) => commands::group_format(name),
                    _ => None,
                };
                if format.is_some() {
                    self.advance();
                }
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
                if let Some(format) = format.filter(|f| *f != GroupFormat::Plain) {
                    return match format {
                        GroupFormat::Emphasis => Some(Inline::Emphasis {
                            content: group_inlines,
                        }),
                        GroupFormat::Strong => Some(Inline::Strong {
                            content: group_inlines,
                        }),
                        _ => Some(Inline::SmallCaps {
                            content: group_inlines,
                        }),
                    };
                }
                if group_inlines.len() == 1 {
                    group_inlines.into_iter().next()
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
                // \texttt is typeset text in a monospace font, not verbatim:
                // its escapes and commands (\{, \textbackslash, \textbf) are
                // read like any other text, then flattened, as pandoc does.
                let content = self.parse_inline_content();
                let value = docmux_ast::inlines_to_plain_text(&content);
                Some(Inline::Code { value, attrs: None })
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
                    attrs: None,
                })
            }
            "url" => {
                let url = self.parse_brace_text();
                Some(Inline::Link {
                    url: url.clone(),
                    title: None,
                    content: vec![Inline::Text { value: url }],
                    attrs: None,
                })
            }
            "cite" | "citep" | "citealp" | "parencite" | "autocite" | "Citep" | "Citealp"
            | "citet" | "citealt" | "textcite" | "Citet" | "Citealt" | "citeauthor" | "newcite"
            | "Citeauthor" | "citeyear" | "citeyearpar" => Some(self.parse_citation(name)),
            "nocite" => {
                self.parse_brace_text();
                None
            }
            "eqref" => {
                // \eqref prints the number in parentheses.
                let target = self.parse_brace_text();
                Some(Inline::Span {
                    content: vec![
                        Inline::text("("),
                        Inline::CrossRef(CrossRef {
                            target,
                            form: RefForm::Number,
                        }),
                        Inline::text(")"),
                    ],
                    attrs: Default::default(),
                })
            }
            "ref" => {
                let target = self.parse_brace_text();
                Some(Inline::CrossRef(CrossRef {
                    target,
                    form: RefForm::Number,
                }))
            }
            "autoref" | "cref" | "Cref" => {
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
                // handler.
                self.parse_brace_argument();
                None
            }
            "footnotemark" => {
                let opt_n = self.parse_optional_argument();
                let id = opt_n.unwrap_or_else(|| {
                    self.footnote_counter += 1;
                    format!("fn{}", self.footnote_counter)
                });
                Some(Inline::FootnoteRef { id })
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
            "underline" | "uline" => Some(Inline::Underline {
                content: self.parse_inline_content(),
            }),
            "textsl" => Some(Inline::Emphasis {
                content: self.parse_inline_content(),
            }),
            "labelcref" => {
                let target = self.parse_brace_text();
                Some(Inline::CrossRef(CrossRef {
                    target,
                    form: RefForm::Number,
                }))
            }
            "verb" | "verb*" => {
                // The lexer turns `\verb|x|` into `\verb{x}`.
                let value = self.parse_brace_text();
                Some(Inline::Code { value, attrs: None })
            }
            "hskip" | "vskip" | "kern" => {
                if let Some(Token::Text { value }) = self.peek() {
                    let rest = commands::strip_dimension(value).to_string();
                    self.tokens[self.pos] = Token::Text { value: rest };
                }
                None
            }
            _ => self.fallback_inline_command(name, line),
        }
    }

    /// Parse a brace argument as a sequence of inlines.
    fn parse_inline_content(&mut self) -> Vec<Inline> {
        let tokens = self.parse_brace_argument();
        self.inlines_from_tokens(tokens)
    }

    /// Parse a token list as a sequence of inlines with a sub-parser.
    fn inlines_from_tokens(&mut self, tokens: Vec<Token>) -> Vec<Inline> {
        // Create a sub-parser to process these tokens as inlines.
        let mut sub = Parser {
            tokens,
            pos: 0,
            warnings: Vec::new(),
            footnote_defs: Vec::new(),
            footnote_counter: self.footnote_counter,
            abstract_text: None,
            open_envs: Vec::new(),
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

    // ── Environment parsing ────────────────────────────────────────────────

    /// Peek ahead for a `\label{...}` command, skipping whitespace/newlines.
    /// If found, consume it and return the label text. Otherwise return None.
    fn peek_label(&mut self) -> Option<String> {
        // Skip whitespace tokens while looking for \label.
        let mut lookahead = self.pos;
        while lookahead < self.tokens.len() {
            match &self.tokens[lookahead] {
                Token::Newline | Token::Comment { .. } => {
                    lookahead += 1;
                }
                Token::Text { value } if value.trim().is_empty() => {
                    lookahead += 1;
                }
                Token::Command { name, .. } if name == "label" => {
                    // Found it — advance past skipped tokens and the command.
                    self.pos = lookahead + 1;
                    let label = self.parse_brace_text();
                    return Some(label);
                }
                _ => break,
            }
        }
        None
    }

    /// `thebibliography` (inline, or from a `.bbl` file): a references
    /// section whose entries carry `ref-<key>` ids and their citation label.
    fn parse_bibliography(&mut self) -> Block {
        self.parse_brace_argument(); // widest label
        let mut entries: Vec<(Option<String>, String, Vec<Token>)> = Vec::new();
        while let Some(tok) = self.advance() {
            match tok {
                Token::EndEnv { ref name, .. } if name == "thebibliography" => break,
                Token::Command { ref name, .. } if name == "bibitem" => {
                    let label = self.parse_optional_text();
                    let key = self.parse_brace_text().trim().to_string();
                    entries.push((label, key, Vec::new()));
                }
                Token::Command { ref name, .. } if name == "newblock" => {
                    if let Some(entry) = entries.last_mut() {
                        entry.2.push(Token::Text { value: " ".into() });
                    }
                }
                // Anything before the first \bibitem is setup code.
                other => {
                    if let Some(entry) = entries.last_mut() {
                        entry.2.push(other);
                    }
                }
            }
        }
        let mut content = vec![Block::Heading {
            level: 1,
            id: Some("references".into()),
            content: vec![Inline::text("References")],
            attrs: Some(Attributes {
                id: None,
                classes: vec!["unnumbered".into()],
                key_values: HashMap::new(),
            }),
        }];
        for (label, key, tokens) in entries {
            content.push(self.bibliography_entry(label, key, tokens));
        }
        Block::Div {
            attrs: Attributes {
                id: Some("refs".into()),
                classes: vec!["references".into()],
                key_values: HashMap::new(),
            },
            content,
        }
    }

    fn bibliography_entry(
        &mut self,
        label: Option<String>,
        key: String,
        tokens: Vec<Token>,
    ) -> Block {
        let mut inlines = self.inlines_from_tokens(tokens);
        let is_space = |i: &Inline| match i {
            Inline::SoftBreak => true,
            Inline::Text { value } => value.trim().is_empty(),
            _ => false,
        };
        while inlines.first().is_some_and(is_space) {
            inlines.remove(0);
        }
        while inlines.last().is_some_and(is_space) {
            inlines.pop();
        }
        let mut key_values = HashMap::new();
        if let Some(label) = label.map(|l| clean_bib_label(&l)).filter(|l| !l.is_empty()) {
            key_values.insert("label".to_string(), label);
        }
        Block::Div {
            attrs: Attributes {
                id: Some(format!("ref-{key}")),
                classes: vec!["csl-entry".into()],
                key_values,
            },
            content: vec![Block::Paragraph { content: inlines }],
        }
    }

    /// Environments without dedicated handling keep their content: layout
    /// containers (`center`, `minipage`) are transparent, theorem-like ones get
    /// their name in bold, and anything else is kept as a classed Div.
    fn parse_container_env(&mut self, env_name: &str, line: usize) -> Block {
        let base = env_name.trim_end_matches('*');
        let title = Self::theorem_title(base);
        if title.is_none() && !Self::is_layout_env(base) {
            self.warn(line, format!("Unknown environment: {env_name}"));
        }
        // minipage{width}, and the [title] of a theorem, which can hold
        // citations and references.
        let note = self.parse_optional_tokens();
        if matches!(base, "minipage" | "adjustbox" | "multicols") {
            self.parse_brace_argument();
        }
        let note = note.map(|tokens| self.inlines_from_tokens(tokens));
        let mut content = self.parse_blocks(Some(env_name));
        if let Some(title) = title {
            let mut lead = vec![Inline::text(title)];
            match note {
                Some(note) if !docmux_ast::inlines_to_plain_text(&note).trim().is_empty() => {
                    lead.push(Inline::text(" ("));
                    lead.extend(note);
                    lead.push(Inline::text(")."));
                }
                _ => lead.push(Inline::text(".")),
            }
            lead.push(Inline::text(" "));
            let label = if base == "proof" {
                Inline::Emphasis { content: lead }
            } else {
                Inline::Strong { content: lead }
            };
            match content.first_mut() {
                Some(Block::Paragraph { content: first }) => first.insert(0, label),
                _ => content.insert(
                    0,
                    Block::Paragraph {
                        content: vec![label],
                    },
                ),
            }
        }
        Block::Div {
            attrs: Attributes {
                id: None,
                // Environment names are arbitrary source text.
                classes: vec![base
                    .chars()
                    .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
                    .collect()],
                key_values: HashMap::new(),
            },
            content,
        }
    }

    fn is_layout_env(name: &str) -> bool {
        matches!(
            name,
            "center"
                | "flushleft"
                | "flushright"
                | "minipage"
                | "small"
                | "footnotesize"
                | "scriptsize"
                | "tiny"
                | "large"
                | "adjustbox"
                | "multicols"
                | "spacing"
                | "singlespace"
                | "onehalfspace"
                | "doublespace"
                | "landscape"
                | "sloppypar"
                | "appendices"
                | "subappendices"
                | "ack"
                | "acks"
                | "acknowledgments"
                | "acknowledgements"
        )
    }

    fn theorem_title(name: &str) -> Option<&'static str> {
        Some(match name {
            "theorem" | "thm" => "Theorem",
            "lemma" | "lem" => "Lemma",
            "proposition" | "prop" => "Proposition",
            "corollary" | "cor" => "Corollary",
            "definition" | "defn" | "dfn" => "Definition",
            "remark" | "rem" => "Remark",
            "example" | "ex" => "Example",
            "claim" => "Claim",
            "assumption" => "Assumption",
            "conjecture" => "Conjecture",
            "observation" => "Observation",
            "note" => "Note",
            "proof" => "Proof",
            "ack" | "acks" | "acknowledgments" | "acknowledgements" => "Acknowledgments",
            _ => return None,
        })
    }

    /// Dispatch an environment by name to the appropriate handler.
    fn parse_environment(&mut self, env_name: &str, line: usize) -> Option<Block> {
        self.open_envs.push(env_name.to_string());
        let block = self.parse_environment_body(env_name, line);
        self.open_envs.pop();
        block
    }

    fn parse_environment_body(&mut self, env_name: &str, line: usize) -> Option<Block> {
        match env_name {
            "itemize" => Some(self.parse_list(false, "itemize")),
            "enumerate" => Some(self.parse_list(true, "enumerate")),
            "quote" | "quotation" => Some(self.parse_blockquote(env_name)),
            "verbatim" => Some(self.parse_verbatim_env("verbatim")),
            "lstlisting" => Some(self.parse_verbatim_env("lstlisting")),
            "equation" | "equation*" | "align" | "align*" | "gather" | "gather*" | "multline"
            | "multline*" => Some(self.parse_math_env(env_name)),
            "eqnarray" | "eqnarray*" | "flalign" | "flalign*" | "alignat" | "alignat*" => {
                Some(self.parse_math_env(env_name))
            }
            "figure" | "figure*" | "wrapfigure" | "subfigure" | "algorithm" | "algorithm*" => {
                Some(self.parse_figure(env_name))
            }
            "table" | "table*" | "wraptable" => Some(self.parse_table_env(env_name)),
            _ if Self::is_tabular_env(env_name) => {
                let table = self.parse_tabular(env_name);
                Some(Block::Table(table))
            }
            "abstract" => {
                let blocks = self.parse_blocks(Some("abstract"));
                self.abstract_text = Some(blocks);
                None
            }
            "thebibliography" => Some(self.parse_bibliography()),
            "comment" | "filecontents" | "filecontents*" => {
                self.collect_raw_until_end(env_name);
                None
            }
            // Drawings only make sense to a TeX engine: keep them raw for
            // LaTeX output, other writers skip raw LaTeX.
            "tikzpicture" | "pgfpicture" | "picture" => {
                self.warn(line, format!("Drawing kept as raw LaTeX: {env_name}"));
                let raw_content = self.collect_raw_until_end(env_name);
                Some(Block::RawBlock {
                    format: "latex".to_string(),
                    content: format!("\\begin{{{env_name}}}{raw_content}\\end{{{env_name}}}"),
                })
            }
            "description" => Some(self.parse_description()),
            "document" => {
                // Should not normally reach here — handled in parse_body.
                None
            }
            _ => Some(self.parse_container_env(env_name, line)),
        }
    }

    /// Collect raw text from tokens until `\end{env_name}`, respecting nesting.
    fn collect_raw_until_end(&mut self, _env_name: &str) -> String {
        let mut raw = String::new();
        let mut depth: u32 = 1;

        while let Some(tok) = self.advance() {
            match tok {
                Token::BeginEnv { ref name, .. } => {
                    depth += 1;
                    raw.push_str(&format!("\\begin{{{}}}", name));
                }
                Token::EndEnv { ref name, .. } => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    raw.push_str(&format!("\\end{{{}}}", name));
                }
                Token::Text { ref value } => raw.push_str(value),
                Token::Command { ref name, .. } => {
                    raw.push('\\');
                    raw.push_str(name);
                }
                Token::BraceOpen => raw.push('{'),
                Token::BraceClose => raw.push('}'),
                Token::BracketOpen => raw.push('['),
                Token::BracketClose => raw.push(']'),
                Token::Newline => raw.push('\n'),
                Token::BlankLine => raw.push_str("\n\n"),
                Token::Tilde => raw.push('~'),
                Token::Ampersand => raw.push('&'),
                Token::MathInline { ref value } => {
                    raw.push('$');
                    raw.push_str(value);
                    raw.push('$');
                }
                Token::MathDisplay { ref value } => {
                    raw.push_str("$$");
                    raw.push_str(value);
                    raw.push_str("$$");
                }
                Token::DoubleBackslash { .. } => {
                    raw.push_str("\\\\");
                }
                Token::Comment { ref value } => {
                    raw.push('%');
                    raw.push_str(value);
                }
            }
        }

        raw
    }

    /// Parse `\begin{itemize}...\end{itemize}` or `\begin{enumerate}...\end{enumerate}`.
    fn parse_list(&mut self, ordered: bool, env_name: &str) -> Block {
        let mut items: Vec<ListItem> = Vec::new();

        // Skip whitespace/newlines before first \item.
        self.skip_whitespace();

        while let Some(tok) = self.peek() {
            if matches!(tok, Token::EndEnv { name, .. } if name == env_name) {
                self.advance();
                break;
            }
            if matches!(tok, Token::EndEnv { .. }) {
                if self.ends_enclosing_env() {
                    break;
                }
                continue;
            }

            // Expect \item command.
            if matches!(tok, Token::Command { ref name, .. } if name == "item") {
                self.advance(); // consume \item
                                // Skip optional bracket argument on \item (e.g. \item[label]).
                self.parse_optional_argument();

                // Collect blocks for this item until next \item or \end.
                let item_blocks = self.parse_list_item_blocks(env_name);
                items.push(ListItem {
                    checked: None,
                    content: item_blocks,
                });
            } else {
                // Skip unexpected tokens.
                self.advance();
            }
        }

        Block::List {
            ordered,
            start: if ordered { Some(1) } else { None },
            items,
            tight: false,
            style: None,
            delimiter: None,
        }
    }

    /// Parse blocks for a single list item until the next `\item` or `\end{env_name}`.
    fn parse_list_item_blocks(&mut self, env_name: &str) -> Vec<Block> {
        let mut blocks = Vec::new();

        while let Some(tok) = self.peek() {
            // Stop on \end{env_name}, or on the \end of an enclosing environment.
            if matches!(tok, Token::EndEnv { name, .. } if name == env_name) {
                break;
            }
            if matches!(tok, Token::EndEnv { .. }) {
                if self.ends_enclosing_env() {
                    break;
                }
                continue;
            }
            // Stop on next \item.
            if matches!(tok, Token::Command { ref name, .. } if name == "item") {
                break;
            }

            // Skip blank lines.
            if matches!(tok, Token::BlankLine) {
                self.advance();
                continue;
            }
            // Skip comments.
            if matches!(tok, Token::Comment { .. }) {
                self.advance();
                continue;
            }
            // Skip bare newlines.
            if matches!(tok, Token::Newline) {
                self.advance();
                continue;
            }

            // Handle nested environments.
            if let Token::BeginEnv { ref name, line, .. } = tok {
                let nested_name = name.clone();
                let nested_line = *line;
                self.advance();
                if let Some(block) = self.parse_environment(&nested_name, nested_line) {
                    blocks.push(block);
                }
                continue;
            }

            // Handle display math.
            if matches!(tok, Token::MathDisplay { .. }) {
                if let Some(Token::MathDisplay { value }) = self.advance() {
                    blocks.push(Block::MathBlock {
                        content: value,
                        label: None,
                    });
                }
                continue;
            }

            // Otherwise, collect paragraph inlines.
            let start = self.pos;
            let inlines = self.collect_paragraph_inlines();
            if !inlines.is_empty() {
                blocks.push(Block::Paragraph { content: inlines });
            }
            if self.pos == start {
                self.skip_unparsed_token();
            }
        }

        blocks
    }

    /// Parse `\begin{quote}...\end{quote}` or `\begin{quotation}...\end{quotation}`.
    fn parse_blockquote(&mut self, env_name: &str) -> Block {
        let content = self.parse_blocks(Some(env_name));
        Block::BlockQuote { content }
    }

    /// Parse `\begin{verbatim}...\end{verbatim}` or `\begin{lstlisting}...\end{lstlisting}`.
    fn parse_verbatim_env(&mut self, env_name: &str) -> Block {
        // For lstlisting, try to extract [language=X] option.
        let language = if env_name == "lstlisting" {
            self.parse_lstlisting_language()
        } else {
            None
        };

        // Collect raw text tokens until \end{env_name}.
        let content = self.collect_raw_text_until_end(env_name);

        Block::CodeBlock {
            language,
            content,
            caption: None,
            label: None,
            attrs: None,
        }
    }

    /// Try to parse `[language=X]` option for lstlisting.
    fn parse_lstlisting_language(&mut self) -> Option<String> {
        if !matches!(self.peek(), Some(Token::BracketOpen)) {
            return None;
        }
        let opt_text = self.parse_optional_argument()?;
        // Parse "language=X" from the option text.
        for part in opt_text.split(',') {
            let part = part.trim();
            if let Some(lang) = part.strip_prefix("language=") {
                return Some(lang.trim().to_string());
            }
        }
        None
    }

    /// Collect raw text content until `\end{env_name}`, preserving whitespace.
    /// Used for verbatim-like environments.
    fn collect_raw_text_until_end(&mut self, env_name: &str) -> String {
        let mut text = String::new();

        while let Some(tok) = self.peek() {
            if matches!(tok, Token::EndEnv { name, .. } if name == env_name) {
                self.advance();
                break;
            }
            let tok = self.advance().unwrap();
            match tok {
                Token::Text { value } => text.push_str(&value),
                Token::Newline => text.push('\n'),
                Token::BlankLine => text.push_str("\n\n"),
                Token::Command { name, .. } => {
                    text.push('\\');
                    text.push_str(&name);
                }
                Token::BraceOpen => text.push('{'),
                Token::BraceClose => text.push('}'),
                Token::BracketOpen => text.push('['),
                Token::BracketClose => text.push(']'),
                Token::Tilde => text.push('~'),
                Token::Ampersand => text.push('&'),
                Token::DoubleBackslash { .. } => text.push_str("\\\\"),
                Token::MathInline { value } => {
                    text.push('$');
                    text.push_str(&value);
                    text.push('$');
                }
                Token::MathDisplay { value } => {
                    text.push_str("$$");
                    text.push_str(&value);
                    text.push_str("$$");
                }
                Token::Comment { value } => {
                    text.push('%');
                    text.push_str(&value);
                }
                Token::BeginEnv { name, .. } => {
                    text.push_str(&format!("\\begin{{{}}}", name));
                }
                Token::EndEnv { name, .. } => {
                    text.push_str(&format!("\\end{{{}}}", name));
                }
            }
        }

        // Strip leading/trailing newline that is just the env boundary.
        let trimmed = text.strip_prefix('\n').unwrap_or(&text);
        let trimmed = trimmed.strip_suffix('\n').unwrap_or(trimmed);
        trimmed.to_string()
    }

    /// Parse a math environment (`equation`, `align`, etc.).
    /// Collects all content (including nested envs like `aligned`) as raw math
    /// string, and extracts any `\label{...}` found inside.
    fn parse_math_env(&mut self, env_name: &str) -> Block {
        let mut content = String::new();
        let mut label: Option<String> = None;
        let mut depth: u32 = 1;

        while let Some(tok) = self.advance() {
            match tok {
                Token::EndEnv { ref name, .. } => {
                    depth -= 1;
                    if depth == 0 && name == env_name {
                        break;
                    }
                    content.push_str(&format!("\\end{{{}}}", name));
                }
                Token::BeginEnv { ref name, .. } => {
                    depth += 1;
                    content.push_str(&format!("\\begin{{{}}}", name));
                }
                Token::Command { ref name, .. } if name == "label" => {
                    let lbl = self.parse_brace_text();
                    label = Some(lbl);
                }
                Token::Text { ref value } => content.push_str(value),
                Token::Newline => content.push('\n'),
                Token::BlankLine => content.push_str("\n\n"),
                Token::Command { ref name, .. } => {
                    content.push('\\');
                    content.push_str(name);
                }
                Token::BraceOpen => content.push('{'),
                Token::BraceClose => content.push('}'),
                Token::BracketOpen => content.push('['),
                Token::BracketClose => content.push(']'),
                Token::Tilde => content.push('~'),
                Token::Ampersand => content.push('&'),
                Token::DoubleBackslash { .. } => content.push_str("\\\\"),
                Token::MathInline { ref value } => content.push_str(value),
                Token::MathDisplay { ref value } => content.push_str(value),
                Token::Comment { .. } => {}
            }
        }

        let content = content.trim().to_string();

        Block::MathBlock { content, label }
    }

    /// Parse `\begin{figure}...\end{figure}`.
    fn parse_figure(&mut self, env_name: &str) -> Block {
        // Consume optional placement arg like [htbp], and wrapfigure's
        // placement and width.
        self.parse_optional_argument();
        if env_name == "wrapfigure" {
            self.parse_brace_argument();
            self.parse_brace_argument();
        }
        let mut table: Option<Table> = None;

        let mut image_url: Option<String> = None;
        let mut caption: Option<Vec<Inline>> = None;
        let mut label: Option<String> = None;

        while let Some(tok) = self.peek() {
            if matches!(tok, Token::EndEnv { name, .. } if name == env_name) {
                self.advance();
                break;
            }

            if let Token::BeginEnv { name, .. } = tok {
                if Self::is_tabular_env(name) {
                    let env = name.clone();
                    self.advance();
                    table.get_or_insert(self.parse_tabular(&env));
                    continue;
                }
            }

            match tok {
                Token::Command { ref name, .. } => {
                    let cmd = name.clone();
                    match cmd.as_str() {
                        "centering" => {
                            self.advance();
                        }
                        "includegraphics" | "includegraphics*" => {
                            self.advance();
                            // Skip optional arguments like [width=\textwidth].
                            self.parse_optional_argument();
                            let url = self.parse_brace_text();
                            // Subfigures: the first image stands for the figure.
                            image_url.get_or_insert(url);
                        }
                        "caption" => {
                            self.advance();
                            let cap = self.parse_inline_content();
                            caption = Some(cap);
                        }
                        "label" => {
                            self.advance();
                            let lbl = self.parse_brace_text();
                            label = Some(lbl);
                        }
                        _ => {
                            self.advance();
                            // Consume any arguments to skip unknown commands.
                            self.parse_optional_argument();
                            if matches!(self.peek(), Some(Token::BraceOpen)) {
                                self.parse_brace_argument();
                            }
                        }
                    }
                }
                Token::Newline | Token::BlankLine | Token::Comment { .. } => {
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }

        if let (Some(mut table), None) = (table, &image_url) {
            // A table typeset inside a figure environment.
            table.caption = caption;
            table.label = label;
            return Block::Table(table);
        }

        Block::Figure {
            image: Image {
                url: image_url.unwrap_or_default(),
                alt: vec![],
                title: None,
                attrs: None,
            },
            caption,
            label,
            attrs: None,
        }
    }

    /// Parse `\begin{table}...\end{table}`.
    fn parse_table_env(&mut self, env_name: &str) -> Block {
        // Consume optional placement arg like [htbp].
        self.parse_optional_argument();

        let mut table: Option<Table> = None;
        let mut caption: Option<Vec<Inline>> = None;
        let mut label: Option<String> = None;

        while let Some(tok) = self.peek() {
            if matches!(tok, Token::EndEnv { name, .. } if name == env_name) {
                self.advance();
                break;
            }

            match tok {
                Token::BeginEnv { ref name, .. } if Self::is_tabular_env(name) => {
                    let env = name.clone();
                    self.advance();
                    table.get_or_insert(self.parse_tabular(&env));
                }
                Token::Command { ref name, .. } => {
                    let cmd = name.clone();
                    match cmd.as_str() {
                        "centering" => {
                            self.advance();
                        }
                        "caption" => {
                            self.advance();
                            let cap = self.parse_inline_content();
                            caption = Some(cap);
                        }
                        "label" => {
                            self.advance();
                            let lbl = self.parse_brace_text();
                            label = Some(lbl);
                        }
                        _ => {
                            self.advance();
                            self.parse_optional_argument();
                            if matches!(self.peek(), Some(Token::BraceOpen)) {
                                self.parse_brace_argument();
                            }
                        }
                    }
                }
                Token::Newline | Token::BlankLine | Token::Comment { .. } => {
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }

        let mut t = table.unwrap_or(Table {
            caption: None,
            label: None,
            columns: Vec::new(),
            header: None,
            rows: Vec::new(),
            foot: None,
            attrs: None,
        });

        t.caption = caption;
        t.label = label;

        Block::Table(t)
    }

    /// Parse `\begin{tabular}{|l|r|c|}...\end{tabular}`.
    fn parse_tabular(&mut self, env_name: &str) -> Table {
        // `tabularx` and `tabular*` take a width before the column spec.
        if matches!(env_name, "tabularx" | "tabular*" | "tabulary") {
            self.parse_brace_argument();
        }
        let col_spec_text = self.parse_brace_text();
        let columns = Self::parse_column_spec(&col_spec_text);

        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut row: Vec<String> = Vec::new();
        let mut cell = String::new();
        // `&` and `\\` inside braces (`\makecell{a\\b}`) do not split cells.
        let mut depth = 0usize;

        while let Some(tok) = self.advance() {
            match tok {
                Token::EndEnv { ref name, .. } if name == env_name => break,
                Token::Ampersand if depth == 0 => row.push(std::mem::take(&mut cell)),
                Token::DoubleBackslash { .. } if depth == 0 => {
                    row.push(std::mem::take(&mut cell));
                    rows.push(std::mem::take(&mut row));
                    self.skip_bracket_group();
                }
                Token::Command { ref name, .. } if Self::is_table_rule(name) => {
                    self.skip_rule_arguments(name);
                }
                Token::BraceOpen => {
                    depth += 1;
                    cell.push('{');
                }
                Token::BraceClose => {
                    depth = depth.saturating_sub(1);
                    cell.push('}');
                }
                other => Self::push_token_text(&mut cell, &other),
            }
        }
        if !cell.trim().is_empty() || !row.is_empty() {
            row.push(cell);
            rows.push(row);
        }

        let mut all_rows: Vec<Vec<TableCell>> = rows
            .into_iter()
            .filter(|r| r.iter().any(|c| !c.trim().is_empty()))
            .map(|r| r.iter().map(|text| self.table_cell(text.trim())).collect())
            .collect();

        // First row is the header.
        let header = (!all_rows.is_empty()).then(|| all_rows.remove(0));
        Table {
            caption: None,
            label: None,
            columns,
            header,
            rows: all_rows,
            foot: None,
            attrs: None,
        }
    }

    fn is_tabular_env(name: &str) -> bool {
        matches!(
            name,
            "tabular" | "tabular*" | "tabularx" | "tabulary" | "longtable" | "array"
        )
    }

    fn is_table_rule(name: &str) -> bool {
        matches!(
            name,
            "hline"
                | "toprule"
                | "midrule"
                | "bottomrule"
                | "cline"
                | "cmidrule"
                | "specialrule"
                | "addlinespace"
                | "hhline"
        )
    }

    /// Skip what follows a rule command: `\cmidrule(lr){2-3}`, `\cline{2-3}`,
    /// `\specialrule{a}{b}{c}`, `\addlinespace[4pt]`.
    fn skip_rule_arguments(&mut self, name: &str) {
        self.skip_bracket_group();
        if let Some(Token::Text { value }) = self.peek() {
            if value.trim_start().starts_with('(') {
                let rest = value.split_once(')').map(|(_, r)| r.to_string());
                match rest {
                    Some(r) if !r.is_empty() => self.tokens[self.pos] = Token::Text { value: r },
                    _ => {
                        self.advance();
                    }
                }
            }
        }
        let braces = match name {
            "cline" | "cmidrule" | "hhline" => 1,
            "specialrule" => 3,
            _ => 0,
        };
        for _ in 0..braces {
            self.parse_brace_argument();
        }
    }

    /// Skip an optional `[...]` group (row spacing after `\\`, rule widths).
    fn skip_bracket_group(&mut self) {
        let mut j = self.pos;
        while matches!(self.tokens.get(j), Some(Token::Newline)) {
            j += 1;
        }
        if matches!(self.tokens.get(j), Some(Token::BracketOpen)) {
            self.pos = j;
            self.parse_optional_argument();
        }
    }

    /// Append a token's source text to a cell being collected.
    fn push_token_text(cell: &mut String, tok: &Token) {
        match tok {
            Token::Text { value } => cell.push_str(value),
            Token::Newline => cell.push(' '),
            Token::Tilde => cell.push('~'),
            Token::BracketOpen => cell.push('['),
            Token::BracketClose => cell.push(']'),
            Token::Ampersand => cell.push_str("\\&"),
            Token::DoubleBackslash { .. } => cell.push_str("\\\\"),
            Token::Command { name, .. } => {
                cell.push('\\');
                cell.push_str(name);
                // Keep `\bf 3` from becoming `\bf3`.
                cell.push(' ');
            }
            Token::MathInline { value } => {
                cell.push('$');
                cell.push_str(value);
                cell.push('$');
            }
            _ => {}
        }
    }

    /// Build a cell; `\multicolumn{n}` and `\multirow{n}` set its span.
    fn table_cell(&mut self, text: &str) -> TableCell {
        let span = |prefix: &str| {
            text.strip_prefix(prefix)
                .and_then(|rest| rest.trim_start().strip_prefix('{'))
                .and_then(|rest| rest.split_once('}'))
                .and_then(|(n, _)| n.trim().parse::<u32>().ok())
                .filter(|n| *n > 0)
                .unwrap_or(1)
        };
        let colspan = span("\\multicolumn");
        // \multirow leaves an empty cell in the rows it covers, so the span is
        // not kept: the text sits in the first row and the grid stays intact.
        let rowspan = 1;
        let inlines = self.parse_cell_text(text);
        TableCell {
            content: if inlines.is_empty() {
                Vec::new()
            } else {
                vec![Block::Paragraph { content: inlines }]
            },
            colspan,
            rowspan,
        }
    }

    /// Parse a column spec string like `|l|r|c|` into column specs.
    fn parse_column_spec(spec: &str) -> Vec<ColumnSpec> {
        let mut columns = Vec::new();
        for ch in spec.chars() {
            match ch {
                'l' => columns.push(ColumnSpec {
                    alignment: Alignment::Left,
                    width: None,
                }),
                'r' => columns.push(ColumnSpec {
                    alignment: Alignment::Right,
                    width: None,
                }),
                'c' => columns.push(ColumnSpec {
                    alignment: Alignment::Center,
                    width: None,
                }),
                // Skip pipe characters, spacing, and other decorators.
                _ => {}
            }
        }
        columns
    }

    /// Parse cell text (a simple string) as inlines by re-tokenizing.
    fn parse_cell_text(&mut self, text: &str) -> Vec<Inline> {
        if text.is_empty() {
            return Vec::new();
        }
        let tokens = crate::lexer::tokenize(text);
        let mut sub = Parser {
            tokens,
            pos: 0,
            warnings: Vec::new(),
            footnote_defs: Vec::new(),
            footnote_counter: self.footnote_counter,
            abstract_text: None,
            open_envs: Vec::new(),
        };

        let mut inlines = Vec::new();
        while sub.peek().is_some() {
            let tok = sub.advance().unwrap();
            if let Some(i) = sub.token_to_inline(tok) {
                inlines.push(i);
            }
        }

        self.warnings.append(&mut sub.warnings);
        self.footnote_defs.append(&mut sub.footnote_defs);
        self.footnote_counter = sub.footnote_counter;

        inlines
    }

    /// Parse `\begin{description}...\end{description}`.
    fn parse_description(&mut self) -> Block {
        let mut items: Vec<DefinitionItem> = Vec::new();

        self.skip_whitespace();

        while let Some(tok) = self.peek() {
            if matches!(tok, Token::EndEnv { name, .. } if name == "description") {
                self.advance();
                break;
            }

            if matches!(tok, Token::Command { ref name, .. } if name == "item") {
                self.advance(); // consume \item

                // Parse the bracket argument as the term.
                let term_text = self.parse_optional_argument().unwrap_or_default();
                let term_inlines = self.parse_cell_text(&term_text);

                // Collect blocks for the definition.
                let def_blocks = self.parse_list_item_blocks("description");

                items.push(DefinitionItem {
                    term: term_inlines,
                    definitions: vec![def_blocks],
                });
            } else {
                self.advance();
            }
        }

        Block::DefinitionList { items }
    }

    /// Skip whitespace and newline tokens.
    fn skip_whitespace(&mut self) {
        while let Some(tok) = self.peek() {
            match tok {
                Token::Newline | Token::BlankLine | Token::Comment { .. } => {
                    self.advance();
                }
                _ => break,
            }
        }
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
                | "item"
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
    fn texttt_reads_its_commands_and_flattens_them() {
        let input = r#"\texttt{"Input: <s>{\textbackslash}n", \textbf{bold}, a\_b \{x\} 50\%}"#;
        let doc = Parser::new(tokenize(input)).parse();
        let Block::Paragraph { content } = &doc.content[0] else {
            panic!("expected a paragraph, got {:?}", doc.content[0]);
        };
        let Inline::Code { value, .. } = &content[0] else {
            panic!("expected code, got {:?}", content[0]);
        };
        assert_eq!(value, r#""Input: <s>\n", bold, a_b {x} 50%"#);
    }

    fn paragraphs_text(blocks: &[Block]) -> String {
        blocks
            .iter()
            .filter_map(|b| match b {
                Block::Paragraph { content } => Some(docmux_ast::inlines_to_plain_text(content)),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn unclosed_environment_ends_with_the_document() {
        let input = "\\begin{document}\n\\begin{note}\nInside.\n\\end{document}";
        let doc = Parser::new(tokenize(input)).parse();
        assert!(matches!(&doc.content[..], [Block::Div { content, .. }] if content.len() == 1));
    }

    #[test]
    fn stray_end_is_skipped() {
        let input = "\\begin{document}\nBefore \\end{note} after.\n\\end{document}";
        let doc = Parser::new(tokenize(input)).parse();
        let text = paragraphs_text(&doc.content);
        assert!(text.contains("Before") && text.contains("after."), "{text}");
    }

    #[test]
    fn item_outside_a_list_environment_keeps_its_text() {
        let input = "\\begin{document}\n\\begin{steps}\n\\item First.\n\\item Second.\n\\end{steps}\n\\end{document}";
        let doc = Parser::new(tokenize(input)).parse();
        let Block::Div { content, .. } = &doc.content[0] else {
            panic!("expected a div, got {:?}", doc.content);
        };
        let text = paragraphs_text(content);
        assert!(
            text.contains("First.") && text.contains("Second."),
            "{text}"
        );
    }

    #[test]
    fn theorem_title_reads_references_and_citations() {
        let input = "\\begin{proof}[Proof of Theorem \\ref{thm:main}]\nDone.\n\\end{proof}";
        let doc = Parser::new(tokenize(input)).parse();
        let Block::Div { content, .. } = &doc.content[0] else {
            panic!("expected a div, got {:?}", doc.content);
        };
        let Block::Paragraph { content } = &content[0] else {
            panic!("expected a paragraph, got {content:?}");
        };
        let Inline::Emphasis { content: lead } = &content[0] else {
            panic!("expected an emphasized lead, got {content:?}");
        };
        assert!(
            lead.iter()
                .any(|i| matches!(i, Inline::CrossRef(r) if r.target == "thm:main")),
            "{lead:?}"
        );
    }

    #[test]
    fn citation_notes_and_bibitem_labels_resolve_commands() {
        let input = "\\begin{document}\nSee \\citep[][\\textit{inter alia}]{k}.\n\\begin{thebibliography}{9}\n\\bibitem[Kone{\\v{c}}n{\\'y} et~al.(2015)]{k} A paper.\n\\end{thebibliography}\n\\end{document}";
        let doc = Parser::new(tokenize(input)).parse();
        let Block::Paragraph { content } = &doc.content[0] else {
            panic!("expected a paragraph, got {:?}", doc.content);
        };
        let cite = content.iter().find_map(|i| match i {
            Inline::Citation(c) => Some(c),
            _ => None,
        });
        assert_eq!(cite.unwrap().items[0].suffix.as_deref(), Some("inter alia"));
        let Block::Div { content: refs, .. } = &doc.content[1] else {
            panic!("expected the references, got {:?}", doc.content);
        };
        let label = refs.iter().find_map(|b| match b {
            Block::Div { attrs, .. } => attrs.key_values.get("label"),
            _ => None,
        });
        assert_eq!(label.map(String::as_str), Some("Konečný et al.(2015)"));
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
    fn parse_unknown_command_keeps_its_text_and_warns() {
        let tokens = tokenize(r"\weirdcommand{arg}");
        let doc = Parser::new(tokens).parse();
        assert!(!doc.warnings.is_empty());
        let Block::Paragraph { content } = &doc.content[0] else {
            panic!("expected paragraph, got {:?}", doc.content);
        };
        assert!(matches!(&content[0], Inline::Text { value } if value == "arg"));
    }

    #[test]
    fn layout_commands_keep_text_and_drop_layout() {
        let tokens = tokenize(
            r"A \textcolor{red}{red} \vspace{2mm}\multicolumn{2}{c}{wide} {\bf bold} {\em it} \ie{} done",
        );
        let doc = Parser::new(tokens).parse();
        let Block::Paragraph { content } = &doc.content[0] else {
            panic!("expected paragraph, got {:?}", doc.content);
        };
        let text = format!("{content:?}");
        assert!(text.contains("\"red\""));
        assert!(text.contains("\"wide\""));
        assert!(!text.contains("2mm"));
        assert!(content.iter().any(|i| matches!(i, Inline::Strong { .. })));
        assert!(content.iter().any(|i| matches!(i, Inline::Emphasis { .. })));
        assert!(text.contains("i.e."));
        assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
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

    // ── Block-level environment tests ────────────────────────────────────

    #[test]
    fn parse_heading_section() {
        let tokens = tokenize(r"\section{Introduction}");
        let doc = Parser::new(tokens).parse();
        assert!(matches!(&doc.content[0], Block::Heading { level: 1, .. }));
    }

    #[test]
    fn parse_heading_subsection() {
        let tokens = tokenize(r"\subsection{Details}");
        let doc = Parser::new(tokens).parse();
        assert!(matches!(&doc.content[0], Block::Heading { level: 2, .. }));
    }

    #[test]
    fn parse_natbib_citations() {
        let tokens = tokenize(
            r"A \citep{a} B \citet{b,c} C \citep[see][p.~5]{d} D \citep[ch.~2]{e} \nocite{f}",
        );
        let doc = Parser::new(tokens).parse();
        let Block::Paragraph { content } = &doc.content[0] else {
            panic!("expected paragraph, got {:?}", doc.content[0]);
        };
        let cites: Vec<&Citation> = content
            .iter()
            .filter_map(|i| match i {
                Inline::Citation(c) => Some(c),
                _ => None,
            })
            .collect();
        assert_eq!(cites.len(), 4);
        assert!(matches!(cites[0].mode, CitationMode::Normal));
        assert!(matches!(cites[1].mode, CitationMode::AuthorOnly));
        assert_eq!(cites[1].items.len(), 2);
        assert_eq!(cites[2].items[0].prefix.as_deref(), Some("see"));
        assert_eq!(cites[2].items[0].suffix.as_deref(), Some("p.\u{00A0}5"));
        assert_eq!(cites[3].items[0].prefix, None);
        assert_eq!(cites[3].items[0].suffix.as_deref(), Some("ch.\u{00A0}2"));
    }

    #[test]
    fn parse_heading_label_after_space() {
        let tokens = tokenize(
            r"\subsection{Attention} \label{sec:attention}
Body.",
        );
        let doc = Parser::new(tokens).parse();
        match &doc.content[0] {
            Block::Heading { id, .. } => assert_eq!(id.as_deref(), Some("sec:attention")),
            other => panic!("expected heading, got {other:?}"),
        }
    }

    #[test]
    fn parse_heading_starred() {
        let tokens = tokenize(r"\section*{No Number}");
        let doc = Parser::new(tokens).parse();
        if let Block::Heading { id, .. } = &doc.content[0] {
            assert!(id.is_none());
        }
    }

    #[test]
    fn parse_itemize() {
        let input = r"\begin{itemize}
\item First
\item Second
\end{itemize}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        if let Block::List { ordered, items, .. } = &doc.content[0] {
            assert!(!ordered);
            assert_eq!(items.len(), 2);
        } else {
            panic!("Expected List, got {:?}", doc.content[0]);
        }
    }

    #[test]
    fn parse_enumerate() {
        let input = r"\begin{enumerate}
\item First
\item Second
\end{enumerate}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        if let Block::List { ordered, .. } = &doc.content[0] {
            assert!(ordered);
        } else {
            panic!("Expected ordered List");
        }
    }

    #[test]
    fn parse_blockquote() {
        let input = r"\begin{quote}
Some quoted text.
\end{quote}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        assert!(matches!(&doc.content[0], Block::BlockQuote { .. }));
    }

    #[test]
    fn parse_verbatim() {
        let input = r"\begin{verbatim}
fn main() {}
\end{verbatim}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        if let Block::CodeBlock {
            language, content, ..
        } = &doc.content[0]
        {
            assert!(language.is_none());
            assert!(content.contains("fn main()"));
        } else {
            panic!("Expected CodeBlock, got {:?}", doc.content[0]);
        }
    }

    #[test]
    fn parse_lstlisting_with_language() {
        let input = r"\begin{lstlisting}[language=Rust]
fn main() {}
\end{lstlisting}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        if let Block::CodeBlock { language, .. } = &doc.content[0] {
            assert_eq!(language.as_deref(), Some("Rust"));
        } else {
            panic!("Expected CodeBlock, got {:?}", doc.content[0]);
        }
    }

    #[test]
    fn parse_equation() {
        let input = r"\begin{equation}
E = mc^2
\end{equation}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        assert!(matches!(&doc.content[0], Block::MathBlock { .. }));
    }

    #[test]
    fn parse_display_math_brackets() {
        let tokens = tokenize(r"\[x^2 + y^2 = z^2\]");
        let doc = Parser::new(tokens).parse();
        assert!(matches!(&doc.content[0], Block::MathBlock { .. }));
    }

    #[test]
    fn parse_figure() {
        let input = r"\begin{figure}[htbp]
\centering
\includegraphics{image.png}
\caption{A figure}
\label{fig:test}
\end{figure}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        if let Block::Figure {
            image,
            caption,
            label,
            ..
        } = &doc.content[0]
        {
            assert_eq!(image.url, "image.png");
            assert!(caption.is_some());
            assert_eq!(label.as_deref(), Some("fig:test"));
        } else {
            panic!("Expected Figure, got {:?}", doc.content[0]);
        }
    }

    #[test]
    fn parse_table_tabular() {
        let input = r"\begin{table}[htbp]
\centering
\begin{tabular}{|l|r|}
\hline
Name & Value \\
\hline
Pi & 3.14 \\
\hline
\end{tabular}
\caption{Results}
\label{tab:results}
\end{table}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        if let Block::Table(table) = &doc.content[0] {
            assert!(table.header.is_some());
            assert!(!table.rows.is_empty());
            assert!(table.caption.is_some());
            assert_eq!(table.label.as_deref(), Some("tab:results"));
        } else {
            panic!("Expected Table, got {:?}", doc.content[0]);
        }
    }

    #[test]
    fn parse_abstract() {
        let input = r"\begin{document}
\begin{abstract}
This is the abstract.
\end{abstract}
Body text.
\end{document}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        assert!(doc.metadata.abstract_text.is_some());
        if let Some(blocks) = &doc.metadata.abstract_text {
            if let Block::Paragraph { content } = &blocks[0] {
                if let Inline::Text { value } = &content[0] {
                    assert_eq!(value.as_str(), "This is the abstract.");
                } else {
                    panic!("Expected Text inline");
                }
            } else {
                panic!("Expected Paragraph block");
            }
        }
    }

    #[test]
    fn parse_description_list() {
        let input = r"\begin{description}
\item[Term] Definition text.
\end{description}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        assert!(matches!(&doc.content[0], Block::DefinitionList { .. }));
    }

    #[test]
    fn parse_thematic_break() {
        let tokens = tokenize(r"\hrule");
        let doc = Parser::new(tokens).parse();
        assert!(matches!(&doc.content[0], Block::ThematicBreak));
    }

    #[test]
    fn parse_unknown_environment_produces_raw_block() {
        let input = r"\begin{tikzpicture}
\draw (0,0) -- (1,1);
\end{tikzpicture}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        assert!(matches!(&doc.content[0], Block::RawBlock { format, .. } if format == "latex"));
        assert!(!doc.warnings.is_empty());
    }

    #[test]
    fn parse_footnotetext() {
        let input = r"Text\footnotemark[1].

\footnotetext[1]{The footnote content.}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        assert!(doc
            .content
            .iter()
            .any(|b| matches!(b, Block::FootnoteDef { id, .. } if id == "1")));
    }

    #[test]
    fn parse_label_after_section() {
        let input = r"\section{Introduction}
\label{sec:intro}

Some text.";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        if let Block::Heading { id, .. } = &doc.content[0] {
            assert_eq!(id.as_deref(), Some("sec:intro"));
        } else {
            panic!("Expected Heading");
        }
    }

    #[test]
    fn parse_equation_with_nested_aligned() {
        let input = r"\begin{equation}
\begin{aligned}
x &= 1 \\
y &= 2
\end{aligned}
\end{equation}";
        let tokens = tokenize(input);
        let doc = Parser::new(tokens).parse();
        if let Block::MathBlock { content, .. } = &doc.content[0] {
            assert!(content.contains("aligned"));
        } else {
            panic!("Expected MathBlock");
        }
    }
}
