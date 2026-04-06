# @docmux/wasm

Universal document converter — WASM bindings for [docmux](https://github.com/aguschirico/docmux).

Convert between Markdown, LaTeX, Typst, MyST, HTML, DOCX, and plaintext in the browser or Node.js.

## Install

```bash
npm install @docmux/wasm
```

## Usage

```typescript
import { convert, convertStandalone, getInputFormats } from "@docmux/wasm";

// Fragment conversion (no document wrapper)
const result = await convert("# Hello", "markdown", "html");
if (result.error) {
  console.error(result.error);
} else {
  console.log(result.output); // <h1>Hello</h1>
}

// Standalone conversion (full HTML document with <head>, <body>, etc.)
const standalone = await convertStandalone("# Hello", "markdown", "html");

// List supported formats
const formats = await getInputFormats();
// ["markdown", "md", "latex", "tex", "typst", "myst", "html", "docx"]
```

## Binary formats (DOCX)

```typescript
import { convertBytes } from "@docmux/wasm";

const bytes = new Uint8Array(await file.arrayBuffer());
const result = await convertBytes(bytes, "docx", "html");
```

## Raw bindings

For advanced use cases (custom WASM initialization, direct access to wasm-bindgen API):

```typescript
import { convert, inputFormats } from "@docmux/wasm/raw";
const html = convert("# Hello", "markdown", "html");
```

## License

MIT
