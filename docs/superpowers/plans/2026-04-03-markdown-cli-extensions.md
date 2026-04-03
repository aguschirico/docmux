# Markdown Reader & CLI Extensions — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `raw_attribute` syntax, table captions, `--id-prefix`, and `--section-divs` to close out Phase 3 markdown/CLI gaps.

**Architecture:** Four independent features. `raw_attribute` and `table captions` are markdown reader post-processing. `--id-prefix` adds a field to `MarkdownReader` and a CLI flag. `--section-divs` is a new transform crate + CLI flag.

**Tech Stack:** Rust, comrak, clap, docmux AST/core traits.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/docmux-reader-markdown/src/lib.rs` | Modify | raw_attribute parsing, table caption extraction, id_prefix |
| `crates/docmux-writer-markdown/src/lib.rs` | Modify | Emit `Table: caption` above tables |
| `crates/docmux-cli/src/main.rs` | Modify | `--id-prefix`, `--section-divs` flags |
| `crates/docmux-cli/Cargo.toml` | Modify | Add `docmux-transform-section-divs` dep |
| `crates/docmux-transform-section-divs/Cargo.toml` | Create | New transform crate |
| `crates/docmux-transform-section-divs/src/lib.rs` | Create | Section-divs transform |
| `Cargo.toml` | Modify | Add new crate to workspace members + deps |

---

### Task 1: raw_attribute — block-level (`RawBlock`)

**Files:**
- Modify: `crates/docmux-reader-markdown/src/lib.rs:185-219`

- [ ] **Step 1: Write the failing test**

Add at the bottom of `mod tests` in `crates/docmux-reader-markdown/src/lib.rs`:

```rust
#[test]
fn raw_attribute_code_block_html() {
    let reader = MarkdownReader::new();
    let doc = reader
        .read("```{=html}\n<div class=\"custom\">raw html</div>\n```")
        .unwrap();
    assert_eq!(doc.content.len(), 1);
    match &doc.content[0] {
        Block::RawBlock { format, content } => {
            assert_eq!(format, "html");
            assert!(content.contains("<div class=\"custom\">raw html</div>"));
        }
        other => panic!("Expected RawBlock, got {:?}", other),
    }
}

#[test]
fn raw_attribute_code_block_latex() {
    let reader = MarkdownReader::new();
    let doc = reader
        .read("```{=latex}\n\\begin{tikzpicture}\n\\draw (0,0) -- (1,1);\n\\end{tikzpicture}\n```")
        .unwrap();
    assert_eq!(doc.content.len(), 1);
    match &doc.content[0] {
        Block::RawBlock { format, content } => {
            assert_eq!(format, "latex");
            assert!(content.contains("\\begin{tikzpicture}"));
        }
        other => panic!("Expected RawBlock, got {:?}", other),
    }
}

#[test]
fn raw_attribute_empty_format_stays_code_block() {
    let reader = MarkdownReader::new();
    let doc = reader.read("```{=}\nsome content\n```").unwrap();
    assert_eq!(doc.content.len(), 1);
    assert!(
        matches!(&doc.content[0], Block::CodeBlock { .. }),
        "Empty format should stay as CodeBlock, got: {:?}",
        doc.content[0]
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-markdown raw_attribute_code_block`
Expected: 2 FAIL (raw_attribute_code_block_html, raw_attribute_code_block_latex parse as CodeBlock instead of RawBlock)

- [ ] **Step 3: Implement raw_attribute block detection**

In `crates/docmux-reader-markdown/src/lib.rs`, modify the `NodeValue::CodeBlock(cb)` arm in `node_to_block` (around line 185). Add raw_attribute detection **before** the existing `{` check:

```rust
NodeValue::CodeBlock(cb) => {
    let info = cb.info.trim();
    // Raw attribute: ```{=format} → RawBlock
    if let Some(raw_fmt) = parse_raw_attribute(info) {
        return Some(Block::RawBlock {
            format: raw_fmt,
            content: cb.literal.clone(),
        });
    }
    let (language, attrs) = if info.starts_with('{') {
        // ... existing pandoc-style attribute parsing ...
```

Add the helper function near the attribute parsing section (around line 642):

```rust
/// Parse a raw attribute format specifier: `{=html}`, `{=latex}`, etc.
/// Returns the format name, or `None` if not a valid raw attribute.
fn parse_raw_attribute(info: &str) -> Option<String> {
    let s = info.trim();
    if !s.starts_with("{=") || !s.ends_with('}') {
        return None;
    }
    let fmt = s[2..s.len() - 1].trim().to_string();
    if fmt.is_empty() {
        return None;
    }
    Some(fmt)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-reader-markdown raw_attribute`
Expected: 3 PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-reader-markdown/src/lib.rs
git commit -m "feat(markdown): parse raw_attribute code blocks into RawBlock"
```

---

### Task 2: raw_attribute — inline-level (`RawInline`)

**Files:**
- Modify: `crates/docmux-reader-markdown/src/lib.rs:343-351` (collect_inlines), `846-888` (postprocess_bracketed_spans)

- [ ] **Step 1: Write the failing test**

Add to `mod tests`:

```rust
#[test]
fn raw_attribute_inline_html() {
    let reader = MarkdownReader::new();
    let doc = reader.read("`<b>bold</b>`{=html}").unwrap();
    assert_eq!(doc.content.len(), 1);
    if let Block::Paragraph { content } = &doc.content[0] {
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::RawInline { format, content } => {
                assert_eq!(format, "html");
                assert_eq!(content, "<b>bold</b>");
            }
            other => panic!("Expected RawInline, got {:?}", other),
        }
    } else {
        panic!("Expected Paragraph");
    }
}

#[test]
fn raw_attribute_inline_latex() {
    let reader = MarkdownReader::new();
    let doc = reader.read(r"`\textbf{bold}`{=latex}").unwrap();
    assert_eq!(doc.content.len(), 1);
    if let Block::Paragraph { content } = &doc.content[0] {
        assert_eq!(content.len(), 1);
        match &content[0] {
            Inline::RawInline { format, content } => {
                assert_eq!(format, "latex");
                assert_eq!(content, r"\textbf{bold}");
            }
            other => panic!("Expected RawInline, got {:?}", other),
        }
    } else {
        panic!("Expected Paragraph");
    }
}

#[test]
fn raw_attribute_inline_with_surrounding_text() {
    let reader = MarkdownReader::new();
    let doc = reader
        .read("Before `<br>`{=html} after.")
        .unwrap();
    assert_eq!(doc.content.len(), 1);
    if let Block::Paragraph { content } = &doc.content[0] {
        let has_raw = content
            .iter()
            .any(|i| matches!(i, Inline::RawInline { format, .. } if format == "html"));
        assert!(has_raw, "Expected RawInline in: {:?}", content);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-markdown raw_attribute_inline`
Expected: 3 FAIL (Code nodes remain, no RawInline produced)

- [ ] **Step 3: Implement inline raw_attribute post-processing**

In `crates/docmux-reader-markdown/src/lib.rs`, add a new post-processing function and call it from `collect_inlines`.

Add the function near `postprocess_bracketed_spans` (around line 846):

```rust
/// Walk `Vec<Inline>` in place and convert `Code` nodes followed by a `Text`
/// node starting with `{=format}` into `RawInline`.
///
/// comrak parses `` `code`{=html} `` as `Code("code")` + `Text("{=html}")`.
/// We detect this pattern and merge them into `RawInline { format, content }`.
fn postprocess_raw_inlines(inlines: &mut Vec<Inline>) {
    let mut i = 0;
    while i + 1 < inlines.len() {
        // Recurse into container inlines first.
        match &mut inlines[i] {
            Inline::Emphasis { content }
            | Inline::Strong { content }
            | Inline::Strikethrough { content }
            | Inline::Superscript { content }
            | Inline::Subscript { content }
            | Inline::SmallCaps { content }
            | Inline::Underline { content }
            | Inline::Span { content, .. }
            | Inline::Link { content, .. } => {
                postprocess_raw_inlines(content);
            }
            _ => {}
        }

        let is_code = matches!(&inlines[i], Inline::Code { .. });
        if !is_code {
            i += 1;
            continue;
        }

        // Check if the next node is Text starting with `{=format}`
        if let Inline::Text { value: next_text } = &inlines[i + 1] {
            if let Some(raw_fmt) = parse_raw_attribute_inline(next_text) {
                // Extract the code value
                let code_value = match &inlines[i] {
                    Inline::Code { value, .. } => value.clone(),
                    _ => unreachable!(),
                };
                let remaining = next_text[raw_fmt.consumed..].to_string();

                // Replace Code + partial Text with RawInline
                inlines[i] = Inline::RawInline {
                    format: raw_fmt.format,
                    content: code_value,
                };
                if remaining.is_empty() {
                    inlines.remove(i + 1);
                } else {
                    inlines[i + 1] = Inline::Text { value: remaining };
                }
                // Don't advance — recheck from the same position
                continue;
            }
        }
        i += 1;
    }

    // Handle the last element's children if it's a container
    if let Some(last) = inlines.last_mut() {
        match last {
            Inline::Emphasis { content }
            | Inline::Strong { content }
            | Inline::Strikethrough { content }
            | Inline::Superscript { content }
            | Inline::Subscript { content }
            | Inline::SmallCaps { content }
            | Inline::Underline { content }
            | Inline::Span { content, .. }
            | Inline::Link { content, .. } => {
                postprocess_raw_inlines(content);
            }
            _ => {}
        }
    }
}

struct RawAttrParse {
    format: String,
    consumed: usize,
}

/// Try to parse `{=format}` at the start of a string.
/// Returns the format and how many bytes were consumed.
fn parse_raw_attribute_inline(s: &str) -> Option<RawAttrParse> {
    if !s.starts_with("{=") {
        return None;
    }
    let end = s.find('}')?;
    let fmt = s[2..end].trim().to_string();
    if fmt.is_empty() {
        return None;
    }
    Some(RawAttrParse {
        format: fmt,
        consumed: end + 1,
    })
}
```

Then modify `collect_inlines` to call this before bracketed spans:

```rust
fn collect_inlines<'a>(&self, node: &'a AstNode<'a>) -> Vec<Inline> {
    let mut inlines = Vec::new();
    for child in node.children() {
        self.node_to_inlines(child, &mut inlines);
    }
    postprocess_raw_inlines(&mut inlines);
    postprocess_bracketed_spans(&mut inlines);
    inlines
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-reader-markdown raw_attribute`
Expected: 6 PASS (all raw_attribute tests)

- [ ] **Step 5: Run full test suite**

Run: `cargo test -p docmux-reader-markdown`
Expected: All existing tests still pass.

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-reader-markdown/src/lib.rs
git commit -m "feat(markdown): parse raw_attribute inline syntax into RawInline"
```

---

### Task 3: Table captions — markdown reader

**Files:**
- Modify: `crates/docmux-reader-markdown/src/lib.rs:509-528` (read method)

- [ ] **Step 1: Write the failing test**

Add to `mod tests`:

```rust
#[test]
fn table_caption_above() {
    let reader = MarkdownReader::new();
    let input = ": Simple caption\n\n| A | B |\n| --- | --- |\n| 1 | 2 |";
    let doc = reader.read(input).unwrap();
    assert_eq!(doc.content.len(), 1, "Caption paragraph should be absorbed. Got: {:#?}", doc.content);
    match &doc.content[0] {
        Block::Table(table) => {
            let cap = table.caption.as_ref().expect("Table should have caption");
            let text = match &cap[0] {
                Inline::Text { value } => value.as_str(),
                other => panic!("Expected Text, got {:?}", other),
            };
            assert_eq!(text, "Simple caption");
        }
        other => panic!("Expected Table, got {:?}", other),
    }
}

#[test]
fn table_caption_with_prefix() {
    let reader = MarkdownReader::new();
    let input = "Table: Results summary\n\n| X | Y |\n| --- | --- |\n| a | b |";
    let doc = reader.read(input).unwrap();
    assert_eq!(doc.content.len(), 1);
    match &doc.content[0] {
        Block::Table(table) => {
            let cap = table.caption.as_ref().expect("Table should have caption");
            let text = match &cap[0] {
                Inline::Text { value } => value.as_str(),
                other => panic!("Expected Text, got {:?}", other),
            };
            assert_eq!(text, "Results summary");
        }
        other => panic!("Expected Table, got {:?}", other),
    }
}

#[test]
fn table_caption_below() {
    let reader = MarkdownReader::new();
    let input = "| A | B |\n| --- | --- |\n| 1 | 2 |\n\n: Below caption";
    let doc = reader.read(input).unwrap();
    assert_eq!(doc.content.len(), 1);
    match &doc.content[0] {
        Block::Table(table) => {
            let cap = table.caption.as_ref().expect("Table should have caption");
            let text = match &cap[0] {
                Inline::Text { value } => value.as_str(),
                other => panic!("Expected Text, got {:?}", other),
            };
            assert_eq!(text, "Below caption");
        }
        other => panic!("Expected Table, got {:?}", other),
    }
}

#[test]
fn table_caption_above_wins_over_below() {
    let reader = MarkdownReader::new();
    let input = ": Above\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\n: Below";
    let doc = reader.read(input).unwrap();
    // Above caption absorbed, below caption stays as paragraph
    let table_block = doc
        .content
        .iter()
        .find(|b| matches!(b, Block::Table(_)))
        .expect("Should have a table");
    match table_block {
        Block::Table(table) => {
            let cap = table.caption.as_ref().expect("Table should have caption");
            let text = match &cap[0] {
                Inline::Text { value } => value.as_str(),
                other => panic!("Expected Text, got {:?}", other),
            };
            assert_eq!(text, "Above");
        }
        _ => unreachable!(),
    }
}

#[test]
fn non_adjacent_colon_paragraph_not_caption() {
    let reader = MarkdownReader::new();
    let input = ": Not a caption\n\nSome paragraph in between.\n\n| A | B |\n| --- | --- |\n| 1 | 2 |";
    let doc = reader.read(input).unwrap();
    // The `: Not a caption` paragraph is NOT adjacent to the table
    // (there's an intervening paragraph), so the table has no caption
    match doc.content.iter().find(|b| matches!(b, Block::Table(_))) {
        Some(Block::Table(table)) => {
            assert!(table.caption.is_none(), "Non-adjacent paragraph should not become caption");
        }
        _ => panic!("Expected a Table block"),
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-markdown table_caption`
Expected: 5 FAIL

- [ ] **Step 3: Implement table caption extraction**

Add a new function in `crates/docmux-reader-markdown/src/lib.rs`, near the `auto_id_headings` section:

```rust
// ─── Table caption extraction ──────────────────────────────────────────────

/// Extract table captions from adjacent paragraphs.
///
/// Pandoc convention: a `Paragraph` starting with `Table:` or `: ` immediately
/// before or after a `Table` is treated as a table caption. Caption above the
/// table takes priority.
fn extract_table_captions(blocks: &mut Vec<Block>) {
    // First pass: captions ABOVE tables (Paragraph then Table).
    // Walk backwards so removals don't shift indices.
    let mut i = blocks.len().wrapping_sub(1);
    while i > 0 && i < blocks.len() {
        if matches!(&blocks[i], Block::Table(_)) {
            if let Some(caption) = try_extract_caption(&blocks[i - 1]) {
                if let Block::Table(ref mut table) = blocks[i] {
                    table.caption = Some(caption);
                }
                blocks.remove(i - 1);
                // Adjust index since we removed the element before i
                i = i.saturating_sub(2);
                continue;
            }
        }
        i = i.wrapping_sub(1);
    }

    // Second pass: captions BELOW tables (Table then Paragraph).
    // Only if the table doesn't already have a caption from above.
    let mut i = 0;
    while i + 1 < blocks.len() {
        if let Block::Table(ref table) = blocks[i] {
            if table.caption.is_none() {
                if let Some(caption) = try_extract_caption(&blocks[i + 1]) {
                    if let Block::Table(ref mut table) = blocks[i] {
                        table.caption = Some(caption);
                    }
                    blocks.remove(i + 1);
                    continue;
                }
            }
        }
        i += 1;
    }
}

/// Check if a block is a caption paragraph (starts with `Table:` or `: `).
/// Returns the caption inlines with the prefix stripped.
fn try_extract_caption(block: &Block) -> Option<Vec<Inline>> {
    let Block::Paragraph { content } = block else {
        return None;
    };
    if content.is_empty() {
        return None;
    }
    let Inline::Text { value } = &content[0] else {
        return None;
    };

    let stripped = if let Some(rest) = value.strip_prefix("Table:") {
        rest.trim_start().to_string()
    } else if let Some(rest) = value.strip_prefix(": ") {
        rest.to_string()
    } else if value == ":" && content.len() > 1 {
        // Bare `:` followed by more inlines (e.g., `: **bold**`)
        String::new()
    } else {
        return None;
    };

    let mut caption = content.clone();
    if stripped.is_empty() && content.len() > 1 {
        // Remove the `: ` text node, keep the rest
        caption.remove(0);
        // Trim leading whitespace from the next text node if present
        if let Some(Inline::Text { value }) = caption.first_mut() {
            *value = value.trim_start().to_string();
        }
    } else if stripped.is_empty() {
        return None; // Just `Table:` or `:` with nothing after
    } else {
        caption[0] = Inline::Text { value: stripped };
    }

    Some(caption)
}
```

Then call it from `read()`, after `auto_id_headings`:

```rust
fn read(&self, input: &str) -> Result<Document> {
    let arena = Arena::new();
    let opts = Self::comrak_options();
    let root = parse_document(&arena, input, &opts);

    let metadata = self.extract_frontmatter(root);
    let mut content = self.convert_node(root);

    auto_id_headings(&mut content);
    extract_table_captions(&mut content);

    Ok(Document {
        metadata,
        content,
        bibliography: None,
        warnings: vec![],
        resources: HashMap::new(),
    })
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-reader-markdown table_caption`
Expected: 5 PASS

- [ ] **Step 5: Run full reader test suite**

Run: `cargo test -p docmux-reader-markdown`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-reader-markdown/src/lib.rs
git commit -m "feat(markdown): extract table captions from adjacent paragraphs"
```

---

### Task 4: Table captions — markdown writer

**Files:**
- Modify: `crates/docmux-writer-markdown/src/lib.rs:352` (write_table)

- [ ] **Step 1: Write the failing test**

Add to `mod tests` in `crates/docmux-writer-markdown/src/lib.rs`:

```rust
#[test]
fn table_with_caption() {
    let doc = Document {
        content: vec![Block::Table(Table {
            caption: Some(vec![Inline::text("Experiment results")]),
            label: None,
            columns: vec![
                ColumnSpec {
                    alignment: Alignment::Left,
                    width: None,
                },
                ColumnSpec {
                    alignment: Alignment::Right,
                    width: None,
                },
            ],
            header: Some(vec![
                TableCell {
                    content: vec![Block::text("Name")],
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: vec![Block::text("Score")],
                    colspan: 1,
                    rowspan: 1,
                },
            ]),
            rows: vec![vec![
                TableCell {
                    content: vec![Block::text("Alice")],
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: vec![Block::text("95")],
                    colspan: 1,
                    rowspan: 1,
                },
            ]],
            foot: None,
            attrs: None,
        })],
        ..Default::default()
    };
    let md = write_md(&doc);
    assert!(
        md.starts_with("Table: Experiment results\n"),
        "Table caption should appear before table. Got:\n{md}"
    );
    assert!(md.contains("| Name"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p docmux-writer-markdown table_with_caption`
Expected: FAIL (no caption emitted)

- [ ] **Step 3: Add caption rendering to write_table**

In `crates/docmux-writer-markdown/src/lib.rs`, at the beginning of `write_table` (line 352), add caption rendering before the column width calculation:

```rust
fn write_table(&self, table: &Table, out: &mut String) {
    // Emit caption above the table (pandoc convention)
    if let Some(cap) = &table.caption {
        out.push_str("Table: ");
        self.write_inlines(cap, out);
        out.push('\n');
        out.push('\n');
    }

    // Collect all rows for width calculation
    let ncols = table.columns.len().max(
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-markdown`
Expected: All tests pass (including new `table_with_caption`).

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-markdown/src/lib.rs
git commit -m "feat(markdown-writer): emit table captions as Table: prefix"
```

---

### Task 5: `--id-prefix` — markdown reader

**Files:**
- Modify: `crates/docmux-reader-markdown/src/lib.rs:18-26` (struct), `509-528` (read), `535-566` (auto_id)

- [ ] **Step 1: Write the failing test**

Add to `mod tests`:

```rust
#[test]
fn id_prefix_auto_generated() {
    let reader = MarkdownReader::new().with_id_prefix("ch1-".to_string());
    let doc = reader.read("# Hello World").unwrap();
    match &doc.content[0] {
        Block::Heading { id, .. } => {
            assert_eq!(id.as_deref(), Some("ch1-hello-world"));
        }
        other => panic!("Expected Heading, got {:?}", other),
    }
}

#[test]
fn id_prefix_explicit_not_prefixed() {
    let reader = MarkdownReader::new().with_id_prefix("ch1-".to_string());
    let doc = reader.read("# Hello {#custom-id}").unwrap();
    match &doc.content[0] {
        Block::Heading { id, .. } => {
            assert_eq!(id.as_deref(), Some("custom-id"));
        }
        other => panic!("Expected Heading, got {:?}", other),
    }
}

#[test]
fn id_prefix_dedup_works() {
    let reader = MarkdownReader::new().with_id_prefix("sec-".to_string());
    let doc = reader.read("# Hello\n\n# Hello").unwrap();
    let ids: Vec<_> = doc
        .content
        .iter()
        .filter_map(|b| match b {
            Block::Heading { id, .. } => id.clone(),
            _ => None,
        })
        .collect();
    assert_eq!(ids, vec!["sec-hello", "sec-hello-1"]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-reader-markdown id_prefix`
Expected: FAIL — `with_id_prefix` method doesn't exist

- [ ] **Step 3: Add id_prefix field and builder method**

In `crates/docmux-reader-markdown/src/lib.rs`, modify the struct and constructor:

```rust
/// A Markdown reader backed by comrak.
#[derive(Debug, Default)]
pub struct MarkdownReader {
    id_prefix: Option<String>,
}

impl MarkdownReader {
    pub fn new() -> Self {
        Self { id_prefix: None }
    }

    /// Set a prefix for auto-generated heading IDs.
    /// Explicit IDs (from `{#id}` attributes) are not prefixed.
    pub fn with_id_prefix(mut self, prefix: String) -> Self {
        self.id_prefix = Some(prefix);
        self
    }
```

- [ ] **Step 4: Thread prefix through auto_id_headings**

Modify `read()` to pass the prefix:

```rust
fn read(&self, input: &str) -> Result<Document> {
    let arena = Arena::new();
    let opts = Self::comrak_options();
    let root = parse_document(&arena, input, &opts);

    let metadata = self.extract_frontmatter(root);
    let mut content = self.convert_node(root);

    auto_id_headings(&mut content, self.id_prefix.as_deref());
    extract_table_captions(&mut content);

    Ok(Document {
        metadata,
        content,
        bibliography: None,
        warnings: vec![],
        resources: HashMap::new(),
    })
}
```

Update `auto_id_headings` and `auto_id_walk` signatures:

```rust
fn auto_id_headings(blocks: &mut [Block], id_prefix: Option<&str>) {
    let mut seen = HashSet::new();
    auto_id_walk(blocks, &mut seen, id_prefix);
}

fn auto_id_walk(blocks: &mut [Block], seen: &mut HashSet<String>, id_prefix: Option<&str>) {
    for block in blocks.iter_mut() {
        match block {
            Block::Heading { id, content, .. } => {
                if let Some(ref existing) = id {
                    seen.insert(existing.clone());
                } else {
                    let slug = slugify_inlines(content);
                    if !slug.is_empty() {
                        let prefixed = match id_prefix {
                            Some(p) => format!("{p}{slug}"),
                            None => slug,
                        };
                        *id = Some(dedup_slug(prefixed, seen));
                    }
                }
            }
            Block::BlockQuote { content } => auto_id_walk(content, seen, id_prefix),
            Block::List { items, .. } => {
                for item in items {
                    auto_id_walk(&mut item.content, seen, id_prefix);
                }
            }
            Block::Admonition { content, .. } => auto_id_walk(content, seen, id_prefix),
            Block::Div { content, .. } => auto_id_walk(content, seen, id_prefix),
            Block::FootnoteDef { content, .. } => auto_id_walk(content, seen, id_prefix),
            _ => {}
        }
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p docmux-reader-markdown id_prefix`
Expected: 3 PASS

- [ ] **Step 6: Run full test suite**

Run: `cargo test -p docmux-reader-markdown`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-reader-markdown/src/lib.rs
git commit -m "feat(markdown): add --id-prefix support for auto-generated heading IDs"
```

---

### Task 6: `--id-prefix` — CLI integration

**Files:**
- Modify: `crates/docmux-cli/src/main.rs:26-129` (Cli struct), `131-146` (build_registry)

- [ ] **Step 1: Add CLI flag and wire it up**

In `crates/docmux-cli/src/main.rs`, add the flag to the `Cli` struct (after `highlight_style`):

```rust
    /// Prefix for auto-generated identifiers (e.g. --id-prefix=ch1-)
    #[arg(long, value_name = "PREFIX")]
    id_prefix: Option<String>,
```

Modify `build_registry` to accept the prefix:

```rust
fn build_registry(id_prefix: Option<&str>) -> Registry {
    let mut reg = Registry::new();
    let md_reader = match id_prefix {
        Some(p) => MarkdownReader::new().with_id_prefix(p.to_string()),
        None => MarkdownReader::new(),
    };
    reg.add_reader(Box::new(md_reader));
    reg.add_reader(Box::new(LatexReader::new()));
    reg.add_reader(Box::new(MystReader::new()));
    reg.add_reader(Box::new(TypstReader::new()));
    reg.add_reader(Box::new(HtmlReader::new()));
    reg.add_binary_reader(Box::new(DocxReader::new()));
    reg.add_writer(Box::new(HtmlWriter::new()));
    reg.add_writer(Box::new(LatexWriter::new()));
    reg.add_writer(Box::new(MarkdownWriter::new()));
    reg.add_writer(Box::new(PlaintextWriter::new()));
    reg.add_writer(Box::new(TypstWriter::new()));
    reg.add_writer(Box::new(DocxWriter::new()));
    reg
}
```

Update the call in `main()`:

```rust
let registry = build_registry(cli.id_prefix.as_deref());
```

- [ ] **Step 2: Build and verify**

Run: `cargo build -p docmux-cli`
Expected: Compiles without errors.

- [ ] **Step 3: Manual smoke test**

Run: `echo "# Hello" | cargo run -p docmux-cli -- - -t json --id-prefix=ch1-`
Expected: JSON output shows heading with `"id": "ch1-hello"`.

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-cli/src/main.rs
git commit -m "feat(cli): add --id-prefix flag for heading ID prefixing"
```

---

### Task 7: `--section-divs` — scaffold crate

**Files:**
- Create: `crates/docmux-transform-section-divs/Cargo.toml`
- Create: `crates/docmux-transform-section-divs/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create crate directory**

Run: `mkdir -p crates/docmux-transform-section-divs/src`

- [ ] **Step 2: Create Cargo.toml**

Create `crates/docmux-transform-section-divs/Cargo.toml`:

```toml
[package]
name = "docmux-transform-section-divs"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Section-divs transform for docmux — wraps heading sections in Div containers"
rust-version.workspace = true

[dependencies]
docmux-ast = { workspace = true }
docmux-core = { workspace = true }
```

- [ ] **Step 3: Create initial lib.rs with failing test**

Create `crates/docmux-transform-section-divs/src/lib.rs`:

```rust
//! # docmux-transform-section-divs
//!
//! Wraps heading-delimited sections in `Block::Div` containers.
//!
//! Each heading of level N and all subsequent blocks until the next heading of
//! level ≤ N (or end of document) are wrapped in a `Div` with class `section`
//! and `levelN`. The heading's ID is moved to the Div to avoid duplication.
//! Nesting is recursive.

use docmux_ast::*;
use docmux_core::{Result, Transform, TransformContext};

#[derive(Debug, Default)]
pub struct SectionDivsTransform;

impl SectionDivsTransform {
    pub fn new() -> Self {
        Self
    }
}

impl Transform for SectionDivsTransform {
    fn name(&self) -> &str {
        "section-divs"
    }

    fn transform(&self, doc: &mut Document, _ctx: &TransformContext) -> Result<()> {
        doc.content = wrap_sections(std::mem::take(&mut doc.content));
        Ok(())
    }
}

fn wrap_sections(_blocks: Vec<Block>) -> Vec<Block> {
    // TODO: implement
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(blocks: Vec<Block>) -> Vec<Block> {
        let mut doc = Document {
            content: blocks,
            ..Default::default()
        };
        SectionDivsTransform::new()
            .transform(&mut doc, &TransformContext::default())
            .unwrap();
        doc.content
    }

    #[test]
    fn single_section() {
        let blocks = vec![
            Block::Heading {
                level: 1,
                id: Some("intro".into()),
                content: vec![Inline::text("Introduction")],
                attrs: None,
            },
            Block::text("Some text."),
        ];
        let result = apply(blocks);
        assert_eq!(result.len(), 1, "Should be wrapped in one Div. Got: {result:#?}");
        match &result[0] {
            Block::Div { attrs, content } => {
                assert_eq!(attrs.id.as_deref(), Some("intro"));
                assert!(attrs.classes.contains(&"section".to_string()));
                assert!(attrs.classes.contains(&"level1".to_string()));
                assert_eq!(content.len(), 2); // heading + paragraph
            }
            other => panic!("Expected Div, got {:?}", other),
        }
    }
}
```

- [ ] **Step 4: Add to workspace**

In `Cargo.toml` (root), add to `members`:

```toml
    "crates/docmux-transform-section-divs",
```

And to `[workspace.dependencies]`:

```toml
docmux-transform-section-divs = { path = "crates/docmux-transform-section-divs" }
```

- [ ] **Step 5: Run test to verify it fails**

Run: `cargo test -p docmux-transform-section-divs`
Expected: FAIL — `wrap_sections` returns empty Vec.

- [ ] **Step 6: Commit scaffold**

```bash
git add crates/docmux-transform-section-divs/ Cargo.toml
git commit -m "feat(section-divs): scaffold transform crate with failing test"
```

---

### Task 8: `--section-divs` — implement transform

**Files:**
- Modify: `crates/docmux-transform-section-divs/src/lib.rs`

- [ ] **Step 1: Write additional failing tests**

Add to `mod tests`:

```rust
#[test]
fn two_sibling_sections() {
    let blocks = vec![
        Block::Heading {
            level: 1,
            id: Some("a".into()),
            content: vec![Inline::text("A")],
            attrs: None,
        },
        Block::text("Content A."),
        Block::Heading {
            level: 1,
            id: Some("b".into()),
            content: vec![Inline::text("B")],
            attrs: None,
        },
        Block::text("Content B."),
    ];
    let result = apply(blocks);
    assert_eq!(result.len(), 2);
    assert!(matches!(&result[0], Block::Div { .. }));
    assert!(matches!(&result[1], Block::Div { .. }));
}

#[test]
fn nested_sections() {
    let blocks = vec![
        Block::Heading {
            level: 1,
            id: Some("ch1".into()),
            content: vec![Inline::text("Chapter 1")],
            attrs: None,
        },
        Block::text("Intro."),
        Block::Heading {
            level: 2,
            id: Some("sec1".into()),
            content: vec![Inline::text("Section 1.1")],
            attrs: None,
        },
        Block::text("Section content."),
    ];
    let result = apply(blocks);
    assert_eq!(result.len(), 1, "Outer h1 wraps everything. Got: {result:#?}");
    match &result[0] {
        Block::Div { content, .. } => {
            // heading + para + nested div
            assert_eq!(content.len(), 3, "Expected heading + para + nested div. Got: {content:#?}");
            assert!(matches!(&content[2], Block::Div { .. }));
        }
        other => panic!("Expected Div, got {:?}", other),
    }
}

#[test]
fn content_before_first_heading() {
    let blocks = vec![
        Block::text("Preamble."),
        Block::Heading {
            level: 1,
            id: Some("first".into()),
            content: vec![Inline::text("First")],
            attrs: None,
        },
        Block::text("Content."),
    ];
    let result = apply(blocks);
    assert_eq!(result.len(), 2, "Preamble stays unwrapped. Got: {result:#?}");
    assert!(matches!(&result[0], Block::Paragraph { .. }));
    assert!(matches!(&result[1], Block::Div { .. }));
}

#[test]
fn empty_section() {
    let blocks = vec![
        Block::Heading {
            level: 1,
            id: Some("a".into()),
            content: vec![Inline::text("A")],
            attrs: None,
        },
        Block::Heading {
            level: 1,
            id: Some("b".into()),
            content: vec![Inline::text("B")],
            attrs: None,
        },
        Block::text("Content B."),
    ];
    let result = apply(blocks);
    assert_eq!(result.len(), 2);
    // First section has only the heading
    match &result[0] {
        Block::Div { content, .. } => assert_eq!(content.len(), 1),
        other => panic!("Expected Div, got {:?}", other),
    }
}

#[test]
fn heading_id_cleared() {
    let blocks = vec![
        Block::Heading {
            level: 1,
            id: Some("intro".into()),
            content: vec![Inline::text("Intro")],
            attrs: None,
        },
        Block::text("Text."),
    ];
    let result = apply(blocks);
    match &result[0] {
        Block::Div { content, .. } => {
            // The heading inside the div should have id cleared
            match &content[0] {
                Block::Heading { id, .. } => {
                    assert!(id.is_none(), "Heading ID should be moved to Div");
                }
                other => panic!("Expected Heading, got {:?}", other),
            }
        }
        other => panic!("Expected Div, got {:?}", other),
    }
}

#[test]
fn no_headings_passthrough() {
    let blocks = vec![Block::text("Just text."), Block::ThematicBreak];
    let result = apply(blocks);
    assert_eq!(result.len(), 2, "No headings → no wrapping");
}
```

- [ ] **Step 2: Implement `wrap_sections`**

Replace the stub `wrap_sections` in `crates/docmux-transform-section-divs/src/lib.rs`:

```rust
/// Group blocks into sections delimited by headings.
///
/// Algorithm:
/// 1. Scan blocks linearly. Blocks before the first heading pass through.
/// 2. When a heading of level N is found, start a new section collecting all
///    blocks until the next heading of level ≤ N or end.
/// 3. Recursively nest: within a level-N section, level-(N+1) headings create
///    nested Divs.
/// 4. The heading's ID is moved to the wrapping Div.
fn wrap_sections(blocks: Vec<Block>) -> Vec<Block> {
    sectionize(blocks, 0)
}

/// Recursive section wrapping at a given minimum heading level.
/// `min_level = 0` means "process all headings".
fn sectionize(blocks: Vec<Block>, min_level: u8) -> Vec<Block> {
    let mut result: Vec<Block> = Vec::new();
    let mut current_section: Option<(Attributes, Vec<Block>)> = None;
    let mut current_level: u8 = 0;

    for mut block in blocks {
        let heading_level = match &block {
            Block::Heading { level, .. } if min_level == 0 || *level >= min_level => Some(*level),
            _ => None,
        };

        if let Some(level) = heading_level {
            if level <= current_level || (min_level > 0 && level == min_level) {
                // Close the current section
                if let Some((attrs, content)) = current_section.take() {
                    result.push(make_section_div(attrs, content, current_level));
                }
            }

            if current_section.is_some() && level > current_level {
                // This is a deeper heading — it goes into the current section
                current_section.as_mut().unwrap().1.push(block);
                continue;
            }

            // Start a new section
            let id = match &mut block {
                Block::Heading { id, .. } => id.take(),
                _ => None,
            };
            let attrs = Attributes {
                id,
                classes: vec!["section".into(), format!("level{level}")],
                key_values: std::collections::HashMap::new(),
            };
            current_level = level;
            current_section = Some((attrs, vec![block]));
        } else if let Some(ref mut section) = current_section {
            section.1.push(block);
        } else {
            result.push(block);
        }
    }

    // Close final section
    if let Some((attrs, content)) = current_section {
        result.push(make_section_div(attrs, content, current_level));
    }

    result
}

/// Build a section Div, recursively nesting any deeper headings.
fn make_section_div(attrs: Attributes, mut content: Vec<Block>, level: u8) -> Block {
    // Recursively wrap sub-sections (headings deeper than this level)
    content = sectionize(content, level + 1);
    Block::Div { attrs, content }
}
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test -p docmux-transform-section-divs`
Expected: 7 PASS

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-transform-section-divs/src/lib.rs
git commit -m "feat(section-divs): implement recursive section wrapping transform"
```

---

### Task 9: `--section-divs` — CLI integration

**Files:**
- Modify: `crates/docmux-cli/Cargo.toml`
- Modify: `crates/docmux-cli/src/main.rs`

- [ ] **Step 1: Add dependency**

In `crates/docmux-cli/Cargo.toml`, add to `[dependencies]`:

```toml
docmux-transform-section-divs = { workspace = true }
```

- [ ] **Step 2: Add CLI flag and wire transform**

In `crates/docmux-cli/src/main.rs`, add the import:

```rust
use docmux_transform_section_divs::SectionDivsTransform;
```

Add the flag to the `Cli` struct (after `id_prefix`):

```rust
    /// Wrap sections (heading + content) in <div> containers
    #[arg(long)]
    section_divs: bool,
```

In `main()`, add the transform application **after** `--number-sections` and **before** `--toc` (around line 290):

```rust
    // Apply --section-divs (after --number-sections, before --toc)
    if cli.section_divs {
        let ctx = TransformContext::default();
        if let Err(e) = SectionDivsTransform::new().transform(&mut doc, &ctx) {
            eprintln!("docmux: section-divs error: {e}");
            std::process::exit(1);
        }
    }
```

- [ ] **Step 3: Build and verify**

Run: `cargo build -p docmux-cli`
Expected: Compiles.

- [ ] **Step 4: Manual smoke test**

Run: `echo "# Hello\n\nWorld." | cargo run -p docmux-cli -- - -t json --section-divs`
Expected: JSON shows the paragraph wrapped inside a Div with classes `["section", "level1"]`.

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-cli/Cargo.toml crates/docmux-cli/src/main.rs
git commit -m "feat(cli): add --section-divs flag"
```

---

### Task 10: Full integration test + clippy

**Files:**
- All modified crates

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All tests pass (including the new ones).

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: No warnings.

- [ ] **Step 3: Run fmt check**

Run: `cargo fmt --all -- --check`
Expected: No formatting issues.

- [ ] **Step 4: Run WASM build**

Run: `cargo build --target wasm32-unknown-unknown -p docmux-wasm`
Expected: Compiles (new crate doesn't affect WASM).

- [ ] **Step 5: Commit any fixes if needed**

If clippy/fmt required fixes:
```bash
git add -A
git commit -m "fix: address clippy warnings and formatting"
```

---

### Task 11: Update ROADMAP.md

**Files:**
- Modify: `ROADMAP.md`

- [ ] **Step 1: Mark completed items**

In `ROADMAP.md`, update the following lines:

- `- [ ] \`raw_attribute\` syntax` → `- [x] \`raw_attribute\` syntax (inline + block, 6 tests)`
- `- [ ] Table captions` → `- [x] Table captions (pandoc convention, 5 tests)`
- `- [ ] \`--section-divs\`, \`--id-prefix=PREFIX\`` → `- [x] \`--section-divs\`, \`--id-prefix=PREFIX\``

Add the new crate to the crates table:

- `docmux-transform-section-divs` under Transforms

- [ ] **Step 2: Update CLAUDE.md**

In `CLAUDE.md`, update the crate count and test count under "Current state".

- [ ] **Step 3: Commit**

```bash
git add ROADMAP.md CLAUDE.md
git commit -m "docs: update roadmap and CLAUDE.md for markdown/CLI extensions"
```
