# docmux-reader-typst — Design Spec

> Date: 2026-03-24
> Status: Approved
> Crate: `docmux-reader-typst`

## Goal

Parse a practical subset of Typst markup into the docmux AST. Same philosophy as the LaTeX reader: best-effort with warnings, not a full Typst compiler. Targets academic papers and technical documents.

**Use cases:**
- Convert Typst documents to HTML, LaTeX, Markdown
- Roundtrip fidelity with the future `docmux-writer-typst`
- Migrate existing Typst documents to other formats (lossy is acceptable)

## Design Decisions

1. **Recursive descent parser** — hand-written lexer + parser (same pattern as LaTeX reader). Typst is more regular than LaTeX, so the lexer is simpler.
2. **Best-effort + warnings** — unrecognized `#func()` calls emit `RawBlock`/`RawInline { format: "typst" }` with `ParseWarning`. The document always parses successfully.
3. **Dual metadata strategy** — extract metadata from both:
   - YAML frontmatter (delimited by `---`) at the top of the file — same as Markdown reader
   - `#set document(title: ..., author: ..., date: ...)` — Typst-native
   - Priority: YAML first, `#set document()` fields fill in anything missing
4. **Silently ignored directives** — `#set` (except `document`), `#show`, `#let`, `#import` are consumed without output or warning. These are styling/layout directives that don't map to document content.
5. **Content blocks** — Typst's `[...]` content blocks are parsed recursively as inline/block content depending on context.
6. **Code mode** — Typst's `{...}` code blocks in content position are emitted as `RawBlock { format: "typst" }` since we don't evaluate Typst code.

## Architecture

```
Input &str
   │
   ▼
+----------+   Vec<Token>   +----------+
|  Lexer   | ────────────>  |  Parser  | ───────> Document
| lexer.rs |                | parser.rs|
+----------+                +----------+
                                 │
                            Metadata from
                            YAML + #set document()
```

### Lexer (`lexer.rs`)

Converts input `&str` into `Vec<Token>`. Character-by-character scanner with line tracking.

| Token | Matches |
|-------|---------|
| `Heading { level, line }` | `=`, `==`, `===`, etc. at line start followed by space |
| `Text { value }` | Accumulated plain text |
| `Star` | `*` (bold delimiter) |
| `Underscore` | `_` (italic delimiter) |
| `Backtick { count }` | `` ` `` or `` ``` `` (code delimiter) |
| `Hash` | `#` (function call prefix) |
| `FuncCall { name, line }` | `#name` (identifier after `#`) |
| `ParenOpen` / `ParenClose` | `(` / `)` |
| `BracketOpen` / `BracketClose` | `[` / `]` |
| `BraceOpen` / `BraceClose` | `{` / `}` |
| `Dollar` | `$` (math mode toggle — parser determines inline vs display) |
| `Dash { count }` | `-`, `--`, `---` |
| `Label { name }` | `<label-name>` (only outside math/code) |
| `AtRef { name }` | `@ref-name` |
| `RawFrontmatter { value }` | `---\n...\n---` at file start |
| `Colon` | `:` (for named arguments) |
| `Comma` | `,` (argument separator) |
| `TermMarker { line }` | `/ ` at line start (definition list term) |
| `Comment { value }` | `// ...` to end of line |
| `BlockComment { value }` | `/* ... */` |
| `BlankLine` | Two consecutive newlines |
| `Newline` | Single newline |
| `Backslash` | `\` (line break or escape prefix) |
| `Escape { ch }` | `\*`, `\_`, `\#`, etc. (backslash + special char) |
| `Quote { value }` | `"..."` string literal |

**Key lexer rules:**
- **Heading detection**: `=` characters at line start (after optional whitespace), followed by a space. Count `=` for level (1–6).
- **Math mode**: `$` emitted as `Dollar` token. The **parser** determines inline vs display: display math has whitespace (space or newline) immediately after the opening `$` and immediately before the closing `$`. This includes multi-line math where `$` is on its own line. Inline math has no such whitespace.
- **Label**: `<` followed by identifier chars (letters, digits, hyphens) and `>` — only when not inside math or code mode.
- **Reference**: `@` followed by identifier chars (letters, digits, hyphens, dots).
- **Function calls**: `#` followed by an alphabetic identifier.
- **Term marker**: `/` followed by a space at line start emits `TermMarker`.
- **Backslash handling**: `\` followed by a special char (`*`, `_`, `#`, `$`, `@`, `<`, `\`, `` ` ``, `/`) produces `Escape { ch }`. A bare `\` (followed by newline or non-special) produces `Backslash`.
- **Dash disambiguation**: `-` at line start followed by space is always a list item marker. `---` on a line by itself (not at file start) is a thematic break candidate (parser decides). At file start, `---` begins YAML frontmatter.

### Parser (`parser.rs`)

Recursive descent parser consuming `Vec<Token>`.

#### Metadata Extraction

1. If `RawFrontmatter` token exists, parse as YAML (reuse logic from `docmux-reader-markdown`'s frontmatter parser or duplicate the small amount needed).
2. Scan for `FuncCall { name: "set" }` followed by `document(...)` arguments. Extract `title`, `author`, `date` from named arguments.
3. Merge: YAML fields take priority; `#set document()` fills gaps.

#### Block-Level Parsing

| Typst | AST Node |
|-------|----------|
| `= Title` | `Block::Heading { level: 1 }` |
| `== Title` | `Block::Heading { level: 2 }` (etc. up to 6) |
| `#heading(level: N)[content]` | `Block::Heading { level: N }` |
| Paragraph text followed by blank line | `Block::Paragraph` |
| `- item` | `Block::List { ordered: false }` |
| `+ item` | `Block::List { ordered: true }` |
| `/ Term: Definition` | `Block::DefinitionList` |
| ` ```lang ... ``` ` | `Block::CodeBlock { language }` |
| `$ ... $` (display — whitespace after/before `$`) | `Block::MathBlock` |
| `---` (line of only dashes, 3+, not at file start) | `Block::ThematicBreak` |
| `#image("path", alt: "text")` at block level | `Block::Figure { caption: None, label: None }` |
| `#figure(image(...), caption: [...])` | `Block::Figure` with caption (see Table/Figure detail below) |
| `#table(columns: N, [cell], ...)` | `Block::Table` (see Table detail below) |
| `#quote(block: true)[...]` | `Block::BlockQuote` |
| `#footnote[...]` | `Block::FootnoteDef` + `Inline::FootnoteRef` |
| `#bibliography("file.bib")` | Stored in `Document.bibliography` path, no processing |
| Unknown `#func(...)` at block level | `Block::RawBlock { format: "typst" }` + warning |

#### Inline-Level Parsing

| Typst | AST Node |
|-------|----------|
| `*bold*` | `Inline::Strong` |
| `_italic_` | `Inline::Emphasis` |
| `` `code` `` | `Inline::Code` |
| `$x^2$` (inline — no whitespace after/before `$`) | `Inline::MathInline` |
| `#link("url")[text]` | `Inline::Link` |
| `#emph[...]` | `Inline::Emphasis` |
| `#strong[...]` | `Inline::Strong` |
| `#strike[...]` | `Inline::Strikethrough` |
| `#sub[...]` | `Inline::Subscript` |
| `#super[...]` | `Inline::Superscript` |
| `#smallcaps[...]` | `Inline::SmallCaps` |
| `#cite(<key>)` | `Inline::Citation` |
| `@key` | `Inline::CrossRef` (see @-reference disambiguation below) |
| `<label>` | Sets label on parent block (`label: Some(...)`) |
| `#footnote[...]` | `Inline::FootnoteRef` + `Block::FootnoteDef` appended |
| `#image(...)` in inline context | `Inline::Image` |
| `#raw("...", lang: "rs")` | `Inline::Code` |
| Single newline within paragraph | `Inline::SoftBreak` |
| `\` followed by newline (explicit line break) | `Inline::HardBreak` |
| Unknown `#func(...)` inline | `Inline::RawInline { format: "typst" }` + warning |

**Not produced by this reader:** `Admonition` (no standard Typst equivalent — package-based admonitions like `gentle-clues` fall through to `RawBlock { format: "typst" }`), `Span`.

#### @-Reference Disambiguation

In Typst, `@key` can reference either a bibliography entry (citation) or a document label (cross-reference). At parse time, we don't have bibliography data, so the reader always emits `Inline::CrossRef` for `@key` references. A future transform (or the writer) can resolve bibliography-bound refs to `Inline::Citation` when bibliography data is available. `#cite(<key>)` explicitly produces `Inline::Citation`.

#### Table Parsing Detail

Typst tables use `#table(columns: N, [cell], [cell], ...)` where cells are positional content arguments and rows wrap automatically based on column count.

- `columns: N` (integer) → N columns with `Alignment::Default`
- `columns: (1fr, 2fr, auto)` → column count from array length; `fr` values map to relative `ColumnSpec.width`; alignment derived from `align:` argument if present
- Cells are positional arguments; every N cells form one row (N = column count)
- `table.header([H1], [H2], ...)` → `Table.header`
- `table.cell(colspan: 2)[content]` → `TableCell { colspan: 2, .. }`
- `align:` named argument → maps to `ColumnSpec.alignment` per column

If column count cannot be determined (e.g., complex expression), default to 1 column and emit a warning.

#### Figure Parsing Detail

Typst figures use `#figure(content, caption: [text])`:
- First positional argument is the figure content (usually `image(...)` or `table(...)`)
- `caption:` named argument → `Block::Figure.caption`
- If content is `image(...)`, extract into `Figure.image`
- If content is `table(...)`, the table is a labeled table (extract label, emit as `Block::Table` with caption)
- Standalone `#image("path", alt: "text")` at block level → `Block::Figure { image: Image { url: path, alt, title: None }, caption: None, label: None }`

#### Silently Ignored Directives

These are consumed (including their arguments) without output or warning:

- `#set` (except `#set document(...)` which extracts metadata)
- `#show`
- `#let`
- `#import`
- `#include` (could be supported later with file I/O)
- `#pagebreak()`
- `#colbreak()`
- `#v(...)` / `#h(...)` (vertical/horizontal spacing)
- `#underline[...]` (no `Inline::Underline` in AST)

#### Content Block Parsing (`[...]`)

Content blocks are Typst's way of passing markup as arguments. When encountered:
- As a function argument (e.g., `#emph[text]`): parse contents as inline elements
- At block level: parse contents as block elements
- The parser tracks context (inline vs block) to choose the right parsing mode

#### Argument Parsing

Typst function calls use `(key: value, ...)` syntax. A simple argument parser handles:
- Positional arguments: `#image("path.png")`
- Named arguments: `#image("path.png", alt: "description", width: 50%)`
- Content arguments: `#figure(image("path.png"), caption: [My caption])`
- String literals: `"quoted strings"`
- Nested function calls: `#figure(image("path.png"))` — parse recursively

We only parse argument structure, not evaluate expressions. Complex expressions (e.g., `width: 50% - 1em`) are captured as string literals.

## Edge Cases

- **Nested emphasis**: `*_bold italic_*` — parser handles nesting by tracking delimiter stack. Inner `_..._` produces `Emphasis` inside `Strong`.
- **`*` and `_` inside math**: Not delimiters. The parser disables emphasis detection inside math mode (`$...$`).
- **`#` inside code blocks or math**: Not a function call. The lexer tracks code/math mode and emits `#` as plain text inside those contexts.
- **Labels `<name>` inside math**: `<` in math mode is a less-than operator, not a label. The lexer only emits `Label` tokens outside math/code.
- **Nested content blocks**: `[...]` containing `[...]` — the parser tracks bracket depth for balanced parsing.
- **Raw blocks containing Typst syntax**: Content between ` ``` ` delimiters is opaque — no parsing of inner content.
- **Unbalanced brackets in string arguments**: String literals (`"..."`) inside argument lists are opaque — brackets within strings don't affect depth counting.
- **Thematic break vs frontmatter vs list**: `---` at file start = YAML frontmatter. `---` on a line by itself (elsewhere) = `ThematicBreak`. `- ` (dash + space) = list item. `--` = en-dash text.
- **Display math multi-line**: `$\n  content\n$` is display math (newline counts as whitespace after `$`).

## File Structure

```
crates/docmux-reader-typst/
├── Cargo.toml
├── src/
│   ├── lib.rs          # TypstReader struct, Reader trait impl
│   ├── lexer.rs        # Tokenizer
│   └── parser.rs       # Recursive descent parser
└── tests/              # (integration tests if needed)
```

## Changes Outside the Crate

1. **`crates/docmux-cli/Cargo.toml`** — add `docmux-reader-typst = { workspace = true }`
2. **`crates/docmux-cli/src/main.rs`** — register `TypstReader` in the registry
3. **`Cargo.toml` (workspace root)** — ensure `docmux-reader-typst` is in `[workspace.dependencies]` if not already
4. **`crates/docmux-cli/tests/`** — add smoke tests for `.typ` input format
5. **`crates/docmux-cli/tests/fixtures/`** — add `.typ` golden file fixtures + expected outputs

## AST Changes

None. All Typst constructs map to existing AST nodes. The `format` field in `RawBlock`/`RawInline` uses `"typst"`.

## Dependencies

Add to `crates/docmux-reader-typst/Cargo.toml`:
```toml
[dependencies]
docmux-ast = { workspace = true }
docmux-core = { workspace = true }
serde_yaml = "0.9"  # for YAML frontmatter parsing (matches markdown reader)
```

Note: `serde_yaml` 0.9 is deprecated upstream but used by the markdown reader for consistency. If migrated project-wide, update both readers.

## CLI Integration

Register in `crates/docmux-cli/src/main.rs`:
```rust
use docmux_reader_typst::TypstReader;
reg.add_reader(Box::new(TypstReader::new()));
```

## Testing Strategy

1. **Lexer unit tests** (~15 tests): one per token type, edge cases for math mode detection (inline vs display, multi-line), escaped chars, heading levels, label vs math `<`.
2. **Parser unit tests** (~30 tests): headings (markup + function form), emphasis/strong/strikethrough, lists (ordered/unordered/definition), tables, images, figures with captions, links, math, labels/refs, `@`-references, metadata extraction (YAML + `#set document`), soft/hard breaks, unknown functions → RawBlock, silently ignored directives.
3. **Integration tests** (~5 tests): full Typst documents → AST verification.
4. **Golden file tests**: add `.typ` input fixtures + expected `.html` and `.tex` outputs to the CLI golden test harness.

**Target: ~50 tests.**

## Out of Scope

- Typst package resolution (`#import "@preview/..."`)
- Expression evaluation (`#let x = 1 + 2`)
- Layout computation (page breaks, column layout, positioning)
- Custom show rules application
- Typst's `context` expressions
- Binary file includes
- `Admonition` blocks (no standard Typst equivalent)
- `Span` nodes (no direct Typst mapping)
