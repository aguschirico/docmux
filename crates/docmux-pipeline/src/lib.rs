//! # docmux-pipeline
//!
//! The transforms every conversion needs between reading and writing, in one
//! place so the CLI and the WASM build produce the same output:
//!
//! 1. citations resolved against the document's own references list (from a
//!    LaTeX `.bbl`), except for LaTeX output, which keeps `\cite`;
//! 2. cross-references resolved to numbered links, except for LaTeX and Typst
//!    output, which number and resolve them natively;
//! 3. math converted to the notation the writer expects (Typst math for
//!    Typst, LaTeX math for everything else, or MathML on request).

use docmux_ast::Document;
use docmux_core::{Result, Transform, TransformContext};
use docmux_transform_crossref::CrossRefTransform;
pub use docmux_transform_math::MathTarget;
use docmux_transform_math::{MathNotation, MathTransform};
use docmux_transform_references::ReferencesTransform;

/// Formats that resolve cross-references and citations themselves.
fn is_latex(format: &str) -> bool {
    matches!(format, "latex" | "tex")
}

fn is_typst(format: &str) -> bool {
    matches!(format, "typst" | "typ")
}

/// Run the standard transforms for a conversion from `from` to `to`.
/// `math` overrides the math target (e.g. MathML for HTML).
pub fn prepare(doc: &mut Document, from: &str, to: &str, math: Option<MathTarget>) -> Result<()> {
    let ctx = TransformContext::default();
    if !is_latex(to) {
        ReferencesTransform::new().transform(doc, &ctx)?;
    }
    if !is_latex(to) && !is_typst(to) {
        CrossRefTransform::new().transform(doc, &ctx)?;
    }
    let source_notation = if is_typst(from) {
        MathNotation::Typst
    } else {
        MathNotation::LaTeX
    };
    let target_format = math.unwrap_or(if is_typst(to) {
        MathTarget::Typst
    } else {
        MathTarget::LaTeX
    });
    MathTransform {
        target_format,
        source_notation,
    }
    .transform(doc, &ctx)
}

/// True when the writer numbers headings itself, so a numbering transform
/// would print every number twice.
pub fn writer_numbers_sections(to: &str) -> bool {
    is_latex(to) || is_typst(to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_ast::{Block, CrossRef, Inline, RefForm};

    fn doc_with_math_and_ref() -> Document {
        Document {
            content: vec![
                Block::Heading {
                    level: 1,
                    id: Some("sec:a".into()),
                    content: vec![Inline::text("A")],
                    attrs: None,
                },
                Block::Paragraph {
                    content: vec![
                        Inline::MathInline {
                            value: r"\alpha".into(),
                        },
                        Inline::CrossRef(CrossRef {
                            target: "sec:a".into(),
                            form: RefForm::Number,
                        }),
                    ],
                },
            ],
            ..Default::default()
        }
    }

    fn paragraph(doc: &Document) -> &[Inline] {
        match &doc.content[1] {
            Block::Paragraph { content } => content,
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn typst_output_gets_typst_math_and_native_refs() {
        let mut doc = doc_with_math_and_ref();
        prepare(&mut doc, "latex", "typst", None).unwrap();
        let content = paragraph(&doc);
        assert!(matches!(&content[0], Inline::MathInline { value } if value == "alpha"));
        assert!(matches!(content[1], Inline::CrossRef(_)));
    }

    #[test]
    fn html_output_gets_latex_math_and_resolved_refs() {
        let mut doc = doc_with_math_and_ref();
        prepare(&mut doc, "latex", "html", None).unwrap();
        let content = paragraph(&doc);
        assert!(matches!(&content[0], Inline::MathInline { value } if value == r"\alpha"));
        assert!(matches!(&content[1], Inline::Link { url, .. } if url == "#sec:a"));
    }

    #[test]
    fn typst_source_math_becomes_latex_for_html() {
        let mut doc = doc_with_math_and_ref();
        if let Block::Paragraph { content } = &mut doc.content[1] {
            content[0] = Inline::MathInline {
                value: "alpha".into(),
            };
        }
        prepare(&mut doc, "typst", "html", None).unwrap();
        assert!(matches!(&paragraph(&doc)[0], Inline::MathInline { value } if value == r"\alpha"));
    }
}
