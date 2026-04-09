---
name: check-doc-staleness-on-stop
enabled: true
event: stop
action: warn
---

**Documentation freshness check before completing.**

If this session added crates, changed conventions, or completed a milestone:
1. Verify `CLAUDE.md` crate count matches `ls -d crates/*/  | wc -l`
2. Verify `memory/project_status.md` test count matches `cargo test --workspace` output
3. If a new `.claude/rules/` file is needed for format-specific architecture, create it
4. If a roadmap phase was completed, update the phase status in CLAUDE.md and memory
