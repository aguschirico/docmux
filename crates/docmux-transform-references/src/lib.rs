//! # docmux-transform-references
//!
//! Resolves `Inline::Citation` nodes against a references list that is
//! already in the document: a `Div` with id `refs` whose entries are `Div`s
//! with id `ref-<key>`, class `csl-entry`, and an optional `label` key-value.
//! The LaTeX reader builds that list from the `.bbl` file BibTeX produced,
//! which is what arXiv ships (the `.bib` database rarely is).
//!
//! Each citation becomes text with links to its entries. The style follows
//! the labels:
//!
//! - natbib author-year labels (`Kingma and Ba(2015)…`) → `(Kingma and Ba, 2015)`
//!   for `\citep`, `Kingma and Ba (2015)` for `\citet`;
//! - other labels (`BCV13`) → `[BCV13]`;
//! - no labels → numbers in list order, `[1, 2]`.
//!
//! Keys that are not in the list are left as citations for the writer.

use std::collections::HashMap;

use docmux_ast::*;
use docmux_core::{Result, Transform, TransformContext};

/// Citation resolver for documents that carry their own references list.
#[derive(Debug, Default)]
pub struct ReferencesTransform;

impl ReferencesTransform {
    pub fn new() -> Self {
        Self
    }
}

impl Transform for ReferencesTransform {
    fn name(&self) -> &str {
        "references"
    }

    fn transform(&self, doc: &mut Document, _ctx: &TransformContext) -> Result<()> {
        let mut entries = Vec::new();
        collect_entries(&doc.content, &mut entries);
        if entries.is_empty() {
            return Ok(());
        }
        let refs = References::new(entries);
        resolve_blocks(&mut doc.content, &refs);
        if let Some(abstract_blocks) = &mut doc.metadata.abstract_text {
            resolve_blocks(abstract_blocks, &refs);
        }
        Ok(())
    }
}

// ─── References list ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
enum Style {
    Numeric,
    Label,
    AuthorYear,
}

#[derive(Debug)]
struct Entry {
    key: String,
    number: usize,
    label: Option<String>,
}

struct References {
    style: Style,
    by_key: HashMap<String, Entry>,
}

impl References {
    fn new(entries: Vec<Entry>) -> Self {
        let labelled = entries.iter().filter(|e| e.label.is_some()).count();
        let author_year = entries
            .iter()
            .filter(|e| e.label.as_deref().and_then(author_year).is_some())
            .count();
        let style = if labelled * 2 < entries.len() {
            Style::Numeric
        } else if author_year * 2 >= entries.len() {
            Style::AuthorYear
        } else {
            Style::Label
        };
        let by_key = entries.into_iter().map(|e| (e.key.clone(), e)).collect();
        Self { style, by_key }
    }
}

/// Split a natbib label `Short(Year)Long` into `(Short, Year)`.
fn author_year(label: &str) -> Option<(&str, &str)> {
    let open = label.find('(')?;
    let close = open + label[open..].find(')')?;
    let author = label[..open].trim();
    let year = label[open + 1..close].trim();
    let year_like = year.chars().next().is_some_and(|c| c.is_ascii_digit()) || year == "n.d.";
    (!author.is_empty() && year_like).then_some((author, year))
}

fn collect_entries(blocks: &[Block], out: &mut Vec<Entry>) {
    for block in blocks {
        match block {
            Block::Div { attrs, content } => {
                let is_entry = attrs.classes.iter().any(|c| c == "csl-entry");
                match attrs.id.as_deref().and_then(|id| id.strip_prefix("ref-")) {
                    Some(key) if is_entry => out.push(Entry {
                        key: key.to_string(),
                        number: out.len() + 1,
                        label: attrs.key_values.get("label").cloned(),
                    }),
                    _ => collect_entries(content, out),
                }
            }
            Block::BlockQuote { content } | Block::Admonition { content, .. } => {
                collect_entries(content, out)
            }
            _ => {}
        }
    }
}

// ─── Rendering ──────────────────────────────────────────────────────────────

fn link(key: &str, text: String) -> Inline {
    Inline::Link {
        url: format!("#ref-{key}"),
        title: None,
        content: vec![Inline::Text { value: text }],
        attrs: None,
    }
}

fn text(value: impl Into<String>) -> Inline {
    Inline::Text {
        value: value.into(),
    }
}

/// Render a citation, or `None` if none of its keys are in the list.
fn render(cite: &Citation, refs: &References) -> Option<Vec<Inline>> {
    if !cite.items.iter().any(|i| refs.by_key.contains_key(&i.key)) {
        return None;
    }
    Some(match refs.style {
        Style::AuthorYear => render_author_year(cite, refs),
        Style::Numeric | Style::Label => render_bracketed(cite, refs),
    })
}

/// `[1, 2]`, `[see 3, p. 5]`, `[BCV13]`.
fn render_bracketed(cite: &Citation, refs: &References) -> Vec<Inline> {
    let mut out = vec![text("[")];
    for (k, item) in cite.items.iter().enumerate() {
        if k > 0 {
            out.push(text(", "));
        }
        if let Some(prefix) = &item.prefix {
            out.push(text(format!("{prefix} ")));
        }
        match refs.by_key.get(&item.key) {
            Some(entry) => {
                let shown = match (&entry.label, refs.style) {
                    (Some(label), Style::Label) => label.clone(),
                    _ => entry.number.to_string(),
                };
                out.push(link(&item.key, shown));
            }
            None => out.push(text("?")),
        }
        if let Some(suffix) = &item.suffix {
            out.push(text(format!(", {suffix}")));
        }
    }
    out.push(text("]"));
    out
}

/// `(Kingma and Ba, 2015; Vaswani et al., 2017)` for `\citep`,
/// `Kingma and Ba (2015)` for `\citet`, `(2015)` for `\citeyear`.
fn render_author_year(cite: &Citation, refs: &References) -> Vec<Inline> {
    let parts = |item: &CiteItem| {
        let label = refs.by_key.get(&item.key).and_then(|e| e.label.as_deref());
        label
            .and_then(author_year)
            .map(|(a, y)| (a.to_string(), y.to_string()))
            .unwrap_or_else(|| ("?".to_string(), "?".to_string()))
    };
    let narrative = matches!(cite.mode, CitationMode::AuthorOnly);
    let mut out = Vec::new();
    if !narrative {
        out.push(text("("));
    }
    for (k, item) in cite.items.iter().enumerate() {
        if k > 0 {
            out.push(text(if narrative { ", " } else { "; " }));
        }
        if let Some(prefix) = &item.prefix {
            out.push(text(format!("{prefix} ")));
        }
        let (author, year) = parts(item);
        let suffix = item
            .suffix
            .as_ref()
            .map(|s| format!(", {s}"))
            .unwrap_or_default();
        let shown = match cite.mode {
            CitationMode::AuthorOnly => format!("{author} ({year}{suffix})"),
            CitationMode::SuppressAuthor => format!("{year}{suffix}"),
            CitationMode::Normal => format!("{author}, {year}{suffix}"),
        };
        out.push(link(&item.key, shown));
    }
    if !narrative {
        out.push(text(")"));
    }
    out
}

// ─── AST walk ───────────────────────────────────────────────────────────────

fn resolve_blocks(blocks: &mut [Block], refs: &References) {
    for block in blocks {
        match block {
            Block::Paragraph { content } | Block::Heading { content, .. } => {
                resolve_inlines(content, refs)
            }
            Block::BlockQuote { content }
            | Block::Div { content, .. }
            | Block::Admonition { content, .. }
            | Block::FootnoteDef { content, .. } => resolve_blocks(content, refs),
            Block::List { items, .. } => {
                for item in items {
                    resolve_blocks(&mut item.content, refs);
                }
            }
            Block::Figure {
                caption: Some(caption),
                ..
            } => resolve_inlines(caption, refs),
            Block::Table(table) => resolve_table(table, refs),
            Block::DefinitionList { items } => {
                for item in items {
                    resolve_inlines(&mut item.term, refs);
                    for def in &mut item.definitions {
                        resolve_blocks(def, refs);
                    }
                }
            }
            _ => {}
        }
    }
}

fn resolve_table(table: &mut Table, refs: &References) {
    if let Some(caption) = &mut table.caption {
        resolve_inlines(caption, refs);
    }
    let header = table.header.iter_mut().flatten();
    let foot = table.foot.iter_mut().flatten();
    for cell in header.chain(table.rows.iter_mut().flatten()).chain(foot) {
        resolve_blocks(&mut cell.content, refs);
    }
}

fn resolve_inlines(inlines: &mut Vec<Inline>, refs: &References) {
    let mut out = Vec::with_capacity(inlines.len());
    for mut inline in std::mem::take(inlines) {
        if let Inline::Citation(cite) = &inline {
            if let Some(rendered) = render(cite, refs) {
                out.extend(rendered);
                continue;
            }
        }
        match &mut inline {
            Inline::Emphasis { content }
            | Inline::Strong { content }
            | Inline::Strikethrough { content }
            | Inline::Underline { content }
            | Inline::Superscript { content }
            | Inline::Subscript { content }
            | Inline::SmallCaps { content }
            | Inline::Span { content, .. }
            | Inline::Quoted { content, .. } => resolve_inlines(content, refs),
            _ => {}
        }
        out.push(inline);
    }
    *inlines = out;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(key: &str, label: Option<&str>) -> Block {
        let mut key_values = HashMap::new();
        if let Some(label) = label {
            key_values.insert("label".to_string(), label.to_string());
        }
        Block::Div {
            attrs: Attributes {
                id: Some(format!("ref-{key}")),
                classes: vec!["csl-entry".into()],
                key_values,
            },
            content: vec![Block::text("Some reference.")],
        }
    }

    fn cite(keys: &[&str], mode: CitationMode) -> Inline {
        Inline::Citation(Citation {
            items: keys
                .iter()
                .map(|k| CiteItem {
                    key: k.to_string(),
                    prefix: None,
                    suffix: None,
                })
                .collect(),
            mode,
        })
    }

    fn render_doc(entries: Vec<Block>, citation: Inline) -> String {
        let mut doc = Document {
            content: vec![
                Block::Paragraph {
                    content: vec![citation],
                },
                Block::Div {
                    attrs: Attributes {
                        id: Some("refs".into()),
                        classes: vec!["references".into()],
                        key_values: HashMap::new(),
                    },
                    content: entries,
                },
            ],
            ..Default::default()
        };
        ReferencesTransform::new()
            .transform(&mut doc, &TransformContext::default())
            .unwrap();
        let Block::Paragraph { content } = &doc.content[0] else {
            panic!("paragraph expected");
        };
        content
            .iter()
            .map(|i| match i {
                Inline::Text { value } => value.clone(),
                Inline::Link { url, content, .. } => match &content[0] {
                    Inline::Text { value } => format!("<{value}|{url}>"),
                    _ => String::new(),
                },
                Inline::Citation(_) => "CITE".into(),
                _ => String::new(),
            })
            .collect()
    }

    #[test]
    fn numeric_style() {
        let out = render_doc(
            vec![entry("a", None), entry("b", None)],
            cite(&["b", "a"], CitationMode::Normal),
        );
        assert_eq!(out, "[<2|#ref-b>, <1|#ref-a>]");
    }

    #[test]
    fn label_style() {
        let out = render_doc(
            vec![entry("a", Some("BCV13"))],
            cite(&["a"], CitationMode::Normal),
        );
        assert_eq!(out, "[<BCV13|#ref-a>]");
    }

    #[test]
    fn author_year_parenthetical_and_narrative() {
        let entries = || {
            vec![
                entry("k", Some("Kingma and Ba(2015)Kingma, Ba")),
                entry("v", Some("Vaswani et al.(2017)")),
            ]
        };
        assert_eq!(
            render_doc(entries(), cite(&["k", "v"], CitationMode::Normal)),
            "(<Kingma and Ba, 2015|#ref-k>; <Vaswani et al., 2017|#ref-v>)"
        );
        assert_eq!(
            render_doc(entries(), cite(&["v"], CitationMode::AuthorOnly)),
            "<Vaswani et al. (2017)|#ref-v>"
        );
    }

    #[test]
    fn prefix_and_suffix() {
        let citation = Inline::Citation(Citation {
            items: vec![CiteItem {
                key: "k".into(),
                prefix: Some("see".into()),
                suffix: Some("p. 5".into()),
            }],
            mode: CitationMode::Normal,
        });
        let out = render_doc(vec![entry("k", Some("Kingma and Ba(2015)"))], citation);
        assert_eq!(out, "(see <Kingma and Ba, 2015, p. 5|#ref-k>)");
    }

    #[test]
    fn unknown_keys_are_left_alone() {
        let out = render_doc(vec![entry("a", None)], cite(&["zzz"], CitationMode::Normal));
        assert_eq!(out, "CITE");
    }

    #[test]
    fn documents_without_references_are_untouched() {
        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![cite(&["a"], CitationMode::Normal)],
            }],
            ..Default::default()
        };
        ReferencesTransform::new()
            .transform(&mut doc, &TransformContext::default())
            .unwrap();
        assert!(matches!(
            &doc.content[0],
            Block::Paragraph { content } if matches!(content[0], Inline::Citation(_))
        ));
    }
}
