---
name: project-conventions
description: docmux coding conventions, architecture rules, testing patterns, and crate structure — triggered when writing or reviewing Rust code
user-invocable: false
---

# docmux Project Conventions

Apply these rules whenever writing or modifying code in this project.

## Architecture

- **Pipeline**: Reader → AST → Transform → Writer. Every format conversion goes through the AST.
- **Crate boundaries**: Each reader, writer, and transform is its own crate under `crates/`. A crate only depends on `docmux-ast` and `docmux-core` (plus format-specific external deps). Never add cross-dependencies between readers/writers/transforms.
- **AST is the contract**: All data flows through `docmux_ast::Document`. Readers produce it, writers consume it, transforms mutate it. No side channels.
- **Owned strings**: All strings in the AST are `String`, never `&str`. No lifetimes in the public API.

## Rust Code

- `cargo fmt --all` must pass (auto-enforced by hook).
- `cargo clippy --workspace --all-targets -- -D warnings` must pass with zero warnings.
- No `unwrap()` in library code — use `?` or return `Result`. `unwrap()` is acceptable only in tests.
- Keep `use` imports grouped: std → external crates → internal crates (`docmux_ast`, `docmux_core`).
- Implement the `Writer`, `Reader`, or `Transform` trait from `docmux_core` — don't invent new interfaces.

## TypeScript/JS (playground)

- No `any` types. Ever. Use proper typing.
- Use `pnpm` for package management (never npm/yarn).
- Never edit `pnpm-lock.yaml` manually — use `pnpm add`.

## Testing

- **Unit tests**: `#[cfg(test)] mod tests` inside the source file.
- **Integration tests**: `tests/` directory within the crate.
- **Golden file tests**: Fixtures in `crates/docmux-cli/tests/fixtures/`. Input `.md` files produce `.html`/`.tex`/etc. outputs compared byte-for-byte.
- **Update golden files**: `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test` — then review the diff before committing.
- Always run `cargo test --workspace` to verify nothing else broke.

## New Crate Checklist

When creating a new crate:

1. Create `crates/docmux-{type}-{name}/Cargo.toml` with `version.workspace = true`, `edition.workspace = true`, etc.
2. Only depend on `docmux-ast` and `docmux-core` from workspace (plus external deps if needed).
3. Add the crate to the workspace `members` list in root `Cargo.toml`.
4. Add it to `[workspace.dependencies]` in root `Cargo.toml`.
5. Implement the appropriate trait (`Reader`, `Writer`, or `Transform`).
6. Add unit tests in `#[cfg(test)] mod tests`.

## Git

- No `Co-Authored-By` lines in commits.
- Commit messages: concise, imperative mood, focused on "why".
- Work on `main` branch directly (no feature branches unless asked).
