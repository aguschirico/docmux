//! Template parser: template string → Vec<TemplateNode>.

// ─── AST ────────────────────────────────────────────────────────────────────

/// A dotted variable path, e.g. `["author", "name"]` for `$author.name$`.
pub(crate) type VarPath = Vec<String>;

/// A node in the parsed template AST.
#[derive(Debug, Clone)]
pub(crate) enum TemplateNode {
    /// Literal text (passed through as-is).
    Literal(String),
    /// Variable substitution: `$name$` or `$obj.field$`.
    Variable(VarPath),
    /// Conditional: `$if(var)$...$else$...$endif$`.
    Conditional {
        var: VarPath,
        if_body: Vec<TemplateNode>,
        else_body: Vec<TemplateNode>,
    },
    /// Loop: `$for(var)$...$sep$...$endfor$`.
    Loop {
        var: VarPath,
        body: Vec<TemplateNode>,
        separator: Vec<TemplateNode>,
    },
}

// ─── Parser ─────────────────────────────────────────────────────────────────

pub(crate) fn parse(_template: &str) -> Result<Vec<TemplateNode>, crate::TemplateError> {
    // Stub — will be implemented in Task 2
    Ok(vec![TemplateNode::Literal(_template.to_string())])
}
