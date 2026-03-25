#!/usr/bin/env bash
set -uo pipefail

# ─── docmux quality gate (Claude Code Stop hook) ────────────────────
# Runs when Claude is about to finish. If there are uncommitted code
# changes, verifies all quality checks pass. Blocks completion on failure.
#
# Fast path: no code changes → exit 0 instantly.
# Slow path: runs fmt, clippy, test, tsc, eslint as needed.
# ─────────────────────────────────────────────────────────────────────

cd "$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0

# Detect uncommitted code changes (staged + unstaged + untracked)
RUST_CHANGED=$(git diff --name-only HEAD -- '*.rs' 2>/dev/null; git ls-files --others --exclude-standard -- '*.rs' 2>/dev/null)
TS_CHANGED=$(git diff --name-only HEAD -- 'playground/*.ts' 'playground/*.tsx' 2>/dev/null; git ls-files --others --exclude-standard -- 'playground/*.ts' 'playground/*.tsx' 2>/dev/null)

# Fast path: no code changes, nothing to check
if [ -z "$RUST_CHANGED" ] && [ -z "$TS_CHANGED" ]; then
    exit 0
fi

FAILURES=""

# ─── Rust checks ────────────────────────────────────────────────────
if [ -n "$RUST_CHANGED" ]; then
    # Format
    if ! cargo fmt --all -- --check >/dev/null 2>&1; then
        FAILURES="${FAILURES}- cargo fmt: formatting issues (run 'cargo fmt --all')\n"
    fi

    # Clippy (returns non-zero on warnings with -D)
    if ! cargo clippy --workspace --all-targets -- -D warnings >/dev/null 2>&1; then
        FAILURES="${FAILURES}- cargo clippy: warnings or errors found\n"
    fi

    # Tests
    if ! cargo test --workspace >/dev/null 2>&1; then
        FAILURES="${FAILURES}- cargo test: test failures\n"
    fi
fi

# ─── TypeScript checks ──────────────────────────────────────────────
if [ -n "$TS_CHANGED" ]; then
    # Type check
    if ! (cd playground && pnpm exec tsc --noEmit >/dev/null 2>&1); then
        FAILURES="${FAILURES}- tsc --noEmit: type errors in playground\n"
    fi

    # Lint
    if ! (cd playground && pnpm run lint >/dev/null 2>&1); then
        FAILURES="${FAILURES}- eslint: lint errors in playground\n"
    fi
fi

# ─── Verdict ─────────────────────────────────────────────────────────
if [ -n "$FAILURES" ]; then
    echo "QUALITY GATE FAILED — fix these before completing:"
    echo -e "$FAILURES"
    echo "Run the failing commands, fix the issues, then try again."
    exit 2
fi

exit 0
