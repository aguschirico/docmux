---
name: release
description: Release a new version of docmux. Use when the user says "release", "publish", "bump version", "cut a release", or wants to ship a new version to npm.
user-invocable: true
---

# Release

Run `scripts/release.sh <version>` with a semver version (e.g. `0.4.1`).

The script handles everything: version bump (Cargo.toml + npm/package.json), quality gates (check, test, clippy), commit, tag, and push. A GitHub Actions workflow then publishes to npm automatically when the tag lands.

## Choosing the version

- **Patch** (0.4.0 → 0.4.1): bug fixes, no new features
- **Minor** (0.4.0 → 0.5.0): new features, backward compatible
- **Major** (0.4.0 → 1.0.0): breaking changes

## Prerequisites

- Working tree must be clean (all changes committed)
- The tag must not already exist

## Steps

1. Determine the next version based on what changed since the last release.
2. Run: `./scripts/release.sh <version>`
3. Confirm the GitHub Actions workflow passes at the repo's Actions tab.
