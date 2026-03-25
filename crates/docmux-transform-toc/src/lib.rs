//! # docmux-transform-toc
//!
//! Table of contents generator for docmux.
//!
//! This transform walks the AST, collects all headings that have an `id`,
//! and inserts a `Block::Div { class: "toc" }` containing a nested
//! `Block::List` at position 0 of `doc.content`.
//!
//! ## Configuration (via `TransformContext.variables`)
//!
//! | Key         | Default | Description                              |
//! |-------------|---------|------------------------------------------|
//! | `toc-depth` | `3`     | Maximum heading level to include (1–6). |

use docmux_ast::*;
use docmux_core::{Result, Transform, TransformContext};

// ─── Public transform struct ─────────────────────────────────────────────────

/// Table-of-contents generator.
#[derive(Debug, Default)]
pub struct TocTransform;

impl TocTransform {
    pub fn new() -> Self {
        Self
    }
}

impl Transform for TocTransform {
    fn name(&self) -> &str {
        "toc"
    }

    fn transform(&self, doc: &mut Document, ctx: &TransformContext) -> Result<()> {
        let max_depth: u8 = ctx
            .variables
            .get("toc-depth")
            .and_then(|v| v.parse().ok())
            .unwrap_or(3);

        // Collect headings (level, id, plain-text content) from the AST.
        let entries = collect_headings(&doc.content, max_depth);

        if entries.is_empty() {
            return Ok(());
        }

        // Build the nested list structure and wrap it in a "toc" Div.
        let toc_list = build_toc_list(&entries, 0, entries[0].level);
        let toc_div = Block::Div {
            attrs: Attributes {
                id: None,
                classes: vec!["toc".to_string()],
                key_values: Default::default(),
            },
            content: vec![toc_list],
        };

        doc.content.insert(0, toc_div);

        Ok(())
    }
}

// ─── Heading entry ───────────────────────────────────────────────────────────

/// A heading found during the collection pass.
#[derive(Debug, Clone)]
struct HeadingEntry {
    level: u8,
    id: String,
    /// Plain-text rendering of the heading's inline content.
    text: String,
}

// ─── Pass 1: Collect headings ────────────────────────────────────────────────

fn collect_headings(blocks: &[Block], max_depth: u8) -> Vec<HeadingEntry> {
    let mut entries = Vec::new();
    collect_headings_from_blocks(blocks, max_depth, &mut entries);
    entries
}

fn collect_headings_from_blocks(blocks: &[Block], max_depth: u8, out: &mut Vec<HeadingEntry>) {
    for block in blocks {
        collect_headings_from_block(block, max_depth, out);
    }
}

fn collect_headings_from_block(block: &Block, max_depth: u8, out: &mut Vec<HeadingEntry>) {
    match block {
        Block::Heading {
            level,
            id: Some(id),
            content,
            ..
        } if *level <= max_depth => {
            out.push(HeadingEntry {
                level: *level,
                id: id.clone(),
                text: inlines_to_plain_text(content),
            });
        }
        // Recurse into nested containers (but not into headings themselves).
        Block::BlockQuote { content } => {
            collect_headings_from_blocks(content, max_depth, out);
        }
        Block::List { items, .. } => {
            for item in items {
                collect_headings_from_blocks(&item.content, max_depth, out);
            }
        }
        Block::Admonition { content, .. } => {
            collect_headings_from_blocks(content, max_depth, out);
        }
        Block::FootnoteDef { content, .. } => {
            collect_headings_from_blocks(content, max_depth, out);
        }
        Block::Div { content, .. } => {
            collect_headings_from_blocks(content, max_depth, out);
        }
        _ => {}
    }
}

/// Recursively extract plain text from inline nodes.
fn inlines_to_plain_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    for inline in inlines {
        inline_to_plain_text(inline, &mut out);
    }
    out
}

fn inline_to_plain_text(inline: &Inline, out: &mut String) {
    match inline {
        Inline::Text { value } => out.push_str(value),
        Inline::Code { value, .. } => out.push_str(value),
        Inline::MathInline { value } => out.push_str(value),
        Inline::SoftBreak => out.push(' '),
        Inline::HardBreak => out.push('\n'),
        Inline::Emphasis { content }
        | Inline::Strong { content }
        | Inline::Strikethrough { content }
        | Inline::Underline { content }
        | Inline::Superscript { content }
        | Inline::Subscript { content }
        | Inline::SmallCaps { content }
        | Inline::Link { content, .. }
        | Inline::Span { content, .. }
        | Inline::Quoted { content, .. } => {
            for child in content {
                inline_to_plain_text(child, out);
            }
        }
        _ => {}
    }
}

// ─── Build nested list ───────────────────────────────────────────────────────

/// Recursively build a `Block::List` for the heading entries starting at
/// `start` index. `current_level` is the level being processed at this
/// recursion depth.
///
/// Returns the list block and the number of entries consumed.
fn build_toc_list(entries: &[HeadingEntry], start: usize, current_level: u8) -> Block {
    let mut items: Vec<ListItem> = Vec::new();
    let mut i = start;

    while i < entries.len() {
        let entry = &entries[i];

        if entry.level < current_level {
            // Heading belongs to a parent level — stop.
            break;
        }

        if entry.level == current_level {
            // Build the link for this entry.
            let link = Inline::Link {
                url: format!("#{}", entry.id),
                title: None,
                content: vec![Inline::Text {
                    value: entry.text.clone(),
                }],
                attrs: None,
            };

            i += 1;

            // Check whether the next entry is a child level.
            let mut item_content: Vec<Block> = vec![Block::Paragraph {
                content: vec![link],
            }];

            if i < entries.len() && entries[i].level > current_level {
                let child_level = entries[i].level;
                let child_list = build_toc_list(entries, i, child_level);
                // Count how many entries were consumed for the sub-list.
                let consumed = count_consumed(entries, i, current_level);
                i += consumed;
                item_content.push(child_list);
            }

            items.push(ListItem {
                checked: None,
                content: item_content,
            });
        } else {
            // entry.level > current_level: skip orphan deeper entries
            // (shouldn't happen in well-formed docs, but be defensive).
            i += 1;
        }
    }

    Block::List {
        ordered: false,
        start: None,
        items,
        tight: true,
        style: None,
        delimiter: None,
    }
}

/// Count how many consecutive entries (starting at `start`) have a level
/// strictly greater than `stop_level`. These are the entries that belong to
/// the child sub-list.
fn count_consumed(entries: &[HeadingEntry], start: usize, stop_level: u8) -> usize {
    let mut count = 0;
    let mut i = start;
    while i < entries.len() && entries[i].level > stop_level {
        i += 1;
        count += 1;
    }
    count
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_core::TransformContext;

    fn heading(level: u8, id: &str, text: &str) -> Block {
        Block::Heading {
            level,
            id: Some(id.to_string()),
            content: vec![Inline::text(text)],
            attrs: None,
        }
    }

    fn heading_no_id(level: u8, text: &str) -> Block {
        Block::Heading {
            level,
            id: None,
            content: vec![Inline::text(text)],
            attrs: None,
        }
    }

    fn make_ctx(toc_depth: Option<u8>) -> TransformContext {
        let mut ctx = TransformContext::default();
        if let Some(depth) = toc_depth {
            ctx.variables
                .insert("toc-depth".to_string(), depth.to_string());
        }
        ctx
    }

    // ── Helper: extract list items from the ToC div ──────────────────────────

    fn toc_div(doc: &Document) -> &Block {
        doc.content.first().expect("expected ToC block at index 0")
    }

    fn toc_list(doc: &Document) -> &Block {
        match toc_div(doc) {
            Block::Div { content, .. } => content.first().expect("toc div should have a list"),
            other => panic!("Expected Div, got {:?}", other),
        }
    }

    fn list_items(block: &Block) -> &[ListItem] {
        match block {
            Block::List { items, .. } => items,
            other => panic!("Expected List, got {:?}", other),
        }
    }

    fn item_link_url(item: &ListItem) -> String {
        match item.content.first().expect("item has content") {
            Block::Paragraph { content } => match content.first().expect("paragraph has inline") {
                Inline::Link { url, .. } => url.clone(),
                other => panic!("Expected Link, got {:?}", other),
            },
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    fn item_link_text(item: &ListItem) -> String {
        match item.content.first().expect("item has content") {
            Block::Paragraph { content } => match content.first().expect("paragraph has inline") {
                Inline::Link { content, .. } => match content.first().expect("link has content") {
                    Inline::Text { value } => value.clone(),
                    other => panic!("Expected Text, got {:?}", other),
                },
                other => panic!("Expected Link, got {:?}", other),
            },
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    // ── Tests ────────────────────────────────────────────────────────────────

    #[test]
    fn basic_toc_h1_h2_h3() {
        let mut doc = Document {
            content: vec![
                heading(1, "intro", "Introduction"),
                heading(2, "background", "Background"),
                heading(3, "history", "History"),
                heading(2, "motivation", "Motivation"),
                heading(1, "conclusion", "Conclusion"),
            ],
            ..Default::default()
        };

        TocTransform::new()
            .transform(&mut doc, &make_ctx(None))
            .unwrap();

        // ToC is inserted at position 0; original blocks shift by 1.
        assert_eq!(doc.content.len(), 6);

        let list = toc_list(&doc);
        let items = list_items(list);
        // Two top-level entries: "Introduction" and "Conclusion"
        assert_eq!(items.len(), 2);
        assert_eq!(item_link_text(&items[0]), "Introduction");
        assert_eq!(item_link_url(&items[0]), "#intro");
        assert_eq!(item_link_text(&items[1]), "Conclusion");
        assert_eq!(item_link_url(&items[1]), "#conclusion");

        // "Introduction" item should have a nested sub-list.
        let sub = items[0].content.get(1).expect("nested list");
        let sub_items = list_items(sub);
        assert_eq!(sub_items.len(), 2); // Background + Motivation
        assert_eq!(item_link_text(&sub_items[0]), "Background");
        assert_eq!(item_link_text(&sub_items[1]), "Motivation");
    }

    #[test]
    fn respects_toc_depth() {
        let mut doc = Document {
            content: vec![
                heading(1, "intro", "Introduction"),
                heading(2, "background", "Background"),
                heading(3, "history", "History"),
            ],
            ..Default::default()
        };

        // depth=1 → only h1 entries
        TocTransform::new()
            .transform(&mut doc, &make_ctx(Some(1)))
            .unwrap();

        let list = toc_list(&doc);
        let items = list_items(list);
        assert_eq!(items.len(), 1);
        assert_eq!(item_link_text(&items[0]), "Introduction");
        // No sub-list because depth=1 excludes h2/h3
        assert_eq!(items[0].content.len(), 1);
    }

    #[test]
    fn headings_without_ids_are_skipped() {
        let mut doc = Document {
            content: vec![
                heading(1, "intro", "Introduction"),
                heading_no_id(2, "No ID heading"),
                heading(2, "methods", "Methods"),
            ],
            ..Default::default()
        };

        TocTransform::new()
            .transform(&mut doc, &make_ctx(None))
            .unwrap();

        let list = toc_list(&doc);
        let items = list_items(list);
        // Only one top-level: Introduction; Methods is a child.
        assert_eq!(items.len(), 1);
        let sub = items[0].content.get(1).expect("sub-list");
        let sub_items = list_items(sub);
        // "No ID heading" skipped; only "Methods" in sub-list.
        assert_eq!(sub_items.len(), 1);
        assert_eq!(item_link_text(&sub_items[0]), "Methods");
    }

    #[test]
    fn empty_document_produces_no_toc() {
        let mut doc = Document::default();

        TocTransform::new()
            .transform(&mut doc, &make_ctx(None))
            .unwrap();

        assert!(doc.content.is_empty());
    }

    #[test]
    fn nested_headings_produce_nested_lists() {
        let mut doc = Document {
            content: vec![
                heading(1, "a", "A"),
                heading(2, "a1", "A1"),
                heading(3, "a1i", "A1i"),
                heading(2, "a2", "A2"),
                heading(1, "b", "B"),
            ],
            ..Default::default()
        };

        TocTransform::new()
            .transform(&mut doc, &make_ctx(None))
            .unwrap();

        let list = toc_list(&doc);
        let top = list_items(list);
        assert_eq!(top.len(), 2); // A and B

        // A has sub-list with A1 and A2
        let a_sub = top[0].content.get(1).expect("A sub-list");
        let a_items = list_items(a_sub);
        assert_eq!(a_items.len(), 2);
        assert_eq!(item_link_text(&a_items[0]), "A1");
        assert_eq!(item_link_text(&a_items[1]), "A2");

        // A1 has sub-list with A1i
        let a1_sub = a_items[0].content.get(1).expect("A1 sub-list");
        let a1_items = list_items(a1_sub);
        assert_eq!(a1_items.len(), 1);
        assert_eq!(item_link_text(&a1_items[0]), "A1i");

        // B has no sub-list
        assert_eq!(top[1].content.len(), 1);
    }

    #[test]
    fn transform_trait_metadata() {
        let t = TocTransform::new();
        assert_eq!(t.name(), "toc");
    }
}
