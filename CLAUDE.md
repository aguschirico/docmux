# docmux — Project Context

## What is docmux?

A universal document converter written in Rust. Architecture: **Reader → AST → Transform → Writer**. The AST is a format-agnostic intermediate representation, so N readers × M writers give N×M conversions without N×M converters. Think pandoc, but MIT-licensed, WASM-first, and Rust-native.

## Architecture

- **Workspace layout**: 24 crates under `crates/`. Each reader/writer/transform is a separate crate. See root `Cargo.toml` for the full list.
- **AST design**: Rich typed nodes (13+ block types, 16+ inline types) — math, citations, cross-refs, admonitions, divs, underline are first-class. All strings are owned (`String`), no lifetimes in the public API.
- **Crate boundaries**: Readers/writers/transforms depend only on `docmux-ast` + `docmux-core`. No cross-dependencies.
- **Testing strategy**: Golden file tests + CLI smoke tests + per-crate unit tests. Update expectations with `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test`.
- Format-specific architecture details are in `.claude/rules/` (markdown-reader, latex, typst, myst).

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

### Quality gates

- **Pre-commit hook** (`.githooks/pre-commit`): cargo fmt → clippy → test → unwrap scan → WASM build → tsc → eslint → no `any` → component size.
- **Claude Code Stop hook**: 2-phase quality gate (code review checklist → mechanical checks).
- **PostToolUse hook**: Auto `cargo fmt` on .rs edits.

## Coding conventions

Detailed conventions are in `.claude/rules/`:
- `rules/rust.md` — Rust code style, crate boundaries, testing (scoped to `crates/**`)
- `rules/typescript.md` — TS/React patterns, WASM boundary, styling (scoped to `playground/**`)

Key rules that apply everywhere:
- No `unwrap()` in library code. No `any` in TypeScript. Ever.
- New functionality must have tests. No exceptions.
- No `Co-Authored-By` lines in commits.

## Current status

See `ROADMAP.md` for full status. Phase 1 and Phase 2 complete, Phase 3 in progress.

### Next up

`--template`, `--bibliography`, `--csl`, cite/math transforms, DOCX reader enhancements, npm package. See `docs/pandoc-parity-check.md` for gap analysis.
