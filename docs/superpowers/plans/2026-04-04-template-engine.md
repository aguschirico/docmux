# Template Engine Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a pandoc-compatible template engine (`$var$`, `$if$`, `$for$`, `$sep$`) as a new `docmux-template` crate, with default templates for HTML/LaTeX/Markdown/Plaintext writers and `--template`/`--print-default-template` CLI flags.

**Architecture:** New `docmux-template` crate with a recursive descent parser (`parser.rs`) and renderer (`renderer.rs`). Writers call `docmux_template::render()` in their `wrap_standalone()` methods using either a user-supplied template or a built-in default embedded via `include_str!`. The CLI adds `--template=FILE` and `--print-default-template=FORMAT`.

**Tech Stack:** Pure Rust, no external dependencies. Workspace crate pattern matching existing transforms.

**Spec:** `docs/superpowers/specs/2026-04-03-template-engine-design.md`

---

### Task 1: Scaffold `docmux-template` crate

**Files:**
- Create: `crates/docmux-template/Cargo.toml`
- Create: `crates/docmux-template/src/lib.rs`
- Create: `crates/docmux-template/src/parser.rs`
- Create: `crates/docmux-template/src/renderer.rs`
- Modify: `Cargo.toml` (workspace root, lines 2-27 members list, lines 38-59 workspace deps)

- [ ] **Step 1: Create `crates/docmux-template/Cargo.toml`**

```toml
[package]
name = "docmux-template"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "Pandoc-compatible template engine for docmux"
rust-version.workspace = true

[dependencies]
thiserror = { workspace = true }
```

- [ ] **Step 2: Create `crates/docmux-template/src/lib.rs`** with public types and API stubs

```rust
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
```

- [ ] **Step 3: Create stub `crates/docmux-template/src/parser.rs`**

```rust
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
```

- [ ] **Step 4: Create stub `crates/docmux-template/src/renderer.rs`**

```rust
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
    ctx: &TemplateContext,
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
```

- [ ] **Step 5: Create placeholder default template files**

Create `crates/docmux-template/templates/default.html`:
```
$body$
```

Create `crates/docmux-template/templates/default.latex`:
```
$body$
```

Create `crates/docmux-template/templates/default.markdown`:
```
$body$
```

Create `crates/docmux-template/templates/default.plaintext`:
```
$body$
```

(These are minimal placeholders so `include_str!` works. Real templates come in Task 5.)

- [ ] **Step 6: Add to workspace**

Add `"crates/docmux-template"` to the `members` list in root `Cargo.toml` (after `docmux-transform-section-divs`).

Add to `[workspace.dependencies]`:
```toml
docmux-template = { path = "crates/docmux-template" }
```

- [ ] **Step 7: Verify it compiles**

Run: `cargo check -p docmux-template`
Expected: compiles with no errors.

- [ ] **Step 8: Run the stub tests**

Run: `cargo test -p docmux-template`
Expected: 3 tests pass (simple variable, missing variable, default templates parse). The first two tests will produce literal text since the parser is stubbed — that's fine for now.

- [ ] **Step 9: Commit**

```bash
git add crates/docmux-template/ Cargo.toml
git commit -m "feat(template): scaffold docmux-template crate with types and stubs"
```

---

### Task 2: Implement template parser

**Files:**
- Modify: `crates/docmux-template/src/parser.rs`

The parser scans for `$` delimiters using a char-by-char state machine. It handles: literal text, `$$` escape, `$var$` / `$var.field$`, `$if(var)$`, `$else$`, `$endif$`, `$for(var)$`, `$sep$`, `$endfor$`.

- [ ] **Step 1: Write parser tests**

Add at the bottom of `crates/docmux-template/src/parser.rs`:

```rust
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
                assert_eq!(if_body.len(), 3); // "<title>", Variable(title), "</title>"
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
                if_body,
                else_body,
                ..
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
                assert_eq!(body.len(), 3); // "[", Variable(item), "]"
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
                assert_eq!(body.len(), 1); // Variable(x)
                assert_eq!(separator.len(), 1); // ", "
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
        assert!(
            matches!(err, crate::TemplateError::Parse { .. }),
            "expected ParseError, got {err:?}"
        );
    }

    #[test]
    fn error_unclosed_for() {
        let err = parse("$for(x)$hello").unwrap_err();
        assert!(
            matches!(err, crate::TemplateError::Parse { .. }),
            "expected ParseError, got {err:?}"
        );
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-template -- parser`
Expected: most tests FAIL (stub parser returns everything as Literal).

- [ ] **Step 3: Implement the parser**

Replace the `parse` function and add helper types in `crates/docmux-template/src/parser.rs`:

```rust
use crate::TemplateError;

// ─── AST (unchanged from above) ────────────────────────────────────────────

pub(crate) type VarPath = Vec<String>;

#[derive(Debug, Clone)]
pub(crate) enum TemplateNode {
    Literal(String),
    Variable(VarPath),
    Conditional {
        var: VarPath,
        if_body: Vec<TemplateNode>,
        else_body: Vec<TemplateNode>,
    },
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-template -- parser`
Expected: all 15 parser tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-template/src/parser.rs
git commit -m "feat(template): implement pandoc-compatible template parser"
```

---

### Task 3: Implement template renderer

**Files:**
- Modify: `crates/docmux-template/src/renderer.rs`

- [ ] **Step 1: Write renderer tests**

Add at the bottom of `crates/docmux-template/src/renderer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse, TemplateContext, TemplateValue};
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
        let nodes = parse(template).unwrap();
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
        assert_eq!(
            render_ok("$if(title)$<h1>$title$</h1>$endif$", &ctx),
            ""
        );
    }

    #[test]
    fn conditional_else() {
        let ctx = TemplateContext::new();
        assert_eq!(
            render_ok("$if(title)$yes$else$no$endif$", &ctx),
            "no"
        );
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-template -- renderer`
Expected: tests FAIL (renderer stub doesn't process variables/conditionals/loops).

- [ ] **Step 3: Implement the renderer**

Replace `crates/docmux-template/src/renderer.rs` with:

```rust
//! Template renderer: Vec<TemplateNode> + TemplateContext → String.

use crate::parser::{TemplateNode, VarPath};
use crate::{TemplateContext, TemplateError, TemplateValue};
use std::collections::HashMap;

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
            let truthy = resolve(var, ctx).map_or(false, |v| v.is_truthy());
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

        // If the item is a Map, also expose `binding.field` paths by
        // inserting the map fields with dotted keys isn't needed — our
        // resolve function handles nested map access.

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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-template`
Expected: ALL tests pass (parser + renderer + lib).

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-template/src/renderer.rs
git commit -m "feat(template): implement template renderer with variables, conditionals, loops"
```

---

### Task 4: Add `TemplateError` variant to `docmux-core`

**Files:**
- Modify: `crates/docmux-core/src/lib.rs` (line 16-36, `ConvertError` enum)
- Modify: `crates/docmux-core/Cargo.toml`

Writers need to return template errors through the existing `ConvertError` type.

- [ ] **Step 1: Add `docmux-template` dependency to `docmux-core`**

In `crates/docmux-core/Cargo.toml`, add:
```toml
docmux-template = { workspace = true }
```

- [ ] **Step 2: Add `Template` variant to `ConvertError`**

In `crates/docmux-core/src/lib.rs`, add a new variant to the `ConvertError` enum (after the `Other` variant):

```rust
    /// A template rendering error.
    #[error("template error: {0}")]
    Template(#[from] docmux_template::TemplateError),
```

Also add the import at the top — actually, `#[from]` handles the conversion automatically via `thiserror`, no explicit import needed beyond the Cargo dep.

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p docmux-core`
Expected: compiles with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-core/
git commit -m "feat(core): add Template error variant for template rendering failures"
```

---

### Task 5: Write default templates

**Files:**
- Modify: `crates/docmux-template/templates/default.html`
- Modify: `crates/docmux-template/templates/default.latex`
- Modify: `crates/docmux-template/templates/default.markdown`
- Modify: `crates/docmux-template/templates/default.plaintext`

These must replicate the output of the current hardcoded `wrap_standalone()` methods.

- [ ] **Step 1: Write `default.html`**

Replace `crates/docmux-template/templates/default.html` with:

```
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
$if(title)$<title>$title$</title>
$endif$$if(math)$$math$
$endif$$for(css)$<link rel="stylesheet" href="$css$">
$endfor$$if(highlighting-css)$<style>
$highlighting-css$
</style>
$endif$</head>
<body>
$body$</body>
</html>
```

Note: The current HTML writer uses `escape_html(title)` for the `<title>` tag. The template system will receive the title already escaped from the writer's `build_template_context()`.

- [ ] **Step 2: Write `default.latex`**

Replace `crates/docmux-template/templates/default.latex` with:

```
\documentclass{$if(documentclass)$$documentclass$$else$article$endif$}
\usepackage[utf8]{inputenc}
\usepackage[T1]{fontenc}
\usepackage{amsmath,amssymb}
\usepackage{graphicx}
\usepackage{hyperref}
\usepackage{listings}
\usepackage{alltt}
\usepackage{xcolor}
\usepackage[normalem]{ulem}
$if(title)$\title{$title$}
$endif$$if(author)$\author{$for(author)$$author.name$$if(author.affiliation)$ \\ $author.affiliation$$endif$$sep$ \and $endfor$}
$endif$$if(date)$\date{$date$}
$endif$
\begin{document}
$if(title)$\maketitle
$endif$$if(abstract)$\begin{abstract}
$abstract$
\end{abstract}
$endif$
$body$\end{document}
```

- [ ] **Step 3: Write `default.markdown`**

Replace `crates/docmux-template/templates/default.markdown` with:

```
$if(has-meta)$---
$if(title)$title: "$title$"
$endif$$if(author-single)$author: "$author-single$"
$endif$$if(author-list)$author:
$for(author-list)$$if(author-list.has-details)$  - name: "$author-list.name$"
$if(author-list.affiliation)$    affiliation: "$author-list.affiliation$"
$endif$$if(author-list.email)$    email: "$author-list.email$"
$endif$$if(author-list.orcid)$    orcid: "$author-list.orcid$"
$endif$$else$  - "$author-list.name$"
$endif$$endfor$$endif$$if(date)$date: "$date$"
$endif$$if(abstract)$abstract: "$abstract$"
$endif$$if(keywords)$keywords: [$for(keyword)$"$keyword$"$sep$, $endfor$]
$endif$$if(custom-meta)$$custom-meta$$endif$---

$endif$$body$
```

Note: The markdown template requires the writer to pre-compute `has-meta`, `author-single` (for 1 author), `author-list` (for >1 authors), `custom-meta` (pre-formatted YAML lines). This is handled by the writer's `build_template_context()`.

- [ ] **Step 4: Write `default.plaintext`**

Replace `crates/docmux-template/templates/default.plaintext` with:

```
$if(title)$$title$
$title-underline$

$endif$$if(author-line)$$author-line$

$endif$$if(date)$$date$

$endif$$if(abstract)$Abstract
--------

$abstract$
$endif$$body$
```

Note: The plaintext writer pre-computes `title-underline` (string of `=` chars matching title length) and `author-line` (comma-joined names).

- [ ] **Step 5: Verify templates parse**

Run: `cargo test -p docmux-template -- default_templates_parse`
Expected: passes.

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-template/templates/
git commit -m "feat(template): add default templates for HTML, LaTeX, Markdown, Plaintext"
```

---

### Task 6: Integrate template engine into HTML writer

**Files:**
- Modify: `crates/docmux-writer-html/Cargo.toml`
- Modify: `crates/docmux-writer-html/src/lib.rs` (lines 530-561, `wrap_standalone`)

- [ ] **Step 1: Add `docmux-template` dependency**

In `crates/docmux-writer-html/Cargo.toml`, add:
```toml
docmux-template = { workspace = true }
```

- [ ] **Step 2: Replace `wrap_standalone` with template-based version**

Replace the `wrap_standalone` method in `crates/docmux-writer-html/src/lib.rs` (currently at line 530) with:

```rust
    fn wrap_standalone(
        &self,
        body: &str,
        doc: &Document,
        opts: &WriteOptions,
    ) -> docmux_core::Result<String> {
        let template_src = match &opts.template {
            Some(path) => std::fs::read_to_string(path)?,
            None => docmux_template::DEFAULT_HTML.to_string(),
        };
        let ctx = self.build_template_context(body, doc, opts);
        docmux_template::render(&template_src, &ctx).map_err(docmux_core::ConvertError::from)
    }

    fn build_template_context(
        &self,
        body: &str,
        doc: &Document,
        opts: &WriteOptions,
    ) -> docmux_template::TemplateContext {
        use docmux_template::TemplateValue;
        let mut ctx = docmux_template::TemplateContext::new();

        // Body
        ctx.insert("body".into(), TemplateValue::Str(body.to_string()));

        // Title (HTML-escaped)
        if let Some(title) = &doc.metadata.title {
            ctx.insert("title".into(), TemplateValue::Str(escape_html(title)));
        }

        // Authors
        if !doc.metadata.authors.is_empty() {
            let author_list: Vec<TemplateValue> = doc
                .metadata
                .authors
                .iter()
                .map(|a| {
                    let mut map = std::collections::HashMap::new();
                    map.insert("name".into(), TemplateValue::Str(escape_html(&a.name)));
                    if let Some(email) = &a.email {
                        map.insert("email".into(), TemplateValue::Str(escape_html(email)));
                    }
                    if let Some(aff) = &a.affiliation {
                        map.insert(
                            "affiliation".into(),
                            TemplateValue::Str(escape_html(aff)),
                        );
                    }
                    if let Some(orcid) = &a.orcid {
                        map.insert("orcid".into(), TemplateValue::Str(escape_html(orcid)));
                    }
                    TemplateValue::Map(map)
                })
                .collect();
            ctx.insert("author".into(), TemplateValue::List(author_list));
        }

        // Date
        if let Some(date) = &doc.metadata.date {
            ctx.insert("date".into(), TemplateValue::Str(escape_html(date)));
        }

        // Abstract (rendered as HTML)
        if let Some(blocks) = &doc.metadata.abstract_text {
            let mut abs_html = String::new();
            self.write_blocks(blocks, opts, doc, &mut abs_html);
            ctx.insert("abstract".into(), TemplateValue::Str(abs_html));
        }

        // Math engine head
        let math_head = match opts.math_engine {
            MathEngine::KaTeX => {
                r#"<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.css">
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/katex.min.js"></script>
<script defer src="https://cdn.jsdelivr.net/npm/katex@0.16.11/dist/contrib/auto-render.min.js"
  onload="renderMathInElement(document.body, {delimiters: [{left: '$$', right: '$$', display: true}, {left: '$', right: '$', display: false}]})"></script>"#
            }
            MathEngine::MathJax => {
                r#"<script src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-mml-chtml.js" async></script>"#
            }
            MathEngine::Raw => "",
        };
        if !math_head.is_empty() {
            ctx.insert("math".into(), TemplateValue::Str(math_head.to_string()));
        }

        // CSS URLs
        let css_urls: Vec<TemplateValue> = opts
            .variables
            .iter()
            .filter(|(k, _)| k == "css" || k.starts_with("css") && k[3..].parse::<u32>().is_ok())
            .map(|(_, v)| TemplateValue::Str(v.clone()))
            .collect();
        if !css_urls.is_empty() {
            ctx.insert("css".into(), TemplateValue::List(css_urls));
        }

        // Merge user variables (these override metadata)
        for (k, v) in &opts.variables {
            if !k.starts_with("css") {
                ctx.insert(k.clone(), TemplateValue::Str(v.clone()));
            }
        }

        ctx
    }
```

- [ ] **Step 3: Update `write()` to propagate the Result from `wrap_standalone`**

The `write()` method (around line 573) currently calls `self.wrap_standalone(...)` which returned `String`. Now it returns `Result<String>`. Update the `write()` method:

```rust
    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
        let mut body = String::with_capacity(4096);
        self.write_blocks(&doc.content, opts, doc, &mut body);

        if opts.standalone {
            self.wrap_standalone(&body, doc, opts)
        } else {
            Ok(body)
        }
    }
```

- [ ] **Step 4: Run HTML writer tests**

Run: `cargo test -p docmux-writer-html`
Expected: all existing tests still pass. The standalone test output may differ slightly in whitespace — if tests fail on exact HTML matching, adjust the default template whitespace to match.

- [ ] **Step 5: Run workspace tests**

Run: `cargo test --workspace`
Expected: all tests pass (including golden file tests). If standalone HTML golden tests fail, update expectations with `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test` after verifying the new output is correct.

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-writer-html/
git commit -m "feat(html-writer): replace hardcoded standalone with template engine"
```

---

### Task 7: Integrate template engine into LaTeX writer

**Files:**
- Modify: `crates/docmux-writer-latex/Cargo.toml`
- Modify: `crates/docmux-writer-latex/src/lib.rs` (lines 444-499, `wrap_standalone`)

- [ ] **Step 1: Add `docmux-template` dependency**

In `crates/docmux-writer-latex/Cargo.toml`, add:
```toml
docmux-template = { workspace = true }
```

- [ ] **Step 2: Replace `wrap_standalone` with template-based version**

Replace the `wrap_standalone` method in `crates/docmux-writer-latex/src/lib.rs`:

```rust
    fn wrap_standalone(
        &self,
        body: &str,
        doc: &Document,
        opts: &WriteOptions,
    ) -> docmux_core::Result<String> {
        let template_src = match &opts.template {
            Some(path) => std::fs::read_to_string(path)?,
            None => docmux_template::DEFAULT_LATEX.to_string(),
        };
        let ctx = self.build_template_context(body, doc, opts);
        docmux_template::render(&template_src, &ctx).map_err(docmux_core::ConvertError::from)
    }

    fn build_template_context(
        &self,
        body: &str,
        doc: &Document,
        opts: &WriteOptions,
    ) -> docmux_template::TemplateContext {
        use docmux_template::TemplateValue;
        let mut ctx = docmux_template::TemplateContext::new();

        ctx.insert("body".into(), TemplateValue::Str(body.to_string()));

        if let Some(title) = &doc.metadata.title {
            ctx.insert("title".into(), TemplateValue::Str(escape_latex(title)));
        }

        if !doc.metadata.authors.is_empty() {
            let author_list: Vec<TemplateValue> = doc
                .metadata
                .authors
                .iter()
                .map(|a| {
                    let mut map = std::collections::HashMap::new();
                    map.insert("name".into(), TemplateValue::Str(escape_latex(&a.name)));
                    if let Some(aff) = &a.affiliation {
                        map.insert(
                            "affiliation".into(),
                            TemplateValue::Str(escape_latex(aff)),
                        );
                    }
                    TemplateValue::Map(map)
                })
                .collect();
            ctx.insert("author".into(), TemplateValue::List(author_list));
        }

        if let Some(date) = &doc.metadata.date {
            ctx.insert("date".into(), TemplateValue::Str(escape_latex(date)));
        }

        if let Some(blocks) = &doc.metadata.abstract_text {
            let mut abs_latex = String::new();
            self.write_blocks(blocks, opts, &mut abs_latex);
            ctx.insert("abstract".into(), TemplateValue::Str(abs_latex));
        }

        // Merge user variables
        for (k, v) in &opts.variables {
            ctx.insert(k.clone(), TemplateValue::Str(v.clone()));
        }

        ctx
    }
```

- [ ] **Step 3: Update `write()` to propagate the Result**

```rust
    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
        let mut body = String::with_capacity(4096);
        self.write_blocks(&doc.content, opts, &mut body);

        if opts.standalone {
            self.wrap_standalone(&body, doc, opts)
        } else {
            Ok(body)
        }
    }
```

- [ ] **Step 4: Run LaTeX writer tests**

Run: `cargo test -p docmux-writer-latex`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-latex/
git commit -m "feat(latex-writer): replace hardcoded standalone with template engine"
```

---

### Task 8: Integrate template engine into Markdown writer

**Files:**
- Modify: `crates/docmux-writer-markdown/Cargo.toml`
- Modify: `crates/docmux-writer-markdown/src/lib.rs` (lines 609-676, `wrap_standalone`)

- [ ] **Step 1: Add `docmux-template` dependency**

In `crates/docmux-writer-markdown/Cargo.toml`, add:
```toml
docmux-template = { workspace = true }
```

- [ ] **Step 2: Replace `wrap_standalone` with template-based version**

Replace the `wrap_standalone` method:

```rust
    fn wrap_standalone(
        &self,
        body: &str,
        doc: &Document,
    ) -> docmux_core::Result<String> {
        let template_src = docmux_template::DEFAULT_MARKDOWN.to_string();
        let ctx = self.build_template_context(body, doc);
        docmux_template::render(&template_src, &ctx).map_err(docmux_core::ConvertError::from)
    }

    fn build_template_context(
        &self,
        body: &str,
        doc: &Document,
    ) -> docmux_template::TemplateContext {
        use docmux_template::TemplateValue;
        let meta = &doc.metadata;
        let mut ctx = docmux_template::TemplateContext::new();

        ctx.insert("body".into(), TemplateValue::Str(body.to_string()));

        let has_meta = meta.title.is_some()
            || !meta.authors.is_empty()
            || meta.date.is_some()
            || meta.abstract_text.is_some()
            || !meta.keywords.is_empty()
            || !meta.custom.is_empty();

        if has_meta {
            ctx.insert("has-meta".into(), TemplateValue::Bool(true));
        }

        if let Some(title) = &meta.title {
            ctx.insert("title".into(), TemplateValue::Str(yaml_escape(title)));
        }

        // Single author vs list
        if meta.authors.len() == 1 {
            ctx.insert(
                "author-single".into(),
                TemplateValue::Str(yaml_escape(&meta.authors[0].name)),
            );
        } else if meta.authors.len() > 1 {
            let list: Vec<TemplateValue> = meta
                .authors
                .iter()
                .map(|a| {
                    let mut map = std::collections::HashMap::new();
                    map.insert("name".into(), TemplateValue::Str(yaml_escape(&a.name)));
                    let has_details =
                        a.affiliation.is_some() || a.email.is_some() || a.orcid.is_some();
                    map.insert("has-details".into(), TemplateValue::Bool(has_details));
                    if let Some(aff) = &a.affiliation {
                        map.insert(
                            "affiliation".into(),
                            TemplateValue::Str(yaml_escape(aff)),
                        );
                    }
                    if let Some(email) = &a.email {
                        map.insert("email".into(), TemplateValue::Str(yaml_escape(email)));
                    }
                    if let Some(orcid) = &a.orcid {
                        map.insert("orcid".into(), TemplateValue::Str(yaml_escape(orcid)));
                    }
                    TemplateValue::Map(map)
                })
                .collect();
            ctx.insert("author-list".into(), TemplateValue::List(list));
        }

        if let Some(date) = &meta.date {
            ctx.insert("date".into(), TemplateValue::Str(yaml_escape(date)));
        }

        if let Some(abstract_blocks) = &meta.abstract_text {
            let text = blocks_to_plain_text(abstract_blocks);
            ctx.insert("abstract".into(), TemplateValue::Str(yaml_escape(&text)));
        }

        if !meta.keywords.is_empty() {
            let kw_list: Vec<TemplateValue> = meta
                .keywords
                .iter()
                .map(|k| TemplateValue::Str(yaml_escape(k)))
                .collect();
            ctx.insert("keyword".into(), TemplateValue::List(kw_list));
        }

        if !meta.custom.is_empty() {
            let mut custom_yaml = String::new();
            for (k, v) in &meta.custom {
                write_meta_value(&mut custom_yaml, k, v, 0);
            }
            ctx.insert("custom-meta".into(), TemplateValue::Str(custom_yaml));
        }

        ctx
    }
```

Note: The Markdown writer currently takes no `opts` in `wrap_standalone`. The signature changes to return `Result` and the `write()` method must propagate. Also, the template doesn't support custom templates (no `opts.template` access) — the markdown writer can add that later if needed. For now, it always uses the default.

- [ ] **Step 3: Update `write()` to propagate the Result**

```rust
    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
        let ctx = Ctx {};
        let mut body = String::with_capacity(4096);
        self.write_blocks(&doc.content, &mut body, &ctx);

        if opts.standalone {
            self.wrap_standalone(&body, doc)
        } else {
            Ok(body)
        }
    }
```

- [ ] **Step 4: Run Markdown writer tests**

Run: `cargo test -p docmux-writer-markdown`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-markdown/
git commit -m "feat(markdown-writer): replace hardcoded standalone with template engine"
```

---

### Task 9: Integrate template engine into Plaintext writer

**Files:**
- Modify: `crates/docmux-writer-plaintext/Cargo.toml`
- Modify: `crates/docmux-writer-plaintext/src/lib.rs` (lines 434-464, `write_standalone_header`)

- [ ] **Step 1: Add `docmux-template` dependency**

In `crates/docmux-writer-plaintext/Cargo.toml`, add:
```toml
docmux-template = { workspace = true }
```

- [ ] **Step 2: Replace `write_standalone_header` with template-based `wrap_standalone`**

Replace the `write_standalone_header` method with:

```rust
    fn wrap_standalone(
        &self,
        body: &str,
        doc: &Document,
    ) -> docmux_core::Result<String> {
        let template_src = docmux_template::DEFAULT_PLAINTEXT.to_string();
        let ctx = self.build_template_context(body, doc);
        docmux_template::render(&template_src, &ctx).map_err(docmux_core::ConvertError::from)
    }

    fn build_template_context(
        &self,
        body: &str,
        doc: &Document,
    ) -> docmux_template::TemplateContext {
        use docmux_template::TemplateValue;
        let mut ctx = docmux_template::TemplateContext::new();

        ctx.insert("body".into(), TemplateValue::Str(body.to_string()));

        if let Some(title) = &doc.metadata.title {
            ctx.insert("title".into(), TemplateValue::Str(title.clone()));
            let underline = "=".repeat(title.len());
            ctx.insert("title-underline".into(), TemplateValue::Str(underline));
        }

        if !doc.metadata.authors.is_empty() {
            let names: Vec<&str> = doc.metadata.authors.iter().map(|a| a.name.as_str()).collect();
            ctx.insert("author-line".into(), TemplateValue::Str(names.join(", ")));
        }

        if let Some(date) = &doc.metadata.date {
            ctx.insert("date".into(), TemplateValue::Str(date.clone()));
        }

        if let Some(abstract_blocks) = &doc.metadata.abstract_text {
            let mut abs_text = String::new();
            self.write_blocks(abstract_blocks, &mut abs_text, "");
            ctx.insert("abstract".into(), TemplateValue::Str(abs_text));
        }

        ctx
    }
```

- [ ] **Step 3: Update `write()` to use `wrap_standalone` instead of `write_standalone_header`**

Replace the `write()` method (around line 476):

```rust
    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
        let mut body = String::with_capacity(4096);
        self.write_blocks(&doc.content, &mut body, "");

        let content = if opts.standalone {
            self.wrap_standalone(&body, doc)?
        } else {
            body
        };

        // Trim trailing whitespace/newlines from the final output.
        let trimmed = content.trim_end().to_string();
        Ok(if trimmed.is_empty() {
            trimmed
        } else {
            let mut result = trimmed;
            result.push('\n');
            result
        })
    }
```

- [ ] **Step 4: Run plaintext writer tests**

Run: `cargo test -p docmux-writer-plaintext`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-plaintext/
git commit -m "feat(plaintext-writer): replace hardcoded standalone with template engine"
```

---

### Task 10: Add `--template` and `--print-default-template` CLI flags

**Files:**
- Modify: `crates/docmux-cli/Cargo.toml`
- Modify: `crates/docmux-cli/src/main.rs`

- [ ] **Step 1: Add `docmux-template` dependency to CLI**

In `crates/docmux-cli/Cargo.toml`, add to `[dependencies]`:
```toml
docmux-template = { workspace = true }
```

- [ ] **Step 2: Add CLI flags to the `Cli` struct**

In `crates/docmux-cli/src/main.rs`, add after the `section_divs` field (around line 129):

```rust
    /// Custom template file (implies --standalone)
    #[arg(long, value_name = "FILE")]
    template: Option<PathBuf>,

    /// Print the default template for the given format and exit
    #[arg(long, value_name = "FORMAT")]
    print_default_template: Option<String>,
```

Also update the `required_unless_present_any` on `input` (line 35) to include `print_default_template`:

```rust
    #[arg(required_unless_present_any = ["list_input_formats", "list_output_formats", "list_highlight_themes", "list_highlight_languages", "print_default_template"])]
```

- [ ] **Step 3: Handle `--print-default-template` in `main()`**

Add after the `list_highlight_languages` handler (after line 191):

```rust
    if let Some(format) = &cli.print_default_template {
        match docmux_template::default_template_for(format) {
            Some(tmpl) => {
                print!("{tmpl}");
                return;
            }
            None => {
                eprintln!(
                    "docmux: no default template for format '{format}'. Available: html, latex, markdown, plain"
                );
                std::process::exit(1);
            }
        }
    }
```

- [ ] **Step 4: Wire `--template` into `WriteOptions`**

Update the `WriteOptions` construction (around line 395) to include the template:

```rust
    // --template implies --standalone
    let standalone = cli.standalone || cli.template.is_some();

    let template = cli.template.as_ref().map(|p| p.display().to_string());

    let opts = WriteOptions {
        standalone,
        template,
        math_engine,
        variables,
        wrap,
        columns: cli.columns,
        eol,
        highlight_style: cli.highlight_style.clone(),
        ..Default::default()
    };
```

- [ ] **Step 5: Run CLI smoke tests**

Run: `cargo test -p docmux-cli`
Expected: all existing tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-cli/
git commit -m "feat(cli): add --template and --print-default-template flags"
```

---

### Task 11: CLI smoke tests for template features

**Files:**
- Modify: `crates/docmux-cli/tests/cli_smoke.rs`

- [ ] **Step 1: Write smoke tests**

Add to `crates/docmux-cli/tests/cli_smoke.rs`:

```rust
#[test]
fn print_default_template_html() {
    let output = Command::new(docmux_bin())
        .arg("--print-default-template=html")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success(), "exit code was non-zero");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("$body$"),
        "expected $body$ in template, got: {stdout}"
    );
    assert!(
        stdout.contains("<!DOCTYPE html>"),
        "expected HTML doctype in template"
    );
}

#[test]
fn print_default_template_latex() {
    let output = Command::new(docmux_bin())
        .arg("--print-default-template=latex")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\\documentclass"));
    assert!(stdout.contains("$body$"));
}

#[test]
fn print_default_template_unknown_format_fails() {
    let output = Command::new(docmux_bin())
        .arg("--print-default-template=unknown")
        .output()
        .expect("failed to run docmux");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no default template"));
}

#[test]
fn custom_template_flag() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let tmp_dir = std::env::temp_dir().join("docmux-template-test");
    std::fs::create_dir_all(&tmp_dir).ok();

    // Write a minimal custom template
    let template_file = tmp_dir.join("custom.html");
    std::fs::write(
        &template_file,
        "<custom>$body$</custom>",
    )
    .expect("failed to write template");

    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("-t")
        .arg("html")
        .arg("--template")
        .arg(&template_file)
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success(), "exit code was non-zero");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("<custom>"),
        "expected custom template wrapper, got: {stdout}"
    );
    assert!(
        stdout.contains("<p>"),
        "expected rendered body content in output"
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn template_implies_standalone() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let tmp_dir = std::env::temp_dir().join("docmux-template-test-standalone");
    std::fs::create_dir_all(&tmp_dir).ok();

    let template_file = tmp_dir.join("minimal.html");
    std::fs::write(&template_file, "HEADER\n$body$\nFOOTER").expect("write template");

    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("-t")
        .arg("html")
        .arg("--template")
        .arg(&template_file)
        // Note: no --standalone flag, but --template implies it
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("HEADER"), "template wrapper expected");
    assert!(stdout.contains("FOOTER"), "template wrapper expected");

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn template_nonexistent_file_fails() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("-t")
        .arg("html")
        .arg("--template")
        .arg("/nonexistent/template.html")
        .output()
        .expect("failed to run docmux");

    assert!(!output.status.success());
}
```

- [ ] **Step 2: Run new smoke tests**

Run: `cargo test -p docmux-cli -- template`
Expected: all 6 new tests pass.

- [ ] **Step 3: Run full workspace tests**

Run: `cargo test --workspace`
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-cli/tests/cli_smoke.rs
git commit -m "test(cli): add smoke tests for --template and --print-default-template"
```

---

### Task 12: Fix golden file test mismatches

**Files:**
- Possibly modify: golden test expectation files

The default templates should produce output identical to the old hardcoded `wrap_standalone()`. But minor whitespace differences are likely. This task exists to detect and fix them.

- [ ] **Step 1: Run golden file tests specifically**

Run: `cargo test -p docmux-cli -- golden`
Expected: either all pass (ideal) or some fail due to whitespace.

- [ ] **Step 2: If failures, review and update expectations**

If there are golden file failures:
1. Review the diff to confirm the new output is semantically correct
2. Run `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli -- golden` to update
3. Manually verify the updated expectations look right

- [ ] **Step 3: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 4: Commit (if any changes)**

```bash
git add -A
git commit -m "fix: update golden test expectations for template engine output"
```

(Skip if no changes needed.)

---

### Task 13: Update ROADMAP.md and docs

**Files:**
- Modify: `ROADMAP.md`

- [ ] **Step 1: Check off completed items in ROADMAP.md**

Mark these as done:
- `--template=FILE` with template engine → `[x]`
- Template engine (variable interpolation, conditionals, loops) → `[x]`
- Built-in default templates per output format → `[x]`

- [ ] **Step 2: Commit**

```bash
git add ROADMAP.md
git commit -m "docs: mark template engine as complete in roadmap"
```
