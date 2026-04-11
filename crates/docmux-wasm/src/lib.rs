//! # docmux-wasm
//!
//! WebAssembly bindings for docmux, exposing the conversion pipeline
//! to JavaScript/TypeScript via wasm-bindgen.

use docmux_ast::ResourceData;
use docmux_core::{Registry, Transform, WriteOptions};
use docmux_reader_docx::DocxReader;
use docmux_reader_html::HtmlReader;
use docmux_reader_latex::LatexReader;
use docmux_reader_markdown::MarkdownReader;
use docmux_reader_myst::MystReader;
use docmux_reader_typst::TypstReader;
use docmux_transform_crossref::CrossRefTransform;
use docmux_transform_number_sections::NumberSectionsTransform;
use docmux_transform_toc::TocTransform;
use docmux_writer_docx::DocxWriter;
use docmux_writer_html::HtmlWriter;
use docmux_writer_latex::LatexWriter;
use docmux_writer_markdown::MarkdownWriter;
use docmux_writer_myst::MystWriter;
use docmux_writer_plaintext::PlaintextWriter;
use docmux_writer_typst::TypstWriter;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

fn build_registry() -> Registry {
    let mut reg = Registry::new();

    reg.add_reader(Box::new(MarkdownReader::new()));
    reg.add_reader(Box::new(LatexReader::new()));
    reg.add_reader(Box::new(TypstReader::new()));
    reg.add_reader(Box::new(MystReader::new()));
    reg.add_reader(Box::new(HtmlReader::new()));
    reg.add_binary_reader(Box::new(DocxReader::new()));

    reg.add_writer(Box::new(HtmlWriter::new()));
    reg.add_writer(Box::new(LatexWriter::new()));
    reg.add_writer(Box::new(TypstWriter::new()));
    reg.add_writer(Box::new(MarkdownWriter::new()));
    reg.add_writer(Box::new(MystWriter::new()));
    reg.add_writer(Box::new(PlaintextWriter::new()));
    reg.add_writer(Box::new(DocxWriter::new()));

    reg
}

/// Convert a JS `Map<string, Uint8Array>` into the AST resources format.
fn js_map_to_resources(map: &js_sys::Map) -> HashMap<String, ResourceData> {
    let mut resources = HashMap::new();
    map.for_each(&mut |value, key| {
        if let Some(name) = key.as_string() {
            let arr = js_sys::Uint8Array::from(value);
            let data = arr.to_vec();
            let mime = if data.len() >= 4 && data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
                "image/png"
            } else if data.len() >= 3 && data[0..3] == [0xFF, 0xD8, 0xFF] {
                "image/jpeg"
            } else {
                "application/octet-stream"
            };
            resources.insert(
                name,
                ResourceData {
                    mime_type: mime.to_string(),
                    data,
                },
            );
        }
    });
    resources
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

/// Convert text input to string output, with image resources for embedding.
#[wasm_bindgen(js_name = "convertWithResources")]
pub fn convert_with_resources(
    input: &str,
    from: &str,
    to: &str,
    resources: &js_sys::Map,
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
    doc.resources = js_map_to_resources(resources);
    let ctx = docmux_core::TransformContext::default();
    let _ = NumberSectionsTransform::new().transform(&mut doc, &ctx);
    let _ = CrossRefTransform::new().transform(&mut doc, &ctx);
    let _ = TocTransform::new().transform(&mut doc, &ctx);
    let opts = WriteOptions {
        standalone: false,
        highlight_style: if to == "html" {
            Some("InspiredGitHub".into())
        } else {
            None
        },
        ..Default::default()
    };
    writer
        .write(&doc, &opts)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Convert text input to binary output (e.g. markdown → DOCX), with image resources.
#[wasm_bindgen(js_name = "convertToBytes")]
pub fn convert_to_bytes(
    input: &str,
    from: &str,
    to: &str,
    resources: &js_sys::Map,
) -> Result<js_sys::Uint8Array, JsError> {
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
    doc.resources = js_map_to_resources(resources);
    let ctx = docmux_core::TransformContext::default();
    let _ = NumberSectionsTransform::new().transform(&mut doc, &ctx);
    let _ = CrossRefTransform::new().transform(&mut doc, &ctx);
    let _ = TocTransform::new().transform(&mut doc, &ctx);
    let opts = WriteOptions::default();
    let bytes = writer
        .write_bytes(&doc, &opts)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(js_sys::Uint8Array::from(&bytes[..]))
}

/// Convert binary input to binary output (e.g. DOCX → DOCX), with additional resources.
#[wasm_bindgen(js_name = "convertBytesToBytes")]
pub fn convert_bytes_to_bytes(
    input: &[u8],
    from: &str,
    to: &str,
    resources: &js_sys::Map,
) -> Result<js_sys::Uint8Array, JsError> {
    let reg = build_registry();
    let binary_reader = reg
        .find_binary_reader(from)
        .ok_or_else(|| JsError::new(&format!("unsupported binary input format: {from}")))?;
    let writer = reg
        .find_writer(to)
        .ok_or_else(|| JsError::new(&format!("unsupported output format: {to}")))?;
    let mut doc = binary_reader
        .read_bytes(input)
        .map_err(|e| JsError::new(&e.to_string()))?;
    // Merge additional resources (don't overwrite existing from reader)
    for (k, v) in js_map_to_resources(resources) {
        doc.resources.entry(k).or_insert(v);
    }
    let ctx = docmux_core::TransformContext::default();
    let _ = NumberSectionsTransform::new().transform(&mut doc, &ctx);
    let _ = CrossRefTransform::new().transform(&mut doc, &ctx);
    let _ = TocTransform::new().transform(&mut doc, &ctx);
    let opts = WriteOptions::default();
    let bytes = writer
        .write_bytes(&doc, &opts)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(js_sys::Uint8Array::from(&bytes[..]))
}

fn convert_inner(input: &str, from: &str, to: &str, standalone: bool) -> Result<String, JsError> {
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

    // Run transforms in order: number sections → cross-refs → table of contents.
    let ctx = docmux_core::TransformContext::default();
    let _ = NumberSectionsTransform::new().transform(&mut doc, &ctx);
    let _ = CrossRefTransform::new().transform(&mut doc, &ctx);
    let _ = TocTransform::new().transform(&mut doc, &ctx);

    let opts = WriteOptions {
        standalone,
        highlight_style: if to == "html" {
            Some("InspiredGitHub".into())
        } else {
            None
        },
        ..Default::default()
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

/// Convert binary input (e.g. DOCX bytes) to another format (fragment mode).
///
/// # Arguments
/// - `input` — the raw binary content (e.g. DOCX bytes from `FileReader`)
/// - `from` — input format name or extension (e.g. `"docx"`)
/// - `to`   — output format name or extension (e.g. `"html"`)
#[wasm_bindgen(js_name = "convertBytes")]
pub fn convert_bytes(input: &[u8], from: &str, to: &str) -> Result<String, JsError> {
    convert_bytes_inner(input, from, to, false)
}

/// Convert binary input producing a standalone file (full HTML, LaTeX with preamble, etc.).
#[wasm_bindgen(js_name = "convertBytesStandalone")]
pub fn convert_bytes_standalone(input: &[u8], from: &str, to: &str) -> Result<String, JsError> {
    convert_bytes_inner(input, from, to, true)
}

fn convert_bytes_inner(
    input: &[u8],
    from: &str,
    to: &str,
    standalone: bool,
) -> Result<String, JsError> {
    let reg = build_registry();
    let binary_reader = reg
        .find_binary_reader(from)
        .ok_or_else(|| JsError::new(&format!("unsupported binary input format: {from}")))?;
    let writer = reg
        .find_writer(to)
        .ok_or_else(|| JsError::new(&format!("unsupported output format: {to}")))?;
    let mut doc = binary_reader
        .read_bytes(input)
        .map_err(|e| JsError::new(&e.to_string()))?;
    let ctx = docmux_core::TransformContext::default();
    let _ = NumberSectionsTransform::new().transform(&mut doc, &ctx);
    let _ = CrossRefTransform::new().transform(&mut doc, &ctx);
    let _ = TocTransform::new().transform(&mut doc, &ctx);
    let opts = WriteOptions {
        standalone,
        highlight_style: if to == "html" {
            Some("InspiredGitHub".into())
        } else {
            None
        },
        ..Default::default()
    };
    writer
        .write(&doc, &opts)
        .map_err(|e| JsError::new(&e.to_string()))
}

/// Parse binary input and return the AST as pretty-printed JSON.
#[wasm_bindgen(js_name = "parseBytesToJson")]
pub fn parse_bytes_to_json(input: &[u8], from: &str) -> Result<String, JsError> {
    let reg = build_registry();
    let binary_reader = reg
        .find_binary_reader(from)
        .ok_or_else(|| JsError::new(&format!("unsupported binary input format: {from}")))?;
    let doc = binary_reader
        .read_bytes(input)
        .map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string_pretty(&doc).map_err(|e| JsError::new(&e.to_string()))
}
