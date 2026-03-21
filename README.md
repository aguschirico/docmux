# docmux

Universal document converter — MIT licensed, WASM-first.

> **Status:** Early development. The project is not yet ready for production use.

## What is docmux?

docmux is a document conversion library and CLI tool that converts between markup formats. Think of it as a focused, MIT-licensed alternative to Pandoc, designed from the ground up for embedding in web applications via WebAssembly.

## Goals

- **MIT license** — embed in any SaaS or application without GPL restrictions
- **WASM-first** — small, fast WebAssembly module with npm package
- **Modern formats** — first-class support for Typst and MyST alongside Markdown, LaTeX, HTML
- **Academic features** — math (KaTeX/MathJax), citations (CSL), cross-references
- **Community-driven** — open governance, RFC process for new formats

## Supported formats

| Format | Reader | Writer |
|--------|--------|--------|
| Markdown (CommonMark + GFM) | ✅ | — |
| HTML5 | — | ✅ |
| LaTeX | 🔜 | ✅ |
| Typst | 🔜 | 🔜 |
| MyST | 🔜 | — |
| DOCX | 🔜 | 🔜 |

## Quick start

### CLI

```bash
cargo install docmux-cli

# Convert Markdown to HTML
docmux input.md -o output.html

# Standalone HTML with KaTeX math support
docmux paper.md -o paper.html --standalone
```

### Rust library

```rust
use docmux_reader_markdown::MarkdownReader;
use docmux_writer_html::HtmlWriter;
use docmux_core::{Pipeline, WriteOptions};

let reader = Box::new(MarkdownReader::new());
let writer = Box::new(HtmlWriter::new());
let pipeline = Pipeline::new(reader, writer);

let html = pipeline.convert("# Hello\n\nWorld!")?;
```

### JavaScript / WASM

```js
import init, { markdownToHtml } from '@docmux/wasm';

await init();
const html = markdownToHtml('# Hello\n\nWorld!');
```

## Architecture

```
Input → Reader → [Document AST] → Transforms → Writer → Output
```

docmux uses a modular reader → AST → writer pipeline (the same pattern as Pandoc, and compilers in general). Each format is a separate Rust crate, enabling tree-shaking in WASM builds.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT — see [LICENSE](LICENSE).
