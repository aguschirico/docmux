//! Template parser: template string → Vec<TemplateNode>.

use crate::TemplateError;

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

struct Parser {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Self {
            chars: input.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.chars.get(self.pos).copied()?;
        self.pos += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }

    fn error(&self, message: impl Into<String>) -> TemplateError {
        TemplateError::Parse {
            message: message.into(),
            line: self.line,
            column: self.column,
        }
    }

    /// Read a `$...$` tag content (after the opening `$`). Returns the content
    /// before the closing `$` and consumes the closing `$`.
    fn read_tag(&mut self) -> Result<String, TemplateError> {
        let mut content = String::new();
        loop {
            match self.advance() {
                Some('$') => return Ok(content),
                Some(ch) => content.push(ch),
                None => return Err(self.error("unclosed $ tag")),
            }
        }
    }

    /// Parse a variable path like `name` or `author.name`.
    fn parse_var_path(s: &str) -> VarPath {
        s.split('.').map(String::from).collect()
    }

    /// Extract the variable name from a directive like `if(title)` or `for(author)`.
    fn extract_directive_arg<'a>(tag: &'a str, prefix: &str) -> Option<&'a str> {
        let rest = tag.strip_prefix(prefix)?;
        let rest = rest.strip_prefix('(')?;
        let rest = rest.strip_suffix(')')?;
        Some(rest)
    }

    /// Parse nodes until we hit a stop tag or end-of-input.
    /// `stop_tags` is a list of tag contents that should cause us to stop
    /// (e.g. `["endif", "else"]` when parsing inside `$if$`).
    /// Returns (nodes, the stop tag that was hit or None if end-of-input).
    fn parse_nodes(
        &mut self,
        stop_tags: &[&str],
    ) -> Result<(Vec<TemplateNode>, Option<String>), TemplateError> {
        let mut nodes = Vec::new();
        let mut literal = String::new();

        while !self.at_end() {
            if self.peek() == Some('$') {
                self.advance(); // consume first $

                // $$ escape
                if self.peek() == Some('$') {
                    self.advance();
                    literal.push('$');
                    continue;
                }

                // Read the tag content
                let tag = self.read_tag()?;

                // Check for stop tags
                if stop_tags.contains(&tag.as_str()) {
                    if !literal.is_empty() {
                        nodes.push(TemplateNode::Literal(literal));
                    }
                    return Ok((nodes, Some(tag)));
                }

                // Flush accumulated literal
                if !literal.is_empty() {
                    nodes.push(TemplateNode::Literal(std::mem::take(&mut literal)));
                }

                // Parse the tag
                if let Some(var_name) = Self::extract_directive_arg(&tag, "if") {
                    let var = Self::parse_var_path(var_name);
                    let (if_body, stop) = self.parse_nodes(&["else", "endif"])?;
                    let stop = stop.ok_or_else(|| self.error("unclosed $if$: expected $endif$"))?;
                    let else_body = if stop == "else" {
                        let (body, stop2) = self.parse_nodes(&["endif"])?;
                        stop2.ok_or_else(|| self.error("unclosed $if$: expected $endif$"))?;
                        body
                    } else {
                        Vec::new()
                    };
                    nodes.push(TemplateNode::Conditional {
                        var,
                        if_body,
                        else_body,
                    });
                } else if let Some(var_name) = Self::extract_directive_arg(&tag, "for") {
                    let var = Self::parse_var_path(var_name);
                    let (body, stop) = self.parse_nodes(&["sep", "endfor"])?;
                    let stop =
                        stop.ok_or_else(|| self.error("unclosed $for$: expected $endfor$"))?;
                    let separator = if stop == "sep" {
                        let (sep_nodes, stop2) = self.parse_nodes(&["endfor"])?;
                        stop2.ok_or_else(|| self.error("unclosed $for$: expected $endfor$"))?;
                        sep_nodes
                    } else {
                        Vec::new()
                    };
                    nodes.push(TemplateNode::Loop {
                        var,
                        body,
                        separator,
                    });
                } else if tag == "endif" || tag == "endfor" || tag == "else" || tag == "sep" {
                    // Unexpected block-closing tag at top level
                    return Err(self.error(format!("unexpected ${tag}$ without matching block")));
                } else {
                    // Plain variable
                    nodes.push(TemplateNode::Variable(Self::parse_var_path(&tag)));
                }
            } else {
                literal.push(self.advance().expect("checked not at end"));
            }
        }

        // Flush remaining literal
        if !literal.is_empty() {
            nodes.push(TemplateNode::Literal(literal));
        }

        // If we expected a stop tag but hit end-of-input, that's an error
        if !stop_tags.is_empty() {
            return Err(self.error(format!(
                "unexpected end of template, expected ${}$",
                stop_tags[0]
            )));
        }

        Ok((nodes, None))
    }
}

/// Parse a template string into a list of template nodes.
pub(crate) fn parse(template: &str) -> Result<Vec<TemplateNode>, TemplateError> {
    let mut parser = Parser::new(template);
    let (nodes, _) = parser.parse_nodes(&[])?;
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(input: &str) -> Vec<TemplateNode> {
        parse(input).unwrap()
    }

    #[test]
    fn literal_only() {
        let nodes = parse_ok("hello world");
        assert_eq!(nodes.len(), 1);
        assert!(matches!(&nodes[0], TemplateNode::Literal(s) if s == "hello world"));
    }

    #[test]
    fn simple_variable() {
        let nodes = parse_ok("Hello $name$!");
        assert_eq!(nodes.len(), 3);
        assert!(matches!(&nodes[0], TemplateNode::Literal(s) if s == "Hello "));
        assert!(matches!(&nodes[1], TemplateNode::Variable(p) if p == &["name"]));
        assert!(matches!(&nodes[2], TemplateNode::Literal(s) if s == "!"));
    }

    #[test]
    fn dot_access() {
        let nodes = parse_ok("$author.name$");
        assert_eq!(nodes.len(), 1);
        assert!(matches!(&nodes[0], TemplateNode::Variable(p) if p == &["author", "name"]));
    }

    #[test]
    fn dollar_escape() {
        let nodes = parse_ok("Price: $$10");
        assert_eq!(nodes.len(), 1);
        assert!(matches!(&nodes[0], TemplateNode::Literal(s) if s == "Price: $10"));
    }

    #[test]
    fn conditional_if_endif() {
        let nodes = parse_ok("$if(title)$<title>$title$</title>$endif$");
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            TemplateNode::Conditional {
                var,
                if_body,
                else_body,
            } => {
                assert_eq!(var, &["title"]);
                assert_eq!(if_body.len(), 3);
                assert!(else_body.is_empty());
            }
            other => panic!("expected Conditional, got {other:?}"),
        }
    }

    #[test]
    fn conditional_if_else_endif() {
        let nodes = parse_ok("$if(x)$yes$else$no$endif$");
        match &nodes[0] {
            TemplateNode::Conditional {
                if_body, else_body, ..
            } => {
                assert_eq!(if_body.len(), 1);
                assert_eq!(else_body.len(), 1);
                assert!(matches!(&if_body[0], TemplateNode::Literal(s) if s == "yes"));
                assert!(matches!(&else_body[0], TemplateNode::Literal(s) if s == "no"));
            }
            other => panic!("expected Conditional, got {other:?}"),
        }
    }

    #[test]
    fn for_loop() {
        let nodes = parse_ok("$for(item)$[$item$]$endfor$");
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            TemplateNode::Loop {
                var,
                body,
                separator,
            } => {
                assert_eq!(var, &["item"]);
                assert_eq!(body.len(), 3);
                assert!(separator.is_empty());
            }
            other => panic!("expected Loop, got {other:?}"),
        }
    }

    #[test]
    fn for_loop_with_separator() {
        let nodes = parse_ok("$for(x)$$x$$sep$, $endfor$");
        match &nodes[0] {
            TemplateNode::Loop {
                var,
                body,
                separator,
            } => {
                assert_eq!(var, &["x"]);
                assert_eq!(body.len(), 1);
                assert_eq!(separator.len(), 1);
                assert!(matches!(&separator[0], TemplateNode::Literal(s) if s == ", "));
            }
            other => panic!("expected Loop, got {other:?}"),
        }
    }

    #[test]
    fn nested_if_inside_for() {
        let nodes = parse_ok("$for(a)$$if(a.name)$$a.name$$endif$$endfor$");
        assert_eq!(nodes.len(), 1);
        match &nodes[0] {
            TemplateNode::Loop { body, .. } => {
                assert_eq!(body.len(), 1);
                assert!(matches!(&body[0], TemplateNode::Conditional { .. }));
            }
            other => panic!("expected Loop, got {other:?}"),
        }
    }

    #[test]
    fn error_unclosed_if() {
        let err = parse("$if(x)$hello").unwrap_err();
        assert!(matches!(err, crate::TemplateError::Parse { .. }));
    }

    #[test]
    fn error_unclosed_for() {
        let err = parse("$for(x)$hello").unwrap_err();
        assert!(matches!(err, crate::TemplateError::Parse { .. }));
    }

    #[test]
    fn error_unexpected_endif() {
        let err = parse("hello$endif$").unwrap_err();
        assert!(matches!(err, crate::TemplateError::Parse { .. }));
    }

    #[test]
    fn error_unexpected_endfor() {
        let err = parse("hello$endfor$").unwrap_err();
        assert!(matches!(err, crate::TemplateError::Parse { .. }));
    }

    #[test]
    fn empty_template() {
        let nodes = parse_ok("");
        assert!(nodes.is_empty());
    }

    #[test]
    fn multiple_variables_adjacent() {
        let nodes = parse_ok("$a$$b$");
        assert_eq!(nodes.len(), 2);
        assert!(matches!(&nodes[0], TemplateNode::Variable(p) if p == &["a"]));
        assert!(matches!(&nodes[1], TemplateNode::Variable(p) if p == &["b"]));
    }
}
