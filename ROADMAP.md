# docmux Roadmap

> Last updated: 2026-03-23

## Phase 1 — MVP (Markdown → HTML + LaTeX) ✅

- [x] Workspace scaffold: 14 crates, CI (check, test, fmt, wasm)
- [x] `docmux-ast` — rich AST: 13 block types, 16 inline types, metadata, bibliography, serde roundtrip (5 tests)
- [x] `docmux-core` — traits (`Reader`, `Writer`, `Transform`), `Pipeline`, `Registry`, `WriteOptions` (2 tests)
- [x] `docmux-reader-markdown` — CommonMark + GFM via comrak, YAML frontmatter parsing (15 tests)
- [x] `docmux-writer-html` — HTML5 semantic output, standalone mode with KaTeX/MathJax (6 tests)
- [x] `docmux-writer-latex` — LaTeX output with document class, math environments, tables (10 tests)
- [x] `docmux-cli` — clap CLI with format auto-detection (8 smoke + 2 golden tests)
- [x] `docmux-wasm` — wasm-bindgen: `convert()`, `markdownToHtml()`, format listing
- [x] Fix display math bug (`$$...$$` promoted from inline to `Block::MathBlock`)
- [x] Golden file test harness (13 fixtures × 2 formats = 26 golden files)
- [x] `docmux-transform-crossref` — auto-number figures, tables, equations; resolve `CrossRef` nodes (7 tests)

**Total: 55 tests | clippy clean | fmt clean | CI green**

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
