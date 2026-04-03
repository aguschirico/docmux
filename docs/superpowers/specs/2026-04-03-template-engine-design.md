# Template Engine Design

> Date: 2026-04-03

## Goal

Add a pandoc-compatible template engine to docmux. Users can customize the "chrome" around converted content (HTML headers, LaTeX preambles, etc.) using template files with `$variable$` syntax. Default templates replace the current hardcoded `wrap_standalone()` logic in each writer.

## Decisions

- **Template syntax**: Pandoc-compatible (`$var$`, `$if$`, `$for$`, `$sep$`, dot access)
- **Crate**: New `docmux-template` crate under `crates/`
- **Default templates**: External files embedded via `include_str!`
- **Scope (v1)**: Variables, body, conditionals (if/else/endif), loops (for/sep/endfor), dot access, literal `$$` escape. Partials, pipes/filters, and `$elseif$` deferred to v2.

## Template Syntax

| Construct | Syntax | Example |
|-----------|--------|---------|
| Variable | `$name$` | `$title$` |
| Body | `$body$` | `$body$` |
| Conditional | `$if(name)$...$endif$` | `$if(title)$<title>$title$</title>$endif$` |
| Else | `$if(name)$...$else$...$endif$` | `$if(date)$$date$$else$No date$endif$` |
| Loop | `$for(name)$...$endfor$` | `$for(author)$$author.name$$endfor$` |
| Separator | `$sep$` (inside `$for$`) | `$for(keyword)$$keyword$$sep$, $endfor$` |
| Dot access | `$obj.field$` | `$author.name$` |
| Literal `$` | `$$` | `Price: $$10` |

### Truthiness rules

- `Str("")` → false, `Str(non-empty)` → true
- `Bool(b)` → b
- `List([])` → false, `List(non-empty)` → true
- `Map({})` → false, `Map(non-empty)` → true
- Missing key → false

### Loop behavior

- `$for(x)$` iterates over `List` values
- Inside the loop, `$x$` refers to the current item
- If the current item is a `Map`, `$x.field$` accesses its fields
- `$sep$` content is emitted between items (not after the last)
- If `x` is not a list, the loop body executes once with `x` bound to the value (pandoc behavior)

## Architecture

### New crate: `docmux-template`

No dependencies on other docmux crates. Pure template parsing and rendering.

```
crates/docmux-template/
├── Cargo.toml
├── src/
│   ├── lib.rs         # Public API: render(), TemplateContext, TemplateValue
│   ├── parser.rs      # Template string → Vec<TemplateNode>
│   └── renderer.rs    # Vec<TemplateNode> + TemplateContext → String
└── templates/         # Default templates (one per output format)
    ├── default.html
    ├── default.latex
    ├── default.markdown
    └── default.plaintext
```

### Public API

```rust
/// Template context value — the types a template variable can hold.
pub enum TemplateValue {
    Str(String),
    Bool(bool),
    List(Vec<TemplateValue>),
    Map(HashMap<String, TemplateValue>),
}

/// Template context — variables available during rendering.
pub type TemplateContext = HashMap<String, TemplateValue>;

/// Parse and render a template string against a context.
pub fn render(template: &str, ctx: &TemplateContext) -> Result<String, TemplateError>;

/// Parse a template string into a reusable template (for repeated rendering).
pub fn parse(template: &str) -> Result<Template, TemplateError>;

/// A parsed template, ready to render.
pub struct Template { nodes: Vec<TemplateNode> }

impl Template {
    pub fn render(&self, ctx: &TemplateContext) -> Result<String, TemplateError>;
}
```

### Internal template AST

```rust
enum TemplateNode {
    Literal(String),
    Variable(VarPath),              // $title$ or $author.name$
    Conditional {
        var: VarPath,
        if_body: Vec<TemplateNode>,
        else_body: Vec<TemplateNode>,
    },
    Loop {
        var: VarPath,
        binding: String,            // loop variable name (same as var name)
        body: Vec<TemplateNode>,
        separator: Vec<TemplateNode>,
    },
}

/// A dotted variable path: ["author", "name"] for $author.name$.
type VarPath = Vec<String>;
```

### Parser design

Recursive descent parser. Scans for `$` delimiters:

1. Text outside `$...$` → `Literal`
2. `$$` → `Literal("$")`
3. `$name$` or `$name.field$` → `Variable`
4. `$if(name)$` → begin conditional, parse until `$else$` or `$endif$`
5. `$for(name)$` → begin loop, parse until `$endfor$`, extract `$sep$` sections
6. Unmatched or malformed → `TemplateError` with line/column

### Error handling

```rust
pub enum TemplateError {
    ParseError { message: String, line: usize, column: usize },
    RenderError { message: String },
}
```

Errors include position info for parse errors. Render errors cover cases like unclosed tags that somehow passed parsing (defensive).

## Default Templates

Each writer gets a default template file under `crates/docmux-template/templates/`. These are embedded at compile time via `include_str!` and replicate the current hardcoded `wrap_standalone()` output exactly.

### `default.html`

```
<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8" />
$if(title)$<title>$title$</title>
$endif$
$for(css)$<link rel="stylesheet" href="$css$" />
$endfor$
$if(math)$$math$$endif$
$if(highlighting-css)$<style>
$highlighting-css$
</style>
$endif$
</head>
<body>
$if(title)$<h1 class="title">$title$</h1>$endif$
$if(author)$<p class="author">$for(author)$$author.name$$sep$, $endfor$</p>$endif$
$if(date)$<p class="date">$date$</p>$endif$
$if(abstract)$<div class="abstract">$abstract$</div>$endif$
$body$
</body>
</html>
```

### `default.latex`

```
\documentclass{$if(documentclass)$$documentclass$$else$article$endif$}
\usepackage[utf8]{inputenc}
\usepackage{amsmath}
\usepackage{amssymb}
\usepackage{graphicx}
\usepackage{hyperref}
\usepackage{listings}
\usepackage{alltt}
\usepackage{xcolor}
\usepackage{ulem}
$if(highlighting-macros)$$highlighting-macros$$endif$
$if(title)$\title{$title$}$endif$
$if(author)$\author{$for(author)$$author.name$$sep$ \and $endfor$}$endif$
$if(date)$\date{$date$}$endif$
\begin{document}
$if(title)$\maketitle$endif$
$if(abstract)$\begin{abstract}
$abstract$
\end{abstract}$endif$
$body$
\end{document}
```

### `default.markdown`

```
---
$if(title)$title: "$title$"
$endif$
$if(author)$author:
$for(author)$- name: "$author.name$"
$endfor$
$endif$
$if(date)$date: "$date$"
$endif$
$if(abstract)$abstract: "$abstract$"
$endif$
$if(keywords)$keywords:
$for(keyword)$- "$keyword$"
$endfor$
$endif$
---
$body$
```

### `default.plaintext`

```
$if(title)$$title$
$for(title-underline)$=$endfor$
$endif$
$if(author)$$for(author)$$author.name$$sep$, $endfor$
$endif$
$if(date)$$date$
$endif$
$if(abstract)$
Abstract
--------
$abstract$
$endif$
$body$
```

Note: The plaintext title underline is a special case — the writer will provide a `title-underline` list of `=` chars matching the title length.

## Template Context Population

Each writer builds a `TemplateContext` from the document and options. Common fields shared across all writers:

| Key | Source | Type |
|-----|--------|------|
| `title` | `doc.metadata.title` | `Str` |
| `author` | `doc.metadata.authors` | `List` of `Map` (`name`, `email`, `affiliation`, `orcid`) |
| `date` | `doc.metadata.date` | `Str` |
| `abstract` | `doc.metadata.abstract_text` rendered to target format | `Str` |
| `keywords` | `doc.metadata.keywords` | `List` of `Str` |
| `body` | Writer-rendered content | `Str` |
| (all `--variable` keys) | `opts.variables` | `Str` |

Format-specific keys:

| Key | Writer | Description |
|-----|--------|-------------|
| `math` | HTML | KaTeX/MathJax `<script>` block |
| `css` | HTML | List of CSS URLs from `--css` |
| `highlighting-css` | HTML | syntect CSS for code highlighting |
| `documentclass` | LaTeX | From `--variable documentclass=book` |
| `highlighting-macros` | LaTeX | LaTeX highlighting macro definitions |

### Merge order (later overrides earlier)

1. Document metadata
2. Format-specific computed values
3. `--variable KEY=VAL` from CLI

## CLI Integration

### New flags

```
--template=FILE              Use custom template file
--print-default-template=FORMAT   Print the built-in default template for FORMAT and exit
```

### `--template` behavior

- If `--template` is provided, read the file and use it instead of the default
- Implies `--standalone` (pandoc behavior)
- Error if file doesn't exist or can't be read

### `--print-default-template` behavior

- Print the embedded default template for the given format to stdout and exit
- Valid formats: `html`, `latex`, `markdown`, `plaintext`
- Error if format has no default template

## Writer Integration

Each writer's `wrap_standalone()` changes from procedural string building to template rendering. The writer is responsible for:

1. Building the `TemplateContext` with format-specific values
2. Loading the template (custom or default)
3. Calling `docmux_template::render()`

```rust
fn wrap_standalone(&self, body: &str, doc: &Document, opts: &WriteOptions) -> Result<String> {
    let template_src = match &opts.template {
        Some(custom) => std::fs::read_to_string(custom)
            .map_err(|e| docmux_core::Error::IoError(format!("template: {e}")))?,
        None => DEFAULT_TEMPLATE.to_string(),
    };
    let ctx = self.build_template_context(body, doc, opts);
    docmux_template::render(&template_src, &ctx)
        .map_err(|e| docmux_core::Error::TemplateError(e.to_string()))
}
```

Where `DEFAULT_TEMPLATE` is:
```rust
const DEFAULT_TEMPLATE: &str = include_str!("../../docmux-template/templates/default.html");
```

## Testing Strategy

### `docmux-template` crate (~35 tests)

**Parser tests (~15)**:
- Literal text passthrough
- Variable substitution (`$title$`)
- Dot access (`$author.name$`)
- Conditional: true branch, false branch, else branch
- Loop: list iteration, separator, single-value fallback
- Escape: `$$` → `$`
- Nested constructs (if inside for, for inside if)
- Error cases: unclosed `$if$`, unknown directive, malformed syntax

**Renderer tests (~15)**:
- Missing variable → empty string
- Truthiness rules for each type
- Nested dot access into maps
- Loop with separator between items
- Nested conditionals and loops
- Large template (default HTML template with real metadata)

**Default template tests (~5)**:
- Each default template parses without error
- Each default template renders with empty context (no panics)

### Writer integration tests

- Golden file tests: confirm that `--standalone` output with default templates matches current hardcoded output (byte-for-byte or after whitespace normalization)
- Custom template test: `--template=custom.html` produces expected output

### CLI smoke tests

- `--template=FILE` reads and applies template
- `--template=nonexistent` → error
- `--print-default-template=html` → outputs template to stdout
- `--print-default-template=unknown` → error

## Deferred to v2

- `$elseif(var)$` — chained conditionals
- `$partial("file.tex")$` — template includes
- `$var/uppercase$`, `$var/lowercase$` — pipe filters
- Typst and DOCX default templates (these writers have complex non-text output)
