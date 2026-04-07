# Cite Transform + Bibliography Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve `Inline::Citation` nodes to formatted text and generate bibliography blocks using hayagriva for CSL processing, with `--bibliography` and `--csl` CLI flags.

**Architecture:** Four coordinated pieces: (1) markdown reader citation parsing via post-processing text nodes, (2) `docmux-transform-cite` crate using hayagriva's `BibliographyDriver` for CSL formatting, (3) CLI flags + metadata fallback wiring, (4) WASM compatibility via separation of I/O from transform logic.

**Tech Stack:** Rust, hayagriva (CSL processing), citationberg (CSL parsing), regex crate (citation syntax matching)

**Spec:** `docs/superpowers/specs/2026-04-07-cite-transform-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `Cargo.toml` (workspace root) | Modify | Add `hayagriva` and `regex` to workspace deps |
| `crates/docmux-transform-cite/Cargo.toml` | Modify | Add `hayagriva`, `citationberg` deps |
| `crates/docmux-transform-cite/src/lib.rs` | Rewrite | Cite transform: resolve citations, insert bibliography |
| `crates/docmux-reader-markdown/Cargo.toml` | Modify | Add `regex` dep |
| `crates/docmux-reader-markdown/src/lib.rs` | Modify | Add `postprocess_citations()` in `collect_inlines()` pipeline |
| `crates/docmux-cli/Cargo.toml` | Modify | Add `docmux-transform-cite` dep |
| `crates/docmux-cli/src/main.rs` | Modify | Add `--bibliography`, `--csl` flags + transform wiring |
| `tests/fixtures/citations/` | Create | Golden file test fixtures |

---

### Task 1: WASM Compatibility Spike

**Files:**
- Modify: `Cargo.toml:38-74` (workspace deps)
- Modify: `crates/docmux-transform-cite/Cargo.toml:10-12`

Verify that hayagriva compiles to `wasm32-unknown-unknown` before writing any real code. If it fails, we'll add a feature gate.

- [ ] **Step 1: Add hayagriva to workspace deps**

In `Cargo.toml` (workspace root), add to `[workspace.dependencies]` after the `base64` line:

```toml
hayagriva = { version = "0.9", default-features = false, features = ["biblatex"] }
citationberg = "0.6"
regex = "1"
```

Note: We start WITHOUT the `archive` feature (which bundles CSL styles and pulls in `ciborium`) to keep WASM clean. The `archive` feature will be CLI-only.

- [ ] **Step 2: Add hayagriva dep to cite transform crate**

In `crates/docmux-transform-cite/Cargo.toml`, replace `[dependencies]` with:

```toml
[dependencies]
docmux-ast = { workspace = true }
docmux-core = { workspace = true }
hayagriva = { workspace = true }
citationberg = { workspace = true }
```

- [ ] **Step 3: Add a minimal import to lib.rs to force compilation**

In `crates/docmux-transform-cite/src/lib.rs`:

```rust
//! # docmux-transform-cite
//!
//! CSL citation processing for docmux using hayagriva.

use docmux_ast::Document;
use docmux_core::{Result, Transform, TransformContext};

/// Cite transform — resolves citations and inserts bibliography.
#[derive(Debug, Default)]
pub struct CiteTransform;

impl CiteTransform {
    pub fn new() -> Self {
        Self
    }
}

impl Transform for CiteTransform {
    fn name(&self) -> &str {
        "cite"
    }

    fn transform(&self, _doc: &mut Document, _ctx: &TransformContext) -> Result<()> {
        // Placeholder — will be implemented in Task 3
        Ok(())
    }
}
```

- [ ] **Step 4: Test WASM compilation**

Run:
```bash
cargo build --target wasm32-unknown-unknown -p docmux-transform-cite
```

Expected: compiles successfully. If it fails on a specific dep, note which one and we'll feature-gate it.

- [ ] **Step 5: Test workspace still compiles**

Run:
```bash
cargo check --workspace
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/docmux-transform-cite/
git commit -m "chore: add hayagriva + citationberg deps, verify WASM compat"
```

---

### Task 2: Markdown Reader — Citation Parsing

**Files:**
- Modify: `crates/docmux-reader-markdown/Cargo.toml:10-15`
- Modify: `crates/docmux-reader-markdown/src/lib.rs:360-368` (collect_inlines), plus new function at end of file

Add `postprocess_citations()` to the markdown reader's inline post-processing pipeline. This parses pandoc-style `[@key]` syntax from text nodes and converts them to `Inline::Citation` AST nodes.

- [ ] **Step 1: Add regex dep**

In `crates/docmux-reader-markdown/Cargo.toml`, add to `[dependencies]`:

```toml
regex = { workspace = true }
```

- [ ] **Step 2: Write failing tests for citation parsing**

At the bottom of `crates/docmux-reader-markdown/src/lib.rs`, inside the existing `#[cfg(test)] mod tests` block, add:

```rust
#[test]
fn citation_single_key() {
    let doc = MarkdownReader::new().read("See [@smith2020].").unwrap();
    let inlines = first_para_inlines(&doc);
    // "See " + Citation + "."
    assert_eq!(inlines.len(), 3);
    match &inlines[1] {
        Inline::Citation(c) => {
            assert_eq!(c.items.len(), 1);
            assert_eq!(c.items[0].key, "smith2020");
            assert_eq!(c.mode, CitationMode::Normal);
        }
        other => panic!("expected Citation, got {other:?}"),
    }
}

#[test]
fn citation_multi_key() {
    let doc = MarkdownReader::new().read("[@smith2020; @jones2021]").unwrap();
    let inlines = first_para_inlines(&doc);
    assert_eq!(inlines.len(), 1);
    match &inlines[0] {
        Inline::Citation(c) => {
            assert_eq!(c.items.len(), 2);
            assert_eq!(c.items[0].key, "smith2020");
            assert_eq!(c.items[1].key, "jones2021");
        }
        other => panic!("expected Citation, got {other:?}"),
    }
}

#[test]
fn citation_suppress_author() {
    let doc = MarkdownReader::new().read("[-@smith2020]").unwrap();
    let inlines = first_para_inlines(&doc);
    match &inlines[0] {
        Inline::Citation(c) => {
            assert_eq!(c.items[0].key, "smith2020");
            assert_eq!(c.mode, CitationMode::SuppressAuthor);
        }
        other => panic!("expected Citation, got {other:?}"),
    }
}

#[test]
fn citation_with_prefix_suffix() {
    let doc = MarkdownReader::new().read("[see @smith2020, p. 42]").unwrap();
    let inlines = first_para_inlines(&doc);
    match &inlines[0] {
        Inline::Citation(c) => {
            assert_eq!(c.items[0].key, "smith2020");
            assert_eq!(c.items[0].prefix.as_deref(), Some("see"));
            assert_eq!(c.items[0].suffix.as_deref(), Some("p. 42"));
        }
        other => panic!("expected Citation, got {other:?}"),
    }
}

#[test]
fn citation_narrative_inline() {
    let doc = MarkdownReader::new().read("As @smith2020 argues").unwrap();
    let inlines = first_para_inlines(&doc);
    // "As " + Citation + " argues"
    assert_eq!(inlines.len(), 3);
    match &inlines[1] {
        Inline::Citation(c) => {
            assert_eq!(c.items[0].key, "smith2020");
            assert_eq!(c.mode, CitationMode::AuthorOnly);
        }
        other => panic!("expected Citation, got {other:?}"),
    }
}

#[test]
fn citation_no_false_positive_email() {
    let doc = MarkdownReader::new().read("Contact user@example.com for info.").unwrap();
    let inlines = first_para_inlines(&doc);
    // Should NOT produce any Citation nodes
    for inline in &inlines {
        assert!(!matches!(inline, Inline::Citation(_)), "email wrongly parsed as citation");
    }
}

/// Helper: extract inlines from the first paragraph.
fn first_para_inlines(doc: &Document) -> &[Inline] {
    match &doc.content[0] {
        Block::Paragraph { content, .. } => content,
        other => panic!("expected Paragraph, got {other:?}"),
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run:
```bash
cargo test -p docmux-reader-markdown -- citation
```

Expected: FAIL — `Citation` nodes are not produced yet (text stays as `Inline::Text`).

- [ ] **Step 4: Implement `postprocess_citations()`**

At the end of `crates/docmux-reader-markdown/src/lib.rs` (before the `#[cfg(test)]` block), add:

```rust
// ─── Citation parsing (pandoc-style) ───────────────────────────────────────

use regex::Regex;
use std::sync::LazyLock;

/// Bracketed citation: `[@key]`, `[@k1; @k2]`, `[see @key, p. 42]`, `[-@key]`
static BRACKETED_CITE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[(?:[^\[\]]*?)(-?)@([\w:.#$%&\-+?<>~/]+)(?:[^\[\]]*?)\]").unwrap()
});

/// Full bracketed citation pattern — matches the entire `[...]` block.
static FULL_BRACKET_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[([^\[\]]*@[\w:.#$%&\-+?<>~/]+[^\[\]]*)\]").unwrap()
});

/// A single cite item within brackets: optional prefix, optional `-`, `@key`, optional suffix.
static CITE_ITEM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|;\s*)([^@;]*?)(-?)@([\w:.#$%&\-+?<>~/]+)([^;]*)").unwrap()
});

/// Inline narrative citation: `@key` not preceded by letter/digit/`[`.
static NARRATIVE_CITE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?<![[\w])@([\w:.#$%&\-+?<>~/]+)").unwrap()
});

/// Walk inline nodes and replace text containing citation syntax with
/// `Inline::Citation` nodes. Text before/after the citation is preserved.
fn postprocess_citations(inlines: &mut Vec<Inline>) {
    let mut i = 0;
    while i < inlines.len() {
        if let Inline::Text { value } = &inlines[i] {
            if let Some(replacements) = parse_citations_in_text(value) {
                inlines.splice(i..=i, replacements.into_iter());
                continue; // re-check at same index
            }
        }
        i += 1;
    }
}

/// Try to parse citation(s) from a text string. Returns `None` if no citations found.
/// Returns `Some(vec of inlines)` with text/citation nodes if citations were found.
fn parse_citations_in_text(text: &str) -> Option<Vec<Inline>> {
    // First try bracketed citations
    if let Some(m) = FULL_BRACKET_RE.find(text) {
        let mut result = Vec::new();

        // Text before the citation
        let before = &text[..m.start()];
        if !before.is_empty() {
            result.push(Inline::Text { value: before.to_string() });
        }

        // Parse the bracket content
        let bracket_content = &text[m.start() + 1..m.end() - 1]; // strip [ and ]
        let citation = parse_bracketed_citation(bracket_content);
        result.push(Inline::Citation(citation));

        // Text after — recurse to find more citations
        let after = &text[m.end()..];
        if !after.is_empty() {
            if let Some(more) = parse_citations_in_text(after) {
                result.extend(more);
            } else {
                result.push(Inline::Text { value: after.to_string() });
            }
        }

        return Some(result);
    }

    // Then try narrative citations (@key inline)
    if let Some(m) = NARRATIVE_CITE_RE.find(text) {
        // Check it's not an email: no letter/digit immediately before @
        let before_char = if m.start() > 0 {
            text[..m.start()].chars().last()
        } else {
            None
        };
        if before_char.is_some_and(|c| c.is_alphanumeric()) {
            return None; // email address, skip
        }

        let mut result = Vec::new();
        let before = &text[..m.start()];
        if !before.is_empty() {
            result.push(Inline::Text { value: before.to_string() });
        }

        let key = &text[m.start() + 1..m.end()]; // skip @
        result.push(Inline::Citation(Citation {
            items: vec![CiteItem {
                key: key.to_string(),
                prefix: None,
                suffix: None,
            }],
            mode: CitationMode::AuthorOnly,
        }));

        let after = &text[m.end()..];
        if !after.is_empty() {
            if let Some(more) = parse_citations_in_text(after) {
                result.extend(more);
            } else {
                result.push(Inline::Text { value: after.to_string() });
            }
        }

        return Some(result);
    }

    None
}

/// Parse the content inside `[...]` into a `Citation`.
/// Input is the text between brackets, e.g. `see @smith2020, p. 42; -@jones2021`.
fn parse_bracketed_citation(content: &str) -> Citation {
    let mut items = Vec::new();
    let mut has_suppress = false;

    for cap in CITE_ITEM_RE.captures_iter(content) {
        let prefix_raw = cap[1].trim();
        let suppress = &cap[2] == "-";
        let key = cap[3].to_string();
        let suffix_raw = cap[4].trim().trim_start_matches(',').trim();

        if suppress {
            has_suppress = true;
        }

        items.push(CiteItem {
            key,
            prefix: if prefix_raw.is_empty() { None } else { Some(prefix_raw.to_string()) },
            suffix: if suffix_raw.is_empty() { None } else { Some(suffix_raw.to_string()) },
        });
    }

    let mode = if has_suppress {
        CitationMode::SuppressAuthor
    } else {
        CitationMode::Normal
    };

    Citation { items, mode }
}
```

- [ ] **Step 5: Wire postprocess_citations into collect_inlines**

In `crates/docmux-reader-markdown/src/lib.rs`, find the `collect_inlines` method (around line 360). After the existing post-processing calls, add `postprocess_citations`:

Change:
```rust
        postprocess_raw_inlines(&mut inlines);
        postprocess_bracketed_spans(&mut inlines);
        inlines
```

To:
```rust
        postprocess_raw_inlines(&mut inlines);
        postprocess_bracketed_spans(&mut inlines);
        postprocess_citations(&mut inlines);
        inlines
```

- [ ] **Step 6: Run tests to verify they pass**

Run:
```bash
cargo test -p docmux-reader-markdown -- citation
```

Expected: all 6 citation tests PASS.

- [ ] **Step 7: Run full workspace tests**

Run:
```bash
cargo test --workspace
```

Expected: no regressions. Existing golden files should not change (no existing fixtures contain `[@...]` syntax).

- [ ] **Step 8: Commit**

```bash
git add crates/docmux-reader-markdown/
git commit -m "feat(markdown-reader): parse pandoc-style citation syntax [@key]"
```

---

### Task 3: Cite Transform — Core Implementation

**Files:**
- Rewrite: `crates/docmux-transform-cite/src/lib.rs`

Implement the two-pass cite transform using hayagriva's `BibliographyDriver`.

- [ ] **Step 1: Write failing tests**

In `crates/docmux-transform-cite/src/lib.rs`, add tests at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use docmux_ast::*;

    fn make_citation(keys: &[&str], mode: CitationMode) -> Inline {
        Inline::Citation(Citation {
            items: keys
                .iter()
                .map(|k| CiteItem {
                    key: k.to_string(),
                    prefix: None,
                    suffix: None,
                })
                .collect(),
            mode,
        })
    }

    fn para(inlines: Vec<Inline>) -> Block {
        Block::Paragraph {
            content: inlines,
            attrs: None,
        }
    }

    fn simple_bib_yaml() -> &'static str {
        r#"
smith2020:
    type: Article
    title: A Great Paper
    author: Smith, John
    date: 2020
    parent:
        type: Periodical
        title: Nature
jones2021:
    type: Book
    title: Some Book
    author: Jones, Alice
    date: 2021
    publisher: MIT Press
"#
    }

    #[test]
    fn resolves_known_citation() {
        let lib = hayagriva::io::from_yaml_str(simple_bib_yaml()).unwrap();
        let transform = CiteTransform::with_library(lib, None);

        let mut doc = Document {
            content: vec![para(vec![make_citation(&["smith2020"], CitationMode::Normal)])],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        // Citation should be resolved to text (not remain as Inline::Citation)
        let inlines = match &doc.content[0] {
            Block::Paragraph { content, .. } => content,
            other => panic!("expected Paragraph, got {other:?}"),
        };
        assert!(
            !matches!(&inlines[0], Inline::Citation(_)),
            "citation should be resolved to text"
        );
    }

    #[test]
    fn unknown_key_becomes_placeholder() {
        let lib = hayagriva::io::from_yaml_str(simple_bib_yaml()).unwrap();
        let transform = CiteTransform::with_library(lib, None);

        let mut doc = Document {
            content: vec![para(vec![make_citation(
                &["nonexistent"],
                CitationMode::Normal,
            )])],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        let inlines = match &doc.content[0] {
            Block::Paragraph { content, .. } => content,
            other => panic!("expected Paragraph, got {other:?}"),
        };
        match &inlines[0] {
            Inline::Text { value } => assert!(value.contains("[?nonexistent]")),
            other => panic!("expected Text with placeholder, got {other:?}"),
        }
    }

    #[test]
    fn bibliography_appended_at_end() {
        let lib = hayagriva::io::from_yaml_str(simple_bib_yaml()).unwrap();
        let transform = CiteTransform::with_library(lib, None);

        let mut doc = Document {
            content: vec![para(vec![make_citation(&["smith2020"], CitationMode::Normal)])],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        // Last block should be the bibliography div
        let last = doc.content.last().unwrap();
        match last {
            Block::Div { attrs, .. } => {
                assert_eq!(attrs.as_ref().and_then(|a| a.id.as_deref()), Some("refs"));
            }
            other => panic!("expected Div#refs, got {other:?}"),
        }
    }

    #[test]
    fn bibliography_replaces_refs_div() {
        let lib = hayagriva::io::from_yaml_str(simple_bib_yaml()).unwrap();
        let transform = CiteTransform::with_library(lib, None);

        let refs_div = Block::Div {
            content: vec![],
            attrs: Some(Attributes {
                id: Some("refs".to_string()),
                classes: vec![],
                key_values: HashMap::new(),
            }),
        };

        let mut doc = Document {
            content: vec![
                para(vec![make_citation(&["smith2020"], CitationMode::Normal)]),
                refs_div,
                para(vec![Inline::Text {
                    value: "Appendix".to_string(),
                }]),
            ],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        // Div#refs should be at position 1 (not at end), and appendix at position 2
        assert_eq!(doc.content.len(), 3);
        match &doc.content[1] {
            Block::Div { attrs, content, .. } => {
                assert_eq!(attrs.as_ref().and_then(|a| a.id.as_deref()), Some("refs"));
                assert!(!content.is_empty(), "refs div should have bibliography entries");
            }
            other => panic!("expected Div#refs at position 1, got {other:?}"),
        }
    }

    #[test]
    fn no_bibliography_when_no_citations() {
        let lib = hayagriva::io::from_yaml_str(simple_bib_yaml()).unwrap();
        let transform = CiteTransform::with_library(lib, None);

        let mut doc = Document {
            content: vec![para(vec![Inline::Text {
                value: "No citations here.".to_string(),
            }])],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        // No bibliography should be added
        assert_eq!(doc.content.len(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:
```bash
cargo test -p docmux-transform-cite
```

Expected: FAIL — `CiteTransform::with_library` doesn't exist yet.

- [ ] **Step 3: Implement the cite transform**

Replace the full content of `crates/docmux-transform-cite/src/lib.rs` with:

```rust
//! # docmux-transform-cite
//!
//! CSL citation processing for docmux. Two-pass transform:
//!
//! 1. **Walk** the AST and collect all `Inline::Citation` nodes, resolving each
//!    to formatted text via hayagriva's `BibliographyDriver`.
//! 2. **Insert** a formatted bibliography at `Div#refs` or at the end of the document.

use citationberg::IndependentStyle;
use docmux_ast::*;
use docmux_core::{Result, Transform, TransformContext};
use hayagriva::{
    BibliographyDriver, BibliographyRequest, BufWriteFormat, CitationItem, CitationRequest,
    CitePurpose, Library,
};
use std::collections::{HashMap, HashSet};

/// Default CSL style XML (Chicago Author-Date 17th edition).
/// Embedded so the transform works without a `--csl` flag.
const DEFAULT_STYLE: &str = include_str!("chicago-author-date.csl");

/// Cite transform — resolves citations and inserts bibliography.
pub struct CiteTransform {
    library: Library,
    style: IndependentStyle,
}

impl std::fmt::Debug for CiteTransform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CiteTransform")
            .field("entries", &self.library.len())
            .finish()
    }
}

impl CiteTransform {
    /// Create a cite transform with a pre-parsed library and optional CSL style XML.
    /// If `csl_xml` is `None`, uses the built-in Chicago Author-Date style.
    pub fn with_library(library: Library, csl_xml: Option<&str>) -> Self {
        let style_str = csl_xml.unwrap_or(DEFAULT_STYLE);
        let style = IndependentStyle::from_xml(style_str)
            .expect("built-in CSL style should always parse");
        Self { library, style }
    }
}

impl Transform for CiteTransform {
    fn name(&self) -> &str {
        "cite"
    }

    fn transform(&self, doc: &mut Document, _ctx: &TransformContext) -> Result<()> {
        // Pass 1: collect all citation groups from the AST
        let citation_groups = collect_citations(&doc.content);
        if citation_groups.is_empty() {
            return Ok(());
        }

        // Build a driver and feed it all citations
        let mut driver = BibliographyDriver::new();
        let mut cited_keys: HashSet<String> = HashSet::new();
        let mut unresolved: HashSet<String> = HashSet::new();

        for group in &citation_groups {
            let mut items: Vec<CitationItem<hayagriva::Entry>> = Vec::new();
            for cite_item in &group.items {
                if let Some(entry) = self.library.get(&cite_item.key) {
                    let mut ci = CitationItem::with_entry(entry);
                    ci.purpose = match group.mode {
                        CitationMode::AuthorOnly => Some(CitePurpose::Prose),
                        CitationMode::SuppressAuthor => Some(CitePurpose::Year),
                        CitationMode::Normal => None,
                    };
                    items.push(ci);
                    cited_keys.insert(cite_item.key.clone());
                } else {
                    unresolved.insert(cite_item.key.clone());
                }
            }

            if !items.is_empty() {
                driver.citation(CitationRequest::from_items(items, &self.style, &[]));
            }
        }

        // Emit warnings for unresolved keys
        for key in &unresolved {
            eprintln!("warning: citation key '{key}' not found in bibliography");
        }

        // Finish the driver to get formatted output
        let rendered = driver.finish(BibliographyRequest {
            style: &self.style,
            locale: None,
            locale_files: &[],
        });

        // Pass 2: replace Citation nodes with formatted text
        let mut cite_idx = 0;
        replace_citations_in_blocks(&mut doc.content, &rendered.citations, &citation_groups, &unresolved, &mut cite_idx);

        // Pass 3: insert bibliography
        if let Some(bib) = &rendered.bibliography {
            let bib_blocks = format_bibliography(bib);
            insert_bibliography(&mut doc.content, bib_blocks);
        }

        Ok(())
    }
}

// ─── Citation collection ───────────────────────────────────────────────────

/// A citation group found in the AST, preserving order.
#[derive(Debug, Clone)]
struct CitationGroup {
    items: Vec<CiteItem>,
    mode: CitationMode,
}

/// Walk the AST and collect all Citation nodes in document order.
fn collect_citations(blocks: &[Block]) -> Vec<CitationGroup> {
    let mut groups = Vec::new();
    for block in blocks {
        collect_citations_in_block(block, &mut groups);
    }
    groups
}

fn collect_citations_in_block(block: &Block, groups: &mut Vec<CitationGroup>) {
    match block {
        Block::Paragraph { content, .. }
        | Block::Heading { content, .. }
        | Block::Caption { content, .. } => {
            collect_citations_in_inlines(content, groups);
        }
        Block::BlockQuote { content, .. }
        | Block::Div { content, .. }
        | Block::Section { content, .. } => {
            for child in content {
                collect_citations_in_block(child, groups);
            }
        }
        Block::OrderedList { items, .. } | Block::BulletList { items, .. } => {
            for item in items {
                for child in &item.content {
                    collect_citations_in_block(child, groups);
                }
            }
        }
        Block::DefinitionList { items, .. } => {
            for item in items {
                collect_citations_in_inlines(&item.term, groups);
                for def in &item.definitions {
                    for child in def {
                        collect_citations_in_block(child, groups);
                    }
                }
            }
        }
        Block::Table { rows, caption, .. } => {
            if let Some(cap) = caption {
                collect_citations_in_block(cap, groups);
            }
            for row in rows {
                for cell in &row.cells {
                    for child in &cell.content {
                        collect_citations_in_block(child, groups);
                    }
                }
            }
        }
        Block::Footnote { content, .. } => {
            for child in content {
                collect_citations_in_block(child, groups);
            }
        }
        // Leaf blocks with no inlines
        Block::CodeBlock { .. }
        | Block::MathBlock { .. }
        | Block::RawBlock { .. }
        | Block::ThematicBreak
        | Block::HorizontalRule
        | Block::Admonition { .. }
        | Block::Image { .. } => {}
    }
}

fn collect_citations_in_inlines(inlines: &[Inline], groups: &mut Vec<CitationGroup>) {
    for inline in inlines {
        match inline {
            Inline::Citation(c) => {
                groups.push(CitationGroup {
                    items: c.items.clone(),
                    mode: c.mode,
                });
            }
            Inline::Emphasis { content }
            | Inline::Strong { content }
            | Inline::Strikethrough { content }
            | Inline::Underline { content }
            | Inline::Subscript { content }
            | Inline::Superscript { content }
            | Inline::SmallCaps { content }
            | Inline::Span { content, .. }
            | Inline::Link { content, .. } => {
                collect_citations_in_inlines(content, groups);
            }
            _ => {}
        }
    }
}

// ─── Citation replacement ──────────────────────────────────────────────────

/// Walk blocks and replace `Inline::Citation` with formatted text.
fn replace_citations_in_blocks(
    blocks: &mut [Block],
    rendered: &[hayagriva::RenderedCitation],
    groups: &[CitationGroup],
    unresolved: &HashSet<String>,
    cite_idx: &mut usize,
) {
    for block in blocks.iter_mut() {
        match block {
            Block::Paragraph { content, .. }
            | Block::Heading { content, .. }
            | Block::Caption { content, .. } => {
                replace_citations_in_inlines(content, rendered, groups, unresolved, cite_idx);
            }
            Block::BlockQuote { content, .. }
            | Block::Div { content, .. }
            | Block::Section { content, .. } => {
                replace_citations_in_blocks(content, rendered, groups, unresolved, cite_idx);
            }
            Block::OrderedList { items, .. } | Block::BulletList { items, .. } => {
                for item in items.iter_mut() {
                    replace_citations_in_blocks(&mut item.content, rendered, groups, unresolved, cite_idx);
                }
            }
            Block::DefinitionList { items, .. } => {
                for item in items.iter_mut() {
                    replace_citations_in_inlines(&mut item.term, rendered, groups, unresolved, cite_idx);
                    for def in &mut item.definitions {
                        replace_citations_in_blocks(def, rendered, groups, unresolved, cite_idx);
                    }
                }
            }
            Block::Table { rows, caption, .. } => {
                if let Some(cap) = caption {
                    replace_citations_in_blocks(std::slice::from_mut(cap), rendered, groups, unresolved, cite_idx);
                }
                for row in rows.iter_mut() {
                    for cell in &mut row.cells {
                        replace_citations_in_blocks(&mut cell.content, rendered, groups, unresolved, cite_idx);
                    }
                }
            }
            Block::Footnote { content, .. } => {
                replace_citations_in_blocks(content, rendered, groups, unresolved, cite_idx);
            }
            _ => {}
        }
    }
}

fn replace_citations_in_inlines(
    inlines: &mut Vec<Inline>,
    rendered: &[hayagriva::RenderedCitation],
    groups: &[CitationGroup],
    unresolved: &HashSet<String>,
    cite_idx: &mut usize,
) {
    let mut i = 0;
    while i < inlines.len() {
        match &mut inlines[i] {
            Inline::Citation(_) => {
                let group = &groups[*cite_idx];

                // Check if all keys in this group are unresolved
                let all_unresolved = group.items.iter().all(|item| unresolved.contains(&item.key));

                let replacement = if all_unresolved {
                    // All keys unresolved — produce placeholder
                    let placeholders: Vec<String> = group
                        .items
                        .iter()
                        .map(|item| format!("[?{}]", item.key))
                        .collect();
                    Inline::Text {
                        value: placeholders.join("; "),
                    }
                } else if let Some(rc) = rendered.get(*cite_idx) {
                    // Use hayagriva's formatted output
                    let mut buf = String::new();
                    rc.citation.write_buf(&mut buf, BufWriteFormat::Plain).ok();
                    Inline::Text { value: buf }
                } else {
                    // Fallback: shouldn't happen, but be safe
                    let keys: Vec<String> = group.items.iter().map(|i| format!("[?{}]", i.key)).collect();
                    Inline::Text { value: keys.join("; ") }
                };

                inlines[i] = replacement;
                *cite_idx += 1;
                i += 1;
            }
            Inline::Emphasis { content }
            | Inline::Strong { content }
            | Inline::Strikethrough { content }
            | Inline::Underline { content }
            | Inline::Subscript { content }
            | Inline::Superscript { content }
            | Inline::SmallCaps { content }
            | Inline::Span { content, .. }
            | Inline::Link { content, .. } => {
                replace_citations_in_inlines(content, rendered, groups, unresolved, cite_idx);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
}

// ─── Bibliography insertion ────────────────────────────────────────────────

/// Format bibliography entries as AST blocks.
fn format_bibliography(bib: &hayagriva::RenderedBibliography) -> Vec<Block> {
    let mut entries = Vec::new();
    for item in &bib.items {
        let mut buf = String::new();
        item.content.write_buf(&mut buf, BufWriteFormat::Plain).ok();
        entries.push(Block::Paragraph {
            content: vec![Inline::Text { value: buf }],
            attrs: None,
        });
    }
    entries
}

/// Insert bibliography blocks at `Div#refs` if it exists, otherwise append at end.
fn insert_bibliography(blocks: &mut Vec<Block>, bib_blocks: Vec<Block>) {
    // Search for existing Div#refs
    for block in blocks.iter_mut() {
        if let Block::Div { attrs, content, .. } = block {
            if attrs.as_ref().and_then(|a| a.id.as_deref()) == Some("refs") {
                *content = bib_blocks;
                return;
            }
        }
    }

    // No Div#refs found — append at end wrapped in Div#refs
    blocks.push(Block::Div {
        content: bib_blocks,
        attrs: Some(Attributes {
            id: Some("refs".to_string()),
            classes: vec!["references".to_string()],
            key_values: HashMap::new(),
        }),
    });
}
```

- [ ] **Step 4: Download and embed the default CSL style**

Download the Chicago Author-Date CSL file and save it in the crate:

```bash
curl -sL "https://raw.githubusercontent.com/citation-style-language/styles/master/chicago-author-date.csl" \
  -o crates/docmux-transform-cite/src/chicago-author-date.csl
```

Verify it's valid XML:
```bash
head -3 crates/docmux-transform-cite/src/chicago-author-date.csl
```

Expected: XML declaration and `<style>` root element.

- [ ] **Step 5: Run tests**

Run:
```bash
cargo test -p docmux-transform-cite
```

Expected: all 5 tests PASS.

- [ ] **Step 6: Run clippy**

Run:
```bash
cargo clippy -p docmux-transform-cite --all-targets -- -D warnings
```

Expected: no warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-transform-cite/
git commit -m "feat(transform-cite): implement CSL citation resolution with hayagriva"
```

---

### Task 4: CLI Flags + Wiring

**Files:**
- Modify: `crates/docmux-cli/Cargo.toml:14-36`
- Modify: `crates/docmux-cli/src/main.rs:4-16` (imports), `26-146` (CLI struct), `306-345` (transform pipeline)

Add `--bibliography` and `--csl` CLI flags, load bibliography files, and wire the cite transform into the pipeline.

- [ ] **Step 1: Add docmux-transform-cite dep to CLI**

In `crates/docmux-cli/Cargo.toml`, add to `[dependencies]`:

```toml
docmux-transform-cite = { workspace = true }
hayagriva = { workspace = true }
```

- [ ] **Step 2: Add CLI flags**

In `crates/docmux-cli/src/main.rs`, add the new fields to the `Cli` struct, after the `section_divs` field (around line 129):

```rust
    /// Bibliography file(s) — BibTeX (.bib) or Hayagriva YAML (.yml/.yaml)
    #[arg(long, value_name = "FILE")]
    bibliography: Vec<PathBuf>,

    /// CSL citation style file (default: Chicago Author-Date)
    #[arg(long, value_name = "FILE")]
    csl: Option<PathBuf>,
```

- [ ] **Step 3: Add import**

At the top of `main.rs`, add with the other transform imports (after line 15):

```rust
use docmux_transform_cite::CiteTransform;
```

- [ ] **Step 4: Wire the cite transform into the pipeline**

In `main.rs`, after the `--toc` transform block (around line 345) and before the verbose warnings block, add:

```rust
    // Apply cite transform (when --bibliography is provided or metadata has bibliography)
    let bib_paths: Vec<PathBuf> = if !cli.bibliography.is_empty() {
        cli.bibliography.clone()
    } else if let Some(MetaValue::String(bib_path)) = doc.metadata.custom.get("bibliography") {
        vec![PathBuf::from(bib_path)]
    } else if let Some(MetaValue::List(bib_list)) = doc.metadata.custom.get("bibliography") {
        bib_list
            .iter()
            .filter_map(|v| {
                if let MetaValue::String(s) = v {
                    Some(PathBuf::from(s))
                } else {
                    None
                }
            })
            .collect()
    } else {
        vec![]
    };

    if !bib_paths.is_empty() {
        // Load all bibliography files into one library
        let mut combined = hayagriva::Library::new();
        for path in &bib_paths {
            let content = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("docmux: cannot read bibliography file {}: {e}", path.display());
                    std::process::exit(1);
                }
            };

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let lib = match ext {
                "bib" => match hayagriva::io::from_biblatex_str(&content) {
                    Ok(lib) => lib,
                    Err(e) => {
                        eprintln!("docmux: BibTeX parse error in {}: {e:?}", path.display());
                        std::process::exit(1);
                    }
                },
                "yml" | "yaml" => match hayagriva::io::from_yaml_str(&content) {
                    Ok(lib) => lib,
                    Err(e) => {
                        eprintln!("docmux: YAML bibliography parse error in {}: {e}", path.display());
                        std::process::exit(1);
                    }
                },
                other => {
                    eprintln!("docmux: unsupported bibliography format '.{other}' (expected .bib, .yml, or .yaml)");
                    std::process::exit(1);
                }
            };

            // Merge entries — Library may not have push(), so clone entries in
            for entry in lib.iter() {
                combined.push(entry.clone());
            }
        }

        // Resolve CSL style path: CLI flag > metadata > default (None = built-in)
        let csl_file = cli.csl.clone().or_else(|| {
            doc.metadata
                .custom
                .get("csl")
                .and_then(|v| match v {
                    MetaValue::String(s) => Some(PathBuf::from(s)),
                    _ => None,
                })
        });

        let csl_xml = match &csl_file {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(s) => Some(s),
                Err(e) => {
                    eprintln!("docmux: cannot read CSL file {}: {e}", path.display());
                    std::process::exit(1);
                }
            },
            None => None, // will use built-in chicago-author-date
        };

        let cite_transform = CiteTransform::with_library(combined, csl_xml.as_deref());
        if let Err(e) = cite_transform.transform(&mut doc, &TransformContext::default()) {
            eprintln!("docmux: cite transform error: {e}");
            std::process::exit(1);
        }
    }
```

- [ ] **Step 5: Verify it compiles**

Run:
```bash
cargo check -p docmux-cli
```

Expected: compiles with no errors.

- [ ] **Step 6: Run workspace tests**

Run:
```bash
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-cli/
git commit -m "feat(cli): add --bibliography and --csl flags for citation processing"
```

---

### Task 5: Golden File Tests + CLI Integration

**Files:**
- Create: `tests/fixtures/citations/basic.md`
- Create: `tests/fixtures/citations/refs.bib`
- Create: `tests/fixtures/citations/basic.html` (auto-generated)
- Modify: `crates/docmux-cli/tests/golden.rs` (if CLI args needed)

End-to-end tests: markdown with citations + bibliography file → resolved HTML output.

Note: The golden test harness runs `MarkdownReader → HtmlWriter` without CLI flags, so it won't exercise `--bibliography`. We need a separate integration test for the full CLI pipeline with bibliography.

- [ ] **Step 1: Create test bibliography file**

Create `tests/fixtures/citations/refs.bib`:

```bibtex
@article{smith2020,
  author = {Smith, John},
  title = {A Great Paper},
  journal = {Nature},
  year = {2020},
  volume = {42},
  pages = {1--10}
}

@book{jones2021,
  author = {Jones, Alice},
  title = {Some Book},
  publisher = {MIT Press},
  year = {2021}
}
```

- [ ] **Step 2: Create test input markdown**

Create `tests/fixtures/citations/basic.md`:

```markdown
---
title: Citation Test
---

This paper is notable [@smith2020].

Multiple citations [@smith2020; @jones2021].

As @smith2020 argues, this is important.

Year only [-@jones2021].

With detail [see @smith2020, p. 42].

Unknown citation [@nonexistent].
```

- [ ] **Step 3: Write CLI integration test**

In `crates/docmux-cli/tests/golden.rs`, add a new test at the bottom:

```rust
#[test]
fn citation_basic_with_bibliography() {
    let md_path = fixtures_dir().join("citations/basic.md");
    let bib_path = fixtures_dir().join("citations/refs.bib");
    let expected_path = fixtures_dir().join("citations/basic.html");

    let input = std::fs::read_to_string(&md_path).expect("read citation fixture");
    let bib_content = std::fs::read_to_string(&bib_path).expect("read bib fixture");

    // Parse bibliography
    let lib = hayagriva::io::from_biblatex_str(&bib_content).expect("parse bib");

    // Read markdown
    let reader = MarkdownReader::new();
    let mut doc = reader.read(&input).expect("read markdown");

    // Apply cite transform
    use docmux_transform_cite::CiteTransform;
    use docmux_core::TransformContext;

    let transform = CiteTransform::with_library(lib, None);
    transform
        .transform(&mut doc, &TransformContext::default())
        .expect("cite transform");

    // Write HTML
    let writer = HtmlWriter::new();
    let opts = WriteOptions::default();
    let actual = writer.write(&doc, &opts).expect("write html");

    if update_mode() {
        std::fs::write(&expected_path, &actual).expect("write expected");
    } else if expected_path.exists() {
        let expected = std::fs::read_to_string(&expected_path).expect("read expected");
        assert_eq!(
            actual.trim(),
            expected.trim(),
            "citation golden file mismatch: {}",
            expected_path.display()
        );
    } else {
        // Bootstrap: create the expected file
        std::fs::create_dir_all(expected_path.parent().unwrap()).ok();
        std::fs::write(&expected_path, &actual).expect("bootstrap expected");
        eprintln!("bootstrapped: {}", expected_path.display());
    }
}
```

- [ ] **Step 4: Add test deps to CLI Cargo.toml**

In `crates/docmux-cli/Cargo.toml`, add to `[dev-dependencies]`:

```toml
docmux-transform-cite = { workspace = true }
hayagriva = { workspace = true }
```

- [ ] **Step 5: Generate expected output**

Run:
```bash
DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden -- citation_basic
```

Expected: test passes, `tests/fixtures/citations/basic.html` is created.

- [ ] **Step 6: Review the generated HTML**

Read `tests/fixtures/citations/basic.html` and verify:
- Resolved citations appear as text (not raw `[@key]`)
- Unknown `[@nonexistent]` shows as `[?nonexistent]`
- Bibliography appears at the end inside a `<div id="refs">` block
- Each bibliography entry is formatted text

- [ ] **Step 7: Run the test without update mode**

Run:
```bash
cargo test -p docmux-cli --test golden -- citation_basic
```

Expected: PASS — output matches generated expectation.

- [ ] **Step 8: Commit**

```bash
git add tests/fixtures/citations/ crates/docmux-cli/tests/ crates/docmux-cli/Cargo.toml
git commit -m "test: add citation golden file tests and CLI integration"
```

---

### Task 6: WASM Build Verification + Final Checks

**Files:**
- No new files

Verify everything works together: WASM build, full workspace tests, clippy.

- [ ] **Step 1: WASM build**

Run:
```bash
cargo build --target wasm32-unknown-unknown -p docmux-wasm
```

Expected: builds successfully. The cite transform is compiled into WASM. If it fails, feature-gate hayagriva and fix.

- [ ] **Step 2: Full workspace tests**

Run:
```bash
cargo test --workspace
```

Expected: all tests pass (507+ tests plus the new citation tests).

- [ ] **Step 3: Clippy**

Run:
```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 4: Format check**

Run:
```bash
cargo fmt --all -- --check
```

Expected: no formatting issues.

- [ ] **Step 5: Commit any fixes**

If any of the above required fixes, commit them:

```bash
git add -A
git commit -m "fix: address clippy/fmt issues in cite transform"
```

---

### Task 7: Update Documentation

**Files:**
- Modify: `ROADMAP.md`
- Modify: `docs/pandoc-parity-check.md`

Mark cite-related items as done.

- [ ] **Step 1: Update ROADMAP.md**

Change the cite-related Phase 3 items from `- [ ]` to `- [x]`:
- `--bibliography=FILE`, `--csl=FILE`
- `docmux-transform-cite` — CSL citation processing

- [ ] **Step 2: Update pandoc-parity-check.md**

Update the following rows from `MISSING` / `partial` to `ok` or `done`:
- Citation row: update status to `ok`
- CSL metadata fields: update to `ok`
- CLI `--bibliography` and `--csl` rows: update to `ok`
- Markdown reader citations: confirm `ok`

- [ ] **Step 3: Commit**

```bash
git add ROADMAP.md docs/pandoc-parity-check.md
git commit -m "docs: mark cite transform and bibliography CLI as complete"
```
