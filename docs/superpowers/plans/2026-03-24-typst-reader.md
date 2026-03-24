# Typst Reader Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `docmux-reader-typst` — a recursive descent Typst parser that converts Typst markup to the docmux AST.

**Architecture:** Hand-written lexer (`lexer.rs`) tokenizes input, parser (`parser.rs`) consumes tokens via recursive descent. Same pattern as `docmux-reader-latex`. YAML frontmatter + `#set document()` for metadata. Unknown constructs → `RawBlock`/`RawInline` with warnings.

**Tech Stack:** Rust, `serde_yaml` for frontmatter, `docmux-ast` / `docmux-core` for types and traits.

**Spec:** `docs/superpowers/specs/2026-03-24-typst-reader-design.md`

---

## Review Fixes (MUST apply during implementation)

The following corrections override the code snippets in the tasks below. Apply these during the corresponding task.

### Fix 1 [BLOCKER — Task 2]: Lexer must track code/math mode

The `tokenize` function must maintain `in_code_block: bool` and `in_math: bool` state flags. When inside triple-backtick regions or between `$` delimiters, suppress special tokenization of `#`, `*`, `_`, `<`, `@`, `/` and accumulate them as plain `Text` tokens. Only `$` (to close math) and `` ` `` (to close code) should be recognized inside their respective modes.

```rust
// Add to tokenize() state:
let mut in_code_block = false;  // between ```...```
let mut in_math = false;         // between $...$

// At start of main loop, before the match:
if in_code_block {
    if c == '`' && i + 2 < len && chars[i+1] == '`' && chars[i+2] == '`' {
        flush_text!();
        in_code_block = false;
        i += 3;
        tokens.push(Token::Backtick { count: 3 });
        continue;
    }
    text_buf.push(c);
    if c == '\n' { line += 1; }
    i += 1;
    continue;
}
if in_math {
    if c == '$' {
        flush_text!();
        in_math = false;
        i += 1;
        tokens.push(Token::Dollar);
        continue;
    }
    text_buf.push(c);
    if c == '\n' { line += 1; }
    i += 1;
    continue;
}
```

When emitting `Token::Backtick { count: 3 }`, set `in_code_block = true`. When emitting `Token::Dollar`, set `in_math = true`.

### Fix 2 [BLOCKER — Task 4]: Replace `matches!` with `match` for guard-scoped patterns

`is_display_math` must use a `match` statement, not `matches!` macro:

```rust
fn is_display_math(&self) -> bool {
    if !matches!(self.tokens.get(self.pos), Some(Token::Dollar)) {
        return false;
    }
    match self.tokens.get(self.pos + 1) {
        Some(Token::Newline) => true,
        Some(Token::Text { value }) if value.starts_with(' ') => true,
        _ => false,
    }
}
```

Same fix for `parse_single_arg` — replace the `while matches!(...)` with:

```rust
loop {
    match self.peek() {
        Some(Token::Newline) => { self.advance(); }
        Some(Token::Text { value }) if value.trim().is_empty() => { self.advance(); }
        _ => break,
    }
}
```

### Fix 3 [Task 2]: Dash at line start only

The `'-'` arm in the lexer must check `at_line_start` for single dashes. Multi-dashes (`--`, `---`) are always emitted:

```rust
'-' => {
    let mut count: u8 = 1;
    i += 1;
    while i < len && chars[i] == '-' { count += 1; i += 1; }
    if count == 1 && !at_line_start(&tokens) {
        // Single dash not at line start = plain text (hyphen)
        text_buf.push('-');
    } else {
        flush_text!();
        tokens.push(Token::Dash { count });
    }
}
```

### Fix 4 [Task 6]: `#bibliography` must store path

Replace the bibliography handler in `parse_func_call_block`:

```rust
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
```

Add `bibliography_path: Option<String>` to the `Parser` struct and use it in `parse()`:

```rust
bibliography: self.bibliography_path.take().map(|_| Bibliography::default()),
```

### Fix 5 [Task 6]: Handle `Token::Label` in `parse_arg_value`

Add a `Token::Label` arm to `parse_arg_value`:

```rust
Some(Token::Label { .. }) => {
    if let Some(Token::Label { name }) = self.advance() {
        ArgValue::Raw(format!("<{name}>"))
    } else {
        ArgValue::Raw(String::new())
    }
}
```

### Fix 6 [Task 1]: Remove dead `Equals` variant

Remove `Equals` from the `Token` enum — it's never produced by any lexer arm.

### Fix 7 [Task 4]: Remove dead display-math path from `parse_math`

`parse_math` should only produce `Inline::MathInline`. Display math is handled at the block level in `parse_block` via `is_display_math` → `parse_display_math_block`. Remove the `has_leading_space`/`has_trailing_space` detection and the placeholder return from `parse_math`.

### Fix 8 [Task 9]: Add Typst → LaTeX golden tests

Add a `golden_typ_to_latex` test function (same pattern as `golden_typ_to_html` but using `LatexWriter` and `.typ.tex` expected files). The spec calls for both HTML and LaTeX expected outputs.

---

## File Structure

```
crates/docmux-reader-typst/
├── Cargo.toml          (MODIFY — add serde_yaml dep)
├── src/
│   ├── lib.rs          (MODIFY — TypstReader struct + Reader trait impl)
│   ├── lexer.rs        (CREATE — tokenizer)
│   └── parser.rs       (CREATE — recursive descent parser)

crates/docmux-cli/
├── Cargo.toml          (MODIFY — add docmux-reader-typst dep)
├── src/main.rs         (MODIFY — register TypstReader in build_registry)
├── tests/cli_smoke.rs  (MODIFY — add Typst smoke tests)
├── tests/golden.rs     (MODIFY — add Typst→HTML golden test function)

tests/fixtures/basic/
├── typst-heading.typ       (CREATE — golden fixture)
├── typst-heading.typ.html  (AUTO-GENERATED on first run)
├── typst-inlines.typ       (CREATE — golden fixture)
├── typst-inlines.typ.html  (AUTO-GENERATED)
├── typst-lists.typ         (CREATE — golden fixture)
├── typst-lists.typ.html    (AUTO-GENERATED)
├── typst-math.typ          (CREATE — golden fixture)
├── typst-math.typ.html     (AUTO-GENERATED)
```

---

### Task 1: Scaffold — Cargo.toml + TypstReader stub

**Files:**
- Modify: `crates/docmux-reader-typst/Cargo.toml`
- Modify: `crates/docmux-reader-typst/src/lib.rs`

- [ ] **Step 1: Add serde_yaml dependency to Cargo.toml**

```toml
[dependencies]
docmux-ast = { workspace = true }
docmux-core = { workspace = true }
serde_yaml = { workspace = true }
```

- [ ] **Step 2: Write TypstReader stub with Reader trait in lib.rs**

```rust
//! # docmux-reader-typst
//!
//! Typst reader for docmux. Parses a practical subset of Typst markup into
//! the docmux AST using a hand-written recursive descent parser.
//!
//! Unrecognized function calls are emitted as `RawBlock`/`RawInline`
//! with warnings accumulated in `Document.warnings`.

pub(crate) mod lexer;
pub(crate) mod parser;

use docmux_ast::Document;
use docmux_core::{Reader, Result};

/// A Typst reader.
#[derive(Debug, Default)]
pub struct TypstReader;

impl TypstReader {
    pub fn new() -> Self {
        Self
    }
}

impl Reader for TypstReader {
    fn format(&self) -> &str {
        "typst"
    }

    fn extensions(&self) -> &[&str] {
        &["typ"]
    }

    fn read(&self, input: &str) -> Result<Document> {
        let tokens = lexer::tokenize(input);
        let doc = parser::Parser::new(tokens, input).parse();
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_ast::Block;

    #[test]
    fn reader_trait_metadata() {
        let reader = TypstReader::new();
        assert_eq!(reader.format(), "typst");
        assert!(reader.extensions().contains(&"typ"));
    }

    #[test]
    fn read_simple_document() {
        let reader = TypstReader::new();
        let doc = reader.read("= Hello\n\nSome text.").unwrap();
        assert_eq!(doc.content.len(), 2);
        assert!(matches!(&doc.content[0], Block::Heading { level: 1, .. }));
    }
}
```

- [ ] **Step 3: Create empty lexer.rs and parser.rs stubs**

`lexer.rs`:
```rust
/// Tokens produced by the Typst lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Text { value: String },
}

/// Tokenize a Typst source string into a flat sequence of tokens.
pub fn tokenize(_input: &str) -> Vec<Token> {
    Vec::new()
}
```

`parser.rs`:
```rust
use docmux_ast::Document;
use crate::lexer::Token;

/// Recursive descent parser for Typst documents.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    raw_input: String,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, raw_input: &str) -> Self {
        Self {
            tokens,
            pos: 0,
            raw_input: raw_input.to_string(),
        }
    }

    pub fn parse(self) -> Document {
        Document::default()
    }
}
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p docmux-reader-typst`
Expected: compiles successfully (tests will fail since parse returns empty doc — that's fine for now)

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-reader-typst/
git commit -m "scaffold: docmux-reader-typst with TypstReader stub and empty lexer/parser"
```

---

### Task 2: Lexer — Core tokens

**Files:**
- Modify: `crates/docmux-reader-typst/src/lexer.rs`

This is the biggest single task. The lexer is a character-by-character scanner that produces all token types. Build it incrementally: each step adds a few token types + their tests.

- [ ] **Step 1: Define the full Token enum**

```rust
/// Tokens produced by the Typst lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// `=`, `==`, `===`, etc. at line start
    Heading { level: u8, line: usize },
    /// Accumulated plain text
    Text { value: String },
    /// `*` — bold delimiter
    Star,
    /// `_` — italic delimiter
    Underscore,
    /// `` ` `` or `` ``` `` — code delimiter with count
    Backtick { count: u8 },
    /// `#name` — function call (identifier after `#`)
    FuncCall { name: String, line: usize },
    /// `(`
    ParenOpen,
    /// `)`
    ParenClose,
    /// `[`
    BracketOpen,
    /// `]`
    BracketClose,
    /// `{`
    BraceOpen,
    /// `}`
    BraceClose,
    /// `$` — math mode toggle
    Dollar,
    /// `-`, `--`, or `---`
    Dash { count: u8 },
    /// `<label-name>`
    Label { name: String },
    /// `@ref-name`
    AtRef { name: String },
    /// YAML frontmatter block `---\n...\n---` at file start
    RawFrontmatter { value: String },
    /// `:`
    Colon,
    /// `,`
    Comma,
    /// `/ ` at line start — definition list term marker
    TermMarker { line: usize },
    /// `// ...` line comment
    Comment { value: String },
    /// `/* ... */` block comment
    BlockComment { value: String },
    /// Two consecutive newlines (paragraph separator)
    BlankLine,
    /// Single newline
    Newline,
    /// `\` (bare backslash — line break or escape prefix)
    Backslash,
    /// `\*`, `\_`, `\#`, etc. (escaped special char)
    Escape { ch: char },
    /// `"..."` string literal
    StringLit { value: String },
    /// `+` at line start — ordered list marker
    Plus { line: usize },
    /// `=` sign (when not a heading — inside arguments)
    Equals,
}
```

- [ ] **Step 2: Write the tokenize function — plain text, newlines, blank lines**

Start with the scanner skeleton that handles:
- Plain text accumulation into `Text` tokens
- `\n` → `Newline` or `BlankLine` (same double-newline detection as LaTeX lexer)
- Line number tracking

```rust
pub fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut text_buf = String::new();
    let mut line: usize = 1;
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    macro_rules! flush_text {
        () => {
            if !text_buf.is_empty() {
                tokens.push(Token::Text {
                    value: std::mem::take(&mut text_buf),
                });
            }
        };
    }

    while i < len {
        let c = chars[i];
        match c {
            '\n' => {
                let mut j = i + 1;
                while j < len && chars[j] != '\n' && chars[j].is_whitespace() {
                    j += 1;
                }
                if j < len && chars[j] == '\n' {
                    flush_text!();
                    line += 1;
                    line += 1;
                    i = j + 1;
                    tokens.push(Token::BlankLine);
                } else {
                    flush_text!();
                    line += 1;
                    i += 1;
                    tokens.push(Token::Newline);
                }
            }
            // ... other cases added in subsequent steps ...
            other => {
                text_buf.push(other);
                i += 1;
            }
        }
    }

    flush_text!();
    tokens
}
```

- [ ] **Step 3: Write tests for plain text, newlines, blank lines**

```rust
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
    fn lex_newline() {
        let tokens = tokenize("line one\nline two");
        assert!(tokens.iter().any(|t| matches!(t, Token::Newline)));
    }

    #[test]
    fn lex_blank_line() {
        let tokens = tokenize("para one\n\npara two");
        assert!(tokens.iter().any(|t| matches!(t, Token::BlankLine)));
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p docmux-reader-typst -- lexer`
Expected: 3 tests pass

- [ ] **Step 5: Add heading tokenization**

In the `match c` block, add heading detection. Headings are `=` at line start followed by space:

```rust
'=' if at_line_start(&tokens) => {
    flush_text!();
    let mut level: u8 = 1;
    i += 1;
    while i < len && chars[i] == '=' {
        level += 1;
        i += 1;
    }
    // Must be followed by space to be a heading
    if i < len && chars[i] == ' ' {
        i += 1; // consume the space
        tokens.push(Token::Heading { level, line });
    } else {
        // Not a heading — push as text
        for _ in 0..level {
            text_buf.push('=');
        }
    }
}
```

Also add a helper function `at_line_start` that checks if the previous token was `Newline`, `BlankLine`, or nothing (start of file):

```rust
fn at_line_start(tokens: &[Token]) -> bool {
    matches!(
        tokens.last(),
        None | Some(Token::Newline) | Some(Token::BlankLine)
    )
}
```

- [ ] **Step 6: Add heading tests**

```rust
#[test]
fn lex_heading_level1() {
    let tokens = tokenize("= Title");
    assert!(matches!(&tokens[0], Token::Heading { level: 1, .. }));
    assert!(matches!(&tokens[1], Token::Text { value } if value == "Title"));
}

#[test]
fn lex_heading_level3() {
    let tokens = tokenize("=== Deep heading");
    assert!(matches!(&tokens[0], Token::Heading { level: 3, .. }));
}

#[test]
fn lex_equals_not_heading() {
    // = not at line start should be text
    let tokens = tokenize("a = b");
    assert!(!tokens.iter().any(|t| matches!(t, Token::Heading { .. })));
}
```

- [ ] **Step 7: Run tests**

Run: `cargo test -p docmux-reader-typst -- lexer`
Expected: 6 tests pass

- [ ] **Step 8: Add delimiters, brackets, punctuation**

Add to the `match c` block: `*`, `_`, `` ` ``, `(`, `)`, `[`, `]`, `{`, `}`, `$`, `:`, `,`.

```rust
'*' => { flush_text!(); i += 1; tokens.push(Token::Star); }
'_' => { flush_text!(); i += 1; tokens.push(Token::Underscore); }
'`' => {
    flush_text!();
    let mut count: u8 = 1;
    i += 1;
    while i < len && chars[i] == '`' { count += 1; i += 1; }
    tokens.push(Token::Backtick { count });
}
'(' => { flush_text!(); i += 1; tokens.push(Token::ParenOpen); }
')' => { flush_text!(); i += 1; tokens.push(Token::ParenClose); }
'[' => { flush_text!(); i += 1; tokens.push(Token::BracketOpen); }
']' => { flush_text!(); i += 1; tokens.push(Token::BracketClose); }
'{' => { flush_text!(); i += 1; tokens.push(Token::BraceOpen); }
'}' => { flush_text!(); i += 1; tokens.push(Token::BraceClose); }
'$' => { flush_text!(); i += 1; tokens.push(Token::Dollar); }
':' => { flush_text!(); i += 1; tokens.push(Token::Colon); }
',' => { flush_text!(); i += 1; tokens.push(Token::Comma); }
```

- [ ] **Step 9: Add tests for delimiters**

```rust
#[test]
fn lex_star() {
    let tokens = tokenize("*bold*");
    assert_eq!(tokens.len(), 3); // Star, Text, Star
    assert!(matches!(&tokens[0], Token::Star));
}

#[test]
fn lex_underscore() {
    let tokens = tokenize("_italic_");
    assert!(matches!(&tokens[0], Token::Underscore));
}

#[test]
fn lex_backtick_single() {
    let tokens = tokenize("`code`");
    assert!(matches!(&tokens[0], Token::Backtick { count: 1 }));
}

#[test]
fn lex_backtick_triple() {
    let tokens = tokenize("```rust");
    assert!(matches!(&tokens[0], Token::Backtick { count: 3 }));
}

#[test]
fn lex_dollar() {
    let tokens = tokenize("$x^2$");
    assert!(matches!(&tokens[0], Token::Dollar));
    assert!(matches!(&tokens[2], Token::Dollar));
}

#[test]
fn lex_brackets_and_parens() {
    let tokens = tokenize("([{}])");
    assert!(matches!(&tokens[0], Token::ParenOpen));
    assert!(matches!(&tokens[1], Token::BracketOpen));
    assert!(matches!(&tokens[2], Token::BraceOpen));
    assert!(matches!(&tokens[3], Token::BraceClose));
    assert!(matches!(&tokens[4], Token::BracketClose));
    assert!(matches!(&tokens[5], Token::ParenClose));
}
```

- [ ] **Step 10: Run tests**

Run: `cargo test -p docmux-reader-typst -- lexer`
Expected: 12 tests pass

- [ ] **Step 11: Add `#` function calls, `@` references, `<>` labels**

```rust
'#' => {
    flush_text!();
    i += 1;
    if i < len && chars[i].is_ascii_alphabetic() {
        let mut name = String::new();
        while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '-') {
            name.push(chars[i]);
            i += 1;
        }
        tokens.push(Token::FuncCall { name, line });
    } else {
        text_buf.push('#');
    }
}
'@' => {
    flush_text!();
    i += 1;
    let mut name = String::new();
    while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '-' || chars[i] == '.' || chars[i] == '_') {
        name.push(chars[i]);
        i += 1;
    }
    if name.is_empty() {
        text_buf.push('@');
    } else {
        tokens.push(Token::AtRef { name });
    }
}
'<' => {
    // Label: <name> — only valid identifier chars
    let start = i;
    i += 1;
    let mut name = String::new();
    while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '-' || chars[i] == '_') {
        name.push(chars[i]);
        i += 1;
    }
    if i < len && chars[i] == '>' && !name.is_empty() {
        flush_text!();
        i += 1; // consume >
        tokens.push(Token::Label { name });
    } else {
        // Not a label — rewind and treat as text
        i = start;
        text_buf.push(chars[i]);
        i += 1;
    }
}
```

- [ ] **Step 12: Add tests for function calls, refs, labels**

```rust
#[test]
fn lex_func_call() {
    let tokens = tokenize("#image");
    assert!(matches!(&tokens[0], Token::FuncCall { name, .. } if name == "image"));
}

#[test]
fn lex_func_call_with_args() {
    let tokens = tokenize("#link(\"url\")");
    assert!(matches!(&tokens[0], Token::FuncCall { name, .. } if name == "link"));
    assert!(matches!(&tokens[1], Token::ParenOpen));
}

#[test]
fn lex_at_ref() {
    let tokens = tokenize("see @fig-results");
    assert!(tokens.iter().any(|t| matches!(t, Token::AtRef { name } if name == "fig-results")));
}

#[test]
fn lex_label() {
    let tokens = tokenize("<my-label>");
    assert!(matches!(&tokens[0], Token::Label { name } if name == "my-label"));
}

#[test]
fn lex_not_label_in_text() {
    // "a < b > c" should not produce a label
    let tokens = tokenize("a < b > c");
    assert!(!tokens.iter().any(|t| matches!(t, Token::Label { .. })));
}
```

- [ ] **Step 13: Run tests**

Run: `cargo test -p docmux-reader-typst -- lexer`
Expected: 17 tests pass

- [ ] **Step 14: Add comments, escapes, backslash, dashes, string literals, term marker, plus, frontmatter**

Handle remaining token types:

```rust
'/' if i + 1 < len && chars[i + 1] == '/' => {
    flush_text!();
    i += 2;
    let mut comment = String::new();
    while i < len && chars[i] != '\n' {
        comment.push(chars[i]);
        i += 1;
    }
    tokens.push(Token::Comment { value: comment });
}
'/' if i + 1 < len && chars[i + 1] == '*' => {
    flush_text!();
    i += 2;
    let mut comment = String::new();
    while i < len {
        if chars[i] == '*' && i + 1 < len && chars[i + 1] == '/' {
            i += 2;
            break;
        }
        if chars[i] == '\n' { line += 1; }
        comment.push(chars[i]);
        i += 1;
    }
    tokens.push(Token::BlockComment { value: comment });
}
'/' if at_line_start(&tokens) && i + 1 < len && chars[i + 1] == ' ' => {
    flush_text!();
    i += 2; // consume "/ "
    tokens.push(Token::TermMarker { line });
}
'\\' => {
    flush_text!();
    i += 1;
    if i < len && is_special_char(chars[i]) {
        tokens.push(Token::Escape { ch: chars[i] });
        i += 1;
    } else {
        tokens.push(Token::Backslash);
    }
}
'-' => {
    flush_text!();
    let mut count: u8 = 1;
    i += 1;
    while i < len && chars[i] == '-' { count += 1; i += 1; }
    tokens.push(Token::Dash { count });
}
'+' if at_line_start(&tokens) => {
    flush_text!();
    i += 1;
    tokens.push(Token::Plus { line });
}
'"' => {
    flush_text!();
    i += 1;
    let mut value = String::new();
    while i < len && chars[i] != '"' {
        if chars[i] == '\\' && i + 1 < len { value.push(chars[i + 1]); i += 2; continue; }
        if chars[i] == '\n' { line += 1; }
        value.push(chars[i]);
        i += 1;
    }
    if i < len { i += 1; } // consume closing "
    tokens.push(Token::StringLit { value });
}
```

Also add the YAML frontmatter detection at the very start of `tokenize`, before the main loop:

```rust
// Check for YAML frontmatter at file start: ---\n...\n---
if input.starts_with("---\n") || input.starts_with("---\r\n") {
    let after_first = if input.starts_with("---\r\n") { 5 } else { 4 };
    if let Some(end_pos) = input[after_first..].find("\n---") {
        let yaml_content = &input[after_first..after_first + end_pos];
        let skip_to = after_first + end_pos + 4; // skip \n---
        // Skip optional trailing newline after closing ---
        let skip_to = if skip_to < input.len() && input.as_bytes()[skip_to] == b'\n' {
            skip_to + 1
        } else {
            skip_to
        };
        tokens.push(Token::RawFrontmatter { value: yaml_content.to_string() });
        // Update line count and scanner position
        line += yaml_content.chars().filter(|&c| c == '\n').count() + 2;
        i = skip_to;
    }
}
```

Add helper:
```rust
fn is_special_char(c: char) -> bool {
    matches!(c, '*' | '_' | '#' | '$' | '@' | '<' | '\\' | '`' | '/' | '[' | ']')
}
```

- [ ] **Step 15: Add tests for remaining tokens**

```rust
#[test]
fn lex_line_comment() {
    let tokens = tokenize("text // comment\nnext");
    assert!(tokens.iter().any(|t| matches!(t, Token::Comment { value } if value == " comment")));
}

#[test]
fn lex_block_comment() {
    let tokens = tokenize("/* multi\nline */");
    assert!(tokens.iter().any(|t| matches!(t, Token::BlockComment { .. })));
}

#[test]
fn lex_escape() {
    let tokens = tokenize("\\*not bold\\*");
    assert!(matches!(&tokens[0], Token::Escape { ch: '*' }));
}

#[test]
fn lex_backslash_line_break() {
    let tokens = tokenize("line\\\nnext");
    assert!(tokens.iter().any(|t| matches!(t, Token::Backslash)));
}

#[test]
fn lex_dashes() {
    let tokens = tokenize("---");
    assert!(matches!(&tokens[0], Token::Dash { count: 3 }));
}

#[test]
fn lex_string_literal() {
    let tokens = tokenize("\"hello world\"");
    assert!(matches!(&tokens[0], Token::StringLit { value } if value == "hello world"));
}

#[test]
fn lex_term_marker() {
    let tokens = tokenize("/ Term");
    assert!(matches!(&tokens[0], Token::TermMarker { .. }));
}

#[test]
fn lex_plus_ordered_list() {
    let tokens = tokenize("+ item");
    assert!(matches!(&tokens[0], Token::Plus { .. }));
}

#[test]
fn lex_yaml_frontmatter() {
    let tokens = tokenize("---\ntitle: Hello\n---\n= Body");
    assert!(matches!(&tokens[0], Token::RawFrontmatter { value } if value.contains("title")));
}
```

- [ ] **Step 16: Run all lexer tests**

Run: `cargo test -p docmux-reader-typst -- lexer`
Expected: ~26 tests pass

- [ ] **Step 17: Commit**

```bash
git add crates/docmux-reader-typst/src/lexer.rs
git commit -m "feat(typst-reader): implement lexer with all token types"
```

---

### Task 3: Parser — Block-level basics (headings, paragraphs, thematic breaks)

**Files:**
- Modify: `crates/docmux-reader-typst/src/parser.rs`

- [ ] **Step 1: Write failing tests for headings and paragraphs**

Add tests at the bottom of `parser.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use docmux_ast::{Block, Inline};

    fn parse(input: &str) -> Document {
        let tokens = tokenize(input);
        Parser::new(tokens, input).parse()
    }

    #[test]
    fn parse_heading_level1() {
        let doc = parse("= Hello World");
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::Heading { level: 1, .. }));
    }

    #[test]
    fn parse_heading_level3() {
        let doc = parse("=== Deep Section");
        assert!(matches!(&doc.content[0], Block::Heading { level: 3, .. }));
    }

    #[test]
    fn parse_paragraph() {
        let doc = parse("Hello world.\n\nSecond paragraph.");
        assert_eq!(doc.content.len(), 2);
        assert!(matches!(&doc.content[0], Block::Paragraph { .. }));
        assert!(matches!(&doc.content[1], Block::Paragraph { .. }));
    }

    #[test]
    fn parse_heading_then_paragraph() {
        let doc = parse("= Title\n\nBody text here.");
        assert_eq!(doc.content.len(), 2);
        assert!(matches!(&doc.content[0], Block::Heading { level: 1, .. }));
        assert!(matches!(&doc.content[1], Block::Paragraph { .. }));
    }

    #[test]
    fn parse_thematic_break() {
        let doc = parse("Above\n\n---\n\nBelow");
        assert_eq!(doc.content.len(), 3);
        assert!(matches!(&doc.content[1], Block::ThematicBreak));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-typst -- parser::tests`
Expected: FAIL — parser returns empty Document

- [ ] **Step 3: Implement parser core — navigation helpers + block parsing**

Replace the parser stub with the full structure:

```rust
use docmux_ast::*;
use crate::lexer::Token;
use std::collections::HashMap;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    warnings: Vec<ParseWarning>,
    raw_input: String,
    footnote_defs: Vec<Block>,
    footnote_counter: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, raw_input: &str) -> Self {
        Self {
            tokens,
            pos: 0,
            warnings: Vec::new(),
            raw_input: raw_input.to_string(),
            footnote_defs: Vec::new(),
            footnote_counter: 0,
        }
    }

    // ── Navigation ──

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(tok)
        } else {
            None
        }
    }

    fn skip_blank_lines(&mut self) {
        while matches!(self.peek(), Some(Token::BlankLine | Token::Newline)) {
            self.advance();
        }
    }

    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Some(Token::Newline)) {
            self.advance();
        }
    }

    // ── Entry point ──

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

    fn parse_metadata(&mut self) -> Metadata {
        // Handle YAML frontmatter if present
        if matches!(self.peek(), Some(Token::RawFrontmatter { .. })) {
            if let Some(Token::RawFrontmatter { value }) = self.advance() {
                return self.parse_yaml_frontmatter(&value);
            }
        }
        Metadata::default()
    }

    fn parse_yaml_frontmatter(&self, yaml: &str) -> Metadata {
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
                "title" => metadata.title = val.as_str().map(String::from),
                "date" => metadata.date = val.as_str().map(String::from),
                "abstract" | "abstract_text" | "description" => {
                    metadata.abstract_text = val.as_str().map(String::from);
                }
                "keywords" | "tags" => {
                    if let Some(seq) = val.as_sequence() {
                        metadata.keywords = seq.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                    }
                }
                "author" | "authors" => {
                    metadata.authors = self.parse_yaml_authors(val);
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

    fn parse_yaml_authors(&self, val: &serde_yaml::Value) -> Vec<Author> {
        match val {
            serde_yaml::Value::String(s) => vec![Author {
                name: s.clone(),
                affiliation: None,
                email: None,
                orcid: None,
            }],
            serde_yaml::Value::Sequence(seq) => seq.iter().map(|v| match v {
                serde_yaml::Value::String(s) => Author {
                    name: s.clone(),
                    affiliation: None,
                    email: None,
                    orcid: None,
                },
                serde_yaml::Value::Mapping(m) => Author {
                    name: m.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    affiliation: m.get("affiliation").and_then(|v| v.as_str()).map(String::from),
                    email: m.get("email").and_then(|v| v.as_str()).map(String::from),
                    orcid: m.get("orcid").and_then(|v| v.as_str()).map(String::from),
                },
                _ => Author { name: String::new(), affiliation: None, email: None, orcid: None },
            }).collect(),
            _ => Vec::new(),
        }
    }

    // ── Block parsing ──

    fn parse_body(&mut self) -> Vec<Block> {
        let mut blocks = Vec::new();
        self.skip_blank_lines();

        while self.peek().is_some() {
            // Skip comments
            if matches!(self.peek(), Some(Token::Comment { .. } | Token::BlockComment { .. })) {
                self.advance();
                self.skip_newlines();
                continue;
            }

            if let Some(block) = self.parse_block() {
                blocks.push(block);
            }
            self.skip_blank_lines();
        }

        blocks
    }

    fn parse_block(&mut self) -> Option<Block> {
        match self.peek()? {
            Token::Heading { .. } => self.parse_heading(),
            Token::Dash { count } if *count >= 3 => {
                self.advance();
                Some(Block::ThematicBreak)
            }
            Token::Dash { count: 1 } => self.parse_unordered_list(),
            Token::Plus { .. } => self.parse_ordered_list(),
            Token::TermMarker { .. } => self.parse_definition_list(),
            Token::Backtick { count } if *count >= 3 => self.parse_code_block(),
            Token::FuncCall { .. } => self.parse_func_call_block(),
            _ => self.parse_paragraph(),
        }
    }

    fn parse_heading(&mut self) -> Option<Block> {
        if let Some(Token::Heading { level, .. }) = self.advance() {
            let content = self.collect_inline_until_newline();
            // Check for trailing label
            let (content, id) = self.extract_trailing_label(content);
            Some(Block::Heading { level, id, content })
        } else {
            None
        }
    }

    fn parse_paragraph(&mut self) -> Option<Block> {
        let content = self.collect_inline_until_blank();
        if content.is_empty() {
            self.advance(); // skip unexpected token
            return None;
        }
        Some(Block::Paragraph { content })
    }
}

fn yaml_to_meta_value(val: &serde_yaml::Value) -> Option<MetaValue> {
    match val {
        serde_yaml::Value::String(s) => Some(MetaValue::String(s.clone())),
        serde_yaml::Value::Bool(b) => Some(MetaValue::Bool(*b)),
        serde_yaml::Value::Number(n) => n.as_f64().map(MetaValue::Number),
        serde_yaml::Value::Sequence(seq) => {
            Some(MetaValue::List(seq.iter().filter_map(yaml_to_meta_value).collect()))
        }
        serde_yaml::Value::Mapping(m) => {
            let map: HashMap<String, MetaValue> = m.iter()
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
```

Note: `collect_inline_until_newline`, `collect_inline_until_blank`, `extract_trailing_label`, `parse_unordered_list`, `parse_ordered_list`, `parse_definition_list`, `parse_code_block`, and `parse_func_call_block` are stub methods that will be implemented in subsequent tasks. For now, add minimal stubs so the tests pass:

```rust
fn collect_inline_until_newline(&mut self) -> Vec<Inline> {
    let mut inlines = Vec::new();
    while let Some(tok) = self.peek() {
        match tok {
            Token::Newline | Token::BlankLine => break,
            _ => {
                if let Some(inline) = self.parse_inline() {
                    inlines.push(inline);
                }
            }
        }
    }
    inlines
}

fn collect_inline_until_blank(&mut self) -> Vec<Inline> {
    let mut inlines = Vec::new();
    while let Some(tok) = self.peek() {
        match tok {
            Token::BlankLine => break,
            Token::Heading { .. } => break,
            Token::Dash { .. } | Token::Plus { .. } | Token::TermMarker { .. } => break,
            Token::Backtick { count } if *count >= 3 => break,
            Token::FuncCall { .. } => {
                // Check if this is a block-level function call
                // For now, break on known block-level functions
                if self.is_block_func_call() { break; }
                if let Some(inline) = self.parse_inline() {
                    inlines.push(inline);
                }
            }
            Token::Newline => {
                self.advance();
                // Single newline = soft break
                if !matches!(self.peek(), Some(Token::BlankLine)) {
                    inlines.push(Inline::SoftBreak);
                }
            }
            _ => {
                if let Some(inline) = self.parse_inline() {
                    inlines.push(inline);
                }
            }
        }
    }
    inlines
}

fn is_block_func_call(&self) -> bool {
    if let Some(Token::FuncCall { name, .. }) = self.peek() {
        matches!(name.as_str(), "image" | "table" | "figure" | "quote" | "bibliography"
            | "set" | "show" | "let" | "import" | "include" | "pagebreak" | "colbreak"
            | "heading")
    } else {
        false
    }
}

fn extract_trailing_label(&self, mut content: Vec<Inline>) -> (Vec<Inline>, Option<String>) {
    // Labels are handled during inline parsing — check if last element is a label
    (content, None)
}

fn parse_inline(&mut self) -> Option<Inline> {
    match self.peek()? {
        Token::Text { .. } => {
            if let Some(Token::Text { value }) = self.advance() {
                Some(Inline::Text { value })
            } else {
                None
            }
        }
        Token::Escape { .. } => {
            if let Some(Token::Escape { ch }) = self.advance() {
                Some(Inline::Text { value: ch.to_string() })
            } else {
                None
            }
        }
        _ => {
            // For now, convert unknown tokens to text
            self.advance();
            None
        }
    }
}

// Stubs for list/code/func parsing — will be implemented in Tasks 4-6
fn parse_unordered_list(&mut self) -> Option<Block> { self.advance(); None }
fn parse_ordered_list(&mut self) -> Option<Block> { self.advance(); None }
fn parse_definition_list(&mut self) -> Option<Block> { self.advance(); None }
fn parse_code_block(&mut self) -> Option<Block> { self.advance(); None }
fn parse_func_call_block(&mut self) -> Option<Block> { self.advance(); None }
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p docmux-reader-typst -- parser::tests`
Expected: 5 tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-reader-typst/src/parser.rs
git commit -m "feat(typst-reader): parser core with headings, paragraphs, thematic breaks"
```

---

### Task 4: Parser — Inline formatting (emphasis, strong, code, math, breaks)

**Files:**
- Modify: `crates/docmux-reader-typst/src/parser.rs`

- [ ] **Step 1: Write failing tests for inline formatting**

```rust
#[test]
fn parse_bold() {
    let doc = parse("*bold text*");
    if let Block::Paragraph { content } = &doc.content[0] {
        assert!(matches!(&content[0], Inline::Strong { .. }));
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn parse_italic() {
    let doc = parse("_italic text_");
    if let Block::Paragraph { content } = &doc.content[0] {
        assert!(matches!(&content[0], Inline::Emphasis { .. }));
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn parse_inline_code() {
    let doc = parse("`some code`");
    if let Block::Paragraph { content } = &doc.content[0] {
        assert!(matches!(&content[0], Inline::Code { .. }));
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn parse_inline_math() {
    let doc = parse("$x^2 + y^2$");
    if let Block::Paragraph { content } = &doc.content[0] {
        assert!(matches!(&content[0], Inline::MathInline { .. }));
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn parse_display_math() {
    let doc = parse("$ E = mc^2 $");
    assert!(matches!(&doc.content[0], Block::MathBlock { .. }));
}

#[test]
fn parse_display_math_multiline() {
    let doc = parse("$\n  x^2 + y^2 = z^2\n$");
    assert!(matches!(&doc.content[0], Block::MathBlock { .. }));
}

#[test]
fn parse_hard_break() {
    let doc = parse("line one\\\nline two");
    if let Block::Paragraph { content } = &doc.content[0] {
        assert!(content.iter().any(|i| matches!(i, Inline::HardBreak)));
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn parse_soft_break() {
    let doc = parse("line one\nline two");
    if let Block::Paragraph { content } = &doc.content[0] {
        assert!(content.iter().any(|i| matches!(i, Inline::SoftBreak)));
    } else {
        panic!("expected paragraph");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-typst -- parser::tests`
Expected: new tests FAIL

- [ ] **Step 3: Implement inline parsing — emphasis, strong, code, math**

Expand `parse_inline` to handle `Star`, `Underscore`, `Backtick`, `Dollar`, `Backslash`:

```rust
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
            let content = self.collect_inline_until_token(&Token::Star);
            self.try_consume(&Token::Star); // consume closing *
            Some(Inline::Strong { content })
        }
        Token::Underscore => {
            self.advance();
            let content = self.collect_inline_until_token(&Token::Underscore);
            self.try_consume(&Token::Underscore);
            Some(Inline::Emphasis { content })
        }
        Token::Backtick { count: 1 } => {
            self.advance();
            let value = self.collect_text_until_backtick(1);
            Some(Inline::Code { value })
        }
        Token::Dollar => self.parse_math(),
        Token::Backslash => {
            self.advance();
            if matches!(self.peek(), Some(Token::Newline)) {
                self.advance();
                Some(Inline::HardBreak)
            } else {
                Some(Inline::Text { value: "\\".to_string() })
            }
        }
        Token::Escape { .. } => {
            if let Some(Token::Escape { ch }) = self.advance() {
                Some(Inline::Text { value: ch.to_string() })
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
        _ => {
            self.advance();
            None
        }
    }
}

fn parse_math(&mut self) -> Option<Inline> {
    self.advance(); // consume opening $

    // Collect everything until closing $
    let mut content = String::new();
    let mut has_leading_space = false;

    // Check for leading whitespace (display math indicator)
    if let Some(tok) = self.peek() {
        match tok {
            Token::Newline => has_leading_space = true,
            Token::Text { value } if value.starts_with(' ') => has_leading_space = true,
            _ => {}
        }
    }

    while let Some(tok) = self.peek() {
        if matches!(tok, Token::Dollar) {
            break;
        }
        match self.advance() {
            Some(Token::Text { value }) => content.push_str(&value),
            Some(Token::Newline) => content.push('\n'),
            Some(Token::BlankLine) => content.push_str("\n\n"),
            Some(other) => content.push_str(&token_to_text(&other)),
            None => break,
        }
    }
    self.try_consume_dollar(); // consume closing $

    let has_trailing_space = content.ends_with(' ') || content.ends_with('\n');
    let content = content.trim().to_string();

    if has_leading_space && has_trailing_space {
        Some(Inline::MathInline { value: String::new() }) // placeholder — will return MathBlock
        // Actually, display math needs to be a Block, not Inline.
        // We handle this by returning a sentinel and promoting in collect_inline_until_blank
    } else {
        Some(Inline::MathInline { value: content })
    }
}
```

Note: Display math detection in inline context is tricky — when the parser encounters `$ content $` with spaces, it needs to produce a `Block::MathBlock` but is inside inline parsing context. The approach: detect display math in `parse_block` by checking if the first token is `Dollar`. Add a dedicated method:

```rust
// In parse_block, add before the default paragraph case:
Token::Dollar => {
    if self.is_display_math() {
        self.parse_display_math_block()
    } else {
        self.parse_paragraph()
    }
}
```

```rust
fn is_display_math(&self) -> bool {
    // Display math: $ followed by space/newline
    if !matches!(self.tokens.get(self.pos), Some(Token::Dollar)) {
        return false;
    }
    matches!(self.tokens.get(self.pos + 1),
        Some(Token::Newline) | Some(Token::Text { value }) if value.starts_with(' '))
}

fn parse_display_math_block(&mut self) -> Option<Block> {
    self.advance(); // consume opening $
    let mut content = String::new();
    while let Some(tok) = self.peek() {
        if matches!(tok, Token::Dollar) { break; }
        match self.advance() {
            Some(Token::Text { value }) => content.push_str(&value),
            Some(Token::Newline) => content.push('\n'),
            Some(other) => content.push_str(&token_to_text(&other)),
            None => break,
        }
    }
    self.try_consume_dollar();
    Some(Block::MathBlock {
        content: content.trim().to_string(),
        label: None,
    })
}
```

Add helpers:
```rust
fn collect_inline_until_token(&mut self, end: &Token) -> Vec<Inline> {
    let mut inlines = Vec::new();
    while let Some(tok) = self.peek() {
        if std::mem::discriminant(tok) == std::mem::discriminant(end) { break; }
        if matches!(tok, Token::Newline | Token::BlankLine) { break; }
        if let Some(inline) = self.parse_inline() {
            inlines.push(inline);
        }
    }
    inlines
}

fn try_consume(&mut self, expected: &Token) -> bool {
    if self.peek().map(|t| std::mem::discriminant(t) == std::mem::discriminant(expected)).unwrap_or(false) {
        self.advance();
        true
    } else {
        false
    }
}

fn try_consume_dollar(&mut self) -> bool {
    if matches!(self.peek(), Some(Token::Dollar)) {
        self.advance();
        true
    } else {
        false
    }
}

fn collect_text_until_backtick(&mut self, count: u8) -> String {
    let mut value = String::new();
    while let Some(tok) = self.peek() {
        if matches!(tok, Token::Backtick { count: c } if *c == count) {
            self.advance(); // consume closing backtick
            break;
        }
        match self.advance() {
            Some(Token::Text { value: v }) => value.push_str(&v),
            Some(other) => value.push_str(&token_to_text(&other)),
            None => break,
        }
    }
    value
}

fn token_to_text(tok: &Token) -> String {
    match tok {
        Token::Star => "*".to_string(),
        Token::Underscore => "_".to_string(),
        Token::Colon => ":".to_string(),
        Token::Comma => ",".to_string(),
        Token::ParenOpen => "(".to_string(),
        Token::ParenClose => ")".to_string(),
        Token::BracketOpen => "[".to_string(),
        Token::BracketClose => "]".to_string(),
        Token::BraceOpen => "{".to_string(),
        Token::BraceClose => "}".to_string(),
        Token::Backslash => "\\".to_string(),
        Token::Dash { count } => "-".repeat(*count as usize),
        Token::Equals => "=".to_string(),
        _ => String::new(),
    }
}

// Stub for Task 5
fn parse_inline_func_call(&mut self) -> Option<Inline> {
    self.advance(); // consume FuncCall token
    None
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p docmux-reader-typst -- parser::tests`
Expected: all tests pass (both old and new)

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-reader-typst/src/parser.rs
git commit -m "feat(typst-reader): inline parsing — emphasis, strong, code, math, breaks"
```

---

### Task 5: Parser — Lists (unordered, ordered, definition)

**Files:**
- Modify: `crates/docmux-reader-typst/src/parser.rs`

- [ ] **Step 1: Write failing tests for lists**

```rust
#[test]
fn parse_unordered_list() {
    let doc = parse("- First\n- Second\n- Third");
    assert_eq!(doc.content.len(), 1);
    if let Block::List { ordered, items, .. } = &doc.content[0] {
        assert!(!ordered);
        assert_eq!(items.len(), 3);
    } else {
        panic!("expected list");
    }
}

#[test]
fn parse_ordered_list() {
    let doc = parse("+ First\n+ Second");
    if let Block::List { ordered, items, .. } = &doc.content[0] {
        assert!(ordered);
        assert_eq!(items.len(), 2);
    } else {
        panic!("expected ordered list");
    }
}

#[test]
fn parse_definition_list() {
    let doc = parse("/ Term One: Definition one\n/ Term Two: Definition two");
    if let Block::DefinitionList { items } = &doc.content[0] {
        assert_eq!(items.len(), 2);
    } else {
        panic!("expected definition list");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-typst -- parser::tests::parse_unordered`
Expected: FAIL

- [ ] **Step 3: Implement list parsing**

Replace the list stubs:

```rust
fn parse_unordered_list(&mut self) -> Option<Block> {
    let mut items = Vec::new();
    while matches!(self.peek(), Some(Token::Dash { count: 1 })) {
        self.advance(); // consume -
        // Skip optional space (it's part of the dash token or next text)
        let content = self.collect_inline_until_newline();
        items.push(ListItem {
            checked: None,
            content: vec![Block::Paragraph { content }],
        });
        self.skip_newlines();
    }
    if items.is_empty() { return None; }
    Some(Block::List { ordered: false, start: None, items })
}

fn parse_ordered_list(&mut self) -> Option<Block> {
    let mut items = Vec::new();
    while matches!(self.peek(), Some(Token::Plus { .. })) {
        self.advance(); // consume +
        let content = self.collect_inline_until_newline();
        items.push(ListItem {
            checked: None,
            content: vec![Block::Paragraph { content }],
        });
        self.skip_newlines();
    }
    if items.is_empty() { return None; }
    Some(Block::List { ordered: true, start: None, items })
}

fn parse_definition_list(&mut self) -> Option<Block> {
    let mut items = Vec::new();
    while matches!(self.peek(), Some(Token::TermMarker { .. })) {
        self.advance(); // consume /

        // Collect term (up to colon)
        let mut term = Vec::new();
        while let Some(tok) = self.peek() {
            if matches!(tok, Token::Colon) { self.advance(); break; }
            if matches!(tok, Token::Newline | Token::BlankLine) { break; }
            if let Some(inline) = self.parse_inline() {
                term.push(inline);
            }
        }

        // Collect definition (rest of line)
        let def_content = self.collect_inline_until_newline();
        items.push(DefinitionItem {
            term,
            definitions: vec![vec![Block::Paragraph { content: def_content }]],
        });
        self.skip_newlines();
    }
    if items.is_empty() { return None; }
    Some(Block::DefinitionList { items })
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p docmux-reader-typst -- parser::tests`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-reader-typst/src/parser.rs
git commit -m "feat(typst-reader): list parsing — unordered, ordered, definition"
```

---

### Task 6: Parser — Code blocks and function calls (#image, #link, #emph, #strong, #strike, #table, etc.)

**Files:**
- Modify: `crates/docmux-reader-typst/src/parser.rs`

- [ ] **Step 1: Write failing tests for code blocks and function calls**

```rust
#[test]
fn parse_code_block() {
    let doc = parse("```rust\nfn main() {}\n```");
    if let Block::CodeBlock { language, content, .. } = &doc.content[0] {
        assert_eq!(language.as_deref(), Some("rust"));
        assert!(content.contains("fn main()"));
    } else {
        panic!("expected code block");
    }
}

#[test]
fn parse_emph_func() {
    let doc = parse("#emph[hello]");
    if let Block::Paragraph { content } = &doc.content[0] {
        assert!(matches!(&content[0], Inline::Emphasis { .. }));
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn parse_strong_func() {
    let doc = parse("#strong[hello]");
    if let Block::Paragraph { content } = &doc.content[0] {
        assert!(matches!(&content[0], Inline::Strong { .. }));
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn parse_strike_func() {
    let doc = parse("#strike[deleted]");
    if let Block::Paragraph { content } = &doc.content[0] {
        assert!(matches!(&content[0], Inline::Strikethrough { .. }));
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn parse_link() {
    let doc = parse("#link(\"https://example.com\")[Example]");
    if let Block::Paragraph { content } = &doc.content[0] {
        assert!(matches!(&content[0], Inline::Link { url, .. } if url == "https://example.com"));
    } else {
        panic!("expected paragraph");
    }
}

#[test]
fn parse_image_block() {
    let doc = parse("#image(\"photo.png\", alt: \"A photo\")");
    assert!(matches!(&doc.content[0], Block::Figure { .. }));
}

#[test]
fn parse_quote_block() {
    let doc = parse("#quote(block: true)[Some quoted text]");
    assert!(matches!(&doc.content[0], Block::BlockQuote { .. }));
}

#[test]
fn parse_silently_ignored_set() {
    let doc = parse("#set text(size: 12pt)\n\nHello");
    // #set should be consumed, only paragraph remains
    assert_eq!(doc.content.len(), 1);
    assert!(matches!(&doc.content[0], Block::Paragraph { .. }));
}

#[test]
fn parse_unknown_func_warning() {
    let doc = parse("#customfunc(arg)");
    assert!(!doc.warnings.is_empty());
}

#[test]
fn parse_heading_func() {
    let doc = parse("#heading(level: 2)[Section Title]");
    assert!(matches!(&doc.content[0], Block::Heading { level: 2, .. }));
}

#[test]
fn parse_footnote() {
    let doc = parse("Text#footnote[A note] more text.");
    // Should produce a paragraph with FootnoteRef + a FootnoteDef at the end
    assert!(doc.content.len() >= 2); // paragraph + footnote def
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-typst -- parser::tests`
Expected: new tests FAIL

- [ ] **Step 3: Implement code block parsing**

```rust
fn parse_code_block(&mut self) -> Option<Block> {
    if let Some(Token::Backtick { count }) = self.advance() {
        if count < 3 { return None; }

        // Language hint (text until newline)
        let language = if let Some(Token::Text { value }) = self.peek() {
            let lang = value.trim().to_string();
            self.advance();
            if lang.is_empty() { None } else { Some(lang) }
        } else {
            None
        };

        // Skip newline after language
        if matches!(self.peek(), Some(Token::Newline)) {
            self.advance();
        }

        // Collect content until closing backticks
        let mut content = String::new();
        while let Some(tok) = self.peek() {
            if matches!(tok, Token::Backtick { count: c } if *c >= 3) {
                self.advance();
                break;
            }
            match self.advance() {
                Some(Token::Text { value }) => content.push_str(&value),
                Some(Token::Newline) => content.push('\n'),
                Some(other) => content.push_str(&token_to_text(&other)),
                None => break,
            }
        }

        // Trim trailing newline from content
        if content.ends_with('\n') {
            content.pop();
        }

        Some(Block::CodeBlock {
            language,
            content,
            caption: None,
            label: None,
        })
    } else {
        None
    }
}
```

- [ ] **Step 4: Implement argument parsing helper**

This is needed for all function calls:

```rust
/// Parsed argument from a Typst function call.
#[derive(Debug, Clone)]
enum Arg {
    Positional(ArgValue),
    Named(String, ArgValue),
}

#[derive(Debug, Clone)]
enum ArgValue {
    String(String),
    Content(Vec<Token>),
    Identifier(String),
    Number(String),
    Bool(bool),
    FuncCall(String, Vec<Arg>),
    Raw(String), // unparsed expression
}

/// Parse arguments inside parentheses: `(arg1, key: val, ...)`
fn parse_args(&mut self) -> Vec<Arg> {
    if !matches!(self.peek(), Some(Token::ParenOpen)) {
        return Vec::new();
    }
    self.advance(); // consume (

    let mut args = Vec::new();
    let mut depth: u32 = 1;

    while depth > 0 && self.peek().is_some() {
        // Skip whitespace/newlines
        while matches!(self.peek(), Some(Token::Newline | Token::BlankLine)) {
            self.advance();
        }

        if matches!(self.peek(), Some(Token::ParenClose)) {
            self.advance();
            depth -= 1;
            break;
        }

        if matches!(self.peek(), Some(Token::Comma)) {
            self.advance();
            continue;
        }

        // Try to parse: identifier ":" value (named arg) or just value (positional)
        let arg = self.parse_single_arg();
        if let Some(a) = arg {
            args.push(a);
        }
    }

    args
}

fn parse_single_arg(&mut self) -> Option<Arg> {
    // Check for named argument: identifier followed by colon
    if let Some(Token::Text { value }) = self.peek() {
        let name = value.trim().to_string();
        if !name.is_empty() {
            // Look ahead for colon
            let saved_pos = self.pos;
            self.advance();
            // Skip whitespace
            while matches!(self.peek(), Some(Token::Newline)) { self.advance(); }
            if matches!(self.peek(), Some(Token::Colon)) {
                self.advance(); // consume :
                // Skip whitespace
                while matches!(self.peek(), Some(Token::Newline | Token::Text { value } if value.trim().is_empty())) {
                    self.advance();
                }
                let val = self.parse_arg_value();
                return Some(Arg::Named(name, val));
            } else {
                // Not a named arg — rewind
                self.pos = saved_pos;
            }
        }
    }

    // Positional argument
    let val = self.parse_arg_value();
    Some(Arg::Positional(val))
}

fn parse_arg_value(&mut self) -> ArgValue {
    match self.peek() {
        Some(Token::StringLit { .. }) => {
            if let Some(Token::StringLit { value }) = self.advance() {
                ArgValue::String(value)
            } else {
                ArgValue::Raw(String::new())
            }
        }
        Some(Token::BracketOpen) => {
            let tokens = self.parse_content_block_tokens();
            ArgValue::Content(tokens)
        }
        Some(Token::FuncCall { .. }) => {
            if let Some(Token::FuncCall { name, .. }) = self.advance() {
                let args = self.parse_args();
                // Also consume trailing content block if present
                ArgValue::FuncCall(name, args)
            } else {
                ArgValue::Raw(String::new())
            }
        }
        Some(Token::Text { value }) => {
            let v = value.trim().to_string();
            self.advance();
            match v.as_str() {
                "true" => ArgValue::Bool(true),
                "false" => ArgValue::Bool(false),
                _ => ArgValue::Raw(v),
            }
        }
        _ => {
            // Consume tokens until comma or close paren
            let mut raw = String::new();
            while let Some(tok) = self.peek() {
                if matches!(tok, Token::Comma | Token::ParenClose) { break; }
                raw.push_str(&token_to_text(tok));
                self.advance();
            }
            ArgValue::Raw(raw)
        }
    }
}

/// Parse a content block `[...]` and return the inner tokens.
fn parse_content_block_tokens(&mut self) -> Vec<Token> {
    if !matches!(self.peek(), Some(Token::BracketOpen)) {
        return Vec::new();
    }
    self.advance(); // consume [

    let mut tokens = Vec::new();
    let mut depth: u32 = 1;

    while depth > 0 {
        match self.advance() {
            Some(Token::BracketOpen) => { depth += 1; tokens.push(Token::BracketOpen); }
            Some(Token::BracketClose) => {
                depth -= 1;
                if depth > 0 { tokens.push(Token::BracketClose); }
            }
            Some(tok) => tokens.push(tok),
            None => break,
        }
    }

    tokens
}

/// Parse a content block `[...]` as inline content.
fn parse_content_block_inlines(&mut self) -> Vec<Inline> {
    let tokens = self.parse_content_block_tokens();
    // Create a sub-parser to parse these tokens
    let mut sub = Parser::new(tokens, "");
    sub.collect_inline_until_blank()
}

/// Parse a content block `[...]` as block content.
fn parse_content_block_blocks(&mut self) -> Vec<Block> {
    let tokens = self.parse_content_block_tokens();
    let mut sub = Parser::new(tokens, "");
    sub.parse_body()
}
```

- [ ] **Step 5: Implement function call dispatch**

```rust
/// Directives that are consumed without output or warning.
const SILENTLY_IGNORED: &[&str] = &[
    "set", "show", "let", "import", "include",
    "pagebreak", "colbreak", "v", "h",
];

fn parse_func_call_block(&mut self) -> Option<Block> {
    let (name, line) = if let Some(Token::FuncCall { name, line }) = self.advance() {
        (name, line)
    } else {
        return None;
    };

    // Silently ignored directives — consume args and optional content block
    if SILENTLY_IGNORED.contains(&name.as_str()) && name != "set" {
        self.consume_func_args_and_content();
        return None;
    }

    // #set — check for #set document(...)
    if name == "set" {
        return self.parse_set_directive();
    }

    match name.as_str() {
        "heading" => self.parse_heading_func(),
        "image" => self.parse_image_func(line),
        "figure" => self.parse_figure_func(line),
        "table" => self.parse_table_func(line),
        "quote" => self.parse_quote_func(),
        "bibliography" => { self.consume_func_args_and_content(); None }
        // Inline functions at block level — wrap in paragraph
        "emph" | "strong" | "strike" | "link" | "sub" | "super" | "smallcaps"
        | "cite" | "footnote" | "raw" | "underline" => {
            // Rewind and parse as paragraph (the inline parser will handle the func)
            self.pos -= 1; // put FuncCall back
            self.parse_paragraph()
        }
        _ => {
            // Unknown function — emit as RawBlock with warning
            let raw = self.collect_func_call_text(&name);
            self.warnings.push(ParseWarning {
                line,
                message: format!("unrecognized function call: #{name}"),
            });
            Some(Block::RawBlock {
                format: "typst".to_string(),
                content: raw,
            })
        }
    }
}

fn parse_inline_func_call(&mut self) -> Option<Inline> {
    let (name, line) = if let Some(Token::FuncCall { name, line }) = self.advance() {
        (name, line)
    } else {
        return None;
    };

    match name.as_str() {
        "emph" => {
            let _args = self.parse_args();
            let content = self.parse_content_block_inlines();
            Some(Inline::Emphasis { content })
        }
        "strong" => {
            let _args = self.parse_args();
            let content = self.parse_content_block_inlines();
            Some(Inline::Strong { content })
        }
        "strike" => {
            let _args = self.parse_args();
            let content = self.parse_content_block_inlines();
            Some(Inline::Strikethrough { content })
        }
        "sub" => {
            let _args = self.parse_args();
            let content = self.parse_content_block_inlines();
            Some(Inline::Subscript { content })
        }
        "super" => {
            let _args = self.parse_args();
            let content = self.parse_content_block_inlines();
            Some(Inline::Superscript { content })
        }
        "smallcaps" => {
            let _args = self.parse_args();
            let content = self.parse_content_block_inlines();
            Some(Inline::SmallCaps { content })
        }
        "link" => {
            let args = self.parse_args();
            let content = if matches!(self.peek(), Some(Token::BracketOpen)) {
                self.parse_content_block_inlines()
            } else {
                Vec::new()
            };
            let url = args.iter().find_map(|a| match a {
                Arg::Positional(ArgValue::String(s)) => Some(s.clone()),
                _ => None,
            }).unwrap_or_default();
            Some(Inline::Link { url, title: None, content })
        }
        "cite" => {
            let args = self.parse_args();
            let key = args.iter().find_map(|a| match a {
                Arg::Positional(ArgValue::Raw(s)) => Some(s.trim_matches('<').trim_matches('>').to_string()),
                _ => None,
            }).unwrap_or_default();
            Some(Inline::Citation(Citation {
                keys: vec![key],
                prefix: None,
                suffix: None,
                mode: CitationMode::Normal,
            }))
        }
        "footnote" => {
            let content_blocks = if matches!(self.peek(), Some(Token::BracketOpen)) {
                self.parse_content_block_blocks()
            } else {
                let _args = self.parse_args();
                Vec::new()
            };
            self.footnote_counter += 1;
            let id = format!("fn-{}", self.footnote_counter);
            self.footnote_defs.push(Block::FootnoteDef {
                id: id.clone(),
                content: content_blocks,
            });
            Some(Inline::FootnoteRef { id })
        }
        "raw" => {
            let args = self.parse_args();
            let value = args.iter().find_map(|a| match a {
                Arg::Positional(ArgValue::String(s)) => Some(s.clone()),
                _ => None,
            }).unwrap_or_default();
            Some(Inline::Code { value })
        }
        "underline" => {
            // No Inline::Underline in AST — silently consume
            let _args = self.parse_args();
            if matches!(self.peek(), Some(Token::BracketOpen)) {
                let content = self.parse_content_block_inlines();
                // Return content without underline wrapping
                if content.len() == 1 {
                    return Some(content.into_iter().next().unwrap());
                }
            }
            None
        }
        _ => {
            // Unknown inline function
            let raw = self.collect_func_call_text(&name);
            self.warnings.push(ParseWarning {
                line,
                message: format!("unrecognized function call: #{name}"),
            });
            Some(Inline::RawInline {
                format: "typst".to_string(),
                content: raw,
            })
        }
    }
}

fn consume_func_args_and_content(&mut self) {
    // Consume parenthesized args if present
    if matches!(self.peek(), Some(Token::ParenOpen)) {
        self.parse_args();
    }
    // Consume content block if present
    if matches!(self.peek(), Some(Token::BracketOpen)) {
        self.parse_content_block_tokens();
    }
}

fn collect_func_call_text(&mut self, name: &str) -> String {
    let mut text = format!("#{name}");
    if matches!(self.peek(), Some(Token::ParenOpen)) {
        text.push('(');
        self.advance();
        let mut depth = 1u32;
        while depth > 0 {
            match self.advance() {
                Some(Token::ParenOpen) => { depth += 1; text.push('('); }
                Some(Token::ParenClose) => { depth -= 1; if depth > 0 { text.push(')'); } }
                Some(ref tok) => text.push_str(&token_to_text(tok)),
                None => break,
            }
        }
        text.push(')');
    }
    if matches!(self.peek(), Some(Token::BracketOpen)) {
        text.push('[');
        let inner = self.parse_content_block_tokens();
        for tok in &inner {
            text.push_str(&token_to_text(tok));
        }
        text.push(']');
    }
    text
}
```

- [ ] **Step 6: Implement block-level function calls (image, table, figure, quote, heading, set)**

```rust
fn parse_set_directive(&mut self) -> Option<Block> {
    // Check for #set document(...)
    if let Some(Token::Text { value }) = self.peek() {
        if value.trim() == "document" || value.trim().starts_with("document") {
            self.advance(); // consume "document"
            // Parse will handle metadata extraction in parse_metadata
            // For now, just consume the args
            self.consume_func_args_and_content();
            return None;
        }
    }
    // Other #set directives — silently ignore
    self.consume_func_args_and_content();
    None
}

fn parse_heading_func(&mut self) -> Option<Block> {
    let args = self.parse_args();
    let content = if matches!(self.peek(), Some(Token::BracketOpen)) {
        self.parse_content_block_inlines()
    } else {
        Vec::new()
    };
    let level = args.iter().find_map(|a| match a {
        Arg::Named(k, ArgValue::Raw(v)) if k == "level" => v.parse::<u8>().ok(),
        _ => None,
    }).unwrap_or(1);
    Some(Block::Heading { level, id: None, content })
}

fn parse_image_func(&mut self, _line: usize) -> Option<Block> {
    let args = self.parse_args();
    let url = args.iter().find_map(|a| match a {
        Arg::Positional(ArgValue::String(s)) => Some(s.clone()),
        _ => None,
    }).unwrap_or_default();
    let alt = args.iter().find_map(|a| match a {
        Arg::Named(k, ArgValue::String(s)) if k == "alt" => Some(s.clone()),
        _ => None,
    }).unwrap_or_default();
    Some(Block::Figure {
        image: Image { url, alt, title: None },
        caption: None,
        label: None,
    })
}

fn parse_figure_func(&mut self, _line: usize) -> Option<Block> {
    let args = self.parse_args();

    // Extract image from positional FuncCall arg
    let image = args.iter().find_map(|a| match a {
        Arg::Positional(ArgValue::FuncCall(name, inner_args)) if name == "image" => {
            let url = inner_args.iter().find_map(|ia| match ia {
                Arg::Positional(ArgValue::String(s)) => Some(s.clone()),
                _ => None,
            }).unwrap_or_default();
            let alt = inner_args.iter().find_map(|ia| match ia {
                Arg::Named(k, ArgValue::String(s)) if k == "alt" => Some(s.clone()),
                _ => None,
            }).unwrap_or_default();
            Some(Image { url, alt, title: None })
        }
        _ => None,
    }).unwrap_or(Image { url: String::new(), alt: String::new(), title: None });

    // Extract caption from named arg
    let caption = args.iter().find_map(|a| match a {
        Arg::Named(k, ArgValue::Content(tokens)) if k == "caption" => {
            let mut sub = Parser::new(tokens.clone(), "");
            let inlines = sub.collect_inline_until_blank();
            if inlines.is_empty() { None } else { Some(inlines) }
        }
        _ => None,
    });

    Some(Block::Figure { image, caption, label: None })
}

fn parse_table_func(&mut self, _line: usize) -> Option<Block> {
    let args = self.parse_args();

    // Extract column count
    let col_count = args.iter().find_map(|a| match a {
        Arg::Named(k, ArgValue::Raw(v)) if k == "columns" => v.trim().parse::<usize>().ok(),
        _ => None,
    }).unwrap_or(1);

    let columns: Vec<ColumnSpec> = (0..col_count)
        .map(|_| ColumnSpec { alignment: Alignment::Default, width: None })
        .collect();

    // Collect positional content args as cells
    let cells: Vec<Vec<Block>> = args.iter().filter_map(|a| match a {
        Arg::Positional(ArgValue::Content(tokens)) => {
            let mut sub = Parser::new(tokens.clone(), "");
            let blocks = sub.parse_body();
            if blocks.is_empty() {
                Some(vec![Block::Paragraph { content: vec![] }])
            } else {
                Some(blocks)
            }
        }
        _ => None,
    }).collect();

    // Split cells into rows based on column count
    let rows: Vec<Vec<TableCell>> = cells.chunks(col_count)
        .map(|chunk| {
            chunk.iter().map(|cell_blocks| TableCell {
                content: cell_blocks.clone(),
                colspan: 1,
                rowspan: 1,
            }).collect()
        })
        .collect();

    Some(Block::Table(Table {
        caption: None,
        label: None,
        columns,
        header: None,
        rows,
    }))
}

fn parse_quote_func(&mut self) -> Option<Block> {
    let args = self.parse_args();
    let content = if matches!(self.peek(), Some(Token::BracketOpen)) {
        self.parse_content_block_blocks()
    } else {
        Vec::new()
    };

    let is_block = args.iter().any(|a| matches!(a,
        Arg::Named(k, ArgValue::Bool(true)) if k == "block"
    ));

    if is_block || !content.is_empty() {
        Some(Block::BlockQuote { content })
    } else {
        None
    }
}
```

- [ ] **Step 7: Run tests**

Run: `cargo test -p docmux-reader-typst -- parser::tests`
Expected: all tests pass

- [ ] **Step 8: Commit**

```bash
git add crates/docmux-reader-typst/src/parser.rs
git commit -m "feat(typst-reader): code blocks, function calls, argument parsing"
```

---

### Task 7: Parser — Metadata from `#set document()` and labels

**Files:**
- Modify: `crates/docmux-reader-typst/src/parser.rs`

- [ ] **Step 1: Write failing tests for metadata and labels**

```rust
#[test]
fn parse_yaml_frontmatter_metadata() {
    let doc = parse("---\ntitle: My Document\nauthor: Jane Doe\ndate: 2026-01-15\n---\n\n= Introduction");
    assert_eq!(doc.metadata.title.as_deref(), Some("My Document"));
    assert_eq!(doc.metadata.authors.len(), 1);
    assert_eq!(doc.metadata.authors[0].name, "Jane Doe");
    assert_eq!(doc.metadata.date.as_deref(), Some("2026-01-15"));
}

#[test]
fn parse_set_document_metadata() {
    let doc = parse("#set document(title: \"My Paper\", author: \"John Smith\")\n\n= Body");
    assert_eq!(doc.metadata.title.as_deref(), Some("My Paper"));
}

#[test]
fn parse_label_on_heading() {
    let doc = parse("= Introduction <intro>");
    if let Block::Heading { id, .. } = &doc.content[0] {
        assert_eq!(id.as_deref(), Some("intro"));
    } else {
        panic!("expected heading");
    }
}

#[test]
fn parse_label_on_math() {
    let doc = parse("$ E = mc^2 $ <eq-einstein>");
    if let Block::MathBlock { label, .. } = &doc.content[0] {
        assert_eq!(label.as_deref(), Some("eq-einstein"));
    } else {
        panic!("expected math block");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-typst -- parser::tests`
Expected: metadata and label tests FAIL

- [ ] **Step 3: Implement `#set document()` metadata extraction**

Modify `parse_metadata` to do a two-pass approach: first handle YAML frontmatter, then scan for `#set document()`:

```rust
fn parse_metadata(&mut self) -> Metadata {
    let mut metadata = Metadata::default();

    // Pass 1: YAML frontmatter
    if matches!(self.peek(), Some(Token::RawFrontmatter { .. })) {
        if let Some(Token::RawFrontmatter { value }) = self.advance() {
            metadata = self.parse_yaml_frontmatter(&value);
        }
    }

    // Pass 2: Scan for #set document(...) — look ahead without consuming non-set tokens
    let saved_pos = self.pos;
    while let Some(tok) = self.peek() {
        match tok {
            Token::FuncCall { name, .. } if name == "set" => {
                let set_pos = self.pos;
                self.advance(); // consume "set"
                // Check if followed by "document"
                if let Some(Token::Text { value }) = self.peek() {
                    if value.trim().starts_with("document") {
                        self.advance(); // consume "document"
                        let args = self.parse_args();
                        // Extract metadata from args — YAML takes priority
                        for arg in &args {
                            match arg {
                                Arg::Named(k, ArgValue::String(v)) => match k.as_str() {
                                    "title" if metadata.title.is_none() => {
                                        metadata.title = Some(v.clone());
                                    }
                                    "date" if metadata.date.is_none() => {
                                        metadata.date = Some(v.clone());
                                    }
                                    "author" if metadata.authors.is_empty() => {
                                        metadata.authors = vec![Author {
                                            name: v.clone(),
                                            affiliation: None,
                                            email: None,
                                            orcid: None,
                                        }];
                                    }
                                    _ => {}
                                },
                                _ => {}
                            }
                        }
                        continue;
                    }
                }
                // Not #set document — rewind to after "set" and consume rest
                self.pos = set_pos;
                self.advance(); // re-consume "set"
                self.consume_func_args_and_content();
            }
            Token::FuncCall { name, .. } if SILENTLY_IGNORED.contains(&name.as_str()) => {
                self.advance();
                self.consume_func_args_and_content();
            }
            Token::Comment { .. } | Token::BlockComment { .. } => { self.advance(); }
            Token::Newline | Token::BlankLine => { self.advance(); }
            _ => break, // Found content — stop scanning
        }
    }

    metadata
}
```

- [ ] **Step 4: Implement label handling on blocks**

Add label extraction after block parsing. After each block is parsed, check if the next token is a `Label`:

```rust
fn parse_block(&mut self) -> Option<Block> {
    let block = match self.peek()? {
        Token::Heading { .. } => self.parse_heading(),
        Token::Dollar => {
            if self.is_display_math() {
                self.parse_display_math_block()
            } else {
                self.parse_paragraph()
            }
        }
        Token::Dash { count } if *count >= 3 => { self.advance(); Some(Block::ThematicBreak) }
        Token::Dash { count: 1 } => self.parse_unordered_list(),
        Token::Plus { .. } => self.parse_ordered_list(),
        Token::TermMarker { .. } => self.parse_definition_list(),
        Token::Backtick { count } if *count >= 3 => self.parse_code_block(),
        Token::FuncCall { .. } => self.parse_func_call_block(),
        _ => self.parse_paragraph(),
    };

    // Check for trailing label <name>
    if let Some(mut b) = block {
        if matches!(self.peek(), Some(Token::Label { .. })) {
            if let Some(Token::Label { name }) = self.advance() {
                self.attach_label(&mut b, name);
            }
        }
        Some(b)
    } else {
        None
    }
}

fn attach_label(&self, block: &mut Block, label: String) {
    match block {
        Block::Heading { id, .. } => *id = Some(label),
        Block::MathBlock { label: l, .. } => *l = Some(label),
        Block::CodeBlock { label: l, .. } => *l = Some(label),
        Block::Figure { label: l, .. } => *l = Some(label),
        Block::Table(t) => t.label = Some(label),
        _ => {} // Labels on other blocks are silently dropped
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p docmux-reader-typst -- parser::tests`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-reader-typst/src/parser.rs
git commit -m "feat(typst-reader): metadata from YAML + #set document(), label attachment"
```

---

### Task 8: CLI Integration + Smoke Tests

**Files:**
- Modify: `crates/docmux-cli/Cargo.toml`
- Modify: `crates/docmux-cli/src/main.rs`
- Modify: `crates/docmux-cli/tests/cli_smoke.rs`

- [ ] **Step 1: Add docmux-reader-typst to CLI Cargo.toml**

Add to `[dependencies]`:
```toml
docmux-reader-typst = { workspace = true }
```

- [ ] **Step 2: Register TypstReader in build_registry**

In `crates/docmux-cli/src/main.rs`, add:
```rust
use docmux_reader_typst::TypstReader;
```

And in `build_registry()`:
```rust
reg.add_reader(Box::new(TypstReader::new()));
```

- [ ] **Step 3: Add Typst smoke tests**

Add to `crates/docmux-cli/tests/cli_smoke.rs`:

```rust
#[test]
fn converts_typst_to_html_stdout() {
    let tmp = std::env::temp_dir().join("docmux-test");
    std::fs::create_dir_all(&tmp).ok();
    let input_file = tmp.join("test.typ");
    std::fs::write(&input_file, "= Hello\n\nWorld.").unwrap();

    let output = Command::new(docmux_bin())
        .arg(&input_file)
        .arg("--to").arg("html")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success(), "docmux exited with error: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("<h1>"), "Expected heading in output, got: {stdout}");
}

#[test]
fn converts_typst_to_latex_stdout() {
    let tmp = std::env::temp_dir().join("docmux-test");
    std::fs::create_dir_all(&tmp).ok();
    let input_file = tmp.join("test.typ");
    std::fs::write(&input_file, "= Hello\n\n*Bold* and _italic_.").unwrap();

    let output = Command::new(docmux_bin())
        .arg(&input_file)
        .arg("--to").arg("latex")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\\section"), "Expected LaTeX section");
}

#[test]
fn typst_format_autodetected() {
    let tmp = std::env::temp_dir().join("docmux-test");
    std::fs::create_dir_all(&tmp).ok();
    let input_file = tmp.join("autodetect.typ");
    std::fs::write(&input_file, "Hello world.").unwrap();

    let output = Command::new(docmux_bin())
        .arg(&input_file)
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success(), "Typst format should be auto-detected from .typ extension");
}
```

- [ ] **Step 4: Run smoke tests**

Run: `cargo test -p docmux-cli --test cli_smoke`
Expected: all tests pass (including new Typst ones)

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-cli/Cargo.toml crates/docmux-cli/src/main.rs crates/docmux-cli/tests/cli_smoke.rs
git commit -m "feat(cli): register Typst reader + smoke tests"
```

---

### Task 9: Golden File Tests

**Files:**
- Modify: `crates/docmux-cli/tests/golden.rs`
- Create: `tests/fixtures/basic/typst-heading.typ`
- Create: `tests/fixtures/basic/typst-inlines.typ`
- Create: `tests/fixtures/basic/typst-lists.typ`
- Create: `tests/fixtures/basic/typst-math.typ`

- [ ] **Step 1: Add golden test infrastructure for Typst → HTML**

Add to `crates/docmux-cli/tests/golden.rs`:

```rust
use docmux_reader_typst::TypstReader;

fn convert_typ_to_html(input: &str) -> String {
    let reader = TypstReader::new();
    let writer = HtmlWriter::new();
    let opts = WriteOptions::default();
    let doc = reader.read(input).expect("typst reader should not fail on fixture");
    writer.write(&doc, &opts).expect("html writer should not fail")
}

fn discover_typ_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if !dir.is_dir() { return results; }
    for entry in std::fs::read_dir(dir).expect("read fixtures dir") {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        if path.is_dir() {
            results.extend(discover_typ_fixtures(&path));
        } else if path.extension().is_some_and(|ext| ext == "typ") {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            if stem.starts_with("typst-") {
                results.push(path);
            }
        }
    }
    results.sort();
    results
}

#[test]
fn golden_typ_to_html() {
    let base = fixtures_dir();
    let fixtures = discover_typ_fixtures(&base);

    if fixtures.is_empty() {
        eprintln!("No .typ fixtures found (skipping golden_typ_to_html)");
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut generated = 0u32;
    let mut updated = 0u32;

    for fixture_path in &fixtures {
        let name = test_name(fixture_path, &base);
        let expected_path = fixture_path.with_extension("typ.html");
        let input = std::fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read input: {e}"));
        let actual = convert_typ_to_html(&input);

        if update_mode() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            updated += 1;
            eprintln!("  updated: {name}.typ.html");
            continue;
        }

        if !expected_path.exists() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            generated += 1;
            eprintln!("  generated: {name}.typ.html (new — review the file)");
            continue;
        }

        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read expected: {e}"));

        if actual != expected {
            failures.push(format!(
                "━━━ MISMATCH: {name}.typ.html ━━━\n--- expected ({path})\n+++ actual\n\n{diff}\nHint: run `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden` to update.\n",
                path = expected_path.display(),
                diff = line_diff(&expected, &actual),
            ));
        }
    }

    if generated > 0 { eprintln!("\n  {} new .typ.html expectation(s) generated.", generated); }
    if updated > 0 { eprintln!("\n  {} .typ.html expectation(s) updated.", updated); }
    if !failures.is_empty() {
        panic!("\n\n{count} .typ→.html golden file(s) mismatched:\n\n{details}",
            count = failures.len(), details = failures.join("\n"));
    }
}
```

Also add `docmux-reader-typst = { workspace = true }` to the golden test's Cargo.toml dependencies (it's in `crates/docmux-cli/Cargo.toml` under `[dev-dependencies]` if needed, or the existing `[dependencies]` since it's already used in main).

- [ ] **Step 2: Create golden fixture files**

`tests/fixtures/basic/typst-heading.typ`:
```typst
= First Heading

Some introductory text.

== Second Level

More content here.

=== Third Level

Deep content.
```

`tests/fixtures/basic/typst-inlines.typ`:
```typst
This has *bold text* and _italic text_ and `inline code`.

Here is a #link("https://example.com")[clickable link] in text.

Some #emph[emphasized] and #strong[strong] and #strike[deleted] text.
```

`tests/fixtures/basic/typst-lists.typ`:
```typst
- First item
- Second item
- Third item

+ Step one
+ Step two
+ Step three

/ Term: Its definition
/ Another: Another definition
```

`tests/fixtures/basic/typst-math.typ`:
```typst
Inline math: $x^2 + y^2 = z^2$ in a sentence.

Display math:

$ E = m c^2 $
```

- [ ] **Step 3: Run golden tests to generate expectations**

Run: `cargo test -p docmux-cli --test golden -- golden_typ_to_html`
Expected: generates `.typ.html` files automatically

- [ ] **Step 4: Review generated expectations**

Run: `git diff tests/fixtures/` and visually verify the HTML output makes sense.

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-cli/tests/golden.rs tests/fixtures/basic/typst-*.typ tests/fixtures/basic/typst-*.typ.html
git commit -m "feat(typst-reader): golden file tests for Typst → HTML"
```

---

### Task 10: Full workspace verification

**Files:** (none — verification only)

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace`
Expected: all tests pass (existing + new)

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --all -- --check`
Expected: no formatting issues

- [ ] **Step 4: Run WASM build check**

Run: `cargo build --target wasm32-unknown-unknown -p docmux-wasm`
Expected: compiles (Typst reader may not be wired into wasm crate yet — verify it doesn't break)

- [ ] **Step 5: Update ROADMAP.md**

Mark the Typst reader as complete:
```markdown
- [x] `docmux-reader-typst` — Typst markup parser (N tests)
```

- [ ] **Step 6: Commit**

```bash
git add ROADMAP.md
git commit -m "mark docmux-reader-typst as complete in roadmap"
```
