# docmux Roadmap

> Last updated: 2026-03-25

## Phase 1 — MVP (Markdown → HTML + LaTeX) ✅

- [x] Workspace scaffold: 14 crates, CI (check, test, fmt, wasm)
- [x] `docmux-ast` — rich AST: 13 block types, 16 inline types, metadata, bibliography, serde roundtrip (5 tests)
- [x] `docmux-core` — traits (`Reader`, `Writer`, `Transform`), `Pipeline`, `Registry`, `WriteOptions` (2 tests)
- [x] `docmux-reader-markdown` — CommonMark + GFM via comrak, YAML frontmatter parsing (15 tests)
- [x] `docmux-writer-html` — HTML5 semantic output, standalone mode with KaTeX/MathJax (6 tests)
- [x] `docmux-writer-latex` — LaTeX output with document class, math environments, tables (10 tests)
- [x] `docmux-cli` — clap CLI with format auto-detection (10 smoke + 3 golden tests)
- [x] `docmux-wasm` — wasm-bindgen: `convert()`, `markdownToHtml()`, format listing
- [x] Fix display math bug (`$$...$$` promoted from inline to `Block::MathBlock`)
- [x] Golden file test harness (13 fixtures × 2 formats = 26 golden files)
- [x] `docmux-transform-crossref` — auto-number figures, tables, equations; resolve `CrossRef` nodes (7 tests)
- [x] `docmux-reader-latex` — recursive descent LaTeX parser, preamble extraction, best-effort with warnings (53 tests)

## Phase 2 — Format Coverage + Pandoc Parity (AST & CLI)

### Pandoc parity — AST ✅

- [x] `Block::Div` with `Attributes` — generic container for fenced divs, MyST directives
- [x] `Inline::Underline` — needed for DOCX, HTML roundtrip
- [x] `attrs: Option<Attributes>` on `Heading`, `CodeBlock`, `Figure`, `Table`
- [x] `tight: bool` on `List` (populated from comrak's `list.tight`)
- [x] `ListStyle` enum (Decimal, LowerAlpha, UpperAlpha, LowerRoman, UpperRoman)
- [x] `ListDelim` enum (Period, OneParen, TwoParens)
- [x] Writers handle `Div` and `Underline`; crossref transform recurses into both

### Pandoc parity — CLI ✅

- [x] `-t json` — dump parsed AST as pretty-printed JSON
- [x] Stdin support (`-` as input)
- [x] Multiple input files (concatenated)
- [x] `--shift-heading-level-by=N`
- [x] `-M KEY=VAL` metadata overrides
- [x] `--math=katex|mathjax|mathml|raw`
- [x] `--css=URL` (repeatable)
- [x] `--variable KEY=VAL`
- [x] `--list-input-formats`, `--list-output-formats`
- [x] `--verbose` / `--quiet`

### Pandoc parity — Markdown reader

- [x] Auto-generate GFM-style heading IDs (slugify, dedup, 4 tests)
- [ ] Header attributes `{#id .class key=val}` (pandoc extension)
- [ ] Fenced code attributes `` ```{.python .numberLines} `` (pandoc extension)

### Format coverage

- [x] `docmux-reader-typst` — Typst markup parser (81 tests)
- [ ] `docmux-writer-typst` — Typst output
- [ ] `docmux-reader-myst` — MyST Markdown (directives, roles, cross-refs; needs `Div`)

## Phase 3 — Production Features

### Pandoc parity — AST

- [ ] `Inline::Quoted` (smart quotes: SingleQuote, DoubleQuote)
- [ ] `attrs` on inline `Code`, `Link`, `Image`
- [ ] `Image.alt` as `Vec<Inline>` instead of `String`
- [ ] Per-key prefix/suffix in `Citation` (match pandoc model)
- [ ] `abstract_text` as `Vec<Block>` (formatted abstract)
- [ ] Table footer (`foot: Option<Vec<TableCell>>`)

### Pandoc parity — CLI

- [ ] `--toc` and `--toc-depth=N`
- [ ] `-N` / `--number-sections`
- [ ] `--template=FILE` with template engine
- [ ] `--bibliography=FILE`, `--csl=FILE`
- [ ] `--highlight-style=STYLE`
- [ ] `--wrap=auto|none|preserve`, `--columns=N`
- [ ] `--section-divs`, `--id-prefix=PREFIX`
- [ ] `--eol=crlf|lf|native`
- [ ] `--top-level-division=section|chapter|part`

### Pandoc parity — Extensions (markdown reader)

- [ ] Smart punctuation (`--smart`)
- [ ] Bracketed spans `[text]{.class}`
- [ ] `raw_attribute` syntax
- [ ] Table captions
- [ ] Superscript `^text^` / subscript `~text~` in markdown

### Transforms

- [ ] `docmux-transform-cite` — CSL citation processing
- [ ] `docmux-transform-toc` — table of contents generation
- [ ] `docmux-transform-number-sections` — heading numbering
- [ ] `docmux-transform-math` — normalize math notation across formats

### Writers & readers

- [ ] Markdown writer — roundtrip, normalization
- [ ] Plain text writer — stripped output
- [ ] HTML reader — web content, HTML→LaTeX
- [ ] DOCX writer — OOXML output via zip + XML generation

### Template system

- [ ] Template engine (variable interpolation, conditionals, loops)
- [ ] Built-in default templates per output format

### Syntax highlighting

- [ ] Server-side highlighting via `syntect` or `tree-sitter-highlight`
- [ ] Line numbers, multiple styles

### Packaging

- [ ] `npm/` package — publishable `@docmux/wasm` with JS/TS wrapper

## Phase 4 — Extended Formats + Advanced Features

### AST

- [ ] `Block::LineBlock` (poetry, addresses)
- [ ] `MetaInlines` / `MetaBlocks` in `MetaValue`
- [ ] Multi-row table headers, row/cell attributes
- [ ] Short + long table captions

### Readers & writers

- [ ] DOCX reader
- [ ] EPUB reader + writer
- [ ] reStructuredText reader + writer
- [ ] AsciiDoc reader
- [ ] Djot reader + writer
- [ ] Jupyter notebook reader
- [ ] Beamer output (LaTeX presentations)
- [ ] reveal.js output (HTML presentations)
- [ ] ODT writer

### CLI

- [ ] `--self-contained` / `--embed-resources`
- [ ] `--extract-media=DIR`
- [ ] `--filter=PROGRAM` (JSON filter protocol)
- [ ] WASM filter support
- [ ] `--log=FILE`, `--trace`
- [ ] Watch mode (`docmux watch input.md -o output.html`)

### Other

- [ ] Emoji replacement (`:name:` → Unicode)
- [ ] Encoding detection / BOM handling
- [ ] Publish to crates.io + npm

## Phase 5 — Long Tail

- [ ] Org Mode reader + writer
- [ ] MediaWiki reader + writer
- [ ] JIRA/Confluence reader + writer
- [ ] man page reader + writer
- [ ] PowerPoint (PPTX) writer
- [ ] DokuWiki, RTF, Textile, CSV/TSV readers
- [ ] Multiple bibliographies, example lists, abbreviations
- [ ] DOCX track changes
- [ ] Custom syntax definitions, line highlighting
- [ ] Presentation options (slide-level, incremental)

## Non-goals (for now)

- PDF output (use LaTeX → pdflatex/tectonic instead)
- GUI application
- Plugin system for third-party formats (revisit in Phase 4)

## Reference

Full pandoc parity analysis: [`docs/pandoc-parity-check.md`](docs/pandoc-parity-check.md) (117 items, 14 categories)
