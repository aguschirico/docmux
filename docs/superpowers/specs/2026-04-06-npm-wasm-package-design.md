# @docmux/wasm npm Package — Design Spec (v2)

> Date: 2026-04-06
> Status: Approved
> Scope: `npm/` directory, playground migration
> Supersedes: `2026-03-24-npm-wasm-package-design.md`

## Goal

Publish `@docmux/wasm` to npm — a dual-target (bundler + Node.js) WASM package that exposes docmux's document conversion capabilities to JavaScript/TypeScript consumers, with a high-level wrapper API and raw wasm-bindgen re-exports.

## Design Decisions

1. **Dual target** — `wasm-pack` builds twice: `--target bundler` and `--target nodejs`. Conditional `exports` in `package.json` route to the correct target.
2. **Two API layers** — main export (`@docmux/wasm`) is a TS wrapper with auto-init and structured error types. `@docmux/wasm/raw` re-exports raw wasm-bindgen bindings.
3. **Semi-automatic publish** — `pnpm run publish-wasm` builds both targets, compiles the wrapper, and publishes. No CI/CD automation yet.
4. **Playground migration** — playground consumes `@docmux/wasm` via pnpm workspace link instead of hardcoded `../../wasm-pkg/` path.

## Package Structure

```
npm/
├── package.json            # @docmux/wasm, conditional exports
├── tsconfig.json           # Compiles src/ → dist/
├── scripts/
│   └── build.sh            # wasm-pack x2 + cleanup
├── src/
│   ├── index.ts            # Main wrapper API (auto-init, ConvertOutcome)
│   ├── index.node.ts       # Node.js entry (imports from node/ target)
│   └── raw.ts              # Re-exports raw wasm-bindgen bindings
├── bundler/                # (generated, gitignored) wasm-pack --target bundler
│   ├── docmux_wasm.js
│   ├── docmux_wasm.d.ts
│   └── docmux_wasm_bg.wasm
├── node/                   # (generated, gitignored) wasm-pack --target nodejs
│   ├── docmux_wasm.js
│   ├── docmux_wasm.d.ts
│   └── docmux_wasm_bg.wasm
└── dist/                   # (generated, gitignored) tsc output
    ├── index.js
    ├── index.node.js
    ├── index.d.ts
    ├── raw.js
    └── raw.d.ts
```

## package.json

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
  "sideEffects": false
}
```

## Wrapper API

### Types

```typescript
interface ConversionResult {
  output: string;
  error: null;
}

interface ConversionError {
  output: null;
  error: string;
}

type ConvertOutcome = ConversionResult | ConversionError;
```

### Functions

All functions are async. WASM initializes lazily on first call (singleton).

```typescript
// Text input
convert(input: string, from: string, to: string): Promise<ConvertOutcome>
convertStandalone(input: string, from: string, to: string): Promise<ConvertOutcome>
parseToJson(input: string, from: string): Promise<ConvertOutcome>

// Binary input (DOCX, etc.)
convertBytes(input: Uint8Array, from: string, to: string): Promise<ConvertOutcome>
convertBytesStandalone(input: Uint8Array, from: string, to: string): Promise<ConvertOutcome>
parseBytesToJson(input: Uint8Array, from: string): Promise<ConvertOutcome>

// Convenience
markdownToHtml(input: string): Promise<ConvertOutcome>

// Metadata
getInputFormats(): Promise<string[]>
getOutputFormats(): Promise<string[]>
```

### Error handling

- WASM conversion errors (bad format, parse failure) → `ConversionError` with message string.
- Init failure (WASM binary can't load) → Promise rejection. This is a fatal environment issue, not a conversion error.

### Init behavior

- **Lazy** — WASM module loads on first function call, not on import.
- **Singleton** — `init()` runs once; all subsequent calls reuse the instance.
- **No public `init()`** — consumers don't manage initialization.

### Raw re-export (`@docmux/wasm/raw`)

Direct re-export of wasm-bindgen generated bindings. Consumers must call `init()` themselves. Useful for custom WASM loading (e.g., specific URL, custom `WebAssembly.instantiate` options).

### Shared type declarations

`index.ts` (bundler) and `index.node.ts` (Node.js) expose the same API surface — only the internal wasm-bindgen import path differs. Both compile to the same `index.d.ts` type declarations. The `tsconfig.json` compiles both entry points to `dist/`, producing `index.js`, `index.node.js`, and a single `index.d.ts`.

## Build Script (`npm/scripts/build.sh`)

```bash
#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
NPM_DIR="$REPO_ROOT/npm"

# 1. Build bundler target
wasm-pack build "$REPO_ROOT/crates/docmux-wasm" \
  --target bundler \
  --out-dir "$NPM_DIR/bundler" \
  --out-name docmux_wasm

# 2. Build nodejs target
wasm-pack build "$REPO_ROOT/crates/docmux-wasm" \
  --target nodejs \
  --out-dir "$NPM_DIR/node" \
  --out-name docmux_wasm

# 3. Clean up wasm-pack artifacts (each target generates its own package.json, .gitignore)
rm -f "$NPM_DIR/bundler/package.json" "$NPM_DIR/bundler/.gitignore"
rm -f "$NPM_DIR/node/package.json" "$NPM_DIR/node/.gitignore"
```

## Playground Migration

### pnpm workspace

Root `pnpm-workspace.yaml` (create or update):

```yaml
packages:
  - playground
  - npm
```

### Playground dependency

In `playground/package.json`:

```json
{
  "dependencies": {
    "@docmux/wasm": "workspace:*"
  }
}
```

### Import changes

`playground/src/wasm/docmux.ts` simplifies from a manual init+wrapper to a thin re-export:

```typescript
// Before
import init, {
  convert as wasmConvert,
  // ... 7 more imports
} from "../../wasm-pkg/docmux_wasm.js";

// After
import {
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

export type { ConvertOutcome, ConversionResult, ConversionError } from "@docmux/wasm";
```

The playground's `callWasm()` helper, `ensureInit()`, and manual error wrapping become unnecessary — the wrapper handles all of that.

### Pre-commit hook

The WASM build step in `.githooks/pre-commit` should be updated to build into `npm/bundler/` (via `npm/scripts/build.sh`) instead of the old `wasm-pkg/` path.

### Cleanup

Remove `wasm-pkg/` directory and its references once the playground is fully migrated.

## Gitignore

Add to root `.gitignore`:

```
npm/bundler/
npm/node/
npm/dist/
```

## What Gets Published to npm

```
@docmux/wasm@0.1.0
├── dist/index.js          # Wrapper (bundler entry)
├── dist/index.node.js     # Wrapper (Node.js entry)
├── dist/index.d.ts        # Shared type declarations
├── dist/raw.js            # Raw re-export
├── dist/raw.d.ts
├── bundler/               # wasm-pack bundler output
│   ├── docmux_wasm.js
│   ├── docmux_wasm.d.ts
│   └── docmux_wasm_bg.wasm
├── node/                  # wasm-pack nodejs output
│   ├── docmux_wasm.js
│   ├── docmux_wasm.d.ts
│   └── docmux_wasm_bg.wasm
├── README.md
└── LICENSE
```

## File Summary

| Action | Path |
|--------|------|
| Create | `npm/package.json` |
| Create | `npm/tsconfig.json` |
| Create | `npm/scripts/build.sh` |
| Create | `npm/src/index.ts` |
| Create | `npm/src/index.node.ts` |
| Create | `npm/src/raw.ts` |
| Create | `pnpm-workspace.yaml` (or update if exists) |
| Modify | `playground/package.json` (add `@docmux/wasm` dep) |
| Modify | `playground/src/wasm/docmux.ts` (use `@docmux/wasm` imports) |
| Modify | `.gitignore` (add npm generated dirs) |
| Modify | `.githooks/pre-commit` (update WASM build path) |
| Delete | `wasm-pkg/` (after migration) |
