use crate::lexer::Token;
use docmux_ast::{
    Block, CrossRef, DefinitionItem, Document, Inline, ListItem, Metadata, ParseWarning, RefForm,
};

/// Recursive descent parser for Typst documents.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    warnings: Vec<ParseWarning>,
    footnote_defs: Vec<Block>,
    /// Reserved for Task 6 (footnote handling).
    #[allow(dead_code)]
    footnote_counter: usize,
    /// Reserved for Task 6/7 (bibliography support).
    #[allow(dead_code)]
    bibliography_path: Option<String>,
    /// Reserved for Task 7 (metadata parsing from raw input).
    #[allow(dead_code)]
    raw_input: String,
}

/// Function calls that represent Typst directives which should be silently
/// consumed (with their arguments) without producing output or warnings.
const SILENTLY_IGNORED_FUNCS: &[&str] = &["set", "show", "let", "import", "pagebreak"];

impl Parser {
    /// Create a new parser from a token stream produced by the lexer.
    pub fn new(tokens: Vec<Token>, raw_input: &str) -> Self {
        Self {
            tokens,
            pos: 0,
            warnings: Vec::new(),
            footnote_defs: Vec::new(),
            footnote_counter: 0,
            bibliography_path: None,
            raw_input: raw_input.to_string(),
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

    /// Skip past any `BlankLine` tokens at the current position.
    fn skip_blank_lines(&mut self) {
        while matches!(self.peek(), Some(Token::BlankLine)) {
            self.advance();
        }
    }

    /// Skip past any `Newline` tokens at the current position.
    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Some(Token::Newline)) {
            self.advance();
        }
    }

    // ── Entry point ─────────────────────────────────────────────────────────

    /// Entry point: parse all tokens and return a `Document`.
    pub fn parse(mut self) -> Document {
        let metadata = self.parse_metadata();
        let content = self.parse_body();

        let mut all_content = content;
        all_content.append(&mut self.footnote_defs);

        Document {
            metadata,
            content: all_content,
            bibliography: None,
            warnings: self.warnings,
        }
    }

    // ── Metadata (stub for Task 7) ──────────────────────────────────────────

    /// Parse frontmatter/metadata. Stub — will be expanded in Task 7.
    fn parse_metadata(&mut self) -> Metadata {
        // If the first token is a RawFrontmatter, consume it but for now
        // return default metadata. Task 7 will do proper YAML parsing.
        if matches!(self.peek(), Some(Token::RawFrontmatter { .. })) {
            self.advance();
        }
        Metadata::default()
    }

    // ── Body parsing ────────────────────────────────────────────────────────

    /// Parse the document body into a sequence of blocks.
    fn parse_body(&mut self) -> Vec<Block> {
        let mut blocks = Vec::new();

        loop {
            self.skip_blank_lines();

            if self.peek().is_none() {
                break;
            }

            // Skip comments at block level.
            if matches!(
                self.peek(),
                Some(Token::Comment { .. }) | Some(Token::BlockComment { .. })
            ) {
                self.advance();
                continue;
            }

            // Skip bare newlines at block level.
            if matches!(self.peek(), Some(Token::Newline)) {
                self.advance();
                continue;
            }

            if let Some(block) = self.parse_block() {
                blocks.push(block);
            }
        }

        blocks
    }

    // ── Block dispatch ──────────────────────────────────────────────────────

    /// Dispatch to the appropriate block parser based on the current token.
    fn parse_block(&mut self) -> Option<Block> {
        match self.peek()? {
            Token::Heading { .. } => Some(self.parse_heading()),

            Token::Dollar => {
                if self.is_display_math() {
                    Some(self.parse_display_math_block())
                } else {
                    Some(self.parse_paragraph())
                }
            }

            Token::Dash { count } => {
                let count = *count;
                if count >= 3 {
                    self.advance(); // consume ---
                    Some(Block::ThematicBreak)
                } else if count == 1 {
                    Some(self.parse_unordered_list())
                } else {
                    // 2-dash en-dash: treat as paragraph content
                    Some(self.parse_paragraph())
                }
            }

            Token::Plus { .. } => Some(self.parse_ordered_list()),

            Token::TermMarker { .. } => Some(self.parse_definition_list()),

            Token::Backtick { count } if *count >= 3 => Some(self.parse_code_block()),

            Token::FuncCall { .. } => self.parse_func_call_block(),

            _ => Some(self.parse_paragraph()),
        }
    }

    // ── Heading ─────────────────────────────────────────────────────────────

    /// Parse a heading: `= Title` / `== Subtitle` etc.
    fn parse_heading(&mut self) -> Block {
        let level = match self.advance() {
            Some(Token::Heading { level, .. }) => level,
            _ => 1,
        };

        let content = self.collect_inlines_until_newline();
        let (content, label) = self.extract_label_from_inlines(content);

        let id = label;

        Block::Heading { level, id, content }
    }

    // ── Paragraph ───────────────────────────────────────────────────────────

    /// Parse a paragraph: collect inlines until a blank line or block-level token.
    fn parse_paragraph(&mut self) -> Block {
        let mut inlines = Vec::new();

        while let Some(tok) = self.peek() {
            // Stop at paragraph boundaries.
            if matches!(tok, Token::BlankLine) {
                break;
            }

            // Stop at block-level tokens (they start a new block).
            if self.is_block_start(tok) {
                break;
            }

            // A newline in a paragraph becomes a soft break, but only if more
            // inline content follows (not a blank line or block start).
            if matches!(tok, Token::Newline) {
                self.advance();
                // Check what follows
                match self.peek() {
                    None | Some(Token::BlankLine) => break,
                    Some(t) if self.is_block_start(t) => break,
                    _ => {
                        inlines.push(Inline::SoftBreak);
                        continue;
                    }
                }
            }

            if let Some(inline) = self.parse_inline() {
                inlines.push(inline);
            }
        }

        // Trim trailing soft breaks.
        while matches!(inlines.last(), Some(Inline::SoftBreak)) {
            inlines.pop();
        }

        Block::Paragraph { content: inlines }
    }

    /// Returns true if the token starts a new block (used to stop paragraph collection).
    fn is_block_start(&self, tok: &Token) -> bool {
        matches!(
            tok,
            Token::Heading { .. }
                | Token::Plus { .. }
                | Token::TermMarker { .. }
                | Token::Dash { count: 1 }
        ) || matches!(tok, Token::Dash { count } if *count >= 3)
            || matches!(tok, Token::Backtick { count } if *count >= 3)
            || matches!(tok, Token::FuncCall { .. })
    }

    // ── Display math block ──────────────────────────────────────────────────

    /// Detect whether the current `Dollar` starts a display math block.
    ///
    /// In Typst, display math is `$ content $` where `$` is followed by a
    /// newline or space.
    fn is_display_math(&self) -> bool {
        if !matches!(self.tokens.get(self.pos), Some(Token::Dollar)) {
            return false;
        }
        // The lexer's math mode absorbs everything between $ tokens into a
        // single Text token. Display math has a leading space or newline.
        match self.tokens.get(self.pos + 1) {
            Some(Token::Newline) => true,
            Some(Token::Text { value }) if value.starts_with(' ') || value.starts_with('\n') => {
                true
            }
            _ => false,
        }
    }

    /// Parse a display math block: `$ ... $` (with leading space/newline).
    fn parse_display_math_block(&mut self) -> Block {
        self.advance(); // consume opening Dollar

        let mut content = String::new();

        while let Some(tok) = self.peek() {
            if matches!(tok, Token::Dollar) {
                self.advance(); // consume closing Dollar
                break;
            }
            match self.advance() {
                Some(Token::Text { value }) => content.push_str(&value),
                Some(Token::Newline) => content.push('\n'),
                Some(Token::BlankLine) => content.push_str("\n\n"),
                Some(Token::Star) => content.push('*'),
                Some(Token::Underscore) => content.push('_'),
                Some(Token::Backslash) => content.push('\\'),
                Some(Token::BraceOpen) => content.push('{'),
                Some(Token::BraceClose) => content.push('}'),
                Some(Token::BracketOpen) => content.push('['),
                Some(Token::BracketClose) => content.push(']'),
                Some(Token::ParenOpen) => content.push('('),
                Some(Token::ParenClose) => content.push(')'),
                Some(Token::Colon) => content.push(':'),
                Some(Token::Comma) => content.push(','),
                Some(Token::Escape { ch }) => {
                    content.push('\\');
                    content.push(ch);
                }
                _ => {}
            }
        }

        let content = content.trim().to_string();

        Block::MathBlock {
            content,
            label: None,
        }
    }

    // ── Code block (stub for Task 6 expansion) ─────────────────────────────

    /// Parse a fenced code block: ` ```lang ... ``` `
    ///
    /// The lexer absorbs everything between ``` delimiters into a single Text
    /// token (e.g. `"rust\nfn main() {}\n"`). We split on the first newline
    /// to extract the optional language identifier.
    fn parse_code_block(&mut self) -> Block {
        self.advance(); // consume opening Backtick{3}

        // The lexer collects all content between ``` as a single Text token.
        let mut raw_content = String::new();
        while let Some(tok) = self.peek() {
            if matches!(tok, Token::Backtick { count } if *count >= 3) {
                self.advance(); // consume closing ```
                break;
            }
            match self.advance() {
                Some(Token::Text { value }) => raw_content.push_str(&value),
                Some(Token::Newline) => raw_content.push('\n'),
                Some(Token::BlankLine) => raw_content.push_str("\n\n"),
                _ => {}
            }
        }

        // Split: first line is the language, rest is code content.
        let (language, content) = if let Some(nl_pos) = raw_content.find('\n') {
            let lang = raw_content[..nl_pos].trim().to_string();
            let code = raw_content[nl_pos + 1..].to_string();
            let lang = if lang.is_empty() { None } else { Some(lang) };
            (lang, code)
        } else {
            // No newline — entire thing is content (unusual but handle it).
            (None, raw_content)
        };

        // Trim trailing newline from content.
        let content = if content.ends_with('\n') {
            content[..content.len() - 1].to_string()
        } else {
            content
        };

        Block::CodeBlock {
            language,
            content,
            caption: None,
            label: None,
        }
    }

    // ── Lists ───────────────────────────────────────────────────────────────

    /// Parse an unordered list: consecutive `- item` lines.
    fn parse_unordered_list(&mut self) -> Block {
        let mut items = Vec::new();

        while matches!(self.peek(), Some(Token::Dash { count: 1 })) {
            self.advance(); // consume Dash{1}

            let inlines = self.collect_inlines_until_newline();
            let inlines = trim_leading_space(inlines);

            items.push(ListItem {
                checked: None,
                content: vec![Block::Paragraph { content: inlines }],
            });

            // Skip the newline between items.
            self.skip_newlines();
        }

        Block::List {
            ordered: false,
            start: None,
            items,
        }
    }

    /// Parse an ordered list: consecutive `+ item` lines.
    fn parse_ordered_list(&mut self) -> Block {
        let mut items = Vec::new();

        while matches!(self.peek(), Some(Token::Plus { .. })) {
            self.advance(); // consume Plus

            let inlines = self.collect_inlines_until_newline();

            items.push(ListItem {
                checked: None,
                content: vec![Block::Paragraph { content: inlines }],
            });

            // Skip the newline between items.
            self.skip_newlines();
        }

        Block::List {
            ordered: true,
            start: Some(1),
            items,
        }
    }

    /// Parse a definition list: consecutive `/ Term: definition` items.
    fn parse_definition_list(&mut self) -> Block {
        let mut items = Vec::new();

        while matches!(self.peek(), Some(Token::TermMarker { .. })) {
            self.advance(); // consume TermMarker

            // Collect term inlines until Colon.
            let mut term = Vec::new();
            while let Some(tok) = self.peek() {
                if matches!(tok, Token::Colon) {
                    self.advance(); // consume Colon
                    break;
                }
                if matches!(tok, Token::Newline | Token::BlankLine) {
                    break;
                }
                if let Some(inline) = self.parse_inline() {
                    term.push(inline);
                }
            }

            // Consume optional space after colon
            // (already handled naturally by Text tokens)

            // Collect definition inlines until newline.
            let def_inlines = self.collect_inlines_until_newline();
            let def_inlines = trim_leading_space(def_inlines);

            items.push(DefinitionItem {
                term,
                definitions: vec![vec![Block::Paragraph {
                    content: def_inlines,
                }]],
            });

            // Skip newlines between items.
            self.skip_newlines();
        }

        Block::DefinitionList { items }
    }

    // ── Inline parsing ──────────────────────────────────────────────────────

    /// Collect inlines until newline, blank line, or EOF.
    fn collect_inlines_until_newline(&mut self) -> Vec<Inline> {
        let mut inlines = Vec::new();

        while let Some(tok) = self.peek() {
            if matches!(tok, Token::Newline | Token::BlankLine) {
                // Don't consume — let the caller handle it.
                break;
            }
            if let Some(inline) = self.parse_inline() {
                inlines.push(inline);
            }
        }

        inlines
    }

    /// Parse a single inline element from the current position.
    fn parse_inline(&mut self) -> Option<Inline> {
        match self.peek()? {
            Token::Text { .. } => {
                if let Some(Token::Text { value }) = self.advance() {
                    Some(Inline::Text { value })
                } else {
                    None
                }
            }

            Token::Star => {
                self.advance(); // consume opening *
                let content = self.collect_inlines_until(|tok| matches!(tok, Token::Star));
                // Consume closing *
                if matches!(self.peek(), Some(Token::Star)) {
                    self.advance();
                }
                Some(Inline::Strong { content })
            }

            Token::Underscore => {
                self.advance(); // consume opening _
                let content = self.collect_inlines_until(|tok| matches!(tok, Token::Underscore));
                // Consume closing _
                if matches!(self.peek(), Some(Token::Underscore)) {
                    self.advance();
                }
                Some(Inline::Emphasis { content })
            }

            Token::Backtick { count: 1 } => {
                self.advance(); // consume opening `
                let value = self.collect_raw_text_until_backtick();
                Some(Inline::Code { value })
            }

            Token::Dollar => {
                // Inline math only (display math is handled at block level).
                Some(self.parse_math())
            }

            Token::Backslash => {
                self.advance(); // consume backslash
                                // If followed by Newline, it's a hard break.
                if matches!(self.peek(), Some(Token::Newline)) {
                    self.advance();
                    Some(Inline::HardBreak)
                } else {
                    Some(Inline::Text {
                        value: "\\".to_string(),
                    })
                }
            }

            Token::Escape { .. } => {
                if let Some(Token::Escape { ch }) = self.advance() {
                    Some(Inline::Text {
                        value: ch.to_string(),
                    })
                } else {
                    None
                }
            }

            Token::AtRef { .. } => {
                if let Some(Token::AtRef { name }) = self.advance() {
                    Some(Inline::CrossRef(CrossRef {
                        target: name,
                        form: RefForm::NumberWithType,
                    }))
                } else {
                    None
                }
            }

            Token::FuncCall { .. } => self.parse_inline_func_call(),

            Token::Label { .. } => {
                // Labels at inline level: skip them (they attach to blocks).
                self.advance();
                None
            }

            Token::Dash { .. } => {
                if let Some(Token::Dash { count }) = self.advance() {
                    let text = match count {
                        2 => "\u{2013}".to_string(), // en-dash
                        3 => "\u{2014}".to_string(), // em-dash
                        _ => "-".repeat(count as usize),
                    };
                    Some(Inline::Text { value: text })
                } else {
                    None
                }
            }

            // Punctuation tokens that appear inline — emit as text.
            Token::Colon => {
                self.advance();
                Some(Inline::Text {
                    value: ":".to_string(),
                })
            }
            Token::Comma => {
                self.advance();
                Some(Inline::Text {
                    value: ",".to_string(),
                })
            }
            Token::ParenOpen => {
                self.advance();
                Some(Inline::Text {
                    value: "(".to_string(),
                })
            }
            Token::ParenClose => {
                self.advance();
                Some(Inline::Text {
                    value: ")".to_string(),
                })
            }
            Token::BracketOpen => {
                self.advance();
                Some(Inline::Text {
                    value: "[".to_string(),
                })
            }
            Token::BracketClose => {
                self.advance();
                Some(Inline::Text {
                    value: "]".to_string(),
                })
            }
            Token::BraceOpen => {
                self.advance();
                Some(Inline::Text {
                    value: "{".to_string(),
                })
            }
            Token::BraceClose => {
                self.advance();
                Some(Inline::Text {
                    value: "}".to_string(),
                })
            }

            // Skip comment tokens that appear mid-paragraph.
            Token::Comment { .. } | Token::BlockComment { .. } => {
                self.advance();
                None
            }

            // StringLit encountered inline — emit its content as text.
            Token::StringLit { .. } => {
                if let Some(Token::StringLit { value }) = self.advance() {
                    Some(Inline::Text { value })
                } else {
                    None
                }
            }

            // Backtick with count > 1 but < 3 — treat as text.
            Token::Backtick { .. } => {
                if let Some(Token::Backtick { count }) = self.advance() {
                    Some(Inline::Text {
                        value: "`".repeat(count as usize),
                    })
                } else {
                    None
                }
            }

            // Tokens that shouldn't normally appear inline — advance to avoid infinite loop.
            _ => {
                self.advance();
                None
            }
        }
    }

    /// Collect inline elements until a predicate matches the current token
    /// or we hit a newline/EOF.
    fn collect_inlines_until<F>(&mut self, stop: F) -> Vec<Inline>
    where
        F: Fn(&Token) -> bool,
    {
        let mut inlines = Vec::new();

        while let Some(tok) = self.peek() {
            if stop(tok) {
                break;
            }
            if matches!(tok, Token::Newline | Token::BlankLine) {
                break;
            }
            if let Some(inline) = self.parse_inline() {
                inlines.push(inline);
            }
        }

        inlines
    }

    /// Collect raw text until the next single backtick (for inline code).
    fn collect_raw_text_until_backtick(&mut self) -> String {
        let mut s = String::new();

        while let Some(tok) = self.peek() {
            if matches!(tok, Token::Backtick { count: 1 }) {
                self.advance(); // consume closing backtick
                break;
            }
            if matches!(tok, Token::Newline | Token::BlankLine) {
                break;
            }
            match self.advance() {
                Some(Token::Text { value }) => s.push_str(&value),
                Some(Token::Star) => s.push('*'),
                Some(Token::Underscore) => s.push('_'),
                Some(Token::Dollar) => s.push('$'),
                Some(Token::Backslash) => s.push('\\'),
                Some(Token::Escape { ch }) => {
                    s.push('\\');
                    s.push(ch);
                }
                Some(Token::Colon) => s.push(':'),
                Some(Token::Comma) => s.push(','),
                Some(Token::ParenOpen) => s.push('('),
                Some(Token::ParenClose) => s.push(')'),
                Some(Token::BracketOpen) => s.push('['),
                Some(Token::BracketClose) => s.push(']'),
                Some(Token::BraceOpen) => s.push('{'),
                Some(Token::BraceClose) => s.push('}'),
                Some(Token::Dash { count }) => {
                    for _ in 0..count {
                        s.push('-');
                    }
                }
                _ => {}
            }
        }

        s
    }

    /// Parse inline math: `$...$`. Only produces `Inline::MathInline`.
    fn parse_math(&mut self) -> Inline {
        self.advance(); // consume opening Dollar

        let mut content = String::new();

        while let Some(tok) = self.peek() {
            if matches!(tok, Token::Dollar) {
                self.advance(); // consume closing Dollar
                break;
            }
            match self.advance() {
                Some(Token::Text { value }) => content.push_str(&value),
                Some(Token::Star) => content.push('*'),
                Some(Token::Underscore) => content.push('_'),
                Some(Token::Backslash) => content.push('\\'),
                Some(Token::BraceOpen) => content.push('{'),
                Some(Token::BraceClose) => content.push('}'),
                Some(Token::BracketOpen) => content.push('['),
                Some(Token::BracketClose) => content.push(']'),
                Some(Token::ParenOpen) => content.push('('),
                Some(Token::ParenClose) => content.push(')'),
                Some(Token::Colon) => content.push(':'),
                Some(Token::Comma) => content.push(','),
                Some(Token::Escape { ch }) => {
                    content.push('\\');
                    content.push(ch);
                }
                _ => {}
            }
        }

        Inline::MathInline {
            value: content.trim().to_string(),
        }
    }

    // ── Function call stubs (Tasks 6 & 7) ───────────────────────────────────

    /// Handle a block-level function call. Stub for Task 6.
    ///
    /// Known directive-style functions (#set, #show, #let, #import, #pagebreak)
    /// are silently consumed. Unknown functions emit a warning and produce a
    /// `RawBlock`.
    fn parse_func_call_block(&mut self) -> Option<Block> {
        let (name, line) = match self.peek() {
            Some(Token::FuncCall { name, line }) => (name.clone(), *line),
            _ => return None,
        };
        self.advance(); // consume FuncCall

        if SILENTLY_IGNORED_FUNCS.contains(&name.as_str()) {
            self.consume_func_arguments();
            return None;
        }

        // Unknown block function — consume args, emit warning + RawBlock.
        let args_text = self.consume_func_arguments_as_text();
        self.warnings.push(ParseWarning {
            line,
            message: format!("Unknown block function: #{}", name),
        });

        Some(Block::RawBlock {
            format: "typst".to_string(),
            content: format!("#{}{}", name, args_text),
        })
    }

    /// Handle an inline function call. Stub for Task 6.
    fn parse_inline_func_call(&mut self) -> Option<Inline> {
        let (name, line) = match self.peek() {
            Some(Token::FuncCall { name, line }) => (name.clone(), *line),
            _ => return None,
        };
        self.advance(); // consume FuncCall

        if SILENTLY_IGNORED_FUNCS.contains(&name.as_str()) {
            self.consume_func_arguments();
            return None;
        }

        // Unknown inline function — consume args, emit warning + RawInline.
        let args_text = self.consume_func_arguments_as_text();
        self.warnings.push(ParseWarning {
            line,
            message: format!("Unknown inline function: #{}", name),
        });

        Some(Inline::RawInline {
            format: "typst".to_string(),
            content: format!("#{}{}", name, args_text),
        })
    }

    /// Consume function arguments (parenthesized and/or bracketed) silently.
    ///
    /// For directive-style calls like `#set text(font: "Arial")`, the tokens
    /// between the function name and the parentheses (e.g. ` text`) are also
    /// consumed. We eat tokens until we find `(`, `[`, newline, or blank line.
    fn consume_func_arguments(&mut self) {
        // Consume any tokens before parenthesized/bracketed args (e.g. `text`
        // in `#set text(...)`).
        loop {
            match self.peek() {
                Some(Token::ParenOpen) | Some(Token::BracketOpen) => break,
                Some(Token::Newline) | Some(Token::BlankLine) | None => return,
                _ => {
                    self.advance();
                }
            }
        }

        // Consume parenthesized args: (...)
        if matches!(self.peek(), Some(Token::ParenOpen)) {
            self.consume_balanced(Token::ParenOpen, Token::ParenClose);
        }
        // Consume content block args: [...]
        if matches!(self.peek(), Some(Token::BracketOpen)) {
            self.consume_balanced(Token::BracketOpen, Token::BracketClose);
        }
    }

    /// Consume function arguments and return them as a string representation.
    fn consume_func_arguments_as_text(&mut self) -> String {
        let mut text = String::new();

        // Consume any tokens before parenthesized/bracketed args.
        loop {
            match self.peek() {
                Some(Token::ParenOpen) | Some(Token::BracketOpen) => break,
                Some(Token::Newline) | Some(Token::BlankLine) | None => return text,
                _ => {
                    if let Some(Token::Text { value }) = self.advance() {
                        text.push_str(&value);
                    }
                }
            }
        }

        // Consume parenthesized args: (...)
        if matches!(self.peek(), Some(Token::ParenOpen)) {
            text.push('(');
            self.advance(); // consume (
            let mut depth: u32 = 1;
            while let Some(tok) = self.advance() {
                match &tok {
                    Token::ParenOpen => {
                        depth += 1;
                        text.push('(');
                    }
                    Token::ParenClose => {
                        depth -= 1;
                        if depth == 0 {
                            text.push(')');
                            break;
                        }
                        text.push(')');
                    }
                    Token::Text { value } => text.push_str(value),
                    Token::StringLit { value } => {
                        text.push('"');
                        text.push_str(value);
                        text.push('"');
                    }
                    Token::Comma => text.push(','),
                    Token::Colon => text.push(':'),
                    _ => {}
                }
            }
        }

        // Consume content block args: [...]
        if matches!(self.peek(), Some(Token::BracketOpen)) {
            text.push('[');
            self.advance(); // consume [
            let mut depth: u32 = 1;
            while let Some(tok) = self.advance() {
                match &tok {
                    Token::BracketOpen => {
                        depth += 1;
                        text.push('[');
                    }
                    Token::BracketClose => {
                        depth -= 1;
                        if depth == 0 {
                            text.push(']');
                            break;
                        }
                        text.push(']');
                    }
                    Token::Text { value } => text.push_str(value),
                    _ => {}
                }
            }
        }

        text
    }

    /// Consume balanced tokens (e.g., matching parens or brackets).
    fn consume_balanced(&mut self, open: Token, close: Token) {
        if self.peek() != Some(&open) {
            return;
        }
        self.advance(); // consume open
        let mut depth: u32 = 1;
        while let Some(tok) = self.advance() {
            if tok == open {
                depth += 1;
            } else if tok == close {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
        }
    }

    // ── Utilities ───────────────────────────────────────────────────────────

    /// Extract a label from the end of an inline sequence.
    /// If the last inline "token" was a Label, remove it and return the name.
    fn extract_label_from_inlines(
        &self,
        mut inlines: Vec<Inline>,
    ) -> (Vec<Inline>, Option<String>) {
        // Labels are skipped during inline parsing (returning None), so we
        // need to check the raw tokens that followed the heading. However,
        // since the label is consumed by parse_inline as None, it won't be
        // in the inlines vec. We use a different approach: peek at the token
        // after the heading content.
        //
        // For now, check if there's a Label token at the current position.
        let label = if let Some(Token::Label { name }) = self.peek() {
            Some(name.clone())
        } else {
            None
        };

        // Clean up trailing whitespace text from inlines.
        if let Some(Inline::Text { value }) = inlines.last() {
            if value.trim().is_empty() {
                inlines.pop();
            }
        }

        (inlines, label)
    }
}

/// Trim a leading space from the first Text inline (for list items where the
/// lexer puts the space into the Text token).
fn trim_leading_space(mut inlines: Vec<Inline>) -> Vec<Inline> {
    if let Some(Inline::Text { value }) = inlines.first_mut() {
        if value.starts_with(' ') {
            *value = value.trim_start().to_string();
            if value.is_empty() {
                inlines.remove(0);
            }
        }
    }
    inlines
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Document {
        let tokens = crate::lexer::tokenize(input);
        Parser::new(tokens, input).parse()
    }

    // ── Task 3: Headings, paragraphs, thematic breaks ───────────────────

    #[test]
    fn parse_heading_level1() {
        let doc = parse("= Introduction\n");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Heading { level, content, .. } => {
                assert_eq!(*level, 1);
                assert!(matches!(&content[0], Inline::Text { value } if value == "Introduction"));
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn parse_heading_level3() {
        let doc = parse("=== Deep section\n");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Heading { level, .. } => assert_eq!(*level, 3),
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn parse_paragraph() {
        let doc = parse("Hello world.");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(matches!(&content[0], Inline::Text { value } if value == "Hello world."));
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_heading_then_paragraph() {
        let doc = parse("= Title\n\nSome text here.");
        assert_eq!(doc.content.len(), 2);
        assert!(matches!(&doc.content[0], Block::Heading { level: 1, .. }));
        assert!(matches!(&doc.content[1], Block::Paragraph { .. }));
    }

    #[test]
    fn parse_thematic_break() {
        let doc = parse("Above\n\n---\n\nBelow");
        // Should be: Paragraph, ThematicBreak, Paragraph
        assert_eq!(doc.content.len(), 3);
        assert!(matches!(&doc.content[0], Block::Paragraph { .. }));
        assert!(matches!(&doc.content[1], Block::ThematicBreak));
        assert!(matches!(&doc.content[2], Block::Paragraph { .. }));
    }

    // ── Task 4: Inline formatting ───────────────────────────────────────

    #[test]
    fn parse_bold() {
        let doc = parse("*bold text*");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(matches!(&content[0], Inline::Strong { .. }));
                if let Inline::Strong { content: inner } = &content[0] {
                    assert!(matches!(&inner[0], Inline::Text { value } if value == "bold text"));
                }
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_italic() {
        let doc = parse("_italic text_");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(matches!(&content[0], Inline::Emphasis { .. }));
                if let Inline::Emphasis { content: inner } = &content[0] {
                    assert!(matches!(&inner[0], Inline::Text { value } if value == "italic text"));
                }
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_inline_code() {
        let doc = parse("`some code`");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(matches!(&content[0], Inline::Code { value } if value == "some code"));
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_inline_math() {
        let doc = parse("$x^2 + y^2$");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(
                    matches!(&content[0], Inline::MathInline { value } if value == "x^2 + y^2")
                );
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_display_math() {
        let doc = parse("$ x^2 + y^2 $");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::MathBlock { content, label } => {
                assert_eq!(content, "x^2 + y^2");
                assert!(label.is_none());
            }
            other => panic!("Expected MathBlock, got {:?}", other),
        }
    }

    #[test]
    fn parse_display_math_multiline() {
        let doc = parse("$\n  a + b\n  = c\n$");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::MathBlock { content, .. } => {
                assert!(content.contains("a + b"));
                assert!(content.contains("= c"));
            }
            other => panic!("Expected MathBlock, got {:?}", other),
        }
    }

    #[test]
    fn parse_hard_break() {
        let doc = parse("line one\\\nline two");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(content.iter().any(|i| matches!(i, Inline::HardBreak)));
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_soft_break() {
        let doc = parse("line one\nline two");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(content.iter().any(|i| matches!(i, Inline::SoftBreak)));
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    // ── Task 5: Lists ───────────────────────────────────────────────────

    #[test]
    fn parse_unordered_list() {
        let doc = parse("- First\n- Second\n- Third");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::List {
                ordered,
                items,
                start,
            } => {
                assert!(!ordered);
                assert!(start.is_none());
                assert_eq!(items.len(), 3);
            }
            other => panic!("Expected List, got {:?}", other),
        }
    }

    #[test]
    fn parse_ordered_list() {
        let doc = parse("+ Alpha\n+ Beta\n+ Gamma");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::List {
                ordered,
                items,
                start,
            } => {
                assert!(ordered);
                assert_eq!(*start, Some(1));
                assert_eq!(items.len(), 3);
            }
            other => panic!("Expected List, got {:?}", other),
        }
    }

    #[test]
    fn parse_definition_list() {
        let doc = parse("/ Term: Definition text");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::DefinitionList { items } => {
                assert_eq!(items.len(), 1);
                // Check term
                assert!(
                    matches!(&items[0].term[0], Inline::Text { value } if value.trim() == "Term")
                );
                // Check definition
                assert_eq!(items[0].definitions.len(), 1);
            }
            other => panic!("Expected DefinitionList, got {:?}", other),
        }
    }

    // ── Stubs ───────────────────────────────────────────────────────────

    #[test]
    fn parse_silently_ignored_functions() {
        let doc = parse("#set text(font: \"Arial\")\n\nHello");
        // #set should be silently consumed, no warnings.
        assert!(doc.warnings.is_empty());
        // Only "Hello" paragraph should remain.
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::Paragraph { .. }));
    }

    #[test]
    fn parse_unknown_function_emits_warning() {
        let doc = parse("#unknown(arg)");
        assert!(!doc.warnings.is_empty());
        assert!(doc.warnings[0].message.contains("unknown"));
    }

    #[test]
    fn parse_code_block() {
        let doc = parse("```rust\nfn main() {}\n```");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::CodeBlock {
                language, content, ..
            } => {
                assert_eq!(language.as_deref(), Some("rust"));
                assert!(content.contains("fn main()"));
            }
            other => panic!("Expected CodeBlock, got {:?}", other),
        }
    }
}
