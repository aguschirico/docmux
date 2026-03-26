# HTML Reader + Syntax Highlighting — Design Spec

> Date: 2026-03-26

## Overview

Two new crates:
- **`docmux-reader-html`** — Parse HTML into docmux AST (semantic-first, best-effort for general web content)
- **`docmux-highlight`** — Shared syntax highlighting library using `syntect`, consumed by writers

## 1. `docmux-reader-html`

### Dependencies

- `scraper` (wrapper over `html5ever`) — robust HTML5 parsing with ergonomic DOM traversal
- `docmux-ast`, `docmux-core`

### Reader implementation

- `HtmlReader` implements `Reader` trait
  - `format()` → `"html"`
  - `extensions()` → `["html", "htm"]`
- `read(input)` parses with `scraper::Html`
- Detects document vs fragment:
  - Document (`<html>`): extracts `<title>` → `Metadata.title`, `<meta>` tags → metadata, walks `<body>`
  - Fragment: parses directly as body content

### Tag mapping — Tier 1 (semantic, full fidelity)

| HTML | AST |
|------|-----|
| `<h1>`-`<h6>` | `Heading` (level, id from attribute) |
| `<p>` | `Paragraph` |
| `<pre><code class="language-X">` | `CodeBlock { language: Some("X") }` |
| `<pre><code>` | `CodeBlock { language: None }` |
| `<blockquote>` | `BlockQuote` |
| `<ul>`, `<ol>` + `<li>` | `List` (ordered/unordered, start attr) |
| `<table>` + `<thead>/<tbody>/<tfoot>/<tr>/<td>/<th>` | `Table` |
| `<figure>` + `<img>` + `<figcaption>` | `Figure` |
| `<img>` (standalone) | `Paragraph` with `Image` inline |
| `<hr>` | `ThematicBreak` |
| `<dl>/<dt>/<dd>` | `DefinitionList` |
| `<em>/<i>` | `Emphasis` |
| `<strong>/<b>` | `Strong` |
| `<del>/<s>` | `Strikethrough` |
| `<u>` | `Underline` |
| `<code>` (inline) | `Code` |
| `<a>` | `Link` |
| `<sub>` | `Subscript` |
| `<sup>` | `Superscript` |
| `<br>` | `HardBreak` |
| `<span>` with attributes | `Span` with `Attributes` |

### Tag mapping — Tier 2 (best-effort for general HTML)

- `<div>` with class/id → `Div` with `Attributes`
- `<section>/<article>/<aside>/<nav>` → `Div` (preserving tag name as class)
- `<script>`, `<style>`, `<noscript>` → ignored
- Unknown tags → unwrap children (content is not lost)

## 2. `docmux-highlight`

### Dependencies

- `syntect` — Sublime Text grammars, 300+ languages built-in
- `docmux-core` (for `Result` type only)

### Public API

```rust
pub struct HighlightToken {
    pub text: String,
    pub style: TokenStyle,
}

pub struct TokenStyle {
    pub foreground: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Highlight source code. Returns tokens grouped by line.
pub fn highlight(code: &str, language: &str, theme: &str) -> Result<Vec<Vec<HighlightToken>>>

/// List available language names.
pub fn available_languages() -> Vec<String>

/// List available theme names.
pub fn available_themes() -> Vec<String>
```

### Internal design

- Lazy-static `SyntaxSet` and `ThemeSet` from syntect defaults (loaded once)
- `highlight()` looks up syntax by name or extension; returns error if not found (writer can fallback)
- Default theme: `"InspiredGitHub"` (light), configurable via `WriteOptions`

## 3. Writer integration

### `WriteOptions` addition

```rust
pub highlight_style: Option<String>,  // None = no highlighting, Some("theme") = highlight
```

### HTML writer

- Calls `highlight()`, renders `<span style="color:#RRGGBB;font-weight:bold">` per token
- Fallback to current `<pre><code>` if highlight fails or `highlight_style` is None

### LaTeX writer

- Calls `highlight()`, renders `\textcolor[RGB]{r,g,b}{\textbf{...}}` per token
- Fallback to current `\begin{verbatim}` if highlight fails or `highlight_style` is None

### Other writers

- Typst, Markdown, Plaintext: no highlighting (Typst has native; Markdown/plaintext are plain text)

## 4. CLI additions

- `--highlight-style=STYLE` — set theme name (default: no highlighting)
- `--list-highlight-themes` — list available themes
- `--list-highlight-languages` — list available languages

## 5. WASM / Playground

- `docmux-highlight` added as WASM dependency
- Playground uses default highlighting for HTML preview output (inline styles, no external CSS)
- Theme hardcoded initially; can be exposed as option later

## 6. Testing strategy

| Crate | Tests |
|-------|-------|
| `docmux-reader-html` | Unit: each tag → AST mapping. Integration: full documents with metadata. Golden files: `.html` → `.md`, `.html` → `.tex`. Edge cases: malformed HTML, fragments, nested structures. |
| `docmux-highlight` | Unit: highlight known snippets (Python, Rust, JS) → non-empty tokens. Theme/language listing. Unknown language → error. |
| HTML writer (updated) | New golden files with highlighting enabled. Tests with and without highlighting. Fallback on unknown language. |
| LaTeX writer (updated) | Same: tests with highlighting enabled, fallback behavior. |

## 7. Implementation order

1. `docmux-highlight` (no dependencies on other new crates)
2. Integrate highlight in HTML writer + LaTeX writer
3. `docmux-reader-html` (independent of highlight)
4. Integrate reader in CLI + WASM
5. CLI flags (`--highlight-style`, `--list-highlight-*`)
