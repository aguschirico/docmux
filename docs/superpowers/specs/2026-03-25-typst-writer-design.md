# docmux-writer-typst Design Spec

> Date: 2026-03-25

## Overview

A Typst writer for docmux that converts the internal AST to idiomatic Typst markup. Follows the same recursive-descent pattern as the existing HTML and LaTeX writers. Uses native Typst syntax where a shorthand exists, and function calls (`#func[...]`) for everything else.

## Crate Structure

```
crates/docmux-writer-typst/
├── Cargo.toml    # deps: docmux-ast, docmux-core
└── src/
    └── lib.rs    # TypstWriter + unit tests
```

Single `TypstWriter` struct implementing the `Writer` trait:

- `format()` → `"typst"`
- `default_extension()` → `"typ"`
- `write()` → entry point; optional footnote pre-pass, then `write_blocks()`

Private methods: `write_blocks()`, `write_block()`, `write_inlines()`, `write_inline()`, `escape_typst()`, `wrap_standalone()`.

## Block Mapping

| AST Node | Typst Output |
|---|---|
| `Paragraph` | Inlines joined + `\n\n` separator |
| `Heading { level, id, content }` | `=`(repeated level times) + ` ` + content. Label `<id>` appended if present |
| `CodeBlock { language, content, caption, label }` | `` ```lang\ncontent\n``` ``. If caption present, wrap in `#figure(caption: [caption])[...]`. Label via `<label>` |
| `MathBlock { content, label }` | `$ content $` (content on its own line for display mode). Label via `<label>` |
| `BlockQuote { content }` | `#quote(block: true)[content]` |
| `List { ordered: false }` | `- item` per item, 2-space indent for nesting |
| `List { ordered: true, start }` | `+ item` per item, 2-space indent for nesting. If `start` is not 1, emit `#set enum(start: N)` before the list |
| `Table { columns, headers, rows, caption, label }` | `#table(columns: N, ..header cells, ..body cells)`. Alignment via `align` parameter. Header cells wrapped in `table.header(...)`. If caption present, wrap in `#figure(caption: [caption])[...]`. Label via `<label>` |
| `Figure { image, caption, label }` | `#figure(image("url"), caption: [caption]) <label>` |
| `ThematicBreak` | `#line(length: 100%)` |
| `RawBlock { format, content }` | Content verbatim if `format == "typst"`, otherwise skipped |
| `Admonition { kind, title, content }` | `#block(inset: 1em, stroke: 0.5pt)[#strong[title]\n\ncontent]` |
| `DefinitionList { items }` | `/ term: definition` per item. Multiple definitions for the same term emit multiple `/ term: def` lines |
| `Div { attrs, content }` | Content pass-through (emit inner blocks directly, attrs ignored) |
| `FootnoteDef` | Omitted from output; consumed by `FootnoteRef` during inline pass |

## Inline Mapping

| AST Node | Typst Output |
|---|---|
| `Text { value }` | Escaped text |
| `Emphasis { content }` | `_content_` |
| `Strong { content }` | `*content*` |
| `Strikethrough { content }` | `#strike[content]` |
| `Code { value }` | `` `value` `` |
| `MathInline { value }` | `$value$` |
| `Link { url, title, content }` | `#link("url")[content]` or `#link("url")` if content is empty. `title` dropped (no Typst equivalent) |
| `Image(image)` | `#image("url", alt: "alt text")` if alt present, otherwise `#image("url")` |
| `Citation { keys, prefix, suffix }` | `@key` per key, space-separated for multi-key (e.g., `@a @b`). Prefix/suffix text emitted around the group if present (e.g., `prefix @key1 @key2 suffix`). `mode` ignored (Non-Goal) |
| `Underline { content }` | `#underline[content]` |
| `FootnoteRef { id }` | `#footnote[expanded content]` (looked up from pre-pass index) |
| `CrossRef { target, form }` | `@target` for all `RefForm` variants. `Number` and `NumberWithType` both emit plain `@target` (Typst auto-determines the supplement). `Page` and `Custom(s)` also emit `@target` — no Typst equivalent for page refs or custom prefixes. Declared as Non-Goal |
| `RawInline { format, content }` | Content verbatim if `format == "typst"`, otherwise skipped |
| `Superscript { content }` | `#super[content]` |
| `Subscript { content }` | `#sub[content]` |
| `SmallCaps { content }` | `#smallcaps[content]` |
| `SoftBreak` | `\n` |
| `HardBreak` | `\` + `\n` |
| `Span { content }` | Content only, attrs ignored |

## Escaping

Characters always special in Typst text: `\`, `*`, `_`, `` ` ``, `$`, `#`, `@`, `<`, `>`. Escaped with backslash prefix (`\*`, `\_`, etc.).

Context-sensitive characters (`=`, `-`, `+`, `/`) are only special at line start. These are escaped only when they appear at the beginning of a text node that starts a new line.

A separate `escape_typst()` function handles this. A `escape_typst_url()` helper escapes `"` inside URL strings.

## Standalone Mode

Minimalista. Only metadata, no layout/font configuration:

```typst
#set document(
  title: "Document Title",
  author: ("Author One", "Author Two"),
  date: datetime(year: 2024, month: 3, day: 25),
)

// body starts here
```

- `title` → string
- `authors` → array of name strings
- `date` → `datetime(...)` if parseable, otherwise raw string
- `abstract` → emitted as an initial blockquote or paragraph (no Typst standard for abstracts)
- `keywords` → omitted (no standard Typst field)
- `WriteOptions.math_engine` → ignored (Typst has native math)

## Footnote Pre-Pass

Before writing, iterate top-level blocks to collect `FootnoteDef { id, content }` into a `HashMap<String, Vec<Block>>`. When `FootnoteRef { id }` is encountered during inline rendering, look up the map and emit `#footnote[content]`. Footnote defs are excluded from the main block output.

## Task Lists

`ListItem { checked: Some(true) }` renders as the unicode checkbox characters before the item content. `Some(false)` uses the empty checkbox unicode character.

## CLI Integration

Register `TypstWriter` in `build_registry()` in `crates/docmux-cli/src/main.rs`. No other CLI changes needed — format auto-detection by `.typ` extension already works.

## Testing

**Unit tests** (~12, in `#[cfg(test)] mod tests`):
- `paragraph`, `heading_with_label`, `emphasis_and_strong`, `inline_code_and_math`, `code_block`, `math_block_with_label`, `link_and_image`, `lists_ordered_unordered`, `definition_list`, `table`, `figure_with_caption`, `standalone_mode`, `footnote_expansion`, `escaping`

**Golden file tests**: Existing `.typ` fixtures run through Typst reader → AST → Typst writer. Expected files: `typst-heading.typ.typ`, `typst-inlines.typ.typ`, `typst-math.typ.typ`, `typst-lists.typ.typ`. Added to `golden.rs` harness.

**CLI smoke tests**: `converts_typst_to_typst_stdout`, `typst_to_typst_file_output`.

## Non-Goals

- Template system (Phase 4)
- Custom page/font/margin configuration in standalone mode
- Typst package imports
- Citation mode variants (AuthorOnly, SuppressAuthor) — all fall back to `@key`
- CrossRef form variants (Page, Custom) — all fall back to plain `@target`
- Bibliography section emission (requires external `.bib`/`.yml` files; out of scope)
- List style/delimiter mapping (Typst `#set enum(numbering: ...)`) — emit default `+` for all ordered lists
- Table column widths — emit column count only, let Typst auto-size
