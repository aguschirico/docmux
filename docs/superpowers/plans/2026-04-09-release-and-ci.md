# Release Script + CI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a release script that bumps versions across Cargo + npm, tags, and pushes — plus CI workflows that validate PRs and auto-publish to npm on tags.

**Architecture:** Local `scripts/release.sh` handles version bumping and git tagging. GitHub Actions CI validates all pushes/PRs (Rust checks + playground TS). A separate release workflow triggers on `v*` tags, builds WASM, and publishes to npm.

**Tech Stack:** Bash, GitHub Actions, wasm-pack, pnpm, npm publish

---

### Task 1: Create `scripts/release.sh`

**Files:**
- Create: `scripts/release.sh`

- [ ] **Step 1: Write the release script**

```bash
#!/usr/bin/env bash
set -euo pipefail

# ── Args ────────────────────────────────────────────────────────────────
VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
  echo "Usage: ./scripts/release.sh <version>" >&2
  echo "Example: ./scripts/release.sh 0.2.0" >&2
  exit 1
fi

# Validate semver (basic: X.Y.Z)
if ! echo "$VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
  echo "Error: '$VERSION' is not valid semver (expected X.Y.Z)" >&2
  exit 1
fi

TAG="v$VERSION"

# ── Preflight ───────────────────────────────────────────────────────────
if [[ -n "$(git status --porcelain)" ]]; then
  echo "Error: working tree is not clean. Commit or stash changes first." >&2
  exit 1
fi

if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "Error: tag '$TAG' already exists." >&2
  exit 1
fi

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "==> Releasing docmux $TAG"

# ── Bump versions ───────────────────────────────────────────────────────
echo "  Bumping Cargo.toml workspace version..."
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" "$REPO_ROOT/Cargo.toml"
rm -f "$REPO_ROOT/Cargo.toml.bak"

echo "  Bumping npm/package.json version..."
sed -i.bak "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" "$REPO_ROOT/npm/package.json"
rm -f "$REPO_ROOT/npm/package.json.bak"

# ── Validate ────────────────────────────────────────────────────────────
echo "  Running cargo check..."
cargo check --workspace --quiet

echo "  Running tests..."
cargo test --workspace --quiet

echo "  Running clippy..."
cargo clippy --workspace --all-targets --quiet -- -D warnings

# ── Commit, tag, push ──────────────────────────────────────────────────
echo "  Committing..."
git add "$REPO_ROOT/Cargo.toml" "$REPO_ROOT/npm/package.json"
git commit -m "release: $TAG"

echo "  Tagging $TAG..."
git tag "$TAG"

echo "  Pushing branch + tag..."
git push
git push origin "$TAG"

echo ""
echo "Done! $TAG pushed. CI will build and publish to npm."
```

- [ ] **Step 2: Make it executable**

Run: `chmod +x scripts/release.sh`

- [ ] **Step 3: Verify the script parses correctly**

Run: `bash -n scripts/release.sh`
Expected: no output (no syntax errors)

- [ ] **Step 4: Test validation flags (dry run)**

Run: `./scripts/release.sh` (no args)
Expected: "Usage: ./scripts/release.sh <version>"

Run: `./scripts/release.sh not-semver`
Expected: "Error: 'not-semver' is not valid semver"

- [ ] **Step 5: Commit**

```bash
git add scripts/release.sh
git commit -m "feat: add release script for version bumping and tagging"
```

---

### Task 2: Update CI workflow — add playground job

**Files:**
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add playground job to ci.yml**

Replace the full file with:

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -Dwarnings

jobs:
  check:
    name: Check & Clippy
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - uses: Swatinem/rust-cache@v2
      - run: cargo check --workspace --all-targets
      - run: cargo clippy --workspace --all-targets

  test:
    name: Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --workspace

  fmt:
    name: Format
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  wasm:
    name: WASM Build
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown
      - uses: Swatinem/rust-cache@v2
      - run: cargo build --target wasm32-unknown-unknown -p docmux-wasm

  playground:
    name: Playground (TS)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v4
        with:
          version: 9
      - uses: actions/setup-node@v4
        with:
          node-version: 22
          cache: pnpm
      - run: pnpm install --frozen-lockfile
      - name: Type check
        run: pnpm exec tsc --noEmit
        working-directory: playground
      - name: Lint
        run: pnpm exec eslint .
        working-directory: playground
```

- [ ] **Step 2: Validate YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"`
Expected: no output (valid YAML)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: add playground TypeScript checks to CI"
```

---

### Task 3: Create release workflow

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1: Write the release workflow**

```yaml
name: Release

on:
  push:
    tags: ["v*"]

env:
  CARGO_TERM_COLOR: always
  RUSTFLAGS: -Dwarnings

jobs:
  validate:
    name: Validate
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
          targets: wasm32-unknown-unknown
      - uses: Swatinem/rust-cache@v2
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace --all-targets
      - run: cargo test --workspace
      - run: cargo build --target wasm32-unknown-unknown -p docmux-wasm

  publish-npm:
    name: Publish to npm
    needs: validate
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: wasm32-unknown-unknown

      - uses: Swatinem/rust-cache@v2

      - name: Install wasm-pack
        run: curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh

      - uses: pnpm/action-setup@v4
        with:
          version: 9

      - uses: actions/setup-node@v4
        with:
          node-version: 22
          registry-url: https://registry.npmjs.org
          cache: pnpm

      - run: pnpm install --frozen-lockfile

      - name: Build WASM + TS wrapper
        run: pnpm --filter @docmux/wasm run build

      - name: Publish to npm
        run: npm publish --access public
        working-directory: npm
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

- [ ] **Step 2: Validate YAML syntax**

Run: `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`
Expected: no output (valid YAML)

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/release.yml
git commit -m "ci: add release workflow for automated npm publish on tags"
```

---

### Task 4: Update ROADMAP.md

**Files:**
- Modify: `ROADMAP.md`

- [ ] **Step 1: Mark npm package and CI items as done**

In the Phase 3 Packaging section (line 119), change:
```
- [ ] `npm/` package — publishable `@docmux/wasm` with JS/TS wrapper
```
to:
```
- [x] `npm/` package — `@docmux/wasm` published with JS/TS wrapper, automated release
```

In Phase 4 Other section (line 160), change:
```
- [ ] Publish to crates.io + npm
```
to:
```
- [x] Publish to npm (automated via CI on tag push)
- [ ] Publish to crates.io
```

Update the "Last updated" date on line 3 to `2026-04-09`.

- [ ] **Step 2: Commit**

```bash
git add ROADMAP.md
git commit -m "docs(roadmap): mark npm publish and CI automation as complete"
```
