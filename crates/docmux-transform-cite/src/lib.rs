//! # docmux-transform-cite
//!
//! CSL citation processing for docmux using hayagriva.
//!
//! This transform performs three passes:
//!
//! 1. **Collect**: walk the AST and gather all `Inline::Citation` nodes in
//!    document order, resolving each cite key against the hayagriva library.
//! 2. **Replace**: walk again, replacing `Inline::Citation` with formatted
//!    text from the bibliography driver (unresolved keys become `[?key]`).
//! 3. **Bibliography**: insert formatted bibliography entries into a `Div#refs`
//!    or append one at the end of the document.

use std::fmt::Write as _;

use citationberg::{IndependentStyle, Locale, LocaleFile};
use hayagriva::{
    BibliographyDriver, BibliographyRequest, CitationItem, CitationRequest, CitePurpose, Entry,
    Library, Rendered,
};

use docmux_ast::*;
use docmux_core::{Result, Transform, TransformContext};

/// The embedded default CSL style (Chicago Author-Date).
const DEFAULT_CSL: &str = include_str!("chicago-author-date.csl");

/// The embedded default CSL locale (en-US).
const DEFAULT_LOCALE: &str = include_str!("locales-en-US.xml");

// ─── Transform ──────────────────────────────────────────────────────────────

/// Cite transform — resolves citations and inserts bibliography.
#[derive(Debug)]
pub struct CiteTransform {
    library: Library,
    style: IndependentStyle,
    locales: Vec<Locale>,
}

impl CiteTransform {
    /// Create a new cite transform with the given library and optional CSL XML.
    ///
    /// If `csl_xml` is `None`, the embedded Chicago Author-Date style is used.
    pub fn with_library(library: Library, csl_xml: Option<&str>) -> Result<Self> {
        let xml = csl_xml.unwrap_or(DEFAULT_CSL);
        let style = IndependentStyle::from_xml(xml)
            .map_err(|e| docmux_core::ConvertError::Other(format!("CSL parse error: {e}")))?;

        let locale_file = LocaleFile::from_xml(DEFAULT_LOCALE)
            .map_err(|e| docmux_core::ConvertError::Other(format!("locale parse error: {e}")))?;
        let locales = vec![Locale::from(locale_file)];

        Ok(Self {
            library,
            style,
            locales,
        })
    }
}

impl Transform for CiteTransform {
    fn name(&self) -> &str {
        "cite"
    }

    fn transform(&self, doc: &mut Document, _ctx: &TransformContext) -> Result<()> {
        // Pass 1: collect citation groups in document order
        let groups = collect_citations(&doc.content);
        if groups.is_empty() {
            return Ok(());
        }

        // Build the bibliography driver
        let (cite_strings, formatted_bib) =
            run_driver(&groups, &self.library, &self.style, &self.locales);

        // Pass 2: replace Inline::Citation nodes with formatted text
        let mut cite_idx: usize = 0;
        replace_cites_in_blocks(&mut doc.content, &cite_strings, &mut cite_idx);

        // Pass 3: insert bibliography
        if let Some(bib_blocks) = formatted_bib {
            insert_bibliography(&mut doc.content, bib_blocks);
        }

        Ok(())
    }
}

// ─── Citation group ─────────────────────────────────────────────────────────

/// A citation group collected during pass 1.
#[derive(Debug)]
struct CitationGroup {
    /// The cite items from the AST node (key + optional prefix/suffix).
    items: Vec<CiteItem>,
    /// The citation mode from the AST node.
    mode: CitationMode,
}

// ─── Pass 1: Collect citations ──────────────────────────────────────────────

fn collect_citations(blocks: &[Block]) -> Vec<CitationGroup> {
    let mut groups = Vec::new();
    collect_from_blocks(blocks, &mut groups);
    groups
}

fn collect_from_blocks(blocks: &[Block], groups: &mut Vec<CitationGroup>) {
    for block in blocks {
        collect_from_block(block, groups);
    }
}

fn collect_from_block(block: &Block, groups: &mut Vec<CitationGroup>) {
    match block {
        Block::Paragraph { content } | Block::Heading { content, .. } => {
            collect_from_inlines(content, groups);
        }
        Block::BlockQuote { content }
        | Block::FootnoteDef { content, .. }
        | Block::Div { content, .. } => {
            collect_from_blocks(content, groups);
        }
        Block::Admonition { title, content, .. } => {
            if let Some(t) = title {
                collect_from_inlines(t, groups);
            }
            collect_from_blocks(content, groups);
        }
        Block::CodeBlock {
            caption: Some(cap), ..
        } => {
            collect_from_inlines(cap, groups);
        }
        Block::List { items, .. } => {
            for item in items {
                collect_from_blocks(&item.content, groups);
            }
        }
        Block::DefinitionList { items } => {
            for item in items {
                collect_from_inlines(&item.term, groups);
                for def in &item.definitions {
                    collect_from_blocks(def, groups);
                }
            }
        }
        Block::Table(table) => collect_from_table(table, groups),
        Block::Figure {
            caption: Some(cap), ..
        } => {
            collect_from_inlines(cap, groups);
        }
        _ => {}
    }
}

fn collect_from_table(table: &Table, groups: &mut Vec<CitationGroup>) {
    if let Some(cap) = &table.caption {
        collect_from_inlines(cap, groups);
    }
    if let Some(header) = &table.header {
        for cell in header {
            collect_from_blocks(&cell.content, groups);
        }
    }
    for row in &table.rows {
        for cell in row {
            collect_from_blocks(&cell.content, groups);
        }
    }
    if let Some(foot) = &table.foot {
        for cell in foot {
            collect_from_blocks(&cell.content, groups);
        }
    }
}

fn collect_from_inlines(inlines: &[Inline], groups: &mut Vec<CitationGroup>) {
    for inline in inlines {
        collect_from_inline(inline, groups);
    }
}

fn collect_from_inline(inline: &Inline, groups: &mut Vec<CitationGroup>) {
    match inline {
        Inline::Citation(cite) => {
            groups.push(CitationGroup {
                items: cite.items.clone(),
                mode: cite.mode,
            });
        }
        Inline::Emphasis { content }
        | Inline::Strong { content }
        | Inline::Strikethrough { content }
        | Inline::Superscript { content }
        | Inline::Subscript { content }
        | Inline::SmallCaps { content }
        | Inline::Underline { content }
        | Inline::Span { content, .. }
        | Inline::Link { content, .. }
        | Inline::Quoted { content, .. } => {
            collect_from_inlines(content, groups);
        }
        _ => {}
    }
}

// ─── Driver: format citations + bibliography ────────────────────────────────

/// Run the hayagriva bibliography driver, returning formatted citation strings
/// and optional bibliography block content.
fn run_driver(
    groups: &[CitationGroup],
    library: &Library,
    style: &IndependentStyle,
    locales: &[Locale],
) -> (Vec<String>, Option<Vec<Block>>) {
    let (result, sent_to_driver) = feed_driver(groups, library, style, locales);
    let cite_strings = extract_cite_strings(&result, &sent_to_driver, groups);
    let bib_blocks = build_bib_blocks(&result);
    (cite_strings, bib_blocks)
}

/// Feed citation groups into the hayagriva driver and return the result.
fn feed_driver(
    groups: &[CitationGroup],
    library: &Library,
    style: &IndependentStyle,
    locales: &[Locale],
) -> (Rendered, Vec<bool>) {
    let mut driver = BibliographyDriver::new();
    let mut sent_to_driver: Vec<bool> = Vec::with_capacity(groups.len());

    for group in groups {
        let cite_items = build_citation_items(group, library);
        if cite_items.is_empty() {
            sent_to_driver.push(false);
        } else {
            let req = CitationRequest::from_items(cite_items, style, locales);
            driver.citation(req);
            sent_to_driver.push(true);
        }
    }

    let result = driver.finish(BibliographyRequest {
        style,
        locale: None,
        locale_files: locales,
    });

    (result, sent_to_driver)
}

/// Extract citation strings from the driver result, aligning with groups.
fn extract_cite_strings(
    result: &Rendered,
    sent_to_driver: &[bool],
    groups: &[CitationGroup],
) -> Vec<String> {
    let mut driver_idx: usize = 0;
    sent_to_driver
        .iter()
        .zip(groups.iter())
        .map(|(&resolved, group)| {
            if resolved {
                let text = result
                    .citations
                    .get(driver_idx)
                    .map(|rc| format!("{:#}", rc.citation))
                    .unwrap_or_default();
                driver_idx += 1;
                apply_affixes(&text, &group.items)
            } else {
                format_unresolved(group)
            }
        })
        .collect()
}

/// Apply prefix/suffix from cite items to the formatted citation string.
///
/// For single-item groups, prefix is prepended and suffix is appended.
/// For multi-item groups, the first item's prefix is prepended and the last
/// item's suffix is appended (matching pandoc behaviour for grouped citations).
fn apply_affixes(text: &str, items: &[CiteItem]) -> String {
    let prefix = items
        .first()
        .and_then(|item| item.prefix.as_deref())
        .filter(|s| !s.is_empty());
    let suffix = items
        .last()
        .and_then(|item| item.suffix.as_deref())
        .filter(|s| !s.is_empty());

    match (prefix, suffix) {
        (Some(p), Some(s)) => format_with_prefix_suffix(p, text, s),
        (Some(p), None) => format!("{p} {text}"),
        (None, Some(s)) => format_with_suffix(text, s),
        (None, None) => text.to_owned(),
    }
}

/// Format citation text with both prefix and suffix.
fn format_with_prefix_suffix(prefix: &str, text: &str, suffix: &str) -> String {
    let suffix_sep = suffix_separator(suffix);
    format!("{prefix} {text}{suffix_sep}{suffix}")
}

/// Format citation text with a suffix only.
fn format_with_suffix(text: &str, suffix: &str) -> String {
    let suffix_sep = suffix_separator(suffix);
    format!("{text}{suffix_sep}{suffix}")
}

/// Return the separator to place before a suffix: empty if the suffix already
/// starts with punctuation, otherwise `", "`.
fn suffix_separator(suffix: &str) -> &'static str {
    if suffix.starts_with(|c: char| c.is_ascii_punctuation()) {
        " "
    } else {
        ", "
    }
}

/// Build bibliography blocks from the driver result.
fn build_bib_blocks(result: &Rendered) -> Option<Vec<Block>> {
    result.bibliography.as_ref().map(|bib| {
        bib.items
            .iter()
            .map(|item| {
                let mut buf = String::new();
                let _ = write!(buf, "{:#}", item.content);
                Block::Paragraph {
                    content: vec![Inline::text(buf)],
                }
            })
            .collect()
    })
}

/// Build CitationItems for a group, filtering out unresolved keys.
fn build_citation_items<'a>(
    group: &CitationGroup,
    library: &'a Library,
) -> Vec<CitationItem<'a, Entry>> {
    let purpose = match group.mode {
        CitationMode::AuthorOnly => Some(CitePurpose::Prose),
        CitationMode::SuppressAuthor => Some(CitePurpose::Year),
        CitationMode::Normal => None,
    };

    group
        .items
        .iter()
        .filter_map(|item| match library.get(&item.key) {
            Some(entry) => {
                let mut ci = CitationItem::with_entry(entry);
                if let Some(p) = purpose {
                    ci.purpose = Some(p);
                }
                Some(ci)
            }
            None => {
                eprintln!(
                    "warning: citation key '{}' not found in bibliography",
                    item.key
                );
                None
            }
        })
        .collect()
}

/// Format a citation group where all keys are unresolved.
fn format_unresolved(group: &CitationGroup) -> String {
    group
        .items
        .iter()
        .map(|item| format!("[?{}]", item.key))
        .collect::<Vec<_>>()
        .join("; ")
}

// ─── Pass 2: Replace citations ──────────────────────────────────────────────

fn replace_cites_in_blocks(blocks: &mut [Block], strings: &[String], idx: &mut usize) {
    for block in blocks.iter_mut() {
        replace_cites_in_block(block, strings, idx);
    }
}

fn replace_cites_in_block(block: &mut Block, strings: &[String], idx: &mut usize) {
    match block {
        Block::Paragraph { content } | Block::Heading { content, .. } => {
            replace_cites_in_inlines(content, strings, idx);
        }
        Block::BlockQuote { content }
        | Block::FootnoteDef { content, .. }
        | Block::Div { content, .. } => {
            replace_cites_in_blocks(content, strings, idx);
        }
        Block::Admonition { title, content, .. } => {
            if let Some(t) = title {
                replace_cites_in_inlines(t, strings, idx);
            }
            replace_cites_in_blocks(content, strings, idx);
        }
        Block::CodeBlock {
            caption: Some(cap), ..
        } => {
            replace_cites_in_inlines(cap, strings, idx);
        }
        Block::List { items, .. } => {
            for item in items {
                replace_cites_in_blocks(&mut item.content, strings, idx);
            }
        }
        Block::DefinitionList { items } => {
            for item in items {
                replace_cites_in_inlines(&mut item.term, strings, idx);
                for def in &mut item.definitions {
                    replace_cites_in_blocks(def, strings, idx);
                }
            }
        }
        Block::Table(table) => replace_cites_in_table(table, strings, idx),
        Block::Figure {
            caption: Some(cap), ..
        } => {
            replace_cites_in_inlines(cap, strings, idx);
        }
        _ => {}
    }
}

fn replace_cites_in_table(table: &mut Table, strings: &[String], idx: &mut usize) {
    if let Some(cap) = &mut table.caption {
        replace_cites_in_inlines(cap, strings, idx);
    }
    if let Some(header) = &mut table.header {
        for cell in header {
            replace_cites_in_blocks(&mut cell.content, strings, idx);
        }
    }
    for row in &mut table.rows {
        for cell in row {
            replace_cites_in_blocks(&mut cell.content, strings, idx);
        }
    }
    if let Some(foot) = &mut table.foot {
        for cell in foot {
            replace_cites_in_blocks(&mut cell.content, strings, idx);
        }
    }
}

fn replace_cites_in_inlines(inlines: &mut [Inline], strings: &[String], idx: &mut usize) {
    for inline in inlines.iter_mut() {
        replace_cite_in_inline(inline, strings, idx);
    }
}

fn replace_cite_in_inline(inline: &mut Inline, strings: &[String], idx: &mut usize) {
    match inline {
        Inline::Citation(_) => {
            if let Some(text) = strings.get(*idx) {
                *inline = Inline::Text {
                    value: text.clone(),
                };
            }
            *idx += 1;
        }
        Inline::Emphasis { content }
        | Inline::Strong { content }
        | Inline::Strikethrough { content }
        | Inline::Superscript { content }
        | Inline::Subscript { content }
        | Inline::SmallCaps { content }
        | Inline::Underline { content }
        | Inline::Span { content, .. }
        | Inline::Link { content, .. }
        | Inline::Quoted { content, .. } => {
            replace_cites_in_inlines(content, strings, idx);
        }
        _ => {}
    }
}

// ─── Pass 3: Insert bibliography ────────────────────────────────────────────

/// Insert bibliography blocks into an existing `Div#refs` or append a new one.
fn insert_bibliography(blocks: &mut Vec<Block>, bib_content: Vec<Block>) {
    // Search for an existing Div with id="refs"
    if let Some(pos) = find_refs_div(blocks) {
        if let Block::Div { content, .. } = &mut blocks[pos] {
            *content = bib_content;
        }
        return;
    }

    // No existing refs div — append one at the end
    blocks.push(Block::Div {
        attrs: Attributes {
            id: Some("refs".into()),
            classes: vec![],
            key_values: Default::default(),
        },
        content: bib_content,
    });
}

/// Find the index of a `Div` block with `id = "refs"`.
fn find_refs_div(blocks: &[Block]) -> Option<usize> {
    blocks.iter().position(|b| match b {
        Block::Div { attrs, .. } => attrs.id.as_deref() == Some("refs"),
        _ => false,
    })
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_core::TransformContext;

    /// Create a minimal hayagriva library with one entry for testing.
    fn test_library() -> Library {
        let yaml = r#"
smith2020:
    type: article
    title: A Great Paper
    author: Smith, John
    date: 2020
    parent:
        type: periodical
        title: Journal of Testing
"#;
        hayagriva::io::from_yaml_str(yaml).unwrap()
    }

    fn make_citation(key: &str, mode: CitationMode) -> Inline {
        Inline::Citation(Citation {
            items: vec![CiteItem {
                key: key.into(),
                prefix: None,
                suffix: None,
            }],
            mode,
        })
    }

    #[test]
    fn resolves_known_citation() {
        let lib = test_library();
        let transform = CiteTransform::with_library(lib, None).unwrap();

        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("See "),
                    make_citation("smith2020", CitationMode::Normal),
                    Inline::text("."),
                ],
            }],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        if let Block::Paragraph { content } = &doc.content[0] {
            // The citation should now be an Inline::Text, not Inline::Citation
            assert!(
                matches!(&content[1], Inline::Text { .. }),
                "Expected Inline::Text, got {:?}",
                &content[1]
            );
            // Should contain author and year (Chicago author-date)
            if let Inline::Text { value } = &content[1] {
                assert!(
                    value.contains("Smith"),
                    "Expected citation text containing 'Smith', got: {value}"
                );
                assert!(
                    value.contains("2020"),
                    "Expected citation text containing year '2020', got: {value}"
                );
            }
        } else {
            panic!("Expected Paragraph");
        }
    }

    #[test]
    fn unknown_key_becomes_placeholder() {
        let lib = test_library();
        let transform = CiteTransform::with_library(lib, None).unwrap();

        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![make_citation("nonexistent", CitationMode::Normal)],
            }],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        if let Block::Paragraph { content } = &doc.content[0] {
            match &content[0] {
                Inline::Text { value } => {
                    assert_eq!(value, "[?nonexistent]");
                }
                other => panic!("Expected placeholder text, got {:?}", other),
            }
        } else {
            panic!("Expected Paragraph");
        }
    }

    #[test]
    fn bibliography_appended_at_end() {
        let lib = test_library();
        let transform = CiteTransform::with_library(lib, None).unwrap();

        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![make_citation("smith2020", CitationMode::Normal)],
            }],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        // Last block should be a Div with id="refs"
        let last = doc.content.last().expect("document should have blocks");
        match last {
            Block::Div { attrs, content } => {
                assert_eq!(attrs.id.as_deref(), Some("refs"));
                assert!(!content.is_empty(), "Bibliography should have entries");
            }
            other => panic!("Expected Div#refs, got {:?}", other),
        }
    }

    #[test]
    fn bibliography_replaces_refs_div() {
        let lib = test_library();
        let transform = CiteTransform::with_library(lib, None).unwrap();

        let mut doc = Document {
            content: vec![
                Block::Paragraph {
                    content: vec![make_citation("smith2020", CitationMode::Normal)],
                },
                // Pre-existing empty refs div
                Block::Div {
                    attrs: Attributes {
                        id: Some("refs".into()),
                        classes: vec![],
                        key_values: Default::default(),
                    },
                    content: vec![],
                },
                Block::Paragraph {
                    content: vec![Inline::text("After refs")],
                },
            ],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        // The refs div should now have content
        match &doc.content[1] {
            Block::Div { attrs, content } => {
                assert_eq!(attrs.id.as_deref(), Some("refs"));
                assert!(!content.is_empty(), "Bibliography div should have entries");
            }
            other => panic!("Expected Div#refs at index 1, got {:?}", other),
        }

        // No additional Div#refs should have been appended
        let refs_count = doc
            .content
            .iter()
            .filter(
                |b| matches!(b, Block::Div { attrs, .. } if attrs.id.as_deref() == Some("refs")),
            )
            .count();
        assert_eq!(refs_count, 1, "Should have exactly one refs div");
    }

    #[test]
    fn citation_with_prefix_suffix() {
        let bib_yaml = r#"
smith2020:
    type: article
    title: Test Article
    author: Smith, John
    date: 2020
    parent:
        type: periodical
        title: Journal
"#;
        let lib = hayagriva::io::from_yaml_str(bib_yaml).unwrap();
        let transform = CiteTransform::with_library(lib, None).unwrap();

        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Citation(Citation {
                    items: vec![CiteItem {
                        key: "smith2020".into(),
                        prefix: Some("see".into()),
                        suffix: Some("p. 42".into()),
                    }],
                    mode: CitationMode::Normal,
                })],
            }],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        let text = match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::Text { value } => value.clone(),
                other => format!("{other:?}"),
            },
            other => format!("{other:?}"),
        };

        assert!(text.contains("see"), "should contain prefix 'see': {text}");
        assert!(
            text.contains("p. 42"),
            "should contain suffix 'p. 42': {text}"
        );
    }

    #[test]
    fn citation_with_prefix_only() {
        let lib = test_library();
        let transform = CiteTransform::with_library(lib, None).unwrap();

        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Citation(Citation {
                    items: vec![CiteItem {
                        key: "smith2020".into(),
                        prefix: Some("see".into()),
                        suffix: None,
                    }],
                    mode: CitationMode::Normal,
                })],
            }],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        let text = match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::Text { value } => value.clone(),
                other => format!("{other:?}"),
            },
            other => format!("{other:?}"),
        };

        assert!(text.contains("see"), "should contain prefix 'see': {text}");
        assert!(
            text.contains("Smith"),
            "should still contain author: {text}"
        );
    }

    #[test]
    fn citation_with_suffix_only() {
        let lib = test_library();
        let transform = CiteTransform::with_library(lib, None).unwrap();

        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Citation(Citation {
                    items: vec![CiteItem {
                        key: "smith2020".into(),
                        prefix: None,
                        suffix: Some("p. 42".into()),
                    }],
                    mode: CitationMode::Normal,
                })],
            }],
            ..Default::default()
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        let text = match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::Text { value } => value.clone(),
                other => format!("{other:?}"),
            },
            other => format!("{other:?}"),
        };

        assert!(
            text.contains("p. 42"),
            "should contain suffix 'p. 42': {text}"
        );
        assert!(
            text.contains("Smith"),
            "should still contain author: {text}"
        );
    }

    #[test]
    fn no_bibliography_when_no_citations() {
        let lib = test_library();
        let transform = CiteTransform::with_library(lib, None).unwrap();

        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::text("No citations here")],
            }],
            ..Default::default()
        };

        let original_len = doc.content.len();

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        assert_eq!(
            doc.content.len(),
            original_len,
            "No blocks should be added when there are no citations"
        );
    }
}
