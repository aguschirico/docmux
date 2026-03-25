use crate::lexer::Token;
use docmux_ast::{
    Alignment, Author, Block, Citation, CitationMode, CiteItem, ColumnSpec, CrossRef,
    DefinitionItem, Document, Image, Inline, ListItem, MetaValue, Metadata, ParseWarning, RefForm,
    Table, TableCell,
};
use std::collections::HashMap;

// ── Internal argument types ──────────────────────────────────────────────────

/// A parsed function argument.
#[derive(Debug, Clone)]
enum Arg {
    Positional(ArgValue),
    Named(String, ArgValue),
}

/// A parsed argument value.
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum ArgValue {
    String(String),
    Content(Vec<Token>),
    Identifier(String),
    Bool(bool),
    FuncCall(String, Vec<Arg>),
    Raw(String),
}

/// Recursive descent parser for Typst documents.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    warnings: Vec<ParseWarning>,
    footnote_defs: Vec<Block>,
    footnote_counter: usize,
    bibliography_path: Option<String>,
    raw_input: String,
}

/// Function calls that represent Typst directives which should be silently
/// consumed (with their arguments) without producing output or warnings.
const SILENTLY_IGNORED_FUNCS: &[&str] = &["show", "let", "import", "pagebreak"];

/// Inline-level function names that, when appearing at block level, should be
/// wrapped in a paragraph rather than producing a RawBlock.
const INLINE_FUNC_NAMES: &[&str] = &[
    "emph",
    "strong",
    "strike",
    "sub",
    "super",
    "smallcaps",
    "link",
    "cite",
    "footnote",
    "raw",
    "underline",
];

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

    // ── Metadata (Task 7) ──────────────────────────────────────────────────

    /// Parse frontmatter/metadata from YAML frontmatter and `#set document()`.
    ///
    /// Two sources, with YAML taking priority:
    /// 1. `---\n...\n---` YAML frontmatter (consumed as `RawFrontmatter` token)
    /// 2. `#set document(title: ..., author: ..., date: ...)` calls in the body
    fn parse_metadata(&mut self) -> Metadata {
        // Step 1: Parse YAML frontmatter if present.
        let yaml_meta = if let Some(Token::RawFrontmatter { .. }) = self.peek() {
            if let Some(Token::RawFrontmatter { value }) = self.advance() {
                Self::parse_yaml_frontmatter(&value)
            } else {
                Metadata::default()
            }
        } else {
            Metadata::default()
        };

        // Step 2: Scan raw_input for `#set document(...)` to extract metadata.
        let set_doc_meta = self.extract_set_document_metadata();

        // Step 3: Merge — YAML takes priority.
        Metadata {
            title: yaml_meta.title.or(set_doc_meta.title),
            authors: if yaml_meta.authors.is_empty() {
                set_doc_meta.authors
            } else {
                yaml_meta.authors
            },
            date: yaml_meta.date.or(set_doc_meta.date),
            abstract_text: yaml_meta.abstract_text.or(set_doc_meta.abstract_text),
            keywords: if yaml_meta.keywords.is_empty() {
                set_doc_meta.keywords
            } else {
                yaml_meta.keywords
            },
            custom: yaml_meta.custom,
        }
    }

    /// Parse a YAML string into Metadata (two-pass approach matching markdown reader).
    fn parse_yaml_frontmatter(yaml: &str) -> Metadata {
        let value: serde_yaml::Value = match serde_yaml::from_str(yaml) {
            Ok(v) => v,
            Err(_) => return Metadata::default(),
        };

        let mapping = match value.as_mapping() {
            Some(m) => m,
            None => return Metadata::default(),
        };

        let mut metadata = Metadata::default();
        let mut custom = HashMap::new();

        for (key, val) in mapping {
            let key_str = match key.as_str() {
                Some(s) => s,
                None => continue,
            };

            match key_str {
                "title" => {
                    metadata.title = val.as_str().map(String::from);
                }
                "date" => {
                    metadata.date = yaml_value_to_string(val);
                }
                "abstract" | "abstract_text" | "description" => {
                    metadata.abstract_text = val.as_str().map(|s| {
                        vec![Block::Paragraph {
                            content: vec![Inline::text(s)],
                        }]
                    });
                }
                "keywords" | "tags" => {
                    metadata.keywords = parse_string_list(val);
                }
                "author" | "authors" => {
                    metadata.authors = parse_authors(val);
                }
                _ => {
                    if let Some(mv) = yaml_to_meta_value(val) {
                        custom.insert(key_str.to_string(), mv);
                    }
                }
            }
        }

        metadata.custom = custom;
        metadata
    }

    /// Scan the raw input for `#set document(...)` and extract metadata fields.
    fn extract_set_document_metadata(&self) -> Metadata {
        let mut meta = Metadata::default();

        // Simple text scan for #set document(...)
        let input = &self.raw_input;
        let needle = "#set document(";
        if let Some(start) = input.find(needle) {
            let after = &input[start + needle.len()..];
            // Find matching closing paren.
            let mut depth: u32 = 1;
            let mut end = 0;
            for (i, ch) in after.char_indices() {
                match ch {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            end = i;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if end > 0 {
                let args_str = &after[..end];
                self.parse_set_document_args(args_str, &mut meta);
            }
        }

        meta
    }

    /// Parse the arguments inside `#set document(...)`.
    fn parse_set_document_args(&self, args_str: &str, meta: &mut Metadata) {
        // Parse key: value pairs, handling strings and basic values.
        // This is a simplified parser for the most common patterns.
        let mut remaining = args_str.trim();

        while !remaining.is_empty() {
            // Skip whitespace and commas.
            remaining = remaining.trim_start_matches(|c: char| c == ',' || c.is_whitespace());
            if remaining.is_empty() {
                break;
            }

            // Find the key: look for `key:`
            let colon_pos = match remaining.find(':') {
                Some(p) => p,
                None => break,
            };
            let key = remaining[..colon_pos].trim();
            remaining = remaining[colon_pos + 1..].trim();

            // Parse the value.
            let value = if remaining.starts_with('"') {
                // String value.
                let end = find_closing_quote(remaining);
                let val = &remaining[1..end];
                remaining = remaining[end + 1..].trim();
                remaining = remaining.trim_start_matches(',').trim();
                val.to_string()
            } else if remaining.starts_with('(') {
                // Tuple/array value — extract content between parens.
                let mut depth: u32 = 0;
                let mut end = 0;
                for (i, ch) in remaining.char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => {
                            depth -= 1;
                            if depth == 0 {
                                end = i;
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let val = &remaining[..=end];
                remaining = remaining[end + 1..].trim();
                remaining = remaining.trim_start_matches(',').trim();
                val.to_string()
            } else {
                // Bare value — read until comma or end.
                let end = remaining.find(',').unwrap_or(remaining.len());
                let val = remaining[..end].trim().to_string();
                remaining = if end < remaining.len() {
                    remaining[end + 1..].trim()
                } else {
                    ""
                };
                val
            };

            match key {
                "title" => {
                    meta.title = Some(value);
                }
                "author" => {
                    // Can be a single string or a tuple like ("Alice", "Bob")
                    if value.starts_with('(') && value.ends_with(')') {
                        let inner = &value[1..value.len() - 1];
                        meta.authors = inner
                            .split(',')
                            .map(|s| {
                                let s = s.trim().trim_matches('"');
                                Author {
                                    name: s.to_string(),
                                    affiliation: None,
                                    email: None,
                                    orcid: None,
                                }
                            })
                            .collect();
                    } else {
                        meta.authors = vec![Author {
                            name: value,
                            affiliation: None,
                            email: None,
                            orcid: None,
                        }];
                    }
                }
                "date" => {
                    meta.date = Some(value);
                }
                _ => {}
            }
        }
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
                // Task 7: Label attachment — check if next token is a Label.
                let block = self.maybe_attach_label(block);
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

            Token::FuncCall { name, .. } if !INLINE_FUNC_NAMES.contains(&name.as_str()) => {
                self.parse_func_call_block()
            }

            _ => Some(self.parse_paragraph()),
        }
    }

    // ── Label attachment (Task 7) ───────────────────────────────────────────

    /// After parsing a block, check if the next token (possibly after whitespace)
    /// is a `Label` and attach it.
    fn maybe_attach_label(&mut self, block: Block) -> Block {
        // Skip whitespace-only Text tokens to find a following Label.
        let saved_pos = self.pos;
        while let Some(Token::Text { value }) = self.peek() {
            if value.trim().is_empty() {
                self.advance();
            } else {
                break;
            }
        }

        if !matches!(self.peek(), Some(Token::Label { .. })) {
            // No label found — restore position.
            self.pos = saved_pos;
            return block;
        }

        let label_name = if let Some(Token::Label { name }) = self.advance() {
            name
        } else {
            self.pos = saved_pos;
            return block;
        };

        match block {
            Block::Heading {
                level, id, content, ..
            } => Block::Heading {
                level,
                id: id.or(Some(label_name)),
                content,
                attrs: None,
            },
            Block::MathBlock { content, label, .. } => Block::MathBlock {
                content,
                label: label.or(Some(label_name)),
            },
            Block::CodeBlock {
                language,
                content,
                caption,
                label,
                ..
            } => Block::CodeBlock {
                language,
                content,
                caption,
                label: label.or(Some(label_name)),
                attrs: None,
            },
            Block::Figure {
                image,
                caption,
                label,
                ..
            } => Block::Figure {
                image,
                caption,
                label: label.or(Some(label_name)),
                attrs: None,
            },
            Block::Table(mut t) => {
                if t.label.is_none() {
                    t.label = Some(label_name);
                }
                Block::Table(t)
            }
            other => other,
        }
    }

    // ── Heading ─────────────────────────────────────────────────────────────

    /// Parse a heading: `= Title` / `== Subtitle` etc.
    fn parse_heading(&mut self) -> Block {
        let level = match self.advance() {
            Some(Token::Heading { level, .. }) => level,
            _ => 1,
        };

        // Collect inlines, stopping at Label tokens (don't consume them).
        let content = self.collect_inlines_until_label_or_newline();

        // Check if the next token is a Label (for the heading id).
        let id = if let Some(Token::Label { .. }) = self.peek() {
            if let Some(Token::Label { name }) = self.advance() {
                Some(name)
            } else {
                None
            }
        } else {
            None
        };

        // Clean up trailing whitespace from the last text inline.
        let mut content = content;
        if let Some(Inline::Text { value }) = content.last_mut() {
            let trimmed = value.trim_end().to_string();
            if trimmed.is_empty() {
                content.pop();
            } else {
                *value = trimmed;
            }
        }

        Block::Heading {
            level,
            id,
            content,
            attrs: None,
        }
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
            || self.is_block_level_func(tok)
    }

    /// Check if a FuncCall token represents a block-level function.
    /// Inline functions (#emph, #strong, etc.) should not break paragraphs.
    fn is_block_level_func(&self, tok: &Token) -> bool {
        if let Token::FuncCall { name, .. } = tok {
            !INLINE_FUNC_NAMES.contains(&name.as_str())
        } else {
            false
        }
    }

    // ── Display math block ──────────────────────────────────────────────────

    /// Detect whether the current `Dollar` starts a display math block.
    fn is_display_math(&self) -> bool {
        if !matches!(self.tokens.get(self.pos), Some(Token::Dollar)) {
            return false;
        }
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

    // ── Code block ──────────────────────────────────────────────────────────

    /// Parse a fenced code block: ` ```lang ... ``` `
    fn parse_code_block(&mut self) -> Block {
        self.advance(); // consume opening Backtick{3}

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

        let (language, content) = if let Some(nl_pos) = raw_content.find('\n') {
            let lang = raw_content[..nl_pos].trim().to_string();
            let code = raw_content[nl_pos + 1..].to_string();
            let lang = if lang.is_empty() { None } else { Some(lang) };
            (lang, code)
        } else {
            (None, raw_content)
        };

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
            attrs: None,
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
            tight: true,
            style: None,
            delimiter: None,
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
            tight: true,
            style: None,
            delimiter: None,
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

    /// Collect inlines until a Label, newline, blank line, or EOF.
    /// Used by heading parsing so that the label token is preserved for id extraction.
    fn collect_inlines_until_label_or_newline(&mut self) -> Vec<Inline> {
        let mut inlines = Vec::new();

        while let Some(tok) = self.peek() {
            if matches!(tok, Token::Newline | Token::BlankLine | Token::Label { .. }) {
                break;
            }
            if let Some(inline) = self.parse_inline() {
                inlines.push(inline);
            }
        }

        inlines
    }

    /// Collect inlines until newline, blank line, or EOF.
    fn collect_inlines_until_newline(&mut self) -> Vec<Inline> {
        let mut inlines = Vec::new();

        while let Some(tok) = self.peek() {
            if matches!(tok, Token::Newline | Token::BlankLine) {
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
                Some(Inline::Code { value, attrs: None })
            }

            Token::Dollar => {
                // Inline math only (display math is handled at block level).
                Some(self.parse_math())
            }

            Token::Backslash => {
                self.advance(); // consume backslash
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

    // ── Argument parsing (Task 6) ───────────────────────────────────────────

    /// Parse function call arguments: `(arg1, key: val, ...)` and/or `[content]`.
    ///
    /// Returns a list of `Arg` values. Content blocks `[...]` are added as a
    /// positional `ArgValue::Content`.
    fn parse_args(&mut self) -> Vec<Arg> {
        let mut args = Vec::new();

        // Parse parenthesized args: (...)
        if matches!(self.peek(), Some(Token::ParenOpen)) {
            self.advance(); // consume (
            self.parse_paren_args(&mut args);
        }

        // Parse content block args: [...]
        if matches!(self.peek(), Some(Token::BracketOpen)) {
            let content_tokens = self.collect_bracket_content();
            args.push(Arg::Positional(ArgValue::Content(content_tokens)));
        }

        args
    }

    /// Parse arguments inside parentheses (already consumed the opening paren).
    fn parse_paren_args(&mut self, args: &mut Vec<Arg>) {
        loop {
            // Skip whitespace-like tokens.
            self.skip_arg_whitespace();

            // Check for closing paren.
            if matches!(self.peek(), Some(Token::ParenClose)) {
                self.advance(); // consume )
                break;
            }
            if self.peek().is_none() {
                break;
            }

            // Try to parse a named arg: `key: value`
            // Look ahead to see if we have `Identifier : Value` pattern.
            if let Some(arg) = self.try_parse_arg() {
                args.push(arg);
            }

            // Skip comma separator.
            if matches!(self.peek(), Some(Token::Comma)) {
                self.advance();
            }
        }
    }

    /// Skip whitespace tokens inside argument lists.
    fn skip_arg_whitespace(&mut self) {
        while matches!(
            self.peek(),
            Some(Token::Newline)
                | Some(Token::BlankLine)
                | Some(Token::Comment { .. })
                | Some(Token::BlockComment { .. })
        ) {
            self.advance();
        }
        // Also skip leading whitespace in Text tokens.
        if let Some(Token::Text { value }) = self.peek() {
            if value.trim().is_empty() {
                self.advance();
            }
        }
    }

    /// Try to parse a single argument (named or positional).
    fn try_parse_arg(&mut self) -> Option<Arg> {
        self.skip_arg_whitespace();

        // Check for named arg: `identifier: value`
        // We need to look ahead: Text(name) Colon Value
        if self.is_named_arg() {
            let name = match self.advance() {
                Some(Token::Text { value }) => value.trim().to_string(),
                _ => return None,
            };
            self.advance(); // consume Colon
            self.skip_arg_whitespace();
            let value = self.parse_arg_value()?;
            return Some(Arg::Named(name, value));
        }

        // Positional argument.
        let value = self.parse_arg_value()?;
        Some(Arg::Positional(value))
    }

    /// Check if the current position has a named argument pattern: `name: `.
    fn is_named_arg(&self) -> bool {
        if let Some(Token::Text { value }) = self.peek() {
            let trimmed = value.trim();
            // Must be a simple identifier.
            if !trimmed.is_empty()
                && trimmed
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
            {
                // Check next token is Colon.
                if matches!(self.tokens.get(self.pos + 1), Some(Token::Colon)) {
                    return true;
                }
            }
        }
        false
    }

    /// Parse a single argument value.
    fn parse_arg_value(&mut self) -> Option<ArgValue> {
        self.skip_arg_whitespace();

        match self.peek() {
            Some(Token::StringLit { .. }) => {
                if let Some(Token::StringLit { value }) = self.advance() {
                    Some(ArgValue::String(value))
                } else {
                    None
                }
            }
            Some(Token::BracketOpen) => {
                let content_tokens = self.collect_bracket_content();
                Some(ArgValue::Content(content_tokens))
            }
            Some(Token::Label { .. }) => {
                if let Some(Token::Label { name }) = self.advance() {
                    Some(ArgValue::Raw(format!("<{name}>")))
                } else {
                    Some(ArgValue::Raw(String::new()))
                }
            }
            Some(Token::FuncCall { .. }) => {
                // Nested function call: e.g. `image("path.png")`
                let (func_name, _line) = match self.peek() {
                    Some(Token::FuncCall { name, line }) => (name.clone(), *line),
                    _ => return None,
                };
                self.advance(); // consume FuncCall
                let nested_args = self.parse_args();
                Some(ArgValue::FuncCall(func_name, nested_args))
            }
            Some(Token::Text { value }) => {
                let trimmed = value.trim().to_string();
                // Check for boolean values.
                if trimmed == "true" {
                    self.advance();
                    Some(ArgValue::Bool(true))
                } else if trimmed == "false" {
                    self.advance();
                    Some(ArgValue::Bool(false))
                } else if trimmed.chars().all(|c| c.is_ascii_digit()) && !trimmed.is_empty() {
                    // Numeric value — store as Raw.
                    self.advance();
                    Some(ArgValue::Raw(trimmed))
                } else if !trimmed.is_empty()
                    && trimmed
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
                {
                    // Check if this identifier is followed by `(` — bare function call.
                    if matches!(self.tokens.get(self.pos + 1), Some(Token::ParenOpen)) {
                        let func_name = trimmed;
                        self.advance(); // consume Text (the function name)
                        let nested_args = self.parse_args();
                        Some(ArgValue::FuncCall(func_name, nested_args))
                    } else {
                        // Plain identifier.
                        self.advance();
                        Some(ArgValue::Identifier(trimmed))
                    }
                } else if trimmed.is_empty() {
                    // Skip empty text and try again.
                    self.advance();
                    self.parse_arg_value()
                } else {
                    // Raw value.
                    self.advance();
                    Some(ArgValue::Raw(trimmed))
                }
            }
            Some(Token::ParenClose) | Some(Token::Comma) => None,
            _ => {
                // Unknown token in args — consume and return Raw.
                self.advance();
                None
            }
        }
    }

    /// Collect balanced bracket content `[...]`, returning the inner tokens.
    fn collect_bracket_content(&mut self) -> Vec<Token> {
        if !matches!(self.peek(), Some(Token::BracketOpen)) {
            return Vec::new();
        }
        self.advance(); // consume [
        let mut tokens = Vec::new();
        let mut depth: u32 = 1;

        while let Some(tok) = self.advance() {
            match &tok {
                Token::BracketOpen => {
                    depth += 1;
                    tokens.push(tok);
                }
                Token::BracketClose => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                    tokens.push(tok);
                }
                _ => {
                    tokens.push(tok);
                }
            }
        }

        tokens
    }

    /// Parse content tokens (from a bracket content block) into inlines.
    fn parse_content_tokens_as_inlines(&mut self, tokens: Vec<Token>) -> Vec<Inline> {
        let mut sub = Parser::new(tokens, "");
        let mut inlines = Vec::new();
        while sub.peek().is_some() {
            if matches!(sub.peek(), Some(Token::Newline | Token::BlankLine)) {
                sub.advance();
                continue;
            }
            if let Some(inline) = sub.parse_inline() {
                inlines.push(inline);
            }
        }
        // Collect any footnote defs from sub-parser.
        self.footnote_defs.append(&mut sub.footnote_defs);
        self.warnings.append(&mut sub.warnings);
        inlines
    }

    /// Parse content tokens as blocks.
    fn parse_content_tokens_as_blocks(&mut self, tokens: Vec<Token>) -> Vec<Block> {
        let mut sub = Parser::new(tokens, "");
        let blocks = sub.parse_body();
        self.footnote_defs.append(&mut sub.footnote_defs);
        self.warnings.append(&mut sub.warnings);
        blocks
    }

    // ── Block-level function calls (Task 6) ─────────────────────────────────

    /// Handle a block-level function call.
    fn parse_func_call_block(&mut self) -> Option<Block> {
        let (name, line) = match self.peek() {
            Some(Token::FuncCall { name, line }) => (name.clone(), *line),
            _ => return None,
        };
        self.advance(); // consume FuncCall

        // Handle `#set` specially: may be `#set document(...)` for metadata,
        // or other #set calls that should be silently ignored.
        if name == "set" {
            self.handle_set_call();
            return None;
        }

        // Silently ignored functions.
        if SILENTLY_IGNORED_FUNCS.contains(&name.as_str()) {
            self.consume_func_arguments();
            return None;
        }

        match name.as_str() {
            "heading" => Some(self.parse_heading_func()),
            "image" => Some(self.parse_image_block()),
            "figure" => Some(self.parse_figure_block()),
            "table" => Some(self.parse_table_block()),
            "quote" => Some(self.parse_quote_block()),
            "bibliography" => {
                let args = self.parse_args();
                let path = args.iter().find_map(|a| match a {
                    Arg::Positional(ArgValue::String(s)) => Some(s.clone()),
                    _ => None,
                });
                if let Some(p) = path {
                    self.bibliography_path = Some(p);
                }
                None
            }
            _ => {
                // Unknown block function — consume args, emit warning + RawBlock.
                let args_text = self.consume_func_arguments_as_text();
                self.warnings.push(ParseWarning {
                    line,
                    message: format!("Unknown block function: #{name}"),
                });
                Some(Block::RawBlock {
                    format: "typst".to_string(),
                    content: format!("#{name}{args_text}"),
                })
            }
        }
    }

    /// Handle `#set` call — consume `set` keyword arguments. If it's
    /// `#set document(...)`, the metadata extraction happens separately via
    /// raw_input scanning, so just consume silently.
    fn handle_set_call(&mut self) {
        self.consume_func_arguments();
    }

    /// Parse `#heading(level: N)[content]`.
    fn parse_heading_func(&mut self) -> Block {
        let args = self.parse_args();

        let level = args
            .iter()
            .find_map(|a| match a {
                Arg::Named(k, ArgValue::Raw(v)) if k == "level" => v.parse::<u8>().ok(),
                Arg::Positional(ArgValue::Raw(v)) => v.parse::<u8>().ok(),
                _ => None,
            })
            .unwrap_or(1);

        let content = args.into_iter().find_map(|a| match a {
            Arg::Positional(ArgValue::Content(tokens)) => Some(tokens),
            Arg::Named(_, ArgValue::Content(tokens)) => Some(tokens),
            _ => None,
        });

        let inlines = if let Some(tokens) = content {
            self.parse_content_tokens_as_inlines(tokens)
        } else {
            Vec::new()
        };

        Block::Heading {
            level,
            id: None,
            content: inlines,
            attrs: None,
        }
    }

    /// Parse `#image("path.png", alt: "text")` at block level.
    fn parse_image_block(&mut self) -> Block {
        let args = self.parse_args();

        let url = args
            .iter()
            .find_map(|a| match a {
                Arg::Positional(ArgValue::String(s)) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        let alt = args
            .iter()
            .find_map(|a| match a {
                Arg::Named(k, ArgValue::String(s)) if k == "alt" => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_default();

        let alt = if alt.is_empty() {
            vec![]
        } else {
            vec![Inline::text(alt)]
        };

        Block::Figure {
            image: Image {
                url,
                alt,
                title: None,
                attrs: None,
            },
            caption: None,
            label: None,
            attrs: None,
        }
    }

    /// Parse `#figure(image(...), caption: [...])`.
    fn parse_figure_block(&mut self) -> Block {
        let args = self.parse_args();

        // Extract image from a nested image() func call.
        let mut image = Image {
            url: String::new(),
            alt: vec![],
            title: None,
            attrs: None,
        };
        let mut caption: Option<Vec<Inline>> = None;
        let mut label: Option<String> = None;

        for arg in &args {
            match arg {
                Arg::Positional(ArgValue::FuncCall(fname, fargs)) if fname == "image" => {
                    // Extract URL from first positional string arg.
                    if let Some(url) = fargs.iter().find_map(|a| match a {
                        Arg::Positional(ArgValue::String(s)) => Some(s.clone()),
                        _ => None,
                    }) {
                        image.url = url;
                    }
                    // Extract alt text.
                    if let Some(alt) = fargs.iter().find_map(|a| match a {
                        Arg::Named(k, ArgValue::String(s)) if k == "alt" => Some(s.clone()),
                        _ => None,
                    }) {
                        image.alt = if alt.is_empty() {
                            vec![]
                        } else {
                            vec![Inline::text(alt)]
                        };
                    }
                }
                Arg::Named(k, ArgValue::Content(tokens)) if k == "caption" => {
                    caption = Some(self.parse_content_tokens_as_inlines(tokens.clone()));
                }
                Arg::Named(k, ArgValue::Raw(v)) if k == "label" => {
                    label = Some(v.clone());
                }
                _ => {}
            }
        }

        Block::Figure {
            image,
            caption,
            label,
            attrs: None,
        }
    }

    /// Parse `#table(columns: N, [cell], ...)`.
    fn parse_table_block(&mut self) -> Block {
        let args = self.parse_args();

        // Extract column count.
        let num_columns = args
            .iter()
            .find_map(|a| match a {
                Arg::Named(k, ArgValue::Raw(v)) if k == "columns" => v.parse::<usize>().ok(),
                _ => None,
            })
            .unwrap_or(1);

        // Collect all content-block positional args as cells.
        let mut cells: Vec<Vec<Block>> = Vec::new();
        for arg in &args {
            if let Arg::Positional(ArgValue::Content(tokens)) = arg {
                let blocks = self.parse_content_tokens_as_blocks(tokens.clone());
                cells.push(blocks);
            }
        }

        // Build columns spec.
        let columns: Vec<ColumnSpec> = (0..num_columns)
            .map(|_| ColumnSpec {
                alignment: Alignment::Default,
                width: None,
            })
            .collect();

        // Split cells into header (first row) and body rows.
        let (header, rows) = if cells.len() > num_columns {
            let header_cells: Vec<TableCell> = cells[..num_columns]
                .iter()
                .map(|blocks| TableCell {
                    content: blocks.clone(),
                    colspan: 1,
                    rowspan: 1,
                })
                .collect();

            let body_cells: Vec<Vec<TableCell>> = cells[num_columns..]
                .chunks(num_columns)
                .map(|chunk| {
                    chunk
                        .iter()
                        .map(|blocks| TableCell {
                            content: blocks.clone(),
                            colspan: 1,
                            rowspan: 1,
                        })
                        .collect()
                })
                .collect();

            (Some(header_cells), body_cells)
        } else {
            // All cells in one row, no header.
            let row: Vec<TableCell> = cells
                .into_iter()
                .map(|blocks| TableCell {
                    content: blocks,
                    colspan: 1,
                    rowspan: 1,
                })
                .collect();
            if row.is_empty() {
                (None, Vec::new())
            } else {
                (None, vec![row])
            }
        };

        Block::Table(Table {
            caption: None,
            label: None,
            columns,
            header,
            rows,
            foot: None,
            attrs: None,
        })
    }

    /// Parse `#quote(block: true)[content]`.
    fn parse_quote_block(&mut self) -> Block {
        let args = self.parse_args();

        let content_tokens = args.into_iter().find_map(|a| match a {
            Arg::Positional(ArgValue::Content(tokens)) => Some(tokens),
            _ => None,
        });

        let blocks = if let Some(tokens) = content_tokens {
            self.parse_content_tokens_as_blocks(tokens)
        } else {
            Vec::new()
        };

        Block::BlockQuote { content: blocks }
    }

    // ── Inline function calls (Task 6) ──────────────────────────────────────

    /// Handle an inline function call.
    fn parse_inline_func_call(&mut self) -> Option<Inline> {
        let (name, line) = match self.peek() {
            Some(Token::FuncCall { name, line }) => (name.clone(), *line),
            _ => return None,
        };
        self.advance(); // consume FuncCall

        // Silently ignored functions.
        if SILENTLY_IGNORED_FUNCS.contains(&name.as_str()) || name == "set" {
            self.consume_func_arguments();
            return None;
        }

        match name.as_str() {
            "emph" => {
                let args = self.parse_args();
                let content = self.extract_content_inlines(args);
                Some(Inline::Emphasis { content })
            }
            "strong" => {
                let args = self.parse_args();
                let content = self.extract_content_inlines(args);
                Some(Inline::Strong { content })
            }
            "strike" => {
                let args = self.parse_args();
                let content = self.extract_content_inlines(args);
                Some(Inline::Strikethrough { content })
            }
            "sub" => {
                let args = self.parse_args();
                let content = self.extract_content_inlines(args);
                Some(Inline::Subscript { content })
            }
            "super" => {
                let args = self.parse_args();
                let content = self.extract_content_inlines(args);
                Some(Inline::Superscript { content })
            }
            "smallcaps" => {
                let args = self.parse_args();
                let content = self.extract_content_inlines(args);
                Some(Inline::SmallCaps { content })
            }
            "underline" => {
                // No AST underline — unwrap content.
                let args = self.parse_args();
                let content = self.extract_content_inlines(args);
                if content.len() == 1 {
                    Some(content.into_iter().next().unwrap())
                } else if content.is_empty() {
                    None
                } else {
                    // Return the first inline if multiple; wrap in Span to preserve grouping.
                    Some(Inline::Span {
                        content,
                        attrs: Default::default(),
                    })
                }
            }
            "link" => {
                let args = self.parse_args();

                let url = args
                    .iter()
                    .find_map(|a| match a {
                        Arg::Positional(ArgValue::String(s)) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                let content = self.extract_content_inlines_from_ref(&args);

                let content = if content.is_empty() {
                    vec![Inline::Text { value: url.clone() }]
                } else {
                    content
                };

                Some(Inline::Link {
                    url,
                    title: None,
                    content,
                    attrs: None,
                })
            }
            "cite" => {
                let args = self.parse_args();

                // Extract citation key from Label-style arg like <key> or string.
                let key = args
                    .iter()
                    .find_map(|a| match a {
                        Arg::Positional(ArgValue::Raw(s)) => {
                            // Strip angle brackets if present.
                            let s = s.trim_start_matches('<').trim_end_matches('>');
                            Some(s.to_string())
                        }
                        Arg::Positional(ArgValue::String(s)) => Some(s.clone()),
                        Arg::Positional(ArgValue::Identifier(s)) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                Some(Inline::Citation(Citation {
                    items: vec![CiteItem {
                        key,
                        prefix: None,
                        suffix: None,
                    }],
                    mode: CitationMode::Normal,
                }))
            }
            "footnote" => {
                let args = self.parse_args();

                self.footnote_counter += 1;
                let id = format!("fn-{}", self.footnote_counter);

                // Parse footnote content.
                let content_tokens = args.into_iter().find_map(|a| match a {
                    Arg::Positional(ArgValue::Content(tokens)) => Some(tokens),
                    _ => None,
                });

                let blocks = if let Some(tokens) = content_tokens {
                    self.parse_content_tokens_as_blocks(tokens)
                } else {
                    Vec::new()
                };

                self.footnote_defs.push(Block::FootnoteDef {
                    id: id.clone(),
                    content: blocks,
                });

                Some(Inline::FootnoteRef { id })
            }
            "raw" => {
                let args = self.parse_args();

                let value = args
                    .iter()
                    .find_map(|a| match a {
                        Arg::Positional(ArgValue::String(s)) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();

                Some(Inline::Code { value, attrs: None })
            }
            _ => {
                // Unknown inline function — consume args, emit warning + RawInline.
                let args_text = self.consume_func_arguments_as_text();
                self.warnings.push(ParseWarning {
                    line,
                    message: format!("Unknown inline function: #{name}"),
                });
                Some(Inline::RawInline {
                    format: "typst".to_string(),
                    content: format!("#{name}{args_text}"),
                })
            }
        }
    }

    /// Extract content inlines from a list of args (first Content arg).
    fn extract_content_inlines(&mut self, args: Vec<Arg>) -> Vec<Inline> {
        let content_tokens = args.into_iter().find_map(|a| match a {
            Arg::Positional(ArgValue::Content(tokens)) => Some(tokens),
            Arg::Named(_, ArgValue::Content(tokens)) => Some(tokens),
            _ => None,
        });

        if let Some(tokens) = content_tokens {
            self.parse_content_tokens_as_inlines(tokens)
        } else {
            Vec::new()
        }
    }

    /// Extract content inlines from a reference to args (without consuming).
    fn extract_content_inlines_from_ref(&mut self, args: &[Arg]) -> Vec<Inline> {
        let content_tokens = args.iter().find_map(|a| match a {
            Arg::Positional(ArgValue::Content(tokens)) => Some(tokens.clone()),
            Arg::Named(_, ArgValue::Content(tokens)) => Some(tokens.clone()),
            _ => None,
        });

        if let Some(tokens) = content_tokens {
            self.parse_content_tokens_as_inlines(tokens)
        } else {
            Vec::new()
        }
    }

    // ── Legacy consume helpers (kept for silently-ignored functions) ─────────

    /// Consume function arguments (parenthesized and/or bracketed) silently.
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
}

// ── YAML frontmatter helpers ────────────────────────────────────────────────

/// Parse the `author`/`authors` field.
fn parse_authors(val: &serde_yaml::Value) -> Vec<Author> {
    match val {
        serde_yaml::Value::String(s) => vec![Author {
            name: s.clone(),
            affiliation: None,
            email: None,
            orcid: None,
        }],
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|item| match item {
                serde_yaml::Value::String(s) => Some(Author {
                    name: s.clone(),
                    affiliation: None,
                    email: None,
                    orcid: None,
                }),
                serde_yaml::Value::Mapping(m) => {
                    let name = m
                        .get(serde_yaml::Value::String("name".into()))?
                        .as_str()?
                        .to_string();
                    let affiliation = m
                        .get(serde_yaml::Value::String("affiliation".into()))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let email = m
                        .get(serde_yaml::Value::String("email".into()))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let orcid = m
                        .get(serde_yaml::Value::String("orcid".into()))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    Some(Author {
                        name,
                        affiliation,
                        email,
                        orcid,
                    })
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Parse a YAML value that should be a list of strings.
fn parse_string_list(val: &serde_yaml::Value) -> Vec<String> {
    match val {
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        serde_yaml::Value::String(s) => s.split(',').map(|s| s.trim().to_string()).collect(),
        _ => Vec::new(),
    }
}

/// Convert a serde_yaml::Value to a string, handling numbers and bools.
fn yaml_value_to_string(val: &serde_yaml::Value) -> Option<String> {
    match val {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Convert a serde_yaml::Value into our MetaValue enum.
fn yaml_to_meta_value(val: &serde_yaml::Value) -> Option<MetaValue> {
    match val {
        serde_yaml::Value::String(s) => Some(MetaValue::String(s.clone())),
        serde_yaml::Value::Bool(b) => Some(MetaValue::Bool(*b)),
        serde_yaml::Value::Number(n) => n.as_f64().map(MetaValue::Number),
        serde_yaml::Value::Sequence(seq) => {
            let items: Vec<MetaValue> = seq.iter().filter_map(yaml_to_meta_value).collect();
            Some(MetaValue::List(items))
        }
        serde_yaml::Value::Mapping(m) => {
            let map: HashMap<String, MetaValue> = m
                .iter()
                .filter_map(|(k, v)| {
                    let key = k.as_str()?.to_string();
                    let val = yaml_to_meta_value(v)?;
                    Some((key, val))
                })
                .collect();
            Some(MetaValue::Map(map))
        }
        _ => None,
    }
}

/// Find the closing quote in a string, handling escape sequences.
fn find_closing_quote(s: &str) -> usize {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 1; // skip opening quote
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            i += 2;
            continue;
        }
        if chars[i] == '"' {
            return i;
        }
        i += 1;
    }
    s.len() - 1
}

/// Trim a leading space from the first Text inline.
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
                assert!(matches!(&content[0], Inline::Code { value, .. } if value == "some code"));
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
                ..
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
                ..
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
                assert!(
                    matches!(&items[0].term[0], Inline::Text { value } if value.trim() == "Term")
                );
                assert_eq!(items[0].definitions.len(), 1);
            }
            other => panic!("Expected DefinitionList, got {:?}", other),
        }
    }

    // ── Task 6: Silently ignored and unknown functions ──────────────────

    #[test]
    fn parse_silently_ignored_functions() {
        let doc = parse("#set text(font: \"Arial\")\n\nHello");
        assert!(doc.warnings.is_empty());
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

    // ── Task 6: Inline function calls ───────────────────────────────────

    #[test]
    fn parse_emph_func() {
        let doc = parse("#emph[hello]");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(matches!(&content[0], Inline::Emphasis { .. }));
                if let Inline::Emphasis { content: inner } = &content[0] {
                    assert!(matches!(&inner[0], Inline::Text { value } if value == "hello"));
                }
            }
            other => panic!("Expected Paragraph with Emphasis, got {:?}", other),
        }
    }

    #[test]
    fn parse_strong_func() {
        let doc = parse("#strong[world]");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(matches!(&content[0], Inline::Strong { .. }));
                if let Inline::Strong { content: inner } = &content[0] {
                    assert!(matches!(&inner[0], Inline::Text { value } if value == "world"));
                }
            }
            other => panic!("Expected Paragraph with Strong, got {:?}", other),
        }
    }

    #[test]
    fn parse_strike_func() {
        let doc = parse("#strike[deleted]");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(matches!(&content[0], Inline::Strikethrough { .. }));
                if let Inline::Strikethrough { content: inner } = &content[0] {
                    assert!(matches!(&inner[0], Inline::Text { value } if value == "deleted"));
                }
            }
            other => panic!("Expected Paragraph with Strikethrough, got {:?}", other),
        }
    }

    #[test]
    fn parse_link_func() {
        let doc = parse("#link(\"https://example.com\")[click here]");
        match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::Link { url, content, .. } => {
                    assert_eq!(url, "https://example.com");
                    assert!(matches!(&content[0], Inline::Text { value } if value == "click here"));
                }
                other => panic!("Expected Link, got {:?}", other),
            },
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_image_block() {
        let doc = parse("#image(\"fig.png\", alt: \"A figure\")");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Figure { image, caption, .. } => {
                assert_eq!(image.url, "fig.png");
                assert!(matches!(&image.alt[0], Inline::Text { value } if value == "A figure"));
                assert!(caption.is_none());
            }
            other => panic!("Expected Figure, got {:?}", other),
        }
    }

    #[test]
    fn parse_figure_with_caption() {
        let doc = parse("#figure(image(\"plot.png\"), caption: [My caption])");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Figure { image, caption, .. } => {
                assert_eq!(image.url, "plot.png");
                assert!(caption.is_some());
                let cap = caption.as_ref().unwrap();
                assert!(matches!(&cap[0], Inline::Text { value } if value == "My caption"));
            }
            other => panic!("Expected Figure, got {:?}", other),
        }
    }

    #[test]
    fn parse_quote_block() {
        let doc = parse("#quote(block: true)[To be or not to be]");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::BlockQuote { content } => {
                assert!(!content.is_empty());
                // The quote content should contain a paragraph.
                assert!(matches!(&content[0], Block::Paragraph { .. }));
            }
            other => panic!("Expected BlockQuote, got {:?}", other),
        }
    }

    #[test]
    fn parse_heading_func() {
        let doc = parse("#heading(level: 2)[My Heading]");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Heading { level, content, .. } => {
                assert_eq!(*level, 2);
                assert!(matches!(&content[0], Inline::Text { value } if value == "My Heading"));
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn parse_footnote() {
        let doc = parse("Text#footnote[A note].");
        // Should have paragraph + footnote def.
        let has_ref = doc.content.iter().any(|b| {
            if let Block::Paragraph { content } = b {
                content
                    .iter()
                    .any(|i| matches!(i, Inline::FootnoteRef { .. }))
            } else {
                false
            }
        });
        assert!(has_ref, "Should have FootnoteRef inline");

        let has_def = doc
            .content
            .iter()
            .any(|b| matches!(b, Block::FootnoteDef { .. }));
        assert!(has_def, "Should have FootnoteDef block");
    }

    #[test]
    fn parse_cite_func() {
        let doc = parse("#cite(<smith2020>)");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(
                    matches!(&content[0], Inline::Citation(c) if c.items[0].key == "smith2020")
                );
            }
            other => panic!("Expected Paragraph with Citation, got {:?}", other),
        }
    }

    #[test]
    fn parse_table_block() {
        let doc = parse("#table(columns: 2, [A], [B], [C], [D])");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Table(t) => {
                assert_eq!(t.columns.len(), 2);
                // Should have header (first row) + one body row.
                assert!(t.header.is_some());
                assert_eq!(t.header.as_ref().unwrap().len(), 2);
                assert_eq!(t.rows.len(), 1);
            }
            other => panic!("Expected Table, got {:?}", other),
        }
    }

    #[test]
    fn parse_bibliography_stores_path() {
        let doc = parse("#bibliography(\"refs.bib\")\n\nSome text.");
        assert!(doc.warnings.is_empty());
        // The bibliography path is stored internally; the parser doesn't
        // produce a block for it. Just verify no warnings.
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::Paragraph { .. }));
    }

    #[test]
    fn parse_inline_func_at_block_level() {
        // #emph at block level should be wrapped in a paragraph.
        let doc = parse("#emph[wrapped]");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(matches!(&content[0], Inline::Emphasis { .. }));
            }
            other => panic!("Expected Paragraph wrapping Emphasis, got {:?}", other),
        }
    }

    #[test]
    fn parse_raw_func() {
        let doc = parse("#raw(\"let x = 1\")");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(matches!(&content[0], Inline::Code { value, .. } if value == "let x = 1"));
            }
            other => panic!("Expected Paragraph with Code, got {:?}", other),
        }
    }

    // ── Task 7: Metadata ────────────────────────────────────────────────

    #[test]
    fn parse_yaml_frontmatter_metadata() {
        let input = "---\ntitle: My Document\nauthor: Alice\ndate: 2024-01-15\nkeywords:\n  - rust\n  - typst\n---\n= Introduction\n";
        let doc = parse(input);
        assert_eq!(doc.metadata.title.as_deref(), Some("My Document"));
        assert_eq!(doc.metadata.authors.len(), 1);
        assert_eq!(doc.metadata.authors[0].name, "Alice");
        assert_eq!(doc.metadata.date.as_deref(), Some("2024-01-15"));
        assert_eq!(doc.metadata.keywords.len(), 2);
        assert!(doc.metadata.keywords.contains(&"rust".to_string()));
        assert!(doc.metadata.keywords.contains(&"typst".to_string()));
    }

    #[test]
    fn parse_yaml_frontmatter_authors_list() {
        let input = "---\ntitle: Test\nauthors:\n  - name: Alice\n    affiliation: MIT\n    email: alice@mit.edu\n  - name: Bob\n---\n";
        let doc = parse(input);
        assert_eq!(doc.metadata.authors.len(), 2);
        assert_eq!(doc.metadata.authors[0].name, "Alice");
        assert_eq!(doc.metadata.authors[0].affiliation.as_deref(), Some("MIT"));
        assert_eq!(
            doc.metadata.authors[0].email.as_deref(),
            Some("alice@mit.edu")
        );
        assert_eq!(doc.metadata.authors[1].name, "Bob");
    }

    #[test]
    fn parse_set_document_metadata() {
        let input = "#set document(title: \"My Title\", author: \"Jane\")\n\nHello world.";
        let doc = parse(input);
        assert_eq!(doc.metadata.title.as_deref(), Some("My Title"));
        assert_eq!(doc.metadata.authors.len(), 1);
        assert_eq!(doc.metadata.authors[0].name, "Jane");
    }

    #[test]
    fn parse_yaml_overrides_set_document() {
        let input = "---\ntitle: YAML Title\n---\n#set document(title: \"Set Title\")\n\nContent.";
        let doc = parse(input);
        // YAML should take priority.
        assert_eq!(doc.metadata.title.as_deref(), Some("YAML Title"));
    }

    // ── Task 7: Labels ──────────────────────────────────────────────────

    #[test]
    fn parse_label_on_heading() {
        let doc = parse("= Introduction <intro>\n");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Heading { id, content, .. } => {
                assert_eq!(id.as_deref(), Some("intro"));
                assert!(matches!(&content[0], Inline::Text { value } if value == "Introduction"));
            }
            other => panic!("Expected Heading with id, got {:?}", other),
        }
    }

    #[test]
    fn parse_label_on_math() {
        let doc = parse("$ E = m c^2 $ <eq-einstein>\n");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::MathBlock { content, label } => {
                assert!(content.contains("E = m c^2"));
                assert_eq!(label.as_deref(), Some("eq-einstein"));
            }
            other => panic!("Expected MathBlock with label, got {:?}", other),
        }
    }

    #[test]
    fn parse_label_on_figure() {
        let doc = parse("#figure(image(\"plot.png\"), caption: [A plot]) <fig-plot>\n");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Figure { label, .. } => {
                assert_eq!(label.as_deref(), Some("fig-plot"));
            }
            other => panic!("Expected Figure with label, got {:?}", other),
        }
    }

    #[test]
    fn parse_label_on_code_block() {
        let doc = parse("```rust\nfn main() {}\n``` <code-main>\n");
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::CodeBlock { label, .. } => {
                assert_eq!(label.as_deref(), Some("code-main"));
            }
            other => panic!("Expected CodeBlock with label, got {:?}", other),
        }
    }

    #[test]
    fn parse_sub_super_smallcaps() {
        let doc = parse("#sub[x] #super[2] #smallcaps[Abc]");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                let has_sub = content
                    .iter()
                    .any(|i| matches!(i, Inline::Subscript { .. }));
                let has_sup = content
                    .iter()
                    .any(|i| matches!(i, Inline::Superscript { .. }));
                let has_sc = content
                    .iter()
                    .any(|i| matches!(i, Inline::SmallCaps { .. }));
                assert!(has_sub, "Should have Subscript");
                assert!(has_sup, "Should have Superscript");
                assert!(has_sc, "Should have SmallCaps");
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_let_silently_ignored() {
        let doc = parse("#let x = 5\n\nContent");
        assert!(doc.warnings.is_empty());
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::Paragraph { .. }));
    }

    #[test]
    fn parse_show_silently_ignored() {
        let doc = parse("#show heading: set text(font: \"Arial\")\n\nContent");
        assert!(doc.warnings.is_empty());
    }

    #[test]
    fn parse_pagebreak_silently_ignored() {
        let doc = parse("#pagebreak()\n\nContent");
        assert!(doc.warnings.is_empty());
        assert_eq!(doc.content.len(), 1);
    }

    #[test]
    fn parse_import_silently_ignored() {
        let doc = parse("#import \"utils.typ\"\n\nContent");
        assert!(doc.warnings.is_empty());
    }

    #[test]
    fn parse_link_no_content() {
        let doc = parse("#link(\"https://example.com\")");
        match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::Link { url, content, .. } => {
                    assert_eq!(url, "https://example.com");
                    // Should use URL as display text.
                    assert!(
                        matches!(&content[0], Inline::Text { value } if value == "https://example.com")
                    );
                }
                other => panic!("Expected Link, got {:?}", other),
            },
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_underline_unwraps() {
        let doc = parse("#underline[content]");
        match &doc.content[0] {
            Block::Paragraph { content } => {
                // Underline should unwrap to its content (no AST underline).
                assert!(
                    matches!(&content[0], Inline::Text { value } if value == "content"),
                    "Expected Text, got {:?}",
                    content[0]
                );
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_yaml_frontmatter_custom_fields() {
        let input = "---\ntitle: Test\nlang: en\ncustom_field: hello\n---\n";
        let doc = parse(input);
        assert_eq!(doc.metadata.title.as_deref(), Some("Test"));
        assert!(doc.metadata.custom.contains_key("lang"));
        assert!(doc.metadata.custom.contains_key("custom_field"));
    }
}
