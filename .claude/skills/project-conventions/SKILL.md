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
- **19 crates total**: 4 readers, 6 writers, 5 transforms, 2 core, 1 CLI, 1 WASM.

## Rust Code

- `cargo fmt --all` must pass (auto-enforced by hook on `.rs` edits).
- `cargo clippy --workspace --all-targets -- -D warnings` must pass with zero warnings.
- No `unwrap()` or `expect()` in library code — use `?` or return `Result`. `unwrap()` is acceptable only in tests.
- Keep `use` imports grouped: std → external crates → internal crates (`docmux_ast`, `docmux_core`).
- Implement the `Writer`, `Reader`, or `Transform` trait from `docmux_core` — don't invent new interfaces.
- Prefer `thiserror` for error types. All errors should be descriptive and actionable.

## TypeScript / Playground (`playground/`)

- **No `any` types. Ever.** Use proper typing — interfaces, generics, discriminated unions.
- **pnpm** for package management. Never npm/yarn. Never edit lock files manually — use `pnpm add`.
- **React components ≤150 lines**. Extract custom hooks, child components, and utilities aggressively.
- **Composition over prop drilling**: Use context for cross-cutting concerns, callbacks for events, composition for layout.
- **WASM boundary**: All WASM calls go through `wasm/docmux.ts`. Components never import wasm bindings directly.
- **State management**: Dexie (IndexedDB) for persistence, React context for UI state. No Redux/Zustand needed.
- **Styling**: Tailwind CSS 4 + shadcn/ui patterns. Use `cn()` utility for conditional classes. Dark mode via `className="dark"` on root.
- **File structure**: Feature-based organization under `components/` — `editor/`, `file-tree/`, `output-tabs/`, `ui/`.

## Testing

- **Unit tests**: `#[cfg(test)] mod tests` inside the source file.
- **Integration tests**: `tests/` directory within the crate.
- **Golden file tests**: Fixtures in `crates/docmux-cli/tests/fixtures/`. Input `.md` files produce `.html`/`.tex`/`.typ`/etc. outputs compared byte-for-byte.
- **Update golden files**: `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test` — then review the diff before committing.
- Always run `cargo test --workspace` to verify nothing else broke.
- New functionality must have tests. No exceptions.

## New Crate Checklist

When creating a new crate, use the `/new-crate` skill which provides templates and step-by-step instructions.

## Git

- No `Co-Authored-By` lines in commits.
- Commit messages: concise, imperative mood, focused on "why".
- Work on `main` branch directly (no feature branches unless asked).
- Pre-commit hook (`.githooks/pre-commit`) runs automatically: fmt, clippy, tests, tsc, eslint.

## Pre-commit Quality Gates

The git pre-commit hook enforces:

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
