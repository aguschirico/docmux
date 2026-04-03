---
name: project-conventions
description: docmux coding conventions, architecture rules, testing patterns, and crate structure — triggered when writing or reviewing Rust or TypeScript/React code
user-invocable: false
---

# docmux Project Conventions

Apply these rules whenever writing or modifying code in this project.

## Architecture

- **Pipeline**: Reader → AST → Transform → Writer. Every format conversion goes through the AST.
- **Crate boundaries**: Each reader, writer, and transform is its own crate under `crates/`. A crate only depends on `docmux-ast` and `docmux-core` (plus format-specific external deps). Never add cross-dependencies between readers/writers/transforms.
- **AST is the contract**: All data flows through `docmux_ast::Document`. Readers produce it, writers consume it, transforms mutate it. No side channels.
- **Owned strings**: All strings in the AST are `String`, never `&str`. No lifetimes in the public API.
- Detailed Rust and TypeScript conventions are in `.claude/rules/rust.md` and `.claude/rules/typescript.md`.
- Format-specific architecture is in `.claude/rules/markdown-reader.md`, `.claude/rules/latex.md`, `.claude/rules/typst.md`, `.claude/rules/myst.md`.

## New Crate Checklist

When creating a new crate, use the `/new-crate` skill which provides templates and step-by-step instructions.

## Git

- No `Co-Authored-By` lines in commits.
- Commit messages: concise, imperative mood, focused on "why".
- Work on `main` branch directly (no feature branches unless asked).
- Pre-commit hook (`.githooks/pre-commit`) runs automatically: fmt, clippy, tests, tsc, eslint.

## Pre-commit Quality Gates

| Check | Scope | Blocking? |
|-------|-------|-----------|
| `cargo fmt --check` | All Rust | Yes |
| `cargo clippy -D warnings` | All Rust | Yes |
| `cargo test --workspace` | All Rust | Yes |
| `unwrap()` in lib code | Staged `.rs` | Warning |
| WASM build | If wasm crate changed | Yes |
| `tsc --noEmit` | Playground TS | Yes |
| `eslint` | Playground TS | Yes |
| No `any` types | Staged TS/TSX | Yes |
| Component ≤150 lines | Staged TSX | Warning |
