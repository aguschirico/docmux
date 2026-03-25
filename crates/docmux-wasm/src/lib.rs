//! # docmux-wasm
//!
//! WebAssembly bindings for docmux, exposing the conversion pipeline
//! to JavaScript/TypeScript via wasm-bindgen.

use docmux_core::{Registry, Transform, WriteOptions};
use docmux_reader_latex::LatexReader;
use docmux_reader_markdown::MarkdownReader;
use docmux_reader_myst::MystReader;
use docmux_reader_typst::TypstReader;
use docmux_transform_crossref::CrossRefTransform;
use docmux_writer_html::HtmlWriter;
use docmux_writer_latex::LatexWriter;
use docmux_writer_markdown::MarkdownWriter;
use docmux_writer_plaintext::PlaintextWriter;
use docmux_writer_typst::TypstWriter;
use wasm_bindgen::prelude::*;

fn build_registry() -> Registry {
    let mut reg = Registry::new();

    reg.add_reader(Box::new(MarkdownReader::new()));
    reg.add_reader(Box::new(LatexReader::new()));
    reg.add_reader(Box::new(TypstReader::new()));
    reg.add_reader(Box::new(MystReader::new()));

    reg.add_writer(Box::new(HtmlWriter::new()));
    reg.add_writer(Box::new(LatexWriter::new()));
    reg.add_writer(Box::new(TypstWriter::new()));
    reg.add_writer(Box::new(MarkdownWriter::new()));
    reg.add_writer(Box::new(PlaintextWriter::new()));

    reg
}

/// Convert a document from one format to another (fragment mode).
///
/// # Arguments
/// - `input` — the source document as a string
/// - `from` — input format name or extension (e.g. `"markdown"`, `"md"`)
/// - `to` — output format name or extension (e.g. `"html"`)
#[wasm_bindgen]
pub fn convert(input: &str, from: &str, to: &str) -> Result<String, JsError> {
    convert_inner(input, from, to, false)
}

/// Convert a document producing a standalone file (full HTML document, LaTeX with preamble, etc.).
#[wasm_bindgen(js_name = "convertStandalone")]
pub fn convert_standalone(input: &str, from: &str, to: &str) -> Result<String, JsError> {
    convert_inner(input, from, to, true)
}

fn convert_inner(
    input: &str,
    from: &str,
    to: &str,
    standalone: bool,
) -> Result<String, JsError> {
    let reg = build_registry();

    let reader = reg
        .find_reader(from)
        .ok_or_else(|| JsError::new(&format!("unsupported input format: {from}")))?;

    let writer = reg
        .find_writer(to)
        .ok_or_else(|| JsError::new(&format!("unsupported output format: {to}")))?;

    let mut doc = reader
        .read(input)
        .map_err(|e| JsError::new(&e.to_string()))?;

    // Run cross-reference transform (auto-numbers figures, tables, equations).
    let ctx = docmux_core::TransformContext::default();
    let crossref = CrossRefTransform::new();
    let _ = crossref.transform(&mut doc, &ctx);

    let opts = WriteOptions {
        standalone,
        ..WriteOptions::default()
    };
    writer
        .write(&doc, &opts)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Parse a document and return the AST as pretty-printed JSON.
#[wasm_bindgen(js_name = "parseToJson")]
pub fn parse_to_json(input: &str, from: &str) -> Result<String, JsError> {
    let reg = build_registry();

    let reader = reg
        .find_reader(from)
        .ok_or_else(|| JsError::new(&format!("unsupported input format: {from}")))?;

    let doc = reader
        .read(input)
        .map_err(|e| JsError::new(&e.to_string()))?;

    serde_json::to_string_pretty(&doc).map_err(|e| JsError::new(&e.to_string()))
}

/// Convert markdown to HTML (convenience wrapper).
#[wasm_bindgen(js_name = "markdownToHtml")]
pub fn markdown_to_html(input: &str) -> Result<String, JsError> {
    convert(input, "markdown", "html")
}

/// Return a list of supported input format names.
#[wasm_bindgen(js_name = "inputFormats")]
pub fn input_formats() -> Vec<String> {
    let reg = build_registry();
    reg.reader_formats().into_iter().map(String::from).collect()
}

/// Return a list of supported output format names.
#[wasm_bindgen(js_name = "outputFormats")]
pub fn output_formats() -> Vec<String> {
    let reg = build_registry();
    reg.writer_formats().into_iter().map(String::from).collect()
}
