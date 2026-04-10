# DOCX Image Pipeline + Playground Binary Output — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make images work in markdown/LaTeX → DOCX conversions (with real dimensions), enable DOCX output in the playground, and support image drag-and-drop in the editor.

**Architecture:** The DOCX writer gains resource-awareness: it checks `doc.resources` (in-memory) before the filesystem. The CLI pre-loads image files into resources. The WASM crate gets new functions for binary output + resource passing. The playground wires VFS images through to WASM and adds DOCX to the output dropdown.

**Tech Stack:** Rust (docmux crates), wasm-bindgen + js-sys, React/TypeScript (playground), Dexie (IndexedDB)

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/docmux-writer-docx/src/lib.rs` | Modify | Resource-aware `embed_image`, image dimension parsing, MIME detection |
| `crates/docmux-cli/src/main.rs` | Modify | Pre-load image files into `doc.resources` before writing |
| `crates/docmux-wasm/src/lib.rs` | Modify | New WASM functions: `convertWithResources`, `convertToBytes`, `convertBytesToBytes` |
| `crates/docmux-wasm/Cargo.toml` | Modify | Add `js-sys` dependency |
| `playground/src/wasm/docmux.ts` | Modify | Re-export new WASM functions |
| `playground/src/hooks/useConversion.ts` | Modify | Accept + pass resources to WASM |
| `playground/src/hooks/useDropZone.ts` | Modify | Accept images, store in VFS, insert markdown |
| `playground/src/components/OutputTabs.tsx` | Modify | Add DOCX to dropdown, binary download, pass resources |
| `playground/src/components/Editor.tsx` | Modify | Wire image drop to editor, insert markdown at cursor |
| `playground/src/hooks/useImageDrop.ts` | Create | VFS storage + dedup for dropped image files |
| `playground/src/lib/formats.ts` | Modify | Add DOCX output extension mapping |

---

### Task 1: Image dimension parsing and MIME detection

**Files:**
- Modify: `crates/docmux-writer-docx/src/lib.rs`

- [ ] **Step 1: Write failing tests for PNG dimension parsing**

Add to the `#[cfg(test)] mod tests` block (after the existing tests, around line 1808):

```rust
#[test]
fn png_dimensions_parsed() {
    // Minimal 1x1 PNG — IHDR at bytes 16..24 contains width=1, height=1
    let png: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE,
    ];
    assert_eq!(image_dimensions(png), Some((1, 1)));
}

#[test]
fn png_dimensions_large_image() {
    // 800x600 PNG header
    let mut png = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    ];
    png.extend_from_slice(&800_u32.to_be_bytes()); // width
    png.extend_from_slice(&600_u32.to_be_bytes()); // height
    assert_eq!(image_dimensions(&png), Some((800, 600)));
}

#[test]
fn jpeg_dimensions_parsed() {
    // Minimal JPEG with SOF0 marker: FF C0, length, precision, height=480, width=640
    let jpeg: &[u8] = &[
        0xFF, 0xD8, 0xFF, 0xE0, // SOI + APP0 marker
        0x00, 0x02, // APP0 length (2 = empty)
        0xFF, 0xC0, // SOF0 marker
        0x00, 0x0B, // length
        0x08,       // precision (8-bit)
        0x01, 0xE0, // height = 480
        0x02, 0x80, // width = 640
    ];
    assert_eq!(image_dimensions(jpeg), Some((640, 480)));
}

#[test]
fn unknown_format_returns_none() {
    assert_eq!(image_dimensions(&[0x00, 0x01, 0x02]), None);
}

#[test]
fn mime_from_magic_bytes_png() {
    let png = [0x89, 0x50, 0x4E, 0x47];
    assert_eq!(detect_mime(&png), Some("image/png"));
}

#[test]
fn mime_from_magic_bytes_jpeg() {
    let jpeg = [0xFF, 0xD8, 0xFF, 0xE0];
    assert_eq!(detect_mime(&jpeg), Some("image/jpeg"));
}

#[test]
fn mime_from_unknown_bytes() {
    assert_eq!(detect_mime(&[0x00, 0x01]), None);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-docx -- png_dimensions jpeg_dimensions unknown_format mime_from`
Expected: compilation errors — `image_dimensions` and `detect_mime` not defined.

- [ ] **Step 3: Implement `image_dimensions` and `detect_mime`**

Add these two functions inside `impl DocxBuilder` (before `embed_image`, around line 475):

```rust
/// Parse image dimensions (width, height) in pixels from PNG or JPEG headers.
fn image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    // PNG: magic bytes + IHDR chunk at offset 16 (width) and 20 (height)
    if data.len() >= 24 && data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        let w = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
        let h = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
        return Some((w, h));
    }
    // JPEG: scan for SOF0 (FF C0) or SOF2 (FF C2) marker
    if data.len() >= 4 && data[0..2] == [0xFF, 0xD8] {
        let mut i = 2;
        while i + 8 < data.len() {
            if data[i] == 0xFF && (data[i + 1] == 0xC0 || data[i + 1] == 0xC2) {
                let h = u16::from_be_bytes([data[i + 3], data[i + 4]]) as u32;
                let w = u16::from_be_bytes([data[i + 5], data[i + 6]]) as u32;
                return Some((w, h));
            }
            // Skip to next marker: read segment length and advance
            if data[i] == 0xFF && i + 3 < data.len() {
                let seg_len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
                i += 2 + seg_len;
            } else {
                i += 1;
            }
        }
    }
    None
}

/// Detect MIME type from magic bytes.
fn detect_mime(data: &[u8]) -> Option<&'static str> {
    if data.len() >= 4 && data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        Some("image/png")
    } else if data.len() >= 3 && data[0..3] == [0xFF, 0xD8, 0xFF] {
        Some("image/jpeg")
    } else {
        None
    }
}
```

Note: these are free functions inside the module (not methods on `DocxBuilder`), so tests can call them directly.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-docx -- png_dimensions jpeg_dimensions unknown_format mime_from`
Expected: all 7 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-docx/src/lib.rs
git commit -m "feat(docx): add image dimension parsing and MIME detection from magic bytes"
```

---

### Task 2: Resource-aware `embed_image`

**Files:**
- Modify: `crates/docmux-writer-docx/src/lib.rs`

- [ ] **Step 1: Write failing test for embedding from `doc.resources`**

Add to the test module:

```rust
#[test]
fn image_embeds_from_resources() {
    // Minimal 1x1 PNG
    let png_bytes: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE,
        0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54,
        0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00,
        0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC, 0x33,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44,
        0xAE, 0x42, 0x60, 0x82,
    ];
    let mut resources = HashMap::new();
    resources.insert(
        "photo.png".to_string(),
        docmux_ast::ResourceData {
            mime_type: "image/png".to_string(),
            data: png_bytes,
        },
    );
    let doc = Document {
        content: vec![Block::Figure {
            image: docmux_ast::Image {
                url: "photo.png".into(),
                alt: vec![Inline::Text { value: "A photo".into() }],
                title: None,
                attrs: None,
            },
            caption: None,
            label: None,
            attrs: None,
        }],
        resources,
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();

    // Verify image is embedded
    let cursor = Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut found = false;
    for i in 0..archive.len() {
        if archive.by_index(i).unwrap().name().starts_with("word/media/") {
            found = true;
        }
    }
    assert!(found, "Image from resources should be embedded in word/media/");

    let xml = extract_document_xml(&bytes);
    assert!(xml.contains("<w:drawing>") || xml.contains("<wp:inline"));
}
```

- [ ] **Step 2: Write test for dimension-based sizing with max-width cap**

```rust
#[test]
fn image_dimensions_capped_at_six_inches() {
    // 1200x600 PNG header → 12.5"x6.25" at 96 DPI → capped to 6"x3"
    let mut png = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A,
        0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    ];
    png.extend_from_slice(&1200_u32.to_be_bytes());
    png.extend_from_slice(&600_u32.to_be_bytes());
    // Pad with enough bytes to be a plausible PNG
    png.extend_from_slice(&[0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE]);
    // IDAT + IEND
    png.extend_from_slice(&[
        0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54,
        0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00,
        0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC, 0x33,
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44,
        0xAE, 0x42, 0x60, 0x82,
    ]);

    let mut resources = HashMap::new();
    resources.insert(
        "wide.png".to_string(),
        docmux_ast::ResourceData {
            mime_type: "image/png".to_string(),
            data: png,
        },
    );
    let doc = Document {
        content: vec![Block::Figure {
            image: docmux_ast::Image {
                url: "wide.png".into(),
                alt: vec![],
                title: None,
                attrs: None,
            },
            caption: None,
            label: None,
            attrs: None,
        }],
        resources,
        ..Default::default()
    };

    let w = DocxWriter::new();
    let bytes = w.write_bytes(&doc, &WriteOptions::default()).unwrap();
    let xml = extract_document_xml(&bytes);

    // Max width = 6 inches = 5486400 EMU.
    // 1200x600 scaled to 6" wide → 3" tall = 2743200 EMU.
    assert!(
        xml.contains("cx=\"5486400\""),
        "Width should be capped at 6 inches (5486400 EMU), got:\n{xml}"
    );
    assert!(
        xml.contains("cy=\"2743200\""),
        "Height should scale proportionally to 3 inches (2743200 EMU), got:\n{xml}"
    );
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-docx -- image_embeds_from_resources image_dimensions_capped`
Expected: FAIL — `embed_image` doesn't check `doc.resources`.

- [ ] **Step 4: Add `resources` field to `DocxBuilder` and populate it**

Add `resources` to the struct (line ~62):

```rust
struct DocxBuilder {
    body_xml: String,
    relationships: Vec<Relationship>,
    footnotes: Vec<(u32, String)>,
    media: Vec<(String, Vec<u8>)>,
    numbering_xml: Option<String>,
    numbering_defs: Vec<NumberingDef>,
    footnote_id_map: HashMap<String, u32>,
    next_rel_id: u32,
    next_footnote_id: u32,
    next_image_id: u32,
    next_num_id: u32,
    resources: HashMap<String, docmux_ast::ResourceData>,
}
```

Update `new()` (line ~77):

```rust
fn new() -> Self {
    Self {
        body_xml: String::new(),
        relationships: Vec::new(),
        footnotes: Vec::new(),
        media: Vec::new(),
        numbering_xml: None,
        numbering_defs: Vec::new(),
        footnote_id_map: HashMap::new(),
        next_rel_id: 1,
        next_footnote_id: 2,
        next_image_id: 1,
        next_num_id: 1,
        resources: HashMap::new(),
    }
}
```

Update `write_bytes` in the `Writer` impl (line ~1042) to pass resources:

```rust
fn write_bytes(&self, doc: &Document, _opts: &WriteOptions) -> Result<Vec<u8>> {
    let mut builder = DocxBuilder::new();
    builder.resources = doc.resources.clone();
    builder.collect_footnotes(&doc.content);
    builder.write_metadata(&doc.metadata);
    builder.write_blocks(&doc.content);
    builder.numbering_xml = builder.build_numbering_xml();
    builder.assemble_zip()
}
```

- [ ] **Step 5: Rewrite `embed_image` to check resources first, use real dimensions**

Replace the current `embed_image` (lines 476-500) with:

```rust
/// Embed an image and return (rel_id, width_emu, height_emu).
/// Resolution order: doc.resources → filesystem → None (fallback).
fn embed_image(&mut self, url: &str) -> Option<(String, u32, u32)> {
    // 1. Check doc.resources
    let data = if let Some(res) = self.resources.get(url) {
        res.data.clone()
    } else {
        // 2. Filesystem fallback
        let path = std::path::Path::new(url);
        if path.exists() {
            std::fs::read(path).ok()?
        } else {
            return None;
        }
    };

    let mime = detect_mime(&data);
    let ext = match mime {
        Some("image/png") => "png",
        Some("image/jpeg") => "jpeg",
        _ => std::path::Path::new(url).extension()?.to_str()?,
    };

    let filename = format!("image{}.{}", self.next_image_id, ext);
    self.next_image_id += 1;

    let rel_id = self.add_relationship(
        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
        &format!("media/{filename}"),
    );

    // Compute dimensions: real size capped at 6 inches wide
    let (cx, cy) = match image_dimensions(&data) {
        Some((w, h)) if w > 0 && h > 0 => {
            let emu_w = w * 914400 / 96;
            let emu_h = h * 914400 / 96;
            let max_width: u32 = 5_486_400; // 6 inches
            if emu_w > max_width {
                let scale = max_width as f64 / emu_w as f64;
                (max_width, (emu_h as f64 * scale) as u32)
            } else {
                (emu_w, emu_h)
            }
        }
        _ => (3_657_600, 2_743_200), // fallback 4"×3"
    };

    self.media.push((filename, data));
    Some((rel_id, cx, cy))
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-docx`
Expected: all tests pass, including the new `image_embeds_from_resources` and `image_dimensions_capped_at_six_inches`.

- [ ] **Step 7: Run clippy and full workspace tests**

Run: `cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`

- [ ] **Step 8: Commit**

```bash
git add crates/docmux-writer-docx/src/lib.rs
git commit -m "feat(docx): resource-aware image embedding with real dimensions"
```

---

### Task 3: CLI resource pre-loading

**Files:**
- Modify: `crates/docmux-cli/src/main.rs`

- [ ] **Step 1: Write the `preload_image_resources` function**

Add this function before `main()` (around line 183, after `build_registry`):

```rust
/// Walk the AST, resolve relative image URLs against `base_dir`, and load
/// matching files into `doc.resources` so the writer can embed them.
fn preload_image_resources(doc: &mut Document, base_dir: &Path) {
    let urls = collect_image_urls(&doc.content);
    for url in urls {
        if doc.resources.contains_key(&url) {
            continue; // already loaded (e.g. from DOCX reader)
        }
        let path = base_dir.join(&url);
        if let Ok(data) = std::fs::read(&path) {
            let mime = detect_mime_cli(&data).unwrap_or("application/octet-stream");
            doc.resources.insert(
                url,
                docmux_ast::ResourceData {
                    mime_type: mime.to_string(),
                    data,
                },
            );
        }
    }
}

/// Recursively collect image URLs from blocks.
fn collect_image_urls(blocks: &[Block]) -> Vec<String> {
    let mut urls = Vec::new();
    for block in blocks {
        match block {
            Block::Figure { image, .. } => urls.push(image.url.clone()),
            Block::Paragraph(inlines)
            | Block::Heading { content: inlines, .. } => {
                collect_inline_image_urls(inlines, &mut urls);
            }
            Block::BlockQuote(inner)
            | Block::Div { content: inner, .. } => {
                urls.extend(collect_image_urls(inner));
            }
            Block::List { items, .. } => {
                for item in items {
                    urls.extend(collect_image_urls(item));
                }
            }
            Block::Footnote { content, .. } => {
                urls.extend(collect_image_urls(content));
            }
            _ => {}
        }
    }
    urls
}

fn collect_inline_image_urls(inlines: &[docmux_ast::Inline], urls: &mut Vec<String>) {
    for inline in inlines {
        match inline {
            docmux_ast::Inline::Image(img) => urls.push(img.url.clone()),
            docmux_ast::Inline::Emph(inner)
            | docmux_ast::Inline::Strong(inner)
            | docmux_ast::Inline::Strikeout(inner)
            | docmux_ast::Inline::Underline(inner)
            | docmux_ast::Inline::Superscript(inner)
            | docmux_ast::Inline::Subscript(inner) => {
                collect_inline_image_urls(inner, urls);
            }
            docmux_ast::Inline::Link { content, .. } => {
                collect_inline_image_urls(content, urls);
            }
            docmux_ast::Inline::Span { content, .. } => {
                collect_inline_image_urls(content, urls);
            }
            _ => {}
        }
    }
}

/// Detect MIME from magic bytes (CLI-side copy to avoid cross-crate dependency).
fn detect_mime_cli(data: &[u8]) -> Option<&'static str> {
    if data.len() >= 4 && data[0..4] == [0x89, 0x50, 0x4E, 0x47] {
        Some("image/png")
    } else if data.len() >= 3 && data[0..3] == [0xFF, 0xD8, 0xFF] {
        Some("image/jpeg")
    } else {
        None
    }
}
```

- [ ] **Step 2: Call `preload_image_resources` in `main()` after parsing**

Insert after the metadata overrides (line ~323), before the heading shift:

```rust
// Apply -M metadata overrides
apply_metadata_overrides(&mut doc.metadata, &cli.metadata);

// Pre-load image files into doc.resources for embedding
if let Some(first_input) = cli.input.first() {
    if first_input.to_str() != Some("-") {
        if let Some(base_dir) = first_input.parent() {
            preload_image_resources(&mut doc, base_dir);
        }
    }
}

// Apply --shift-heading-level-by
```

- [ ] **Step 3: Run workspace tests**

Run: `cargo test --workspace`
Expected: all tests pass. The preload is a no-op when there are no images or files don't exist.

- [ ] **Step 4: Manual CLI test with a real image**

```bash
# Create a test directory
mkdir -p /tmp/docmux-img-test
# Create a tiny test PNG (1x1 pixel)
printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDAT\x08\xd7c\xf8\xcf\xc0\x00\x00\x00\x02\x00\x01\xe2!\xbc3\x00\x00\x00\x00IEND\xaeB`\x82' > /tmp/docmux-img-test/photo.png
# Create markdown file
echo '![A photo](photo.png)' > /tmp/docmux-img-test/test.md
# Convert
cargo run -p docmux-cli -- /tmp/docmux-img-test/test.md -o /tmp/docmux-img-test/output.docx
# Verify (should show word/media/ entry)
unzip -l /tmp/docmux-img-test/output.docx | grep media
```

Expected: a `word/media/image1.png` entry in the ZIP listing.

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-cli/src/main.rs
git commit -m "feat(cli): pre-load relative image files into doc.resources for embedding"
```

---

### Task 4: WASM binary output and resource functions

**Files:**
- Modify: `crates/docmux-wasm/Cargo.toml`
- Modify: `crates/docmux-wasm/src/lib.rs`

- [ ] **Step 1: Add `js-sys` dependency**

In `crates/docmux-wasm/Cargo.toml`, add after the `wasm-bindgen` line:

```toml
js-sys = "0.3"
```

- [ ] **Step 2: Add helper to convert JS Map to resources HashMap**

At the top of `lib.rs`, add `use docmux_ast::ResourceData;` to the imports. Then add this helper after `build_registry()`:

```rust
/// Convert a JS `Map<string, Uint8Array>` into the AST resources format.
fn js_map_to_resources(
    map: &js_sys::Map,
) -> HashMap<String, ResourceData> {
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
            resources.insert(name, ResourceData {
                mime_type: mime.to_string(),
                data,
            });
        }
    });
    resources
}
```

Also add at the top: `use std::collections::HashMap;`

- [ ] **Step 3: Add `convertWithResources` function**

Add after `convert_standalone` (around line 60):

```rust
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
```

- [ ] **Step 4: Add `convertToBytes` function**

Add after `convertWithResources`:

```rust
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
```

- [ ] **Step 5: Add `convertBytesToBytes` function**

Add after `convertToBytes`:

```rust
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
```

- [ ] **Step 6: Verify WASM build compiles**

Run: `cargo build --target wasm32-unknown-unknown -p docmux-wasm`
Expected: build succeeds.

- [ ] **Step 7: Run workspace tests**

Run: `cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`

- [ ] **Step 8: Commit**

```bash
git add crates/docmux-wasm/Cargo.toml crates/docmux-wasm/src/lib.rs
git commit -m "feat(wasm): add binary output and resource-aware conversion functions"
```

---

### Task 5: Playground — DOCX output format and binary download

**Files:**
- Modify: `playground/src/lib/formats.ts`
- Modify: `playground/src/wasm/docmux.ts`
- Modify: `playground/src/components/OutputTabs.tsx`

- [ ] **Step 1: Add DOCX extension mapping to `formats.ts`**

The `BINARY_FORMATS` set already has `"docx"`. No changes needed there. But we need to add the output extension. This is done in `OutputTabs.tsx`'s `FORMAT_TO_EXT` — we'll do that in step 3.

- [ ] **Step 2: Export new WASM functions in `wasm/docmux.ts`**

Replace the contents of `playground/src/wasm/docmux.ts`:

```typescript
export {
  convert,
  convertStandalone,
  convertBytes,
  convertBytesStandalone,
  convertWithResources,
  convertToBytes,
  convertBytesToBytes,
  parseToJson,
  parseBytesToJson,
  markdownToHtml,
  getInputFormats,
  getOutputFormats,
} from "@docmux/wasm";

export type {
  ConvertOutcome,
  ConversionResult,
  ConversionError,
} from "@docmux/wasm";
```

- [ ] **Step 3: Add DOCX to output format list and binary download in `OutputTabs.tsx`**

Update `OUTPUT_FORMATS` (line 20):

```typescript
const OUTPUT_FORMATS = [
  { value: "html", label: "HTML" },
  { value: "latex", label: "LaTeX" },
  { value: "typst", label: "Typst" },
  { value: "markdown", label: "Markdown" },
  { value: "plain", label: "Plain Text" },
  { value: "docx", label: "DOCX" },
] as const;
```

Update `FORMAT_TO_EXT` (line 36):

```typescript
const FORMAT_TO_EXT: Record<string, string> = {
  html: "html",
  latex: "tex",
  typst: "typ",
  markdown: "md",
  plain: "txt",
  docx: "docx",
};
```

Update `FORMAT_TO_MONACO` (line 28):

```typescript
const FORMAT_TO_MONACO: Record<string, string> = {
  html: "html",
  latex: "latex",
  typst: "plaintext",
  markdown: "markdown",
  plain: "plaintext",
  docx: "plaintext",
};
```

- [ ] **Step 4: Update Source tab and download handler for binary output format**

In `OutputTabs.tsx`, the conversion hook will gain a `binaryOutput` field in Task 7. For now, set up the UI to handle it. The download handler and Export button will be fully wired in Task 7.

For the Source tab, when output is DOCX, show a placeholder message instead of the editor:

```typescript
<TabsContent value="source" className="flex-1 overflow-auto">
  {isBinaryFormat(outputFormat) ? (
    <div className="flex h-full items-center justify-center text-sm text-zinc-500">
      Binary format — use Export to download
    </div>
  ) : (
    <ReadOnlyEditor
      value={source}
      language={FORMAT_TO_MONACO[outputFormat] ?? "plaintext"}
      emptyMessage="Select a file and output format"
    />
  )}
</TabsContent>
```

The download handler and Export button disabled state remain unchanged for now — they still use `source`. Task 7 will update them to also handle `binaryOutput`.

- [ ] **Step 5: Verify TypeScript compiles**

Run: `cd playground && pnpm exec tsc --noEmit`

Note: this may show errors for the new WASM exports that don't exist yet in the npm package. That's expected — they'll work after rebuilding WASM. For now, just verify no other TS errors are introduced. If the new exports cause errors, temporarily comment them out in `wasm/docmux.ts` and note to uncomment after WASM rebuild.

- [ ] **Step 6: Commit**

```bash
git add playground/src/wasm/docmux.ts playground/src/components/OutputTabs.tsx playground/src/lib/formats.ts
git commit -m "feat(playground): add DOCX to output formats with binary download support"
```

---

### Task 6: Playground — Image drag-and-drop on editor

**Files:**
- Modify: `playground/src/hooks/useDropZone.ts`

- [ ] **Step 1: Extend `useDropZone` to detect and handle image files**

The current hook takes a single `onFile` callback. We need to distinguish between DOCX imports and image drops. Change the signature to accept two callbacks:

Replace `playground/src/hooks/useDropZone.ts`:

```typescript
import { useState, useCallback, type DragEvent } from "react";

const IMAGE_EXTENSIONS = new Set(["png", "jpg", "jpeg", "gif", "webp"]);

function getFileExtension(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot >= 0 ? name.slice(dot + 1).toLowerCase() : "";
}

export function isImageFile(file: File): boolean {
  return IMAGE_EXTENSIONS.has(getFileExtension(file.name));
}

export function useDropZone(onFile: (file: File) => void, onImage?: (file: File) => void) {
  const [dragCount, setDragCount] = useState(0);

  const handleDragEnter = useCallback((e: DragEvent) => {
    e.preventDefault();
    setDragCount((n) => n + 1);
  }, []);

  const handleDragLeave = useCallback((e: DragEvent) => {
    e.preventDefault();
    setDragCount((n) => n - 1);
  }, []);

  const handleDragOver = useCallback((e: DragEvent) => {
    e.preventDefault();
  }, []);

  const handleDrop = useCallback(
    (e: DragEvent) => {
      e.preventDefault();
      setDragCount(0);
      const file = e.dataTransfer.files[0];
      if (!file) return;
      if (onImage && isImageFile(file)) {
        onImage(file);
      } else {
        onFile(file);
      }
    },
    [onFile, onImage],
  );

  return {
    isDragging: dragCount > 0,
    dropProps: {
      onDragEnter: handleDragEnter,
      onDragLeave: handleDragLeave,
      onDragOver: handleDragOver,
      onDrop: handleDrop,
    },
  };
}
```

- [ ] **Step 2: Create `useImageDrop` hook for VFS storage and markdown insertion**

Create `playground/src/hooks/useImageDrop.ts`:

```typescript
import { useCallback } from "react";
import { useWorkspace } from "@/contexts/workspace-context";
import { db, createBinaryFile } from "@/vfs/db";
import { toast } from "sonner";

/**
 * Returns a callback that stores a dropped image in the VFS
 * and returns the filename to insert in the editor.
 */
export function useImageDrop(): (file: File) => Promise<string | null> {
  const { activeWorkspaceId } = useWorkspace();

  return useCallback(
    async (file: File): Promise<string | null> => {
      if (!activeWorkspaceId) return null;

      const buffer = await file.arrayBuffer();
      let filename = file.name;

      // Dedup: check if filename already exists in workspace
      const existing = await db.files
        .where("[workspaceId+path]")
        .equals([activeWorkspaceId, filename])
        .first();

      if (existing) {
        const dot = filename.lastIndexOf(".");
        const base = dot >= 0 ? filename.slice(0, dot) : filename;
        const ext = dot >= 0 ? filename.slice(dot) : "";
        let n = 1;
        while (
          await db.files
            .where("[workspaceId+path]")
            .equals([activeWorkspaceId, `${base}-${n}${ext}`])
            .first()
        ) {
          n++;
        }
        filename = `${base}-${n}${ext}`;
      }

      await createBinaryFile(activeWorkspaceId, filename, buffer);
      toast.success(`Added ${filename}`);
      return filename;
    },
    [activeWorkspaceId],
  );
}
```

- [ ] **Step 3: Wire image drop to the editor component**

The drop zone is used in `playground/src/components/Editor.tsx` (line 58). The editor is a `MonacoEditor` from `@monaco-editor/react` (line 103). Wire the image drop:

1. Import the new hook and add a Monaco editor ref:

```typescript
import { useImageDrop } from "@/hooks/useImageDrop";
import type { editor as monacoEditor } from "monaco-editor";
```

2. In the `Editor` component, add a ref and the hook:

```typescript
const editorRef = useRef<monacoEditor.IStandaloneCodeEditor | null>(null);
const handleImageDrop = useImageDrop();
```

3. Create the `onImage` callback that stores the file and inserts markdown at the cursor:

```typescript
const onImage = useCallback(
  async (file: File) => {
    const filename = await handleImageDrop(file);
    if (!filename || !editorRef.current) return;
    const ed = editorRef.current;
    const pos = ed.getPosition();
    if (pos) {
      const text = `![](${filename})`;
      ed.executeEdits("image-drop", [
        { range: { startLineNumber: pos.lineNumber, startColumn: pos.column, endLineNumber: pos.lineNumber, endColumn: pos.column }, text },
      ]);
    }
  },
  [handleImageDrop],
);
```

4. Pass `onImage` as the second argument to `useDropZone`:

```typescript
const { isDragging, dropProps } = useDropZone(importDocxFile, onImage);
```

5. Wire the editor ref via `onMount`:

```typescript
<MonacoEditor
  onMount={(editor) => { editorRef.current = editor; }}
  // ... existing props
/>
```

6. Update the `DropOverlay` text from `"Drop .docx file"` to `"Drop file"` (it now accepts both .docx and images).

Add `useRef`, `useCallback` to the react import at line 1.

- [ ] **Step 4: Verify TypeScript compiles and lint passes**

Run: `cd playground && pnpm exec tsc --noEmit && pnpm run lint`

- [ ] **Step 5: Commit**

```bash
git add playground/src/hooks/useDropZone.ts playground/src/hooks/useImageDrop.ts playground/src/components/Editor.tsx
git commit -m "feat(playground): image drag-and-drop with VFS storage and markdown insertion"
```

---

### Task 7: Playground — Pass VFS resources to WASM conversion

**Files:**
- Modify: `playground/src/hooks/useConversion.ts`
- Modify: `playground/src/components/OutputTabs.tsx`

- [ ] **Step 1: Update `useConversion` to accept and use resources**

Replace `playground/src/hooks/useConversion.ts`:

```typescript
import { useEffect, useRef, useState } from "react";
import {
  convert,
  convertStandalone,
  convertBytes,
  convertBytesStandalone,
  convertWithResources,
  convertToBytes,
  convertBytesToBytes,
  parseToJson,
  parseBytesToJson,
  type ConvertOutcome,
} from "@/wasm/docmux";
import { isBinaryFormat } from "@/lib/formats";

const DEBOUNCE_MS = 200;

export interface ConversionState {
  /** HTML preview (standalone mode for full rendering) */
  preview: string | null;
  /** Source output in the selected target format (null for binary outputs) */
  source: string | null;
  /** Binary output for binary target formats */
  binaryOutput: Uint8Array | null;
  /** AST as pretty-printed JSON */
  ast: string | null;
  /** Any conversion errors */
  errors: string[];
  /** Whether a conversion is in progress */
  converting: boolean;
}

const INITIAL: ConversionState = {
  preview: null,
  source: null,
  binaryOutput: null,
  ast: null,
  errors: [],
  converting: false,
};

export function useConversion(
  content: string | null,
  inputFormat: string | null,
  outputFormat: string,
  binaryContent?: Uint8Array | null,
  resources?: Map<string, Uint8Array> | null,
): ConversionState {
  const [state, setState] = useState<ConversionState>(INITIAL);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const seqRef = useRef(0);

  const hasBinary = binaryContent != null && binaryContent.length > 0 && inputFormat !== null;
  const hasText = content !== null && inputFormat !== null;
  const hasInput = hasBinary || hasText;
  const hasResources = resources != null && resources.size > 0;
  const isBinaryOutput = isBinaryFormat(outputFormat);

  useEffect(() => {
    if (!hasInput) return;

    clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      const seq = ++seqRef.current;
      setState((prev) => ({ ...prev, converting: true }));

      // Build JS Map for WASM if we have resources
      const jsResources = new Map<string, Uint8Array>();
      if (hasResources) {
        resources!.forEach((v, k) => jsResources.set(k, v));
      }

      const runConversion = async () => {
        const errors: string[] = [];
        let preview: string | null = null;
        let source: string | null = null;
        let binaryOutput: Uint8Array | null = null;
        let ast: string | null = null;

        try {
          // Preview (always string HTML)
          if (hasBinary) {
            const r = await convertBytesStandalone(binaryContent, inputFormat!, "html");
            if (r.error) errors.push(`[preview] ${r.error}`);
            preview = r.output;
          } else if (hasResources) {
            const r = await convertWithResources(content!, inputFormat!, "html", jsResources);
            if (r.error) errors.push(`[preview] ${r.error}`);
            preview = r.output;
          } else {
            const r = await convertStandalone(content!, inputFormat!, "html");
            if (r.error) errors.push(`[preview] ${r.error}`);
            preview = r.output;
          }
        } catch (e) {
          errors.push(`[preview] ${e}`);
        }

        try {
          // Source / binary output
          if (isBinaryOutput) {
            if (hasBinary) {
              binaryOutput = await convertBytesToBytes(binaryContent, inputFormat!, outputFormat, jsResources);
            } else {
              binaryOutput = await convertToBytes(content!, inputFormat!, outputFormat, jsResources);
            }
          } else if (hasBinary) {
            const r = await convertBytes(binaryContent, inputFormat!, outputFormat);
            if (r.error) errors.push(`[source] ${r.error}`);
            source = r.output;
          } else if (hasResources) {
            const r = await convertWithResources(content!, inputFormat!, outputFormat, jsResources);
            if (r.error) errors.push(`[source] ${r.error}`);
            source = r.output;
          } else {
            const r = await convert(content!, inputFormat!, outputFormat);
            if (r.error) errors.push(`[source] ${r.error}`);
            source = r.output;
          }
        } catch (e) {
          errors.push(`[source] ${e}`);
        }

        try {
          // AST
          if (hasBinary) {
            const r = await parseBytesToJson(binaryContent, inputFormat!);
            if (r.error) errors.push(`[ast] ${r.error}`);
            ast = r.output;
          } else {
            const r = await parseToJson(content!, inputFormat!);
            if (r.error) errors.push(`[ast] ${r.error}`);
            ast = r.output;
          }
        } catch (e) {
          errors.push(`[ast] ${e}`);
        }

        if (seq !== seqRef.current) return;
        setState({ preview, source, binaryOutput, ast, errors, converting: false });
      };

      runConversion();
    }, DEBOUNCE_MS);

    return () => clearTimeout(timerRef.current);
  }, [content, binaryContent, inputFormat, outputFormat, hasInput, hasBinary, hasResources, isBinaryOutput, resources]);

  if (!hasInput) return INITIAL;
  return state;
}
```

Note: the new WASM functions `convertToBytes` and `convertBytesToBytes` return `Uint8Array` directly (not `ConvertOutcome`), so we call them differently. The `convertWithResources` function returns `ConvertOutcome` like the existing functions.

**Important:** The exact return types of the new WASM functions need to match what wasm-bindgen generates. `convertToBytes` returns `Result<Uint8Array, JsError>` which in JS becomes a `Promise<Uint8Array>` that throws on error. So we wrap these in try/catch instead of checking `.error`.

- [ ] **Step 2: Update `OutputTabs.tsx` to collect VFS resources and pass to conversion**

In `OutputTabs.tsx`, add resource collection from the VFS. Add these imports at the top:

```typescript
import { useRef, useState } from "react";
```

Add a hook to collect image resources from the workspace. Before the `useConversion` call:

```typescript
const { activeFileId, activeWorkspaceId } = useWorkspace();

// Collect image resources from VFS
const imageResources = useLiveQuery(
  async () => {
    if (!activeWorkspaceId) return null;
    const files = await db.files
      .where("workspaceId")
      .equals(activeWorkspaceId)
      .toArray();
    const map = new Map<string, Uint8Array>();
    for (const f of files) {
      if (f.binaryContent && /\.(png|jpe?g|gif|webp)$/i.test(f.path)) {
        map.set(f.path, new Uint8Array(f.binaryContent));
      }
    }
    return map.size > 0 ? map : null;
  },
  [activeWorkspaceId],
);
```

Update the `useConversion` call to pass resources:

```typescript
const { preview, source, binaryOutput, ast, errors, converting } = useConversion(
  content,
  inputFormat,
  outputFormat,
  binaryContent,
  imageResources,
);
```

Update the download handler to use `binaryOutput` from state:

```typescript
function handleDownload() {
  const isBinaryOutput = isBinaryFormat(outputFormat);
  if (isBinaryOutput && binaryOutput) {
    const ext = FORMAT_TO_EXT[outputFormat] ?? "bin";
    const baseName = file?.path
      ? file.path.replace(`.${getExtension(file.path)}`, "")
      : "output";
    const blob = new Blob([binaryOutput], {
      type: "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${baseName}.${ext}`;
    a.click();
    URL.revokeObjectURL(url);
  } else if (source) {
    const ext = FORMAT_TO_EXT[outputFormat] ?? "txt";
    const baseName = file?.path
      ? file.path.replace(`.${getExtension(file.path)}`, "")
      : "output";
    const blob = new Blob([source], { type: "text/plain;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${baseName}.${ext}`;
    a.click();
    URL.revokeObjectURL(url);
  }
}
```

Update the Export button's disabled prop:

```typescript
disabled={isBinaryFormat(outputFormat) ? !binaryOutput : !source}
```

Remove the `binaryOutputRef` added in Task 5 (it's no longer needed since `binaryOutput` comes from state now).

- [ ] **Step 3: Build WASM and update playground dependency**

Run:
```bash
cd /Users/augustochirico/Documents/src/side-projects/docmux
cargo build --target wasm32-unknown-unknown -p docmux-wasm
cd playground && pnpm run build
```

Note: the playground references `@docmux/wasm` — check how the local dev setup links to the built WASM. The implementing agent should ensure the WASM build output is linked correctly (likely via a workspace path or a local `pnpm link`).

- [ ] **Step 4: Verify TypeScript compiles and lint passes**

Run: `cd playground && pnpm exec tsc --noEmit && pnpm run lint`

- [ ] **Step 5: Commit**

```bash
git add playground/src/hooks/useConversion.ts playground/src/components/OutputTabs.tsx
git commit -m "feat(playground): wire VFS image resources through WASM conversion pipeline"
```

---

### Task 8: Integration testing and manual verification

**Files:** none (testing only)

- [ ] **Step 1: Run full Rust test suite**

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: all tests pass, no clippy warnings.

- [ ] **Step 2: Run WASM build**

```bash
cargo build --target wasm32-unknown-unknown -p docmux-wasm
```

- [ ] **Step 3: Run playground checks**

```bash
cd playground
pnpm exec tsc --noEmit
pnpm run lint
pnpm run build
```

- [ ] **Step 4: Manual CLI test — markdown with image to DOCX**

```bash
mkdir -p /tmp/docmux-test
printf '\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDAT\x08\xd7c\xf8\xcf\xc0\x00\x00\x00\x02\x00\x01\xe2!\xbc3\x00\x00\x00\x00IEND\xaeB`\x82' > /tmp/docmux-test/photo.png
cat > /tmp/docmux-test/doc.md << 'MDEOF'
# Test Document

Here is an image:

![A test photo](photo.png)

And some text after.
MDEOF
cargo run -p docmux-cli -- /tmp/docmux-test/doc.md -o /tmp/docmux-test/output.docx
unzip -l /tmp/docmux-test/output.docx | grep media
```

Expected: `word/media/image1.png` in the listing.

- [ ] **Step 5: Manual playground test**

1. Start dev server: `cd playground && pnpm run dev`
2. Open in browser
3. Write markdown with `![](test.png)` in the editor
4. Drag a PNG image onto the editor — verify it inserts `![](filename.png)` and appears in the file tree
5. Switch output format to DOCX
6. Click Export — verify a `.docx` file downloads
7. Open the `.docx` in Word/LibreOffice — verify the image is visible

- [ ] **Step 6: Final commit if any fixes were needed**

```bash
git add -u
git commit -m "fix: integration test fixes for image pipeline"
```
