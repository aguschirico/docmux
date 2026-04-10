//! # docmux-transform-math
//!
//! AST transform that converts math notation between LaTeX, Typst, and MathML.
//! Walks the entire document tree, rewriting `MathBlock` and `MathInline`
//! nodes according to the chosen source notation and target format.

pub mod latex_to_mathml;
pub mod latex_to_typst;
pub mod tables;
pub mod tokenizer;
pub mod typst_to_latex;

use docmux_ast::{Block, Document, Inline};
use docmux_core::{Result, Transform, TransformContext};

// ─── Public types ───────────────────────────────────────────────────────────

/// The target math format to convert to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathTarget {
    /// Convert to Typst math notation.
    Typst,
    /// Convert to LaTeX math notation.
    LaTeX,
    /// Convert to MathML markup.
    MathML,
    /// No conversion (no-op).
    None,
}

/// The notation used in the source document's math nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathNotation {
    /// Source math is written in LaTeX notation.
    LaTeX,
    /// Source math is written in Typst notation.
    Typst,
}

/// An AST transform that rewrites math content from one notation to another.
pub struct MathTransform {
    /// The target format to convert math into.
    pub target_format: MathTarget,
    /// The notation used in the source document.
    pub source_notation: MathNotation,
}

// ─── Transform trait ────────────────────────────────────────────────────────

impl Transform for MathTransform {
    fn name(&self) -> &str {
        "math"
    }

    fn transform(&self, doc: &mut Document, _ctx: &TransformContext) -> Result<()> {
        if self.is_noop() {
            return Ok(());
        }
        transform_blocks(&mut doc.content, self.source_notation, self.target_format);
        Ok(())
    }
}

impl MathTransform {
    /// Returns true when no conversion is needed.
    fn is_noop(&self) -> bool {
        if self.target_format == MathTarget::None {
            return true;
        }
        matches!(
            (self.source_notation, self.target_format),
            (MathNotation::LaTeX, MathTarget::LaTeX) | (MathNotation::Typst, MathTarget::Typst)
        )
    }
}

// ─── Conversion dispatch ────────────────────────────────────────────────────

/// Convert a math string from source notation to the target format.
/// `display` indicates whether this is display math (true) or inline (false),
/// which only matters for MathML wrapping.
fn convert_math(input: &str, source: MathNotation, target: MathTarget, display: bool) -> String {
    match (source, target) {
        (MathNotation::LaTeX, MathTarget::Typst) => latex_to_typst::latex_to_typst(input),
        (MathNotation::Typst, MathTarget::LaTeX) => typst_to_latex::typst_to_latex(input),
        (MathNotation::LaTeX, MathTarget::MathML) => latex_to_mathml::wrap_mathml(input, display),
        (MathNotation::Typst, MathTarget::MathML) => {
            let latex = typst_to_latex::typst_to_latex(input);
            latex_to_mathml::wrap_mathml(&latex, display)
        }
        // Same notation or None — no-op (caller should check is_noop first).
        _ => input.to_string(),
    }
}

// ─── AST walkers ────────────────────────────────────────────────────────────

/// Walk a list of blocks, rewriting math content in place.
fn transform_blocks(blocks: &mut [Block], source: MathNotation, target: MathTarget) {
    for block in blocks.iter_mut() {
        transform_block(block, source, target);
    }
}

/// Walk a single block, rewriting math content and recursing into children.
fn transform_block(block: &mut Block, source: MathNotation, target: MathTarget) {
    match block {
        Block::MathBlock { content, .. } => {
            *content = convert_math(content, source, target, true);
        }
        Block::Paragraph { content } | Block::Heading { content, .. } => {
            transform_inlines(content, source, target);
        }
        Block::BlockQuote { content }
        | Block::Div { content, .. }
        | Block::Admonition { content, .. }
        | Block::FootnoteDef { content, .. } => {
            transform_blocks(content, source, target);
        }
        Block::List { items, .. } => {
            for item in items {
                transform_blocks(&mut item.content, source, target);
            }
        }
        Block::Figure { caption, .. } => {
            if let Some(cap) = caption {
                transform_inlines(cap, source, target);
            }
        }
        Block::Table(table) => {
            if let Some(cap) = &mut table.caption {
                transform_inlines(cap, source, target);
            }
            transform_table_cells(&mut table.rows, source, target);
            if let Some(header) = &mut table.header {
                for cell in header.iter_mut() {
                    transform_blocks(&mut cell.content, source, target);
                }
            }
            if let Some(foot) = &mut table.foot {
                for cell in foot.iter_mut() {
                    transform_blocks(&mut cell.content, source, target);
                }
            }
        }
        Block::DefinitionList { items } => {
            for item in items {
                transform_inlines(&mut item.term, source, target);
                for def in &mut item.definitions {
                    transform_blocks(def, source, target);
                }
            }
        }
        Block::CodeBlock { .. } | Block::ThematicBreak | Block::RawBlock { .. } => {}
    }
}

/// Walk table body rows.
fn transform_table_cells(
    rows: &mut [Vec<docmux_ast::TableCell>],
    source: MathNotation,
    target: MathTarget,
) {
    for row in rows.iter_mut() {
        for cell in row.iter_mut() {
            transform_blocks(&mut cell.content, source, target);
        }
    }
}

/// Walk a list of inlines, rewriting math content in place.
fn transform_inlines(inlines: &mut [Inline], source: MathNotation, target: MathTarget) {
    for inline in inlines.iter_mut() {
        transform_inline(inline, source, target);
    }
}

/// Walk a single inline, rewriting math content and recursing into children.
fn transform_inline(inline: &mut Inline, source: MathNotation, target: MathTarget) {
    match inline {
        Inline::MathInline { value } => {
            *value = convert_math(value, source, target, false);
        }
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
            transform_inlines(content, source, target);
        }
        Inline::Text { .. }
        | Inline::Code { .. }
        | Inline::Image(_)
        | Inline::Citation(_)
        | Inline::FootnoteRef { .. }
        | Inline::CrossRef(_)
        | Inline::RawInline { .. }
        | Inline::SoftBreak
        | Inline::HardBreak => {}
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_ast::*;

    fn make_doc_with_math(inline_val: &str, block_content: &str) -> Document {
        Document {
            content: vec![
                Block::Paragraph {
                    content: vec![Inline::MathInline {
                        value: inline_val.to_string(),
                    }],
                },
                Block::MathBlock {
                    content: block_content.to_string(),
                    label: None,
                },
            ],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
            bibliography: None,
        }
    }

    #[test]
    fn latex_to_typst_transform() {
        let mut doc = make_doc_with_math(r"\alpha", r"\frac{a}{b}");
        let t = MathTransform {
            target_format: MathTarget::Typst,
            source_notation: MathNotation::LaTeX,
        };
        t.transform(&mut doc, &TransformContext::default()).unwrap();

        match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::MathInline { value } => assert_eq!(value, "alpha"),
                other => panic!("expected MathInline, got {other:?}"),
            },
            other => panic!("expected Paragraph, got {other:?}"),
        }
        match &doc.content[1] {
            Block::MathBlock { content, .. } => assert_eq!(content, "(a)/(b)"),
            other => panic!("expected MathBlock, got {other:?}"),
        }
    }

    #[test]
    fn latex_to_mathml_transform() {
        let mut doc = make_doc_with_math("x", r"\frac{a}{b}");
        let t = MathTransform {
            target_format: MathTarget::MathML,
            source_notation: MathNotation::LaTeX,
        };
        t.transform(&mut doc, &TransformContext::default()).unwrap();

        match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::MathInline { value } => {
                    assert!(value.contains("<math display=\"inline\">"));
                    assert!(value.contains("<mi>x</mi>"));
                }
                other => panic!("expected MathInline, got {other:?}"),
            },
            other => panic!("expected Paragraph, got {other:?}"),
        }
        match &doc.content[1] {
            Block::MathBlock { content, .. } => {
                assert!(content.contains("<math display=\"block\">"));
                assert!(content.contains("<mfrac>"));
            }
            other => panic!("expected MathBlock, got {other:?}"),
        }
    }

    #[test]
    fn noop_when_same_notation() {
        let mut doc = make_doc_with_math(r"\alpha", r"\frac{a}{b}");
        let t = MathTransform {
            target_format: MathTarget::None,
            source_notation: MathNotation::LaTeX,
        };
        t.transform(&mut doc, &TransformContext::default()).unwrap();

        match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::MathInline { value } => assert_eq!(value, r"\alpha"),
                other => panic!("expected MathInline, got {other:?}"),
            },
            other => panic!("expected Paragraph, got {other:?}"),
        }
    }
}
