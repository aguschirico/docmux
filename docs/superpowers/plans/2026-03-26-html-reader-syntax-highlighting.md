# HTML Reader + Syntax Highlighting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an HTML reader that parses HTML into the docmux AST, and a shared syntax highlighting library (syntect) consumed by writers for colorized code output.

**Architecture:** Two new crates: `docmux-highlight` (shared highlighting via syntect, returns token spans per line) and `docmux-reader-html` (scraper/html5ever, semantic-first with best-effort for general HTML). Writers call `docmux-highlight` when `WriteOptions.highlight_style` is set. CLI exposes `--highlight-style`, `--list-highlight-themes`, `--list-highlight-languages`.

**Tech Stack:** Rust, syntect (Sublime grammars), scraper (html5ever wrapper), docmux workspace crates

---

## File Map

### New files

| File | Responsibility |
|------|---------------|
| `crates/docmux-highlight/Cargo.toml` | Crate manifest for highlighting library |
| `crates/docmux-highlight/src/lib.rs` | `highlight()`, `available_languages()`, `available_themes()`, types |
| `crates/docmux-reader-html/Cargo.toml` | Crate manifest for HTML reader |
| `crates/docmux-reader-html/src/lib.rs` | `HtmlReader` struct, `Reader` impl, block/inline conversion |
| `tests/fixtures/basic/simple.html` | Golden test fixture: simple HTML input |
| `tests/fixtures/basic/simple.html.md` | Golden test expectation: HTML → Markdown |
| `tests/fixtures/basic/simple.html.tex` | Golden test expectation: HTML → LaTeX |

### Modified files

| File | Changes |
|------|---------|
| `Cargo.toml` (workspace) | Add `docmux-highlight` and `docmux-reader-html` to members + deps, add `scraper` and `syntect` to workspace deps |
| `crates/docmux-core/src/lib.rs` | Add `highlight_style: Option<String>` to `WriteOptions` |
| `crates/docmux-writer-html/Cargo.toml` | Add `docmux-highlight` dependency |
| `crates/docmux-writer-html/src/lib.rs` | Use highlight for CodeBlock when `highlight_style` is set |
| `crates/docmux-writer-latex/Cargo.toml` | Add `docmux-highlight` dependency |
| `crates/docmux-writer-latex/src/lib.rs` | Use highlight for CodeBlock when `highlight_style` is set |
| `crates/docmux-cli/Cargo.toml` | Add `docmux-reader-html` and `docmux-highlight` dependencies |
| `crates/docmux-cli/src/main.rs` | Register `HtmlReader`, add `--highlight-style`, `--list-highlight-themes`, `--list-highlight-languages` |
| `crates/docmux-wasm/Cargo.toml` | Add `docmux-reader-html` and `docmux-highlight` dependencies |
| `crates/docmux-wasm/src/lib.rs` | Register `HtmlReader`, pass highlight style |

---

### Task 1: Scaffold `docmux-highlight` crate

**Files:**
- Create: `crates/docmux-highlight/Cargo.toml`
- Create: `crates/docmux-highlight/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add `syntect` to workspace deps and register crate**

In `Cargo.toml` (workspace root), add to `[workspace.members]`:
```
"crates/docmux-highlight",
```

Add to `[workspace.dependencies]`:
```toml
docmux-highlight = { path = "crates/docmux-highlight" }
syntect = { version = "5", default-features = false, features = ["default-syntaxes", "default-themes", "html", "regex-onig"] }
```

- [ ] **Step 2: Create crate Cargo.toml**

Create `crates/docmux-highlight/Cargo.toml`:
```toml
[package]
name = "docmux-highlight"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Syntax highlighting for docmux using syntect"
rust-version.workspace = true

[dependencies]
docmux-core = { workspace = true }
syntect = { workspace = true }
```

- [ ] **Step 3: Write failing test for `highlight()`**

Create `crates/docmux-highlight/src/lib.rs`:
```rust
//! # docmux-highlight
//!
//! Syntax highlighting for docmux, backed by syntect.
//! Returns styled tokens per line — writers decide how to render them.

use docmux_core::{ConvertError, Result};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_rust_returns_tokens() {
        let lines = highlight("fn main() {}", "rust", "InspiredGitHub").unwrap();
        assert!(!lines.is_empty());
        assert!(!lines[0].is_empty());
        // "fn" should be a keyword token with some foreground color
        let first = &lines[0][0];
        assert!(first.style.foreground.is_some());
    }

    #[test]
    fn highlight_unknown_language_returns_error() {
        let result = highlight("code", "nonexistent-lang-xyz", "InspiredGitHub");
        assert!(result.is_err());
    }

    #[test]
    fn available_languages_is_nonempty() {
        let langs = available_languages();
        assert!(langs.len() > 50);
        assert!(langs.iter().any(|l| l == "Rust"));
    }

    #[test]
    fn available_themes_is_nonempty() {
        let themes = available_themes();
        assert!(!themes.is_empty());
        assert!(themes.iter().any(|t| t == "InspiredGitHub"));
    }
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo test -p docmux-highlight`
Expected: FAIL — `highlight`, `available_languages`, `available_themes` not defined.

- [ ] **Step 5: Implement types and functions**

Add to the top of `crates/docmux-highlight/src/lib.rs` (before the `#[cfg(test)]` block):

```rust
use std::sync::LazyLock;
use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::SyntaxSet;
use syntect::easy::HighlightLines;

static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

/// An RGB color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Style information for a highlighted token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenStyle {
    pub foreground: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// A single highlighted token with its text and style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightToken {
    pub text: String,
    pub style: TokenStyle,
}

impl From<Style> for TokenStyle {
    fn from(s: Style) -> Self {
        Self {
            foreground: Some(Color {
                r: s.foreground.r,
                g: s.foreground.g,
                b: s.foreground.b,
            }),
            bold: s.font_style.contains(syntect::highlighting::FontStyle::BOLD),
            italic: s.font_style.contains(syntect::highlighting::FontStyle::ITALIC),
            underline: s.font_style.contains(syntect::highlighting::FontStyle::UNDERLINE),
        }
    }
}

/// Highlight `code` as `language` using `theme`. Returns tokens grouped by line.
pub fn highlight(code: &str, language: &str, theme: &str) -> Result<Vec<Vec<HighlightToken>>> {
    let syntax = SYNTAX_SET
        .find_syntax_by_token(language)
        .ok_or_else(|| ConvertError::Unsupported {
            feature: format!("highlight language '{language}'"),
        })?;

    let theme = THEME_SET
        .themes
        .get(theme)
        .ok_or_else(|| ConvertError::Unsupported {
            feature: format!("highlight theme '{theme}'"),
        })?;

    let mut h = HighlightLines::new(syntax, theme);
    let mut result = Vec::new();

    for line in syntect::util::LinesWithEndings::from(code) {
        let ranges = h
            .highlight_line(line, &SYNTAX_SET)
            .map_err(|e| ConvertError::Unsupported {
                feature: format!("highlighting: {e}"),
            })?;

        let tokens: Vec<HighlightToken> = ranges
            .into_iter()
            .map(|(style, text)| HighlightToken {
                text: text.to_string(),
                style: TokenStyle::from(style),
            })
            .collect();

        result.push(tokens);
    }

    Ok(result)
}

/// List available syntax language names.
pub fn available_languages() -> Vec<String> {
    SYNTAX_SET
        .syntaxes()
        .iter()
        .map(|s| s.name.clone())
        .collect()
}

/// List available theme names.
pub fn available_themes() -> Vec<String> {
    THEME_SET.themes.keys().cloned().collect()
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p docmux-highlight`
Expected: all 4 tests PASS.

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -p docmux-highlight --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/docmux-highlight/ Cargo.toml
git commit -m "feat: add docmux-highlight crate with syntect-based highlighting"
```

---

### Task 2: Add `highlight_style` to `WriteOptions`

**Files:**
- Modify: `crates/docmux-core/src/lib.rs`

- [ ] **Step 1: Add field to `WriteOptions`**

In `crates/docmux-core/src/lib.rs`, add to the `WriteOptions` struct (after the `eol` field):

```rust
    /// Syntax highlighting theme name (e.g. `"InspiredGitHub"`).
    /// `None` disables highlighting.
    pub highlight_style: Option<String>,
```

And in the `Default` impl, add:
```rust
            highlight_style: None,
```

- [ ] **Step 2: Verify workspace compiles**

Run: `cargo check --workspace`
Expected: compiles cleanly. All existing code uses `..Default::default()` so the new field is handled.

- [ ] **Step 3: Commit**

```bash
git add crates/docmux-core/src/lib.rs
git commit -m "feat: add highlight_style to WriteOptions"
```

---

### Task 3: Integrate highlighting into HTML writer

**Files:**
- Modify: `crates/docmux-writer-html/Cargo.toml`
- Modify: `crates/docmux-writer-html/src/lib.rs`

- [ ] **Step 1: Add `docmux-highlight` dependency**

In `crates/docmux-writer-html/Cargo.toml`, add:
```toml
docmux-highlight = { workspace = true }
```

- [ ] **Step 2: Write a test for highlighted code block output**

In `crates/docmux-writer-html/src/lib.rs`, add to the test module:

```rust
    #[test]
    fn code_block_with_highlighting() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("rust".into()),
                content: "fn main() {}".into(),
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let opts = WriteOptions {
            highlight_style: Some("InspiredGitHub".into()),
            ..Default::default()
        };
        let html = HtmlWriter::new().write(&doc, &opts).unwrap();
        // Should contain colored spans instead of plain escaped text
        assert!(html.contains("<span style=\""));
        assert!(html.contains("fn"));
        // Should still have the pre/code wrapper
        assert!(html.contains("<pre"));
    }

    #[test]
    fn code_block_highlighting_unknown_lang_falls_back() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("nonexistent-xyz".into()),
                content: "some code".into(),
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let opts = WriteOptions {
            highlight_style: Some("InspiredGitHub".into()),
            ..Default::default()
        };
        let html = HtmlWriter::new().write(&doc, &opts).unwrap();
        // Falls back to plain escaped code
        assert!(html.contains("some code"));
        assert!(html.contains("<pre><code"));
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-html`
Expected: FAIL — new tests fail because highlighting is not yet implemented.

- [ ] **Step 4: Implement highlighted CodeBlock rendering**

In `crates/docmux-writer-html/src/lib.rs`, the `write_block` method needs access to `opts`. Currently the HTML writer passes `opts` around. Modify the `Block::CodeBlock` arm to check `opts.highlight_style`. The writer's `write_block` method signature already receives `opts: &WriteOptions` (verify this, and if not, it's accessible in the `write` method scope).

Replace the existing `Block::CodeBlock` match arm:

```rust
Block::CodeBlock {
    language, content, ..
} => {
    if let (Some(lang), Some(theme)) = (language.as_deref(), opts.highlight_style.as_deref()) {
        if let Ok(lines) = docmux_highlight::highlight(content, lang, theme) {
            out.push_str("<pre><code class=\"language-");
            out.push_str(&escape_html(lang));
            out.push_str("\">");
            for line in &lines {
                for token in line {
                    let s = &token.style;
                    let mut styles = Vec::new();
                    if let Some(c) = s.foreground {
                        styles.push(format!("color:#{:02x}{:02x}{:02x}", c.r, c.g, c.b));
                    }
                    if s.bold {
                        styles.push("font-weight:bold".into());
                    }
                    if s.italic {
                        styles.push("font-style:italic".into());
                    }
                    if s.underline {
                        styles.push("text-decoration:underline".into());
                    }
                    if styles.is_empty() {
                        out.push_str(&escape_html(&token.text));
                    } else {
                        out.push_str("<span style=\"");
                        out.push_str(&styles.join(";"));
                        out.push_str("\">");
                        out.push_str(&escape_html(&token.text));
                        out.push_str("</span>");
                    }
                }
            }
            out.push_str("</code></pre>\n");
            return;
        }
    }
    // Fallback: no highlighting
    if let Some(lang) = language {
        out.push_str(&format!(
            "<pre><code class=\"language-{}\">",
            escape_html(lang)
        ));
    } else {
        out.push_str("<pre><code>");
    }
    out.push_str(&escape_html(content));
    out.push_str("</code></pre>\n");
}
```

Add the import at the top of the file:
```rust
use docmux_highlight;
```

Note: The `write_block` function must have access to `opts: &WriteOptions`. Check the actual signature — if `opts` is not already passed through, thread it from the `write()` method. The current HTML writer calls internal methods; ensure `opts` is available where `Block::CodeBlock` is matched.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-html`
Expected: all tests PASS including the two new ones.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -p docmux-writer-html --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-writer-html/
git commit -m "feat: add syntax highlighting to HTML writer via docmux-highlight"
```

---

### Task 4: Integrate highlighting into LaTeX writer

**Files:**
- Modify: `crates/docmux-writer-latex/Cargo.toml`
- Modify: `crates/docmux-writer-latex/src/lib.rs`

- [ ] **Step 1: Add `docmux-highlight` dependency**

In `crates/docmux-writer-latex/Cargo.toml`, add:
```toml
docmux-highlight = { workspace = true }
```

- [ ] **Step 2: Write a test for highlighted LaTeX code block**

In `crates/docmux-writer-latex/src/lib.rs`, add to the test module:

```rust
    #[test]
    fn code_block_with_highlighting() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("rust".into()),
                content: "fn main() {}".into(),
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let opts = WriteOptions {
            highlight_style: Some("InspiredGitHub".into()),
            ..Default::default()
        };
        let tex = LatexWriter::new().write(&doc, &opts).unwrap();
        // Should contain \textcolor commands
        assert!(tex.contains("\\textcolor[RGB]"));
        assert!(!tex.contains("\\begin{lstlisting}"));
    }

    #[test]
    fn code_block_highlighting_fallback() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("nonexistent-xyz".into()),
                content: "code".into(),
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let opts = WriteOptions {
            highlight_style: Some("InspiredGitHub".into()),
            ..Default::default()
        };
        let tex = LatexWriter::new().write(&doc, &opts).unwrap();
        // Falls back to lstlisting
        assert!(tex.contains("\\begin{lstlisting}"));
    }
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-latex`
Expected: FAIL — new tests fail.

- [ ] **Step 4: Implement highlighted CodeBlock in LaTeX**

Replace the `Block::CodeBlock` arm in the LaTeX writer:

```rust
Block::CodeBlock {
    language, content, ..
} => {
    if let (Some(lang), Some(theme)) = (language.as_deref(), opts.highlight_style.as_deref()) {
        if let Ok(lines) = docmux_highlight::highlight(content, lang, theme) {
            out.push_str("\\begin{alltt}\n");
            for line in &lines {
                for token in line {
                    let s = &token.style;
                    let escaped = latex_escape_verbatim(&token.text);
                    let mut wrapped = escaped.clone();
                    if s.bold {
                        wrapped = format!("\\textbf{{{wrapped}}}");
                    }
                    if s.italic {
                        wrapped = format!("\\textit{{{wrapped}}}");
                    }
                    if let Some(c) = s.foreground {
                        wrapped = format!(
                            "\\textcolor[RGB]{{{},{},{}}}{{{}}}",
                            c.r, c.g, c.b, wrapped
                        );
                    }
                    out.push_str(&wrapped);
                }
            }
            out.push_str("\\end{alltt}\n");
            return;
        }
    }
    // Fallback: no highlighting
    if let Some(lang) = language {
        out.push_str(&format!("\\begin{{lstlisting}}[language={}]\n", lang));
    } else {
        out.push_str("\\begin{lstlisting}\n");
    }
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\\end{lstlisting}\n");
}
```

Add a helper function for escaping text inside `alltt`:
```rust
/// Escape special chars for alltt environment (fewer chars than normal LaTeX).
fn latex_escape_verbatim(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            _ => out.push(c),
        }
    }
    out
}
```

Also ensure `alltt` is in the standalone preamble packages. In the standalone wrapper, add `\usepackage{alltt}` alongside existing packages. And ensure `\usepackage{xcolor}` is present for `\textcolor`.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-latex`
Expected: all tests PASS.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -p docmux-writer-latex --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-writer-latex/
git commit -m "feat: add syntax highlighting to LaTeX writer via docmux-highlight"
```

---

### Task 5: Scaffold `docmux-reader-html` crate with basic block parsing

**Files:**
- Create: `crates/docmux-reader-html/Cargo.toml`
- Create: `crates/docmux-reader-html/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add `scraper` to workspace deps and register crate**

In `Cargo.toml` (workspace root), add to `[workspace.members]`:
```
"crates/docmux-reader-html",
```

Add to `[workspace.dependencies]`:
```toml
docmux-reader-html = { path = "crates/docmux-reader-html" }
scraper = "0.22"
```

- [ ] **Step 2: Create crate Cargo.toml**

Create `crates/docmux-reader-html/Cargo.toml`:
```toml
[package]
name = "docmux-reader-html"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "HTML reader for docmux — parses HTML into the docmux AST"
rust-version.workspace = true

[dependencies]
docmux-ast = { workspace = true }
docmux-core = { workspace = true }
scraper = { workspace = true }
```

- [ ] **Step 3: Write failing tests for Reader trait and basic block elements**

Create `crates/docmux-reader-html/src/lib.rs`:
```rust
//! # docmux-reader-html
//!
//! HTML reader for docmux. Parses semantic HTML (Tier 1) and best-effort
//! general web content (Tier 2) into the docmux AST using scraper/html5ever.

use docmux_ast::{Document, Metadata};
use docmux_core::{Reader, Result};

pub struct HtmlReader;

impl HtmlReader {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HtmlReader {
    fn default() -> Self {
        Self::new()
    }
}

impl Reader for HtmlReader {
    fn format(&self) -> &str {
        "html"
    }

    fn extensions(&self) -> &[&str] {
        &["html", "htm"]
    }

    fn read(&self, _input: &str) -> Result<Document> {
        Ok(Document::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_ast::Block;

    #[test]
    fn reader_trait_metadata() {
        let r = HtmlReader::new();
        assert_eq!(r.format(), "html");
        assert!(r.extensions().contains(&"html"));
        assert!(r.extensions().contains(&"htm"));
    }

    #[test]
    fn parse_paragraph() {
        let r = HtmlReader::new();
        let doc = r.read("<p>Hello world</p>").unwrap();
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::Paragraph { .. }));
    }

    #[test]
    fn parse_headings() {
        let r = HtmlReader::new();
        let doc = r.read("<h1>Title</h1><h2 id=\"sub\">Sub</h2>").unwrap();
        assert_eq!(doc.content.len(), 2);
        assert!(matches!(&doc.content[0], Block::Heading { level: 1, .. }));
        if let Block::Heading { level, id, .. } = &doc.content[1] {
            assert_eq!(*level, 2);
            assert_eq!(id.as_deref(), Some("sub"));
        } else {
            panic!("expected heading");
        }
    }

    #[test]
    fn parse_code_block() {
        let r = HtmlReader::new();
        let doc = r.read("<pre><code class=\"language-python\">print('hi')</code></pre>").unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::CodeBlock { language, content, .. } = &doc.content[0] {
            assert_eq!(language.as_deref(), Some("python"));
            assert_eq!(content, "print('hi')");
        } else {
            panic!("expected code block");
        }
    }

    #[test]
    fn parse_blockquote() {
        let r = HtmlReader::new();
        let doc = r.read("<blockquote><p>quoted</p></blockquote>").unwrap();
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::BlockQuote { .. }));
    }

    #[test]
    fn parse_unordered_list() {
        let r = HtmlReader::new();
        let doc = r.read("<ul><li>a</li><li>b</li></ul>").unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::List { ordered, items, .. } = &doc.content[0] {
            assert!(!ordered);
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn parse_ordered_list() {
        let r = HtmlReader::new();
        let doc = r.read("<ol start=\"3\"><li>c</li><li>d</li></ol>").unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::List { ordered, start, items, .. } = &doc.content[0] {
            assert!(ordered);
            assert_eq!(*start, Some(3));
            assert_eq!(items.len(), 2);
        } else {
            panic!("expected list");
        }
    }

    #[test]
    fn parse_thematic_break() {
        let r = HtmlReader::new();
        let doc = r.read("<hr>").unwrap();
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::ThematicBreak));
    }

    #[test]
    fn parse_definition_list() {
        let r = HtmlReader::new();
        let doc = r.read("<dl><dt>Term</dt><dd>Definition</dd></dl>").unwrap();
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::DefinitionList { .. }));
    }
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-html`
Expected: FAIL — `parse_paragraph` and others fail because `read()` returns empty document.

- [ ] **Step 5: Implement block-level parsing**

Replace the `read` method and add block conversion logic. This is the core of the reader — parse the HTML DOM and walk it to produce AST blocks:

```rust
use scraper::{Html, Node, ElementRef};
use docmux_ast::{
    Block, Document, Inline, ListItem, Metadata, Author, DefinitionItem,
    Table, TableCell, ColumnSpec, Alignment, Image, Attributes,
};

impl Reader for HtmlReader {
    fn format(&self) -> &str {
        "html"
    }

    fn extensions(&self) -> &[&str] {
        &["html", "htm"]
    }

    fn read(&self, input: &str) -> Result<Document> {
        let html = Html::parse_document(input);
        let mut metadata = Metadata::default();

        // Try to extract metadata from <head>
        if let Some(head) = html.select(&selector("head")).next() {
            extract_metadata(&head, &mut metadata);
        }

        // Find <body> or fall back to document root
        let root = html
            .select(&selector("body"))
            .next()
            .map(|e| e.id())
            .unwrap_or_else(|| html.root_element().id());

        let root_ref = ElementRef::wrap(html.tree.get(root).unwrap()).unwrap_or(html.root_element());
        let blocks = convert_children_to_blocks(&root_ref);

        Ok(Document {
            metadata,
            content: blocks,
            ..Default::default()
        })
    }
}

fn selector(s: &str) -> scraper::Selector {
    scraper::Selector::parse(s).unwrap()
}

fn extract_metadata(head: &ElementRef, meta: &mut Metadata) {
    for el in head.select(&selector("title")) {
        meta.title = Some(el.text().collect::<String>().trim().to_string());
    }
    for el in head.select(&selector("meta")) {
        let name = el.value().attr("name").unwrap_or("");
        let content = el.value().attr("content").unwrap_or("");
        match name {
            "author" => {
                meta.authors.push(Author {
                    name: content.to_string(),
                    affiliation: None,
                    email: None,
                    orcid: None,
                });
            }
            "date" => meta.date = Some(content.to_string()),
            "keywords" => {
                meta.keywords = content.split(',').map(|k| k.trim().to_string()).collect();
            }
            "description" => {
                meta.custom.insert(
                    "description".to_string(),
                    docmux_ast::MetaValue::String(content.to_string()),
                );
            }
            _ => {}
        }
    }
}

fn convert_children_to_blocks(parent: &ElementRef) -> Vec<Block> {
    let mut blocks = Vec::new();
    for child in parent.children() {
        match child.value() {
            Node::Element(_) => {
                if let Some(el) = ElementRef::wrap(child) {
                    if let Some(block) = convert_element_to_block(&el) {
                        blocks.push(block);
                    }
                }
            }
            Node::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    blocks.push(Block::Paragraph {
                        content: vec![Inline::Text {
                            value: trimmed.to_string(),
                        }],
                    });
                }
            }
            _ => {}
        }
    }
    blocks
}

fn convert_element_to_block(el: &ElementRef) -> Option<Block> {
    let tag = el.value().name();
    match tag {
        "p" => Some(Block::Paragraph {
            content: convert_children_to_inlines(el),
        }),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = tag[1..].parse::<u8>().unwrap_or(1);
            let id = el.value().attr("id").map(String::from);
            Some(Block::Heading {
                level,
                id,
                content: convert_children_to_inlines(el),
                attrs: extract_attrs(el),
            })
        }
        "pre" => {
            // Look for <code> child
            if let Some(code_el) = el.select(&selector("code")).next() {
                let language = code_el
                    .value()
                    .attr("class")
                    .and_then(|c| {
                        c.split_whitespace()
                            .find(|cls| cls.starts_with("language-"))
                            .map(|cls| cls.strip_prefix("language-").unwrap().to_string())
                    });
                let content = code_el.text().collect::<String>();
                Some(Block::CodeBlock {
                    language,
                    content,
                    caption: None,
                    label: None,
                    attrs: None,
                })
            } else {
                let content = el.text().collect::<String>();
                Some(Block::CodeBlock {
                    language: None,
                    content,
                    caption: None,
                    label: None,
                    attrs: None,
                })
            }
        }
        "blockquote" => Some(Block::BlockQuote {
            content: convert_children_to_blocks(el),
        }),
        "ul" => {
            let items = list_items(el);
            Some(Block::List {
                ordered: false,
                start: None,
                items,
                tight: false,
                style: None,
                delimiter: None,
            })
        }
        "ol" => {
            let start = el
                .value()
                .attr("start")
                .and_then(|s| s.parse::<u32>().ok());
            let items = list_items(el);
            Some(Block::List {
                ordered: true,
                start: start.or(Some(1)),
                items,
                tight: false,
                style: None,
                delimiter: None,
            })
        }
        "hr" => Some(Block::ThematicBreak),
        "table" => Some(convert_table(el)),
        "figure" => convert_figure(el),
        "img" => {
            let image = convert_img(el);
            Some(Block::Paragraph {
                content: vec![Inline::Image(image)],
            })
        }
        "dl" => convert_definition_list(el),
        "div" => Some(Block::Div {
            attrs: extract_attrs(el).unwrap_or_default(),
            content: convert_children_to_blocks(el),
        }),
        "section" | "article" | "aside" | "nav" | "main" | "header" | "footer" => {
            let mut attrs = extract_attrs(el).unwrap_or_default();
            attrs.classes.push(tag.to_string());
            Some(Block::Div {
                attrs,
                content: convert_children_to_blocks(el),
            })
        }
        // Ignored elements
        "script" | "style" | "noscript" | "link" | "meta" | "head" | "title" => None,
        // Unknown: unwrap children
        _ => {
            let blocks = convert_children_to_blocks(el);
            if blocks.len() == 1 {
                Some(blocks.into_iter().next().unwrap())
            } else if blocks.is_empty() {
                // Try as inline content wrapped in paragraph
                let inlines = convert_children_to_inlines(el);
                if inlines.is_empty() {
                    None
                } else {
                    Some(Block::Paragraph { content: inlines })
                }
            } else {
                Some(Block::Div {
                    attrs: Attributes::default(),
                    content: blocks,
                })
            }
        }
    }
}

fn list_items(el: &ElementRef) -> Vec<ListItem> {
    el.select(&selector("li"))
        .map(|li| ListItem {
            content: convert_children_to_blocks(&li),
            checked: None,
        })
        .collect()
}

fn extract_attrs(el: &ElementRef) -> Option<Attributes> {
    let id = el.value().attr("id").map(String::from);
    let classes: Vec<String> = el
        .value()
        .attr("class")
        .map(|c| c.split_whitespace().map(String::from).collect())
        .unwrap_or_default();
    if id.is_none() && classes.is_empty() {
        None
    } else {
        Some(Attributes {
            id,
            classes,
            key_values: std::collections::HashMap::new(),
        })
    }
}
```

Note: `convert_children_to_inlines`, `convert_table`, `convert_figure`, `convert_img`, `convert_definition_list` are stubs for now — they'll be implemented in later tasks. For this task, add minimal stubs so the block tests pass:

```rust
fn convert_children_to_inlines(el: &ElementRef) -> Vec<Inline> {
    let mut inlines = Vec::new();
    for child in el.children() {
        match child.value() {
            Node::Text(text) => {
                if !text.is_empty() {
                    inlines.push(Inline::Text {
                        value: text.to_string(),
                    });
                }
            }
            Node::Element(_) => {
                if let Some(child_el) = ElementRef::wrap(child) {
                    let mut child_inlines = convert_element_to_inlines(&child_el);
                    inlines.append(&mut child_inlines);
                }
            }
            _ => {}
        }
    }
    inlines
}

fn convert_element_to_inlines(el: &ElementRef) -> Vec<Inline> {
    // Minimal stub — expanded in Task 6
    let tag = el.value().name();
    match tag {
        "em" | "i" => vec![Inline::Emphasis {
            content: convert_children_to_inlines(el),
        }],
        "strong" | "b" => vec![Inline::Strong {
            content: convert_children_to_inlines(el),
        }],
        "code" => vec![Inline::Code {
            value: el.text().collect(),
            attrs: None,
        }],
        "a" => {
            let url = el.value().attr("href").unwrap_or("").to_string();
            let title = el.value().attr("title").map(String::from);
            vec![Inline::Link {
                url,
                title,
                content: convert_children_to_inlines(el),
                attrs: None,
            }]
        }
        "br" => vec![Inline::HardBreak],
        _ => convert_children_to_inlines(el),
    }
}

fn convert_table(el: &ElementRef) -> Block {
    let mut header = None;
    let mut rows = Vec::new();
    let mut foot = None;

    if let Some(thead) = el.select(&selector("thead")).next() {
        if let Some(tr) = thead.select(&selector("tr")).next() {
            header = Some(convert_table_row(&tr));
        }
    }

    let tbody_selector = selector("tbody");
    let tbodies: Vec<_> = el.select(&tbody_selector).collect();
    if tbodies.is_empty() {
        // No tbody — rows are direct children of table
        for tr in el.select(&selector("tr")) {
            // Skip header row if already captured
            if header.is_some() && tr.parent().and_then(|p| ElementRef::wrap(p)).map(|p| p.value().name()) == Some("thead") {
                continue;
            }
            rows.push(convert_table_row(&tr));
        }
    } else {
        for tbody in &tbodies {
            for tr in tbody.select(&selector("tr")) {
                rows.push(convert_table_row(&tr));
            }
        }
    }

    if let Some(tfoot) = el.select(&selector("tfoot")).next() {
        if let Some(tr) = tfoot.select(&selector("tr")).next() {
            foot = Some(convert_table_row(&tr));
        }
    }

    let num_cols = header
        .as_ref()
        .map(|h| h.len())
        .or_else(|| rows.first().map(|r| r.len()))
        .unwrap_or(0);

    Block::Table(Table {
        caption: None,
        label: None,
        columns: (0..num_cols)
            .map(|_| ColumnSpec {
                alignment: Alignment::Default,
                width: None,
            })
            .collect(),
        header,
        rows,
        foot,
        attrs: extract_attrs(el),
    })
}

fn convert_table_row(tr: &ElementRef) -> Vec<TableCell> {
    tr.children()
        .filter_map(ElementRef::wrap)
        .filter(|el| matches!(el.value().name(), "td" | "th"))
        .map(|cell| {
            let colspan = cell
                .value()
                .attr("colspan")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
            let rowspan = cell
                .value()
                .attr("rowspan")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1);
            TableCell {
                content: convert_children_to_blocks(&cell),
                colspan,
                rowspan,
            }
        })
        .collect()
}

fn convert_figure(el: &ElementRef) -> Option<Block> {
    let img_el = el.select(&selector("img")).next()?;
    let image = convert_img(&img_el);
    let caption = el
        .select(&selector("figcaption"))
        .next()
        .map(|cap| convert_children_to_inlines(&cap));
    Some(Block::Figure {
        image,
        caption,
        label: el.value().attr("id").map(String::from),
        attrs: extract_attrs(el),
    })
}

fn convert_img(el: &ElementRef) -> Image {
    Image {
        url: el.value().attr("src").unwrap_or("").to_string(),
        title: el.value().attr("title").map(String::from),
        alt: el
            .value()
            .attr("alt")
            .map(|a| vec![Inline::Text { value: a.to_string() }])
            .unwrap_or_default(),
        attrs: None,
    }
}

fn convert_definition_list(el: &ElementRef) -> Option<Block> {
    let mut items = Vec::new();
    let mut current_terms: Vec<Vec<Inline>> = Vec::new();

    for child in el.children().filter_map(ElementRef::wrap) {
        match child.value().name() {
            "dt" => {
                current_terms.push(convert_children_to_inlines(&child));
            }
            "dd" => {
                let term = if current_terms.is_empty() {
                    Vec::new()
                } else {
                    current_terms.remove(0)
                };
                items.push(DefinitionItem {
                    term,
                    definitions: convert_children_to_blocks(&child),
                });
            }
            _ => {}
        }
    }

    if items.is_empty() {
        None
    } else {
        Some(Block::DefinitionList { items })
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p docmux-reader-html`
Expected: all 9 tests PASS.

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -p docmux-reader-html --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 8: Commit**

```bash
git add crates/docmux-reader-html/ Cargo.toml
git commit -m "feat: add docmux-reader-html crate with block-level parsing"
```

---

### Task 6: Complete inline element parsing in HTML reader

**Files:**
- Modify: `crates/docmux-reader-html/src/lib.rs`

- [ ] **Step 1: Write failing tests for inline elements**

Add to the test module:

```rust
    #[test]
    fn parse_inline_emphasis() {
        let r = HtmlReader::new();
        let doc = r.read("<p><em>italic</em> and <i>also italic</i></p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[0], Inline::Emphasis { .. }));
            assert!(matches!(&content[2], Inline::Emphasis { .. }));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_inline_strong() {
        let r = HtmlReader::new();
        let doc = r.read("<p><strong>bold</strong></p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[0], Inline::Strong { .. }));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_inline_strikethrough() {
        let r = HtmlReader::new();
        let doc = r.read("<p><del>deleted</del> and <s>struck</s></p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[0], Inline::Strikethrough { .. }));
            assert!(matches!(&content[2], Inline::Strikethrough { .. }));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_inline_underline() {
        let r = HtmlReader::new();
        let doc = r.read("<p><u>underlined</u></p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[0], Inline::Underline { .. }));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_inline_code() {
        let r = HtmlReader::new();
        let doc = r.read("<p>Use <code>fn main()</code> here</p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[1], Inline::Code { .. }));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_inline_link() {
        let r = HtmlReader::new();
        let doc = r.read("<p><a href=\"https://example.com\" title=\"Ex\">link</a></p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            if let Inline::Link { url, title, .. } = &content[0] {
                assert_eq!(url, "https://example.com");
                assert_eq!(title.as_deref(), Some("Ex"));
            } else {
                panic!("expected link");
            }
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_inline_image() {
        let r = HtmlReader::new();
        let doc = r.read("<p><img src=\"photo.jpg\" alt=\"A photo\"></p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[0], Inline::Image(_)));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_sub_sup() {
        let r = HtmlReader::new();
        let doc = r.read("<p>H<sub>2</sub>O is x<sup>2</sup></p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[1], Inline::Subscript { .. }));
            assert!(matches!(&content[3], Inline::Superscript { .. }));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_span_with_attrs() {
        let r = HtmlReader::new();
        let doc = r.read("<p><span class=\"highlight\" id=\"s1\">text</span></p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            if let Inline::Span { attrs, .. } = &content[0] {
                assert_eq!(attrs.id.as_deref(), Some("s1"));
                assert!(attrs.classes.contains(&"highlight".to_string()));
            } else {
                panic!("expected span");
            }
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_hard_break() {
        let r = HtmlReader::new();
        let doc = r.read("<p>line1<br>line2</p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(content.iter().any(|i| matches!(i, Inline::HardBreak)));
        } else {
            panic!("expected paragraph");
        }
    }
```

- [ ] **Step 2: Run tests to see which fail**

Run: `cargo test -p docmux-reader-html`
Expected: some new tests fail (strikethrough, underline, sub, sup, span, image inline).

- [ ] **Step 3: Expand `convert_element_to_inlines`**

Replace the minimal `convert_element_to_inlines` function with the full version:

```rust
fn convert_element_to_inlines(el: &ElementRef) -> Vec<Inline> {
    let tag = el.value().name();
    match tag {
        "em" | "i" => vec![Inline::Emphasis {
            content: convert_children_to_inlines(el),
        }],
        "strong" | "b" => vec![Inline::Strong {
            content: convert_children_to_inlines(el),
        }],
        "del" | "s" => vec![Inline::Strikethrough {
            content: convert_children_to_inlines(el),
        }],
        "u" => vec![Inline::Underline {
            content: convert_children_to_inlines(el),
        }],
        "code" => vec![Inline::Code {
            value: el.text().collect(),
            attrs: None,
        }],
        "a" => {
            let url = el.value().attr("href").unwrap_or("").to_string();
            let title = el.value().attr("title").map(String::from);
            vec![Inline::Link {
                url,
                title,
                content: convert_children_to_inlines(el),
                attrs: None,
            }]
        }
        "img" => {
            vec![Inline::Image(convert_img(el))]
        }
        "sub" => vec![Inline::Subscript {
            content: convert_children_to_inlines(el),
        }],
        "sup" => vec![Inline::Superscript {
            content: convert_children_to_inlines(el),
        }],
        "br" => vec![Inline::HardBreak],
        "span" => {
            let attrs = extract_attrs(el).unwrap_or_default();
            vec![Inline::Span {
                content: convert_children_to_inlines(el),
                attrs,
            }]
        }
        // Unknown inline: unwrap children
        _ => convert_children_to_inlines(el),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-reader-html`
Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-reader-html/src/lib.rs
git commit -m "feat: complete inline element parsing in HTML reader"
```

---

### Task 7: Add full document and metadata parsing to HTML reader

**Files:**
- Modify: `crates/docmux-reader-html/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add to the test module:

```rust
    #[test]
    fn parse_full_document_metadata() {
        let r = HtmlReader::new();
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>My Document</title>
    <meta name="author" content="Jane Doe">
    <meta name="date" content="2026-03-26">
    <meta name="keywords" content="rust, docmux, html">
</head>
<body>
    <h1>Hello</h1>
    <p>World</p>
</body>
</html>"#;
        let doc = r.read(html).unwrap();
        assert_eq!(doc.metadata.title.as_deref(), Some("My Document"));
        assert_eq!(doc.metadata.authors.len(), 1);
        assert_eq!(doc.metadata.authors[0].name, "Jane Doe");
        assert_eq!(doc.metadata.date.as_deref(), Some("2026-03-26"));
        assert_eq!(doc.metadata.keywords, vec!["rust", "docmux", "html"]);
        assert_eq!(doc.content.len(), 2);
    }

    #[test]
    fn parse_fragment() {
        let r = HtmlReader::new();
        let doc = r.read("<h2>Section</h2><p>Text</p>").unwrap();
        assert_eq!(doc.content.len(), 2);
        // No metadata from fragments
        assert!(doc.metadata.title.is_none());
    }

    #[test]
    fn parse_figure() {
        let r = HtmlReader::new();
        let doc = r.read(r#"<figure id="fig1"><img src="img.png" alt="desc"><figcaption>Caption</figcaption></figure>"#).unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::Figure { image, caption, label, .. } = &doc.content[0] {
            assert_eq!(image.url, "img.png");
            assert!(caption.is_some());
            assert_eq!(label.as_deref(), Some("fig1"));
        } else {
            panic!("expected figure");
        }
    }

    #[test]
    fn parse_table_with_header_and_body() {
        let r = HtmlReader::new();
        let doc = r.read(r#"<table>
            <thead><tr><th>Name</th><th>Age</th></tr></thead>
            <tbody><tr><td>Alice</td><td>30</td></tr></tbody>
        </table>"#).unwrap();
        if let Block::Table(table) = &doc.content[0] {
            assert!(table.header.is_some());
            assert_eq!(table.header.as_ref().unwrap().len(), 2);
            assert_eq!(table.rows.len(), 1);
        } else {
            panic!("expected table");
        }
    }

    #[test]
    fn ignored_elements_produce_nothing() {
        let r = HtmlReader::new();
        let doc = r.read("<script>alert('x')</script><style>body{}</style><p>kept</p>").unwrap();
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::Paragraph { .. }));
    }

    #[test]
    fn unknown_tags_unwrap_children() {
        let r = HtmlReader::new();
        let doc = r.read("<custom-tag><p>inner</p></custom-tag>").unwrap();
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::Paragraph { .. }));
    }

    #[test]
    fn semantic_tags_become_divs() {
        let r = HtmlReader::new();
        let doc = r.read("<section><p>content</p></section>").unwrap();
        if let Block::Div { attrs, content } = &doc.content[0] {
            assert!(attrs.classes.contains(&"section".to_string()));
            assert_eq!(content.len(), 1);
        } else {
            panic!("expected div");
        }
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p docmux-reader-html`
Expected: most or all new tests PASS (the block conversion already handles these). If any fail, fix the specific issue.

- [ ] **Step 3: Fix any failing tests and run clippy**

Run: `cargo clippy -p docmux-reader-html --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-reader-html/src/lib.rs
git commit -m "feat: add full document, metadata, and edge case handling to HTML reader"
```

---

### Task 8: Register HTML reader in CLI and add highlight CLI flags

**Files:**
- Modify: `crates/docmux-cli/Cargo.toml`
- Modify: `crates/docmux-cli/src/main.rs`

- [ ] **Step 1: Add dependencies to CLI Cargo.toml**

In `crates/docmux-cli/Cargo.toml`, add to `[dependencies]`:
```toml
docmux-reader-html = { workspace = true }
docmux-highlight = { workspace = true }
```

- [ ] **Step 2: Register HtmlReader and add CLI args**

In `crates/docmux-cli/src/main.rs`:

Add to imports:
```rust
use docmux_reader_html::HtmlReader;
```

In `build_registry()`, add after the other readers:
```rust
    reg.add_reader(Box::new(HtmlReader::new()));
```

Add CLI args to `struct Cli` (after the `quiet` field):
```rust
    /// Syntax highlighting theme for code blocks (e.g. "InspiredGitHub")
    #[arg(long, value_name = "STYLE")]
    highlight_style: Option<String>,

    /// List available syntax highlighting themes and exit
    #[arg(long)]
    list_highlight_themes: bool,

    /// List available syntax highlighting languages and exit
    #[arg(long)]
    list_highlight_languages: bool,
```

Update `required_unless_present_any` on the `input` field to include the new list args:
```rust
    #[arg(required_unless_present_any = ["list_input_formats", "list_output_formats", "list_highlight_themes", "list_highlight_languages"])]
    input: Vec<PathBuf>,
```

Add handling right after the `list_output_formats` block:
```rust
    if cli.list_highlight_themes {
        for theme in docmux_highlight::available_themes() {
            println!("{theme}");
        }
        return;
    }
    if cli.list_highlight_languages {
        for lang in docmux_highlight::available_languages() {
            println!("{lang}");
        }
        return;
    }
```

In the `WriteOptions` construction (around line 318), add:
```rust
        highlight_style: cli.highlight_style.clone(),
```

- [ ] **Step 3: Run existing CLI tests**

Run: `cargo test -p docmux-cli`
Expected: all existing tests PASS.

- [ ] **Step 4: Test manually**

Run: `cargo run -p docmux-cli -- --list-highlight-themes`
Expected: prints theme names (InspiredGitHub, base16-ocean.dark, etc.)

Run: `cargo run -p docmux-cli -- --list-highlight-languages`
Expected: prints language names (Rust, Python, JavaScript, etc.)

Run: `echo '<h1>Hello</h1><p>World</p>' | cargo run -p docmux-cli -- -f html -t markdown -`
Expected: prints `# Hello` and `World` in Markdown.

Run: `echo '```rust\nfn main() {}\n```' | cargo run -p docmux-cli -- -f md -t html --highlight-style=InspiredGitHub -`
Expected: HTML with `<span style="color:...">` tokens in the code block.

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-cli/
git commit -m "feat: register HTML reader in CLI, add --highlight-style and list flags"
```

---

### Task 9: Register HTML reader and highlighting in WASM crate

**Files:**
- Modify: `crates/docmux-wasm/Cargo.toml`
- Modify: `crates/docmux-wasm/src/lib.rs`

- [ ] **Step 1: Add dependencies**

In `crates/docmux-wasm/Cargo.toml`, add:
```toml
docmux-reader-html = { workspace = true }
docmux-highlight = { workspace = true }
```

- [ ] **Step 2: Register reader and pass highlight option**

In `crates/docmux-wasm/src/lib.rs`:

Add imports:
```rust
use docmux_reader_html::HtmlReader;
```

In `build_registry()`, add after other readers:
```rust
    reg.add_reader(Box::new(HtmlReader::new()));
```

In the `convert_inner` function, update the `WriteOptions` to pass highlighting for HTML output:
```rust
    let opts = WriteOptions {
        standalone,
        highlight_style: if to == "html" {
            Some("InspiredGitHub".into())
        } else {
            None
        },
        ..Default::default()
    };
```

- [ ] **Step 3: Verify WASM builds**

Run: `cargo build --target wasm32-unknown-unknown -p docmux-wasm`
Expected: builds successfully. Note: `syntect` must compile for WASM. If there are issues with the `onig` regex backend, switch to the `fancy-regex` feature in the workspace Cargo.toml:

```toml
syntect = { version = "5", default-features = false, features = ["default-syntaxes", "default-themes", "html", "regex-fancy"] }
```

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-wasm/
git commit -m "feat: register HTML reader and highlighting in WASM crate"
```

---

### Task 10: Add golden test fixtures for HTML reader

**Files:**
- Create: `tests/fixtures/basic/simple.html`
- Create: (expectation files auto-generated on first run)
- Modify: `crates/docmux-cli/tests/golden.rs` (if needed to add HTML→format test functions)

- [ ] **Step 1: Create HTML fixture**

Create `tests/fixtures/basic/simple.html`:
```html
<!DOCTYPE html>
<html>
<head>
    <title>Test Document</title>
    <meta name="author" content="Test Author">
</head>
<body>
    <h1>Introduction</h1>
    <p>This is a <strong>bold</strong> and <em>italic</em> test.</p>
    <h2 id="code">Code Example</h2>
    <pre><code class="language-python">def hello():
    print("world")</code></pre>
    <blockquote><p>A wise quote.</p></blockquote>
    <ul>
        <li>Item one</li>
        <li>Item two</li>
    </ul>
    <hr>
    <p>A <a href="https://example.com">link</a> and <code>inline code</code>.</p>
</body>
</html>
```

- [ ] **Step 2: Add golden test function for HTML→Markdown**

Check if the existing golden test harness already discovers `.html` files. If not, add a test function in `crates/docmux-cli/tests/golden.rs` similar to `golden_md_to_html` but for HTML input:

```rust
#[test]
fn golden_html_to_md() {
    let base = fixtures_dir();
    let fixtures: Vec<_> = discover_fixtures_with_ext(&base, "html");

    let mut failures = Vec::new();
    let mut generated = 0u32;

    for fixture_path in &fixtures {
        let name = test_name(fixture_path, &base);
        let expected_path = fixture_path.with_extension("html.md");

        let input = std::fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read: {e}"));

        let reader = docmux_reader_html::HtmlReader::new();
        let doc = reader.read(&input).unwrap();
        let actual = docmux_writer_markdown::MarkdownWriter::new()
            .write(&doc, &WriteOptions::default())
            .unwrap();

        if update_mode() {
            std::fs::write(&expected_path, &actual).unwrap();
            generated += 1;
            continue;
        }

        if !expected_path.exists() {
            std::fs::write(&expected_path, &actual).unwrap();
            generated += 1;
            eprintln!("  generated: {name}");
            continue;
        }

        let expected = std::fs::read_to_string(&expected_path).unwrap();
        if actual != expected {
            failures.push(format!("MISMATCH: {name}"));
        }
    }

    if generated > 0 {
        eprintln!("  {} expectation(s) generated — review and commit.", generated);
    }
    if !failures.is_empty() {
        panic!("{} golden file(s) mismatched: {:?}", failures.len(), failures);
    }
}
```

Add a helper `discover_fixtures_with_ext` if needed, or modify the existing `discover_fixtures` to accept an extension parameter.

- [ ] **Step 3: Run golden tests to generate expectation files**

Run: `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden golden_html`
Expected: generates expectation files.

- [ ] **Step 4: Review generated expectations**

Read the generated `.html.md` file and verify it looks correct — headings, paragraphs, code blocks, lists should be properly converted.

- [ ] **Step 5: Run golden tests normally**

Run: `cargo test -p docmux-cli --test golden`
Expected: all golden tests PASS.

- [ ] **Step 6: Commit**

```bash
git add tests/fixtures/basic/simple.html tests/fixtures/basic/simple.html.md crates/docmux-cli/tests/golden.rs
git commit -m "test: add golden test fixtures for HTML reader"
```

---

### Task 11: Full workspace verification

**Files:** None (verification only)

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace`
Expected: all tests PASS across all 21 crates.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --all -- --check`
Expected: no formatting issues.

- [ ] **Step 4: Build WASM**

Run: `cargo build --target wasm32-unknown-unknown -p docmux-wasm`
Expected: builds cleanly.

- [ ] **Step 5: Update ROADMAP.md**

Mark the completed items:
- `[x] HTML reader — web content, HTML→LaTeX`
- `[x] Syntax highlighting via syntect` (or add it if not listed)
- `[x] --highlight-style=STYLE`
- `[x] --list-highlight-themes`, `--list-highlight-languages` (add to CLI section)

- [ ] **Step 6: Commit**

```bash
git add ROADMAP.md
git commit -m "docs: update roadmap with completed HTML reader and syntax highlighting"
```
