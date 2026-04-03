---
paths:
  - "crates/**/*.rs"
  - "crates/**/Cargo.toml"
---

# Rust Conventions

- `cargo fmt --all` before committing (auto-enforced by PostToolUse hook).
- `cargo clippy --workspace --all-targets -- -D warnings` must pass with zero warnings.
- No `unwrap()` or `expect()` in library code — use `?` and `docmux_core::Result`. Fine in tests.
- Keep `use` imports grouped: std → external crates → internal crates (`docmux_ast`, `docmux_core`).
- Prefer `thiserror` for error types. Errors must be descriptive and actionable.
- Functions ≤40 lines. Prefer iterators over index loops, `?` over match-then-return.
- No unnecessary `.clone()` — borrow when possible.

## Crate boundaries

- Each reader/writer/transform is its own crate under `crates/`.
- A crate depends only on `docmux-ast` + `docmux-core` (plus format-specific external deps).
- No cross-dependencies between readers, writers, or transforms.
- Implement the `Reader`, `Writer`, or `Transform` trait from `docmux_core` — no custom interfaces.

## Testing

- Unit tests: `#[cfg(test)] mod tests` inside the source file.
- Integration tests: `tests/` directory within the crate.
- Golden file tests: `crates/docmux-cli/tests/fixtures/`. Update with `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test`.
- New functionality must have tests. Cover happy path + at least one edge case.
- Always run `cargo test --workspace` to verify nothing else broke.
