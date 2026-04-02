# docmux — Project Context

## What is docmux?

A universal document converter written in Rust. Architecture: **Reader → AST → Transform → Writer**. The AST is a format-agnostic intermediate representation, so N readers × M writers give N×M conversions without N×M converters. Think pandoc, but MIT-licensed, WASM-first, and Rust-native.

## Architecture decisions (already made)

- **Workspace layout**: 21 crates under `crates/`. Each reader/writer/transform is a separate crate for independent compilation and optional features.
- **AST design**: Rich typed nodes (13+ block types, 16+ inline types) — math, citations, cross-refs, admonitions, divs, underline are first-class. All strings are owned (`String`), no lifetimes in the public API.
- **Comrak for Markdown**: Using comrak with GFM extensions (tables, tasklists, footnotes, math_dollars, description_lists, front_matter_delimiter, subscript, superscript).
- **YAML frontmatter**: Two-pass parsing — first to `serde_yaml::Value` (captures everything), then extract known fields (`title`, `author`, `date`, `abstract`, `keywords`) into typed `Metadata` fields, rest goes to `custom: HashMap<String, MetaValue>`.
- **Author parsing**: Supports 3 formats — single string, list of strings, list of objects with name/affiliation/email/orcid.
- **Display math fix**: comrak wraps `$$...$$` in Paragraph nodes. We detect single-child paragraphs containing display math and promote them to `Block::MathBlock`.
- **LaTeX writer scope**: Full coverage of all AST node types. Standalone mode emits `\documentclass{article}` with amsmath, graphicx, hyperref, listings, ulem packages. Math is native LaTeX (`$...$` / `\[...\]` / `\begin{equation}`). 10 special chars escaped: `# $ % & ~ _ ^ \ { }`.
- **LaTeX reader scope**: Parse a **practical subset** of LaTeX (not Turing-complete TeX). Goal is roundtrip fidelity for academic papers — `\section`, `\begin{figure}`, `\cite`, math environments, etc.
- **Typst reader/writer**: Full Typst markup support — headings, emphasis, lists, code, math, figures, tables, labels, references.
- **MyST reader**: MyST Markdown — directives, roles, labels, recursive nesting.
- **Cross-ref transform**: Two-pass (collect labels → resolve CrossRef nodes). Numbers figures, tables, equations, code blocks, sections sequentially.
- **ToC transform**: Table of contents generation from headings.
- **Number-sections transform**: Hierarchical heading numbering (1, 1.1, 1.1.1, etc.).
- **Testing strategy**: Golden file tests (`.md` → `.html` / `.tex` / `.typ` compared byte-for-byte) + CLI smoke tests + per-crate unit tests. Update with `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test`.
- **No Co-Authored-By lines in commits**.

## Build & test

### Rust (workspace)

```sh
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build --target wasm32-unknown-unknown -p docmux-wasm
```

### Playground (React/TypeScript/Vite)

```sh
cd playground
pnpm install
pnpm run dev       # dev server
pnpm run build     # tsc + vite build
pnpm run lint      # eslint
pnpm exec tsc --noEmit  # type check only
```

### Pre-commit hook

Git pre-commit hook in `.githooks/pre-commit` (configured via `core.hooksPath`). Runs automatically on `git commit`:
- **Rust**: `cargo fmt --check` → `cargo clippy` → `cargo test --workspace` → unwrap() scan
- **WASM**: `cargo build --target wasm32-unknown-unknown -p docmux-wasm` (if wasm crate changed)
- **TypeScript**: `tsc --noEmit` → `eslint` → no `any` types → component size ≤150 lines

## Current state

See `ROADMAP.md` for full status. **Phase 1 ✅ · Phase 2 ✅ · Phase 3 in progress.**

### Crates (22 total, 440+ tests)

| Category | Crates |
|----------|--------|
| Core | `docmux-ast`, `docmux-core` |
| Readers | `docmux-reader-markdown` (15), `docmux-reader-latex` (53), `docmux-reader-typst` (81), `docmux-reader-myst` (15), `docmux-reader-html` (29), `docmux-reader-docx` (49) |
| Writers | `docmux-writer-html` (10), `docmux-writer-latex` (12), `docmux-writer-typst` (16), `docmux-writer-markdown` (28), `docmux-writer-plaintext` (29), `docmux-writer-docx` |
| Transforms | `docmux-transform-crossref` (7), `docmux-transform-toc` (6), `docmux-transform-number-sections` (7), `docmux-transform-cite`, `docmux-transform-math` |
| Highlight | `docmux-highlight` (8) |
| Integration | `docmux-cli` (20 tests), `docmux-wasm` |

### Playground app

Web-based document converter at `playground/`. Stack: React 19 + TypeScript 5.9 + Vite 8 + Tailwind CSS 4. Features: Monaco editor, file tree (IndexedDB via Dexie), resizable 3-panel layout, live WASM conversion, multiple output tabs (preview, source, AST, diagnostics), DOCX binary input with drag-and-drop.

### Next up

`--template`, `--bibliography`, `--csl`, cite/math transforms, DOCX reader enhancements (character styling, list assembly), npm package. Full gap analysis in `docs/pandoc-parity-check.md`.

## Coding conventions

### Rust
- Run `cargo fmt` before committing (auto-enforced by Claude Code hook on `.rs` file edits).
- Clippy must pass with `-D warnings`.
- No `unwrap()` in library code — use `?` and `docmux_core::Result`. `unwrap()` is fine in tests.
- Keep crate dependencies minimal — each crate only depends on what it needs.
- Crate boundaries: readers/writers/transforms depend only on `docmux-ast` + `docmux-core`. No cross-dependencies.
- Tests go in `#[cfg(test)] mod tests` within the source file for unit tests, `tests/` for integration tests.

### TypeScript / Playground
- No `any` types. Ever. Use proper typing.
- Use `pnpm` for package management. Never edit lock files manually.
- React components ≤150 lines — extract hooks, child components, utilities aggressively.
- Push state down, lift events up. Prefer composition over prop drilling.
- WASM calls go through `wasm/docmux.ts` — never import wasm bindings directly from components.

## SSH

The repo uses SSH key alias `pk_gh_aguschirico` for GitHub access.
