//! Template renderer: Vec<TemplateNode> + TemplateContext → String.

use crate::parser::TemplateNode;
use crate::{TemplateContext, TemplateError};

pub(crate) fn render(
    nodes: &[TemplateNode],
    ctx: &TemplateContext,
) -> Result<String, TemplateError> {
    let mut out = String::new();
    render_nodes(nodes, ctx, &mut out)?;
    Ok(out)
}

fn render_nodes(
    nodes: &[TemplateNode],
    _ctx: &TemplateContext,
    out: &mut String,
) -> Result<(), TemplateError> {
    for node in nodes {
        match node {
            TemplateNode::Literal(text) => out.push_str(text),
            TemplateNode::Variable(_)
            | TemplateNode::Conditional { .. }
            | TemplateNode::Loop { .. } => {
                // Stub — will be implemented in Task 3
            }
        }
    }
    Ok(())
}
