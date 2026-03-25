//! # docmux CLI
//!
//! Command-line interface for the docmux universal document converter.

use clap::Parser;
use docmux_ast::{Block, Metadata};
use docmux_core::{MathEngine, Registry, WriteOptions};
use docmux_reader_latex::LatexReader;
use docmux_reader_markdown::MarkdownReader;
use docmux_reader_myst::MystReader;
use docmux_reader_typst::TypstReader;
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
    #[arg(required_unless_present_any = ["list_input_formats", "list_output_formats"])]
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
}

fn build_registry() -> Registry {
    let mut reg = Registry::new();
    reg.add_reader(Box::new(MarkdownReader::new()));
    reg.add_reader(Box::new(LatexReader::new()));
    reg.add_reader(Box::new(MystReader::new()));
    reg.add_reader(Box::new(TypstReader::new()));
    reg.add_writer(Box::new(HtmlWriter::new()));
    reg.add_writer(Box::new(LatexWriter::new()));
    reg.add_writer(Box::new(MarkdownWriter::new()));
    reg.add_writer(Box::new(PlaintextWriter::new()));
    reg.add_writer(Box::new(TypstWriter::new()));
    reg
}

fn main() {
    let cli = Cli::parse();
    let registry = build_registry();

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

    // Read and concatenate all inputs
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

    // Look up reader
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

    // Parse
    let mut doc = match reader.read(&combined_input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("docmux: parse error: {e}");
            std::process::exit(1);
        }
    };

    // Apply -M metadata overrides
    apply_metadata_overrides(&mut doc.metadata, &cli.metadata);

    // Apply --shift-heading-level-by
    if let Some(shift) = cli.shift_heading_level_by {
        shift_headings(&mut doc.content, shift);
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
        Some("raw") | Some("mathml") => MathEngine::Raw,
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

    let opts = WriteOptions {
        standalone: cli.standalone,
        math_engine,
        variables,
        ..Default::default()
    };

    let output = match writer.write(&doc, &opts) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("docmux: write error: {e}");
            std::process::exit(1);
        }
    };

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
