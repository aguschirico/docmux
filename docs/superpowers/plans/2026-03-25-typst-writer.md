# docmux-writer-typst Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a Typst writer that converts the docmux AST into idiomatic Typst markup, completing the Typst roundtrip (reader + writer).

**Architecture:** Single `TypstWriter` struct implementing the `Writer` trait, following the same recursive-descent pattern as the existing HTML and LaTeX writers. Uses native Typst markup (`=`, `*`, `_`, `-`, `+`) where shorthand exists, and function calls (`#strike[...]`, `#link("url")[...]`) for everything else. Footnote expansion via a pre-pass index.

**Tech Stack:** Rust, docmux-ast, docmux-core. No external dependencies.

**Spec:** `docs/superpowers/specs/2026-03-25-typst-writer-design.md`

---

### Task 1: Scaffold — escape functions + Writer trait impl + paragraph test

**Files:**
- Modify: `crates/docmux-writer-typst/src/lib.rs` (replace placeholder)
- Modify: `crates/docmux-cli/Cargo.toml` (add docmux-writer-typst dep)
- Modify: `crates/docmux-cli/src/main.rs` (register writer)

- [ ] **Step 1: Write the failing test for paragraph output**

In `crates/docmux-writer-typst/src/lib.rs`, replace the entire file with:

```rust
//! # docmux-writer-typst
//!
//! Typst writer for docmux. Converts the docmux AST into idiomatic Typst markup.

use std::collections::HashMap;

use docmux_ast::*;
use docmux_core::{Result, WriteOptions, Writer};

/// A Typst writer.
#[derive(Debug, Default)]
pub struct TypstWriter;

impl TypstWriter {
    pub fn new() -> Self {
        Self
    }
}

impl Writer for TypstWriter {
    fn format(&self) -> &str {
        "typst"
    }

    fn default_extension(&self) -> &str {
        "typ"
    }

    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
        let _ = (doc, opts);
        todo!()
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Escape Typst special characters in text content.
///
/// Always-special: \ * _ ` $ # @ < >
fn escape_typst(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' | '*' | '_' | '`' | '$' | '#' | '@' | '<' | '>' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Escape double quotes inside Typst string literals (URLs, etc.).
fn escape_typst_url(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn write_typst(doc: &Document) -> String {
        let writer = TypstWriter::new();
        writer.write(doc, &WriteOptions::default()).unwrap()
    }

    #[test]
    fn paragraph() {
        let doc = Document {
            content: vec![Block::text("Hello world!")],
            ..Default::default()
        };
        let typ = write_typst(&doc);
        assert_eq!(typ.trim(), "Hello world!");
    }

    #[test]
    fn escaping() {
        assert_eq!(escape_typst("a * b"), "a \\* b");
        assert_eq!(escape_typst("$10 & #tag"), "\\$10 & \\#tag");
        assert_eq!(escape_typst("a@b <c>"), "a\\@b \\<c\\>");
        assert_eq!(escape_typst("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn url_escaping() {
        assert_eq!(escape_typst_url(r#"he said "hi""#), r#"he said \"hi\""#);
    }

    #[test]
    fn writer_trait_metadata() {
        let writer = TypstWriter::new();
        assert_eq!(writer.format(), "typst");
        assert_eq!(writer.default_extension(), "typ");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p docmux-writer-typst -- paragraph --no-capture`
Expected: FAIL — `todo!()` panics

- [ ] **Step 3: Implement write() with paragraph support**

Replace the `write` method and add private helpers. In `TypstWriter`, add:

```rust
impl TypstWriter {
    pub fn new() -> Self {
        Self
    }

    fn write_blocks(&self, blocks: &[Block], _opts: &WriteOptions, out: &mut String) {
        let mut first = true;
        for block in blocks {
            if !first {
                // Blank line between blocks
                if !out.ends_with("\n\n") {
                    if out.ends_with('\n') {
                        out.push('\n');
                    } else {
                        out.push_str("\n\n");
                    }
                }
            }
            first = false;
            self.write_block(block, _opts, out);
        }
    }

    fn write_block(&self, block: &Block, opts: &WriteOptions, out: &mut String) {
        match block {
            Block::Paragraph { content } => {
                self.write_inlines(content, opts, out);
                out.push('\n');
            }
            _ => {} // other blocks added in later tasks
        }
    }

    fn write_inlines(&self, inlines: &[Inline], opts: &WriteOptions, out: &mut String) {
        for inline in inlines {
            self.write_inline(inline, opts, out);
        }
    }

    fn write_inline(&self, inline: &Inline, opts: &WriteOptions, out: &mut String) {
        match inline {
            Inline::Text { value } => {
                out.push_str(&escape_typst(value));
            }
            Inline::SoftBreak => out.push('\n'),
            Inline::HardBreak => out.push_str("\\\n"),
            _ => {} // other inlines added in later tasks
        }
    }
}
```

And update the `Writer` impl:

```rust
fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
    let mut body = String::with_capacity(4096);
    self.write_blocks(&doc.content, opts, &mut body);

    if opts.standalone {
        Ok(self.wrap_standalone(&body, doc))
    } else {
        Ok(body)
    }
}
```

Add a stub `wrap_standalone`:

```rust
fn wrap_standalone(&self, body: &str, _doc: &Document) -> String {
    body.to_string() // implemented in Task 5
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-typst`
Expected: 4 tests PASS (paragraph, escaping, url_escaping, writer_trait_metadata)

- [ ] **Step 5: Register writer in CLI**

In `crates/docmux-cli/Cargo.toml`, add:
```toml
docmux-writer-typst = { workspace = true }
```

In `crates/docmux-cli/src/main.rs`, add import:
```rust
use docmux_writer_typst::TypstWriter;
```

And in `build_registry()`, add after `LatexWriter`:
```rust
reg.add_writer(Box::new(TypstWriter::new()));
```

- [ ] **Step 6: Run full workspace check**

Run: `cargo check --workspace && cargo test -p docmux-writer-typst`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-writer-typst/src/lib.rs crates/docmux-cli/Cargo.toml crates/docmux-cli/src/main.rs
git commit -m "feat(typst-writer): scaffold with escape helpers, paragraph support, CLI registration"
```

---

### Task 2: Headings, code blocks, math blocks

**Files:**
- Modify: `crates/docmux-writer-typst/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add to the `tests` module in `crates/docmux-writer-typst/src/lib.rs`:

```rust
#[test]
fn heading_with_label() {
    let doc = Document {
        content: vec![Block::Heading {
            level: 2,
            id: Some("intro".into()),
            content: vec![Inline::text("Introduction")],
            attrs: None,
        }],
        ..Default::default()
    };
    let typ = write_typst(&doc);
    assert_eq!(typ.trim(), "== Introduction <intro>");
}

#[test]
fn code_block() {
    let doc = Document {
        content: vec![Block::CodeBlock {
            language: Some("python".into()),
            content: "print('hello')".into(),
            caption: None,
            label: None,
            attrs: None,
        }],
        ..Default::default()
    };
    let typ = write_typst(&doc);
    assert!(typ.contains("```python\nprint('hello')\n```"));
}

#[test]
fn math_block_with_label() {
    let doc = Document {
        content: vec![Block::MathBlock {
            content: "E = m c^2".into(),
            label: Some("eq:einstein".into()),
        }],
        ..Default::default()
    };
    let typ = write_typst(&doc);
    assert!(typ.contains("$\nE = m c^2\n$"));
    assert!(typ.contains("<eq:einstein>"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-typst -- heading_with_label code_block math_block_with_label`
Expected: FAIL — empty output for these block types

- [ ] **Step 3: Implement heading, code block, math block in write_block()**

Add match arms to `write_block()`:

```rust
Block::Heading { level, id, content, .. } => {
    for _ in 0..*level {
        out.push('=');
    }
    out.push(' ');
    self.write_inlines(content, opts, out);
    if let Some(id) = id {
        out.push_str(&format!(" <{}>", id));
    }
    out.push('\n');
}
Block::CodeBlock { language, content, caption, label, .. } => {
    if caption.is_some() || label.is_some() {
        out.push_str("#figure(\n");
        if let Some(cap) = caption {
            out.push_str("  caption: [");
            self.write_inlines(cap, opts, out);
            out.push_str("],\n");
        }
        out.push_str(")[\n");
    }
    out.push_str("```");
    if let Some(lang) = language {
        out.push_str(lang);
    }
    out.push('\n');
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("```\n");
    if caption.is_some() || label.is_some() {
        out.push(']');
        if let Some(label) = label {
            out.push_str(&format!(" <{}>", label));
        }
        out.push('\n');
    }
}
Block::MathBlock { content, label } => {
    out.push_str("$\n");
    out.push_str(content.trim());
    out.push_str("\n$");
    if let Some(label) = label {
        out.push_str(&format!(" <{}>", label));
    }
    out.push('\n');
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-typst`
Expected: 7 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-typst/src/lib.rs
git commit -m "feat(typst-writer): headings, code blocks, math blocks"
```

---

### Task 3: Inline formatting — emphasis, strong, strikethrough, code, math, links, images

**Files:**
- Modify: `crates/docmux-writer-typst/src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn emphasis_and_strong() {
    let doc = Document {
        content: vec![Block::Paragraph {
            content: vec![
                Inline::Emphasis {
                    content: vec![Inline::text("italic")],
                },
                Inline::text(" and "),
                Inline::Strong {
                    content: vec![Inline::text("bold")],
                },
            ],
        }],
        ..Default::default()
    };
    let typ = write_typst(&doc);
    assert!(typ.contains("_italic_"));
    assert!(typ.contains("*bold*"));
}

#[test]
fn inline_code_and_math() {
    let doc = Document {
        content: vec![Block::Paragraph {
            content: vec![
                Inline::Code { value: "x + 1".into() },
                Inline::text(" and "),
                Inline::MathInline { value: "x^2".into() },
            ],
        }],
        ..Default::default()
    };
    let typ = write_typst(&doc);
    assert!(typ.contains("`x + 1`"));
    assert!(typ.contains("$x^2$"));
}

#[test]
fn link_and_image() {
    let doc = Document {
        content: vec![Block::Paragraph {
            content: vec![
                Inline::Link {
                    url: "https://example.com".into(),
                    title: None,
                    content: vec![Inline::text("Example")],
                },
                Inline::text(" "),
                Inline::Image(Image {
                    url: "photo.png".into(),
                    alt: "A photo".into(),
                    title: None,
                }),
            ],
        }],
        ..Default::default()
    };
    let typ = write_typst(&doc);
    assert!(typ.contains(r#"#link("https://example.com")[Example]"#));
    assert!(typ.contains(r#"#image("photo.png", alt: "A photo")"#));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-typst -- emphasis_and_strong inline_code_and_math link_and_image`
Expected: FAIL

- [ ] **Step 3: Implement inline formatting in write_inline()**

Add match arms to `write_inline()`:

```rust
Inline::Emphasis { content } => {
    out.push('_');
    self.write_inlines(content, opts, out);
    out.push('_');
}
Inline::Strong { content } => {
    out.push('*');
    self.write_inlines(content, opts, out);
    out.push('*');
}
Inline::Strikethrough { content } => {
    out.push_str("#strike[");
    self.write_inlines(content, opts, out);
    out.push(']');
}
Inline::Code { value } => {
    out.push('`');
    out.push_str(value);
    out.push('`');
}
Inline::MathInline { value } => {
    out.push('$');
    out.push_str(value);
    out.push('$');
}
Inline::Link { url, content, .. } => {
    out.push_str(&format!("#link(\"{}\")", escape_typst_url(url)));
    if !content.is_empty() {
        out.push('[');
        self.write_inlines(content, opts, out);
        out.push(']');
    }
}
Inline::Image(img) => {
    out.push_str(&format!("#image(\"{}\"", escape_typst_url(&img.url)));
    if !img.alt.is_empty() {
        out.push_str(&format!(", alt: \"{}\"", escape_typst_url(&img.alt)));
    }
    out.push(')');
}
Inline::Superscript { content } => {
    out.push_str("#super[");
    self.write_inlines(content, opts, out);
    out.push(']');
}
Inline::Subscript { content } => {
    out.push_str("#sub[");
    self.write_inlines(content, opts, out);
    out.push(']');
}
Inline::SmallCaps { content } => {
    out.push_str("#smallcaps[");
    self.write_inlines(content, opts, out);
    out.push(']');
}
Inline::Underline { content } => {
    out.push_str("#underline[");
    self.write_inlines(content, opts, out);
    out.push(']');
}
Inline::Span { content, .. } => {
    self.write_inlines(content, opts, out);
}
Inline::RawInline { format, content } => {
    if format == "typst" || format == "typ" {
        out.push_str(content);
    }
}
Inline::Citation(cite) => {
    if let Some(prefix) = &cite.prefix {
        out.push_str(&escape_typst(prefix));
        out.push(' ');
    }
    for (i, key) in cite.keys.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push('@');
        out.push_str(key);
    }
    if let Some(suffix) = &cite.suffix {
        out.push(' ');
        out.push_str(&escape_typst(suffix));
    }
}
Inline::CrossRef(cr) => {
    out.push('@');
    out.push_str(&cr.target);
}
Inline::FootnoteRef { id } => {
    // Placeholder — footnote expansion implemented in Task 4
    out.push_str(&format!("#footnote[See footnote {}]", escape_typst(id)));
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-typst`
Expected: 10 tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-typst/src/lib.rs
git commit -m "feat(typst-writer): all inline formatting — emphasis, strong, code, math, links, images, citations, cross-refs"
```

---

### Task 4: Lists, blockquotes, definition lists, remaining blocks, footnote pre-pass

**Files:**
- Modify: `crates/docmux-writer-typst/src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn lists_ordered_unordered() {
    let doc = Document {
        content: vec![
            Block::List {
                ordered: false,
                start: None,
                items: vec![
                    ListItem { checked: None, content: vec![Block::text("Alpha")] },
                    ListItem { checked: None, content: vec![Block::text("Beta")] },
                ],
                tight: true,
                style: None,
                delimiter: None,
            },
            Block::List {
                ordered: true,
                start: None,
                items: vec![
                    ListItem { checked: None, content: vec![Block::text("First")] },
                    ListItem { checked: None, content: vec![Block::text("Second")] },
                ],
                tight: true,
                style: None,
                delimiter: None,
            },
        ],
        ..Default::default()
    };
    let typ = write_typst(&doc);
    assert!(typ.contains("- Alpha"));
    assert!(typ.contains("- Beta"));
    assert!(typ.contains("+ First"));
    assert!(typ.contains("+ Second"));
}

#[test]
fn definition_list() {
    let doc = Document {
        content: vec![Block::DefinitionList {
            items: vec![DefinitionItem {
                term: vec![Inline::text("Rust")],
                definitions: vec![vec![Block::text("A systems programming language.")]],
            }],
        }],
        ..Default::default()
    };
    let typ = write_typst(&doc);
    assert!(typ.contains("/ Rust: A systems programming language."));
}

#[test]
fn footnote_expansion() {
    let doc = Document {
        content: vec![
            Block::Paragraph {
                content: vec![
                    Inline::text("See note"),
                    Inline::FootnoteRef { id: "fn1".into() },
                    Inline::text("."),
                ],
            },
            Block::FootnoteDef {
                id: "fn1".into(),
                content: vec![Block::text("This is the footnote.")],
            },
        ],
        ..Default::default()
    };
    let typ = write_typst(&doc);
    assert!(typ.contains("#footnote[This is the footnote.]"));
    // FootnoteDef should not appear as a separate block
    assert!(!typ.contains("fn1"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-typst -- lists_ordered definition_list footnote_expansion`
Expected: FAIL

- [ ] **Step 3: Implement footnote pre-pass**

Add a `collect_footnotes` method to `TypstWriter` and a `footnotes` field to the write context. Since the writer is stateless, pass footnotes through a helper:

```rust
fn collect_footnotes(blocks: &[Block]) -> HashMap<String, Vec<Block>> {
    let mut map = HashMap::new();
    for block in blocks {
        if let Block::FootnoteDef { id, content } = block {
            map.insert(id.clone(), content.clone());
        }
    }
    map
}
```

Update `write()`:

```rust
fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
    let footnotes = collect_footnotes(&doc.content);
    let mut body = String::with_capacity(4096);
    self.write_blocks_with_footnotes(&doc.content, opts, &mut body, &footnotes);

    if opts.standalone {
        Ok(self.wrap_standalone(&body, doc))
    } else {
        Ok(body)
    }
}
```

Thread `&HashMap<String, Vec<Block>>` through `write_blocks`, `write_block`, `write_inlines`, `write_inline` as a `footnotes` parameter. Update the `FootnoteRef` arm:

```rust
Inline::FootnoteRef { id } => {
    if let Some(content) = footnotes.get(id) {
        out.push_str("#footnote[");
        let mut inner = String::new();
        self.write_blocks_with_footnotes(content, opts, &mut inner, footnotes);
        out.push_str(inner.trim());
        out.push(']');
    }
}
```

- [ ] **Step 4: Implement remaining blocks in write_block()**

Add match arms:

```rust
Block::BlockQuote { content } => {
    out.push_str("#quote(block: true)[\n");
    self.write_blocks_with_footnotes(content, opts, out, footnotes);
    out.push_str("]\n");
}
Block::List { ordered, start, items, .. } => {
    if *ordered {
        if let Some(s) = start {
            if *s != 1 {
                out.push_str(&format!("#set enum(start: {})\n", s));
            }
        }
    }
    let marker = if *ordered { "+ " } else { "- " };
    for item in items {
        if let Some(checked) = item.checked {
            let checkbox = if checked { "\u{2611} " } else { "\u{2610} " };
            out.push_str(marker);
            out.push_str(checkbox);
        } else {
            out.push_str(marker);
        }
        let mut item_content = String::new();
        self.write_blocks_with_footnotes(&item.content, opts, &mut item_content, footnotes);
        // For tight lists, trim trailing newlines and put on one line
        out.push_str(item_content.trim());
        out.push('\n');
    }
}
Block::Table(table) => {
    self.write_table(table, opts, out, footnotes);
}
Block::Figure { image, caption, label, .. } => {
    out.push_str("#figure(\n");
    out.push_str(&format!("  image(\"{}\"),\n", escape_typst_url(&image.url)));
    if let Some(cap) = caption {
        out.push_str("  caption: [");
        self.write_inlines_with_footnotes(cap, opts, out, footnotes);
        out.push_str("],\n");
    }
    out.push(')');
    if let Some(label) = label {
        out.push_str(&format!(" <{}>", label));
    }
    out.push('\n');
}
Block::ThematicBreak => {
    out.push_str("#line(length: 100%)\n");
}
Block::RawBlock { format, content } => {
    if format == "typst" || format == "typ" {
        out.push_str(content);
        if !content.ends_with('\n') {
            out.push('\n');
        }
    }
}
Block::Admonition { kind, title, content } => {
    let label = match kind {
        AdmonitionKind::Note => "Note",
        AdmonitionKind::Warning => "Warning",
        AdmonitionKind::Tip => "Tip",
        AdmonitionKind::Important => "Important",
        AdmonitionKind::Caution => "Caution",
        AdmonitionKind::Custom(c) => c.as_str(),
    };
    out.push_str("#block(inset: 1em, stroke: 0.5pt)[\n");
    if let Some(t) = title {
        out.push_str("*");
        self.write_inlines_with_footnotes(t, opts, out, footnotes);
        out.push_str("*\n\n");
    } else {
        out.push_str(&format!("*{}*\n\n", label));
    }
    self.write_blocks_with_footnotes(content, opts, out, footnotes);
    out.push_str("]\n");
}
Block::DefinitionList { items } => {
    for item in items {
        for def in &item.definitions {
            out.push_str("/ ");
            self.write_inlines_with_footnotes(&item.term, opts, out, footnotes);
            out.push_str(": ");
            let mut def_content = String::new();
            self.write_blocks_with_footnotes(def, opts, &mut def_content, footnotes);
            out.push_str(def_content.trim());
            out.push('\n');
        }
    }
}
Block::Div { content, .. } => {
    self.write_blocks_with_footnotes(content, opts, out, footnotes);
}
Block::FootnoteDef { .. } => {
    // Consumed by footnote pre-pass; skip in output
}
```

- [ ] **Step 5: Add table helper**

```rust
fn write_table(
    &self,
    table: &Table,
    opts: &WriteOptions,
    out: &mut String,
    footnotes: &HashMap<String, Vec<Block>>,
) {
    let has_wrapper = table.caption.is_some() || table.label.is_some();
    if has_wrapper {
        out.push_str("#figure(\n");
        if let Some(cap) = &table.caption {
            out.push_str("  caption: [");
            self.write_inlines_with_footnotes(cap, opts, out, footnotes);
            out.push_str("],\n");
        }
    }
    let ncols = table.columns.len().max(
        table.header.as_ref().map(|h| h.len())
            .or_else(|| table.rows.first().map(|r| r.len()))
            .unwrap_or(1),
    );
    out.push_str(&format!("#table(\n  columns: {},\n", ncols));

    // Alignment
    if table.columns.iter().any(|c| !matches!(c.alignment, Alignment::Default | Alignment::Left)) {
        out.push_str("  align: (");
        for (i, col) in table.columns.iter().enumerate() {
            if i > 0 { out.push_str(", "); }
            match col.alignment {
                Alignment::Left | Alignment::Default => out.push_str("left"),
                Alignment::Center => out.push_str("center"),
                Alignment::Right => out.push_str("right"),
            }
        }
        out.push_str("),\n");
    }

    // Header
    if let Some(header) = &table.header {
        out.push_str("  table.header(\n");
        for cell in header {
            out.push_str("    [");
            let mut cell_content = String::new();
            self.write_blocks_with_footnotes(&cell.content, opts, &mut cell_content, footnotes);
            out.push_str(cell_content.trim());
            out.push_str("],\n");
        }
        out.push_str("  ),\n");
    }

    // Body rows
    for row in &table.rows {
        for cell in row {
            out.push_str("  [");
            let mut cell_content = String::new();
            self.write_blocks_with_footnotes(&cell.content, opts, &mut cell_content, footnotes);
            out.push_str(cell_content.trim());
            out.push_str("],\n");
        }
    }

    out.push(')');
    if has_wrapper {
        out.push(')');
        if let Some(label) = &table.label {
            out.push_str(&format!(" <{}>", label));
        }
    }
    out.push('\n');
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-typst`
Expected: 13 tests PASS

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -p docmux-writer-typst --all-targets -- -D warnings`
Expected: Clean

- [ ] **Step 8: Commit**

```bash
git add crates/docmux-writer-typst/src/lib.rs
git commit -m "feat(typst-writer): lists, blockquotes, tables, figures, definitions, footnote expansion"
```

---

### Task 5: Standalone mode

**Files:**
- Modify: `crates/docmux-writer-typst/src/lib.rs`

- [ ] **Step 1: Write failing test**

```rust
#[test]
fn standalone_mode() {
    let doc = Document {
        metadata: Metadata {
            title: Some("My Paper".into()),
            authors: vec![Author {
                name: "Jane Doe".into(),
                affiliation: Some("MIT".into()),
                email: None,
                orcid: None,
            }],
            date: Some("2026-03-25".into()),
            abstract_text: Some("This paper is about things.".into()),
            ..Default::default()
        },
        content: vec![Block::text("Body text.")],
        ..Default::default()
    };
    let writer = TypstWriter::new();
    let opts = WriteOptions {
        standalone: true,
        ..Default::default()
    };
    let typ = writer.write(&doc, &opts).unwrap();
    assert!(typ.contains("#set document("));
    assert!(typ.contains("title: \"My Paper\""));
    assert!(typ.contains("Jane Doe"));
    assert!(typ.contains("Body text."));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p docmux-writer-typst -- standalone_mode`
Expected: FAIL — wrap_standalone is a stub

- [ ] **Step 3: Implement wrap_standalone()**

```rust
fn wrap_standalone(&self, body: &str, doc: &Document) -> String {
    let mut preamble = String::with_capacity(512);

    let has_meta = doc.metadata.title.is_some()
        || !doc.metadata.authors.is_empty()
        || doc.metadata.date.is_some();

    if has_meta {
        preamble.push_str("#set document(\n");
        if let Some(title) = &doc.metadata.title {
            preamble.push_str(&format!("  title: \"{}\",\n", escape_typst_url(title)));
        }
        if !doc.metadata.authors.is_empty() {
            let names: Vec<String> = doc
                .metadata
                .authors
                .iter()
                .map(|a| format!("\"{}\"", escape_typst_url(&a.name)))
                .collect();
            if names.len() == 1 {
                preamble.push_str(&format!("  author: {},\n", names[0]));
            } else {
                preamble.push_str(&format!("  author: ({}),\n", names.join(", ")));
            }
        }
        if let Some(date) = &doc.metadata.date {
            // Try to parse YYYY-MM-DD, otherwise use raw string
            let parts: Vec<&str> = date.split('-').collect();
            if parts.len() == 3
                && parts[0].parse::<u32>().is_ok()
                && parts[1].parse::<u32>().is_ok()
                && parts[2].parse::<u32>().is_ok()
            {
                preamble.push_str(&format!(
                    "  date: datetime(year: {}, month: {}, day: {}),\n",
                    parts[0], parts[1], parts[2]
                ));
            } else {
                preamble.push_str(&format!("  date: \"{}\",\n", escape_typst_url(date)));
            }
        }
        preamble.push_str(")\n\n");
    }

    if let Some(abstract_text) = &doc.metadata.abstract_text {
        preamble.push_str("#quote(block: true)[\n");
        preamble.push_str(&escape_typst(abstract_text));
        preamble.push_str("\n]\n\n");
    }

    preamble.push_str(body);
    preamble
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-typst`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-typst/src/lib.rs
git commit -m "feat(typst-writer): standalone mode with metadata preamble"
```

---

### Task 6: Table test, figure test, remaining edge cases

**Files:**
- Modify: `crates/docmux-writer-typst/src/lib.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[test]
fn table() {
    let doc = Document {
        content: vec![Block::Table(Table {
            caption: Some(vec![Inline::text("Results")]),
            label: Some("tab:results".into()),
            columns: vec![
                ColumnSpec { alignment: Alignment::Left, width: None },
                ColumnSpec { alignment: Alignment::Right, width: None },
            ],
            header: Some(vec![
                TableCell { content: vec![Block::text("Name")], colspan: 1, rowspan: 1 },
                TableCell { content: vec![Block::text("Value")], colspan: 1, rowspan: 1 },
            ]),
            rows: vec![vec![
                TableCell { content: vec![Block::text("Pi")], colspan: 1, rowspan: 1 },
                TableCell { content: vec![Block::text("3.14")], colspan: 1, rowspan: 1 },
            ]],
            attrs: None,
        })],
        ..Default::default()
    };
    let typ = write_typst(&doc);
    assert!(typ.contains("#table("));
    assert!(typ.contains("columns: 2"));
    assert!(typ.contains("[Name]"));
    assert!(typ.contains("[Pi]"));
    assert!(typ.contains("<tab:results>"));
}

#[test]
fn figure_with_caption() {
    let doc = Document {
        content: vec![Block::Figure {
            image: Image {
                url: "diagram.png".into(),
                alt: "Architecture".into(),
                title: None,
            },
            caption: Some(vec![Inline::text("System architecture")]),
            label: Some("fig:arch".into()),
            attrs: None,
        }],
        ..Default::default()
    };
    let typ = write_typst(&doc);
    assert!(typ.contains("#figure("));
    assert!(typ.contains("image(\"diagram.png\")"));
    assert!(typ.contains("caption: [System architecture]"));
    assert!(typ.contains("<fig:arch>"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-typst -- table figure_with_caption`
Expected: FAIL or PASS depending on what Task 4 already covers. If they pass, great — move on.

- [ ] **Step 3: Fix any assertion mismatches**

Adjust output formatting if the assertions don't match the exact output. This is a refinement step.

- [ ] **Step 4: Run full test suite + clippy + fmt**

Run: `cargo test -p docmux-writer-typst && cargo clippy -p docmux-writer-typst --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-typst/src/lib.rs
git commit -m "feat(typst-writer): table and figure tests, edge case fixes"
```

---

### Task 7: Golden file tests + CLI smoke tests

**Files:**
- Modify: `crates/docmux-cli/tests/golden.rs`
- Modify: `crates/docmux-cli/tests/cli_smoke.rs`

- [ ] **Step 1: Add Typst→Typst golden test function to golden.rs**

Add import at top of `golden.rs`:
```rust
use docmux_writer_typst::TypstWriter;
```

Add converter function:
```rust
fn convert_typ_to_typst(input: &str) -> String {
    let reader = TypstReader::new();
    let writer = TypstWriter::new();
    let opts = WriteOptions::default();
    let doc = reader
        .read(input)
        .expect("typst reader should not fail on fixture");
    writer
        .write(&doc, &opts)
        .expect("typst writer should not fail")
}
```

Add golden test (follow same pattern as `golden_typ_to_html` / `golden_typ_to_latex`):

```rust
#[test]
fn golden_typ_to_typst() {
    let base = fixtures_dir();
    let fixtures = discover_typ_fixtures(&base);

    if fixtures.is_empty() {
        eprintln!("No .typ fixtures found (skipping golden_typ_to_typst)");
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut generated = 0u32;
    let mut updated = 0u32;

    for fixture_path in &fixtures {
        let name = test_name(fixture_path, &base);
        let expected_path = fixture_path.with_extension("typ.typ");

        let input = std::fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read input: {e}"));
        let actual = convert_typ_to_typst(&input);

        if update_mode() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            updated += 1;
            eprintln!("  updated: {name}.typ.typ");
            continue;
        }

        if !expected_path.exists() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            generated += 1;
            eprintln!("  generated: {name}.typ.typ (new — review the file)");
            continue;
        }

        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read expected: {e}"));

        if actual != expected {
            failures.push(format!(
                "━━━ MISMATCH: {name}.typ.typ ━━━\n--- expected ({path})\n+++ actual\n\n{diff}\nHint: run `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden` to update.\n",
                path = expected_path.display(),
                diff = line_diff(&expected, &actual),
            ));
        }
    }

    if generated > 0 {
        eprintln!("\n  {} new .typ.typ expectation(s) generated.", generated);
    }
    if updated > 0 {
        eprintln!("\n  {} .typ.typ expectation(s) updated.", updated);
    }

    if !failures.is_empty() {
        panic!(
            "\n\n{count} .typ→.typ golden file(s) mismatched:\n\n{details}",
            count = failures.len(),
            details = failures.join("\n"),
        );
    }
}
```

- [ ] **Step 2: Add docmux-writer-typst dep to docmux-cli Cargo.toml (if not already)**

Should already be added in Task 1. Verify.

- [ ] **Step 3: Add CLI smoke tests**

In `crates/docmux-cli/tests/cli_smoke.rs`:

```rust
#[test]
fn converts_typst_to_typst_stdout() {
    let tmp = std::env::temp_dir().join("docmux-test");
    std::fs::create_dir_all(&tmp).ok();
    let input_file = tmp.join("roundtrip.typ");
    std::fs::write(&input_file, "= Hello\n\n*Bold* and _italic_.").unwrap();

    let output = Command::new(docmux_bin())
        .arg(&input_file)
        .arg("--to")
        .arg("typst")
        .output()
        .expect("failed to run docmux");

    assert!(
        output.status.success(),
        "docmux exited with error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("= Hello"), "Expected Typst heading in output");
    assert!(stdout.contains("*Bold*"), "Expected bold markup");
}
```

- [ ] **Step 4: Run golden tests to auto-generate expectation files**

Run: `cargo test -p docmux-cli --test golden -- golden_typ_to_typst`
Expected: 4 `.typ.typ` files generated. Review them.

- [ ] **Step 5: Run all CLI tests**

Run: `cargo test -p docmux-cli`
Expected: All pass (existing + new)

- [ ] **Step 6: Run full workspace tests + clippy**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-cli/tests/golden.rs crates/docmux-cli/tests/cli_smoke.rs tests/fixtures/
git commit -m "test(typst-writer): golden file tests (Typst→Typst roundtrip), CLI smoke tests"
```

---

### Task 8: Update roadmap and final verification

**Files:**
- Modify: `ROADMAP.md`

- [ ] **Step 1: Update roadmap**

Change the `docmux-writer-typst` line from `[ ]` to `[x]`:
```
- [x] `docmux-writer-typst` — Typst output
```

- [ ] **Step 2: Run full workspace verification**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
Expected: All green

- [ ] **Step 3: Commit**

```bash
git add ROADMAP.md
git commit -m "Mark docmux-writer-typst as complete in roadmap"
```
