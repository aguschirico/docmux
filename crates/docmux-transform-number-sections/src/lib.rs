//! # docmux-transform-number-sections
//!
//! Hierarchical section-numbering transform for docmux.
//!
//! Numbers headings hierarchically: h1 → "1", "2", …; h2 under the first h1
//! → "1.1", "1.2", …; h3 → "1.1.1", etc.
//!
//! ## Configuration (via `TransformContext.variables`)
//!
//! | Key                    | Default     | Description                                  |
//! |------------------------|-------------|----------------------------------------------|
//! | `top-level-division`   | `"section"` | `"section"` / `"chapter"` / `"part"`         |
//!
//! - `"section"`: h1 → `1`, `2`, …
//! - `"chapter"`: h1 → `Chapter 1`, `Chapter 2`, …
//! - `"part"`:    h1 → `Part 1`, `Part 2`, …
//!
//! For all modes h2 and below are numbered relative to their parent (e.g. `1.1`).

use docmux_ast::*;
use docmux_core::{Result, Transform, TransformContext};

// ─── Public transform struct ─────────────────────────────────────────────────

/// Section-numbering transform.
#[derive(Debug, Default)]
pub struct NumberSectionsTransform;

impl NumberSectionsTransform {
    pub fn new() -> Self {
        Self
    }
}

impl Transform for NumberSectionsTransform {
    fn name(&self) -> &str {
        "number-sections"
    }

    fn transform(&self, doc: &mut Document, ctx: &TransformContext) -> Result<()> {
        let top_level = ctx
            .variables
            .get("top-level-division")
            .map(|s| s.as_str())
            .unwrap_or("section");

        let mut state = NumberingState::new(top_level);
        number_blocks(&mut doc.content, &mut state);

        Ok(())
    }
}

// ─── Numbering state ─────────────────────────────────────────────────────────

/// Tracks counters at each heading depth (indices 0–5 correspond to h1–h6).
struct NumberingState {
    counters: [u32; 6],
    top_level: TopLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TopLevel {
    Section,
    Chapter,
    Part,
}

impl NumberingState {
    fn new(top_level: &str) -> Self {
        let tl = match top_level {
            "chapter" => TopLevel::Chapter,
            "part" => TopLevel::Part,
            _ => TopLevel::Section,
        };
        Self {
            counters: [0; 6],
            top_level: tl,
        }
    }

    /// Increment the counter for `level` (1-based) and reset all deeper levels.
    fn bump(&mut self, level: u8) {
        let idx = (level - 1) as usize;
        self.counters[idx] += 1;
        // Reset all deeper counters.
        for deeper in self.counters.iter_mut().skip(idx + 1) {
            *deeper = 0;
        }
    }

    /// Build the display prefix for the given level.
    fn prefix(&self, level: u8) -> String {
        let idx = (level - 1) as usize;
        // Build dotted number from level 1 up to `level`.
        // Only include levels that are non-zero (skip leading zeros for gaps).
        let dotted: String = self.counters[..=idx]
            .iter()
            .enumerate()
            .filter_map(|(i, &c)| {
                if c > 0 || i < idx {
                    // Always include all components once we've seen any non-zero.
                    Some(c.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(".");

        match (level, self.top_level) {
            (1, TopLevel::Chapter) => format!("Chapter {}", self.counters[0]),
            (1, TopLevel::Part) => format!("Part {}", self.counters[0]),
            _ => dotted,
        }
    }
}

// ─── Recursive numbering ─────────────────────────────────────────────────────

fn number_blocks(blocks: &mut [Block], state: &mut NumberingState) {
    for block in blocks.iter_mut() {
        number_block(block, state);
    }
}

fn number_block(block: &mut Block, state: &mut NumberingState) {
    match block {
        Block::Heading { level, content, .. } => {
            state.bump(*level);
            let prefix = state.prefix(*level);
            // Prepend "<prefix> " as Text inline.
            content.insert(
                0,
                Inline::Text {
                    value: " ".to_string(),
                },
            );
            content.insert(0, Inline::Text { value: prefix });
        }
        // Recurse into block containers.
        Block::BlockQuote { content } => number_blocks(content, state),
        Block::List { items, .. } => {
            for item in items.iter_mut() {
                number_blocks(&mut item.content, state);
            }
        }
        Block::Admonition { content, .. } => number_blocks(content, state),
        Block::FootnoteDef { content, .. } => number_blocks(content, state),
        Block::Div { content, .. } => number_blocks(content, state),
        _ => {}
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_core::TransformContext;

    fn heading(level: u8, text: &str) -> Block {
        Block::Heading {
            level,
            id: None,
            content: vec![Inline::text(text)],
            attrs: None,
        }
    }

    fn make_ctx(top_level: Option<&str>) -> TransformContext {
        let mut ctx = TransformContext::default();
        if let Some(tl) = top_level {
            ctx.variables
                .insert("top-level-division".to_string(), tl.to_string());
        }
        ctx
    }

    /// Extract the heading content as a flat string (concatenating Text nodes).
    fn heading_text(block: &Block) -> String {
        match block {
            Block::Heading { content, .. } => content
                .iter()
                .map(|i| match i {
                    Inline::Text { value } => value.clone(),
                    _ => String::new(),
                })
                .collect(),
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn basic_h1_h2_h3_numbering() {
        let mut doc = Document {
            content: vec![
                heading(1, "Introduction"),
                heading(2, "Background"),
                heading(3, "History"),
                heading(2, "Motivation"),
                heading(1, "Conclusion"),
                heading(2, "Summary"),
            ],
            ..Default::default()
        };

        NumberSectionsTransform::new()
            .transform(&mut doc, &make_ctx(None))
            .unwrap();

        assert_eq!(heading_text(&doc.content[0]), "1 Introduction");
        assert_eq!(heading_text(&doc.content[1]), "1.1 Background");
        assert_eq!(heading_text(&doc.content[2]), "1.1.1 History");
        assert_eq!(heading_text(&doc.content[3]), "1.2 Motivation");
        assert_eq!(heading_text(&doc.content[4]), "2 Conclusion");
        assert_eq!(heading_text(&doc.content[5]), "2.1 Summary");
    }

    #[test]
    fn multiple_h1s_reset_h2_counters() {
        let mut doc = Document {
            content: vec![
                heading(1, "Alpha"),
                heading(2, "Alpha-1"),
                heading(2, "Alpha-2"),
                heading(1, "Beta"),
                heading(2, "Beta-1"),
            ],
            ..Default::default()
        };

        NumberSectionsTransform::new()
            .transform(&mut doc, &make_ctx(None))
            .unwrap();

        assert_eq!(heading_text(&doc.content[0]), "1 Alpha");
        assert_eq!(heading_text(&doc.content[1]), "1.1 Alpha-1");
        assert_eq!(heading_text(&doc.content[2]), "1.2 Alpha-2");
        assert_eq!(heading_text(&doc.content[3]), "2 Beta");
        assert_eq!(heading_text(&doc.content[4]), "2.1 Beta-1");
    }

    #[test]
    fn gap_in_levels_h1_then_h3() {
        // h1 → h3 with no h2 in between. The counter for h2 stays at 0, so
        // h3 should produce "1.0.1".
        let mut doc = Document {
            content: vec![heading(1, "Top"), heading(3, "Deep")],
            ..Default::default()
        };

        NumberSectionsTransform::new()
            .transform(&mut doc, &make_ctx(None))
            .unwrap();

        assert_eq!(heading_text(&doc.content[0]), "1 Top");
        // gap: h2 counter is 0, so prefix is "1.0.1"
        assert_eq!(heading_text(&doc.content[1]), "1.0.1 Deep");
    }

    #[test]
    fn empty_document_is_noop() {
        let mut doc = Document::default();

        NumberSectionsTransform::new()
            .transform(&mut doc, &make_ctx(None))
            .unwrap();

        assert!(doc.content.is_empty());
    }

    #[test]
    fn chapter_mode_prefixes_h1() {
        let mut doc = Document {
            content: vec![heading(1, "First"), heading(2, "Sub"), heading(1, "Second")],
            ..Default::default()
        };

        NumberSectionsTransform::new()
            .transform(&mut doc, &make_ctx(Some("chapter")))
            .unwrap();

        assert_eq!(heading_text(&doc.content[0]), "Chapter 1 First");
        assert_eq!(heading_text(&doc.content[1]), "1.1 Sub");
        assert_eq!(heading_text(&doc.content[2]), "Chapter 2 Second");
    }

    #[test]
    fn part_mode_prefixes_h1() {
        let mut doc = Document {
            content: vec![heading(1, "PartA"), heading(1, "PartB")],
            ..Default::default()
        };

        NumberSectionsTransform::new()
            .transform(&mut doc, &make_ctx(Some("part")))
            .unwrap();

        assert_eq!(heading_text(&doc.content[0]), "Part 1 PartA");
        assert_eq!(heading_text(&doc.content[1]), "Part 2 PartB");
    }

    #[test]
    fn transform_trait_metadata() {
        let t = NumberSectionsTransform::new();
        assert_eq!(t.name(), "number-sections");
    }
}
