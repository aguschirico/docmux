//! # docmux CLI
//!
//! Command-line interface for the docmux universal document converter.

use clap::Parser;
use docmux_ast::{Block, Metadata};
use docmux_core::{Eol, MathEngine, Registry, Transform, TransformContext, WrapMode, WriteOptions};
use docmux_reader_docx::DocxReader;
use docmux_reader_html::HtmlReader;
use docmux_reader_latex::LatexReader;
use docmux_reader_markdown::MarkdownReader;
use docmux_reader_myst::MystReader;
use docmux_reader_typst::TypstReader;
use docmux_transform_cite::CiteTransform;
use docmux_transform_math::{MathNotation, MathTarget, MathTransform};
use docmux_transform_number_sections::NumberSectionsTransform;
use docmux_transform_section_divs::SectionDivsTransform;
use docmux_transform_toc::TocTransform;
use docmux_writer_docx::DocxWriter;
use docmux_writer_html::HtmlWriter;
use docmux_writer_latex::LatexWriter;
use docmux_writer_markdown::MarkdownWriter;
use docmux_writer_plaintext::PlaintextWriter;
use docmux_writer_typst::TypstWriter;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "docmux",
    about = "Universal document converter — MIT licensed, WASM-first",
    version
)]
struct Cli {
    /// Input file(s). Use `-` for stdin.
    #[arg(required_unless_present_any = ["list_input_formats", "list_output_formats", "list_highlight_themes", "list_highlight_languages", "print_default_template"])]
    input: Vec<PathBuf>,

    /// Output file path (use `-` for stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Input format (auto-detected from extension if omitted)
    #[arg(short, long)]
    from: Option<String>,

    /// Output format (auto-detected from extension if omitted).
    /// Use `json` to dump the raw AST.
    #[arg(short, long)]
    to: Option<String>,

    /// Produce a standalone document (full HTML with <head>, etc.)
    #[arg(short, long)]
    standalone: bool,

    /// Math rendering engine for HTML output
    #[arg(long, value_name = "ENGINE", value_parser = ["katex", "mathjax", "mathml", "raw"])]
    math: Option<String>,

    /// CSS stylesheet URL (HTML output only, can be repeated)
    #[arg(long, value_name = "URL")]
    css: Vec<String>,

    /// Set metadata value (can be repeated: -M title="My Doc" -M date=2026)
    #[arg(short = 'M', long = "metadata", value_name = "KEY=VAL")]
    metadata: Vec<String>,

    /// Set template variable (can be repeated: --variable lang=es)
    #[arg(long = "variable", value_name = "KEY=VAL")]
    variable: Vec<String>,

    /// Shift heading levels by N (positive = demote, negative = promote)
    #[arg(long, value_name = "N", allow_hyphen_values = true)]
    shift_heading_level_by: Option<i8>,

    /// Include a table of contents in the output
    #[arg(long)]
    toc: bool,

    /// Maximum heading depth for the table of contents (1–6, default 3)
    #[arg(long, value_name = "N", default_value = "3")]
    toc_depth: u8,

    /// Number section headings (1, 1.1, 1.1.1, …)
    #[arg(short = 'N', long)]
    number_sections: bool,

    /// How to interpret top-level headings when numbering (section, chapter, part)
    #[arg(long, value_name = "TYPE", value_parser = ["section", "chapter", "part"], default_value = "section")]
    top_level_division: String,

    /// Text wrapping: auto (at --columns width), none (no wrapping), preserve (keep source breaks)
    #[arg(long, value_name = "MODE", value_parser = ["auto", "none", "preserve"], default_value = "none")]
    wrap: String,

    /// Column width for --wrap=auto (default 72)
    #[arg(long, value_name = "N", default_value = "72")]
    columns: usize,

    /// Line ending style: lf, crlf, native
    #[arg(long, value_name = "STYLE", value_parser = ["lf", "crlf", "native"], default_value = "lf")]
    eol: String,

    /// List supported input formats and exit
    #[arg(long)]
    list_input_formats: bool,

    /// List supported output formats and exit
    #[arg(long)]
    list_output_formats: bool,

    /// Verbose output (show warnings and diagnostics on stderr)
    #[arg(long, short = 'v')]
    verbose: bool,

    /// Suppress all non-error output on stderr
    #[arg(long, short = 'q', conflicts_with = "verbose")]
    quiet: bool,

    /// Syntax highlighting theme for code blocks (e.g. "InspiredGitHub")
    #[arg(long, value_name = "STYLE")]
    highlight_style: Option<String>,

    /// Prefix for auto-generated identifiers (e.g. --id-prefix=ch1-)
    #[arg(long, value_name = "PREFIX")]
    id_prefix: Option<String>,

    /// Wrap sections (heading + content) in <div> containers
    #[arg(long)]
    section_divs: bool,

    /// Bibliography file(s) — BibTeX (.bib) or Hayagriva YAML (.yml/.yaml)
    #[arg(long, value_name = "FILE")]
    bibliography: Vec<PathBuf>,

    /// CSL citation style file (default: Chicago Author-Date)
    #[arg(long, value_name = "FILE")]
    csl: Option<PathBuf>,

    /// Custom template file (implies --standalone)
    #[arg(long, value_name = "FILE")]
    template: Option<PathBuf>,

    /// Print the default template for the given format and exit
    #[arg(long, value_name = "FORMAT")]
    print_default_template: Option<String>,

    /// List available syntax highlighting themes and exit
    #[arg(long)]
    list_highlight_themes: bool,

    /// List available syntax highlighting languages and exit
    #[arg(long)]
    list_highlight_languages: bool,
}

fn build_registry(id_prefix: Option<&str>) -> Registry {
    let mut reg = Registry::new();
    let md_reader = match id_prefix {
        Some(p) => MarkdownReader::new().with_id_prefix(p.to_string()),
        None => MarkdownReader::new(),
    };
    reg.add_reader(Box::new(md_reader));
    reg.add_reader(Box::new(LatexReader::new()));
    reg.add_reader(Box::new(MystReader::new()));
    reg.add_reader(Box::new(TypstReader::new()));
    reg.add_reader(Box::new(HtmlReader::new()));
    reg.add_binary_reader(Box::new(DocxReader::new()));
    reg.add_writer(Box::new(HtmlWriter::new()));
    reg.add_writer(Box::new(LatexWriter::new()));
    reg.add_writer(Box::new(MarkdownWriter::new()));
    reg.add_writer(Box::new(PlaintextWriter::new()));
    reg.add_writer(Box::new(TypstWriter::new()));
    reg.add_writer(Box::new(DocxWriter::new()));
    reg
}

fn main() {
    let cli = Cli::parse();
    let registry = build_registry(cli.id_prefix.as_deref());

    // --list-input-formats / --list-output-formats
    if cli.list_input_formats {
        for fmt in registry.reader_formats() {
            println!("{fmt}");
        }
        // JSON is always available as output
        return;
    }
    if cli.list_output_formats {
        for fmt in registry.writer_formats() {
            println!("{fmt}");
        }
        println!("json");
        return;
    }
    if cli.list_highlight_themes {
        for theme in docmux_highlight::available_themes() {
            println!("{theme}");
        }
        return;
    }
    if cli.list_highlight_languages {
        for lang in docmux_highlight::available_languages() {
            println!("{lang}");
        }
        return;
    }

    if let Some(format) = &cli.print_default_template {
        match docmux_template::default_template_for(format) {
            Some(tmpl) => {
                print!("{tmpl}");
                return;
            }
            None => {
                eprintln!(
                    "docmux: no default template for format '{format}'. Available: html, latex, markdown, plain"
                );
                std::process::exit(1);
            }
        }
    }

    // Determine input format from first non-stdin file
    let from = cli.from.as_deref().or_else(|| {
        cli.input
            .iter()
            .find(|p| p.to_str() != Some("-"))
            .and_then(|p| p.extension())
            .and_then(|e| e.to_str())
    });
    let from = from.unwrap_or("md");

    // Determine output format
    let to = cli
        .to
        .as_deref()
        .or_else(|| {
            cli.output
                .as_ref()
                .and_then(|p| p.extension())
                .and_then(|e| e.to_str())
        })
        .unwrap_or("html");

    // Parse — binary formats (e.g. DOCX) are read as bytes; text formats as String.
    let mut doc = if let Some(binary_reader) = registry.find_binary_reader(from) {
        // Read the first input file as bytes (binary formats don't support stdin or multi-file)
        let path = cli.input.first().unwrap_or_else(|| {
            eprintln!("docmux: binary input requires a file path (not stdin)");
            std::process::exit(1);
        });
        if path.to_str() == Some("-") {
            eprintln!("docmux: binary input format '{from}' cannot be read from stdin");
            std::process::exit(1);
        }
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("docmux: error reading {}: {e}", path.display());
                std::process::exit(1);
            }
        };
        match binary_reader.read_bytes(&bytes) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("docmux: parse error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        // Text path — read and concatenate all inputs
        let reader = match registry.find_reader(from) {
            Some(r) => r,
            None => {
                eprintln!(
                    "docmux: unsupported input format '{from}'. Available: {:?}",
                    registry.reader_formats()
                );
                std::process::exit(1);
            }
        };

        let mut combined_input = String::new();
        for (i, path) in cli.input.iter().enumerate() {
            if i > 0 {
                combined_input.push('\n');
            }
            if path.to_str() == Some("-") {
                if let Err(e) = std::io::stdin().read_to_string(&mut combined_input) {
                    eprintln!("docmux: error reading stdin: {e}");
                    std::process::exit(1);
                }
            } else {
                match std::fs::read_to_string(path) {
                    Ok(s) => combined_input.push_str(&s),
                    Err(e) => {
                        eprintln!("docmux: error reading {}: {e}", path.display());
                        std::process::exit(1);
                    }
                }
            }
        }

        match reader.read(&combined_input) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("docmux: parse error: {e}");
                std::process::exit(1);
            }
        }
    };

    // Apply -M metadata overrides
    apply_metadata_overrides(&mut doc.metadata, &cli.metadata);

    // Apply --shift-heading-level-by
    if let Some(shift) = cli.shift_heading_level_by {
        shift_headings(&mut doc.content, shift);
    }

    // Apply --number-sections (before --toc so the ToC sees numbered headings)
    if cli.number_sections {
        let mut ctx = TransformContext::default();
        ctx.variables.insert(
            "top-level-division".to_string(),
            cli.top_level_division.clone(),
        );
        if let Err(e) = NumberSectionsTransform::new().transform(&mut doc, &ctx) {
            eprintln!("docmux: number-sections error: {e}");
            std::process::exit(1);
        }
    }

    // Apply --section-divs (after --number-sections, before --toc)
    if cli.section_divs {
        let ctx = TransformContext::default();
        if let Err(e) = SectionDivsTransform::new().transform(&mut doc, &ctx) {
            eprintln!("docmux: section-divs error: {e}");
            std::process::exit(1);
        }
    }

    // Apply --toc
    if cli.toc {
        let mut ctx = TransformContext::default();
        ctx.variables
            .insert("toc-depth".to_string(), cli.toc_depth.to_string());
        if let Err(e) = TocTransform::new().transform(&mut doc, &ctx) {
            eprintln!("docmux: toc error: {e}");
            std::process::exit(1);
        }
    }

    // Apply cite transform (when --bibliography is provided or metadata has bibliography)
    let bib_paths: Vec<PathBuf> = if !cli.bibliography.is_empty() {
        cli.bibliography.clone()
    } else if let Some(docmux_ast::MetaValue::String(bib_path)) =
        doc.metadata.custom.get("bibliography")
    {
        vec![PathBuf::from(bib_path)]
    } else if let Some(docmux_ast::MetaValue::List(bib_list)) =
        doc.metadata.custom.get("bibliography")
    {
        bib_list
            .iter()
            .filter_map(|v| {
                if let docmux_ast::MetaValue::String(s) = v {
                    Some(PathBuf::from(s))
                } else {
                    None
                }
            })
            .collect()
    } else {
        vec![]
    };

    if !bib_paths.is_empty() {
        // Load all bibliography files into one library
        let mut combined = hayagriva::Library::new();
        for path in &bib_paths {
            let content = match std::fs::read_to_string(path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "docmux: cannot read bibliography file {}: {e}",
                        path.display()
                    );
                    std::process::exit(1);
                }
            };

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let lib = match ext {
                "bib" => match hayagriva::io::from_biblatex_str(&content) {
                    Ok(lib) => lib,
                    Err(e) => {
                        eprintln!("docmux: BibTeX parse error in {}: {e:?}", path.display());
                        std::process::exit(1);
                    }
                },
                "yml" | "yaml" => match hayagriva::io::from_yaml_str(&content) {
                    Ok(lib) => lib,
                    Err(e) => {
                        eprintln!(
                            "docmux: YAML bibliography parse error in {}: {e}",
                            path.display()
                        );
                        std::process::exit(1);
                    }
                },
                other => {
                    eprintln!(
                        "docmux: unsupported bibliography format '.{other}' \
                         (expected .bib, .yml, or .yaml)"
                    );
                    std::process::exit(1);
                }
            };

            for entry in lib.iter() {
                combined.push(entry);
            }
        }

        // Resolve CSL style path: CLI flag > metadata > default (None = built-in)
        let csl_file = cli.csl.clone().or_else(|| {
            doc.metadata.custom.get("csl").and_then(|v| match v {
                docmux_ast::MetaValue::String(s) => Some(PathBuf::from(s)),
                _ => None,
            })
        });

        let csl_xml = match &csl_file {
            Some(path) => match std::fs::read_to_string(path) {
                Ok(s) => Some(s),
                Err(e) => {
                    eprintln!("docmux: cannot read CSL file {}: {e}", path.display());
                    std::process::exit(1);
                }
            },
            None => None, // will use built-in chicago-author-date
        };

        let cite_transform = match CiteTransform::with_library(combined, csl_xml.as_deref()) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("docmux: cite transform init error: {e}");
                std::process::exit(1);
            }
        };
        if let Err(e) = cite_transform.transform(&mut doc, &TransformContext::default()) {
            eprintln!("docmux: cite transform error: {e}");
            std::process::exit(1);
        }
    }

    // Apply math transform when needed
    let source_notation = match from {
        "typst" => MathNotation::Typst,
        _ => MathNotation::LaTeX,
    };
    let target_format = match (to, &cli.math) {
        (_, Some(ref m)) if m == "mathml" => MathTarget::MathML,
        ("typst", _) => MathTarget::Typst,
        _ => MathTarget::None,
    };
    if target_format != MathTarget::None {
        let math_transform = MathTransform {
            target_format,
            source_notation,
        };
        if let Err(e) = math_transform.transform(&mut doc, &TransformContext::default()) {
            eprintln!("docmux: math transform error: {e}");
            std::process::exit(1);
        }
    }

    // Show warnings in verbose mode
    if cli.verbose && !doc.warnings.is_empty() {
        for w in &doc.warnings {
            eprintln!("docmux: warning at line {}: {}", w.line, w.message);
        }
    }

    // JSON AST dump — special case, no writer needed
    if to == "json" {
        let output = match serde_json::to_string_pretty(&doc) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("docmux: JSON serialization error: {e}");
                std::process::exit(1);
            }
        };
        write_output(&cli, &output);
        return;
    }

    // Look up writer
    let writer = match registry.find_writer(to) {
        Some(w) => w,
        None => {
            eprintln!(
                "docmux: unsupported output format '{to}'. Available: {:?} + json",
                registry.writer_formats()
            );
            std::process::exit(1);
        }
    };

    // Build write options
    let math_engine = match cli.math.as_deref() {
        Some("katex") | None => MathEngine::KaTeX,
        Some("mathjax") => MathEngine::MathJax,
        Some("mathml") => MathEngine::MathML,
        Some("raw") => MathEngine::Raw,
        Some(other) => {
            eprintln!("docmux: unknown math engine '{other}'");
            std::process::exit(1);
        }
    };

    let variables: HashMap<String, String> = cli
        .variable
        .iter()
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            Some((k.to_string(), v.to_string()))
        })
        .chain(cli.css.iter().enumerate().map(|(i, url)| {
            // Pass CSS URLs as variables so templates/writers can use them
            (
                format!("css{}", if i == 0 { String::new() } else { i.to_string() }),
                url.clone(),
            )
        }))
        .collect();

    let wrap = match cli.wrap.as_str() {
        "auto" => WrapMode::Auto,
        "preserve" => WrapMode::Preserve,
        _ => WrapMode::None,
    };

    let eol = match cli.eol.as_str() {
        "crlf" => Eol::Crlf,
        "native" => Eol::Native,
        _ => Eol::Lf,
    };

    let standalone = cli.standalone || cli.template.is_some();
    let template = cli.template.as_ref().map(|p| p.display().to_string());

    let opts = WriteOptions {
        standalone,
        template,
        math_engine,
        variables,
        wrap,
        columns: cli.columns,
        eol,
        highlight_style: cli.highlight_style.clone(),
        ..Default::default()
    };

    // Binary formats (e.g. DOCX) use write_bytes and require -o FILE
    if writer.default_extension() == "docx" {
        let bytes = match writer.write_bytes(&doc, &opts) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("docmux: write error: {e}");
                std::process::exit(1);
            }
        };
        match &cli.output {
            Some(path) if path.to_str() != Some("-") => {
                if let Err(e) = std::fs::write(path, &bytes) {
                    eprintln!("docmux: error writing {}: {e}", path.display());
                    std::process::exit(1);
                }
                if !cli.quiet {
                    let first_input = cli
                        .input
                        .first()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "-".into());
                    eprintln!(
                        "docmux: {} -> {} ({} bytes)",
                        first_input,
                        path.display(),
                        bytes.len()
                    );
                }
            }
            _ => {
                eprintln!(
                    "docmux: DOCX output requires -o FILE (binary format cannot be written to stdout)"
                );
                std::process::exit(1);
            }
        }
        return;
    }

    let output = match writer.write(&doc, &opts) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("docmux: write error: {e}");
            std::process::exit(1);
        }
    };

    let output = postprocess(&output, wrap, cli.columns, eol);
    write_output(&cli, &output);
}

/// Write output to file or stdout.
fn write_output(cli: &Cli, output: &str) {
    match &cli.output {
        Some(path) if path.to_str() != Some("-") => {
            if let Err(e) = std::fs::write(path, output) {
                eprintln!("docmux: error writing {}: {e}", path.display());
                std::process::exit(1);
            }
            if !cli.quiet {
                let first_input = cli
                    .input
                    .first()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".into());
                eprintln!(
                    "docmux: {} -> {} ({} bytes)",
                    first_input,
                    path.display(),
                    output.len()
                );
            }
        }
        _ => {
            print!("{output}");
        }
    }
}

/// Apply `--wrap`, `--columns`, and `--eol` post-processing to writer output.
fn postprocess(text: &str, wrap: WrapMode, columns: usize, eol: Eol) -> String {
    let wrapped = match wrap {
        WrapMode::Auto => wrap_text(text, columns),
        WrapMode::None | WrapMode::Preserve => text.to_string(),
    };

    match eol {
        Eol::Lf => wrapped,
        Eol::Crlf => wrapped.replace('\n', "\r\n"),
        Eol::Native => {
            if cfg!(windows) {
                wrapped.replace('\n', "\r\n")
            } else {
                wrapped
            }
        }
    }
}

/// Word-wrap paragraphs at `columns` width.
///
/// Blank-line-separated blocks are wrapped independently. Lines that look
/// like code, headings, list markers, tables, or HTML tags are left
/// untouched. Fenced code blocks (``` or ~~~) are passed through verbatim.
fn wrap_text(text: &str, columns: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut para = String::new();
    let mut in_fence = false;

    for line in text.lines() {
        // Track fenced code blocks
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            // Flush any pending paragraph
            if !para.is_empty() {
                wrap_paragraph(&para, columns, &mut out);
                para.clear();
            }
            in_fence = !in_fence;
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if in_fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if line.trim().is_empty() {
            // Flush accumulated paragraph
            if !para.is_empty() {
                wrap_paragraph(&para, columns, &mut out);
                para.clear();
            }
            out.push('\n');
        } else if is_verbatim_line(line) {
            // Flush any pending paragraph first
            if !para.is_empty() {
                wrap_paragraph(&para, columns, &mut out);
                para.clear();
            }
            out.push_str(line);
            out.push('\n');
        } else {
            // Accumulate paragraph words
            if !para.is_empty() {
                para.push(' ');
            }
            para.push_str(line.trim());
        }
    }

    // Flush final paragraph
    if !para.is_empty() {
        wrap_paragraph(&para, columns, &mut out);
    }

    // Preserve whether original ended with newline
    if text.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    }

    out
}

/// Returns true for lines that should not be reflowed.
fn is_verbatim_line(line: &str) -> bool {
    // Indented code (4+ spaces or tab)
    if line.starts_with("    ") || line.starts_with('\t') {
        return true;
    }
    let trimmed = line.trim_start();
    // Headings, blockquotes, HTML tags, list markers, table pipes, div fences
    trimmed.starts_with('#')
        || trimmed.starts_with('>')
        || trimmed.starts_with('|')
        || trimmed.starts_with("- ")
        || trimmed.starts_with("* ")
        || trimmed.starts_with("+ ")
        || trimmed.starts_with('<')
        || trimmed.starts_with(":::")
        || trimmed.bytes().take_while(|b| b.is_ascii_digit()).count() > 0
            && trimmed.chars().find(|c| !c.is_ascii_digit()) == Some('.')
}

/// Wrap a single paragraph's text at `columns` and append to `out`.
fn wrap_paragraph(para: &str, columns: usize, out: &mut String) {
    let mut col = 0;
    for (i, word) in para.split_whitespace().enumerate() {
        let wlen = word.len();
        if i > 0 && col + 1 + wlen > columns {
            out.push('\n');
            col = 0;
        } else if i > 0 {
            out.push(' ');
            col += 1;
        }
        out.push_str(word);
        col += wlen;
    }
    out.push('\n');
}

/// Apply `-M KEY=VAL` overrides to document metadata.
fn apply_metadata_overrides(metadata: &mut Metadata, overrides: &[String]) {
    for kv in overrides {
        let Some((key, val)) = kv.split_once('=') else {
            continue;
        };
        match key {
            "title" => metadata.title = Some(val.to_string()),
            "date" => metadata.date = Some(val.to_string()),
            "abstract" | "abstract_text" => {
                metadata.abstract_text = Some(vec![docmux_ast::Block::text(val)]);
            }
            _ => {
                metadata.custom.insert(
                    key.to_string(),
                    docmux_ast::MetaValue::String(val.to_string()),
                );
            }
        }
    }
}

/// Shift all heading levels by `shift` (positive = demote, negative = promote).
fn shift_headings(blocks: &mut [Block], shift: i8) {
    for block in blocks.iter_mut() {
        match block {
            Block::Heading { level, .. } => {
                let new_level = (*level as i8 + shift).clamp(1, 6) as u8;
                *level = new_level;
            }
            Block::BlockQuote { content } => shift_headings(content, shift),
            Block::List { items, .. } => {
                for item in items {
                    shift_headings(&mut item.content, shift);
                }
            }
            Block::Admonition { content, .. } => shift_headings(content, shift),
            Block::Div { content, .. } => shift_headings(content, shift),
            Block::FootnoteDef { content, .. } => shift_headings(content, shift),
            _ => {}
        }
    }
}
