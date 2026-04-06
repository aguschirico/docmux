# @docmux/wasm npm Package — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish `@docmux/wasm` to npm with a dual-target (bundler + Node.js) build, a TypeScript wrapper API, raw re-exports, and migrate the playground to consume it via pnpm workspace.

**Architecture:** `wasm-pack` builds twice (bundler + nodejs targets) into `npm/bundler/` and `npm/node/`. A TypeScript wrapper (`npm/src/index.ts`) provides auto-init and `ConvertOutcome` error types. The playground links to `@docmux/wasm` via pnpm workspace instead of hardcoded `../../wasm-pkg/`.

**Tech Stack:** wasm-pack, wasm-bindgen, TypeScript 5.9, pnpm workspaces

**Spec:** `docs/superpowers/specs/2026-04-06-npm-wasm-package-design.md`

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `npm/package.json` | Package metadata, conditional exports, scripts |
| Create | `npm/tsconfig.json` | Compile `src/` → `dist/` |
| Create | `npm/scripts/build.sh` | wasm-pack x2 + cleanup |
| Create | `npm/src/index.ts` | Wrapper API (bundler entry): auto-init, ConvertOutcome, 9 functions |
| Create | `npm/src/index.node.ts` | Wrapper API (Node.js entry): same API, imports from `node/` target |
| Create | `npm/src/raw.ts` | Re-exports raw wasm-bindgen bindings (bundler) |
| Create | `pnpm-workspace.yaml` | Workspace: playground + npm |
| Modify | `playground/package.json` | Add `@docmux/wasm` workspace dep |
| Modify | `playground/src/wasm/docmux.ts` | Re-export from `@docmux/wasm` instead of manual wrapper |
| Modify | `.gitignore` | Add `npm/bundler/`, `npm/node/`, `npm/dist/` |
| Modify | `.githooks/pre-commit` | Update WASM build path |
| Delete | `wasm-pkg/` | Old build output, replaced by npm package |

---

### Task 1: Create the build script and package scaffolding

**Files:**
- Create: `npm/scripts/build.sh`
- Create: `npm/package.json`
- Create: `npm/tsconfig.json`
- Modify: `.gitignore`

- [ ] **Step 1: Create `npm/scripts/build.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
NPM_DIR="$REPO_ROOT/npm"

echo "Building bundler target..."
wasm-pack build "$REPO_ROOT/crates/docmux-wasm" \
  --target bundler \
  --out-dir "$NPM_DIR/bundler" \
  --out-name docmux_wasm

echo "Building nodejs target..."
wasm-pack build "$REPO_ROOT/crates/docmux-wasm" \
  --target nodejs \
  --out-dir "$NPM_DIR/node" \
  --out-name docmux_wasm

# Clean up wasm-pack artifacts
rm -f "$NPM_DIR/bundler/package.json" "$NPM_DIR/bundler/.gitignore" "$NPM_DIR/bundler/README.md"
rm -f "$NPM_DIR/node/package.json" "$NPM_DIR/node/.gitignore" "$NPM_DIR/node/README.md"

echo "WASM build complete."
```

Make executable: `chmod +x npm/scripts/build.sh`

- [ ] **Step 2: Create `npm/package.json`**

```json
{
  "name": "@docmux/wasm",
  "version": "0.1.0",
  "description": "Universal document converter — WASM bindings for docmux",
  "license": "MIT",
  "type": "module",
  "repository": {
    "type": "git",
    "url": "https://github.com/aguschirico/docmux",
    "directory": "npm"
  },
  "exports": {
    ".": {
      "node": {
        "import": "./dist/index.node.js",
        "types": "./dist/index.d.ts"
      },
      "default": {
        "import": "./dist/index.js",
        "types": "./dist/index.d.ts"
      }
    },
    "./raw": {
      "node": {
        "import": "./node/docmux_wasm.js",
        "types": "./node/docmux_wasm.d.ts"
      },
      "default": {
        "import": "./bundler/docmux_wasm.js",
        "types": "./bundler/docmux_wasm.d.ts"
      }
    }
  },
  "files": [
    "dist/",
    "bundler/",
    "node/",
    "README.md",
    "LICENSE"
  ],
  "scripts": {
    "build:wasm": "./scripts/build.sh",
    "build:ts": "tsc",
    "build": "pnpm run build:wasm && pnpm run build:ts",
    "publish-wasm": "pnpm run build && npm publish --access public"
  },
  "devDependencies": {
    "typescript": "~5.9.3"
  },
  "sideEffects": false
}
```

- [ ] **Step 3: Create `npm/tsconfig.json`**

```json
{
  "compilerOptions": {
    "target": "ES2023",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "declaration": true,
    "declarationMap": true,
    "outDir": "dist",
    "rootDir": "src",
    "strict": true,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "verbatimModuleSyntax": true
  },
  "include": ["src"]
}
```

- [ ] **Step 4: Add generated directories to `.gitignore`**

Append to `/Users/augustochirico/Documents/src/side-projects/docmux/.gitignore`:

```
npm/bundler/
npm/node/
npm/dist/
```

- [ ] **Step 5: Run the build script to verify it works**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux && npm/scripts/build.sh`

Expected: Two directories created (`npm/bundler/`, `npm/node/`), each containing `docmux_wasm.js`, `docmux_wasm.d.ts`, `docmux_wasm_bg.wasm`. No `package.json` or `.gitignore` inside them.

- [ ] **Step 6: Commit**

```bash
git add npm/package.json npm/tsconfig.json npm/scripts/build.sh .gitignore
git commit -m "feat(npm): add package scaffolding and dual-target WASM build script"
```

---

### Task 2: Write the TypeScript wrapper API (bundler entry)

**Files:**
- Create: `npm/src/index.ts`

The wrapper must be built AFTER Task 1's build script runs, because `src/index.ts` imports from `../bundler/docmux_wasm.js` which is generated by wasm-pack.

- [ ] **Step 1: Create `npm/src/index.ts`**

```typescript
import init, {
  convert as wasmConvert,
  convertStandalone as wasmConvertStandalone,
  convertBytes as wasmConvertBytes,
  convertBytesStandalone as wasmConvertBytesStandalone,
  parseToJson as wasmParseToJson,
  parseBytesToJson as wasmParseBytesToJson,
  markdownToHtml as wasmMarkdownToHtml,
  inputFormats as wasmInputFormats,
  outputFormats as wasmOutputFormats,
} from "../bundler/docmux_wasm.js";

// ── Types ──────────────────────────────────────────────────────────────

export interface ConversionResult {
  output: string;
  error: null;
}

export interface ConversionError {
  output: null;
  error: string;
}

export type ConvertOutcome = ConversionResult | ConversionError;

// ── Lazy singleton init ────────────────────────────────────────────────

let initPromise: Promise<void> | null = null;

function ensureInit(): Promise<void> {
  if (!initPromise) {
    initPromise = init().then(() => undefined);
  }
  return initPromise;
}

// ── Internal helper ────────────────────────────────────────────────────

async function callWasm<Args extends unknown[]>(
  fn: (...args: Args) => string,
  ...args: Args
): Promise<ConvertOutcome> {
  await ensureInit();
  try {
    return { output: fn(...args), error: null };
  } catch (e: unknown) {
    return { output: null, error: String(e) };
  }
}

// ── Public API ─────────────────────────────────────────────────────────

export function convert(
  input: string,
  from: string,
  to: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmConvert, input, from, to);
}

export function convertStandalone(
  input: string,
  from: string,
  to: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmConvertStandalone, input, from, to);
}

export function parseToJson(
  input: string,
  from: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmParseToJson, input, from);
}

export function convertBytes(
  input: Uint8Array,
  from: string,
  to: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmConvertBytes, input, from, to);
}

export function convertBytesStandalone(
  input: Uint8Array,
  from: string,
  to: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmConvertBytesStandalone, input, from, to);
}

export function parseBytesToJson(
  input: Uint8Array,
  from: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmParseBytesToJson, input, from);
}

export function markdownToHtml(input: string): Promise<ConvertOutcome> {
  return callWasm(wasmMarkdownToHtml, input);
}

export async function getInputFormats(): Promise<string[]> {
  await ensureInit();
  return wasmInputFormats();
}

export async function getOutputFormats(): Promise<string[]> {
  await ensureInit();
  return wasmOutputFormats();
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux/npm && pnpm install && pnpm run build:ts`

Expected: `dist/index.js` and `dist/index.d.ts` generated. No type errors.

- [ ] **Step 3: Commit**

```bash
git add npm/src/index.ts
git commit -m "feat(npm): add TypeScript wrapper API with auto-init and ConvertOutcome types"
```

---

### Task 3: Write the Node.js entry and raw re-export

**Files:**
- Create: `npm/src/index.node.ts`
- Create: `npm/src/raw.ts`

- [ ] **Step 1: Create `npm/src/index.node.ts`**

Same API as `index.ts`, but imports from the `nodejs` wasm-pack target:

```typescript
import {
  convert as wasmConvert,
  convertStandalone as wasmConvertStandalone,
  convertBytes as wasmConvertBytes,
  convertBytesStandalone as wasmConvertBytesStandalone,
  parseToJson as wasmParseToJson,
  parseBytesToJson as wasmParseBytesToJson,
  markdownToHtml as wasmMarkdownToHtml,
  inputFormats as wasmInputFormats,
  outputFormats as wasmOutputFormats,
} from "../node/docmux_wasm.js";

// ── Types ──────────────────────────────────────────────────────────────

export interface ConversionResult {
  output: string;
  error: null;
}

export interface ConversionError {
  output: null;
  error: string;
}

export type ConvertOutcome = ConversionResult | ConversionError;

// ── Internal helper ────────────────────────────────────────────────────

async function callWasm<Args extends unknown[]>(
  fn: (...args: Args) => string,
  ...args: Args
): Promise<ConvertOutcome> {
  try {
    return { output: fn(...args), error: null };
  } catch (e: unknown) {
    return { output: null, error: String(e) };
  }
}

// ── Public API ─────────────────────────────────────────────────────────

export function convert(
  input: string,
  from: string,
  to: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmConvert, input, from, to);
}

export function convertStandalone(
  input: string,
  from: string,
  to: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmConvertStandalone, input, from, to);
}

export function parseToJson(
  input: string,
  from: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmParseToJson, input, from);
}

export function convertBytes(
  input: Uint8Array,
  from: string,
  to: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmConvertBytes, input, from, to);
}

export function convertBytesStandalone(
  input: Uint8Array,
  from: string,
  to: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmConvertBytesStandalone, input, from, to);
}

export function parseBytesToJson(
  input: Uint8Array,
  from: string,
): Promise<ConvertOutcome> {
  return callWasm(wasmParseBytesToJson, input, from);
}

export function markdownToHtml(input: string): Promise<ConvertOutcome> {
  return callWasm(wasmMarkdownToHtml, input);
}

export async function getInputFormats(): Promise<string[]> {
  return wasmInputFormats();
}

export async function getOutputFormats(): Promise<string[]> {
  return wasmOutputFormats();
}
```

Key difference from `index.ts`: the Node.js wasm-pack target auto-initializes WASM from the filesystem — no `init()` call needed. The functions are still async for API consistency.

- [ ] **Step 2: Create `npm/src/raw.ts`**

```typescript
export {
  default,
  convert,
  convertStandalone,
  convertBytes,
  convertBytesStandalone,
  parseToJson,
  parseBytesToJson,
  markdownToHtml,
  inputFormats,
  outputFormats,
} from "../bundler/docmux_wasm.js";
```

Note: `./raw` export only serves bundler consumers. Node.js consumers using `./raw` get routed to `node/docmux_wasm.js` directly by the conditional exports in `package.json` — no `raw.node.ts` needed.

- [ ] **Step 3: Verify TypeScript compiles with all three files**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux/npm && pnpm run build:ts`

Expected: `dist/` contains `index.js`, `index.d.ts`, `index.node.js`, `index.node.d.ts`, `raw.js`, `raw.d.ts`. No errors.

- [ ] **Step 4: Commit**

```bash
git add npm/src/index.node.ts npm/src/raw.ts
git commit -m "feat(npm): add Node.js entry point and raw wasm-bindgen re-exports"
```

---

### Task 4: Node.js smoke test

**Files:**
- Create: `npm/test.mjs`

- [ ] **Step 1: Create `npm/test.mjs`**

```javascript
import { strict as assert } from "node:assert";

// Test the Node.js entry point directly (bypass conditional exports for testing)
import {
  convert as wasmConvert,
  inputFormats,
  outputFormats,
  markdownToHtml,
} from "./node/docmux_wasm.js";

// --- Raw bindings work ---

const html = wasmConvert("# Hello\n\nWorld", "markdown", "html");
assert(html.includes("<h1"), "convert should produce h1");
console.log("  ✓ convert (raw)");

const inputs = inputFormats();
assert(inputs.includes("markdown"), "should include markdown");
assert(inputs.includes("latex"), "should include latex");
assert(inputs.includes("docx"), "should include docx");
console.log("  ✓ inputFormats");

const outputs = outputFormats();
assert(outputs.includes("html"), "should include html");
assert(outputs.includes("latex"), "should include latex");
console.log("  ✓ outputFormats");

// --- Wrapper API works ---

const wrapper = await import("./dist/index.node.js");

const result = await wrapper.convert("**bold**", "markdown", "html");
assert.equal(result.error, null, "should not error");
assert(result.output.includes("<strong>"), "should produce strong tag");
console.log("  ✓ wrapper convert");

const mdResult = await wrapper.markdownToHtml("# Test");
assert.equal(mdResult.error, null);
assert(mdResult.output.includes("<h1"), "markdownToHtml should produce h1");
console.log("  ✓ wrapper markdownToHtml");

const badResult = await wrapper.convert("hello", "nonexistent", "html");
assert(badResult.error !== null, "bad format should return error");
assert.equal(badResult.output, null);
console.log("  ✓ wrapper error handling");

const formats = await wrapper.getInputFormats();
assert(Array.isArray(formats), "should return array");
assert(formats.length > 0, "should have formats");
console.log("  ✓ wrapper getInputFormats");

console.log("\nAll npm package tests passed ✓");
```

- [ ] **Step 2: Run the full build + test**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux/npm && pnpm run build && node test.mjs`

Expected: All assertions pass, prints "All npm package tests passed ✓".

- [ ] **Step 3: Commit**

```bash
git add npm/test.mjs
git commit -m "test(npm): add Node.js smoke tests for raw bindings and wrapper API"
```

---

### Task 5: Set up pnpm workspace and migrate playground

**Files:**
- Create: `pnpm-workspace.yaml`
- Modify: `playground/package.json`
- Modify: `playground/src/wasm/docmux.ts`

- [ ] **Step 1: Create `pnpm-workspace.yaml` at the repo root**

```yaml
packages:
  - playground
  - npm
```

- [ ] **Step 2: Add `@docmux/wasm` dependency to playground**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux/playground && pnpm add @docmux/wasm@workspace:*`

This adds `"@docmux/wasm": "workspace:*"` to `playground/package.json` dependencies.

- [ ] **Step 3: Rewrite `playground/src/wasm/docmux.ts`**

Replace the entire file with:

```typescript
export {
  convert,
  convertStandalone,
  convertBytes,
  convertBytesStandalone,
  parseToJson,
  parseBytesToJson,
  markdownToHtml,
  getInputFormats,
  getOutputFormats,
} from "@docmux/wasm";

export type {
  ConvertOutcome,
  ConversionResult,
  ConversionError,
} from "@docmux/wasm";
```

- [ ] **Step 4: Verify playground types still check**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux/playground && pnpm exec tsc --noEmit`

Expected: No type errors. The `useConversion.ts` hook imports `ConvertOutcome` from `@/wasm/docmux`, which now re-exports from `@docmux/wasm` — the type is identical.

- [ ] **Step 5: Verify playground builds**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux/playground && pnpm run build`

Expected: Vite build succeeds. WASM loads correctly through the workspace link.

- [ ] **Step 6: Commit**

```bash
git add pnpm-workspace.yaml playground/package.json playground/pnpm-lock.yaml playground/src/wasm/docmux.ts
git commit -m "refactor(playground): consume @docmux/wasm via pnpm workspace link"
```

---

### Task 6: Update pre-commit hook and clean up old wasm-pkg

**Files:**
- Modify: `.githooks/pre-commit`
- Modify: `.gitignore`
- Delete: `wasm-pkg/`

- [ ] **Step 1: Update the WASM check in `.githooks/pre-commit`**

Replace lines 85-93 (the WASM check section):

```bash
# ─── WASM check ─────────────────────────────────────────────────────
if [ "$WASM_CHANGED" = true ]; then
    section "WASM"

    if cargo build --target wasm32-unknown-unknown -p docmux-wasm 2>/dev/null; then
        pass "wasm32 build"
    else
        fail "wasm32 build — WASM crate doesn't compile"
    fi
fi
```

With:

```bash
# ─── WASM check ─────────────────────────────────────────────────────
if [ "$WASM_CHANGED" = true ]; then
    section "WASM"

    if cargo build --target wasm32-unknown-unknown -p docmux-wasm 2>/dev/null; then
        pass "wasm32 build"
    else
        fail "wasm32 build — WASM crate doesn't compile"
    fi

    # Full wasm-pack build to keep npm package in sync
    if npm/scripts/build.sh >/dev/null 2>&1; then
        pass "wasm-pack dual build"
    else
        warn "wasm-pack build failed — npm package may be stale"
    fi
fi
```

- [ ] **Step 2: Also detect changes in `npm/src/` as TS changes**

In the detection loop (lines 27-33), add a case for `npm/src/`:

Replace:

```bash
        playground/*.ts|playground/*.tsx) TS_CHANGED=true ;;
```

With:

```bash
        playground/*.ts|playground/*.tsx) TS_CHANGED=true ;;
        npm/src/*.ts) TS_CHANGED=true; WASM_CHANGED=true ;;
```

- [ ] **Step 3: Remove old `wasm-pkg/` directory**

Run: `rm -rf /Users/augustochirico/Documents/src/side-projects/docmux/wasm-pkg`

- [ ] **Step 4: Clean up `.gitignore`**

Remove these lines that are now redundant (the old wasm-pkg references):

```
playground/wasm-pkg/
```

Keep the new lines added in Task 1:

```
npm/bundler/
npm/node/
npm/dist/
```

Also keep `*.wasm` and `pkg/` since they still serve as general safety catches.

- [ ] **Step 5: Copy LICENSE and create README.md for the npm package**

Run: `cp /Users/augustochirico/Documents/src/side-projects/docmux/LICENSE /Users/augustochirico/Documents/src/side-projects/docmux/npm/LICENSE`

Create `npm/README.md`:

```markdown
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
import init, { convert, inputFormats } from "@docmux/wasm/raw";
await init(); // manual initialization required
const html = convert("# Hello", "markdown", "html");
```

## License

MIT
```

- [ ] **Step 6: Verify the full pre-commit hook works**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux && .githooks/pre-commit`

Expected: All checks pass (or only expected warnings).

- [ ] **Step 7: Commit**

```bash
git add .githooks/pre-commit .gitignore npm/LICENSE npm/README.md
git rm -r --cached wasm-pkg/ 2>/dev/null || true
git commit -m "chore: update pre-commit hook for npm package, remove old wasm-pkg"
```

---

### Task 7: Final verification

- [ ] **Step 1: Run workspace Rust tests**

Run: `cargo test --workspace`

Expected: All 507+ tests pass. Nothing broken by the npm package changes (no Rust code was modified).

- [ ] **Step 2: Run npm package smoke tests**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux/npm && pnpm run build && node test.mjs`

Expected: All tests pass.

- [ ] **Step 3: Run playground build**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux/playground && pnpm run build`

Expected: Build succeeds.

- [ ] **Step 4: Run playground type check and lint**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux/playground && pnpm exec tsc --noEmit && pnpm run lint`

Expected: No type errors, no lint errors.

- [ ] **Step 5: Dry-run npm publish**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux/npm && npm pack --dry-run`

Expected: Lists files that would be published — `dist/`, `bundler/`, `node/`, `README.md`, `LICENSE`. No `src/`, no `scripts/`, no `test.mjs`. Package size should be ~2MB (two .wasm binaries ~860KB each + JS glue + types).

- [ ] **Step 6: Verify conditional exports resolve correctly**

Run: `cd /Users/augustochirico/Documents/src/side-projects/docmux/npm && node -e "import('@docmux/wasm').then(m => console.log(Object.keys(m)))"`

Expected: Prints the exported function names: `convert`, `convertStandalone`, `convertBytes`, `convertBytesStandalone`, `parseToJson`, `parseBytesToJson`, `markdownToHtml`, `getInputFormats`, `getOutputFormats`, `ConversionResult`, `ConversionError`, `ConvertOutcome`.
