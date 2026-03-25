#!/usr/bin/env bash
set -uo pipefail

# ─── docmux quality gate (Claude Code Stop hook) ────────────────────
# Two phases:
#   Phase 1 — Code review: Injects quality checklist. Claude reviews
#             its own changes for DRY, dead code, simplicity, etc.
#             Fires once per unique change set (tracked via diff hash).
#   Phase 2 — Mechanical checks: cargo fmt, clippy, test, tsc, eslint.
#             Runs every time after the review phase has been done.
# ─────────────────────────────────────────────────────────────────────

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$REPO_ROOT"

# ─── Detect code changes (staged + unstaged + untracked) ────────────
RUST_CHANGED=$(git diff --name-only HEAD -- '*.rs' 2>/dev/null; git ls-files --others --exclude-standard -- '*.rs' 2>/dev/null)
TS_CHANGED=$(git diff --name-only HEAD -- 'playground/*.ts' 'playground/*.tsx' 2>/dev/null; git ls-files --others --exclude-standard -- 'playground/*.ts' 'playground/*.tsx' 2>/dev/null)

# Fast path: no code changes → exit immediately
if [ -z "$RUST_CHANGED" ] && [ -z "$TS_CHANGED" ]; then
    exit 0
fi

# ─── Phase 1: Code review (once per change set) ─────────────────────
DIFF_HASH=$(git diff HEAD 2>/dev/null | shasum -a 256 | cut -d' ' -f1)
MARKER="/tmp/docmux-qg-${DIFF_HASH}"

if [ ! -f "$MARKER" ]; then
    touch "$MARKER"
    cat "$REPO_ROOT/.claude/hooks/quality-review.md"
    exit 2
fi

# ─── Phase 2: Mechanical checks (after review is done) ──────────────
FAILURES=""

if [ -n "$RUST_CHANGED" ]; then
    if ! cargo fmt --all -- --check >/dev/null 2>&1; then
        FAILURES="${FAILURES}- cargo fmt: formatting issues (run 'cargo fmt --all')\n"
    fi
    if ! cargo clippy --workspace --all-targets -- -D warnings >/dev/null 2>&1; then
        FAILURES="${FAILURES}- cargo clippy: warnings or errors found\n"
    fi
    if ! cargo test --workspace >/dev/null 2>&1; then
        FAILURES="${FAILURES}- cargo test: test failures\n"
    fi
fi

if [ -n "$TS_CHANGED" ]; then
    if ! (cd playground && pnpm exec tsc --noEmit >/dev/null 2>&1); then
        FAILURES="${FAILURES}- tsc --noEmit: type errors in playground\n"
    fi
    if ! (cd playground && pnpm run lint >/dev/null 2>&1); then
        FAILURES="${FAILURES}- eslint: lint errors in playground\n"
    fi
fi

if [ -n "$FAILURES" ]; then
    echo "MECHANICAL CHECKS FAILED — fix before completing:"
    echo -e "$FAILURES"
    echo "Run the failing commands, fix the issues, then try again."
    # Clear the review marker so the review re-runs after fixes
    rm -f "$MARKER"
    exit 2
fi

exit 0
