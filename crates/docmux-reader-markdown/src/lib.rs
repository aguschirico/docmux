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
use std::collections::HashMap;

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
                    metadata.abstract_text = val.as_str().map(String::from);
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
                let content = self.collect_inlines(node);
                Some(Block::Heading {
                    level: h.level,
                    id: None, // Could compute from content
                    content,
                    attrs: None,
                })
            }
            NodeValue::CodeBlock(cb) => {
                let language = if cb.info.is_empty() {
                    None
                } else {
                    Some(cb.info.clone())
                };
                Some(Block::CodeBlock {
                    language,
                    content: cb.literal.clone(),
                    caption: None,
                    label: None,
                    attrs: None,
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

    /// Collect inline children of a node.
    fn collect_inlines<'a>(&self, node: &'a AstNode<'a>) -> Vec<Inline> {
        let mut inlines = Vec::new();
        for child in node.children() {
            self.node_to_inlines(child, &mut inlines);
        }
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
                });
            }
            NodeValue::Image(img) => {
                // Collect alt text from children
                let alt_inlines = self.collect_inlines(node);
                let alt = alt_inlines
                    .iter()
                    .map(|i| match i {
                        Inline::Text { value } => value.as_str(),
                        _ => "",
                    })
                    .collect::<String>();
                out.push(Inline::Image(Image {
                    url: img.url.clone(),
                    alt,
                    title: if img.title.is_empty() {
                        None
                    } else {
                        Some(img.title.clone())
                    },
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
        let content = self.convert_node(root);

        Ok(Document {
            metadata,
            content,
            bibliography: None,
            warnings: vec![],
        })
    }
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
        assert_eq!(
            doc.metadata.abstract_text.as_deref(),
            Some("This is the abstract.")
        );
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
}
