# LaTeX Reader Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `docmux-reader-latex` — a recursive descent parser that converts a practical subset of LaTeX into the docmux AST.

**Architecture:** Manual lexer (`&str` → `Vec<Token>`) followed by a recursive descent parser (`Vec<Token>` → `Document`). Unrecognized commands/environments become `RawBlock`/`RawInline` with warnings. Preamble metadata (`\title`, `\author`, `\date`) is extracted into typed `Metadata` fields.

**Tech Stack:** Rust, no new external dependencies (only `docmux-ast`, `docmux-core`).

**Spec:** `docs/superpowers/specs/2026-03-23-latex-reader-design.md`

---

## File Map

| Action | Path | Responsibility |
|--------|------|---------------|
| Modify | `crates/docmux-ast/src/lib.rs` | Add `ParseWarning` struct and `warnings` field to `Document` |
| Modify | `crates/docmux-reader-markdown/src/lib.rs` | Add `warnings: vec![]` to `Document` construction |
| Verify | `crates/docmux-reader-latex/Cargo.toml` | No changes needed (only `docmux-ast` and `docmux-core`) |
| Create | `crates/docmux-reader-latex/src/unescape.rs` | LaTeX special char unescaping |
| Create | `crates/docmux-reader-latex/src/lexer.rs` | Tokenizer: `&str` → `Vec<Token>` |
| Create | `crates/docmux-reader-latex/src/parser.rs` | Recursive descent: `Vec<Token>` → `Document` |
| Modify | `crates/docmux-reader-latex/src/lib.rs` | `LatexReader` struct + `Reader` trait impl |
| Modify | `crates/docmux-cli/Cargo.toml` | Add `docmux-reader-latex` dependency |
| Modify | `crates/docmux-cli/src/main.rs` | Register `LatexReader` in `build_registry()` |
| Create | `tests/fixtures/basic/latex-heading.tex` | Test fixture |
| Create | `tests/fixtures/basic/latex-paragraph.tex` | Test fixture |
| Create | `tests/fixtures/basic/latex-math.tex` | Test fixture |
| Create | `tests/fixtures/basic/latex-lists.tex` | Test fixture |
| Create | `tests/fixtures/basic/latex-table.tex` | Test fixture |
| Create | `tests/fixtures/basic/latex-figure.tex` | Test fixture |
| Create | `tests/fixtures/basic/latex-code.tex` | Test fixture |
| Create | `tests/fixtures/basic/latex-inlines.tex` | Test fixture |
| Create | `tests/fixtures/complex/latex-academic-paper.tex` | Complex test fixture |

---

### Task 1: Add `ParseWarning` to AST

**Files:**
- Modify: `crates/docmux-ast/src/lib.rs:15-26` (Document struct and imports)
- Modify: `crates/docmux-reader-markdown/src/lib.rs:433-437` (Document construction)
- Modify: `crates/docmux-transform-crossref/src/lib.rs` (any direct Document construction in tests)

- [ ] **Step 1: Add `ParseWarning` struct and `warnings` field to `Document`**

In `crates/docmux-ast/src/lib.rs`, add after the `Document` struct definition:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParseWarning {
    pub line: usize,
    pub message: String,
}
```

And add the field to `Document`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Document {
    pub metadata: Metadata,
    pub content: Vec<Block>,
    pub bibliography: Option<Bibliography>,
    #[serde(default)]
    pub warnings: Vec<ParseWarning>,
}
```

- [ ] **Step 2: Fix all compilation errors across the workspace**

Every place that constructs `Document { metadata, content, bibliography }` without `warnings` will fail. Find and fix them:

- `crates/docmux-reader-markdown/src/lib.rs:433-437`: add `warnings: vec![]`
- `crates/docmux-ast/src/lib.rs` tests (`document_with_content`, `serialization_roundtrip`): add `warnings: vec![]`
- Any other crate that constructs `Document` directly.
- Note: writer crate tests (`docmux-writer-html`, `docmux-writer-latex`) use `..Default::default()` and will compile without changes since `Vec<ParseWarning>` implements `Default`.

- [ ] **Step 3: Run `cargo check --workspace` to verify**

Run: `cargo check --workspace`
Expected: compiles clean.

- [ ] **Step 4: Run `cargo test --workspace` to verify nothing broke**

Run: `cargo test --workspace`
Expected: all 55 existing tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-ast/src/lib.rs crates/docmux-reader-markdown/src/lib.rs crates/docmux-transform-crossref/src/lib.rs
git commit -m "Add ParseWarning to Document AST for reader diagnostics"
```

---

### Task 2: Implement `unescape.rs`

**Files:**
- Create: `crates/docmux-reader-latex/src/unescape.rs`

- [ ] **Step 1: Write tests for unescape**

At the bottom of `unescape.rs`, add:

```rust
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
    fn unescape_no_special_chars() {
        assert_eq!(unescape_latex("plain text"), "plain text");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-latex`
Expected: FAIL — `unescape_latex` not found.

- [ ] **Step 3: Implement `unescape_latex`**

```rust
/// Revert LaTeX special character escaping.
///
/// Handles the 10 escape sequences produced by the LaTeX writer's
/// `escape_latex()` function, converting them back to plain characters.
pub fn unescape_latex(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            // Try multi-char sequences first
            let rest: String = chars.clone().collect();
            if rest.starts_with("textbackslash{}") {
                out.push('\\');
                // consume "textbackslash{}"
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
                    '{' => { out.push('{'); chars.next(); }
                    '}' => { out.push('}'); chars.next(); }
                    '#' => { out.push('#'); chars.next(); }
                    '$' => { out.push('$'); chars.next(); }
                    '%' => { out.push('%'); chars.next(); }
                    '&' => { out.push('&'); chars.next(); }
                    '_' => { out.push('_'); chars.next(); }
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-reader-latex`
Expected: all unescape tests pass.

- [ ] **Step 5: Run `cargo clippy -p docmux-reader-latex -- -D warnings`**

Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-reader-latex/src/unescape.rs
git commit -m "Add LaTeX special character unescaping"
```

---

### Task 3: Implement `lexer.rs` — Token types and basic tokenization

**Files:**
- Create: `crates/docmux-reader-latex/src/lexer.rs`

- [ ] **Step 1: Write tests for the lexer**

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
        assert!(tokens.iter().any(|t| matches!(t, Token::DoubleBackslash { .. })));
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-latex`
Expected: FAIL — `Token` and `tokenize` not found.

- [ ] **Step 3: Implement `Token` enum and `tokenize` function**

```rust
/// A token produced by the LaTeX lexer.
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// A LaTeX command like `\section`, `\textbf`.
    Command { name: String, line: usize },
    /// `\begin{envname}`
    BeginEnv { name: String, line: usize },
    /// `\end{envname}`
    EndEnv { name: String, line: usize },
    /// `{`
    BraceOpen,
    /// `}`
    BraceClose,
    /// `[`
    BracketOpen,
    /// `]`
    BracketClose,
    /// Plain text content.
    Text { value: String },
    /// Inline math: content between `$...$`.
    MathInline { value: String },
    /// Display math: content between `$$...$$` or `\[...\]`.
    MathDisplay { value: String },
    /// A comment: `% ...` until end of line.
    Comment { value: String },
    /// A blank line (paragraph separator).
    BlankLine,
    /// `~` (non-breaking space).
    Tilde,
    /// `&` (table cell separator).
    Ampersand,
    /// `\\` (line break / table row end).
    DoubleBackslash { line: usize },
    /// A single newline (within text, between inline content).
    Newline,
}
```

Implement `pub fn tokenize(input: &str) -> Vec<Token>` — a character-by-character scanner that:
- Tracks `line` number (1-based)
- Recognizes `\\` as `DoubleBackslash` (must check before single `\`)
- Recognizes `\[...\]` as `MathDisplay`
- Recognizes `\begin{X}` and `\end{X}` as `BeginEnv`/`EndEnv`
- Recognizes `\commandname` (letters only) as `Command`
- Recognizes `$$...$$` as `MathDisplay` and `$...$` as `MathInline`
- Recognizes `%...` to end-of-line as `Comment`
- Recognizes `\n\n` (or `\n` followed by whitespace-only line) as `BlankLine`
- Recognizes `~`, `&`, `{`, `}`, `[`, `]` as their respective tokens
- For commands, if the next char after the command name is `*`, includes it in the name (e.g., `Command { name: "section*" }`)
- Emits `Newline` for single newlines within text (distinct from `BlankLine` which is `\n\n`)
- Accumulates everything else into `Text` tokens

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-reader-latex`
Expected: all lexer tests pass.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p docmux-reader-latex -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-reader-latex/src/lexer.rs
git commit -m "Add LaTeX lexer with token types and tokenizer"
```

---

### Task 4: Implement `parser.rs` — Core parsing infrastructure

**Files:**
- Create: `crates/docmux-reader-latex/src/parser.rs`

This task builds the parser skeleton: struct, helpers (`parse_brace_argument`, `parse_optional_argument`, `peek`, `advance`), preamble extraction, and paragraph assembly from inline tokens. No block environments yet.

- [ ] **Step 1: Write tests for core parser functionality**

```rust
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
            assert!(content.iter().any(|i| matches!(i, Inline::MathInline { .. })));
        } else {
            panic!("Expected paragraph");
        }
    }

    #[test]
    fn parse_tilde_as_nbsp() {
        let tokens = tokenize(r"Dr.~Smith");
        let doc = Parser::new(tokens).parse();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(content.iter().any(|i| matches!(i, Inline::Text { value } if value == "\u{00A0}")));
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
            assert!(content.iter().any(|i| matches!(i, Inline::RawInline { format, .. } if format == "latex")));
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-latex`
Expected: FAIL — `Parser` not found.

- [ ] **Step 3: Implement Parser struct and core methods**

Implement `Parser` struct with:
- `tokens: Vec<Token>`, `pos: usize`, `warnings: Vec<ParseWarning>`
- `fn parse(mut self) -> Document` — entry point: detect preamble, parse body
- `fn peek(&self) -> Option<&Token>`
- `fn advance(&mut self) -> Option<Token>`
- `fn expect_brace_open(&mut self) -> bool`
- `fn parse_brace_argument(&mut self) -> Vec<Token>` — balanced brace consumption
- `fn parse_optional_argument(&mut self) -> Option<String>` — consume `[...]` if present
- `fn parse_preamble(&mut self) -> (Metadata, Option<String>)` — extract title/author/date, collect raw preamble
- `fn parse_body(&mut self) -> Vec<Block>` — parse between `\begin{document}` and `\end{document}` (or all tokens in snippet mode)
- `fn parse_blocks(&mut self, stop_at: Option<&str>) -> Vec<Block>` — collect blocks until `\end{stop_at}` or EOF
- `fn collect_paragraph_inlines(&mut self) -> Vec<Inline>` — gather inlines until blank line or block-level command
- `fn token_to_inline(&mut self, token: Token) -> Inline` — dispatch inline commands (`\emph`, `\textbf`, `\texttt`, etc.)
- `fn is_block_command(name: &str) -> bool` — returns true for `section`, `subsection`, etc.
- `fn warn(&mut self, line: usize, message: impl Into<String>)`

Inline command dispatch in `token_to_inline`:
- `emph` | `textit` → `Inline::Emphasis { content }`
- `textbf` → `Inline::Strong { content }`
- `sout` → `Inline::Strikethrough { content }`
- `texttt` → `Inline::Code { value }` (collect text content as string)
- `textsc` → `Inline::SmallCaps { content }`
- `textsuperscript` → `Inline::Superscript { content }`
- `textsubscript` → `Inline::Subscript { content }`
- `href` → `Inline::Link { url, content }` (two brace args)
- `url` → `Inline::Link { url, content: [Text(url)] }`
- `cite` → `Inline::Citation { mode: Normal, keys }`
- `citet` → `Inline::Citation { mode: AuthorOnly, keys }`
- `citeyear` → `Inline::Citation { mode: SuppressAuthor, keys }`
- `ref` → `Inline::CrossRef { form: Number }`
- `autoref` → `Inline::CrossRef { form: NumberWithType }`
- `pageref` → `Inline::CrossRef { form: Page }`
- `label` → skip (captured later by parent block)
- `footnote` → generate `FootnoteDef` (stored aside) + return `FootnoteRef`
- Unknown → `Inline::RawInline { format: "latex" }` + warning

Handle `Token::Tilde` → `Inline::Text { value: "\u{00A0}" }`
Handle `Token::MathInline` → `Inline::MathInline { value }`
Handle `Token::DoubleBackslash` → `Inline::HardBreak`
Handle `Token::Newline` → `Inline::SoftBreak`

Silently ignored commands (consume with arguments, no warning):
- `documentclass`, `usepackage`, `newcommand`, `renewcommand`, `maketitle`, `tableofcontents`, `bibliographystyle`, `pagestyle`, `thispagestyle`, `setlength`, `setcounter`
- These are checked both in preamble parsing and in body parsing; they produce no output and no warning.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-reader-latex`
Expected: all parser core tests pass.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p docmux-reader-latex -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-reader-latex/src/parser.rs
git commit -m "Add LaTeX parser core: paragraphs, inlines, preamble extraction"
```

---

### Task 5: Implement block-level environment parsing

**Files:**
- Modify: `crates/docmux-reader-latex/src/parser.rs`

Adds `parse_environment` dispatch and handlers for all block environments.

- [ ] **Step 1: Write tests for block environments**

Add to the test module in `parser.rs`:

```rust
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
    if let Block::CodeBlock { language, content, .. } = &doc.content[0] {
        assert!(language.is_none());
        assert!(content.contains("fn main()"));
    } else {
        panic!("Expected CodeBlock");
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
        panic!("Expected CodeBlock");
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
    if let Block::Figure { image, caption, label } = &doc.content[0] {
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
    assert_eq!(doc.metadata.abstract_text.as_deref(), Some("This is the abstract."));
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
    assert!(doc.content.iter().any(|b| matches!(b, Block::FootnoteDef { id, .. } if id == "1")));
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

#[test]
fn parse_silently_ignored_commands_no_warning() {
    let tokens = tokenize(r"\maketitle
\tableofcontents
\bibliographystyle{plain}

Some text.");
    let doc = Parser::new(tokens).parse();
    assert!(doc.warnings.is_empty(), "Expected no warnings for ignored commands");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-latex`
Expected: FAIL — new tests fail.

- [ ] **Step 3: Implement environment parsing**

Add to `parser.rs`:

- `fn parse_block_command(&mut self, name: &str, line: usize) -> Option<Block>` — dispatch for block-level commands:
  - `section` → `Heading { level: 1 }`, `subsection` → level 2, `subsubsection` → 3, `paragraph` → 4, `subparagraph` → 5
  - Starred variants (`section*`) → same but no `id`
  - `hrule` → `ThematicBreak`
  - `noindent` → check if followed by `\rule{...}` → `ThematicBreak`
  - `footnotetext` → consume optional `[N]` and brace argument → `Block::FootnoteDef { id: N.to_string(), content }`
  - After parsing a heading, peek for `\label{...}` and set `Heading.id` from it

- `fn parse_environment(&mut self, env_name: &str, line: usize) -> Block` — dispatch by environment name:
  - `itemize` → `parse_list(false)`
  - `enumerate` → `parse_list(true)`
  - `quote` | `quotation` → `parse_blockquote()`
  - `verbatim` → `parse_verbatim_env("verbatim")`
  - `lstlisting` → `parse_verbatim_env("lstlisting")` (extract `[language=X]`)
  - `equation` | `align` | `align*` | `gather` | `gather*` | `multline` → `parse_math_env()`
  - `figure` → `parse_figure()`
  - `table` → `parse_table_env()`
  - `tabular` → `parse_tabular()`
  - `abstract` → extract text → set `metadata.abstract_text`; return nothing (handled specially)
  - `description` → `parse_description()`
  - `document` → `parse_blocks(Some("document"))` (handled by `parse_document`)
  - Unknown → collect raw tokens until `\end{X}`, emit `RawBlock { format: "latex" }` + warning

- `fn parse_list(&mut self, ordered: bool) -> Block` — split by `\item`, parse each item's content
- `fn parse_blockquote(&mut self) -> Block` — parse blocks until `\end{quote}`
- `fn parse_verbatim_env(&mut self, env_name: &str) -> Block` — collect raw text until `\end{env_name}`
- `fn parse_math_env(&mut self) -> Block` — collect raw text until `\end{...}`
- `fn parse_figure(&mut self) -> Block` — scan for `\includegraphics`, `\caption`, `\label`
- `fn parse_table_env(&mut self) -> Block` — scan for `\begin{tabular}`, `\caption`, `\label`
- `fn parse_tabular(&mut self) -> Table` — parse column spec from `{|l|r|c|}`, rows split by `\\`, cells by `&`
- `fn parse_description(&mut self) -> Block` — `\item[term]` split

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-reader-latex`
Expected: all block environment tests pass.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p docmux-reader-latex -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-reader-latex/src/parser.rs
git commit -m "Add block-level environment parsing to LaTeX parser"
```

---

### Task 6: Wire up `LatexReader` and `lib.rs`

**Files:**
- Modify: `crates/docmux-reader-latex/Cargo.toml`
- Modify: `crates/docmux-reader-latex/src/lib.rs`

- [ ] **Step 1: Verify Cargo.toml is correct**

The existing `Cargo.toml` already has the needed dependencies (`docmux-ast`, `docmux-core`). No changes needed — the reader never fails (best-effort), so `thiserror` is not required.

- [ ] **Step 2: Implement `lib.rs`**

```rust
//! # docmux-reader-latex
//!
//! LaTeX reader for docmux. Parses a practical subset of LaTeX into the
//! docmux AST using a hand-written recursive descent parser.
//!
//! Unrecognized commands and environments are emitted as `RawBlock`/`RawInline`
//! with warnings accumulated in `Document.warnings`.

mod lexer;
mod parser;
mod unescape;

use docmux_ast::Document;
use docmux_core::{Reader, Result};

pub use lexer::Token;

/// A LaTeX reader.
#[derive(Debug, Default)]
pub struct LatexReader;

impl LatexReader {
    pub fn new() -> Self {
        Self
    }
}

impl Reader for LatexReader {
    fn format(&self) -> &str {
        "latex"
    }

    fn extensions(&self) -> &[&str] {
        &["tex", "latex"]
    }

    fn read(&self, input: &str) -> Result<Document> {
        let tokens = lexer::tokenize(input);
        let doc = parser::Parser::new(tokens).parse();
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_trait_metadata() {
        let reader = LatexReader::new();
        assert_eq!(reader.format(), "latex");
        assert!(reader.extensions().contains(&"tex"));
    }

    #[test]
    fn read_simple_document() {
        let reader = LatexReader::new();
        let doc = reader.read(r"\section{Hello}

Some text.").unwrap();
        assert_eq!(doc.content.len(), 2);
    }
}
```

- [ ] **Step 3: Run `cargo check -p docmux-reader-latex`**

Expected: compiles.

- [ ] **Step 4: Run `cargo test -p docmux-reader-latex`**

Expected: all tests pass (unescape + lexer + parser + lib).

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-reader-latex/
git commit -m "Wire up LatexReader with Reader trait implementation"
```

---

### Task 7: Integrate into CLI

**Files:**
- Modify: `crates/docmux-cli/Cargo.toml`
- Modify: `crates/docmux-cli/src/main.rs`

- [ ] **Step 1: Add dependency to CLI Cargo.toml**

Add to `[dependencies]`:

```toml
docmux-reader-latex = { workspace = true }
```

- [ ] **Step 2: Register reader in `build_registry()`**

In `crates/docmux-cli/src/main.rs`, add import:

```rust
use docmux_reader_latex::LatexReader;
```

And register in `build_registry()`:

```rust
fn build_registry() -> Registry {
    let mut reg = Registry::new();
    reg.add_reader(Box::new(MarkdownReader::new()));
    reg.add_reader(Box::new(LatexReader::new()));
    reg.add_writer(Box::new(HtmlWriter::new()));
    reg.add_writer(Box::new(LatexWriter::new()));
    reg
}
```

- [ ] **Step 3: Run `cargo check -p docmux-cli`**

Expected: compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-cli/Cargo.toml crates/docmux-cli/src/main.rs
git commit -m "Register LatexReader in CLI registry"
```

---

### Task 8: Add CLI smoke tests for LaTeX input

**Files:**
- Modify: `crates/docmux-cli/tests/cli_smoke.rs`
- Create: `tests/fixtures/basic/latex-paragraph.tex`

- [ ] **Step 1: Create a simple LaTeX fixture**

Create `tests/fixtures/basic/latex-paragraph.tex`:

```latex
\documentclass{article}
\begin{document}
This is a \textbf{bold} and \emph{italic} paragraph.
\end{document}
```

- [ ] **Step 2: Add smoke tests**

Add to `crates/docmux-cli/tests/cli_smoke.rs`:

```rust
#[test]
fn converts_latex_to_html_stdout() {
    let input = fixtures_dir().join("basic/latex-paragraph.tex");
    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("--to")
        .arg("html")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success(), "docmux exited with error: {}", String::from_utf8_lossy(&output.stderr));

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("<strong>bold</strong>"), "Expected bold in output, got: {stdout}");
    assert!(stdout.contains("<em>italic</em>"), "Expected italic in output, got: {stdout}");
}

#[test]
fn latex_auto_detects_format_by_extension() {
    let input = fixtures_dir().join("basic/latex-paragraph.tex");
    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("--to")
        .arg("html")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("<p>"), "Expected HTML paragraph output");
}
```

- [ ] **Step 3: Run smoke tests**

Run: `cargo test -p docmux-cli --test cli_smoke`
Expected: all smoke tests pass (including the 8 existing ones).

- [ ] **Step 4: Commit**

```bash
git add tests/fixtures/basic/latex-paragraph.tex crates/docmux-cli/tests/cli_smoke.rs
git commit -m "Add CLI smoke tests for LaTeX-to-HTML conversion"
```

---

### Task 9: Add golden test fixtures

**Files:**
- Create: `tests/fixtures/basic/latex-heading.tex`
- Create: `tests/fixtures/basic/latex-math.tex`
- Create: `tests/fixtures/basic/latex-lists.tex`
- Create: `tests/fixtures/basic/latex-table.tex`
- Create: `tests/fixtures/basic/latex-figure.tex`
- Create: `tests/fixtures/basic/latex-code.tex`
- Create: `tests/fixtures/basic/latex-inlines.tex`
- Create: `tests/fixtures/complex/latex-academic-paper.tex`
- Modify: `crates/docmux-cli/tests/golden.rs`

- [ ] **Step 1: Create `.tex` fixtures**

Create each fixture file covering the respective LaTeX constructs. Example `latex-heading.tex`:

```latex
\section{Introduction}

First paragraph.

\subsection{Background}

Second paragraph.

\subsubsection{Details}

Third paragraph.
```

Example `latex-math.tex`:

```latex
The formula $E = mc^2$ is inline.

\begin{equation}
x^2 + y^2 = z^2
\end{equation}
```

Example `latex-academic-paper.tex`:

```latex
\documentclass{article}
\usepackage[utf8]{inputenc}
\usepackage{amsmath}
\usepackage{graphicx}
\title{A Sample Paper}
\author{Jane Doe \and John Smith}
\date{2026-03-23}
\begin{document}
\maketitle

\begin{abstract}
This paper demonstrates the LaTeX reader.
\end{abstract}

\section{Introduction}

This is the introduction with inline math $E = mc^2$ and a citation \cite{smith2020}.

\subsection{Background}

See Figure~\ref{fig:example} and Table~\ref{tab:results}.

\begin{figure}[htbp]
\centering
\includegraphics{example.png}
\caption{An example figure}
\label{fig:example}
\end{figure}

\begin{table}[htbp]
\centering
\begin{tabular}{|l|r|}
\hline
Name & Value \\
\hline
Pi & 3.14 \\
E & 2.72 \\
\hline
\end{tabular}
\caption{Results}
\label{tab:results}
\end{table}

\section{Methods}

We used \textbf{bold} and \emph{italic} formatting. The equation:

\begin{equation}
\nabla \cdot \mathbf{E} = \frac{\rho}{\epsilon_0}
\end{equation}

\begin{itemize}
\item First item
\item Second item
\end{itemize}

\begin{enumerate}
\item Step one
\item Step two
\end{enumerate}

\begin{quote}
A famous quote goes here.
\end{quote}

\begin{verbatim}
fn main() {
    println!("Hello!");
}
\end{verbatim}

\end{document}
```

- [ ] **Step 2: Add `golden_tex_to_html` test to `golden.rs`**

Add a new golden test function to `crates/docmux-cli/tests/golden.rs`:

```rust
use docmux_reader_latex::LatexReader;

fn convert_tex_to_html(input: &str) -> String {
    let reader = LatexReader::new();
    let writer = HtmlWriter::new();
    let opts = WriteOptions::default();

    let doc = reader
        .read(input)
        .expect("latex reader should not fail on fixture");
    writer
        .write(&doc, &opts)
        .expect("html writer should not fail")
}

fn discover_tex_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if !dir.is_dir() {
        return results;
    }
    for entry in std::fs::read_dir(dir).expect("read fixtures dir") {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        if path.is_dir() {
            results.extend(discover_tex_fixtures(&path));
        } else if path.extension().is_some_and(|ext| ext == "tex") {
            // Skip .tex files that are golden outputs for .md fixtures
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            if stem.starts_with("latex-") {
                results.push(path);
            }
        }
    }
    results.sort();
    results
}

#[test]
fn golden_tex_to_html() {
    let base = fixtures_dir();
    let fixtures = discover_tex_fixtures(&base);

    if fixtures.is_empty() {
        eprintln!("No .tex fixtures found (skipping golden_tex_to_html)");
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut generated = 0u32;
    let mut updated = 0u32;

    for fixture_path in &fixtures {
        let name = test_name(fixture_path, &base);
        // .tex input → .tex.html expected output
        let expected_path = fixture_path.with_extension("tex.html");

        let input = std::fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read input: {e}"));

        let actual = convert_tex_to_html(&input);

        if update_mode() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            updated += 1;
            eprintln!("  updated: {name}.tex.html");
            continue;
        }

        if !expected_path.exists() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            generated += 1;
            eprintln!("  generated: {name}.tex.html (new — review the file)");
            continue;
        }

        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read expected: {e}"));

        if actual != expected {
            failures.push(format!(
                "━━━ MISMATCH: {name}.tex.html ━━━\n\
                 --- expected ({path})\n\
                 +++ actual\n\n\
                 {diff}\n\
                 Hint: run `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden` to update.\n",
                path = expected_path.display(),
                diff = line_diff(&expected, &actual),
            ));
        }
    }

    if generated > 0 {
        eprintln!("\n  {} new .tex.html expectation(s) generated.", generated);
    }

    if updated > 0 {
        eprintln!("\n  {} .tex.html expectation(s) updated.", updated);
    }

    if !failures.is_empty() {
        panic!(
            "\n\n{count} .tex→.html golden file(s) mismatched:\n\n{details}",
            count = failures.len(),
            details = failures.join("\n"),
        );
    }
}
```

- [ ] **Step 3: Add `docmux-reader-latex` dependency to CLI's Cargo.toml (if not already)**

It should already be there from Task 7. Verify.

- [ ] **Step 4: Run golden tests to bootstrap expectation files**

Run: `cargo test -p docmux-cli --test golden`
Expected: generates `.tex.html` files for each `.tex` fixture. All existing `.md` golden tests still pass.

- [ ] **Step 5: Review generated `.tex.html` files**

Manually review the generated HTML output for correctness.

- [ ] **Step 6: Run full workspace test suite**

Run: `cargo test --workspace`
Expected: all tests pass.

- [ ] **Step 7: Run clippy and fmt**

Run: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add tests/fixtures/ crates/docmux-cli/tests/golden.rs crates/docmux-cli/Cargo.toml
git commit -m "Add golden test fixtures and harness for LaTeX-to-HTML"
```

---

### Task 10: Update ROADMAP.md

**Files:**
- Modify: `ROADMAP.md`

- [ ] **Step 1: Mark `docmux-reader-latex` as complete**

Change:
```markdown
- [ ] `docmux-reader-latex` — parse LaTeX subset into AST
```
To:
```markdown
- [x] `docmux-reader-latex` — parse LaTeX subset into AST (N tests)
```

(Replace `N` with the actual test count.)

- [ ] **Step 2: Update the total test count**

Update the "Total:" line to reflect the new test count.

- [ ] **Step 3: Commit**

```bash
git add ROADMAP.md
git commit -m "Mark LaTeX reader as complete in roadmap"
```
