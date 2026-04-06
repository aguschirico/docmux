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
