//! Template renderer: Vec<TemplateNode> + TemplateContext → String.

use crate::parser::{TemplateNode, VarPath};
use crate::{TemplateContext, TemplateError, TemplateValue};

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
    ctx: &TemplateContext,
    out: &mut String,
) -> Result<(), TemplateError> {
    for node in nodes {
        render_node(node, ctx, out)?;
    }
    Ok(())
}

fn render_node(
    node: &TemplateNode,
    ctx: &TemplateContext,
    out: &mut String,
) -> Result<(), TemplateError> {
    match node {
        TemplateNode::Literal(text) => {
            out.push_str(text);
        }
        TemplateNode::Variable(path) => {
            if let Some(val) = resolve(path, ctx) {
                out.push_str(&val.to_output_string());
            }
        }
        TemplateNode::Conditional {
            var,
            if_body,
            else_body,
        } => {
            let truthy = resolve(var, ctx).is_some_and(|v| v.is_truthy());
            if truthy {
                render_nodes(if_body, ctx, out)?;
            } else {
                render_nodes(else_body, ctx, out)?;
            }
        }
        TemplateNode::Loop {
            var,
            body,
            separator,
        } => {
            render_loop(var, body, separator, ctx, out)?;
        }
    }
    Ok(())
}

fn render_loop(
    var: &VarPath,
    body: &[TemplateNode],
    separator: &[TemplateNode],
    ctx: &TemplateContext,
    out: &mut String,
) -> Result<(), TemplateError> {
    let binding_name = var.last().expect("var path is non-empty");

    let Some(val) = resolve(var, ctx) else {
        return Ok(()); // missing → no output
    };

    let items: Vec<&TemplateValue> = match val {
        TemplateValue::List(list) => list.iter().collect(),
        other => vec![other],
    };

    for (i, item) in items.iter().enumerate() {
        // Build a child context with the loop variable bound to the current item
        let mut child_ctx = ctx.clone();
        child_ctx.insert(binding_name.clone(), (*item).clone());

        if i > 0 && !separator.is_empty() {
            render_nodes(separator, &child_ctx, out)?;
        }
        render_nodes(body, &child_ctx, out)?;
    }

    Ok(())
}

/// Resolve a dotted variable path against a context.
fn resolve<'a>(path: &VarPath, ctx: &'a TemplateContext) -> Option<&'a TemplateValue> {
    let mut parts = path.iter();
    let first = parts.next()?;
    let mut current = ctx.get(first)?;

    for part in parts {
        match current {
            TemplateValue::Map(map) => {
                current = map.get(part)?;
            }
            _ => return None,
        }
    }

    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse as parse_nodes;
    use crate::{TemplateContext, TemplateValue};
    use std::collections::HashMap;

    fn ctx_with(pairs: &[(&str, TemplateValue)]) -> TemplateContext {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect()
    }

    fn str_val(s: &str) -> TemplateValue {
        TemplateValue::Str(s.into())
    }

    fn render_ok(template: &str, ctx: &TemplateContext) -> String {
        let nodes = parse_nodes(template).unwrap();
        render(&nodes, ctx).unwrap()
    }

    #[test]
    fn variable_substitution() {
        let ctx = ctx_with(&[("title", str_val("My Doc"))]);
        assert_eq!(render_ok("Title: $title$", &ctx), "Title: My Doc");
    }

    #[test]
    fn missing_variable_empty() {
        let ctx = TemplateContext::new();
        assert_eq!(render_ok("[$missing$]", &ctx), "[]");
    }

    #[test]
    fn dot_access_into_map() {
        let mut author = HashMap::new();
        author.insert("name".into(), TemplateValue::Str("Alice".into()));
        let ctx = ctx_with(&[("author", TemplateValue::Map(author))]);
        assert_eq!(render_ok("By $author.name$", &ctx), "By Alice");
    }

    #[test]
    fn conditional_true() {
        let ctx = ctx_with(&[("title", str_val("Hello"))]);
        assert_eq!(
            render_ok("$if(title)$<h1>$title$</h1>$endif$", &ctx),
            "<h1>Hello</h1>"
        );
    }

    #[test]
    fn conditional_false() {
        let ctx = TemplateContext::new();
        assert_eq!(render_ok("$if(title)$<h1>$title$</h1>$endif$", &ctx), "");
    }

    #[test]
    fn conditional_else() {
        let ctx = TemplateContext::new();
        assert_eq!(render_ok("$if(title)$yes$else$no$endif$", &ctx), "no");
    }

    #[test]
    fn truthiness_empty_string_is_false() {
        let ctx = ctx_with(&[("x", str_val(""))]);
        assert_eq!(render_ok("$if(x)$yes$else$no$endif$", &ctx), "no");
    }

    #[test]
    fn truthiness_empty_list_is_false() {
        let ctx = ctx_with(&[("x", TemplateValue::List(vec![]))]);
        assert_eq!(render_ok("$if(x)$yes$else$no$endif$", &ctx), "no");
    }

    #[test]
    fn for_loop_over_list() {
        let items = TemplateValue::List(vec![str_val("a"), str_val("b"), str_val("c")]);
        let ctx = ctx_with(&[("item", items)]);
        assert_eq!(render_ok("$for(item)$[$item$]$endfor$", &ctx), "[a][b][c]");
    }

    #[test]
    fn for_loop_with_separator() {
        let items = TemplateValue::List(vec![str_val("x"), str_val("y"), str_val("z")]);
        let ctx = ctx_with(&[("item", items)]);
        assert_eq!(
            render_ok("$for(item)$$item$$sep$, $endfor$", &ctx),
            "x, y, z"
        );
    }

    #[test]
    fn for_loop_single_value_not_list() {
        let ctx = ctx_with(&[("x", str_val("only"))]);
        assert_eq!(render_ok("$for(x)$[$x$]$endfor$", &ctx), "[only]");
    }

    #[test]
    fn for_loop_with_map_items() {
        let mut a1 = HashMap::new();
        a1.insert("name".into(), TemplateValue::Str("Alice".into()));
        let mut a2 = HashMap::new();
        a2.insert("name".into(), TemplateValue::Str("Bob".into()));
        let authors = TemplateValue::List(vec![TemplateValue::Map(a1), TemplateValue::Map(a2)]);
        let ctx = ctx_with(&[("author", authors)]);
        assert_eq!(
            render_ok("$for(author)$$author.name$$sep$ and $endfor$", &ctx),
            "Alice and Bob"
        );
    }

    #[test]
    fn for_loop_missing_var_no_output() {
        let ctx = TemplateContext::new();
        assert_eq!(render_ok("$for(x)$[$x$]$endfor$", &ctx), "");
    }

    #[test]
    fn nested_if_in_for() {
        let mut a1 = HashMap::new();
        a1.insert("name".into(), TemplateValue::Str("Alice".into()));
        a1.insert("email".into(), TemplateValue::Str("a@b.c".into()));
        let mut a2 = HashMap::new();
        a2.insert("name".into(), TemplateValue::Str("Bob".into()));
        let authors = TemplateValue::List(vec![TemplateValue::Map(a1), TemplateValue::Map(a2)]);
        let ctx = ctx_with(&[("author", authors)]);
        assert_eq!(
            render_ok(
                "$for(author)$$author.name$$if(author.email)$ <$author.email$>$endif$$sep$; $endfor$",
                &ctx,
            ),
            "Alice <a@b.c>; Bob"
        );
    }

    #[test]
    fn dollar_escape() {
        let ctx = TemplateContext::new();
        assert_eq!(render_ok("$$100", &ctx), "$100");
    }
}
