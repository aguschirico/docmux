//! # docmux-writer-latex
//!
//! LaTeX writer for docmux. Converts the docmux AST into LaTeX output
//! suitable for compilation with pdflatex, xelatex, or lualatex.

use docmux_ast::*;
use docmux_core::{MathEngine, Result, WriteOptions, Writer};
use docmux_highlight::{HighlightToken, LineOptions};

/// A LaTeX writer.
#[derive(Debug, Default)]
pub struct LatexWriter;

impl LatexWriter {
    pub fn new() -> Self {
        Self
    }

    fn write_blocks(&self, blocks: &[Block], opts: &WriteOptions, out: &mut String) {
        for block in blocks {
            self.write_block(block, opts, out);
        }
    }

    fn write_block(&self, block: &Block, opts: &WriteOptions, out: &mut String) {
        match block {
            Block::Paragraph { content } => {
                self.write_inlines(content, opts, out);
                out.push_str("\n\n");
            }
            Block::Heading {
                level, id, content, ..
            } => {
                let cmd = match level {
                    1 => "section",
                    2 => "subsection",
                    3 => "subsubsection",
                    4 => "paragraph",
                    5 => "subparagraph",
                    _ => "subparagraph",
                };
                out.push_str(&format!("\\{cmd}{{"));
                let inlines = strip_section_number_prefix(content);
                self.write_inlines(inlines, opts, out);
                out.push('}');
                if let Some(id) = id {
                    out.push_str(&format!("\\label{{{}}}", escape_label(id)));
                }
                out.push('\n');
            }
            Block::CodeBlock {
                language,
                content,
                attrs,
                ..
            } => {
                let line_opts = LineOptions::from_attrs(attrs.as_ref());
                if let (Some(lang), Some(theme)) =
                    (language.as_deref(), opts.highlight_style.as_deref())
                {
                    if let Ok(lines) = docmux_highlight::highlight(content, lang, theme) {
                        write_highlighted_code(&lines, &line_opts, out);
                    } else {
                        // Highlight failed — fall back to lstlisting
                        write_lstlisting(language.as_deref(), content, &line_opts, out);
                    }
                } else {
                    write_lstlisting(language.as_deref(), content, &line_opts, out);
                }
            }
            Block::MathBlock { content, label } => {
                if let Some(label) = label {
                    out.push_str("\\begin{equation}\n");
                    out.push_str(&format!("\\label{{{}}}\n", escape_label(label)));
                    out.push_str(content.trim());
                    out.push('\n');
                    out.push_str("\\end{equation}\n");
                } else {
                    out.push_str("\\[\n");
                    out.push_str(content.trim());
                    out.push('\n');
                    out.push_str("\\]\n");
                }
            }
            Block::BlockQuote { content } => {
                out.push_str("\\begin{quote}\n");
                self.write_blocks(content, opts, out);
                out.push_str("\\end{quote}\n");
            }
            Block::List {
                ordered,
                start,
                items,
                tight,
                ..
            } => {
                let env = if *ordered { "enumerate" } else { "itemize" };
                out.push_str(&format!("\\begin{{{env}}}\n"));
                if *tight {
                    out.push_str("\\tightlist\n");
                }
                if *ordered {
                    if let Some(s) = start {
                        if *s != 1 {
                            out.push_str(&format!("\\setcounter{{enumi}}{{{}}}\n", s - 1));
                        }
                    }
                }
                for item in items {
                    if let Some(checked) = item.checked {
                        let marker = if checked {
                            "$\\boxtimes$"
                        } else {
                            "$\\square$"
                        };
                        out.push_str(&format!("\\item[{marker}] "));
                    } else {
                        out.push_str("\\item ");
                    }
                    self.write_blocks(&item.content, opts, out);
                }
                out.push_str(&format!("\\end{{{env}}}\n"));
            }
            Block::Table(table) => {
                self.write_table(table, opts, out);
            }
            Block::Figure {
                image,
                caption,
                label,
                ..
            } => {
                out.push_str("\\begin{figure}[htbp]\n");
                out.push_str("\\centering\n");
                let img_opts = includegraphics_options(image.attrs.as_ref());
                out.push_str(&format!(
                    "\\includegraphics{img_opts}{{{}}}\n",
                    escape_latex(&image.url)
                ));
                if let Some(cap) = caption {
                    out.push_str("\\caption{");
                    self.write_inlines(cap, opts, out);
                    out.push_str("}\n");
                }
                if let Some(label) = label {
                    out.push_str(&format!("\\label{{{}}}\n", escape_label(label)));
                }
                out.push_str("\\end{figure}\n");
            }
            Block::ThematicBreak => {
                out.push_str("\n\\noindent\\rule{\\textwidth}{0.4pt}\n\n");
            }
            Block::RawBlock { format, content } => {
                if format == "latex" || format == "tex" {
                    out.push_str(content);
                    out.push('\n');
                }
                // Non-LaTeX raw blocks are silently dropped
            }
            Block::Admonition {
                kind,
                title,
                content,
            } => {
                // Use a simple framed box. A full implementation would
                // use tcolorbox or mdframed, but those require extra packages.
                let label = match kind {
                    AdmonitionKind::Note => "Note",
                    AdmonitionKind::Warning => "Warning",
                    AdmonitionKind::Tip => "Tip",
                    AdmonitionKind::Important => "Important",
                    AdmonitionKind::Caution => "Caution",
                    AdmonitionKind::Custom(c) => c.as_str(),
                };
                out.push_str("\\begin{quote}\n");
                if let Some(t) = title {
                    out.push_str("\\textbf{");
                    self.write_inlines(t, opts, out);
                    out.push_str("}\n\n");
                } else {
                    out.push_str(&format!("\\textbf{{{label}}}\n\n"));
                }
                self.write_blocks(content, opts, out);
                out.push_str("\\end{quote}\n");
            }
            Block::DefinitionList { items } => {
                out.push_str("\\begin{description}\n");
                for item in items {
                    out.push_str("\\item[");
                    self.write_inlines(&item.term, opts, out);
                    out.push_str("] ");
                    for def in &item.definitions {
                        self.write_blocks(def, opts, out);
                    }
                }
                out.push_str("\\end{description}\n");
            }
            Block::Div { attrs, content } => {
                if attrs.classes.iter().any(|c| c == "toc") {
                    // LaTeX generates its own TOC from \section commands.
                    out.push_str("\\tableofcontents\n");
                } else {
                    // LaTeX has no generic div; emit content directly.
                    self.write_blocks(content, opts, out);
                }
            }
            Block::FootnoteDef { id, content } => {
                // Footnotes in LaTeX are typically inline (\footnote{...}),
                // but since we get them as separate blocks from the AST, we
                // emit them as end-notes using a label for cross-referencing.
                out.push_str(&format!("\\footnotetext[{}]{{", escape_latex(id)));
                // Write content inline (strip trailing paragraph breaks)
                let mut inner = String::new();
                self.write_blocks(content, opts, &mut inner);
                out.push_str(inner.trim());
                out.push_str("}\n");
            }
        }
    }

    fn write_table(&self, table: &Table, opts: &WriteOptions, out: &mut String) {
        // Build column spec string: l, c, r based on alignment
        let col_spec: String = table
            .columns
            .iter()
            .map(|c| match c.alignment {
                Alignment::Left | Alignment::Default => 'l',
                Alignment::Center => 'c',
                Alignment::Right => 'r',
            })
            .collect();

        // If there are no column specs, infer from header/first row
        let col_spec = if col_spec.is_empty() {
            let n = table
                .header
                .as_ref()
                .map(|h| h.len())
                .or_else(|| table.rows.first().map(|r| r.len()))
                .unwrap_or(1);
            "l".repeat(n)
        } else {
            col_spec
        };

        out.push_str(&format!("\\begin{{longtable}}[]{{@{{}}{col_spec}@{{}}}}\n"));

        if table.caption.is_some() || table.label.is_some() {
            if let Some(cap) = &table.caption {
                out.push_str("\\caption{");
                self.write_inlines(cap, opts, out);
                out.push('}');
            }
            if let Some(label) = &table.label {
                out.push_str(&format!("\\label{{{}}}", escape_label(label)));
            }
            out.push_str("\\\\\n");
        }

        out.push_str("\\toprule\n");
        if let Some(header) = &table.header {
            self.write_table_row(header, opts, out, true);
            out.push_str("\\midrule\n");
        }

        for row in &table.rows {
            self.write_table_row(row, opts, out, false);
        }

        if let Some(foot) = &table.foot {
            out.push_str("\\midrule\n");
            self.write_table_row(foot, opts, out, false);
        }

        out.push_str("\\bottomrule\n");
        out.push_str("\\end{longtable}\n");
    }

    fn write_table_row(
        &self,
        cells: &[TableCell],
        opts: &WriteOptions,
        out: &mut String,
        bold: bool,
    ) {
        for (i, cell) in cells.iter().enumerate() {
            if i > 0 {
                out.push_str(" & ");
            }
            let mut cell_content = String::new();
            self.write_blocks(&cell.content, opts, &mut cell_content);
            let trimmed = cell_content.trim();
            if bold {
                out.push_str(&format!("\\textbf{{{trimmed}}}"));
            } else {
                out.push_str(trimmed);
            }
        }
        out.push_str(" \\\\\n");
    }

    fn write_inlines(&self, inlines: &[Inline], opts: &WriteOptions, out: &mut String) {
        for inline in inlines {
            self.write_inline(inline, opts, out);
        }
    }

    fn write_inline(&self, inline: &Inline, opts: &WriteOptions, out: &mut String) {
        match inline {
            Inline::Text { value } => {
                out.push_str(&escape_latex(value));
            }
            Inline::Emphasis { content } => {
                out.push_str("\\emph{");
                self.write_inlines(content, opts, out);
                out.push('}');
            }
            Inline::Strong { content } => {
                out.push_str("\\textbf{");
                self.write_inlines(content, opts, out);
                out.push('}');
            }
            Inline::Strikethrough { content } => {
                out.push_str("\\sout{");
                self.write_inlines(content, opts, out);
                out.push('}');
            }
            Inline::Code { value, .. } => {
                // Use \texttt with escaped content (not \verb since it
                // can't be nested inside other commands)
                out.push_str("\\texttt{");
                out.push_str(&escape_latex(value));
                out.push('}');
            }
            Inline::MathInline { value } => {
                match opts.math_engine {
                    MathEngine::Raw
                    | MathEngine::KaTeX
                    | MathEngine::MathJax
                    | MathEngine::MathML => {
                        // In LaTeX output, math is always raw LaTeX
                        out.push('$');
                        out.push_str(value);
                        out.push('$');
                    }
                }
            }
            Inline::Link {
                url,
                title: _,
                content,
                ..
            } => {
                // Check if the link text matches the URL (autolink)
                let mut link_text = String::new();
                self.write_inlines(content, opts, &mut link_text);
                if link_text == *url {
                    out.push_str(&format!("\\url{{{}}}", url));
                } else {
                    out.push_str(&format!("\\href{{{}}}", url));
                    out.push('{');
                    self.write_inlines(content, opts, out);
                    out.push('}');
                }
            }
            Inline::Image(img) => {
                let img_opts = includegraphics_options(img.attrs.as_ref());
                out.push_str(&format!(
                    "\\includegraphics{img_opts}{{{}}}",
                    escape_latex(&img.url)
                ));
            }
            Inline::Citation(cite) => {
                if let Some(prefix) = cite.items.first().and_then(|i| i.prefix.as_deref()) {
                    out.push_str(&format!("{}~", escape_latex(prefix)));
                }
                match cite.mode {
                    CitationMode::Normal => {
                        out.push_str(&format!("\\cite{{{}}}", cite.keys().join(",")));
                    }
                    CitationMode::AuthorOnly => {
                        out.push_str(&format!("\\citet{{{}}}", cite.keys().join(",")));
                    }
                    CitationMode::SuppressAuthor => {
                        out.push_str(&format!("\\citeyear{{{}}}", cite.keys().join(",")));
                    }
                }
                if let Some(suffix) = cite.items.last().and_then(|i| i.suffix.as_deref()) {
                    out.push_str(&format!(" {}", escape_latex(suffix)));
                }
            }
            Inline::Quoted {
                quote_type,
                content,
            } => match quote_type {
                QuoteType::SingleQuote => {
                    out.push('`');
                    self.write_inlines(content, opts, out);
                    out.push('\'');
                }
                QuoteType::DoubleQuote => {
                    out.push_str("``");
                    self.write_inlines(content, opts, out);
                    out.push_str("''");
                }
            },
            Inline::FootnoteRef { id } => {
                out.push_str(&format!("\\footnotemark[{}]", escape_latex(id)));
            }
            Inline::CrossRef(cr) => match cr.form {
                RefForm::Number => {
                    out.push_str(&format!("\\ref{{{}}}", escape_label(&cr.target)));
                }
                RefForm::NumberWithType => {
                    out.push_str(&format!("\\autoref{{{}}}", escape_label(&cr.target)));
                }
                RefForm::Page => {
                    out.push_str(&format!("\\pageref{{{}}}", escape_label(&cr.target)));
                }
                RefForm::Custom(ref text) => {
                    out.push_str(&format!(
                        "{}~\\ref{{{}}}",
                        escape_latex(text),
                        escape_label(&cr.target)
                    ));
                }
            },
            Inline::RawInline { format, content } => {
                if format == "latex" || format == "tex" {
                    out.push_str(content);
                }
            }
            Inline::Superscript { content } => {
                out.push_str("\\textsuperscript{");
                self.write_inlines(content, opts, out);
                out.push('}');
            }
            Inline::Subscript { content } => {
                out.push_str("\\textsubscript{");
                self.write_inlines(content, opts, out);
                out.push('}');
            }
            Inline::SmallCaps { content } => {
                out.push_str("\\textsc{");
                self.write_inlines(content, opts, out);
                out.push('}');
            }
            Inline::SoftBreak => {
                out.push('\n');
            }
            Inline::HardBreak => {
                out.push_str("\\\\\n");
            }
            Inline::Underline { content } => {
                out.push_str("\\underline{");
                self.write_inlines(content, opts, out);
                out.push('}');
            }
            Inline::Span { content, .. } => {
                // LaTeX doesn't have a generic span; just emit content
                self.write_inlines(content, opts, out);
            }
        }
    }

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
                        map.insert("affiliation".into(), TemplateValue::Str(escape_latex(aff)));
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
}

impl Writer for LatexWriter {
    fn format(&self) -> &str {
        "latex"
    }

    fn default_extension(&self) -> &str {
        "tex"
    }

    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
        let mut body = String::with_capacity(4096);
        self.write_blocks(&doc.content, opts, &mut body);

        if opts.standalone {
            self.wrap_standalone(&body, doc, opts)
        } else {
            Ok(body)
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Write a code block using `\begin{lstlisting}...\end{lstlisting}`.
fn write_lstlisting(
    language: Option<&str>,
    content: &str,
    line_opts: &LineOptions,
    out: &mut String,
) {
    let mut options: Vec<String> = Vec::new();
    if let Some(lang) = language {
        options.push(format!("language={lang}"));
    }
    if line_opts.number_lines {
        options.push("numbers=left".into());
        if line_opts.start_from != 1 {
            options.push(format!("firstnumber={}", line_opts.start_from));
        }
    }
    if options.is_empty() {
        out.push_str("\\begin{lstlisting}\n");
    } else {
        out.push_str(&format!("\\begin{{lstlisting}}[{}]\n", options.join(",")));
    }
    // Code blocks are verbatim — no escaping
    out.push_str(content);
    if !content.ends_with('\n') {
        out.push('\n');
    }
    out.push_str("\\end{lstlisting}\n");
}

/// Write syntax-highlighted code inside an `alltt` environment using
/// `\textcolor[RGB]` commands for coloured tokens.
fn write_highlighted_code(
    lines: &[Vec<HighlightToken>],
    line_opts: &LineOptions,
    out: &mut String,
) {
    out.push_str("\\begin{alltt}\n");
    for (idx, line) in lines.iter().enumerate() {
        let line_no = line_opts.start_from + idx as u32;
        let highlight = line_opts.is_highlighted(line_no);

        let mut line_content = String::new();
        if line_opts.number_lines {
            line_content.push_str(&format!("\\makebox[2em][r]{{{}}}\\;\\,", line_no));
        }
        for token in line {
            line_content.push_str(&render_token(token));
        }

        if highlight {
            // Strip trailing newline before wrapping, re-add after
            let trimmed = line_content.trim_end_matches('\n');
            out.push_str(&format!("\\colorbox{{yellow!15}}{{{trimmed}}}\n"));
        } else {
            out.push_str(&line_content);
        }
    }
    out.push_str("\\end{alltt}\n");
}

/// Render a single `HighlightToken` into its LaTeX representation.
fn render_token(token: &HighlightToken) -> String {
    let escaped = latex_escape_verbatim(&token.text);
    let c = token.style.foreground;
    let mut inner = format!("\\textcolor[RGB]{{{},{},{}}}{{", c.r, c.g, c.b);
    inner.push_str(&escaped);
    inner.push('}');
    if token.style.bold {
        inner = format!("\\textbf{{{inner}}}");
    }
    if token.style.italic {
        inner = format!("\\textit{{{inner}}}");
    }
    inner
}

/// Escape characters that are special inside the `alltt` environment.
/// Only `\`, `{`, and `}` need escaping in alltt.
fn latex_escape_verbatim(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            _ => out.push(c),
        }
    }
    out
}

/// Escape LaTeX special characters: # $ % & ~ _ ^ \ { }
fn escape_latex(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '#' => out.push_str("\\#"),
            '$' => out.push_str("\\$"),
            '%' => out.push_str("\\%"),
            '&' => out.push_str("\\&"),
            '~' => out.push_str("\\textasciitilde{}"),
            '_' => out.push_str("\\_"),
            '^' => out.push_str("\\textasciicircum{}"),
            _ => out.push(c),
        }
    }
    out
}

/// Build the `[key=value,...]` options string for `\includegraphics` from
/// image attributes.  Returns an empty string when there are no options.
///
/// Dimension mapping:
///   - `"100%"` → `\textwidth`
///   - `"<N>%"` → `<N/100>\textwidth`  (e.g. `"50%"` → `0.5\textwidth`)
///   - Anything else (e.g. `"10cm"`, `"3in"`) → passed through unchanged.
fn includegraphics_options(attrs: Option<&Attributes>) -> String {
    let Some(attrs) = attrs else {
        return String::new();
    };
    let mut opts: Vec<String> = Vec::new();
    let mut has_width = false;
    let mut has_height = false;
    for key in &["width", "height"] {
        if let Some(val) = attrs.key_values.get(*key) {
            let latex_val = css_dim_to_latex(val, key);
            opts.push(format!("{key}={latex_val}"));
            if *key == "width" {
                has_width = true;
            } else {
                has_height = true;
            }
        }
    }
    if opts.is_empty() {
        return String::new();
    }
    // Cap the missing dimension so tall/wide images can't overflow the page,
    // and preserve aspect ratio so no image gets distorted.
    if !has_height {
        opts.push("height=\\textheight".to_string());
    }
    if !has_width {
        opts.push("width=\\textwidth".to_string());
    }
    opts.push("keepaspectratio".to_string());
    format!("[{}]", opts.join(","))
}

/// Convert a CSS-style dimension value to its LaTeX equivalent.
fn css_dim_to_latex(value: &str, key: &str) -> String {
    let reference = if key == "height" {
        "\\textheight"
    } else {
        "\\textwidth"
    };
    if value == "100%" {
        reference.to_string()
    } else if let Some(pct) = value.strip_suffix('%') {
        if let Ok(n) = pct.parse::<f64>() {
            let frac = n / 100.0;
            // Avoid trailing zeros: 0.50 → "0.5", 0.30 → "0.3"
            let formatted = format!("{frac}");
            format!("{formatted}{reference}")
        } else {
            value.to_string()
        }
    } else {
        value.to_string()
    }
}

/// Strip the number prefix that `NumberSectionsTransform` prepends to
/// heading content.  The transform always inserts exactly two `Inline::Text`
/// nodes at the front: the number (e.g. `"1"` or `"2.3.1"`) and a space
/// `" "`.  LaTeX numbers sections natively, so we skip these two nodes.
fn strip_section_number_prefix(inlines: &[Inline]) -> &[Inline] {
    if let [Inline::Text { value: num }, Inline::Text { value: sep }, ..] = inlines {
        let is_section_number =
            !num.is_empty() && num.chars().all(|c| c.is_ascii_digit() || c == '.');
        if is_section_number && sep == " " {
            return &inlines[2..];
        }
    }
    inlines
}

/// Sanitize a string for use as a LaTeX \label{} argument.
/// Labels can contain letters, digits, colons, hyphens, and dots.
fn escape_label(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ':' || c == '-' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn write_latex(doc: &Document) -> String {
        let writer = LatexWriter::new();
        writer.write(doc, &WriteOptions::default()).unwrap()
    }

    #[test]
    fn paragraph() {
        let doc = Document {
            content: vec![Block::text("Hello!")],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert_eq!(tex.trim(), "Hello!");
    }

    #[test]
    fn special_characters_escaped() {
        let doc = Document {
            content: vec![Block::text("Price: $10 & 20% off #1")],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(tex.contains("\\$10"));
        assert!(tex.contains("\\&"));
        assert!(tex.contains("20\\%"));
        assert!(tex.contains("\\#1"));
    }

    #[test]
    fn heading_levels() {
        let doc = Document {
            content: vec![
                Block::heading(1, "Section"),
                Block::heading(2, "Subsection"),
                Block::heading(3, "Subsubsection"),
            ],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(tex.contains("\\section{Section}"));
        assert!(tex.contains("\\subsection{Subsection}"));
        assert!(tex.contains("\\subsubsection{Subsubsection}"));
    }

    #[test]
    fn inline_math() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::text("The formula "),
                    Inline::MathInline {
                        value: "E = mc^2".into(),
                    },
                    Inline::text(" is famous."),
                ],
            }],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(tex.contains("$E = mc^2$"));
    }

    #[test]
    fn display_math_unlabelled() {
        let doc = Document {
            content: vec![Block::MathBlock {
                content: "x^2 + y^2 = z^2".into(),
                label: None,
            }],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(tex.contains("\\[\nx^2 + y^2 = z^2\n\\]"));
    }

    #[test]
    fn display_math_labelled() {
        let doc = Document {
            content: vec![Block::MathBlock {
                content: "E = mc^2".into(),
                label: Some("eq:einstein".into()),
            }],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(tex.contains("\\begin{equation}"));
        assert!(tex.contains("\\label{eq:einstein}"));
        assert!(tex.contains("\\end{equation}"));
    }

    #[test]
    fn code_block() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "print('hello')".into(),
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(tex.contains("\\begin{lstlisting}[language=python]"));
        assert!(tex.contains("print('hello')"));
        assert!(tex.contains("\\end{lstlisting}"));
    }

    #[test]
    fn standalone_mode() {
        let doc = Document {
            metadata: Metadata {
                title: Some("My Paper".into()),
                authors: vec![Author {
                    name: "Jane Doe".into(),
                    affiliation: Some("MIT".into()),
                    email: None,
                    orcid: None,
                }],
                date: Some("2026".into()),
                abstract_text: Some(vec![Block::text("This paper is about things.")]),
                ..Default::default()
            },
            content: vec![Block::text("Body text.")],
            ..Default::default()
        };
        let writer = LatexWriter::new();
        let opts = WriteOptions {
            standalone: true,
            ..Default::default()
        };
        let tex = writer.write(&doc, &opts).unwrap();
        assert!(tex.contains("\\documentclass{article}"));
        assert!(tex.contains("\\title{My Paper}"));
        assert!(tex.contains("\\author{Jane Doe \\\\ MIT}"));
        assert!(tex.contains("\\date{2026}"));
        assert!(tex.contains("\\maketitle"));
        assert!(tex.contains("\\begin{abstract}"));
        assert!(tex.contains("\\end{document}"));
        // Pandoc-parity preamble directives (issue #2, items 1-7)
        assert!(tex.contains("\\usepackage{iftex}"));
        assert!(tex.contains("\\usepackage{longtable,booktabs}"));
        assert!(tex.contains("\\usepackage{microtype}"));
        assert!(tex.contains("\\setcounter{secnumdepth}{-\\maxdimen}"));
        assert!(tex.contains("\\setlength{\\emergencystretch}{3em}"));
        assert!(tex.contains("\\providecommand{\\tightlist}"));
    }

    #[test]
    fn emphasis_and_strong() {
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![
                    Inline::Emphasis {
                        content: vec![Inline::text("italic")],
                    },
                    Inline::text(" and "),
                    Inline::Strong {
                        content: vec![Inline::text("bold")],
                    },
                ],
            }],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(tex.contains("\\emph{italic}"));
        assert!(tex.contains("\\textbf{bold}"));
    }

    #[test]
    fn writer_trait_metadata() {
        let writer = LatexWriter::new();
        assert_eq!(writer.format(), "latex");
        assert_eq!(writer.default_extension(), "tex");
    }

    #[test]
    fn code_block_with_highlighting() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("rust".into()),
                content: "fn main() {}".into(),
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let writer = LatexWriter::new();
        let opts = WriteOptions {
            highlight_style: Some("InspiredGitHub".into()),
            ..Default::default()
        };
        let tex = writer.write(&doc, &opts).unwrap();
        assert!(
            tex.contains("\\textcolor[RGB]"),
            "expected \\textcolor[RGB] in highlighted output, got: {tex}"
        );
        assert!(
            !tex.contains("\\begin{lstlisting}"),
            "should NOT fall back to lstlisting when highlighting succeeds"
        );
    }

    #[test]
    fn code_block_highlighting_fallback() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("nonexistent-xyz".into()),
                content: "fn main() {}".into(),
                caption: None,
                label: None,
                attrs: None,
            }],
            ..Default::default()
        };
        let writer = LatexWriter::new();
        let opts = WriteOptions {
            highlight_style: Some("InspiredGitHub".into()),
            ..Default::default()
        };
        let tex = writer.write(&doc, &opts).unwrap();
        assert!(
            tex.contains("\\begin{lstlisting}"),
            "expected lstlisting fallback for unknown language, got: {tex}"
        );
    }

    #[test]
    fn code_block_with_line_numbers_latex() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "a = 1\nb = 2".into(),
                attrs: Some(Attributes {
                    id: None,
                    classes: vec!["numberLines".into()],
                    key_values: std::collections::HashMap::new(),
                }),
                caption: None,
                label: None,
            }],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
            bibliography: None,
        };
        let writer = LatexWriter::new();
        let opts = WriteOptions::default();
        let latex = writer.write(&doc, &opts).unwrap();
        // lstlisting path should have numbers=left
        assert!(
            latex.contains("numbers=left"),
            "should have line numbers option"
        );
    }

    #[test]
    fn code_block_highlight_lines_latex() {
        let mut kvs = std::collections::HashMap::new();
        kvs.insert("highlight".into(), "2".into());
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "a\nb\nc".into(),
                attrs: Some(Attributes {
                    id: None,
                    classes: vec![],
                    key_values: kvs,
                }),
                caption: None,
                label: None,
            }],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
            bibliography: None,
        };
        let writer = LatexWriter::new();
        let opts = WriteOptions {
            highlight_style: Some("InspiredGitHub".into()),
            ..Default::default()
        };
        let latex = writer.write(&doc, &opts).unwrap();
        assert!(
            latex.contains("colorbox"),
            "highlighted line should use colorbox"
        );
    }

    // ─── Bug #1: TOC div should emit \tableofcontents ──────────────────────

    #[test]
    fn toc_div_emits_tableofcontents() {
        // The TocTransform produces a Div with class "toc" containing a nested
        // list with href links. The LaTeX writer should recognise this and emit
        // `\tableofcontents` instead of rendering the list.
        let toc_div = Block::Div {
            attrs: Attributes {
                id: None,
                classes: vec!["toc".into()],
                key_values: std::collections::HashMap::new(),
            },
            content: vec![Block::List {
                ordered: false,
                start: None,
                items: vec![docmux_ast::ListItem {
                    content: vec![Block::Paragraph {
                        content: vec![Inline::Link {
                            url: "#intro".into(),
                            title: None,
                            content: vec![Inline::text("Introduction")],
                            attrs: None,
                        }],
                    }],
                    checked: None,
                }],
                tight: false,
                style: None,
                delimiter: None,
            }],
        };
        let doc = Document {
            content: vec![toc_div],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(
            tex.contains("\\tableofcontents"),
            "TOC div should emit \\tableofcontents, got: {tex}"
        );
        assert!(
            !tex.contains("\\begin{itemize}"),
            "TOC div should NOT emit \\begin{{itemize}}, got: {tex}"
        );
    }

    // ─── Bug #2: Section numbers should be stripped for LaTeX ───────────────

    #[test]
    fn heading_strips_number_prefix() {
        // After NumberSectionsTransform, heading content looks like:
        // [Text("1"), Text(" "), Text("Introduction")]
        // The LaTeX writer should strip the number prefix because LaTeX
        // handles section numbering natively.
        let doc = Document {
            content: vec![Block::Heading {
                level: 1,
                id: Some("intro".into()),
                content: vec![
                    Inline::Text {
                        value: "1".to_string(),
                    },
                    Inline::Text {
                        value: " ".to_string(),
                    },
                    Inline::Text {
                        value: "Introduction".to_string(),
                    },
                ],
                attrs: None,
            }],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(
            tex.contains("\\section{Introduction}"),
            "heading should not include the number prefix, got: {tex}"
        );
        assert!(
            !tex.contains("\\section{1 Introduction}"),
            "heading must NOT bake in the section number, got: {tex}"
        );
    }

    #[test]
    fn subsection_strips_dotted_number_prefix() {
        // NumberSectionsTransform produces "1.1 " prefix for level 2.
        let doc = Document {
            content: vec![Block::Heading {
                level: 2,
                id: Some("details".into()),
                content: vec![
                    Inline::Text {
                        value: "1.1".to_string(),
                    },
                    Inline::Text {
                        value: " ".to_string(),
                    },
                    Inline::Text {
                        value: "Details".to_string(),
                    },
                ],
                attrs: None,
            }],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(
            tex.contains("\\subsection{Details}"),
            "subsection should strip dotted number prefix, got: {tex}"
        );
    }

    // ─── Bug #3: Image attributes → \includegraphics options ───────────────

    #[test]
    fn inline_image_with_width_attr() {
        let mut kvs = std::collections::HashMap::new();
        kvs.insert("width".into(), "100%".into());
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Image(Image {
                    url: "diagram.pdf".into(),
                    alt: vec![],
                    title: None,
                    attrs: Some(Attributes {
                        id: None,
                        classes: vec![],
                        key_values: kvs,
                    }),
                })],
            }],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(
            tex.contains(
                "\\includegraphics[width=\\textwidth,height=\\textheight,keepaspectratio]{diagram.pdf}"
            ),
            "100% width should map to width=\\textwidth with height cap and keepaspectratio, got: {tex}"
        );
    }

    #[test]
    fn figure_image_with_width_attr() {
        let mut kvs = std::collections::HashMap::new();
        kvs.insert("width".into(), "50%".into());
        let doc = Document {
            content: vec![Block::Figure {
                image: Image {
                    url: "photo.png".into(),
                    alt: vec![],
                    title: None,
                    attrs: Some(Attributes {
                        id: None,
                        classes: vec![],
                        key_values: kvs,
                    }),
                },
                caption: Some(vec![Inline::text("A photo")]),
                label: Some("fig:photo".into()),
                attrs: None,
            }],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(
            tex.contains(
                "\\includegraphics[width=0.5\\textwidth,height=\\textheight,keepaspectratio]{photo.png}"
            ),
            "50% width should map to width=0.5\\textwidth with height cap and keepaspectratio, got: {tex}"
        );
    }

    #[test]
    fn image_with_explicit_dimension() {
        let mut kvs = std::collections::HashMap::new();
        kvs.insert("width".into(), "10cm".into());
        let doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Image(Image {
                    url: "fig.png".into(),
                    alt: vec![],
                    title: None,
                    attrs: Some(Attributes {
                        id: None,
                        classes: vec![],
                        key_values: kvs,
                    }),
                })],
            }],
            ..Default::default()
        };
        let tex = write_latex(&doc);
        assert!(
            tex.contains(
                "\\includegraphics[width=10cm,height=\\textheight,keepaspectratio]{fig.png}"
            ),
            "explicit dimension should pass through with height cap and keepaspectratio, got: {tex}"
        );
    }
}
