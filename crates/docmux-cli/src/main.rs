//! # docmux CLI
//!
//! Command-line interface for the docmux universal document converter.

use clap::Parser;
use docmux_core::{Registry, WriteOptions};
use docmux_reader_markdown::MarkdownReader;
use docmux_writer_html::HtmlWriter;
use docmux_writer_latex::LatexWriter;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "docmux",
    about = "Universal document converter — MIT licensed, WASM-first",
    version
)]
struct Cli {
    /// Input file path
    input: PathBuf,

    /// Output file path (use `-` for stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Input format (auto-detected from extension if omitted)
    #[arg(short, long)]
    from: Option<String>,

    /// Output format (auto-detected from extension if omitted)
    #[arg(short, long)]
    to: Option<String>,

    /// Produce a standalone document (full HTML with <head>, etc.)
    #[arg(short, long)]
    standalone: bool,
}

fn build_registry() -> Registry {
    let mut reg = Registry::new();
    reg.add_reader(Box::new(MarkdownReader::new()));
    reg.add_writer(Box::new(HtmlWriter::new()));
    reg.add_writer(Box::new(LatexWriter::new()));
    reg
}

fn main() {
    let cli = Cli::parse();

    // Read input
    let input = match std::fs::read_to_string(&cli.input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("docmux: error reading {}: {}", cli.input.display(), e);
            std::process::exit(1);
        }
    };

    let registry = build_registry();

    // Determine input format
    let from = cli
        .from
        .as_deref()
        .or_else(|| cli.input.extension().and_then(|e| e.to_str()))
        .unwrap_or("md");

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

    // Look up reader/writer
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

    let writer = match registry.find_writer(to) {
        Some(w) => w,
        None => {
            eprintln!(
                "docmux: unsupported output format '{to}'. Available: {:?}",
                registry.writer_formats()
            );
            std::process::exit(1);
        }
    };

    // Convert
    let opts = WriteOptions {
        standalone: cli.standalone,
        ..Default::default()
    };

    let doc = match reader.read(&input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("docmux: parse error: {e}");
            std::process::exit(1);
        }
    };

    let output = match writer.write(&doc, &opts) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("docmux: write error: {e}");
            std::process::exit(1);
        }
    };

    // Write output
    match &cli.output {
        Some(path) if path.to_str() != Some("-") => {
            if let Err(e) = std::fs::write(path, &output) {
                eprintln!("docmux: error writing {}: {}", path.display(), e);
                std::process::exit(1);
            }
            eprintln!(
                "docmux: {} -> {} ({} bytes)",
                cli.input.display(),
                path.display(),
                output.len()
            );
        }
        _ => {
            print!("{output}");
        }
    }
}
