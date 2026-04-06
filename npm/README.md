# @docmux/wasm

Universal document converter — WASM bindings for [docmux](https://github.com/aguschirico/docmux).

Convert between Markdown, LaTeX, Typst, MyST, HTML, DOCX, and plaintext in the browser or Node.js.

## Install

```bash
npm install @docmux/wasm
```

## Usage

```typescript
import { convert, convertStandalone } from "@docmux/wasm";

// Fragment conversion (no document wrapper)
const result = await convert("# Hello\n\nWorld!", "markdown", "html");
if (result.error) {
  console.error(result.error);
} else {
  console.log(result.output);
  // <h1>Hello</h1>
  // <p>World!</p>
}

// Standalone conversion (full HTML document with <head>, <body>, etc.)
const doc = await convertStandalone("# Hello", "markdown", "html");
// Returns a complete HTML page with doctype, head, body
```

## Format conversion examples

```typescript
import { convert } from "@docmux/wasm";

// Markdown → HTML
await convert("**bold** and *italic*", "markdown", "html");

// Markdown → LaTeX
await convert("# Chapter\n\nSome text.", "markdown", "latex");

// LaTeX → HTML
await convert("\\textbf{bold} and \\textit{italic}", "latex", "html");

// Typst → Markdown
await convert("= Heading\nSome *emphasized* text.", "typst", "markdown");

// HTML → Plaintext
await convert("<h1>Title</h1><p>Content</p>", "html", "plaintext");
```

## Binary formats (DOCX)

DOCX files are binary — use the `Bytes` variants with `Uint8Array` input:

```typescript
import { convertBytes, convertBytesStandalone, parseBytesToJson } from "@docmux/wasm";

// From a File input
const file = document.querySelector("input[type=file]").files[0];
const bytes = new Uint8Array(await file.arrayBuffer());

// DOCX → HTML
const result = await convertBytes(bytes, "docx", "html");

// DOCX → Standalone HTML (full page)
const standalone = await convertBytesStandalone(bytes, "docx", "html");

// DOCX → AST JSON (inspect the parsed structure)
const ast = await parseBytesToJson(bytes, "docx");
```

## Parsing to AST

Inspect the parsed document structure as JSON:

```typescript
import { parseToJson } from "@docmux/wasm";

const ast = await parseToJson("# Hello\n\n- item 1\n- item 2", "markdown");
if (!ast.error) {
  console.log(JSON.parse(ast.output));
  // { blocks: [{ Heading: { level: 1, ... } }, { List: { ... } }], meta: { ... } }
}
```

## Supported formats

| Format | Input name | Reader | Writer |
|--------|-----------|--------|--------|
| Markdown (CommonMark + GFM) | `"markdown"` or `"md"` | ✅ | ✅ |
| HTML5 | `"html"` | ✅ | ✅ |
| LaTeX | `"latex"` or `"tex"` | ✅ | ✅ |
| Typst | `"typst"` | ✅ | ✅ |
| MyST Markdown | `"myst"` | ✅ | — |
| DOCX (binary) | `"docx"` | ✅ | ✅ |
| Plaintext | `"plaintext"` or `"txt"` | — | ✅ |

List formats programmatically:

```typescript
import { getInputFormats, getOutputFormats } from "@docmux/wasm";

const inputs = await getInputFormats();   // ["markdown", "md", "latex", ...]
const outputs = await getOutputFormats();  // ["html", "latex", "typst", ...]
```

## Error handling

All functions return a `ConvertOutcome` discriminated union:

```typescript
type ConvertOutcome = ConversionResult | ConversionError;

interface ConversionResult {
  output: string;
  error: null;
}

interface ConversionError {
  output: null;
  error: string;  // Human-readable error message
}
```

Check `result.error` to narrow the type:

```typescript
const result = await convert(input, "markdown", "html");
if (result.error) {
  // result is ConversionError — result.output is null
  showError(result.error);
} else {
  // result is ConversionResult — result.output is string
  render(result.output);
}
```

## Raw bindings

For advanced use cases, import directly from `@docmux/wasm/raw` to get the raw wasm-bindgen functions without the wrapper:

```typescript
import { convert, inputFormats } from "@docmux/wasm/raw";

// Functions are synchronous and throw on error (no ConvertOutcome wrapper)
try {
  const html = convert("# Hello", "markdown", "html");
} catch (e) {
  console.error("Conversion failed:", e);
}

const formats = inputFormats(); // string[]
```

## Environment support

- **Bundler** (Vite, Webpack, etc.) — import normally, the bundler handles `.wasm` loading
- **Node.js** — WASM loads automatically from the filesystem

## License

MIT — see [LICENSE](https://github.com/aguschirico/docmux/blob/main/LICENSE).
