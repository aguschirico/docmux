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

/// Decode a map of byte vectors as UTF-8 strings. Entries that fail to decode
/// are silently dropped; the caller may surface a downstream warning.
///
/// This is the pure-Rust core that's straightforward to unit-test; the
/// `js_sys::Map` adapter below delegates here.
fn decode_text_files(bytes_map: HashMap<String, Vec<u8>>) -> HashMap<String, String> {
    bytes_map
        .into_iter()
        .filter_map(|(name, bytes)| String::from_utf8(bytes).ok().map(|s| (name, s)))
        .collect()
}

/// Convert a JS `Map<string, Uint8Array>` to a Rust `HashMap<String, String>`,
/// decoding each entry as UTF-8. Entries that fail to decode are skipped
/// (the include referencing them will then warn "file not found").
fn js_map_to_text_files(map: &js_sys::Map) -> HashMap<String, String> {
    let mut bytes_map = HashMap::new();
    map.for_each(&mut |value, key| {
        if let Some(name) = key.as_string() {
            let arr = js_sys::Uint8Array::from(value);
            bytes_map.insert(name, arr.to_vec());
        }
    });
    decode_text_files(bytes_map)
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
    let opts = WriteOptions::default();
    let bytes = writer
        .write_bytes(&doc, &opts)
        .map_err(|e| JsError::new(&e.to_string()))?;
    Ok(js_sys::Uint8Array::from(&bytes[..]))
}

/// Convert a LaTeX document with `\input{}` / `\include{}` resolved against
/// the supplied file map. Currently only meaningful for `from = "latex"`;
/// for other formats, `files` is ignored (use `convertWithResources` instead).
///
/// # Arguments
/// - `input` — main `.tex` source as a string
/// - `from` — input format name (typically `"latex"`)
/// - `to` — output format name (e.g. `"markdown"`)
/// - `files` — `Map<string, Uint8Array>` of included files (UTF-8). Keys are
///   filenames as referenced by `\input{X}` (with or without `.tex`). Entries
///   whose bytes are not valid UTF-8 are skipped; the corresponding `\input`
///   will surface as a "file not found" warning in `doc.warnings`. Only
///   meaningful for `from = "latex"` or `from = "tex"`; passing a non-empty
///   map with any other format returns an error.
/// - `resources` — `Map<string, Uint8Array>` of binary resources for the writer
///   (images embedded in HTML/DOCX/etc.). Pass an empty Map if not needed.
/// - `standalone` — produce a complete output document (HTML head, LaTeX
///   preamble, etc.)
#[wasm_bindgen(js_name = "convertWithFiles")]
pub fn convert_with_files(
    input: &str,
    from: &str,
    to: &str,
    files: &js_sys::Map,
    resources: &js_sys::Map,
    standalone: bool,
) -> Result<String, JsError> {
    let reg = build_registry();
    let is_latex = from == "latex" || from == "tex";
    if !is_latex && files.size() > 0 {
        return Err(JsError::new(
            "convertWithFiles: `files` is only supported when `from` is 'latex' or 'tex'. \
             For embedding binary resources, use `convertWithResources` instead.",
        ));
    }
    let writer = reg
        .find_writer(to)
        .ok_or_else(|| JsError::new(&format!("unsupported output format: {to}")))?;

    let mut doc = if is_latex {
        let text_files = js_map_to_text_files(files);
        docmux_reader_latex::LatexReader::new()
            .read_with_files(input, &text_files)
            .map_err(|e| JsError::new(&e.to_string()))?
    } else {
        let reader = reg
            .find_reader(from)
            .ok_or_else(|| JsError::new(&format!("unsupported input format: {from}")))?;
        reader
            .read(input)
            .map_err(|e| JsError::new(&e.to_string()))?
    };

    doc.resources = js_map_to_resources(resources);

    let ctx = docmux_core::TransformContext::default();
    let _ = NumberSectionsTransform::new().transform(&mut doc, &ctx);
    let _ = CrossRefTransform::new().transform(&mut doc, &ctx);

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

    // Run transforms: number sections → cross-refs.
    let ctx = docmux_core::TransformContext::default();
    let _ = NumberSectionsTransform::new().transform(&mut doc, &ctx);
    let _ = CrossRefTransform::new().transform(&mut doc, &ctx);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_text_files_keeps_utf8_entries() {
        let mut input = HashMap::new();
        input.insert("intro.tex".to_string(), b"hello".to_vec());
        input.insert("body.tex".to_string(), "café".as_bytes().to_vec());

        let out = decode_text_files(input);

        assert_eq!(out.get("intro.tex").map(String::as_str), Some("hello"));
        assert_eq!(out.get("body.tex").map(String::as_str), Some("café"));
    }

    #[test]
    fn decode_text_files_drops_invalid_utf8() {
        let mut input = HashMap::new();
        input.insert("good.tex".to_string(), b"ok".to_vec());
        // 0xFF is never a valid lead byte in UTF-8.
        input.insert("bad.bin".to_string(), vec![0xFF, 0xFE, 0xFD]);

        let out = decode_text_files(input);

        assert_eq!(out.get("good.tex").map(String::as_str), Some("ok"));
        assert!(
            !out.contains_key("bad.bin"),
            "non-UTF-8 entry must be skipped"
        );
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn decode_text_files_empty_input_returns_empty() {
        let out = decode_text_files(HashMap::new());
        assert!(out.is_empty());
    }
}
