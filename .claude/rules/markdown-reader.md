---
paths:
  - "crates/docmux-reader-markdown/**"
---

# Markdown Reader Architecture

- Uses **comrak** with GFM extensions: tables, tasklists, footnotes, math_dollars, description_lists, front_matter_delimiter, subscript, superscript.
- **YAML frontmatter**: Two-pass parsing — first to `serde_yaml::Value` (captures everything), then extract known fields (`title`, `author`, `date`, `abstract`, `keywords`) into typed `Metadata`, rest goes to `custom: HashMap<String, MetaValue>`.
- **Author parsing**: 3 formats — single string, list of strings, list of objects (name/affiliation/email/orcid).
- **Display math fix**: comrak wraps `$$...$$` in Paragraph nodes. Detect single-child paragraphs with display math and promote to `Block::MathBlock`.
- **Raw attributes**: Parse `{=format}` syntax on code blocks → `RawBlock`, on inline code → `RawInline`.
- **Table captions**: Extract from adjacent paragraphs with `Table:` or `:` prefix.
