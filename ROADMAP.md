# docmux Roadmap

> Last updated: 2026-04-09

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
- [x] Header attributes `{#id .class key=val}` (pandoc extension, 7 tests)
- [x] Fenced code attributes `` ```{.python .numberLines} `` (pandoc extension, 5 tests)

### Format coverage

- [x] `docmux-reader-typst` — Typst markup parser (81 tests)
- [x] `docmux-writer-typst` — Typst output (16 unit tests, 4 golden files)
- [x] `docmux-reader-myst` — MyST Markdown: directives, roles, labels, recursive nesting (15 tests)

## Phase 3 — Production Features

### Pandoc parity — AST ✅

- [x] `Inline::Quoted` (smart quotes: SingleQuote, DoubleQuote)
- [x] `attrs` on inline `Code`, `Link`, `Image`
- [x] `Image.alt` as `Vec<Inline>` instead of `String` (+ `alt_text()` helper)
- [x] Per-key prefix/suffix in `Citation` (`CiteItem` struct, `keys()` helper)
- [x] `abstract_text` as `Vec<Block>` (formatted abstract)
- [x] Table footer (`foot: Option<Vec<TableCell>>`)

### Pandoc parity — CLI

- [x] `--toc` and `--toc-depth=N`
- [x] `-N` / `--number-sections`
- [x] `--top-level-division=section|chapter|part`
- [x] `--wrap=auto|none|preserve`, `--columns=N`
- [x] `--eol=crlf|lf|native`
- [x] `--template=FILE` with template engine
- [x] `--bibliography=FILE`, `--csl=FILE`
- [x] `--highlight-style=STYLE`, `--list-highlight-themes`, `--list-highlight-languages`
- [x] `--section-divs`, `--id-prefix=PREFIX`

### Pandoc parity — Extensions (markdown reader)

- [x] Smart punctuation (`--smart`) — enabled via comrak `parse.smart`
- [x] Bracketed spans `[text]{.class}` (5 tests)
- [x] `raw_attribute` syntax (inline + block, 6 tests)
- [x] Table captions (pandoc convention, 5 tests)
- [x] Subscript `~text~` in markdown (via comrak extension)
- [x] Superscript `^text^` in markdown (via comrak extension)

### Transforms

- [x] `docmux-transform-cite` — CSL citation processing
  - [x] Fix: embed `locales-en-US.xml` so dates render in citations
  - [x] Forward `CiteItem.prefix`/`suffix` to hayagriva (enables `(see Smith 2020, p. 42)`)
  - [x] `--nocite` flag
- [x] `docmux-transform-toc` — table of contents generation (6 tests)
- [x] `docmux-transform-number-sections` — heading numbering (7 tests)
- [x] `docmux-transform-section-divs` — wrap sections in Div containers (7 tests)
- [x] `docmux-transform-math` — LaTeX ↔ Typst conversion + MathML output

### Writers & readers

- [x] `docmux-writer-markdown` — CommonMark/GFM roundtrip, normalization (28 tests)
- [x] `docmux-writer-plaintext` — stripped text output (29 tests)
- [x] `docmux-reader-html` — HTML reader with scraper/html5ever (29 tests)
- [x] `docmux-writer-docx` — OOXML output via zip + XML generation (20 unit + 1 integration test)

### Template system

- [x] Template engine (variable interpolation, conditionals, loops) — `docmux-template` crate, 33 tests
- [x] Built-in default templates per output format (HTML, LaTeX, Markdown, Plaintext)

### Syntax highlighting

- [x] `docmux-highlight` — server-side highlighting via `syntect` (8 tests), integrated in HTML and LaTeX writers
- [x] Line numbers (`.numberLines`, `startFrom`), line highlighting (`highlight="2,4-6"`)

### Packaging

- [x] `npm/` package — `@docmux/wasm` published with JS/TS wrapper, automated release

## Phase 4 — Extended Formats + Advanced Features

### AST

- [ ] `Block::LineBlock` (poetry, addresses)
- [ ] `MetaInlines` / `MetaBlocks` in `MetaValue`
- [ ] Multi-row table headers, row/cell attributes
- [ ] Short + long table captions

### Readers & writers

- [x] DOCX reader (49 tests, BinaryReader trait, style classifier, CLI + WASM integration)
  - [x] Document body, tables, multi-paragraph cells, hyperlinks, footnotes
  - [x] Image extraction (`<w:drawing>` inline + anchor → `Document.resources` → data URIs)
  - [ ] Character styling (`<w:color>`, `<w:sz>`, `<w:highlight>` → inline CSS in HTML output)
  - [ ] List assembly (numbered/bulleted from `<w:numPr>`)
  - [ ] Drawing/shape parsing (VML `<v:imagedata>`, SmartArt)
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

### Math

- [ ] OMML math output in DOCX writer (LaTeX → Office Math Markup Language)

### Syntax highlighting

- [ ] Load custom `.tmTheme` theme files
- [ ] Per-code-block theme selection

### Other

- [ ] Emoji replacement (`:name:` → Unicode)
- [ ] Encoding detection / BOM handling
- [x] Publish to npm (automated via CI on tag push)
- [ ] Publish to crates.io

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
