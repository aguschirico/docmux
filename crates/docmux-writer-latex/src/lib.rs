//! # docmux-writer-latex
//!
//! LaTeX writer for docmux. Converts the docmux AST into LaTeX output
//! suitable for compilation with pdflatex, xelatex, or lualatex.

use docmux_ast::*;
use docmux_core::{MathEngine, Result, WriteOptions, Writer};
use docmux_highlight::HighlightToken;

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
                self.write_inlines(content, opts, out);
                out.push('}');
                if let Some(id) = id {
                    out.push_str(&format!("\\label{{{}}}", escape_label(id)));
                }
                out.push('\n');
            }
            Block::CodeBlock {
                language, content, ..
            } => {
                if let (Some(lang), Some(theme)) =
                    (language.as_deref(), opts.highlight_style.as_deref())
                {
                    if let Ok(lines) = docmux_highlight::highlight(content, lang, theme) {
                        write_highlighted_code(&lines, out);
                    } else {
                        // Highlight failed — fall back to lstlisting
                        write_lstlisting(language.as_deref(), content, out);
                    }
                } else {
                    write_lstlisting(language.as_deref(), content, out);
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
                ..
            } => {
                let env = if *ordered { "enumerate" } else { "itemize" };
                out.push_str(&format!("\\begin{{{env}}}\n"));
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
                out.push_str(&format!(
                    "\\includegraphics{{{}}}\n",
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
            Block::Div { content, .. } => {
                // LaTeX has no generic div; emit content directly
                self.write_blocks(content, opts, out);
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

        out.push_str("\\begin{table}[htbp]\n");
        out.push_str("\\centering\n");

        if let Some(cap) = &table.caption {
            out.push_str("\\caption{");
            self.write_inlines(cap, opts, out);
            out.push_str("}\n");
        }
        if let Some(label) = &table.label {
            out.push_str(&format!("\\label{{{}}}\n", escape_label(label)));
        }

        out.push_str(&format!("\\begin{{tabular}}{{{col_spec}}}\n"));
        out.push_str("\\hline\n");

        if let Some(header) = &table.header {
            self.write_table_row(header, opts, out, true);
            out.push_str("\\hline\n");
        }

        for row in &table.rows {
            self.write_table_row(row, opts, out, false);
        }

        if let Some(foot) = &table.foot {
            self.write_table_row(foot, opts, out, false);
        }

        out.push_str("\\hline\n");
        out.push_str("\\end{tabular}\n");
        out.push_str("\\end{table}\n");
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
                    MathEngine::Raw | MathEngine::KaTeX | MathEngine::MathJax => {
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
                out.push_str(&format!("\\includegraphics{{{}}}", escape_latex(&img.url)));
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

    fn wrap_standalone(&self, body: &str, doc: &Document, opts: &WriteOptions) -> String {
        let mut preamble = String::with_capacity(1024);

        preamble.push_str("\\documentclass{article}\n");
        preamble.push_str("\\usepackage[utf8]{inputenc}\n");
        preamble.push_str("\\usepackage[T1]{fontenc}\n");
        preamble.push_str("\\usepackage{amsmath,amssymb}\n");
        preamble.push_str("\\usepackage{graphicx}\n");
        preamble.push_str("\\usepackage{hyperref}\n");
        preamble.push_str("\\usepackage{listings}\n");
        preamble.push_str("\\usepackage{alltt}\n");
        preamble.push_str("\\usepackage{xcolor}\n");
        preamble.push_str("\\usepackage[normalem]{ulem}\n"); // for \sout (strikethrough)

        if let Some(title) = &doc.metadata.title {
            preamble.push_str(&format!("\\title{{{}}}\n", escape_latex(title)));
        }

        if !doc.metadata.authors.is_empty() {
            let authors: Vec<String> = doc
                .metadata
                .authors
                .iter()
                .map(|a| {
                    let mut s = escape_latex(&a.name);
                    if let Some(aff) = &a.affiliation {
                        s.push_str(&format!(" \\\\ {}", escape_latex(aff)));
                    }
                    s
                })
                .collect();
            preamble.push_str(&format!("\\author{{{}}}\n", authors.join(" \\and ")));
        }

        if let Some(date) = &doc.metadata.date {
            preamble.push_str(&format!("\\date{{{}}}\n", escape_latex(date)));
        }

        preamble.push_str("\n\\begin{document}\n");

        if doc.metadata.title.is_some() {
            preamble.push_str("\\maketitle\n");
        }

        if let Some(blocks) = &doc.metadata.abstract_text {
            preamble.push_str("\\begin{abstract}\n");
            self.write_blocks(blocks, opts, &mut preamble);
            preamble.push_str("\\end{abstract}\n");
        }

        preamble.push('\n');
        preamble.push_str(body);
        preamble.push_str("\\end{document}\n");

        preamble
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
            Ok(self.wrap_standalone(&body, doc, opts))
        } else {
            Ok(body)
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Write a code block using `\begin{lstlisting}...\end{lstlisting}`.
fn write_lstlisting(language: Option<&str>, content: &str, out: &mut String) {
    if let Some(lang) = language {
        out.push_str(&format!("\\begin{{lstlisting}}[language={}]\n", lang));
    } else {
        out.push_str("\\begin{lstlisting}\n");
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
fn write_highlighted_code(lines: &[Vec<HighlightToken>], out: &mut String) {
    out.push_str("\\begin{alltt}\n");
    for line in lines {
        for token in line {
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
            out.push_str(&inner);
        }
    }
    out.push_str("\\end{alltt}\n");
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
}
