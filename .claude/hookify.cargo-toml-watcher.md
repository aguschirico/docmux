---
name: warn-cargo-toml-doc-update
enabled: true
event: file
conditions:
  - field: file_path
    operator: ends_with
    pattern: docmux/Cargo.toml
  - field: new_text
    operator: regex_match
    pattern: members|docmux-
action: warn
---

**Workspace Cargo.toml modified — documentation may need updating.**

If you added or removed a crate from `[workspace] members`, update these files:
1. `CLAUDE.md` — crate count in Architecture section
2. `memory/project_status.md` — crate count and test count
3. If new format-specific crate, create a `.claude/rules/{name}.md` with path-scoped rules
