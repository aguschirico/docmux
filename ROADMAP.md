# docmux Roadmap

> Last updated: 2026-10-14
> Current phase: Phase 4, measure against pandoc

## Goal: compatibility with pandoc

pandoc is the reference for document conversion, and docmux measures itself against it. The goal is compatibility: docmux should read the documents pandoc users write, produce output they recognize, and exchange documents and filters with pandoc itself. It should do that while staying small enough to embed, in the browser through WebAssembly, in Node, or as Rust crates.

Compatibility is measured, not estimated. Phase 4 builds a harness that runs the same inputs through pandoc and docmux and compares the results, so every format gets a real number and every gap becomes a test.

### Principles

- **pandoc is the oracle.** When docmux and pandoc disagree, pandoc's behavior is the default answer. Deliberate differences are documented.
- **The core stays WASM-first.** Readers, writers and transforms compile to `wasm32-unknown-unknown`. Features that need processes or a file system (external filters, Lua, PDF engines) live only in the CLI.
- **Real documents first.** Gaps found by real documents (the arXiv corpus, issues with attached files) go ahead of gaps found by reading the pandoc manual.
- **Every gap becomes a test** in the crate that owns it, so the fix stays fixed when the source document is gone.

## Where we are (v0.6.0)

Shipped through Phases 1 to 3, plus what came after:

- **Formats:** Markdown (CommonMark + GFM), LaTeX, Typst, Word (.docx), HTML and MyST, read and write. Plain text, write only.
- **AST:** pandoc's block and inline model, including `Div`, `Span`, `Underline`, `Quoted`, attributes on headings, code, links, images, tables and figures, list styles and delimiters, table footers, per-key citation prefixes and suffixes. docmux extras: `MathBlock`, `CrossRef`, `Admonition`, `FootnoteDef`.
- **CLI:** `--standalone`, `--template`, `--toc`, `-N`, `--top-level-division`, `--bibliography`, `--csl`, `--nocite`, `--math`, `--css`, `-M`, `--variable`, `--wrap`, `--columns`, `--eol`, `--highlight-style`, `--section-divs`, `--id-prefix`, `--shift-heading-level-by`, stdin and multiple inputs, `-t json` (docmux AST).
- **Transforms:** CSL citations (hayagriva), cross-references, table of contents, section numbering, section divs, LaTeX/Typst/MathML math.
- **LaTeX as people write it:** user macros expanded, `\input`/`\include` followed, citations resolved from the `.bbl`, checked against 397 arXiv papers across fields, most of them sampled at random, whose Typst output compiles.
- **Word:** math as native OMML equations (178,132 across the corpus, checked on every run by reading them back with pandoc), bookmarks and internal links.
- **Packaging:** `@docmux/wasm` on npm (trusted publishing, staged releases), 26 crates on crates.io, playground and landing at [docmux.pages.dev](https://docmux.pages.dev).

The detailed Phase 1 to 3 checklists are in the git history of this file.

## Phase 4: Measure against pandoc

Everything after this phase is prioritized by what this harness finds.

### pandoc JSON interchange

- [ ] `pandoc-json` reader and writer for pandoc's JSON AST (pandoc-types 1.23, as written by pandoc 3.x)
- [ ] Mapping for docmux-only nodes, documented both ways: `MathBlock` as `Para [Math DisplayMath]`, `Admonition` as a `Div` with the admonition class, `FootnoteDef` and `FootnoteRef` as `Note`, `CrossRef` as a decision to make (a link to `#target`, or pandoc-crossref's citation form)
- [ ] AST additions needed for a lossless round trip: `Block::LineBlock`, `MetaInlines` and `MetaBlocks` in `MetaValue`, multi-row table heads, row and cell attributes, short and long captions, multiple table bodies
- [ ] Round-trip property test: pandoc JSON → docmux → pandoc JSON is identical for every case in the harness

### Compatibility harness

- [ ] `scripts/compat/`: fetches pandoc's own test inputs (`test/command/*.md`, the reader and writer test suites) into a gitignored cache, pinned to a pandoc release. They are not committed, as with the arXiv sources.
- [ ] Reader check: `pandoc -f X -t json` vs `docmux -f X -t pandoc-json`, compared after normalization
- [ ] Writer check: render the same pandoc JSON with both tools, then read both outputs back with pandoc and compare the ASTs, so formatting differences that do not change meaning are not counted
- [ ] Report: pass rate per reader and per writer, written to `docs/compat/report.md`
- [ ] Weekly CI job against the pinned pandoc version; it reports and does not block merges

### Filters

- [ ] `--filter PROGRAM` (CLI only): pipe pandoc JSON through any pandoc JSON filter (pandocfilters, panflute, and others)
- [ ] `-f json` and `-t json` keep meaning the docmux AST; the pandoc AST is `pandoc-json`. To decide: whether `json` should switch to pandoc's AST for drop-in compatibility

## Phase 5: Pandoc Markdown

pandoc's own Markdown is what its users write, and today docmux reads CommonMark + GFM through comrak. Checked against pandoc 3.12.1 on 2026-10-14, these come out as plain text in docmux:

- [ ] Fenced divs `::: class`
- [ ] Inline notes `^[...]`
- [ ] Line blocks `| line`
- [ ] Simple, multiline and grid tables
- [ ] Fancy lists (`a)`, `i.`, `#.`) and example lists `(@)`
- [ ] Implicit figures (a standalone image with alt text becomes a figure)
- [ ] Citation locators kept without a bibliography (`[@doe, p. 3]` currently drops `p. 3`)
- [ ] pandoc title block (`% Title`)
- [ ] Raw TeX in Markdown (`raw_tex`)

Then the format names and switches pandoc users type:

- [ ] Format names: `markdown` (pandoc flavor), `gfm`, `commonmark`, `commonmark_x`, `markdown_strict`, `markdown_mmd`, `markdown_phpextra`
- [ ] Extension toggles: `-f markdown+smart-raw_html`, `--list-extensions[=FORMAT]`
- [ ] Markdown writer that emits each flavor, pandoc Markdown by default for `-t markdown`

Decision to make first, with a spike: extend comrak with pre- and post-processing, or write a pandoc Markdown parser. The harness from Phase 4 decides how close each option gets.

## Phase 6: Formats pandoc users reach for

In order of expected demand. Issues with real documents move items up.

- [ ] EPUB writer (EPUB 3), then reader
- [ ] ODT writer, then reader; `--reference-doc` for DOCX and ODT
- [ ] reStructuredText reader and writer
- [ ] Jupyter notebooks (`ipynb`) reader and writer
- [ ] Org Mode reader and writer
- [ ] AsciiDoc writer, then reader
- [ ] Slides: reveal.js and Beamer writers, then PowerPoint (PPTX); `--slide-level`, `--incremental`
- [ ] Djot reader and writer
- [ ] JATS writer (journal publishing)
- [ ] Bibliographies as formats: BibTeX, BibLaTeX and CSL JSON readers and writers

## Phase 7: pandoc's tooling

- [ ] `--defaults FILE` (YAML defaults files)
- [ ] `--embed-resources`, `--extract-media=DIR`, `--resource-path`
- [ ] WASM filters: docmux-native filters that also run in the browser
- [ ] Lua filters (CLI only), if the harness and issues show demand
- [ ] CSL locales beyond en-US, multiple bibliographies
- [ ] DOCX reader: character styling, list assembly from `<w:numPr>`, drawings and shapes, track changes (`--track-changes`)
- [ ] `--log=FILE`, `--trace`
- [ ] Custom `.tmTheme` highlight themes, custom syntax definitions
- [ ] Emoji (`:name:`), encoding detection and BOM handling
- [ ] Watch mode (`docmux watch input.md -o output.html`)

## Phase 8: Long tail

Driven by demand. pandoc 3.12 reads about 50 formats and writes about 80; these are the rest:

- Wikis: MediaWiki, DokuWiki, Jira, TikiWiki, TWiki, VimWiki, XWiki, ZimWiki, Creole
- Documentation: man, mdoc, Texinfo, DocBook, POD, Haddock, ms
- Publishing and markup: TEI, ICML, ConTeXt, Muse, txt2tags, Textile, Markua, FB2, OPML, BBCode, ANSI
- Data and office: RTF, CSV, TSV, XLSX and PPTX readers, RIS and EndNote XML

## Non-goals (for now)

- PDF through external engines in the CLI. Use LaTeX or Typst output and their own compilers.
- A desktop application. The playground is a demo of the library.
- Byte-for-byte identical output. Compatibility means the same document, which the harness checks through pandoc's AST.

## Reference

- [`docs/pandoc-parity-check.md`](docs/pandoc-parity-check.md): gap analysis from 2026-03-25, kept for history. Phase 4's compatibility report replaces it.
- Open issues about pandoc differences: [#2 LaTeX writer preamble and environment gaps](https://github.com/aguschirico/docmux/issues/2)
