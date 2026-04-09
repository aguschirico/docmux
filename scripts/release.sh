#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_TOML="$REPO_ROOT/Cargo.toml"
NPM_PKG="$REPO_ROOT/npm/package.json"

# ---------------------------------------------------------------------------
# 1. Validate argument
# ---------------------------------------------------------------------------
if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <version>  (e.g. 0.2.0)" >&2
    exit 1
fi

VERSION="$1"

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "Error: version must be semver X.Y.Z (got: '$VERSION')" >&2
    exit 1
fi

TAG="v$VERSION"

# ---------------------------------------------------------------------------
# 2. Working tree must be clean
# ---------------------------------------------------------------------------
if ! git -C "$REPO_ROOT" diff --quiet || ! git -C "$REPO_ROOT" diff --cached --quiet; then
    echo "Error: working tree is not clean. Commit or stash your changes first." >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# 3. Tag must not already exist
# ---------------------------------------------------------------------------
if git -C "$REPO_ROOT" tag --list | grep -qx "$TAG"; then
    echo "Error: tag '$TAG' already exists." >&2
    exit 1
fi

echo "==> Releasing $TAG"

# ---------------------------------------------------------------------------
# 4. Bump versions
# ---------------------------------------------------------------------------
echo "==> Bumping version in Cargo.toml and npm/package.json"

# Cargo.toml: replace `version = "X.Y.Z"` under [workspace.package]
# Use a temp .bak file for macOS compatibility
sed -i.bak 's/^version = "[0-9]*\.[0-9]*\.[0-9]*"$/version = "'"$VERSION"'"/' "$CARGO_TOML"
rm -f "$CARGO_TOML.bak"

# npm/package.json: replace `"version": "X.Y.Z"`
sed -i.bak 's/"version": "[0-9]*\.[0-9]*\.[0-9]*"/"version": "'"$VERSION"'"/' "$NPM_PKG"
rm -f "$NPM_PKG.bak"

# Verify the bumps landed
if ! grep -q "^version = \"$VERSION\"" "$CARGO_TOML"; then
    echo "Error: failed to update version in Cargo.toml" >&2
    exit 1
fi

if ! grep -q "\"version\": \"$VERSION\"" "$NPM_PKG"; then
    echo "Error: failed to update version in npm/package.json" >&2
    exit 1
fi

# ---------------------------------------------------------------------------
# 5. Quality gates
# ---------------------------------------------------------------------------
echo "==> cargo check"
cargo check --workspace --manifest-path "$CARGO_TOML"

echo "==> cargo test"
cargo test --workspace --manifest-path "$CARGO_TOML"

echo "==> cargo clippy"
cargo clippy --workspace --all-targets --quiet --manifest-path "$CARGO_TOML" -- -D warnings

# ---------------------------------------------------------------------------
# 6. Commit
# ---------------------------------------------------------------------------
echo "==> Committing version bump"
git -C "$REPO_ROOT" add "$CARGO_TOML" "$NPM_PKG"
git -C "$REPO_ROOT" commit -m "release: $TAG"

# ---------------------------------------------------------------------------
# 7. Tag
# ---------------------------------------------------------------------------
echo "==> Tagging $TAG"
git -C "$REPO_ROOT" tag "$TAG"

# ---------------------------------------------------------------------------
# 8. Push branch + tag
# ---------------------------------------------------------------------------
echo "==> Pushing branch and tag"
git -C "$REPO_ROOT" push
git -C "$REPO_ROOT" push origin "$TAG"

echo ""
echo "Released $TAG successfully."
