#!/usr/bin/env bash
set -uo pipefail

# ─── docmux quality gate (Claude Code Stop hook) ────────────────────
# Reads JSON from stdin. Uses stop_hook_active to prevent infinite loops.
# Outputs JSON to stdout with {"decision":"block","reason":"..."} to block.
# Exit 0 with no JSON = allow Claude to stop.
# ─────────────────────────────────────────────────────────────────────

# Read hook input from stdin
INPUT=$(cat)

# CRITICAL: Break infinite loop — if Claude is already continuing from
# a previous stop hook, allow it to stop this time.
STOP_HOOK_ACTIVE=$(echo "$INPUT" | jq -r '.stop_hook_active // false')
if [ "$STOP_HOOK_ACTIVE" = "true" ]; then
    exit 0
fi

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$REPO_ROOT"

# ─── Detect code changes (staged + unstaged + untracked) ────────────
RUST_CHANGED=$(git diff --name-only HEAD -- '*.rs' 2>/dev/null; git ls-files --others --exclude-standard -- '*.rs' 2>/dev/null)
TS_CHANGED=$(git diff --name-only HEAD -- 'playground/*.ts' 'playground/*.tsx' 2>/dev/null; git ls-files --others --exclude-standard -- 'playground/*.ts' 'playground/*.tsx' 2>/dev/null)

# No code changes → allow stop
if [ -z "$RUST_CHANGED" ] && [ -z "$TS_CHANGED" ]; then
    exit 0
fi

# ─── Phase 1: Code review (once per change set) ─────────────────────
DIFF_HASH=$(git diff HEAD 2>/dev/null | shasum -a 256 | cut -d' ' -f1)
MARKER="/tmp/docmux-qg-${DIFF_HASH}"

if [ ! -f "$MARKER" ]; then
    touch "$MARKER"
    REVIEW=$(cat "$REPO_ROOT/.claude/hooks/quality-review.md")
    jq -n --arg reason "$REVIEW" '{"decision":"block","reason":$reason}'
    exit 0
fi

# ─── Phase 2: Mechanical checks (once per change set, after review) ──
MECH_MARKER="/tmp/docmux-qg-mech-${DIFF_HASH}"
if [ -f "$MECH_MARKER" ]; then
    exit 0
fi

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
    REASON=$(printf "MECHANICAL CHECKS FAILED — fix before completing:\n%b\nRun the failing commands, fix the issues, then try again." "$FAILURES")
    rm -f "$MARKER" "$MECH_MARKER"
    jq -n --arg reason "$REASON" '{"decision":"block","reason":$reason}'
    exit 0
fi

# All checks passed
touch "$MECH_MARKER"
exit 0
