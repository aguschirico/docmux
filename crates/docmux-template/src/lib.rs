//! # docmux-template
//!
//! Pandoc-compatible template engine for docmux.
//!
//! Supports `$variable$` substitution, `$if(var)$...$endif$` conditionals,
//! `$for(var)$...$endfor$` loops with `$sep$`, dot access (`$author.name$`),
//! and `$$` literal escaping.

mod parser;
mod renderer;

use std::collections::HashMap;

// ─── Errors ──────────────────────────────────────────────────────────────────

/// Errors that can occur during template parsing or rendering.
#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("template parse error at {line}:{column}: {message}")]
    Parse {
        message: String,
        line: usize,
        column: usize,
    },

    #[error("template render error: {message}")]
    Render { message: String },
}

// ─── Public types ────────────────────────────────────────────────────────────

/// A value in the template context.
#[derive(Debug, Clone)]
pub enum TemplateValue {
    Str(String),
    Bool(bool),
    List(Vec<TemplateValue>),
    Map(HashMap<String, TemplateValue>),
}

impl TemplateValue {
    /// Truthiness: non-empty strings, true bools, non-empty lists/maps.
    pub fn is_truthy(&self) -> bool {
        match self {
            TemplateValue::Str(s) => !s.is_empty(),
            TemplateValue::Bool(b) => *b,
            TemplateValue::List(l) => !l.is_empty(),
            TemplateValue::Map(m) => !m.is_empty(),
        }
    }

    /// Convert to string for output.
    pub fn to_output_string(&self) -> String {
        match self {
            TemplateValue::Str(s) => s.clone(),
            TemplateValue::Bool(b) => b.to_string(),
            TemplateValue::List(_) | TemplateValue::Map(_) => String::new(),
        }
    }
}

/// Template context: variables available during rendering.
pub type TemplateContext = HashMap<String, TemplateValue>;

/// A parsed template, ready to render against a context.
#[derive(Debug)]
pub struct Template {
    nodes: Vec<parser::TemplateNode>,
}

impl Template {
    /// Render this template against the given context.
    pub fn render(&self, ctx: &TemplateContext) -> Result<String, TemplateError> {
        renderer::render(&self.nodes, ctx)
    }
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Parse a template string into a reusable [`Template`].
pub fn parse(template: &str) -> Result<Template, TemplateError> {
    let nodes = parser::parse(template)?;
    Ok(Template { nodes })
}

/// Parse and render a template string in one step.
pub fn render(template: &str, ctx: &TemplateContext) -> Result<String, TemplateError> {
    let tmpl = parse(template)?;
    tmpl.render(ctx)
}

// ─── Default templates ──────────────────────────────────────────────────────

/// Built-in default HTML template.
pub const DEFAULT_HTML: &str = include_str!("../templates/default.html");

/// Built-in default LaTeX template.
pub const DEFAULT_LATEX: &str = include_str!("../templates/default.latex");

/// Built-in default Markdown template.
pub const DEFAULT_MARKDOWN: &str = include_str!("../templates/default.markdown");

/// Built-in default plaintext template.
pub const DEFAULT_PLAINTEXT: &str = include_str!("../templates/default.plaintext");

/// Look up the default template for a given output format name.
pub fn default_template_for(format: &str) -> Option<&'static str> {
    match format {
        "html" => Some(DEFAULT_HTML),
        "latex" => Some(DEFAULT_LATEX),
        "markdown" => Some(DEFAULT_MARKDOWN),
        "plain" | "plaintext" => Some(DEFAULT_PLAINTEXT),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_simple_variable() {
        let mut ctx = TemplateContext::new();
        ctx.insert("name".into(), TemplateValue::Str("world".into()));
        let result = render("Hello $name$!", &ctx).unwrap();
        assert_eq!(result, "Hello world!");
    }

    #[test]
    fn render_missing_variable_is_empty() {
        let ctx = TemplateContext::new();
        let result = render("Hello $name$!", &ctx).unwrap();
        assert_eq!(result, "Hello !");
    }

    #[test]
    fn default_templates_parse_without_error() {
        for (name, src) in [
            ("html", DEFAULT_HTML),
            ("latex", DEFAULT_LATEX),
            ("markdown", DEFAULT_MARKDOWN),
            ("plaintext", DEFAULT_PLAINTEXT),
        ] {
            parse(src).unwrap_or_else(|e| panic!("default {name} template failed to parse: {e}"));
        }
    }
}
