# Markdown Reader & CLI Extensions — Design Spec

> Date: 2026-04-03

Four features to close out Phase 3 markdown reader and CLI gaps (excluding template engine, which gets its own spec).

## 1. `raw_attribute` Syntax (Markdown Reader)

### What

Pandoc extension: `` `code`{=format} `` produces `Inline::RawInline { format, content }` and ` ```{=format} ` produces `Block::RawBlock { format, content }`.

### Why

Allows authors to embed format-specific content (raw HTML, raw LaTeX) inline or as blocks, with the reader routing it to the correct AST node. Writers already handle `RawBlock`/`RawInline` — they pass through content matching their format and silently drop the rest.

### Approach

Post-processing in the markdown reader (comrak has no native `raw_attribute` extension).

**Inline raw:** After parsing an `Inline::Code` node, check if the source text has a trailing `{=format}` suffix. Extend the existing attribute parser (`parse_attributes`) to detect the `=` prefix as a raw format specifier. When found, replace the `Code` node with `RawInline { format, content }`.

**Block raw:** In `parse_code_block`, if the info string matches `{=format}` (starts with `{=` and ends with `}`), produce `Block::RawBlock { format, content }` instead of `Block::CodeBlock`.

### Edge cases

- `` `code`{=html .class} `` — the `=format` takes precedence; classes are ignored (pandoc behavior).
- Empty format `` `code`{=} `` — treat as regular code (no transformation).
- Info string `{=html}` on a code block — entire content is raw, no syntax highlighting.

### Testing

- Inline: `` `<b>bold</b>`{=html} `` → `RawInline { format: "html", content: "<b>bold</b>" }`
- Inline: `` `\textbf{bold}`{=latex} `` → `RawInline { format: "latex", content: "\\textbf{bold}" }`
- Block: ` ```{=html}\n<div>raw</div>\n``` ` → `RawBlock { format: "html", content: "<div>raw</div>" }`
- Roundtrip: HTML writer outputs raw HTML blocks, drops raw LaTeX blocks.
- Golden file test with mixed raw attributes.

## 2. Table Captions (Markdown Reader)

### What

Parse `Table:` or `:` caption paragraphs adjacent to tables and populate `Table.caption`.

### Why

The `Table.caption: Option<Vec<Inline>>` field exists in the AST but the markdown reader always sets it to `None`. Pandoc supports table captions via a paragraph starting with `Table:` or `: ` immediately before or after a table.

### Approach

Post-processing pass over the `Vec<Block>` after initial parsing.

1. Iterate through blocks looking for `Paragraph` + `Table` or `Table` + `Paragraph` pairs.
2. Check if the paragraph's inline content starts with the text `Table:` or `: `.
3. If matched, strip the prefix, assign remaining inlines to `Table.caption`, and remove the `Paragraph` from the block list.
4. Caption before the table takes priority (pandoc convention). If both positions have captions, use the one above.

### Writer support check

Verify that HTML, LaTeX, Typst, and Markdown writers render `Table.caption` when present:
- HTML: `<caption>...</caption>` inside `<table>`.
- LaTeX: `\caption{...}` inside `table` environment.
- Typst: caption parameter on `#table` or `#figure`.
- Markdown: re-emit as `Table: caption text` above the table.

### Testing

- `: Simple caption` above a table → `Table.caption = Some([Text("Simple caption")])`.
- `Table: **Bold** caption` → caption with `Strong` inline node.
- Caption below table (no caption above) → captured.
- Caption both above and below → above wins.
- Paragraph starting with `:` but not adjacent to a table → left alone (adjacency is determined by position in the `Vec<Block>` — no intervening blocks between paragraph and table).
- Golden file test for HTML + LaTeX output with captioned tables.

## 3. `--id-prefix=PREFIX` (CLI + Markdown Reader)

### What

CLI flag to prepend a string to all auto-generated heading IDs.

### Why

Useful when combining multiple documents to avoid ID collisions. Pandoc supports `--id-prefix`.

### Approach

1. Add `--id-prefix` flag to `Cli` struct in `main.rs`.
2. Add `id_prefix: Option<String>` to `ReadOptions` (in `docmux-core`).
3. In the markdown reader's auto-ID generation (slugify function), prepend the prefix to the generated slug.
4. Explicit IDs (`# Heading {#my-id}`) are **not** prefixed — they are intentional.
5. Cross-ref and ToC transforms work automatically since they reference IDs from the AST.

### Testing

- `--id-prefix=ch1-` with `# Hello` → heading ID `ch1-hello`.
- Explicit `# Hello {#custom}` → ID stays `custom` (no prefix).
- Cross-refs to auto-generated IDs resolve correctly with prefix.
- ToC links use prefixed IDs.

## 4. `--section-divs` (Transform + CLI)

### What

New transform crate `docmux-transform-section-divs` that wraps heading-delimited sections in `Block::Div` nodes.

### Why

Pandoc's `--section-divs` wraps each section (heading + content until next heading of same/higher level) in a container. Useful for styling, JavaScript targeting, and semantic structure. Modeled as an AST transform (not writer-specific logic) to maintain the Reader → AST → Transform → Writer architecture.

### Approach

**Algorithm:**
1. Process blocks linearly. When a `Heading` of level N is found, collect it plus all subsequent blocks until the next heading of level ≤ N (or end of document).
2. Wrap the collected blocks in a `Block::Div` with:
   - `id`: moved from the heading (heading's ID is cleared to avoid duplication).
   - `classes`: `["section", "level{N}"]`.
   - No extra `key_values`.
3. Apply recursively: a level-2 heading inside a level-1 section produces a nested div.

**Crate:** `docmux-transform-section-divs` in `crates/`, implementing `Transform` trait.

**CLI:** Add `--section-divs` boolean flag. Apply the transform in the pipeline after `--number-sections` and before `--toc`.

**Writer behavior:** `Block::Div` already renders as `<div>` in HTML. Rendering as `<section>` is a future enhancement — `<div class="section level2">` matches pandoc's default output.

### Testing

- Single heading + content → wrapped in one div.
- Two headings same level → two sibling divs.
- Nested headings (h1 + h2) → nested divs.
- Content before first heading → not wrapped (stays at top level).
- Empty section (heading with no content before next heading) → div with just the heading.
- Interaction with `--number-sections`: numbering applied first, then section-divs wraps.
- Interaction with `--toc`: ToC links resolve to section div IDs.
- 7+ unit tests covering these cases.

## Implementation order

1. `raw_attribute` — extends existing attribute parsing, self-contained in markdown reader.
2. `table captions` — post-processing pass in markdown reader, may need writer fixes.
3. `--id-prefix` — touches core + reader + CLI, but minimal code.
4. `--section-divs` — new crate, most code, but well-isolated.

## Out of scope

- Template engine (`--template=FILE`) — separate spec.
- `--bibliography`, `--csl` — depends on cite transform, separate work.
- Rendering `<section>` instead of `<div>` in HTML writer — future enhancement.
