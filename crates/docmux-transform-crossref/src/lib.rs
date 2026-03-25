//! # docmux-transform-crossref
//!
//! Cross-reference resolver for docmux. This transform performs two passes:
//!
//! 1. **Collect**: walk the AST and build a map of `label → (LabelKind, number)`,
//!    numbering figures, tables, equations, code blocks, and sections sequentially.
//! 2. **Resolve**: walk the AST again and replace every `Inline::CrossRef` node
//!    with rendered text (e.g. "Figure 3", "Eq. 1", "Table 2").

use docmux_ast::*;
use docmux_core::{Result, Transform, TransformContext};
use std::collections::HashMap;

/// A resolved label: its kind and sequential number.
#[derive(Debug, Clone)]
struct ResolvedLabel {
    kind: LabelKind,
    number: u32,
}

/// Cross-reference transform.
#[derive(Debug, Default)]
pub struct CrossRefTransform;

impl CrossRefTransform {
    pub fn new() -> Self {
        Self
    }
}

impl Transform for CrossRefTransform {
    fn name(&self) -> &str {
        "crossref"
    }

    fn transform(&self, doc: &mut Document, _ctx: &TransformContext) -> Result<()> {
        // Pass 1: collect all labels and assign numbers
        let labels = collect_labels(&doc.content);

        // Pass 2: resolve CrossRef nodes in-place
        resolve_blocks(&mut doc.content, &labels);

        Ok(())
    }
}

// ─── Pass 1: Collect labels ─────────────────────────────────────────────────

/// Counters for each label kind, incremented as labels are found.
#[derive(Debug, Default)]
struct Counters {
    figure: u32,
    table: u32,
    equation: u32,
    code: u32,
    section: u32,
}

/// Walk the AST and build a map from label id → resolved info.
fn collect_labels(blocks: &[Block]) -> HashMap<String, ResolvedLabel> {
    let mut map = HashMap::new();
    let mut counters = Counters::default();
    collect_labels_from_blocks(blocks, &mut map, &mut counters);
    map
}

fn collect_labels_from_blocks(
    blocks: &[Block],
    map: &mut HashMap<String, ResolvedLabel>,
    counters: &mut Counters,
) {
    for block in blocks {
        collect_labels_from_block(block, map, counters);
    }
}

fn collect_labels_from_block(
    block: &Block,
    map: &mut HashMap<String, ResolvedLabel>,
    counters: &mut Counters,
) {
    match block {
        Block::Figure {
            label: Some(label), ..
        } => {
            counters.figure += 1;
            map.insert(
                label.clone(),
                ResolvedLabel {
                    kind: LabelKind::Figure,
                    number: counters.figure,
                },
            );
        }
        Block::Table(table) => {
            if let Some(label) = &table.label {
                counters.table += 1;
                map.insert(
                    label.clone(),
                    ResolvedLabel {
                        kind: LabelKind::Table,
                        number: counters.table,
                    },
                );
            }
        }
        Block::MathBlock {
            label: Some(label), ..
        } => {
            counters.equation += 1;
            map.insert(
                label.clone(),
                ResolvedLabel {
                    kind: LabelKind::Equation,
                    number: counters.equation,
                },
            );
        }
        Block::CodeBlock {
            label: Some(label), ..
        } => {
            counters.code += 1;
            map.insert(
                label.clone(),
                ResolvedLabel {
                    kind: LabelKind::Code,
                    number: counters.code,
                },
            );
        }
        Block::Heading { id: Some(id), .. } => {
            counters.section += 1;
            map.insert(
                id.clone(),
                ResolvedLabel {
                    kind: LabelKind::Section,
                    number: counters.section,
                },
            );
        }
        // Recurse into nested block containers
        Block::BlockQuote { content } => {
            collect_labels_from_blocks(content, map, counters);
        }
        Block::List { items, .. } => {
            for item in items {
                collect_labels_from_blocks(&item.content, map, counters);
            }
        }
        Block::Admonition { content, .. } => {
            collect_labels_from_blocks(content, map, counters);
        }
        Block::FootnoteDef { content, .. } => {
            collect_labels_from_blocks(content, map, counters);
        }
        Block::Div { content, .. } => {
            collect_labels_from_blocks(content, map, counters);
        }
        _ => {}
    }
}

// ─── Pass 2: Resolve CrossRef nodes ─────────────────────────────────────────

fn resolve_blocks(blocks: &mut [Block], labels: &HashMap<String, ResolvedLabel>) {
    for block in blocks.iter_mut() {
        resolve_block(block, labels);
    }
}

fn resolve_block(block: &mut Block, labels: &HashMap<String, ResolvedLabel>) {
    match block {
        Block::Paragraph { content } => resolve_inlines(content, labels),
        Block::Heading { content, .. } => resolve_inlines(content, labels),
        Block::BlockQuote { content } => resolve_blocks(content, labels),
        Block::List { items, .. } => {
            for item in items {
                resolve_blocks(&mut item.content, labels);
            }
        }
        Block::Admonition { content, title, .. } => {
            resolve_blocks(content, labels);
            if let Some(title) = title {
                resolve_inlines(title, labels);
            }
        }
        Block::FootnoteDef { content, .. } => resolve_blocks(content, labels),
        Block::Div { content, .. } => resolve_blocks(content, labels),
        Block::Figure {
            caption: Some(cap), ..
        } => {
            resolve_inlines(cap, labels);
        }
        Block::Table(table) => {
            if let Some(cap) = &mut table.caption {
                resolve_inlines(cap, labels);
            }
        }
        _ => {}
    }
}

fn resolve_inlines(inlines: &mut [Inline], labels: &HashMap<String, ResolvedLabel>) {
    for inline in inlines.iter_mut() {
        resolve_inline(inline, labels);
    }
}

fn resolve_inline(inline: &mut Inline, labels: &HashMap<String, ResolvedLabel>) {
    match inline {
        Inline::CrossRef(cr) => {
            if let Some(resolved) = labels.get(&cr.target) {
                let text = render_crossref(cr, resolved);
                *inline = Inline::Text { value: text };
            }
            // If unresolved, leave the CrossRef as-is — writers will
            // handle it (e.g. LaTeX writer emits \ref{} which LaTeX resolves)
        }
        // Recurse into inline containers
        Inline::Emphasis { content }
        | Inline::Strong { content }
        | Inline::Strikethrough { content }
        | Inline::Superscript { content }
        | Inline::Subscript { content }
        | Inline::SmallCaps { content }
        | Inline::Underline { content }
        | Inline::Span { content, .. }
        | Inline::Link { content, .. } => {
            resolve_inlines(content, labels);
        }
        _ => {}
    }
}

/// Render a cross-reference to text based on its form.
fn render_crossref(cr: &CrossRef, resolved: &ResolvedLabel) -> String {
    match &cr.form {
        RefForm::Number => {
            format!("{}", resolved.number)
        }
        RefForm::NumberWithType => {
            let type_name = label_kind_name(&resolved.kind);
            format!("{} {}", type_name, resolved.number)
        }
        RefForm::Page => {
            // Page references only make sense in paged media (PDF/LaTeX).
            // In non-paged output we fall back to the number.
            format!("{}", resolved.number)
        }
        RefForm::Custom(text) => {
            format!("{} {}", text, resolved.number)
        }
    }
}

/// Human-readable name for a label kind.
fn label_kind_name(kind: &LabelKind) -> &'static str {
    match kind {
        LabelKind::Figure => "Figure",
        LabelKind::Table => "Table",
        LabelKind::Equation => "Equation",
        LabelKind::Section => "Section",
        LabelKind::Code => "Listing",
        LabelKind::Custom(_) => "Item",
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_core::TransformContext;

    #[test]
    fn numbers_figures_sequentially() {
        let mut doc = Document {
            content: vec![
                Block::Figure {
                    image: Image {
                        url: "a.png".into(),
                        alt: "A".into(),
                        title: None,
                    },
                    caption: Some(vec![Inline::text("First")]),
                    label: Some("fig:a".into()),
                    attrs: None,
                },
                Block::Figure {
                    image: Image {
                        url: "b.png".into(),
                        alt: "B".into(),
                        title: None,
                    },
                    caption: Some(vec![Inline::text("Second")]),
                    label: Some("fig:b".into()),
                    attrs: None,
                },
                Block::Paragraph {
                    content: vec![
                        Inline::text("See "),
                        Inline::CrossRef(CrossRef {
                            target: "fig:a".into(),
                            form: RefForm::NumberWithType,
                        }),
                        Inline::text(" and "),
                        Inline::CrossRef(CrossRef {
                            target: "fig:b".into(),
                            form: RefForm::Number,
                        }),
                        Inline::text("."),
                    ],
                },
            ],
            ..Default::default()
        };

        let transform = CrossRefTransform::new();
        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        // Check the paragraph's resolved text
        if let Block::Paragraph { content } = &doc.content[2] {
            assert_eq!(content.len(), 5);
            match &content[1] {
                Inline::Text { value } => assert_eq!(value, "Figure 1"),
                other => panic!("Expected resolved Text, got {:?}", other),
            }
            match &content[3] {
                Inline::Text { value } => assert_eq!(value, "2"),
                other => panic!("Expected resolved Text, got {:?}", other),
            }
        } else {
            panic!("Expected Paragraph");
        }
    }

    #[test]
    fn numbers_equations_and_tables() {
        let mut doc = Document {
            content: vec![
                Block::MathBlock {
                    content: "E = mc^2".into(),
                    label: Some("eq:einstein".into()),
                },
                Block::Table(Table {
                    caption: None,
                    label: Some("tab:results".into()),
                    columns: vec![],
                    header: None,
                    rows: vec![],
                    attrs: None,
                }),
                Block::MathBlock {
                    content: "F = ma".into(),
                    label: Some("eq:newton".into()),
                },
                Block::Paragraph {
                    content: vec![
                        Inline::CrossRef(CrossRef {
                            target: "eq:einstein".into(),
                            form: RefForm::NumberWithType,
                        }),
                        Inline::text(", "),
                        Inline::CrossRef(CrossRef {
                            target: "tab:results".into(),
                            form: RefForm::NumberWithType,
                        }),
                        Inline::text(", "),
                        Inline::CrossRef(CrossRef {
                            target: "eq:newton".into(),
                            form: RefForm::NumberWithType,
                        }),
                    ],
                },
            ],
            ..Default::default()
        };

        let transform = CrossRefTransform::new();
        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        if let Block::Paragraph { content } = &doc.content[3] {
            match &content[0] {
                Inline::Text { value } => assert_eq!(value, "Equation 1"),
                other => panic!("Expected 'Equation 1', got {:?}", other),
            }
            match &content[2] {
                Inline::Text { value } => assert_eq!(value, "Table 1"),
                other => panic!("Expected 'Table 1', got {:?}", other),
            }
            match &content[4] {
                Inline::Text { value } => assert_eq!(value, "Equation 2"),
                other => panic!("Expected 'Equation 2', got {:?}", other),
            }
        } else {
            panic!("Expected Paragraph");
        }
    }

    #[test]
    fn unresolved_crossref_left_intact() {
        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::CrossRef(CrossRef {
                    target: "nonexistent".into(),
                    form: RefForm::Number,
                })],
            }],
            ..Default::default()
        };

        let transform = CrossRefTransform::new();
        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        // CrossRef should remain since "nonexistent" has no label
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[0], Inline::CrossRef(_)));
        }
    }

    #[test]
    fn custom_ref_form() {
        let mut doc = Document {
            content: vec![
                Block::Figure {
                    image: Image {
                        url: "x.png".into(),
                        alt: "X".into(),
                        title: None,
                    },
                    caption: None,
                    label: Some("fig:x".into()),
                    attrs: None,
                },
                Block::Paragraph {
                    content: vec![Inline::CrossRef(CrossRef {
                        target: "fig:x".into(),
                        form: RefForm::Custom("Fig.".into()),
                    })],
                },
            ],
            ..Default::default()
        };

        let transform = CrossRefTransform::new();
        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        if let Block::Paragraph { content } = &doc.content[1] {
            match &content[0] {
                Inline::Text { value } => assert_eq!(value, "Fig. 1"),
                other => panic!("Expected 'Fig. 1', got {:?}", other),
            }
        }
    }

    #[test]
    fn resolves_inside_nested_containers() {
        let mut doc = Document {
            content: vec![
                Block::MathBlock {
                    content: "x = 1".into(),
                    label: Some("eq:x".into()),
                },
                Block::BlockQuote {
                    content: vec![Block::Paragraph {
                        content: vec![
                            Inline::text("As shown in "),
                            Inline::CrossRef(CrossRef {
                                target: "eq:x".into(),
                                form: RefForm::NumberWithType,
                            }),
                        ],
                    }],
                },
            ],
            ..Default::default()
        };

        let transform = CrossRefTransform::new();
        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        if let Block::BlockQuote { content } = &doc.content[1] {
            if let Block::Paragraph { content } = &content[0] {
                match &content[1] {
                    Inline::Text { value } => assert_eq!(value, "Equation 1"),
                    other => panic!("Expected 'Equation 1', got {:?}", other),
                }
            }
        }
    }

    #[test]
    fn sections_numbered() {
        let mut doc = Document {
            content: vec![
                Block::Heading {
                    level: 1,
                    id: Some("sec:intro".into()),
                    content: vec![Inline::text("Introduction")],
                    attrs: None,
                },
                Block::Heading {
                    level: 2,
                    id: Some("sec:methods".into()),
                    content: vec![Inline::text("Methods")],
                    attrs: None,
                },
                Block::Paragraph {
                    content: vec![Inline::CrossRef(CrossRef {
                        target: "sec:methods".into(),
                        form: RefForm::NumberWithType,
                    })],
                },
            ],
            ..Default::default()
        };

        let transform = CrossRefTransform::new();
        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        if let Block::Paragraph { content } = &doc.content[2] {
            match &content[0] {
                Inline::Text { value } => assert_eq!(value, "Section 2"),
                other => panic!("Expected 'Section 2', got {:?}", other),
            }
        }
    }

    #[test]
    fn transform_trait_metadata() {
        let t = CrossRefTransform::new();
        assert_eq!(t.name(), "crossref");
    }
}
