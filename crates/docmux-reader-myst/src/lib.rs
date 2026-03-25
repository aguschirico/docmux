//! # docmux-reader-myst
//!
//! MyST Markdown reader for docmux.
//!
//! Parses [MyST (Markedly Structured Text)](https://mystmd.org/) into the docmux AST by:
//!
//! 1. **Pre-processing** the raw source: extracting MyST directives (`:::`) and
//!    target labels (`(label)=`), replacing them with placeholder markers.
//! 2. Delegating the cleaned source to [`MarkdownReader`] for base CommonMark + GFM parsing.
//! 3. **Post-processing** the resulting AST: expanding markers back into typed
//!    AST nodes, resolving inline roles, and attaching labels to their target blocks.

use docmux_ast::*;
use docmux_core::{Reader, Result};
use docmux_reader_markdown::MarkdownReader;
use std::collections::HashMap;

// ─── Marker constants ────────────────────────────────────────────────────────

const DIRECTIVE_MARKER_PREFIX: &str = "DOCMUX_MYST_DIR_";
const LABEL_MARKER_PREFIX: &str = "DOCMUX_MYST_LABEL_";

// ─── Pre-processor data structures ───────────────────────────────────────────

/// A parsed MyST directive block extracted during pre-processing.
#[derive(Debug, Clone)]
struct DirectiveBlock {
    /// The directive name (e.g. `"note"`, `"figure"`, `"code-block"`).
    name: String,
    /// The argument on the opening fence line (e.g. the image URL for `figure`).
    argument: Option<String>,
    /// `:key: value` options parsed from the first lines of the body.
    options: HashMap<String, String>,
    /// The remaining body text after stripping option lines.
    body: String,
}

/// Output of the pre-processing pass.
struct Preprocessed {
    /// Source with directives and labels replaced by marker paragraphs.
    source: String,
    /// Extracted directives, indexed by their marker number (0-based).
    directives: Vec<DirectiveBlock>,
    /// Extracted label strings, indexed by their marker number (0-based).
    labels: Vec<String>,
}

// ─── Pre-processor ───────────────────────────────────────────────────────────

/// Scan `input` line-by-line and extract MyST directives and target labels.
///
/// Directives (`:::{name}`) are replaced with `DOCMUX_MYST_DIR_N` paragraphs.
/// Labels (`(label)=`) are replaced with `DOCMUX_MYST_LABEL_N` paragraphs.
fn preprocess(input: &str) -> Preprocessed {
    let mut directives: Vec<DirectiveBlock> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    let mut output_lines: Vec<String> = Vec::new();

    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];

        // ── Label: `(my-label)=` ─────────────────────────────────────────────
        if let Some(label) = try_parse_label(line) {
            let idx = labels.len();
            labels.push(label);
            // Emit a blank line before, the marker paragraph, and a blank after
            // so comrak sees it as a standalone paragraph.
            output_lines.push(String::new());
            output_lines.push(format!("{}{}", LABEL_MARKER_PREFIX, idx));
            output_lines.push(String::new());
            i += 1;
            continue;
        }

        // ── Directive: `:::{name} [arg]` ─────────────────────────────────────
        if let Some((fence_len, name, argument)) = try_parse_directive_open(line) {
            let start = i + 1;
            // Scan for the closing fence (≥ fence_len colons on its own line).
            let mut end = start;
            while end < lines.len() {
                if is_directive_close(lines[end], fence_len) {
                    break;
                }
                end += 1;
            }

            let body_lines = &lines[start..end];
            let (options, body) = split_directive_options(body_lines);

            let idx = directives.len();
            directives.push(DirectiveBlock {
                name,
                argument,
                options,
                body,
            });

            output_lines.push(String::new());
            output_lines.push(format!("{}{}", DIRECTIVE_MARKER_PREFIX, idx));
            output_lines.push(String::new());

            // Skip past the closing fence (or EOF if unclosed).
            i = if end < lines.len() { end + 1 } else { end };
            continue;
        }

        output_lines.push(line.to_string());
        i += 1;
    }

    Preprocessed {
        source: output_lines.join("\n"),
        directives,
        labels,
    }
}

/// If `line` matches `(label)=`, return the label string.
fn try_parse_label(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('(')?.strip_suffix(")=")?;
    let label = inner.trim();
    if label.is_empty() {
        None
    } else {
        Some(label.to_string())
    }
}

/// If `line` is a directive opening fence (`:::{name} [arg]`), return
/// `(fence_len, name, argument)`.
fn try_parse_directive_open(line: &str) -> Option<(usize, String, Option<String>)> {
    let trimmed = line.trim_start();
    // Count leading colons (must be at least 3).
    let colon_count = trimmed.chars().take_while(|&c| c == ':').count();
    if colon_count < 3 {
        return None;
    }

    let rest = &trimmed[colon_count..];
    // Must be followed by `{name}`.
    let rest = rest.strip_prefix('{')?;
    let brace_end = rest.find('}')?;
    let name = rest[..brace_end].trim().to_string();
    if name.is_empty() {
        return None;
    }

    let after_brace = rest[brace_end + 1..].trim();
    let argument = if after_brace.is_empty() {
        None
    } else {
        Some(after_brace.to_string())
    };

    Some((colon_count, name, argument))
}

/// Return `true` if `line` is a valid closing fence for a directive opened
/// with `opening_len` colons.
fn is_directive_close(line: &str, opening_len: usize) -> bool {
    let trimmed = line.trim();
    let colon_count = trimmed.chars().take_while(|&c| c == ':').count();
    colon_count >= opening_len && trimmed[colon_count..].trim().is_empty()
}

/// Split directive body lines into `:key: value` options (leading block) and
/// the remaining body text.
fn split_directive_options(lines: &[&str]) -> (HashMap<String, String>, String) {
    let mut options = HashMap::new();
    let mut body_start = 0;

    for (idx, &line) in lines.iter().enumerate() {
        if let Some(kv) = try_parse_option_line(line) {
            options.insert(kv.0, kv.1);
            body_start = idx + 1;
        } else if line.trim().is_empty() && body_start == idx {
            // Allow a blank line at start of options block.
            body_start = idx + 1;
        } else {
            break;
        }
    }

    let body = lines[body_start..].join("\n");
    (options, body)
}

/// If `line` looks like `:key: value`, return `(key, value)`.
fn try_parse_option_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim();
    let rest = trimmed.strip_prefix(':')?;
    let colon_pos = rest.find(':')?;
    let key = rest[..colon_pos].trim();
    if key.is_empty() {
        return None;
    }
    let value = rest[colon_pos + 1..].trim();
    Some((key.to_string(), value.to_string()))
}

// ─── Directive → AST ─────────────────────────────────────────────────────────

/// Map a [`DirectiveBlock`] to a docmux [`Block`] node.
///
/// Directive content is parsed recursively via [`parse_myst_content`] so that
/// nested directives work correctly.
fn directive_to_block(directive: DirectiveBlock) -> Block {
    let name = directive.name.as_str();

    match name {
        "note" | "warning" | "tip" | "important" | "caution" => {
            let kind = admonition_kind_from_name(name);
            let title = directive.argument.map(|t| vec![Inline::Text { value: t }]);
            let content = parse_myst_content(&directive.body);
            Block::Admonition {
                kind,
                title,
                content,
            }
        }

        "admonition" => {
            let kind = match &directive.argument {
                Some(t) => AdmonitionKind::Custom(t.clone()),
                None => AdmonitionKind::Note,
            };
            let content = parse_myst_content(&directive.body);
            Block::Admonition {
                kind,
                title: directive.argument.map(|t| vec![Inline::Text { value: t }]),
                content,
            }
        }

        "figure" => {
            let url = directive.argument.unwrap_or_default();
            let label = directive.options.get("name").cloned();
            let alt_text = directive.options.get("alt").cloned();
            let alt = alt_text
                .map(|t| vec![Inline::Text { value: t }])
                .unwrap_or_default();

            let caption_blocks = parse_myst_content(&directive.body);
            let caption = blocks_to_inline_caption(caption_blocks);

            Block::Figure {
                image: Image {
                    url,
                    alt,
                    title: None,
                    attrs: None,
                },
                caption,
                label,
                attrs: None,
            }
        }

        "code" | "code-block" | "sourcecode" => {
            let language = directive.argument.filter(|s| !s.is_empty());
            let label = directive.options.get("name").cloned();
            Block::CodeBlock {
                language,
                content: directive.body.trim_end_matches('\n').to_string(),
                caption: None,
                label,
                attrs: None,
            }
        }

        "math" => {
            let label = directive.options.get("label").cloned();
            Block::MathBlock {
                content: directive.body.trim_end_matches('\n').to_string(),
                label,
            }
        }

        _ => {
            // Unknown directive → Div with the directive name as a class.
            let mut key_values = directive.options;
            if let Some(arg) = directive.argument {
                key_values.insert("argument".to_string(), arg);
            }
            let attrs = Attributes {
                id: None,
                classes: vec![name.to_string()],
                key_values,
            };
            let content = parse_myst_content(&directive.body);
            Block::Div { attrs, content }
        }
    }
}

/// Parse a directive body string as MyST content (recursive — handles nested
/// directives inside directive bodies).
fn parse_myst_content(source: &str) -> Vec<Block> {
    if source.trim().is_empty() {
        return Vec::new();
    }
    let reader = MystReader::new();
    match reader.read(source) {
        Ok(doc) => doc.content,
        Err(_) => vec![Block::Paragraph {
            content: vec![Inline::Text {
                value: source.to_string(),
            }],
        }],
    }
}

/// Convert the first paragraph's inline content from a block list into an
/// inline caption (used for figure captions).
fn blocks_to_inline_caption(blocks: Vec<Block>) -> Option<Vec<Inline>> {
    for block in blocks {
        if let Block::Paragraph { content } = block {
            if !content.is_empty() {
                return Some(content);
            }
        }
    }
    None
}

/// Map a directive name to an [`AdmonitionKind`].
fn admonition_kind_from_name(name: &str) -> AdmonitionKind {
    match name {
        "note" => AdmonitionKind::Note,
        "warning" => AdmonitionKind::Warning,
        "tip" => AdmonitionKind::Tip,
        "important" => AdmonitionKind::Important,
        "caution" => AdmonitionKind::Caution,
        other => AdmonitionKind::Custom(other.to_string()),
    }
}

// ─── Role → Inline ───────────────────────────────────────────────────────────

/// Walk the inline content of a paragraph and resolve MyST roles.
///
/// comrak represents `` {role}`text` `` as a `Text` node whose value ends
/// with `{role_name}` followed by a `Code` node. This function detects that
/// pattern and replaces both nodes with the appropriate AST inline.
fn resolve_roles_in_inlines(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut result: Vec<Inline> = Vec::with_capacity(inlines.len());
    let mut iter = inlines.into_iter().peekable();

    while let Some(inline) = iter.next() {
        match inline {
            Inline::Text { value } => {
                // Check if this text ends with `{role_name}` (possibly with
                // preceding text before the `{`).
                if let Some((prefix_text, role_name)) = extract_role_suffix(&value) {
                    // Peek ahead for the Code node that carries the role argument.
                    if let Some(Inline::Code { .. }) = iter.peek() {
                        if let Some(Inline::Code {
                            value: code_value, ..
                        }) = iter.next()
                        {
                            // Emit any preceding text that came before `{role}`.
                            if !prefix_text.is_empty() {
                                result.push(Inline::Text { value: prefix_text });
                            }
                            result.push(role_to_inline(&role_name, code_value));
                            continue;
                        }
                    }
                }
                // No role pattern — emit as-is.
                result.push(Inline::Text { value });
            }

            // Recurse into container inlines (owned — no cloning needed).
            Inline::Emphasis { content } => result.push(Inline::Emphasis {
                content: resolve_roles_in_inlines(content),
            }),
            Inline::Strong { content } => result.push(Inline::Strong {
                content: resolve_roles_in_inlines(content),
            }),
            Inline::Strikethrough { content } => result.push(Inline::Strikethrough {
                content: resolve_roles_in_inlines(content),
            }),
            Inline::Superscript { content } => result.push(Inline::Superscript {
                content: resolve_roles_in_inlines(content),
            }),
            Inline::Subscript { content } => result.push(Inline::Subscript {
                content: resolve_roles_in_inlines(content),
            }),
            Inline::Span { content, attrs } => result.push(Inline::Span {
                content: resolve_roles_in_inlines(content),
                attrs,
            }),
            Inline::Link {
                url,
                title,
                content,
                attrs,
            } => result.push(Inline::Link {
                url,
                title,
                content: resolve_roles_in_inlines(content),
                attrs,
            }),

            other => result.push(other),
        }
    }

    result
}

/// If `text` ends with `{role_name}`, return `(prefix_text, role_name)`.
fn extract_role_suffix(text: &str) -> Option<(String, String)> {
    let brace_end = text.rfind('}')?;
    // The `}` must be the last character (possibly with trailing whitespace).
    if text[brace_end + 1..].trim().is_empty() {
        // Check for the opening `{`.
        let brace_start = text[..brace_end].rfind('{')?;
        let role_name = text[brace_start + 1..brace_end].trim();
        if role_name.is_empty() {
            return None;
        }
        let prefix = text[..brace_start].to_string();
        Some((prefix, role_name.to_string()))
    } else {
        None
    }
}

/// Map a role name and its argument text to the appropriate [`Inline`] node.
fn role_to_inline(role: &str, text: String) -> Inline {
    match role {
        "ref" => Inline::CrossRef(CrossRef {
            target: text,
            form: RefForm::Number,
        }),
        "numref" => Inline::CrossRef(CrossRef {
            target: text,
            form: RefForm::NumberWithType,
        }),
        "eq" => Inline::CrossRef(CrossRef {
            target: text,
            form: RefForm::Number,
        }),
        "doc" | "download" => Inline::Link {
            url: text.clone(),
            title: None,
            content: vec![Inline::Text { value: text }],
            attrs: None,
        },
        "math" => Inline::MathInline { value: text },
        "sub" => Inline::Subscript {
            content: vec![Inline::Text { value: text }],
        },
        "sup" => Inline::Superscript {
            content: vec![Inline::Text { value: text }],
        },
        "cite" | "cite:p" => Inline::Citation(Citation {
            items: vec![CiteItem {
                key: text,
                prefix: None,
                suffix: None,
            }],
            mode: CitationMode::Normal,
        }),
        "cite:t" => Inline::Citation(Citation {
            items: vec![CiteItem {
                key: text,
                prefix: None,
                suffix: None,
            }],
            mode: CitationMode::AuthorOnly,
        }),
        _ => {
            // Unknown role → Span with a `role-{name}` class.
            Inline::Span {
                content: vec![Inline::Text { value: text }],
                attrs: Attributes {
                    id: None,
                    classes: vec![format!("role-{}", role)],
                    key_values: HashMap::new(),
                },
            }
        }
    }
}

// ─── Post-processor ───────────────────────────────────────────────────────────

/// Walk the block list, expanding directive/label markers into real AST nodes
/// and resolving inline roles within paragraphs.
fn postprocess(blocks: Vec<Block>, directives: &[DirectiveBlock], labels: &[String]) -> Vec<Block> {
    let expanded = expand_markers(blocks, directives, labels);
    attach_labels(expanded)
}

/// First pass: expand `DOCMUX_MYST_DIR_N` and `DOCMUX_MYST_LABEL_N` marker
/// paragraphs and resolve inline roles.
fn expand_markers(
    blocks: Vec<Block>,
    directives: &[DirectiveBlock],
    labels: &[String],
) -> Vec<Block> {
    let mut result: Vec<Block> = Vec::with_capacity(blocks.len());

    for block in blocks {
        match block {
            Block::Paragraph { content } => {
                // Check if this is a single-text marker paragraph.
                if content.len() == 1 {
                    if let Inline::Text { value } = &content[0] {
                        // Directive marker?
                        if let Some(idx) = value.strip_prefix(DIRECTIVE_MARKER_PREFIX) {
                            if let Ok(n) = idx.parse::<usize>() {
                                if n < directives.len() {
                                    result.push(directive_to_block(directives[n].clone()));
                                    continue;
                                }
                            }
                        }

                        // Label marker?
                        if let Some(idx) = value.strip_prefix(LABEL_MARKER_PREFIX) {
                            if let Ok(n) = idx.parse::<usize>() {
                                if n < labels.len() {
                                    // Emit a special label-placeholder Div that
                                    // `attach_labels` will consume.
                                    result.push(Block::Div {
                                        attrs: Attributes {
                                            id: None,
                                            classes: Vec::new(),
                                            key_values: HashMap::from([(
                                                "__myst_label__".to_string(),
                                                labels[n].clone(),
                                            )]),
                                        },
                                        content: Vec::new(),
                                    });
                                    continue;
                                }
                            }
                        }
                    }
                }

                // Normal paragraph — resolve roles.
                result.push(Block::Paragraph {
                    content: resolve_roles_in_inlines(content),
                });
            }

            // Recurse into container blocks.
            Block::BlockQuote { content } => result.push(Block::BlockQuote {
                content: expand_markers(content, directives, labels),
            }),
            Block::List {
                ordered,
                start,
                items,
                tight,
                style,
                delimiter,
            } => {
                let new_items = items
                    .into_iter()
                    .map(|item| ListItem {
                        checked: item.checked,
                        content: expand_markers(item.content, directives, labels),
                    })
                    .collect();
                result.push(Block::List {
                    ordered,
                    start,
                    items: new_items,
                    tight,
                    style,
                    delimiter,
                });
            }
            Block::Admonition {
                kind,
                title,
                content,
            } => result.push(Block::Admonition {
                kind,
                title,
                content: expand_markers(content, directives, labels),
            }),
            Block::Div { attrs, content } => result.push(Block::Div {
                attrs,
                content: expand_markers(content, directives, labels),
            }),
            Block::FootnoteDef { id, content } => result.push(Block::FootnoteDef {
                id,
                content: expand_markers(content, directives, labels),
            }),
            // Headings — resolve roles in their inline content.
            Block::Heading {
                level,
                id,
                content,
                attrs,
            } => result.push(Block::Heading {
                level,
                id,
                content: resolve_roles_in_inlines(content),
                attrs,
            }),
            other => result.push(other),
        }
    }

    result
}

/// Second pass: find label-placeholder `Div` nodes (injected by
/// `expand_markers`) and apply the label to the immediately following sibling.
fn attach_labels(blocks: Vec<Block>) -> Vec<Block> {
    let mut result: Vec<Block> = Vec::with_capacity(blocks.len());
    let mut pending_label: Option<String> = None;

    for block in blocks {
        // Is this a label placeholder?
        if let Block::Div {
            ref attrs,
            ref content,
        } = block
        {
            if content.is_empty() {
                if let Some(label) = attrs.key_values.get("__myst_label__") {
                    pending_label = Some(label.clone());
                    continue; // consume the placeholder
                }
            }
        }

        // Apply any pending label to this block.
        let block = if let Some(label) = pending_label.take() {
            apply_label_to_block(block, label)
        } else {
            block
        };

        result.push(block);
    }

    result
}

/// Attach `label` to `block`, returning the (possibly modified) block.
fn apply_label_to_block(block: Block, label: String) -> Block {
    match block {
        Block::Heading {
            level,
            id: _,
            content,
            attrs,
        } => Block::Heading {
            level,
            id: Some(label),
            content,
            attrs,
        },
        Block::Figure {
            image,
            caption,
            label: _,
            attrs,
        } => Block::Figure {
            image,
            caption,
            label: Some(label),
            attrs,
        },
        Block::MathBlock { content, label: _ } => Block::MathBlock {
            content,
            label: Some(label),
        },
        Block::CodeBlock {
            language,
            content,
            caption,
            label: _,
            attrs,
        } => Block::CodeBlock {
            language,
            content,
            caption,
            label: Some(label),
            attrs,
        },
        Block::Table(mut table) => {
            table.label = Some(label);
            Block::Table(table)
        }
        other => {
            // For all other block types, wrap in a Div with the label as id.
            Block::Div {
                attrs: Attributes {
                    id: Some(label),
                    ..Attributes::default()
                },
                content: vec![other],
            }
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_core::Reader;

    fn read(input: &str) -> Vec<Block> {
        MystReader::new().read(input).unwrap().content
    }

    // ── Directives ───────────────────────────────────────────────────────────

    #[test]
    fn note_directive() {
        let blocks = read(":::{note}\nThis is a note.\n:::");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Admonition {
                kind,
                title,
                content,
            } => {
                assert!(matches!(kind, AdmonitionKind::Note));
                assert!(title.is_none());
                assert!(!content.is_empty());
            }
            other => panic!("Expected Admonition, got {:?}", other),
        }
    }

    #[test]
    fn warning_directive_with_title() {
        let blocks = read(":::{warning} Title Here\nBody text.\n:::");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Admonition {
                kind,
                title,
                content,
            } => {
                assert!(matches!(kind, AdmonitionKind::Warning));
                let t = title.as_ref().expect("expected title");
                assert_eq!(t.len(), 1);
                match &t[0] {
                    Inline::Text { value } => assert_eq!(value, "Title Here"),
                    other => panic!("Expected Text, got {:?}", other),
                }
                assert!(!content.is_empty());
            }
            other => panic!("Expected Admonition, got {:?}", other),
        }
    }

    #[test]
    fn figure_directive_with_options() {
        let input = ":::{figure} image.png\n:alt: A description\n:name: fig-id\nCaption text.\n:::";
        let blocks = read(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Figure {
                image,
                caption,
                label,
                ..
            } => {
                assert_eq!(image.url, "image.png");
                assert_eq!(image.alt_text(), "A description");
                assert_eq!(label.as_deref(), Some("fig-id"));
                let cap = caption.as_ref().expect("expected caption");
                assert!(!cap.is_empty());
            }
            other => panic!("Expected Figure, got {:?}", other),
        }
    }

    #[test]
    fn nested_directives() {
        // 4-colon outer wraps a 3-colon inner.
        let input = "::::{note}\n:::{warning}\nInner.\n:::\n::::";
        let blocks = read(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Admonition { kind, content, .. } => {
                assert!(matches!(kind, AdmonitionKind::Note));
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Block::Admonition {
                        kind: inner_kind, ..
                    } => {
                        assert!(matches!(inner_kind, AdmonitionKind::Warning));
                    }
                    other => panic!("Expected inner Admonition, got {:?}", other),
                }
            }
            other => panic!("Expected outer Admonition, got {:?}", other),
        }
    }

    #[test]
    fn code_block_directive() {
        let input = ":::{code-block} python\nprint('hello')\n:::";
        let blocks = read(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::CodeBlock {
                language, content, ..
            } => {
                assert_eq!(language.as_deref(), Some("python"));
                assert!(content.contains("print"));
            }
            other => panic!("Expected CodeBlock, got {:?}", other),
        }
    }

    #[test]
    fn math_directive() {
        let input = ":::{math}\n:label: eq:myeq\nE = mc^2\n:::";
        let blocks = read(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::MathBlock { content, label } => {
                assert!(content.contains("mc^2"));
                assert_eq!(label.as_deref(), Some("eq:myeq"));
            }
            other => panic!("Expected MathBlock, got {:?}", other),
        }
    }

    #[test]
    fn unknown_directive_becomes_div() {
        let input = ":::{my-custom-box}\nSome content.\n:::";
        let blocks = read(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Div { attrs, .. } => {
                assert!(attrs.classes.contains(&"my-custom-box".to_string()));
            }
            other => panic!("Expected Div, got {:?}", other),
        }
    }

    // ── Roles ─────────────────────────────────────────────────────────────────

    #[test]
    fn role_ref_crossref() {
        let blocks = read("See {ref}`my-target` for details.");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Paragraph { content } => {
                let crossref = content.iter().find(|i| matches!(i, Inline::CrossRef(_)));
                match crossref.expect("expected CrossRef") {
                    Inline::CrossRef(cr) => {
                        assert_eq!(cr.target, "my-target");
                        assert!(matches!(cr.form, RefForm::Number));
                    }
                    _ => unreachable!(),
                }
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn role_math_inline() {
        let blocks = read("Inline math: {math}`x^2`.");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Paragraph { content } => {
                let math = content
                    .iter()
                    .find(|i| matches!(i, Inline::MathInline { .. }));
                match math.expect("expected MathInline") {
                    Inline::MathInline { value } => assert_eq!(value, "x^2"),
                    _ => unreachable!(),
                }
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn role_sub_subscript() {
        let blocks = read("H{sub}`2`O");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Paragraph { content } => {
                let sub = content
                    .iter()
                    .find(|i| matches!(i, Inline::Subscript { .. }));
                match sub.expect("expected Subscript") {
                    Inline::Subscript { content: inner } => {
                        assert_eq!(inner.len(), 1);
                        match &inner[0] {
                            Inline::Text { value } => assert_eq!(value, "2"),
                            other => panic!("Expected Text inside Subscript, got {:?}", other),
                        }
                    }
                    _ => unreachable!(),
                }
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    #[test]
    fn unknown_role_becomes_span() {
        let blocks = read("{unknown-role}`content`");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Paragraph { content } => {
                let span = content.iter().find(|i| matches!(i, Inline::Span { .. }));
                match span.expect("expected Span") {
                    Inline::Span {
                        attrs,
                        content: inner,
                    } => {
                        assert!(attrs.classes.contains(&"role-unknown-role".to_string()));
                        assert_eq!(inner.len(), 1);
                        match &inner[0] {
                            Inline::Text { value } => assert_eq!(value, "content"),
                            other => panic!("Expected Text inside Span, got {:?}", other),
                        }
                    }
                    _ => unreachable!(),
                }
            }
            other => panic!("Expected Paragraph, got {:?}", other),
        }
    }

    // ── Labels ────────────────────────────────────────────────────────────────

    #[test]
    fn label_before_heading() {
        let input = "(my-label)=\n\n# Section Title";
        let blocks = read(input);
        let heading = blocks
            .iter()
            .find(|b| matches!(b, Block::Heading { .. }))
            .expect("expected Heading");
        match heading {
            Block::Heading { id, .. } => assert_eq!(id.as_deref(), Some("my-label")),
            _ => unreachable!(),
        }
    }

    #[test]
    fn label_before_figure_directive() {
        let input = "(fig:x)=\n\n:::{figure} photo.jpg\nA caption.\n:::";
        let blocks = read(input);
        let figure = blocks
            .iter()
            .find(|b| matches!(b, Block::Figure { .. }))
            .expect("expected Figure");
        match figure {
            Block::Figure { label, .. } => assert_eq!(label.as_deref(), Some("fig:x")),
            _ => unreachable!(),
        }
    }

    // ── Integration ───────────────────────────────────────────────────────────

    #[test]
    fn full_document_pipeline() {
        let input = r#"---
title: Test Doc
---

# Introduction

Some text with a role: {ref}`sec:methods`.

:::{note}
A **bold** note.
:::
"#;
        let doc = MystReader::new().read(input).unwrap();
        assert_eq!(doc.metadata.title.as_deref(), Some("Test Doc"));
        assert!(!doc.content.is_empty());

        let heading = doc
            .content
            .iter()
            .find(|b| matches!(b, Block::Heading { .. }));
        assert!(heading.is_some(), "expected a Heading");

        let admonition = doc
            .content
            .iter()
            .find(|b| matches!(b, Block::Admonition { .. }));
        assert!(admonition.is_some(), "expected an Admonition");

        let para_with_crossref = doc.content.iter().find(|b| {
            if let Block::Paragraph { content } = b {
                content.iter().any(|i| matches!(i, Inline::CrossRef(_)))
            } else {
                false
            }
        });
        assert!(
            para_with_crossref.is_some(),
            "expected paragraph with CrossRef"
        );
    }

    #[test]
    fn directive_body_parsed_as_markdown() {
        let input = ":::{note}\n**bold** and a [link](https://example.com).\n:::";
        let blocks = read(input);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            Block::Admonition { content, .. } => {
                let para = content
                    .iter()
                    .find(|b| matches!(b, Block::Paragraph { .. }));
                match para.expect("expected Paragraph inside Admonition") {
                    Block::Paragraph { content: inlines } => {
                        let has_strong = inlines.iter().any(|i| matches!(i, Inline::Strong { .. }));
                        let has_link = inlines.iter().any(|i| matches!(i, Inline::Link { .. }));
                        assert!(has_strong, "expected Strong inside directive body");
                        assert!(has_link, "expected Link inside directive body");
                    }
                    _ => unreachable!(),
                }
            }
            other => panic!("Expected Admonition, got {:?}", other),
        }
    }
}

// ─── Reader trait implementation ─────────────────────────────────────────────

/// MyST Markdown reader for docmux.
///
/// Supports MyST directives (`::: {name}`), target labels (`(label)=`), and
/// inline roles (`` {role}`text` ``), layered on top of the CommonMark + GFM
/// base provided by [`MarkdownReader`].
#[derive(Debug, Default)]
pub struct MystReader;

impl MystReader {
    /// Create a new [`MystReader`].
    pub fn new() -> Self {
        Self
    }
}

impl Reader for MystReader {
    fn format(&self) -> &str {
        "myst"
    }

    fn extensions(&self) -> &[&str] {
        &["myst"]
    }

    fn read(&self, input: &str) -> Result<Document> {
        // 1. Pre-process: extract directives and labels, emit marker paragraphs.
        let preprocessed = preprocess(input);

        // 2. Delegate base CommonMark + GFM parsing to MarkdownReader.
        let md_reader = MarkdownReader::new();
        let mut doc = md_reader.read(&preprocessed.source)?;

        // 3. Post-process: expand markers, resolve roles, attach labels.
        let processed = postprocess(doc.content, &preprocessed.directives, &preprocessed.labels);
        doc.content = processed;

        Ok(doc)
    }
}
