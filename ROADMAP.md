# docmux Roadmap

> Last updated: 2026-03-21

## Phase 1 — MVP (Markdown to HTML + LaTeX)

- [x] Workspace scaffold: 14 crates, CI (check, test, fmt, wasm)
- [x] `docmux-ast` — rich AST: 13 block types, 16 inline types, metadata, bibliography, serde roundtrip
- [x] `docmux-core` — traits (`Reader`, `Writer`, `Transform`), `Pipeline`, `Registry`, `WriteOptions`
- [x] `docmux-reader-markdown` — CommonMark + GFM via comrak (tables, tasklists, footnotes, math, description lists)
- [x] `docmux-writer-html` — HTML5 semantic output, standalone mode with KaTeX/MathJax
- [x] `docmux-cli` — clap CLI with format auto-detection
- [x] `docmux-wasm` — wasm-bindgen: `convert()`, `markdownToHtml()`, format listing
- [x] Fix display math bug (`$$...$$` promoted from inline to `Block::MathBlock`)
- [x] Golden file test harness (12 fixtures: 10 basic + 2 complex)
- [x] CLI smoke tests (8 tests: stdout, file output, --standalone, format flags, errors)
- [x] `docmux-writer-latex` — LaTeX output with document class, math environments, tables
- [x] YAML frontmatter parsing in Markdown reader → `Metadata`
- [x] `docmux-transform-crossref` — auto-number figures, tables, equations; resolve `CrossRef` nodes

## Phase 2 — Ecosystem

- [ ] `docmux-reader-latex` — parse LaTeX subset into AST
- [ ] `npm/` package setup — publishable `@docmux/wasm` with JS/TS wrapper
- [ ] `docmux-transform-cite` — basic CSL citation resolution
- [ ] `docmux-transform-math` — normalize math notation across formats (KaTeX ↔ MathJax ↔ raw)

## Phase 3 — Modern Formats

- [ ] `docmux-reader-typst` — Typst markup parser
- [ ] `docmux-writer-typst` — Typst output
- [ ] `docmux-reader-myst` — MyST Markdown (directives, roles, cross-refs)
- [ ] `docmux-writer-docx` — OOXML output via zip + XML generation

## Phase 4 — Production Readiness

- [ ] Error recovery / partial parsing (graceful degradation)
- [ ] Template system for writers (Handlebars or similar)
- [ ] CLI watch mode (`docmux watch input.md -o output.html`)
- [ ] Publish to crates.io
- [ ] Publish `@docmux/wasm` to npm
- [ ] Documentation site

## Non-goals (for now)

- PDF output (use LaTeX → pdflatex/tectonic instead)
- GUI application
- Plugin system for third-party formats (revisit in Phase 4)
