//! # docmux-ast
//!
//! Abstract syntax tree types for the docmux document converter.
//!
//! This crate defines the intermediate representation used by all docmux
//! readers and writers. The AST is designed to be:
//!
//! - **Format-agnostic**: captures document structure without being tied to
//!   any specific input or output format
//! - **Rich**: math, citations, cross-references, and admonitions are
//!   first-class nodes (not raw inline hacks)
//! - **Owned**: all strings are owned (`String`), keeping lifetimes simple
//!   and the public API ergonomic

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Document ────────────────────────────────────────────────────────────────

/// A diagnostic warning emitted during parsing (e.g. unrecognized commands).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParseWarning {
    pub line: usize,
    pub message: String,
}

/// Binary resource embedded in the document (e.g. an image from a DOCX).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceData {
    pub mime_type: String,
    /// Raw bytes — skipped during serialization to keep JSON output clean.
    #[serde(skip)]
    pub data: Vec<u8>,
}

/// A complete document: metadata + content + optional bibliography.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Document {
    pub metadata: Metadata,
    pub content: Vec<Block>,
    pub bibliography: Option<Bibliography>,
    #[serde(default)]
    pub warnings: Vec<ParseWarning>,
    /// Embedded binary resources keyed by relative path (e.g. "media/image1.png").
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub resources: HashMap<String, ResourceData>,
}

// ─── Metadata ────────────────────────────────────────────────────────────────

/// Document metadata, typically from YAML/TOML front matter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    pub title: Option<String>,
    pub authors: Vec<Author>,
    pub date: Option<String>,
    /// Formatted abstract (as block content, not plain text).
    pub abstract_text: Option<Vec<Block>>,
    pub keywords: Vec<String>,
    /// Arbitrary key-value pairs not captured by the typed fields above.
    #[serde(default)]
    pub custom: HashMap<String, MetaValue>,
}

/// An author with optional academic metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub affiliation: Option<String>,
    pub email: Option<String>,
    pub orcid: Option<String>,
}

/// A dynamically-typed metadata value (for front-matter fields we don't
/// model explicitly).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MetaValue {
    String(String),
    Bool(bool),
    Number(f64),
    List(Vec<MetaValue>),
    Map(HashMap<String, MetaValue>),
}

// ─── Block-level nodes ───────────────────────────────────────────────────────

/// A block-level element in the document tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Block {
    /// A paragraph of inline content.
    Paragraph { content: Vec<Inline> },

    /// A section heading (levels 1–6).
    Heading {
        level: u8,
        id: Option<String>,
        content: Vec<Inline>,
        /// Extra attributes (classes, key-value pairs) from source format.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attrs: Option<Attributes>,
    },

    /// A fenced or indented code block.
    CodeBlock {
        language: Option<String>,
        content: String,
        caption: Option<Vec<Inline>>,
        label: Option<String>,
        /// Extra attributes (classes, key-value pairs) from source format.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attrs: Option<Attributes>,
    },

    /// A display-math block (e.g. `$$…$$`).
    MathBlock {
        content: String,
        label: Option<String>,
    },

    /// A block quotation.
    BlockQuote { content: Vec<Block> },

    /// An ordered or unordered list.
    List {
        ordered: bool,
        start: Option<u32>,
        items: Vec<ListItem>,
        /// Whether items are tight (no `<p>` wrapping) or loose.
        #[serde(default)]
        tight: bool,
        /// Number style for ordered lists (e.g. decimal, lower-alpha).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        style: Option<ListStyle>,
        /// Delimiter for ordered lists (e.g. period, paren).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        delimiter: Option<ListDelim>,
    },

    /// A table with optional caption and column specs.
    Table(Table),

    /// A figure: image + optional caption and label.
    Figure {
        image: Image,
        caption: Option<Vec<Inline>>,
        label: Option<String>,
        /// Extra attributes (classes, key-value pairs) from source format.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attrs: Option<Attributes>,
    },

    /// A thematic break (`---`).
    ThematicBreak,

    /// Raw content in a specific output format (pass-through).
    RawBlock { format: String, content: String },

    /// An admonition box (note, warning, tip, etc.).
    /// First-class support for MyST-style directives.
    Admonition {
        kind: AdmonitionKind,
        title: Option<Vec<Inline>>,
        content: Vec<Block>,
    },

    /// A definition list.
    DefinitionList { items: Vec<DefinitionItem> },

    /// A footnote definition (referenced by `Inline::FootnoteRef`).
    FootnoteDef { id: String, content: Vec<Block> },

    /// A generic block container with attributes (id, classes, key-value pairs).
    /// Used for fenced divs, MyST directives, and arbitrary block wrappers.
    Div {
        attrs: Attributes,
        content: Vec<Block>,
    },
}

/// A single item inside a `Block::List`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItem {
    /// `None` = normal list item; `Some(true/false)` = task-list checkbox.
    pub checked: Option<bool>,
    pub content: Vec<Block>,
}

/// Number style for ordered lists.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ListStyle {
    /// 1, 2, 3, …
    Decimal,
    /// a, b, c, …
    LowerAlpha,
    /// A, B, C, …
    UpperAlpha,
    /// i, ii, iii, …
    LowerRoman,
    /// I, II, III, …
    UpperRoman,
}

/// Delimiter style for ordered list markers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ListDelim {
    /// `1.`
    Period,
    /// `1)`
    OneParen,
    /// `(1)`
    TwoParens,
}

// ─── Table ───────────────────────────────────────────────────────────────────

/// A table with optional caption, column specs, header row, body rows, and footer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Table {
    pub caption: Option<Vec<Inline>>,
    pub label: Option<String>,
    pub columns: Vec<ColumnSpec>,
    pub header: Option<Vec<TableCell>>,
    pub rows: Vec<Vec<TableCell>>,
    /// Optional footer row (e.g. totals, summaries).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foot: Option<Vec<TableCell>>,
    /// Extra attributes (id, classes, key-value pairs) from source format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attrs: Option<Attributes>,
}

/// Column specification: alignment + optional relative width.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub alignment: Alignment,
    /// Relative width as a fraction of total table width (0.0–1.0).
    pub width: Option<f32>,
}

/// Text alignment.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum Alignment {
    #[default]
    Default,
    Left,
    Center,
    Right,
}

/// A single table cell, supporting col/row span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableCell {
    pub content: Vec<Block>,
    #[serde(default = "one")]
    pub colspan: u32,
    #[serde(default = "one")]
    pub rowspan: u32,
}

fn one() -> u32 {
    1
}

// ─── Admonition ──────────────────────────────────────────────────────────────

/// The kind of admonition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AdmonitionKind {
    Note,
    Warning,
    Tip,
    Important,
    Caution,
    Custom(String),
}

// ─── Definition list ─────────────────────────────────────────────────────────

/// A term + one or more definitions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionItem {
    pub term: Vec<Inline>,
    pub definitions: Vec<Vec<Block>>,
}

// ─── Inline-level nodes ──────────────────────────────────────────────────────

/// An inline-level element.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Inline {
    /// Plain text.
    Text { value: String },

    /// Emphasized (italic) content.
    Emphasis { content: Vec<Inline> },

    /// Strong (bold) content.
    Strong { content: Vec<Inline> },

    /// Struck-through content.
    Strikethrough { content: Vec<Inline> },

    /// Inline code.
    Code {
        value: String,
        /// Extra attributes (classes, key-value pairs) from source format.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attrs: Option<Attributes>,
    },

    /// Inline math (e.g. `$x^2$`).
    MathInline { value: String },

    /// A hyperlink.
    Link {
        url: String,
        title: Option<String>,
        content: Vec<Inline>,
        /// Extra attributes (classes, key-value pairs) from source format.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attrs: Option<Attributes>,
    },

    /// An inline image.
    Image(Image),

    /// A citation (e.g. `[@smith2020; @jones2021]`).
    Citation(Citation),

    /// A reference to a footnote definition.
    FootnoteRef { id: String },

    /// A cross-reference to a labelled element (figure, equation, section…).
    CrossRef(CrossRef),

    /// Raw inline content in a specific format (pass-through).
    RawInline { format: String, content: String },

    /// Superscript.
    Superscript { content: Vec<Inline> },

    /// Subscript.
    Subscript { content: Vec<Inline> },

    /// Small caps.
    SmallCaps { content: Vec<Inline> },

    /// A soft line break (typically rendered as a space).
    SoftBreak,

    /// A hard line break (`<br>`).
    HardBreak,

    /// Underlined content.
    Underline { content: Vec<Inline> },

    /// A generic span with attributes (id, classes, key-value pairs).
    Span {
        content: Vec<Inline>,
        attrs: Attributes,
    },

    /// Quoted content (smart quotes).
    Quoted {
        quote_type: QuoteType,
        content: Vec<Inline>,
    },
}

/// Type of smart quotation marks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuoteType {
    SingleQuote,
    DoubleQuote,
}

// ─── Image ───────────────────────────────────────────────────────────────────

/// An image reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    pub url: String,
    /// Alt text as rich inline content (matches pandoc model).
    pub alt: Vec<Inline>,
    pub title: Option<String>,
    /// Extra attributes (classes, key-value pairs) from source format.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attrs: Option<Attributes>,
}

impl Image {
    /// Convenience: extract alt text as a plain string (concatenating text nodes).
    pub fn alt_text(&self) -> String {
        fn collect_text(inlines: &[Inline], out: &mut String) {
            for inline in inlines {
                match inline {
                    Inline::Text { value } => out.push_str(value),
                    Inline::Emphasis { content }
                    | Inline::Strong { content }
                    | Inline::Strikethrough { content }
                    | Inline::Underline { content }
                    | Inline::Superscript { content }
                    | Inline::Subscript { content }
                    | Inline::SmallCaps { content }
                    | Inline::Span { content, .. }
                    | Inline::Quoted { content, .. } => collect_text(content, out),
                    Inline::Code { value, .. } => out.push_str(value),
                    Inline::MathInline { value } => out.push_str(value),
                    Inline::SoftBreak => out.push(' '),
                    Inline::HardBreak => out.push('\n'),
                    _ => {}
                }
            }
        }
        let mut s = String::new();
        collect_text(&self.alt, &mut s);
        s
    }
}

// ─── Citation ────────────────────────────────────────────────────────────────

/// A citation referencing one or more bibliography entries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    /// Individual citation items, each with its own key and optional prefix/suffix.
    pub items: Vec<CiteItem>,
    pub mode: CitationMode,
}

/// A single item within a citation group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CiteItem {
    /// BibTeX key (e.g. `"smith2020"`).
    pub key: String,
    /// Text before this citation item (e.g. "see").
    pub prefix: Option<String>,
    /// Text after this citation item (e.g. "p. 42").
    pub suffix: Option<String>,
}

impl Citation {
    /// Convenience: collect all keys from citation items.
    pub fn keys(&self) -> Vec<&str> {
        self.items.iter().map(|item| item.key.as_str()).collect()
    }
}

/// How the citation should be rendered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CitationMode {
    /// Parenthetical: (Smith, 2020)
    #[default]
    Normal,
    /// Narrative / author-only: Smith (2020)
    AuthorOnly,
    /// Suppress author: (2020)
    SuppressAuthor,
}

// ─── Cross-references ────────────────────────────────────────────────────────

/// A cross-reference to a labelled block element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRef {
    /// The label id being referenced.
    pub target: String,
    pub form: RefForm,
}

/// How the cross-reference should be rendered.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum RefForm {
    /// Just the number: "3"
    #[default]
    Number,
    /// Type + number: "Figure 3"
    NumberWithType,
    /// Page reference: "page 5"
    Page,
    /// Custom supplement text.
    Custom(String),
}

/// The kind of labelled element (used by the cross-ref resolver).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LabelKind {
    Figure,
    Table,
    Equation,
    Section,
    Code,
    Custom(String),
}

// ─── Attributes ──────────────────────────────────────────────────────────────

/// Generic attributes that can be attached to spans and other elements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Attributes {
    pub id: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub key_values: HashMap<String, String>,
}

// ─── Bibliography ────────────────────────────────────────────────────────────

/// A bibliography attached to the document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bibliography {
    pub entries: Vec<BibEntry>,
    /// CSL style name (e.g. "apa", "ieee").
    pub style: Option<String>,
}

/// A single bibliography entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BibEntry {
    /// Unique key (e.g. "smith2020").
    pub key: String,
    /// Entry type (e.g. "article", "book", "inproceedings").
    pub entry_type: String,
    /// All fields as key-value pairs.
    pub fields: HashMap<String, String>,
}

// ─── Convenience constructors ────────────────────────────────────────────────

impl Document {
    /// Create an empty document.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Block {
    /// Shorthand for a paragraph with a single text inline.
    pub fn text(s: impl Into<String>) -> Self {
        Block::Paragraph {
            content: vec![Inline::Text { value: s.into() }],
        }
    }

    /// Shorthand for a heading.
    pub fn heading(level: u8, text: impl Into<String>) -> Self {
        Block::Heading {
            level,
            id: None,
            content: vec![Inline::Text { value: text.into() }],
            attrs: None,
        }
    }
}

impl Inline {
    /// Shorthand for plain text.
    pub fn text(s: impl Into<String>) -> Self {
        Inline::Text { value: s.into() }
    }

    /// Shorthand for inline code.
    pub fn code(s: impl Into<String>) -> Self {
        Inline::Code {
            value: s.into(),
            attrs: None,
        }
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_document() {
        let doc = Document::new();
        assert!(doc.content.is_empty());
        assert!(doc.metadata.title.is_none());
        assert!(doc.bibliography.is_none());
    }

    #[test]
    fn document_with_content() {
        let doc = Document {
            metadata: Metadata {
                title: Some("Test Document".into()),
                ..Default::default()
            },
            content: vec![
                Block::heading(1, "Introduction"),
                Block::text("Hello, world!"),
                Block::MathBlock {
                    content: "E = mc^2".into(),
                    label: Some("eq:einstein".into()),
                },
            ],
            bibliography: None,
            warnings: vec![],
            resources: HashMap::new(),
        };

        assert_eq!(doc.content.len(), 3);
        assert_eq!(doc.metadata.title.as_deref(), Some("Test Document"));
    }

    #[test]
    fn citation_default_mode() {
        let cite = Citation {
            items: vec![CiteItem {
                key: "smith2020".into(),
                prefix: None,
                suffix: None,
            }],
            mode: CitationMode::default(),
        };
        assert!(matches!(cite.mode, CitationMode::Normal));
        assert_eq!(cite.keys(), vec!["smith2020"]);
    }

    #[test]
    fn table_with_spans() {
        let table = Table {
            caption: Some(vec![Inline::text("Results")]),
            label: Some("tab:results".into()),
            columns: vec![
                ColumnSpec {
                    alignment: Alignment::Left,
                    width: None,
                },
                ColumnSpec {
                    alignment: Alignment::Right,
                    width: Some(0.3),
                },
            ],
            header: Some(vec![
                TableCell {
                    content: vec![Block::text("Name")],
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: vec![Block::text("Value")],
                    colspan: 1,
                    rowspan: 1,
                },
            ]),
            rows: vec![vec![
                TableCell {
                    content: vec![Block::text("Pi")],
                    colspan: 1,
                    rowspan: 1,
                },
                TableCell {
                    content: vec![Block::text("3.14")],
                    colspan: 1,
                    rowspan: 1,
                },
            ]],
            foot: None,
            attrs: None,
        };
        assert_eq!(table.rows.len(), 1);
        assert_eq!(table.columns.len(), 2);
    }

    #[test]
    fn serialization_roundtrip() {
        let doc = Document {
            metadata: Metadata {
                title: Some("Roundtrip Test".into()),
                ..Default::default()
            },
            content: vec![Block::text("Hello")],
            bibliography: None,
            warnings: vec![],
            resources: HashMap::new(),
        };

        let json = serde_json::to_string(&doc).expect("serialize");
        let back: Document = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.metadata.title.as_deref(), Some("Roundtrip Test"));
    }
}
