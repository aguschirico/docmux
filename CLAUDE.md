# docmux — Project Context

## What is docmux?

A universal document converter written in Rust. Architecture: **Reader → AST → Transform → Writer**. The AST is a format-agnostic intermediate representation, so N readers × M writers give N×M conversions without N×M converters. Think pandoc, but MIT-licensed, WASM-first, and Rust-native.

## Architecture decisions (already made)

- **Workspace layout**: 14 crates under `crates/`. Each reader/writer/transform is a separate crate for independent compilation and optional features.
- **AST design**: Rich typed nodes (13 block types, 16 inline types) — math, citations, cross-refs, admonitions are first-class, not raw-string hacks. All strings are owned (`String`), no lifetimes in the public API.
- **Comrak for Markdown**: Using comrak with GFM extensions (tables, tasklists, footnotes, math_dollars, description_lists, front_matter_delimiter).
- **YAML frontmatter**: Two-pass parsing — first to `serde_yaml::Value` (captures everything), then extract known fields (`title`, `author`, `date`, `abstract`, `keywords`) into typed `Metadata` fields, rest goes to `custom: HashMap<String, MetaValue>`.
- **Author parsing**: Supports 3 formats — single string, list of strings, list of objects with name/affiliation/email/orcid.
- **Display math fix**: comrak wraps `$$...$$` in Paragraph nodes. We detect single-child paragraphs containing display math and promote them to `Block::MathBlock`.
- **LaTeX writer scope**: Full coverage of all AST node types. Standalone mode emits `\documentclass{article}` with amsmath, graphicx, hyperref, listings, ulem packages. Math is native LaTeX (`$...$` / `\[...\]` / `\begin{equation}`). 10 special chars escaped: `# $ % & ~ _ ^ \ { }`.
- **LaTeX reader scope**: Parse a **practical subset** of LaTeX (not Turing-complete TeX). Goal is roundtrip fidelity for academic papers — `\section`, `\begin{figure}`, `\cite`, math environments, etc. The reader should parse back what the writer produces, plus common academic LaTeX.
- **Cross-ref transform**: Two-pass (collect labels → resolve CrossRef nodes). Numbers figures, tables, equations, code blocks, sections sequentially. Unresolved refs are left as-is for writers (e.g. LaTeX `\ref{}`) to handle.
- **Testing strategy**: Golden file tests (`.md` → `.html` / `.tex` compared byte-for-byte) + CLI smoke tests + per-crate unit tests. Update with `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test`.
- **No Co-Authored-By lines in commits**.

## Build & test

```sh
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build --target wasm32-unknown-unknown -p docmux-wasm
```

## Current state (Phase 1 complete)

See `ROADMAP.md` for full status. Implemented:
- `docmux-ast` (5 tests), `docmux-core` (2 tests)
- `docmux-reader-markdown` with YAML frontmatter (15 tests)
- `docmux-writer-html` (6 tests), `docmux-writer-latex` (10 tests)
- `docmux-cli` (8 smoke + 2 golden tests)
- `docmux-wasm` (wasm-bindgen bindings)
- `docmux-transform-crossref` (7 tests)
- 13 golden file fixtures × 2 formats = 26 golden files

Phase 2 complete (MyST reader in progress separately). Phase 3 in progress. Completed: Phase 3 AST enhancements (`Inline::Quoted`, attrs on inline `Code`/`Link`/`Image`, `Image.alt` as `Vec<Inline>`, per-key `CiteItem` prefix/suffix, `abstract_text` as `Vec<Block>`, table footer), Markdown writer (28 tests). Next: CLI features (`--toc`, `-N`, `--template`, etc.), transforms (cite, toc, number-sections), more writers/readers. Full gap analysis in `docs/pandoc-parity-check.md`.

## Coding conventions

- No `any` types (applies to any future TypeScript/JS code).
- Run `cargo fmt` before committing.
- Clippy must pass with `-D warnings`.
- Keep crate dependencies minimal — each crate only depends on what it needs.
- Tests go in `#[cfg(test)] mod tests` within the source file for unit tests, `tests/` for integration tests.

## SSH

The repo uses SSH key alias `pk_gh_aguschirico` for GitHub access.
