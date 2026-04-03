#!/usr/bin/env bash
# ─── docmux SessionStart hook ────────────────────────────────────────
# Injects live project metrics as additional context at session start.
# This ensures Claude always has accurate crate/test counts, even if
# CLAUDE.md or memory files are stale.
# ─────────────────────────────────────────────────────────────────────

REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || exit 0
cd "$REPO_ROOT"

# Count crates
CRATE_COUNT=$(ls -d crates/*/ 2>/dev/null | wc -l | tr -d ' ')

# Count tests (fast: just parse test names, don't run them)
TEST_COUNT=$(cargo test --workspace -- --list 2>/dev/null | grep -c ': test$' || echo "?")

# Get current phase from ROADMAP.md
PHASE="unknown"
if [ -f ROADMAP.md ]; then
    if grep -q "Phase 3.*in progress\|Phase 3.*🔄" ROADMAP.md 2>/dev/null; then
        PHASE="Phase 3 in progress"
    elif grep -q "Phase 3.*✅\|Phase 3.*complete" ROADMAP.md 2>/dev/null; then
        PHASE="Phase 3 complete"
    fi
fi

# Output as additionalContext
cat <<EOF
LIVE PROJECT METRICS ($(date +%Y-%m-%d)):
- Crates: $CRATE_COUNT (in crates/)
- Tests: $TEST_COUNT (workspace total)
- Status: $PHASE
If CLAUDE.md or memory show different numbers, update them.
EOF
