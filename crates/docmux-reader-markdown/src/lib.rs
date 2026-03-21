//! # docmux-reader-markdown
//!
//! Markdown reader for docmux. Parses CommonMark + GFM extensions into the
//! docmux AST using [comrak](https://crates.io/crates/comrak) under the hood.

use comrak::{
    nodes::{AstNode, NodeValue},
    parse_document, Arena, Options,
};
use docmux_ast::*;
use docmux_core::{Reader, Result};

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
        // Parse options
        opts.parse.smart = true;
        opts
    }

    /// Convert a comrak AST node tree into our docmux AST blocks.
    fn convert_node<'a>(&self, node: &'a AstNode<'a>) -> Vec<Block> {
        let mut blocks = Vec::new();

        for child in node.children() {
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
                })
            }
            NodeValue::BlockQuote => {
                let content = self.convert_node(node);
                Some(Block::BlockQuote { content })
            }
            NodeValue::List(list) => {
                let ordered = matches!(
                    list.list_type,
                    comrak::nodes::ListType::Ordered
                );
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
                out.push(Inline::FootnoteRef { id: fref.name.clone() });
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

        let content = self.convert_node(root);

        // TODO: extract YAML frontmatter into Metadata
        let metadata = Metadata::default();

        Ok(Document {
            metadata,
            content,
            bibliography: None,
        })
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
            let has_math = content.iter().any(|i| matches!(i, Inline::MathInline { .. }));
            assert!(has_math, "Expected inline math in: {:?}", content);
        }
    }

    #[test]
    fn parse_display_math() {
        let reader = MarkdownReader::new();
        let doc = reader.read("Before.\n\n$$\nx^2 + y^2 = z^2\n$$\n\nAfter.").unwrap();
        assert_eq!(doc.content.len(), 3, "Expected 3 blocks, got: {:#?}", doc.content);
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
            Block::CodeBlock { language, content, .. } => {
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
}
