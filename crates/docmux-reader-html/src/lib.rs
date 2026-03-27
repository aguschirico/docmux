//! # docmux-reader-html
//!
//! HTML reader for docmux. Parses HTML into the docmux AST using
//! [scraper](https://crates.io/crates/scraper) (Servo's html5ever) for
//! browser-grade parsing.
//!
//! Supports both full documents (`<html><body>...`) and HTML fragments.
//! Block-level elements (`<p>`, `<h1>`–`<h6>`, `<pre>`, `<blockquote>`,
//! `<ul>`, `<ol>`, `<hr>`, `<dl>`, `<table>`, `<figure>`) are converted to
//! their corresponding AST nodes. Inline elements are converted with basic
//! coverage (text, `<em>`, `<strong>`, `<code>`, `<a>`, `<br>`, `<sub>`,
//! `<sup>`, `<s>`, `<u>`, `<img>`).

use docmux_ast::{
    Alignment, Attributes, Block, ColumnSpec, DefinitionItem, Document, Image, Inline, ListItem,
    Metadata, Table, TableCell,
};
use docmux_core::{Reader, Result};
use scraper::{ElementRef, Html, Node, Selector};

/// An HTML reader.
#[derive(Debug, Default)]
pub struct HtmlReader;

impl HtmlReader {
    pub fn new() -> Self {
        Self
    }

    /// Determine the root element to walk: `<body>` if present, otherwise the
    /// document/fragment root element.
    fn find_root<'a>(html: &'a Html) -> Option<ElementRef<'a>> {
        let body_sel = Selector::parse("body").unwrap();
        if let Some(body) = html.select(&body_sel).next() {
            return Some(body);
        }
        // Fragment mode: root_element() returns the synthetic html wrapper.
        Some(html.root_element())
    }

    /// Walk direct children of a root element and produce blocks.
    fn convert_children(element: &ElementRef<'_>) -> Vec<Block> {
        let mut blocks = Vec::new();
        for child in element.children() {
            match child.value() {
                Node::Element(_) => {
                    if let Some(el) = ElementRef::wrap(child) {
                        if let Some(block) = Self::convert_element(&el) {
                            blocks.push(block);
                        }
                    }
                }
                Node::Text(text) => {
                    let trimmed = text.text.trim();
                    if !trimmed.is_empty() {
                        blocks.push(Block::Paragraph {
                            content: vec![Inline::Text {
                                value: trimmed.to_string(),
                            }],
                        });
                    }
                }
                _ => {}
            }
        }
        blocks
    }

    /// Convert a single HTML element to a Block.
    fn convert_element(el: &ElementRef<'_>) -> Option<Block> {
        let tag = el.value().name();
        match tag {
            "p" => Some(Block::Paragraph {
                content: Self::convert_inlines(el),
            }),

            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = tag[1..].parse::<u8>().unwrap_or(1);
                let id = el.value().id().map(String::from);
                Some(Block::Heading {
                    level,
                    id,
                    content: Self::convert_inlines(el),
                    attrs: None,
                })
            }

            "pre" => {
                let code_sel = Selector::parse("code").unwrap();
                if let Some(code_el) = el.select(&code_sel).next() {
                    let language = Self::extract_language(&code_el);
                    let content: String = code_el.text().collect();
                    Some(Block::CodeBlock {
                        language,
                        content,
                        caption: None,
                        label: None,
                        attrs: None,
                    })
                } else {
                    // <pre> without <code>
                    let content: String = el.text().collect();
                    Some(Block::CodeBlock {
                        language: None,
                        content,
                        caption: None,
                        label: None,
                        attrs: None,
                    })
                }
            }

            "blockquote" => {
                let content = Self::convert_children(el);
                Some(Block::BlockQuote { content })
            }

            "ul" => {
                let items = Self::convert_list_items(el);
                Some(Block::List {
                    ordered: false,
                    start: None,
                    items,
                    tight: false,
                    style: None,
                    delimiter: None,
                })
            }

            "ol" => {
                let start = el.value().attr("start").and_then(|s| s.parse::<u32>().ok());
                let items = Self::convert_list_items(el);
                Some(Block::List {
                    ordered: true,
                    start,
                    items,
                    tight: false,
                    style: None,
                    delimiter: None,
                })
            }

            "hr" => Some(Block::ThematicBreak),

            "dl" => Some(Self::convert_definition_list(el)),

            "table" => Some(Block::Table(Self::convert_table(el))),

            "figure" => Some(Self::convert_figure(el)),

            "div" => {
                // Convert <div> to block content — recurse into children.
                let content = Self::convert_children(el);
                if content.len() == 1 {
                    // Unwrap single-child divs for cleaner AST.
                    content.into_iter().next()
                } else if content.is_empty() {
                    None
                } else {
                    let attrs = Self::extract_attributes(el);
                    Some(Block::Div { attrs, content })
                }
            }

            // Skip <head>, <script>, <style>, <meta>, <link>, <title>, <nav>,
            // <header>, <footer> etc.
            "head" | "script" | "style" | "meta" | "link" | "title" | "html" => None,

            // For semantic HTML5 sectioning elements, recurse into their children.
            "section" | "article" | "main" | "aside" | "nav" | "header" | "footer" => {
                let content = Self::convert_children(el);
                if content.is_empty() {
                    None
                } else if content.len() == 1 {
                    content.into_iter().next()
                } else {
                    let attrs = Self::extract_attributes(el);
                    Some(Block::Div { attrs, content })
                }
            }

            _ => {
                // Unknown block element — try to extract text as a paragraph.
                let inlines = Self::convert_inlines(el);
                if inlines.is_empty() {
                    None
                } else {
                    Some(Block::Paragraph { content: inlines })
                }
            }
        }
    }

    /// Extract the programming language from a `<code>` element's class attribute.
    /// Supports `class="language-python"` and `class="python"` conventions.
    fn extract_language(code_el: &ElementRef<'_>) -> Option<String> {
        let class = code_el.value().attr("class")?;
        for cls in class.split_whitespace() {
            if let Some(lang) = cls.strip_prefix("language-") {
                if !lang.is_empty() {
                    return Some(lang.to_string());
                }
            }
        }
        // Fall back: if there's a single class that isn't a known utility class,
        // treat it as the language.
        let classes: Vec<&str> = class.split_whitespace().collect();
        if classes.len() == 1 && !classes[0].starts_with("highlight") {
            return Some(classes[0].to_string());
        }
        None
    }

    /// Extract generic attributes (id, classes) from an element.
    fn extract_attributes(el: &ElementRef<'_>) -> Attributes {
        let id = el.value().id().map(String::from);
        let classes: Vec<String> = el.value().classes().map(String::from).collect();
        Attributes {
            id,
            classes,
            key_values: std::collections::HashMap::new(),
        }
    }

    /// Convert `<li>` children of a list element into `ListItem`s.
    fn convert_list_items(list_el: &ElementRef<'_>) -> Vec<ListItem> {
        let li_sel = Selector::parse("li").unwrap();
        let mut items = Vec::new();
        // Only select direct-child <li> elements, not nested ones.
        for li in list_el.child_elements() {
            if li.value().name() != "li" {
                continue;
            }
            // Check for task list checkbox
            let checked = Self::detect_checkbox(&li);
            let content = Self::convert_li_content(&li);
            items.push(ListItem { checked, content });
        }
        // Fall back to select if child_elements missed them (shouldn't happen).
        if items.is_empty() {
            for li in list_el.select(&li_sel) {
                let checked = Self::detect_checkbox(&li);
                let content = Self::convert_li_content(&li);
                items.push(ListItem { checked, content });
            }
        }
        items
    }

    /// Detect a task-list checkbox in a `<li>` element.
    fn detect_checkbox(li: &ElementRef<'_>) -> Option<bool> {
        let input_sel = Selector::parse("input[type=\"checkbox\"]").unwrap();
        if let Some(input) = li.select(&input_sel).next() {
            let checked = input.value().attr("checked").is_some();
            Some(checked)
        } else {
            None
        }
    }

    /// Convert the content of a `<li>` element.
    /// If the li contains block elements (p, ul, ol, etc.), preserve them.
    /// Otherwise, wrap inline content in a paragraph.
    fn convert_li_content(li: &ElementRef<'_>) -> Vec<Block> {
        let has_block_children = li.child_elements().any(|child| {
            matches!(
                child.value().name(),
                "p" | "ul" | "ol" | "blockquote" | "pre" | "div" | "table" | "hr" | "dl"
            )
        });

        if has_block_children {
            Self::convert_children(li)
        } else {
            // Wrap inline content in a paragraph.
            let inlines = Self::convert_inlines(li);
            if inlines.is_empty() {
                Vec::new()
            } else {
                vec![Block::Paragraph { content: inlines }]
            }
        }
    }

    /// Convert a `<dl>` element to a `Block::DefinitionList`.
    fn convert_definition_list(dl: &ElementRef<'_>) -> Block {
        let mut items: Vec<DefinitionItem> = Vec::new();
        let mut current_term: Option<Vec<Inline>> = None;
        let mut current_defs: Vec<Vec<Block>> = Vec::new();

        for child in dl.child_elements() {
            match child.value().name() {
                "dt" => {
                    // Flush previous item if we have one.
                    if let Some(term) = current_term.take() {
                        items.push(DefinitionItem {
                            term,
                            definitions: std::mem::take(&mut current_defs),
                        });
                    }
                    current_term = Some(Self::convert_inlines(&child));
                }
                "dd" => {
                    // If no term yet, create an empty one.
                    if current_term.is_none() {
                        current_term = Some(Vec::new());
                    }
                    let blocks = Self::convert_children(&child);
                    if blocks.is_empty() {
                        // If dd has only inline content, wrap in paragraph.
                        let inlines = Self::convert_inlines(&child);
                        if !inlines.is_empty() {
                            current_defs.push(vec![Block::Paragraph { content: inlines }]);
                        }
                    } else {
                        current_defs.push(blocks);
                    }
                }
                _ => {}
            }
        }

        // Flush last item.
        if let Some(term) = current_term {
            items.push(DefinitionItem {
                term,
                definitions: current_defs,
            });
        }

        Block::DefinitionList { items }
    }

    /// Convert a `<table>` element.
    fn convert_table(table_el: &ElementRef<'_>) -> Table {
        let caption = Self::extract_table_caption(table_el);

        let thead_sel = Selector::parse("thead").unwrap();
        let tbody_sel = Selector::parse("tbody").unwrap();
        let tfoot_sel = Selector::parse("tfoot").unwrap();
        let tr_sel = Selector::parse("tr").unwrap();

        // Extract header rows.
        let header = if let Some(thead) = table_el.select(&thead_sel).next() {
            thead
                .select(&tr_sel)
                .next()
                .map(|tr| Self::convert_row(&tr))
        } else {
            None
        };

        // Extract body rows.
        let mut rows = Vec::new();
        if let Some(tbody) = table_el.select(&tbody_sel).next() {
            for tr in tbody.select(&tr_sel) {
                rows.push(Self::convert_row(&tr));
            }
        } else {
            // No explicit tbody — collect <tr> children, skip first if we used
            // it as header.
            let all_rows: Vec<_> = table_el.select(&tr_sel).collect();
            let skip = if header.is_some() { 1 } else { 0 };
            for tr in all_rows.into_iter().skip(skip) {
                rows.push(Self::convert_row(&tr));
            }
        }

        // Extract footer.
        let foot = table_el.select(&tfoot_sel).next().and_then(|tfoot| {
            tfoot
                .select(&tr_sel)
                .next()
                .map(|tr| Self::convert_row(&tr))
        });

        // Determine number of columns from header or first row.
        let num_cols = header
            .as_ref()
            .map(|h| h.len())
            .or_else(|| rows.first().map(|r| r.len()))
            .unwrap_or(0);

        let columns = vec![
            ColumnSpec {
                alignment: Alignment::Default,
                width: None,
            };
            num_cols
        ];

        Table {
            caption,
            label: None,
            columns,
            header,
            rows,
            foot,
            attrs: None,
        }
    }

    /// Extract `<caption>` from a table.
    fn extract_table_caption(table_el: &ElementRef<'_>) -> Option<Vec<Inline>> {
        let caption_sel = Selector::parse("caption").unwrap();
        table_el
            .select(&caption_sel)
            .next()
            .map(|cap| Self::convert_inlines(&cap))
    }

    /// Convert a `<tr>` to a vector of `TableCell`s.
    fn convert_row(tr: &ElementRef<'_>) -> Vec<TableCell> {
        let td_sel = Selector::parse("td, th").unwrap();
        tr.select(&td_sel)
            .map(|cell| {
                let colspan = cell
                    .value()
                    .attr("colspan")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1);
                let rowspan = cell
                    .value()
                    .attr("rowspan")
                    .and_then(|s| s.parse::<u32>().ok())
                    .unwrap_or(1);
                let content = vec![Block::Paragraph {
                    content: Self::convert_inlines(&cell),
                }];
                TableCell {
                    content,
                    colspan,
                    rowspan,
                }
            })
            .collect()
    }

    /// Convert a `<figure>` element.
    fn convert_figure(figure_el: &ElementRef<'_>) -> Block {
        let img_sel = Selector::parse("img").unwrap();
        let figcaption_sel = Selector::parse("figcaption").unwrap();

        let image = if let Some(img) = figure_el.select(&img_sel).next() {
            let url = img.value().attr("src").unwrap_or("").to_string();
            let alt_text = img.value().attr("alt").unwrap_or("").to_string();
            let title = img.value().attr("title").map(String::from);
            let alt = if alt_text.is_empty() {
                Vec::new()
            } else {
                vec![Inline::Text { value: alt_text }]
            };
            Image {
                url,
                alt,
                title,
                attrs: None,
            }
        } else {
            Image {
                url: String::new(),
                alt: Vec::new(),
                title: None,
                attrs: None,
            }
        };

        let caption = figure_el
            .select(&figcaption_sel)
            .next()
            .map(|cap| Self::convert_inlines(&cap));

        let label = figure_el.value().id().map(String::from);

        Block::Figure {
            image,
            caption,
            label,
            attrs: None,
        }
    }

    /// Convert inline content of an element, recursively processing child nodes.
    fn convert_inlines(el: &ElementRef<'_>) -> Vec<Inline> {
        let mut inlines = Vec::new();
        for child in el.children() {
            match child.value() {
                Node::Text(text) => {
                    let s: &str = text;
                    if !s.is_empty() {
                        inlines.push(Inline::Text {
                            value: s.to_string(),
                        });
                    }
                }
                Node::Element(_) => {
                    if let Some(child_el) = ElementRef::wrap(child) {
                        let tag = child_el.value().name();
                        match tag {
                            "em" | "i" => {
                                inlines.push(Inline::Emphasis {
                                    content: Self::convert_inlines(&child_el),
                                });
                            }
                            "strong" | "b" => {
                                inlines.push(Inline::Strong {
                                    content: Self::convert_inlines(&child_el),
                                });
                            }
                            "code" => {
                                let value: String = child_el.text().collect();
                                inlines.push(Inline::Code { value, attrs: None });
                            }
                            "a" => {
                                let url = child_el.value().attr("href").unwrap_or("").to_string();
                                let title = child_el.value().attr("title").map(String::from);
                                inlines.push(Inline::Link {
                                    url,
                                    title,
                                    content: Self::convert_inlines(&child_el),
                                    attrs: None,
                                });
                            }
                            "br" => {
                                inlines.push(Inline::HardBreak);
                            }
                            "img" => {
                                let url = child_el.value().attr("src").unwrap_or("").to_string();
                                let alt_text =
                                    child_el.value().attr("alt").unwrap_or("").to_string();
                                let title = child_el.value().attr("title").map(String::from);
                                let alt = if alt_text.is_empty() {
                                    Vec::new()
                                } else {
                                    vec![Inline::Text { value: alt_text }]
                                };
                                inlines.push(Inline::Image(Image {
                                    url,
                                    alt,
                                    title,
                                    attrs: None,
                                }));
                            }
                            "sub" => {
                                inlines.push(Inline::Subscript {
                                    content: Self::convert_inlines(&child_el),
                                });
                            }
                            "sup" => {
                                inlines.push(Inline::Superscript {
                                    content: Self::convert_inlines(&child_el),
                                });
                            }
                            "s" | "del" | "strike" => {
                                inlines.push(Inline::Strikethrough {
                                    content: Self::convert_inlines(&child_el),
                                });
                            }
                            "u" | "ins" => {
                                inlines.push(Inline::Underline {
                                    content: Self::convert_inlines(&child_el),
                                });
                            }
                            "span" => {
                                let id = child_el.value().attr("id").map(String::from);
                                let classes: Vec<String> = child_el
                                    .value()
                                    .attr("class")
                                    .map(|c| c.split_whitespace().map(String::from).collect())
                                    .unwrap_or_default();
                                if id.is_some() || !classes.is_empty() {
                                    inlines.push(Inline::Span {
                                        content: Self::convert_inlines(&child_el),
                                        attrs: Attributes {
                                            id,
                                            classes,
                                            key_values: std::collections::HashMap::new(),
                                        },
                                    });
                                } else {
                                    inlines.extend(Self::convert_inlines(&child_el));
                                }
                            }
                            _ => {
                                // Unknown inline element — extract text content.
                                let inner = Self::convert_inlines(&child_el);
                                inlines.extend(inner);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        inlines
    }
}

impl Reader for HtmlReader {
    fn format(&self) -> &str {
        "html"
    }

    fn extensions(&self) -> &[&str] {
        &["html", "htm"]
    }

    fn read(&self, input: &str) -> Result<Document> {
        let html = Html::parse_document(input);
        let root = Self::find_root(&html);
        let blocks = match root {
            Some(root_el) => Self::convert_children(&root_el),
            None => Vec::new(),
        };

        Ok(Document {
            metadata: Metadata::default(),
            content: blocks,
            bibliography: None,
            warnings: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_ast::Block;

    #[test]
    fn reader_trait_metadata() {
        let reader = HtmlReader::new();
        assert_eq!(reader.format(), "html");
        assert!(reader.extensions().contains(&"html"));
        assert!(reader.extensions().contains(&"htm"));
    }

    #[test]
    fn parse_paragraph() {
        let reader = HtmlReader::new();
        let doc = reader.read("<p>Hello</p>").unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Inline::Text { value } => assert_eq!(value, "Hello"),
                    other => panic!("Expected Text, got {:?}", other),
                }
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_headings() {
        let reader = HtmlReader::new();
        let html = r#"
            <h1 id="intro">Introduction</h1>
            <h2>Background</h2>
            <h3>Details</h3>
            <h4>Sub-details</h4>
            <h5>Minor</h5>
            <h6>Tiny</h6>
        "#;
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 6);

        // h1 with id
        match &doc.content[0] {
            Block::Heading {
                level, id, content, ..
            } => {
                assert_eq!(*level, 1);
                assert_eq!(id.as_deref(), Some("intro"));
                match &content[0] {
                    Inline::Text { value } => assert_eq!(value, "Introduction"),
                    other => panic!("Expected Text, got {:?}", other),
                }
            }
            other => panic!("Expected Heading, got {:?}", other),
        }

        // h2 without id
        match &doc.content[1] {
            Block::Heading { level, id, .. } => {
                assert_eq!(*level, 2);
                assert!(id.is_none());
            }
            other => panic!("Expected Heading, got {:?}", other),
        }

        // Verify levels 3-6
        for (i, expected_level) in (3u8..=6).enumerate() {
            match &doc.content[i + 2] {
                Block::Heading { level, .. } => assert_eq!(*level, expected_level),
                other => panic!("Expected Heading level {}, got {:?}", expected_level, other),
            }
        }
    }

    #[test]
    fn parse_code_block() {
        let reader = HtmlReader::new();
        let html = r#"<pre><code class="language-python">def hello():
    print("Hello")</code></pre>"#;
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::CodeBlock {
                language, content, ..
            } => {
                assert_eq!(language.as_deref(), Some("python"));
                assert!(content.contains("def hello()"));
                assert!(content.contains("print"));
            }
            other => panic!("Expected CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn parse_code_block_no_lang() {
        let reader = HtmlReader::new();
        let html = "<pre><code>some code here</code></pre>";
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::CodeBlock {
                language, content, ..
            } => {
                assert!(language.is_none());
                assert_eq!(content, "some code here");
            }
            other => panic!("Expected CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn parse_blockquote() {
        let reader = HtmlReader::new();
        let html = "<blockquote><p>To be or not to be.</p></blockquote>";
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::BlockQuote { content } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Paragraph { content } => {
                        assert_eq!(content.len(), 1);
                        match &content[0] {
                            Inline::Text { value } => {
                                assert_eq!(value, "To be or not to be.")
                            }
                            other => panic!("Expected Text, got {:?}", other),
                        }
                    }
                    other => panic!("Expected Paragraph inside BlockQuote, got {:?}", other),
                }
            }
            other => panic!("Expected BlockQuote, got {:?}", other),
        }
    }

    #[test]
    fn parse_unordered_list() {
        let reader = HtmlReader::new();
        let html = "<ul><li>Alpha</li><li>Beta</li><li>Gamma</li></ul>";
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::List {
                ordered,
                start,
                items,
                ..
            } => {
                assert!(!ordered);
                assert!(start.is_none());
                assert_eq!(items.len(), 3);
                // Check first item content.
                match &items[0].content[0] {
                    Block::Paragraph { content } => match &content[0] {
                        Inline::Text { value } => assert_eq!(value, "Alpha"),
                        other => panic!("Expected Text, got {:?}", other),
                    },
                    other => panic!("Expected Paragraph in ListItem, got {:?}", other),
                }
                assert!(items[0].checked.is_none());
            }
            other => panic!("Expected List, got {:?}", other),
        }
    }

    #[test]
    fn parse_ordered_list() {
        let reader = HtmlReader::new();
        let html = r#"<ol start="3"><li>Third</li><li>Fourth</li></ol>"#;
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::List {
                ordered,
                start,
                items,
                ..
            } => {
                assert!(ordered);
                assert_eq!(*start, Some(3));
                assert_eq!(items.len(), 2);
                match &items[0].content[0] {
                    Block::Paragraph { content } => match &content[0] {
                        Inline::Text { value } => assert_eq!(value, "Third"),
                        other => panic!("Expected Text, got {:?}", other),
                    },
                    other => panic!("Expected Paragraph in ListItem, got {:?}", other),
                }
            }
            other => panic!("Expected List, got {:?}", other),
        }
    }

    #[test]
    fn parse_thematic_break() {
        let reader = HtmlReader::new();
        let doc = reader.read("<hr>").unwrap();
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::ThematicBreak));
    }

    #[test]
    fn parse_definition_list() {
        let reader = HtmlReader::new();
        let html = "<dl><dt>Term 1</dt><dd>Definition 1</dd><dt>Term 2</dt><dd>Definition 2a</dd><dd>Definition 2b</dd></dl>";
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::DefinitionList { items } => {
                assert_eq!(items.len(), 2);

                // First item: one term, one definition.
                match &items[0].term[0] {
                    Inline::Text { value } => assert_eq!(value, "Term 1"),
                    other => panic!("Expected Text, got {:?}", other),
                }
                assert_eq!(items[0].definitions.len(), 1);

                // Second item: one term, two definitions.
                match &items[1].term[0] {
                    Inline::Text { value } => assert_eq!(value, "Term 2"),
                    other => panic!("Expected Text, got {:?}", other),
                }
                assert_eq!(items[1].definitions.len(), 2);
            }
            other => panic!("Expected DefinitionList, got {:?}", other),
        }
    }

    #[test]
    fn parse_inline_emphasis_and_strong() {
        let reader = HtmlReader::new();
        let html = "<p>This is <em>emphasized</em> and <strong>bold</strong> text.</p>";
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert_eq!(content.len(), 5);
                assert!(matches!(&content[1], Inline::Emphasis { .. }));
                assert!(matches!(&content[3], Inline::Strong { .. }));
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_inline_link() {
        let reader = HtmlReader::new();
        let html = r#"<p>Visit <a href="https://example.com" title="Example">our site</a>.</p>"#;
        let doc = reader.read(html).unwrap();
        match &doc.content[0] {
            Block::Paragraph { content } => {
                let link = content.iter().find(|i| matches!(i, Inline::Link { .. }));
                assert!(link.is_some());
                match link.unwrap() {
                    Inline::Link {
                        url,
                        title,
                        content,
                        ..
                    } => {
                        assert_eq!(url, "https://example.com");
                        assert_eq!(title.as_deref(), Some("Example"));
                        match &content[0] {
                            Inline::Text { value } => assert_eq!(value, "our site"),
                            other => panic!("Expected Text, got {:?}", other),
                        }
                    }
                    other => panic!("Expected Link, got {:?}", other),
                }
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_full_document() {
        let reader = HtmlReader::new();
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test</title></head>
<body>
    <h1>Hello</h1>
    <p>World</p>
</body>
</html>"#;
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 2);
        assert!(matches!(&doc.content[0], Block::Heading { level: 1, .. }));
        assert!(matches!(&doc.content[1], Block::Paragraph { .. }));
    }

    #[test]
    fn parse_fragment() {
        let reader = HtmlReader::new();
        let html = "<p>Just a fragment</p>";
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 1);
        assert!(matches!(&doc.content[0], Block::Paragraph { .. }));
    }

    #[test]
    fn empty_input() {
        let reader = HtmlReader::new();
        let doc = reader.read("").unwrap();
        assert!(doc.content.is_empty());
    }

    #[test]
    fn parse_table() {
        let reader = HtmlReader::new();
        let html = r#"<table>
            <thead><tr><th>Name</th><th>Value</th></tr></thead>
            <tbody>
                <tr><td>Pi</td><td>3.14</td></tr>
                <tr><td>E</td><td>2.72</td></tr>
            </tbody>
        </table>"#;
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Table(table) => {
                assert!(table.header.is_some());
                assert_eq!(table.header.as_ref().unwrap().len(), 2);
                assert_eq!(table.rows.len(), 2);
                assert_eq!(table.columns.len(), 2);
            }
            other => panic!("Expected Table, got {:?}", other),
        }
    }

    #[test]
    fn parse_figure() {
        let reader = HtmlReader::new();
        let html = r#"<figure id="fig-photo">
            <img src="photo.jpg" alt="A photo" title="My Photo">
            <figcaption>A beautiful photo</figcaption>
        </figure>"#;
        let doc = reader.read(html).unwrap();
        assert_eq!(doc.content.len(), 1);
        match &doc.content[0] {
            Block::Figure {
                image,
                caption,
                label,
                ..
            } => {
                assert_eq!(image.url, "photo.jpg");
                assert_eq!(image.title.as_deref(), Some("My Photo"));
                assert!(caption.is_some());
                assert_eq!(label.as_deref(), Some("fig-photo"));
            }
            other => panic!("Expected Figure, got {:?}", other),
        }
    }

    #[test]
    fn parse_hard_break() {
        let reader = HtmlReader::new();
        let html = "<p>Line one<br>Line two</p>";
        let doc = reader.read(html).unwrap();
        match &doc.content[0] {
            Block::Paragraph { content } => {
                assert!(content.iter().any(|i| matches!(i, Inline::HardBreak)));
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn parse_inline_strikethrough() {
        let r = HtmlReader::new();
        let doc = r
            .read("<p><del>deleted</del> and <s>struck</s></p>")
            .unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[0], Inline::Strikethrough { .. }));
            assert!(matches!(&content[2], Inline::Strikethrough { .. }));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_inline_underline() {
        let r = HtmlReader::new();
        let doc = r.read("<p><u>underlined</u></p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[0], Inline::Underline { .. }));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_inline_code() {
        let r = HtmlReader::new();
        let doc = r.read("<p>Use <code>fn main()</code> here</p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[1], Inline::Code { .. }));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_inline_image() {
        let r = HtmlReader::new();
        let doc = r
            .read("<p><img src=\"photo.jpg\" alt=\"A photo\"></p>")
            .unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(matches!(&content[0], Inline::Image(_)));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_sub_sup() {
        let r = HtmlReader::new();
        let doc = r.read("<p>H<sub>2</sub>O is x<sup>2</sup></p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            assert!(content
                .iter()
                .any(|i| matches!(i, Inline::Subscript { .. })));
            assert!(content
                .iter()
                .any(|i| matches!(i, Inline::Superscript { .. })));
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_span_with_attrs() {
        let r = HtmlReader::new();
        let doc = r
            .read("<p><span class=\"highlight\" id=\"s1\">text</span></p>")
            .unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            if let Inline::Span { attrs, .. } = &content[0] {
                assert_eq!(attrs.id.as_deref(), Some("s1"));
                assert!(attrs.classes.contains(&"highlight".to_string()));
            } else {
                panic!("expected span, got {:?}", content[0]);
            }
        } else {
            panic!("expected paragraph");
        }
    }

    #[test]
    fn parse_span_without_attrs_flattens() {
        let r = HtmlReader::new();
        let doc = r.read("<p><span>just text</span></p>").unwrap();
        if let Block::Paragraph { content } = &doc.content[0] {
            // Should flatten to Text, not Span
            assert!(matches!(&content[0], Inline::Text { .. }));
        } else {
            panic!("expected paragraph");
        }
    }
}
