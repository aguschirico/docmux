//! # docmux-reader-markdown
//!
//! Markdown reader for docmux. Parses CommonMark + GFM extensions into the
//! docmux AST using [comrak](https://crates.io/crates/comrak) under the hood.
//!
//! Supports YAML frontmatter (delimited by `---`) which is parsed into the
//! [`Metadata`] struct. Known fields (`title`, `author`, `date`, `abstract`)
//! are extracted into typed fields; everything else goes into `custom`.

use comrak::{
    nodes::{AstNode, NodeValue},
    parse_document, Arena, Options,
};
use docmux_ast::*;
use docmux_core::{Reader, Result};
use std::collections::{HashMap, HashSet};

/// A Markdown reader backed by comrak.
#[derive(Debug, Default)]
pub struct MarkdownReader;

impl MarkdownReader {
    pub fn new() -> Self {
        Self
    }

    fn comrak_options() -> Options<'static> {
        let mut opts = Options::default();
        // Enable common extensions
        opts.extension.strikethrough = true;
        opts.extension.table = true;
        opts.extension.autolink = true;
        opts.extension.tasklist = true;
        opts.extension.footnotes = true;
        opts.extension.description_lists = true;
        opts.extension.math_dollars = true;
        opts.extension.math_code = true;
        opts.extension.front_matter_delimiter = Some("---".into());
        opts.extension.subscript = true;
        opts.extension.superscript = true;
        // Parse options
        opts.parse.smart = true;
        opts
    }

    /// Extract YAML frontmatter from the comrak AST and parse it into Metadata.
    fn extract_frontmatter<'a>(&self, root: &'a AstNode<'a>) -> Metadata {
        for child in root.children() {
            let ast = child.data.borrow();
            if let NodeValue::FrontMatter(ref raw) = ast.value {
                // comrak includes the delimiters; strip them
                let yaml = raw
                    .trim()
                    .strip_prefix("---")
                    .unwrap_or(raw)
                    .strip_suffix("---")
                    .unwrap_or(raw)
                    .trim();

                if yaml.is_empty() {
                    return Metadata::default();
                }

                return self.parse_yaml_frontmatter(yaml);
            }
        }
        Metadata::default()
    }

    /// Parse a YAML string into our Metadata struct (two-pass approach).
    ///
    /// First pass: deserialize to `serde_yaml::Value` to capture everything.
    /// Second pass: extract known fields into typed Metadata fields, put the
    /// rest into `custom`.
    fn parse_yaml_frontmatter(&self, yaml: &str) -> Metadata {
        let value: serde_yaml::Value = match serde_yaml::from_str(yaml) {
            Ok(v) => v,
            Err(_) => return Metadata::default(),
        };

        let mapping = match value.as_mapping() {
            Some(m) => m,
            None => return Metadata::default(),
        };

        let mut metadata = Metadata::default();
        let mut custom = HashMap::new();

        for (key, val) in mapping {
            let key_str = match key.as_str() {
                Some(s) => s,
                None => continue,
            };

            match key_str {
                "title" => {
                    metadata.title = val.as_str().map(String::from);
                }
                "date" => {
                    metadata.date = yaml_value_to_string(val);
                }
                "abstract" | "abstract_text" | "description" => {
                    metadata.abstract_text = val.as_str().map(|s| {
                        vec![Block::Paragraph {
                            content: vec![Inline::Text {
                                value: s.to_string(),
                            }],
                        }]
                    });
                }
                "keywords" | "tags" => {
                    metadata.keywords = parse_string_list(val);
                }
                "author" | "authors" => {
                    metadata.authors = parse_authors(val);
                }
                _ => {
                    if let Some(mv) = yaml_to_meta_value(val) {
                        custom.insert(key_str.to_string(), mv);
                    }
                }
            }
        }

        metadata.custom = custom;
        metadata
    }

    /// Convert a comrak AST node tree into our docmux AST blocks.
    /// Skips FrontMatter nodes (already extracted by `extract_frontmatter`).
    fn convert_node<'a>(&self, node: &'a AstNode<'a>) -> Vec<Block> {
        let mut blocks = Vec::new();

        for child in node.children() {
            // Skip frontmatter — already handled
            if matches!(child.data.borrow().value, NodeValue::FrontMatter(_)) {
                continue;
            }
            if let Some(block) = self.node_to_block(child) {
                blocks.push(block);
            }
        }

        blocks
    }

    fn node_to_block<'a>(&self, node: &'a AstNode<'a>) -> Option<Block> {
        let ast = node.data.borrow();
        match &ast.value {
            NodeValue::Paragraph => {
                // Check for a paragraph that wraps a single display-math node.
                // comrak places `$$…$$` inside a Paragraph; we promote it to
                // a proper Block::MathBlock so writers can render it as a
                // display equation (e.g. <div> instead of <span>).
                if let Some(math_block) = self.try_extract_display_math(node) {
                    return Some(math_block);
                }
                let content = self.collect_inlines(node);
                Some(Block::Paragraph { content })
            }
            NodeValue::Heading(h) => {
                let mut content = self.collect_inlines(node);
                let parsed_attrs = extract_trailing_attrs(&mut content);
                let id = parsed_attrs.as_ref().and_then(|a| a.id.clone());
                // Only store attrs if there are classes or key-values
                // (the id is already on Heading.id)
                let attrs = parsed_attrs.and_then(|a| {
                    if a.classes.is_empty() && a.key_values.is_empty() {
                        None
                    } else {
                        Some(Attributes {
                            id: None,
                            classes: a.classes,
                            key_values: a.key_values,
                        })
                    }
                });
                Some(Block::Heading {
                    level: h.level,
                    id,
                    content,
                    attrs,
                })
            }
            NodeValue::CodeBlock(cb) => {
                let info = cb.info.trim();
                // Raw attribute: ```{=format} → RawBlock
                if let Some(raw_fmt) = parse_raw_attribute(info) {
                    return Some(Block::RawBlock {
                        format: raw_fmt,
                        content: cb.literal.clone(),
                    });
                }
                let (language, attrs) = if info.starts_with('{') {
                    // Pandoc-style fenced code attributes: ```{.python .numberLines}
                    match parse_attr_block(info) {
                        Some(a) => {
                            let lang = a.classes.first().cloned();
                            let attrs = Some(a);
                            (lang, attrs)
                        }
                        None => {
                            // Failed to parse as attrs — treat whole info as language
                            let lang = if info.is_empty() {
                                None
                            } else {
                                Some(info.to_string())
                            };
                            (lang, None)
                        }
                    }
                } else if info.is_empty() {
                    (None, None)
                } else {
                    // Standard info string: first word is language
                    let lang = info.split_whitespace().next().map(String::from);
                    (lang, None)
                };
                Some(Block::CodeBlock {
                    language,
                    content: cb.literal.clone(),
                    caption: None,
                    label: None,
                    attrs,
                })
            }
            NodeValue::BlockQuote => {
                let content = self.convert_node(node);
                Some(Block::BlockQuote { content })
            }
            NodeValue::List(list) => {
                let ordered = matches!(list.list_type, comrak::nodes::ListType::Ordered);
                let start = if ordered {
                    Some(list.start as u32)
                } else {
                    None
                };
                let items: Vec<ListItem> = node
                    .children()
                    .map(|item| {
                        let ast = item.data.borrow();
                        let checked = if let NodeValue::TaskItem(Some(c)) = &ast.value {
                            // comrak uses char for task items; 'x' or 'X' means checked
                            Some(*c == 'x' || *c == 'X')
                        } else {
                            None
                        };
                        ListItem {
                            checked,
                            content: self.convert_node(item),
                        }
                    })
                    .collect();
                Some(Block::List {
                    ordered,
                    start,
                    items,
                    tight: list.tight,
                    style: None,
                    delimiter: None,
                })
            }
            NodeValue::Table(..) => {
                let rows = self.parse_table(node);
                Some(Block::Table(rows))
            }
            NodeValue::ThematicBreak => Some(Block::ThematicBreak),
            NodeValue::FootnoteDefinition(ref def) => {
                let content = self.convert_node(node);
                Some(Block::FootnoteDef {
                    id: def.name.clone(),
                    content,
                })
            }
            NodeValue::Math(math) => {
                if math.display_math {
                    Some(Block::MathBlock {
                        content: math.literal.clone(),
                        label: None,
                    })
                } else {
                    // Inline math shouldn't appear at block level,
                    // but wrap it in a paragraph if it does.
                    Some(Block::Paragraph {
                        content: vec![Inline::MathInline {
                            value: math.literal.clone(),
                        }],
                    })
                }
            }
            NodeValue::DescriptionList => {
                let items: Vec<DefinitionItem> = node
                    .children()
                    .filter_map(|item_node| {
                        let item_ast = item_node.data.borrow();
                        if !matches!(item_ast.value, NodeValue::DescriptionItem(_)) {
                            return None;
                        }
                        drop(item_ast);

                        let mut term = Vec::new();
                        let mut definitions = Vec::new();

                        for child in item_node.children() {
                            let child_ast = child.data.borrow();
                            match &child_ast.value {
                                NodeValue::DescriptionTerm => {
                                    drop(child_ast);
                                    term = self.collect_inlines(child);
                                }
                                NodeValue::DescriptionDetails => {
                                    drop(child_ast);
                                    definitions.push(self.convert_node(child));
                                }
                                _ => {}
                            }
                        }

                        Some(DefinitionItem { term, definitions })
                    })
                    .collect();
                Some(Block::DefinitionList { items })
            }
            _ => {
                // Skip unknown node types for now
                None
            }
        }
    }

    /// If `node` is a Paragraph whose sole child is a display-math node,
    /// extract it as a `Block::MathBlock`. Returns `None` otherwise.
    fn try_extract_display_math<'a>(&self, node: &'a AstNode<'a>) -> Option<Block> {
        let children: Vec<_> = node.children().collect();
        if children.len() != 1 {
            return None;
        }
        let child_ast = children[0].data.borrow();
        if let NodeValue::Math(ref math) = child_ast.value {
            if math.display_math {
                return Some(Block::MathBlock {
                    content: math.literal.trim().to_string(),
                    label: None,
                });
            }
        }
        None
    }

    /// Collect inline children of a node, applying bracketed-span post-processing.
    fn collect_inlines<'a>(&self, node: &'a AstNode<'a>) -> Vec<Inline> {
        let mut inlines = Vec::new();
        for child in node.children() {
            self.node_to_inlines(child, &mut inlines);
        }
        postprocess_bracketed_spans(&mut inlines);
        inlines
    }

    fn node_to_inlines<'a>(&self, node: &'a AstNode<'a>, out: &mut Vec<Inline>) {
        let ast = node.data.borrow();
        match &ast.value {
            NodeValue::Text(t) => {
                out.push(Inline::Text { value: t.clone() });
            }
            NodeValue::Code(c) => {
                out.push(Inline::Code {
                    value: c.literal.clone(),
                    attrs: None,
                });
            }
            NodeValue::Emph => {
                let content = self.collect_inlines(node);
                out.push(Inline::Emphasis { content });
            }
            NodeValue::Strong => {
                let content = self.collect_inlines(node);
                out.push(Inline::Strong { content });
            }
            NodeValue::Strikethrough => {
                let content = self.collect_inlines(node);
                out.push(Inline::Strikethrough { content });
            }
            NodeValue::Link(link) => {
                let content = self.collect_inlines(node);
                out.push(Inline::Link {
                    url: link.url.clone(),
                    title: if link.title.is_empty() {
                        None
                    } else {
                        Some(link.title.clone())
                    },
                    content,
                    attrs: None,
                });
            }
            NodeValue::Image(img) => {
                // Collect alt text from children
                let alt = self.collect_inlines(node);
                out.push(Inline::Image(Image {
                    url: img.url.clone(),
                    alt,
                    title: if img.title.is_empty() {
                        None
                    } else {
                        Some(img.title.clone())
                    },
                    attrs: None,
                }));
            }
            NodeValue::SoftBreak => {
                out.push(Inline::SoftBreak);
            }
            NodeValue::LineBreak => {
                out.push(Inline::HardBreak);
            }
            NodeValue::FootnoteReference(ref fref) => {
                out.push(Inline::FootnoteRef {
                    id: fref.name.clone(),
                });
            }
            NodeValue::Math(math) => {
                if math.display_math {
                    // Display math in inline context — treat as inline
                    out.push(Inline::MathInline {
                        value: math.literal.clone(),
                    });
                } else {
                    out.push(Inline::MathInline {
                        value: math.literal.clone(),
                    });
                }
            }
            NodeValue::Superscript => {
                let content = self.collect_inlines(node);
                out.push(Inline::Superscript { content });
            }
            NodeValue::Subscript => {
                let content = self.collect_inlines(node);
                out.push(Inline::Subscript { content });
            }
            _ => {
                // For unknown inlines, try to collect children
                for child in node.children() {
                    self.node_to_inlines(child, out);
                }
            }
        }
    }

    /// Parse a comrak table node into our Table type.
    fn parse_table<'a>(&self, node: &'a AstNode<'a>) -> Table {
        let mut columns = Vec::new();
        let mut header = None;
        let mut rows = Vec::new();
        let mut is_first_row = true;

        // Extract column alignments from the Table node
        if let NodeValue::Table(ref table) = node.data.borrow().value {
            columns = table
                .alignments
                .iter()
                .map(|a| ColumnSpec {
                    alignment: match a {
                        comrak::nodes::TableAlignment::Left => Alignment::Left,
                        comrak::nodes::TableAlignment::Center => Alignment::Center,
                        comrak::nodes::TableAlignment::Right => Alignment::Right,
                        comrak::nodes::TableAlignment::None => Alignment::Default,
                    },
                    width: None,
                })
                .collect();
        }

        for row_node in node.children() {
            let cells: Vec<TableCell> = row_node
                .children()
                .map(|cell_node| TableCell {
                    content: vec![Block::Paragraph {
                        content: self.collect_inlines(cell_node),
                    }],
                    colspan: 1,
                    rowspan: 1,
                })
                .collect();

            if is_first_row {
                header = Some(cells);
                is_first_row = false;
            } else {
                rows.push(cells);
            }
        }

        Table {
            caption: None,
            label: None,
            columns,
            header,
            rows,
            foot: None,
            attrs: None,
        }
    }
}

impl Reader for MarkdownReader {
    fn format(&self) -> &str {
        "markdown"
    }

    fn extensions(&self) -> &[&str] {
        &["md", "markdown", "mkd"]
    }

    fn read(&self, input: &str) -> Result<Document> {
        let arena = Arena::new();
        let opts = Self::comrak_options();
        let root = parse_document(&arena, input, &opts);

        // Extract frontmatter before converting content
        let metadata = self.extract_frontmatter(root);
        let mut content = self.convert_node(root);

        // Auto-generate heading IDs (GFM-style slugification)
        auto_id_headings(&mut content);

        Ok(Document {
            metadata,
            content,
            bibliography: None,
            warnings: vec![],
            resources: HashMap::new(),
        })
    }
}

// ─── Heading auto-ID (GFM-style) ────────────────────────────────────────────

/// Walk blocks and assign GFM-style IDs to headings that don't already have one.
/// Duplicate slugs get a `-1`, `-2`, … suffix.
fn auto_id_headings(blocks: &mut [Block]) {
    let mut seen = HashSet::new();
    auto_id_walk(blocks, &mut seen);
}

fn auto_id_walk(blocks: &mut [Block], seen: &mut HashSet<String>) {
    for block in blocks.iter_mut() {
        match block {
            Block::Heading { id, content, .. } => {
                if let Some(ref existing) = id {
                    // Register explicit IDs so auto-generated ones don't collide
                    seen.insert(existing.clone());
                } else {
                    let slug = slugify_inlines(content);
                    if !slug.is_empty() {
                        *id = Some(dedup_slug(slug, seen));
                    }
                }
            }
            Block::BlockQuote { content } => auto_id_walk(content, seen),
            Block::List { items, .. } => {
                for item in items {
                    auto_id_walk(&mut item.content, seen);
                }
            }
            Block::Admonition { content, .. } => auto_id_walk(content, seen),
            Block::Div { content, .. } => auto_id_walk(content, seen),
            Block::FootnoteDef { content, .. } => auto_id_walk(content, seen),
            _ => {}
        }
    }
}

/// Convert inlines to a GFM-style slug:
/// 1. Flatten to plain text (lowercase)
/// 2. Replace spaces/underscores with hyphens
/// 3. Strip anything that isn't alphanumeric or hyphen
/// 4. Collapse consecutive hyphens
fn slugify_inlines(inlines: &[Inline]) -> String {
    let mut text = String::new();
    collect_plain_text(inlines, &mut text);

    let slug: String = text
        .to_lowercase()
        .chars()
        .map(|c| match c {
            ' ' | '_' => '-',
            c if c.is_alphanumeric() || c == '-' => c,
            _ => '\0',
        })
        .filter(|&c| c != '\0')
        .collect();

    // Collapse consecutive hyphens and trim
    let mut result = String::with_capacity(slug.len());
    let mut prev_hyphen = false;
    for c in slug.chars() {
        if c == '-' {
            if !prev_hyphen && !result.is_empty() {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    result.trim_end_matches('-').to_string()
}

/// Recursively collect plain text from inlines.
fn collect_plain_text(inlines: &[Inline], out: &mut String) {
    for inline in inlines {
        match inline {
            Inline::Text { value } => out.push_str(value),
            Inline::Code { value, .. } => out.push_str(value),
            Inline::MathInline { value } => out.push_str(value),
            Inline::Emphasis { content }
            | Inline::Strong { content }
            | Inline::Strikethrough { content }
            | Inline::Superscript { content }
            | Inline::Subscript { content }
            | Inline::SmallCaps { content }
            | Inline::Underline { content }
            | Inline::Link { content, .. }
            | Inline::Span { content, .. } => collect_plain_text(content, out),
            Inline::SoftBreak | Inline::HardBreak => out.push(' '),
            _ => {}
        }
    }
}

/// Ensure uniqueness: if slug is already seen, append `-1`, `-2`, etc.
fn dedup_slug(slug: String, seen: &mut HashSet<String>) -> String {
    if seen.insert(slug.clone()) {
        return slug;
    }
    let mut n = 1u32;
    loop {
        let candidate = format!("{slug}-{n}");
        if seen.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

// ─── Raw attribute parsing ────────────────────────────────────────────────────

/// Parse a raw attribute format specifier: `{=html}`, `{=latex}`, etc.
/// Returns the format name, or `None` if not a valid raw attribute.
fn parse_raw_attribute(info: &str) -> Option<String> {
    let s = info.trim();
    if !s.starts_with("{=") || !s.ends_with('}') {
        return None;
    }
    let fmt = s[2..s.len() - 1].trim().to_string();
    if fmt.is_empty() {
        return None;
    }
    Some(fmt)
}

// ─── Pandoc-style attribute parsing ──────────────────────────────────────────

/// Parse a pandoc-style attribute block: `{#id .class1 .class2 key=val key2="quoted"}`.
///
/// Returns `None` if the string is not a well-formed attribute block (i.e., it
/// contains tokens that aren't `#id`, `.class`, or `key=val`).
fn parse_attr_block(s: &str) -> Option<Attributes> {
    let s = s.trim();
    if !s.starts_with('{') || !s.ends_with('}') {
        return None;
    }
    let inner = s[1..s.len() - 1].trim();
    if inner.is_empty() {
        return Some(Attributes::default());
    }

    let mut attrs = Attributes::default();
    let tokens = tokenize_attr_block(inner);
    if tokens.is_empty() {
        return Some(Attributes::default());
    }

    for token in &tokens {
        if let Some(id) = token.strip_prefix('#') {
            if id.is_empty() {
                return None;
            }
            attrs.id = Some(id.to_string());
        } else if let Some(class) = token.strip_prefix('.') {
            if class.is_empty() {
                return None;
            }
            attrs.classes.push(class.to_string());
        } else if let Some((key, val)) = token.split_once('=') {
            if key.is_empty() {
                return None;
            }
            let val = val.trim_matches('"');
            attrs.key_values.insert(key.to_string(), val.to_string());
        } else {
            // Token doesn't match any valid pattern → not an attribute block
            return None;
        }
    }

    Some(attrs)
}

/// Split an attribute block's inner content into tokens, respecting quoted values.
///
/// `#id .class key="value with spaces"` → `["#id", ".class", "key=\"value with spaces\""]`
fn tokenize_attr_block(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;

    for c in s.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
            current.push(c);
        } else if c.is_whitespace() && !in_quotes {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(c);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// Check if the last inline in `content` ends with a `{...}` attribute block.
/// If found, parse it, strip it from the content, and return the attributes.
fn extract_trailing_attrs(content: &mut Vec<Inline>) -> Option<Attributes> {
    // Check if the last inline is a Text node with a trailing attr block
    let attr_result = if let Some(Inline::Text { value }) = content.last() {
        if let Some(brace_start) = value.rfind('{') {
            let candidate = &value[brace_start..];
            if candidate.ends_with('}') {
                // Normalize smart quotes (comrak replaces " with \u{201c}/\u{201d}
                // when opts.parse.smart is enabled)
                let normalized = normalize_smart_quotes(candidate);
                parse_attr_block(&normalized).map(|attrs| {
                    let remaining = value[..brace_start].trim_end().to_string();
                    (attrs, remaining)
                })
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if let Some((attrs, remaining)) = attr_result {
        if remaining.is_empty() {
            content.pop();
        } else if let Some(Inline::Text { value }) = content.last_mut() {
            *value = remaining;
        }
        Some(attrs)
    } else {
        None
    }
}

/// Replace Unicode smart quotes with ASCII equivalents so the attribute parser
/// can handle values quoted with curly quotes produced by comrak's smart mode.
fn normalize_smart_quotes(s: &str) -> String {
    s.replace(['\u{201c}', '\u{201d}'], "\"") // left/right double
        .replace(['\u{2018}', '\u{2019}'], "'") // left/right single
}

// ─── YAML frontmatter helpers ────────────────────────────────────────────────

/// Parse the `author`/`authors` field which can be:
/// - A single string: `"Jane Doe"`
/// - A list of strings: `["Jane Doe", "John Smith"]`
/// - A list of objects: `[{name: "Jane Doe", affiliation: "MIT"}]`
fn parse_authors(val: &serde_yaml::Value) -> Vec<Author> {
    match val {
        serde_yaml::Value::String(s) => vec![Author {
            name: s.clone(),
            affiliation: None,
            email: None,
            orcid: None,
        }],
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|item| match item {
                serde_yaml::Value::String(s) => Some(Author {
                    name: s.clone(),
                    affiliation: None,
                    email: None,
                    orcid: None,
                }),
                serde_yaml::Value::Mapping(m) => {
                    let name = m
                        .get(serde_yaml::Value::String("name".into()))?
                        .as_str()?
                        .to_string();
                    let affiliation = m
                        .get(serde_yaml::Value::String("affiliation".into()))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let email = m
                        .get(serde_yaml::Value::String("email".into()))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let orcid = m
                        .get(serde_yaml::Value::String("orcid".into()))
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    Some(Author {
                        name,
                        affiliation,
                        email,
                        orcid,
                    })
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Parse a YAML value that should be a list of strings.
fn parse_string_list(val: &serde_yaml::Value) -> Vec<String> {
    match val {
        serde_yaml::Value::Sequence(seq) => seq
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        serde_yaml::Value::String(s) => s.split(',').map(|s| s.trim().to_string()).collect(),
        _ => Vec::new(),
    }
}

/// Convert a serde_yaml::Value to a string, handling numbers and bools.
fn yaml_value_to_string(val: &serde_yaml::Value) -> Option<String> {
    match val {
        serde_yaml::Value::String(s) => Some(s.clone()),
        serde_yaml::Value::Number(n) => Some(n.to_string()),
        serde_yaml::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

// ─── Bracketed span post-processing ─────────────────────────────────────────

/// Walk a `Vec<Inline>` in place and convert any `Text` node that contains
/// the literal pattern `[content]{attrs}` into a `Span`.
///
/// comrak does not understand this syntax, so `[text]{.cls}` arrives as a
/// single `Text("[text]{.cls}")`. We detect and replace it here.
///
/// Only the **last** such pattern in a given `Text` value is converted per
/// pass; the function loops until no more patterns can be found.
fn postprocess_bracketed_spans(inlines: &mut Vec<Inline>) {
    let mut i = 0;
    while i < inlines.len() {
        // Recurse into container inlines first.
        match &mut inlines[i] {
            Inline::Emphasis { content }
            | Inline::Strong { content }
            | Inline::Strikethrough { content }
            | Inline::Superscript { content }
            | Inline::Subscript { content }
            | Inline::SmallCaps { content }
            | Inline::Underline { content }
            | Inline::Span { content, .. }
            | Inline::Link { content, .. } => {
                postprocess_bracketed_spans(content);
            }
            _ => {}
        }

        if let Inline::Text { value } = &inlines[i] {
            if let Some((before, span_text, attrs, after)) = try_parse_bracketed_span(value) {
                let mut replacements: Vec<Inline> = Vec::new();
                if !before.is_empty() {
                    replacements.push(Inline::Text { value: before });
                }
                replacements.push(Inline::Span {
                    content: vec![Inline::Text { value: span_text }],
                    attrs,
                });
                if !after.is_empty() {
                    replacements.push(Inline::Text { value: after });
                }
                let end = i + 1;
                inlines.splice(i..end, replacements);
                // Do NOT advance i — re-check from the same position so we
                // handle multiple patterns in one Text node (the `before`
                // portion may itself contain another span).
                continue;
            }
        }
        i += 1;
    }
}

/// Try to find and parse the pattern `[content]{attrs}` inside `s`.
///
/// Finds the **first** `[` in `s` and the matching `]` followed immediately
/// by `{...}`. Returns `(before, span_text, attrs, after)` on success, or
/// `None` if no valid pattern is found.
fn try_parse_bracketed_span(s: &str) -> Option<(String, String, Attributes, String)> {
    // Find the first '['.
    let open_bracket = s.find('[')?;
    // Find the matching ']' by tracking nesting.
    let rest = &s[open_bracket + 1..];
    let mut depth: usize = 1;
    let mut close_bracket_rel: Option<usize> = None;
    let chars: Vec<char> = rest.chars().collect();
    let mut byte_pos = 0usize;
    for &ch in &chars {
        if ch == '[' {
            depth += 1;
        } else if ch == ']' {
            depth -= 1;
            if depth == 0 {
                close_bracket_rel = Some(byte_pos);
                break;
            }
        }
        byte_pos += ch.len_utf8();
    }
    let close_bracket_rel = close_bracket_rel?;
    let span_text = rest[..close_bracket_rel].to_string();

    // The character immediately after ']' must be '{'.
    let after_bracket = open_bracket + 1 + close_bracket_rel + 1;
    if after_bracket >= s.len() {
        return None;
    }
    if !s[after_bracket..].starts_with('{') {
        return None;
    }

    // Find the matching '}'.
    let brace_start = after_bracket;
    let brace_rest = &s[brace_start..];
    let mut brace_depth: usize = 0;
    let mut brace_end_rel: Option<usize> = None;
    let mut bp = 0usize;
    for ch in brace_rest.chars() {
        if ch == '{' {
            brace_depth += 1;
        } else if ch == '}' {
            brace_depth -= 1;
            if brace_depth == 0 {
                brace_end_rel = Some(bp + ch.len_utf8());
                break;
            }
        }
        bp += ch.len_utf8();
    }
    let brace_end_rel = brace_end_rel?;
    let attr_str = &s[brace_start..brace_start + brace_end_rel];
    let normalized = normalize_smart_quotes(attr_str);
    let attrs = parse_attr_block(&normalized)?;

    let before = s[..open_bracket].to_string();
    let after = s[brace_start + brace_end_rel..].to_string();
    Some((before, span_text, attrs, after))
}

/// Convert a serde_yaml::Value into our MetaValue enum.
fn yaml_to_meta_value(val: &serde_yaml::Value) -> Option<MetaValue> {
    match val {
        serde_yaml::Value::String(s) => Some(MetaValue::String(s.clone())),
        serde_yaml::Value::Bool(b) => Some(MetaValue::Bool(*b)),
        serde_yaml::Value::Number(n) => n.as_f64().map(MetaValue::Number),
        serde_yaml::Value::Sequence(seq) => {
            let items: Vec<MetaValue> = seq.iter().filter_map(yaml_to_meta_value).collect();
            Some(MetaValue::List(items))
        }
        serde_yaml::Value::Mapping(m) => {
            let map: HashMap<String, MetaValue> = m
                .iter()
                .filter_map(|(k, v)| {
                    let key = k.as_str()?.to_string();
                    let val = yaml_to_meta_value(v)?;
                    Some((key, val))
                })
                .collect();
            Some(MetaValue::Map(map))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_paragraph() {
        let reader = MarkdownReader::new();
        let doc = reader.read("Hello, world!").unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Inline::Text { value } => assert_eq!(value, "Hello, world!"),
                    other => panic!("Expected Text, got {:?}", other),
                }
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_heading() {
        let reader = MarkdownReader::new();
        let doc = reader.read("# Title\n\nBody text.").unwrap();
        assert_eq!(doc.content.len(), 2);
        match &doc.content[0] {
            Block::Heading { level, content, .. } => {
                assert_eq!(*level, 1);
                assert_eq!(content.len(), 1);
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn parse_inline_math() {
        let reader = MarkdownReader::new();
        let doc = reader.read("The formula $E = mc^2$ is famous.").unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::Paragraph { content } = &doc.content[0] {
            let has_math = content
                .iter()
                .any(|i| matches!(i, Inline::MathInline { .. }));
            assert!(has_math, "Expected inline math in: {:?}", content);
        }
    }

    #[test]
    fn parse_display_math() {
        let reader = MarkdownReader::new();
        let doc = reader
            .read("Before.\n\n$$\nx^2 + y^2 = z^2\n$$\n\nAfter.")
            .unwrap();
        assert_eq!(
            doc.content.len(),
            3,
            "Expected 3 blocks, got: {:#?}",
            doc.content
        );
        match &doc.content[1] {
            Block::MathBlock { content, label } => {
                assert!(
                    content.contains("x^2 + y^2 = z^2"),
                    "Expected math content, got: {content}"
                );
                assert!(label.is_none());
            }
            other => panic!("Expected MathBlock, got {:?}", other),
        }
    }

    #[test]
    fn parse_code_block() {
        let reader = MarkdownReader::new();
        let doc = reader.read("```rust\nfn main() {}\n```").unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::CodeBlock {
                language, content, ..
            } => {
                assert_eq!(language.as_deref(), Some("rust"));
                assert!(content.contains("fn main()"));
            }
            other => panic!("Expected CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn parse_table() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |";
        let reader = MarkdownReader::new();
        let doc = reader.read(input).unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Table(table) => {
                assert!(table.header.is_some());
                assert_eq!(table.rows.len(), 2);
                assert_eq!(table.columns.len(), 2);
            }
            other => panic!("Expected Table, got {:?}", other),
        }
    }

    #[test]
    fn parse_list() {
        let input = "- Item 1\n- Item 2\n- Item 3";
        let reader = MarkdownReader::new();
        let doc = reader.read(input).unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::List { ordered, items, .. } => {
                assert!(!ordered);
                assert_eq!(items.len(), 3);
            }
            other => panic!("Expected List, got {:?}", other),
        }
    }

    #[test]
    fn reader_trait_metadata() {
        let reader = MarkdownReader::new();
        assert_eq!(reader.format(), "markdown");
        assert!(reader.extensions().contains(&"md"));
    }

    // ─── Frontmatter tests ──────────────────────────────────────────────────

    #[test]
    fn frontmatter_title_and_date() {
        let reader = MarkdownReader::new();
        let doc = reader
            .read("---\ntitle: My Paper\ndate: 2026-03-21\n---\n\nBody text.")
            .unwrap();
        assert_eq!(doc.metadata.title.as_deref(), Some("My Paper"));
        assert_eq!(doc.metadata.date.as_deref(), Some("2026-03-21"));
        // Body should be parsed normally (frontmatter not in content)
        assert_eq!(doc.content.len(), 1);
    }

    #[test]
    fn frontmatter_single_author_string() {
        let reader = MarkdownReader::new();
        let doc = reader.read("---\nauthor: Jane Doe\n---\n\nHello.").unwrap();
        assert_eq!(doc.metadata.authors.len(), 1);
        assert_eq!(doc.metadata.authors[0].name, "Jane Doe");
    }

    #[test]
    fn frontmatter_author_list_of_strings() {
        let reader = MarkdownReader::new();
        let doc = reader
            .read("---\nauthor:\n  - Jane Doe\n  - John Smith\n---\n\nHello.")
            .unwrap();
        assert_eq!(doc.metadata.authors.len(), 2);
        assert_eq!(doc.metadata.authors[0].name, "Jane Doe");
        assert_eq!(doc.metadata.authors[1].name, "John Smith");
    }

    #[test]
    fn frontmatter_author_list_of_objects() {
        let reader = MarkdownReader::new();
        let input = "---\nauthor:\n  - name: Jane Doe\n    affiliation: MIT\n    email: jane@mit.edu\n---\n\nBody.";
        let doc = reader.read(input).unwrap();
        assert_eq!(doc.metadata.authors.len(), 1);
        assert_eq!(doc.metadata.authors[0].name, "Jane Doe");
        assert_eq!(doc.metadata.authors[0].affiliation.as_deref(), Some("MIT"));
        assert_eq!(
            doc.metadata.authors[0].email.as_deref(),
            Some("jane@mit.edu")
        );
    }

    #[test]
    fn frontmatter_abstract_and_keywords() {
        let reader = MarkdownReader::new();
        let input =
            "---\ntitle: Test\nabstract: This is the abstract.\nkeywords:\n  - rust\n  - wasm\n---\n\nBody.";
        let doc = reader.read(input).unwrap();
        let abstract_text = doc.metadata.abstract_text.as_ref().and_then(|blocks| {
            if let Some(Block::Paragraph { content }) = blocks.first() {
                if let Some(Inline::Text { value }) = content.first() {
                    return Some(value.as_str());
                }
            }
            None
        });
        assert_eq!(abstract_text, Some("This is the abstract."));
        assert_eq!(doc.metadata.keywords, vec!["rust", "wasm"]);
    }

    #[test]
    fn frontmatter_custom_fields_preserved() {
        let reader = MarkdownReader::new();
        let input = "---\ntitle: Test\nlang: es\nbibliography: refs.bib\n---\n\nBody.";
        let doc = reader.read(input).unwrap();
        assert_eq!(doc.metadata.title.as_deref(), Some("Test"));
        assert!(doc.metadata.custom.contains_key("lang"));
        assert!(doc.metadata.custom.contains_key("bibliography"));
    }

    #[test]
    fn no_frontmatter_returns_default_metadata() {
        let reader = MarkdownReader::new();
        let doc = reader.read("Just a paragraph.").unwrap();
        assert!(doc.metadata.title.is_none());
        assert!(doc.metadata.authors.is_empty());
    }

    // ─── Auto-ID tests ──────────────────────────────────────────────────────

    #[test]
    fn heading_gets_auto_id() {
        let reader = MarkdownReader::new();
        let doc = reader.read("# Hello World").unwrap();
        match &doc.content[0] {
            Block::Heading { id, .. } => {
                assert_eq!(id.as_deref(), Some("hello-world"));
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn heading_id_strips_punctuation() {
        let reader = MarkdownReader::new();
        let doc = reader.read("# Section 1.2: Methods & Results!").unwrap();
        match &doc.content[0] {
            Block::Heading { id, .. } => {
                assert_eq!(id.as_deref(), Some("section-12-methods-results"));
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn duplicate_headings_get_suffix() {
        let reader = MarkdownReader::new();
        let doc = reader.read("# Intro\n\n## Intro\n\n### Intro").unwrap();
        let ids: Vec<_> = doc
            .content
            .iter()
            .filter_map(|b| match b {
                Block::Heading { id, .. } => id.as_deref(),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["intro", "intro-1", "intro-2"]);
    }

    #[test]
    fn heading_with_inline_formatting_slugifies() {
        let reader = MarkdownReader::new();
        let doc = reader.read("# **Bold** and *italic*").unwrap();
        match &doc.content[0] {
            Block::Heading { id, .. } => {
                assert_eq!(id.as_deref(), Some("bold-and-italic"));
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    // ─── Header attribute tests ─────────────────────────────────────────────

    #[test]
    fn heading_explicit_id() {
        let reader = MarkdownReader::new();
        let doc = reader.read("# Hello {#custom-id}").unwrap();
        match &doc.content[0] {
            Block::Heading {
                id, content, attrs, ..
            } => {
                assert_eq!(id.as_deref(), Some("custom-id"));
                // Content should not include the attr block
                assert_eq!(content.len(), 1);
                if let Inline::Text { value } = &content[0] {
                    assert_eq!(value, "Hello");
                } else {
                    panic!("Expected Text, got {:?}", content[0]);
                }
                // No classes or key-values → attrs is None
                assert!(attrs.is_none());
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn heading_class_attr() {
        let reader = MarkdownReader::new();
        let doc = reader.read("# Warning {.warning}").unwrap();
        match &doc.content[0] {
            Block::Heading { id, attrs, .. } => {
                // No explicit #id → auto-generated
                assert_eq!(id.as_deref(), Some("warning"));
                let a = attrs.as_ref().expect("should have attrs");
                assert_eq!(a.classes, vec!["warning"]);
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn heading_full_attrs() {
        let reader = MarkdownReader::new();
        let doc = reader
            .read("# Section {#sec-intro .important lang=en}")
            .unwrap();
        match &doc.content[0] {
            Block::Heading {
                id, content, attrs, ..
            } => {
                assert_eq!(id.as_deref(), Some("sec-intro"));
                if let Inline::Text { value } = &content[0] {
                    assert_eq!(value, "Section");
                }
                let a = attrs.as_ref().expect("should have attrs");
                assert_eq!(a.classes, vec!["important"]);
                assert_eq!(a.key_values.get("lang").unwrap(), "en");
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn heading_curly_braces_not_attrs() {
        // {world} isn't a valid attr block (no #, ., or key=val)
        let reader = MarkdownReader::new();
        let doc = reader.read("# Hello {world}").unwrap();
        match &doc.content[0] {
            Block::Heading { id, content, .. } => {
                // Should auto-generate ID from full text including {world}
                assert!(id.is_some());
                // Content should still include the curly braces
                if let Inline::Text { value } = &content[0] {
                    assert!(value.contains("{world}"));
                }
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn heading_bold_with_attrs() {
        let reader = MarkdownReader::new();
        let doc = reader.read("# **Bold** heading {#bold-heading}").unwrap();
        match &doc.content[0] {
            Block::Heading { id, content, .. } => {
                assert_eq!(id.as_deref(), Some("bold-heading"));
                // First inline should be Strong
                assert!(matches!(&content[0], Inline::Strong { .. }));
                // Remaining text should be " heading" (no attr block)
                if let Inline::Text { value } = &content[1] {
                    assert_eq!(value, " heading");
                }
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    #[test]
    fn heading_explicit_id_dedup_with_auto() {
        let reader = MarkdownReader::new();
        // First heading has explicit id="intro", second auto-generates to "intro"
        // → should become "intro-1"
        let doc = reader.read("# First {#intro}\n\n## Intro").unwrap();
        let ids: Vec<_> = doc
            .content
            .iter()
            .filter_map(|b| match b {
                Block::Heading { id, .. } => id.as_deref(),
                _ => None,
            })
            .collect();
        assert_eq!(ids, vec!["intro", "intro-1"]);
    }

    #[test]
    fn heading_quoted_attr_value() {
        let reader = MarkdownReader::new();
        let doc = reader
            .read("# Title {#tid data-note=\"some value here\"}")
            .unwrap();
        match &doc.content[0] {
            Block::Heading { id, attrs, .. } => {
                assert_eq!(id.as_deref(), Some("tid"));
                let a = attrs.as_ref().expect("should have attrs");
                assert_eq!(a.key_values.get("data-note").unwrap(), "some value here");
            }
            other => panic!("Expected Heading, got {:?}", other),
        }
    }

    // ─── Fenced code attribute tests ────────────────────────────────────────

    #[test]
    fn code_block_pandoc_attrs_language() {
        let reader = MarkdownReader::new();
        let doc = reader.read("```{.python}\nprint('hi')\n```").unwrap();
        match &doc.content[0] {
            Block::CodeBlock {
                language, attrs, ..
            } => {
                assert_eq!(language.as_deref(), Some("python"));
                let a = attrs.as_ref().expect("should have attrs");
                assert_eq!(a.classes, vec!["python"]);
            }
            other => panic!("Expected CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn code_block_pandoc_attrs_multiple_classes() {
        let reader = MarkdownReader::new();
        let doc = reader.read("```{.python .numberLines}\ncode\n```").unwrap();
        match &doc.content[0] {
            Block::CodeBlock {
                language, attrs, ..
            } => {
                assert_eq!(language.as_deref(), Some("python"));
                let a = attrs.as_ref().expect("should have attrs");
                assert_eq!(a.classes, vec!["python", "numberLines"]);
            }
            other => panic!("Expected CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn code_block_pandoc_attrs_full() {
        let reader = MarkdownReader::new();
        let doc = reader
            .read("```{#my-code .python startFrom=\"5\"}\ncode\n```")
            .unwrap();
        match &doc.content[0] {
            Block::CodeBlock {
                language, attrs, ..
            } => {
                assert_eq!(language.as_deref(), Some("python"));
                let a = attrs.as_ref().expect("should have attrs");
                assert_eq!(a.id.as_deref(), Some("my-code"));
                assert_eq!(a.classes, vec!["python"]);
                assert_eq!(a.key_values.get("startFrom").unwrap(), "5");
            }
            other => panic!("Expected CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn code_block_standard_info_unchanged() {
        // Standard ```rust should still work
        let reader = MarkdownReader::new();
        let doc = reader.read("```rust\nfn main() {}\n```").unwrap();
        match &doc.content[0] {
            Block::CodeBlock {
                language, attrs, ..
            } => {
                assert_eq!(language.as_deref(), Some("rust"));
                assert!(attrs.is_none());
            }
            other => panic!("Expected CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn code_block_empty_attrs() {
        let reader = MarkdownReader::new();
        let doc = reader.read("```{}\ncode\n```").unwrap();
        match &doc.content[0] {
            Block::CodeBlock {
                language, attrs, ..
            } => {
                assert!(language.is_none());
                // Empty attrs still produces Some(Attributes::default())
                assert!(attrs.is_some());
            }
            other => panic!("Expected CodeBlock, got {:?}", other),
        }
    }

    // ─── parse_attr_block unit tests ────────────────────────────────────────

    #[test]
    fn parse_attr_block_basic() {
        let attrs = parse_attr_block("{#my-id .cls1 .cls2 k=v}").unwrap();
        assert_eq!(attrs.id.as_deref(), Some("my-id"));
        assert_eq!(attrs.classes, vec!["cls1", "cls2"]);
        assert_eq!(attrs.key_values.get("k").unwrap(), "v");
    }

    #[test]
    fn parse_attr_block_invalid_token() {
        assert!(parse_attr_block("{world}").is_none());
        assert!(parse_attr_block("{hello world}").is_none());
    }

    #[test]
    fn parse_attr_block_empty() {
        let attrs = parse_attr_block("{}").unwrap();
        assert!(attrs.id.is_none());
        assert!(attrs.classes.is_empty());
    }

    #[test]
    fn parse_attr_block_quoted_value() {
        let attrs = parse_attr_block("{key=\"value with spaces\"}").unwrap();
        assert_eq!(attrs.key_values.get("key").unwrap(), "value with spaces");
    }

    #[test]
    fn parse_attr_block_not_braces() {
        assert!(parse_attr_block("no braces").is_none());
        assert!(parse_attr_block("{unclosed").is_none());
        assert!(parse_attr_block("unclosed}").is_none());
    }

    // ─── Subscript tests ────────────────────────────────────────────────────

    #[test]
    fn subscript_tilde_syntax() {
        let reader = MarkdownReader::new();
        let doc = reader.read("H~2~O").unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::Paragraph { content } = &doc.content[0] {
            // Expect: Text("H"), Subscript([Text("2")]), Text("O")
            assert_eq!(content.len(), 3, "Expected 3 inlines, got: {:#?}", content);
            assert!(matches!(&content[0], Inline::Text { value } if value == "H"));
            match &content[1] {
                Inline::Subscript { content: sub } => {
                    assert_eq!(sub.len(), 1);
                    assert!(matches!(&sub[0], Inline::Text { value } if value == "2"));
                }
                other => panic!("Expected Subscript, got {:?}", other),
            }
            assert!(matches!(&content[2], Inline::Text { value } if value == "O"));
        } else {
            panic!("Expected Paragraph");
        }
    }

    // ─── Superscript tests ─────────────────────────────────────────────────

    #[test]
    fn superscript_caret_syntax() {
        let reader = MarkdownReader::new();
        let doc = reader.read("x^2^").unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::Paragraph { content } = &doc.content[0] {
            // Expect: Text("x"), Superscript([Text("2")])
            assert_eq!(content.len(), 2, "Expected 2 inlines, got: {:#?}", content);
            assert!(matches!(&content[0], Inline::Text { value } if value == "x"));
            match &content[1] {
                Inline::Superscript { content: sup } => {
                    assert_eq!(sup.len(), 1);
                    assert!(matches!(&sup[0], Inline::Text { value } if value == "2"));
                }
                other => panic!("Expected Superscript, got {:?}", other),
            }
        } else {
            panic!("Expected Paragraph");
        }
    }

    // ─── Description list tests ──────────────────────────────────────────────

    #[test]
    fn description_list_basic() {
        let reader = MarkdownReader::new();
        let doc = reader.read("Term\n: Definition here").unwrap();
        let dl = doc
            .content
            .iter()
            .find(|b| matches!(b, Block::DefinitionList { .. }));
        assert!(
            dl.is_some(),
            "Expected DefinitionList, got: {:#?}",
            doc.content
        );
        if let Block::DefinitionList { items } = dl.unwrap() {
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].term.len(), 1);
            assert!(matches!(&items[0].term[0], Inline::Text { value } if value == "Term"));
            assert_eq!(items[0].definitions.len(), 1);
        }
    }

    #[test]
    fn description_list_multiple_items() {
        let reader = MarkdownReader::new();
        let doc = reader.read("Apple\n: A fruit\n\nDog\n: An animal").unwrap();
        let dl = doc
            .content
            .iter()
            .find(|b| matches!(b, Block::DefinitionList { .. }));
        assert!(
            dl.is_some(),
            "Expected DefinitionList, got: {:#?}",
            doc.content
        );
        if let Block::DefinitionList { items } = dl.unwrap() {
            assert_eq!(items.len(), 2, "Expected 2 items, got: {:#?}", items);
        }
    }

    // ─── Bracketed span tests ────────────────────────────────────────────────

    #[test]
    fn bracketed_span_basic() {
        let reader = MarkdownReader::new();
        let doc = reader.read("[text]{.highlight}").unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::Paragraph { content } = &doc.content[0] {
            assert_eq!(content.len(), 1, "Expected 1 inline, got: {:#?}", content);
            match &content[0] {
                Inline::Span {
                    content: inner,
                    attrs,
                } => {
                    assert_eq!(inner.len(), 1);
                    assert!(matches!(&inner[0], Inline::Text { value } if value == "text"));
                    assert_eq!(attrs.classes, vec!["highlight"]);
                    assert!(attrs.id.is_none());
                }
                other => panic!("Expected Span, got {:?}", other),
            }
        } else {
            panic!("Expected Paragraph");
        }
    }

    #[test]
    fn bracketed_span_with_id() {
        let reader = MarkdownReader::new();
        let doc = reader.read("[text]{#myid .cls}").unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::Paragraph { content } = &doc.content[0] {
            assert_eq!(content.len(), 1);
            match &content[0] {
                Inline::Span {
                    content: inner,
                    attrs,
                } => {
                    assert_eq!(inner.len(), 1);
                    assert!(matches!(&inner[0], Inline::Text { value } if value == "text"));
                    assert_eq!(attrs.id.as_deref(), Some("myid"));
                    assert_eq!(attrs.classes, vec!["cls"]);
                }
                other => panic!("Expected Span, got {:?}", other),
            }
        } else {
            panic!("Expected Paragraph");
        }
    }

    #[test]
    fn bracketed_span_in_paragraph() {
        let reader = MarkdownReader::new();
        let doc = reader.read("Before [text]{.cls} after").unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::Paragraph { content } = &doc.content[0] {
            // Expect: Text("Before "), Span, Text(" after")
            assert_eq!(content.len(), 3, "Expected 3 inlines, got: {:#?}", content);
            assert!(matches!(&content[0], Inline::Text { value } if value == "Before "));
            assert!(matches!(&content[1], Inline::Span { .. }));
            assert!(matches!(&content[2], Inline::Text { value } if value == " after"));
        } else {
            panic!("Expected Paragraph");
        }
    }

    #[test]
    fn bracketed_span_invalid_attrs_left_as_text() {
        // {world} is not a valid attr block → should NOT become a Span
        let reader = MarkdownReader::new();
        let doc = reader.read("[text]{world}").unwrap();
        assert_eq!(doc.content.len(), 1);
        if let Block::Paragraph { content } = &doc.content[0] {
            // Should remain as a plain Text node (comrak parses it as text)
            let has_span = content.iter().any(|i| matches!(i, Inline::Span { .. }));
            assert!(!has_span, "Should not have produced a Span: {:#?}", content);
        } else {
            panic!("Expected Paragraph");
        }
    }

    // ─── Raw attribute tests ─────────────────────────────────────────────────

    #[test]
    fn raw_attribute_code_block_html() {
        let reader = MarkdownReader::new();
        let doc = reader
            .read("```{=html}\n<div class=\"custom\">raw html</div>\n```")
            .unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::RawBlock { format, content } => {
                assert_eq!(format, "html");
                assert!(content.contains("<div class=\"custom\">raw html</div>"));
            }
            other => panic!("Expected RawBlock, got {:?}", other),
        }
    }

    #[test]
    fn raw_attribute_code_block_latex() {
        let reader = MarkdownReader::new();
        let doc = reader
            .read("```{=latex}\n\\begin{tikzpicture}\n\\draw (0,0) -- (1,1);\n\\end{tikzpicture}\n```")
            .unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::RawBlock { format, content } => {
                assert_eq!(format, "latex");
                assert!(content.contains("\\begin{tikzpicture}"));
            }
            other => panic!("Expected RawBlock, got {:?}", other),
        }
    }

    #[test]
    fn raw_attribute_empty_format_stays_code_block() {
        let reader = MarkdownReader::new();
        let doc = reader.read("```{=}\nsome content\n```").unwrap();
        assert_eq!(doc.content.len(), 1);
        assert!(
            matches!(&doc.content[0], Block::CodeBlock { .. }),
            "Empty format should stay as CodeBlock, got: {:?}",
            doc.content[0]
        );
    }
}
