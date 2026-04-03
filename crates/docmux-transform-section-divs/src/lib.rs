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

            if let Some(ref mut section) = current_section {
                if level > current_level {
                    // This is a deeper heading — it goes into the current section
                    section.1.push(block);
                    continue;
                }
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
        assert_eq!(
            result.len(),
            1,
            "Should be wrapped in one Div. Got: {result:#?}"
        );
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
        assert_eq!(
            result.len(),
            1,
            "Outer h1 wraps everything. Got: {result:#?}"
        );
        match &result[0] {
            Block::Div { content, .. } => {
                // heading + para + nested div
                assert_eq!(
                    content.len(),
                    3,
                    "Expected heading + para + nested div. Got: {content:#?}"
                );
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
        assert_eq!(
            result.len(),
            2,
            "Preamble stays unwrapped. Got: {result:#?}"
        );
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
            Block::Div { content, .. } => match &content[0] {
                Block::Heading { id, .. } => {
                    assert!(id.is_none(), "Heading ID should be moved to Div");
                }
                other => panic!("Expected Heading, got {:?}", other),
            },
            other => panic!("Expected Div, got {:?}", other),
        }
    }

    #[test]
    fn no_headings_passthrough() {
        let blocks = vec![Block::text("Just text."), Block::ThematicBreak];
        let result = apply(blocks);
        assert_eq!(result.len(), 2, "No headings → no wrapping");
    }
}
