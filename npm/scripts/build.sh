#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
NPM_DIR="$REPO_ROOT/npm"

echo "Building bundler target..."
wasm-pack build "$REPO_ROOT/crates/docmux-wasm" \
  --target bundler \
  --out-dir "$NPM_DIR/bundler" \
  --out-name docmux_wasm

echo "Building web target..."
wasm-pack build "$REPO_ROOT/crates/docmux-wasm" \
  --target web \
  --out-dir "$NPM_DIR/web" \
  --out-name docmux_wasm

echo "Building nodejs target..."
wasm-pack build "$REPO_ROOT/crates/docmux-wasm" \
  --target nodejs \
  --out-dir "$NPM_DIR/node" \
  --out-name docmux_wasm

# Clean up wasm-pack artifacts
rm -f "$NPM_DIR/bundler/package.json" "$NPM_DIR/bundler/.gitignore" "$NPM_DIR/bundler/README.md"
rm -f "$NPM_DIR/web/package.json" "$NPM_DIR/web/.gitignore" "$NPM_DIR/web/README.md"
rm -f "$NPM_DIR/node/package.json" "$NPM_DIR/node/.gitignore" "$NPM_DIR/node/README.md"

# ── Convert node target from CJS to ESM ──────────────────────────────
# wasm-pack --target nodejs emits CommonJS (exports.X, require, __dirname).
# Our package uses "type": "module", so we need ESM syntax.
echo "Converting node bindings from CJS to ESM..."
NODE_JS="$NPM_DIR/node/docmux_wasm.js"

# Use a single Node.js script to do the CJS→ESM conversion reliably
node -e "
const fs = require('fs');
let src = fs.readFileSync('$NODE_JS', 'utf8');

// 1. Collect exported names from 'exports.X = X;' lines
const exportNames = [];
src = src.replace(/^exports\.(\w+) = \w+;$/gm, (_match, name) => {
  exportNames.push(name);
  return '';  // remove the line
});

// 2. Replace __dirname + require('fs') wasm-loading block with ESM equivalent
src = src.replace(
  /const wasmPath = .*__dirname.*\n.*require\('fs'\)\.readFileSync\(wasmPath\);/,
  \`import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
const __node_dirname = fileURLToPath(new URL('.', import.meta.url));
const wasmPath = __node_dirname + '/docmux_wasm_bg.wasm';
const wasmBytes = readFileSync(wasmPath);\`
);

// 3. Move the imports to the top (after the @ts-self-types comment if present)
const importBlock = src.match(/^import \{[^}]+\} from '[^']+';$/gm) || [];
for (const imp of importBlock) {
  src = src.replace(imp + '\n', '');
}
const tsComment = src.match(/^\/\* @ts-self-types=.*\*\/\n/);
if (tsComment) {
  src = src.replace(tsComment[0], tsComment[0] + importBlock.join('\n') + '\n');
} else {
  src = importBlock.join('\n') + '\n' + src;
}

// 4. Append named export list
src = src.trimEnd() + '\n\nexport { ' + exportNames.join(', ') + ' };\n';

fs.writeFileSync('$NODE_JS', src);
console.log('  Converted exports:', exportNames.join(', '));
"

echo "WASM build complete."
