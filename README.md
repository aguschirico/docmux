# docmux

Universal document converter — MIT licensed, WASM-first, written in Rust.

## What is docmux?

docmux converts between markup formats using a modular **Reader → AST → Writer** pipeline. Each format is a separate Rust crate, enabling tree-shaking in WASM builds. Think of it as a focused, MIT-licensed alternative to Pandoc, designed for embedding in web applications via WebAssembly.

## Supported formats

| Format | Reader | Writer |
|--------|--------|--------|
| Markdown (CommonMark + GFM) | ✅ | ✅ |
| HTML5 | ✅ | ✅ |
| LaTeX | ✅ | ✅ |
| Typst | ✅ | ✅ |
| MyST Markdown | ✅ | — |
| DOCX | ✅ | ✅ |
| Plaintext | — | ✅ |

## Quick start

### CLI

```bash
# Install from source
cargo install --path crates/docmux-cli

# Convert Markdown to HTML
docmux input.md -o output.html

# Standalone HTML with math support
docmux paper.md -o paper.html --standalone --math=katex

# LaTeX to Typst
docmux paper.tex -t typst -o paper.typ

# DOCX to Markdown
docmux report.docx -o report.md

# Dump AST as JSON
docmux input.md -t json
```

### JavaScript / WASM

```bash
npm install @docmux/wasm
```

```typescript
import { convert, convertStandalone } from "@docmux/wasm";

const result = await convert("# Hello\n\nWorld!", "markdown", "html");
if (result.error) {
  console.error(result.error);
} else {
  console.log(result.output); // <h1>Hello</h1>\n<p>World!</p>
}
```

See the [@docmux/wasm README](npm/README.md) for full API documentation.

### Rust library

```rust
use docmux_reader_markdown::MarkdownReader;
use docmux_writer_html::HtmlWriter;
use docmux_core::{Reader, Writer, WriteOptions};

let reader = MarkdownReader::new();
let writer = HtmlWriter::new();

let doc = reader.read("# Hello\n\nWorld!")?;
let html = writer.write(&doc, &WriteOptions::default())?;
```

## Features

- **13+ block types, 16+ inline types** — math, citations, cross-references, admonitions, tables, footnotes
- **Syntax highlighting** via syntect (HTML and LaTeX output)
- **Template engine** — pandoc-compatible `$variable$` syntax with conditionals and loops
- **Transforms** — table of contents, section numbering, cross-reference resolution, section divs
- **CLI parity** — `--standalone`, `--toc`, `--number-sections`, `--template`, `--math`, `--css`, `--wrap`, and more

## Architecture

```
Input → Reader → [Document AST] → Transforms → Writer → Output
```

The AST is format-agnostic: N readers × M writers give N×M conversions without N×M converters. Each reader, writer, and transform is a separate crate under `crates/`.

## Contributing

Contributions are welcome! Here's how to get started:

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) (for WASM builds)
- [Node.js](https://nodejs.org/) 18+ and [pnpm](https://pnpm.io/) (for the playground and npm package)

### Build and test

```bash
# Rust workspace
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# WASM build
cargo build --target wasm32-unknown-unknown -p docmux-wasm

# npm package
cd npm && pnpm install && pnpm run build && node test.mjs

# Playground
cd playground && pnpm install && pnpm run dev
```

### Code style

- No `unwrap()` in library code — use `?` and proper error types
- No `any` in TypeScript — use interfaces, generics, discriminated unions
- New functionality must have tests
- Run `cargo fmt` and `cargo clippy` before committing

### Submitting changes

1. Fork the repo and create a branch
2. Make your changes with tests
3. Ensure all checks pass: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
4. Open a pull request with a clear description

## License

MIT — see [LICENSE](LICENSE).
