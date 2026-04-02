# DOCX Image Extraction — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract embedded images from DOCX files and render them as data URIs in HTML output.

**Architecture:** Resource map on `Document` (pandoc-style media bag). The DOCX reader loads all media bytes upfront, the document parser emits `Inline::Image` nodes referencing media paths, and the HTML writer converts resources to base64 data URIs at render time.

**Tech Stack:** Rust, quick-xml, base64, wasm-pack

---

### Task 1: Add ResourceData and Document.resources to AST

**Files:**
- Modify: `crates/docmux-ast/src/lib.rs:28-35` (Document struct)

- [ ] **Step 1: Add ResourceData struct and resources field**

In `crates/docmux-ast/src/lib.rs`, add the `ResourceData` struct before `Document`, and add the `resources` field to `Document`:

```rust
/// Binary resource embedded in the document (e.g. an image from a DOCX).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceData {
    pub mime_type: String,
    /// Raw bytes — skipped during serialization to keep JSON output clean.
    #[serde(skip)]
    pub data: Vec<u8>,
}

/// A complete document: metadata + content + optional bibliography.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Document {
    pub metadata: Metadata,
    pub content: Vec<Block>,
    pub bibliography: Option<Bibliography>,
    #[serde(default)]
    pub warnings: Vec<ParseWarning>,
    /// Embedded binary resources keyed by relative path (e.g. "media/image1.png").
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub resources: HashMap<String, ResourceData>,
}
```

- [ ] **Step 2: Verify workspace compiles**

Run: `cargo check --workspace`
Expected: compiles cleanly. All existing code that constructs `Document` uses `Default` or struct-init with `..Default::default()`, so the new field is automatically `HashMap::new()`.

- [ ] **Step 3: Fix any compilation errors**

If any code constructs `Document` with explicit fields without `..Default::default()`, add `resources: HashMap::new()` to those sites. Check:

Run: `cargo check --workspace 2>&1 | head -40`

The DOCX reader at `crates/docmux-reader-docx/src/lib.rs:130-135` constructs Document explicitly — update it:

```rust
Ok(Document {
    metadata: metadata_result,
    content,
    bibliography: None,
    warnings: vec![],
    resources: HashMap::new(),
})
```

Add `use std::collections::HashMap;` to the imports if not already present.

- [ ] **Step 4: Run tests**

Run: `cargo test --workspace`
Expected: all tests pass (the new field defaults to empty HashMap).

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-ast/src/lib.rs crates/docmux-reader-docx/src/lib.rs
git commit -m "feat(ast): add ResourceData struct and Document.resources field"
```

---

### Task 2: Load media resources in DOCX reader

**Files:**
- Modify: `crates/docmux-reader-docx/src/lib.rs:65-136` (read_bytes method)

- [ ] **Step 1: Write failing test — DOCX with image populates resources**

In `crates/docmux-reader-docx/src/lib.rs`, add a test inside `mod tests`:

```rust
#[test]
fn read_docx_loads_media_resources() {
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body><w:p><w:r><w:t>Has image</w:t></w:r></w:p></w:body>
</w:document>"#;

    let fake_png = b"\x89PNG\r\n\x1a\nfake image data";
    let zip_bytes = make_zip(&[
        ("word/document.xml", doc_xml.as_bytes()),
        ("word/media/image1.png", fake_png),
    ]);

    let reader = DocxReader;
    let doc = reader.read_bytes(&zip_bytes).unwrap();
    assert_eq!(doc.resources.len(), 1);

    let res = doc.resources.get("media/image1.png").expect("resource should exist");
    assert_eq!(res.mime_type, "image/png");
    assert_eq!(res.data, fake_png);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p docmux-reader-docx read_docx_loads_media_resources -- --nocapture`
Expected: FAIL — `doc.resources` is empty.

- [ ] **Step 3: Implement resource loading in read_bytes**

In `crates/docmux-reader-docx/src/lib.rs`, add a helper function and update `read_bytes`:

```rust
use docmux_ast::{Block, Document, ResourceData};
use std::collections::HashMap;
```

Add this helper function before the `impl BinaryReader`:

```rust
/// Infer MIME type from file extension.
fn mime_from_path(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "tiff" | "tif" => "image/tiff",
        "wmf" => "image/x-wmf",
        "emf" => "image/x-emf",
        _ => "application/octet-stream",
    }
}
```

In `read_bytes`, after parsing footnotes and metadata (step 5) but before parsing the document body (step 6), add resource loading:

```rust
// 5b. Load embedded media resources
let mut resources = HashMap::new();
for full_path in archive.media_paths() {
    if let Some(bytes) = archive.get_bytes(full_path) {
        // Strip "word/" prefix → "media/image1.png"
        let key = full_path.strip_prefix("word/").unwrap_or(full_path);
        resources.insert(
            key.to_string(),
            ResourceData {
                mime_type: mime_from_path(full_path).to_string(),
                data: bytes.to_vec(),
            },
        );
    }
}
```

Update the return statement:

```rust
Ok(Document {
    metadata: metadata_result,
    content,
    bibliography: None,
    warnings: vec![],
    resources,
})
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p docmux-reader-docx read_docx_loads_media_resources -- --nocapture`
Expected: PASS

- [ ] **Step 5: Run full test suite**

Run: `cargo test --workspace`
Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-reader-docx/src/lib.rs
git commit -m "feat(docx): load embedded media into Document.resources"
```

---

### Task 3: Parse `<w:drawing>` in document parser

**Files:**
- Modify: `crates/docmux-reader-docx/src/document.rs:189-369` (parse_runs function)

- [ ] **Step 1: Write failing test — inline drawing produces Inline::Image**

In `crates/docmux-reader-docx/src/document.rs`, inside `mod tests`, add:

```rust
#[test]
fn parse_drawing_inline() {
    let styles = StyleMap::new();
    let numbering = NumberingMap::new();
    let mut rels = RelMap::new();
    rels.insert(
        "rId5".to_string(),
        crate::relationships::Relationship {
            rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image".to_string(),
            target: "media/image1.png".to_string(),
            target_mode: None,
        },
    );
    let archive_bytes = crate::tests::make_zip(&[
        ("word/document.xml", b"<doc/>"),
        ("word/media/image1.png", b"\x89PNG"),
    ]);
    let archive = crate::archive::DocxArchive::from_bytes(&archive_bytes).unwrap();
    let ctx = ParseContext {
        styles: &styles,
        numbering: &numbering,
        rels: &rels,
        archive: &archive,
    };

    let xml = r#"<w:p xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                       xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
                       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                       xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
        <w:r>
            <w:drawing>
                <wp:inline>
                    <wp:docPr id="1" name="My Logo"/>
                    <a:graphic>
                        <a:graphicData>
                            <pic:pic>
                                <pic:blipFill>
                                    <a:blip r:embed="rId5"/>
                                </pic:blipFill>
                            </pic:pic>
                        </a:graphicData>
                    </a:graphic>
                </wp:inline>
            </w:drawing>
        </w:r>
    </w:p>"#;

    let block = parse_paragraph(xml, &ctx).unwrap().unwrap();
    if let Block::Paragraph { content } = &block {
        assert_eq!(content.len(), 1, "should have one inline");
        if let Inline::Image(img) = &content[0] {
            assert_eq!(img.url, "media/image1.png");
            assert_eq!(img.alt_text(), "My Logo");
        } else {
            panic!("expected Image, got {:?}", content[0]);
        }
    } else {
        panic!("expected Paragraph, got {:?}", block);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p docmux-reader-docx parse_drawing_inline -- --nocapture`
Expected: compilation error — `ParseContext` doesn't have `archive` field yet.

- [ ] **Step 3: Make make_zip accessible from document tests**

In `crates/docmux-reader-docx/src/lib.rs`, change the `make_zip` function in `mod tests` to `pub(crate)`:

```rust
pub(crate) fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
```

Also make `archive::DocxArchive` accessible from tests by ensuring the `archive` module is `pub(crate)` (it already is based on the current module declaration).

- [ ] **Step 4: Add archive to ParseContext**

In `crates/docmux-reader-docx/src/document.rs`, update `ParseContext`:

```rust
pub(crate) struct ParseContext<'a> {
    pub(crate) styles: &'a StyleMap,
    #[allow(dead_code)]
    pub(crate) numbering: &'a NumberingMap,
    pub(crate) rels: &'a RelMap,
    pub(crate) archive: &'a crate::archive::DocxArchive,
}
```

Update the `ParseContext` construction in `crates/docmux-reader-docx/src/lib.rs:114-118`:

```rust
let ctx = document::ParseContext {
    styles: &styles,
    numbering: &numbering,
    rels: &rels,
    archive: &archive,
};
```

Update all existing test `ParseContext` constructions in `document.rs` `mod tests` to include archive. Add a test helper at the top of `mod tests`:

```rust
fn empty_archive() -> crate::archive::DocxArchive {
    let zip = crate::tests::make_zip(&[("word/document.xml", b"<doc/>")]);
    crate::archive::DocxArchive::from_bytes(&zip).unwrap()
}
```

Then update every existing test's `ParseContext` to include `archive: &empty_archive()`. Since there are many tests, use this pattern — store the archive in a let binding before the ctx:

```rust
let archive = empty_archive();
let ctx = ParseContext {
    styles: &styles,
    numbering: &numbering,
    rels: &rels,
    archive: &archive,
};
```

- [ ] **Step 5: Verify tests compile and existing tests pass**

Run: `cargo test -p docmux-reader-docx -- --nocapture 2>&1 | tail -20`
Expected: all existing tests pass, `parse_drawing_inline` fails (drawing not parsed yet).

- [ ] **Step 6: Implement drawing parsing in parse_runs**

In `parse_runs()`, add state variables after the hyperlink state block (around line 206):

```rust
// Drawing state
let mut in_drawing = false;
let mut drawing_xml = String::new();
let mut drawing_depth: u32 = 0;
```

In the `Event::Start` / `Event::Empty` match arm (line 210), add a case for `drawing` before the existing `_ => {}` catch-all. The approach: when we encounter `<w:drawing>`, we collect all its XML and parse it separately.

Add this inside the `match name` block, before `_ => {}`:

```rust
b"drawing" if in_run && !in_drawing => {
    in_drawing = true;
    drawing_depth = 1;
    drawing_xml.clear();
    append_start_tag(&mut drawing_xml, e);
}
```

For `Event::Start` when already in_drawing (must be checked before other matches — add at the very beginning of the Start arm):

Actually, the cleanest approach is to add drawing accumulation at the top of the event loop. Restructure the match to first check `in_drawing`:

In the main event loop, before the existing match arms, add drawing accumulation. The most practical way: add checks at the start of each event arm.

For `Event::Start`:
```rust
if in_drawing {
    drawing_depth += 1;
    append_start_tag(&mut drawing_xml, e);
    // Don't process other matches while accumulating drawing XML
} else {
    // existing match block
}
```

Handle `drawing` start detection inside the else branch:
```rust
b"drawing" if in_run => {
    in_drawing = true;
    drawing_depth = 1;
    drawing_xml.clear();
    append_start_tag(&mut drawing_xml, e);
}
```

For `Event::End`:
```rust
if in_drawing {
    drawing_depth -= 1;
    append_end_tag(&mut drawing_xml, e.name().as_ref());
    if drawing_depth == 0 {
        in_drawing = false;
        if let Some(image_inline) = parse_drawing(&drawing_xml, ctx)? {
            if in_hyperlink {
                hyperlink_inlines.push(image_inline);
            } else {
                inlines.push(image_inline);
            }
        }
        drawing_xml.clear();
    }
} else {
    // existing match block
}
```

For `Event::Empty`:
```rust
if in_drawing {
    append_empty_tag(&mut drawing_xml, e);
} else {
    // existing match block (only the `match name` block contents, not the full arm)
}
```

For `Event::Text`:
```rust
if in_drawing {
    let text = e.unescape().map_err(|err| DocxError::Xml(err.to_string()))?;
    drawing_xml.push_str(&quick_xml_escape(&text));
} else if in_t && in_run {
    // existing text handling
}
```

- [ ] **Step 7: Implement parse_drawing helper**

Add this function before `parse_runs` in `document.rs`:

```rust
/// Extract an `Inline::Image` from a `<w:drawing>` element.
///
/// Handles both `<wp:inline>` and `<wp:anchor>` drawing types.
/// Returns `None` if no image relationship can be resolved.
fn parse_drawing(xml: &str, ctx: &ParseContext<'_>) -> Result<Option<Inline>, DocxError> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();

    let mut alt_text = String::new();
    let mut embed_rid: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let name = e.local_name();
                match name.as_ref() {
                    b"docPr" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"name" {
                                alt_text = String::from_utf8_lossy(&attr.value).into_owned();
                            }
                        }
                    }
                    b"blip" => {
                        for attr in e.attributes().flatten() {
                            let key = attr.key.as_ref();
                            if key == b"r:embed" || key.ends_with(b":embed") {
                                embed_rid = Some(String::from_utf8_lossy(&attr.value).into_owned());
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(DocxError::Xml(e.to_string())),
            _ => {}
        }
        buf.clear();
    }

    let rid = match embed_rid {
        Some(r) => r,
        None => return Ok(None),
    };

    let rel = match ctx.rels.get(&rid) {
        Some(r) => r,
        None => return Ok(None),
    };

    let url = rel.target.clone();

    let alt = if alt_text.is_empty() {
        vec![]
    } else {
        vec![Inline::Text { value: alt_text }]
    };

    Ok(Some(Inline::Image(Image {
        url,
        alt,
        title: None,
        attrs: None,
    })))
}
```

Add `Image` to the imports at the top of `document.rs`:

```rust
use docmux_ast::{Alignment, Block, ColumnSpec, Image, Inline, Table, TableCell};
```

- [ ] **Step 8: Run the drawing test**

Run: `cargo test -p docmux-reader-docx parse_drawing_inline -- --nocapture`
Expected: PASS

- [ ] **Step 9: Write anchor drawing test**

Add another test in `mod tests`:

```rust
#[test]
fn parse_drawing_anchor() {
    let styles = StyleMap::new();
    let numbering = NumberingMap::new();
    let mut rels = RelMap::new();
    rels.insert(
        "rId7".to_string(),
        crate::relationships::Relationship {
            rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image".to_string(),
            target: "media/photo.jpg".to_string(),
            target_mode: None,
        },
    );
    let archive_bytes = crate::tests::make_zip(&[
        ("word/document.xml", b"<doc/>"),
        ("word/media/photo.jpg", b"\xFF\xD8"),
    ]);
    let archive = crate::archive::DocxArchive::from_bytes(&archive_bytes).unwrap();
    let ctx = ParseContext {
        styles: &styles,
        numbering: &numbering,
        rels: &rels,
        archive: &archive,
    };

    let xml = r#"<w:p xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                       xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
                       xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
                       xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
                       xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
        <w:r>
            <w:drawing>
                <wp:anchor distT="0" distB="0" distL="0" distR="0">
                    <wp:docPr id="2" name="A photo"/>
                    <a:graphic>
                        <a:graphicData>
                            <pic:pic>
                                <pic:blipFill>
                                    <a:blip r:embed="rId7"/>
                                </pic:blipFill>
                            </pic:pic>
                        </a:graphicData>
                    </a:graphic>
                </wp:anchor>
            </w:drawing>
        </w:r>
    </w:p>"#;

    let block = parse_paragraph(xml, &ctx).unwrap().unwrap();
    if let Block::Paragraph { content } = &block {
        assert_eq!(content.len(), 1);
        if let Inline::Image(img) = &content[0] {
            assert_eq!(img.url, "media/photo.jpg");
            assert_eq!(img.alt_text(), "A photo");
        } else {
            panic!("expected Image, got {:?}", content[0]);
        }
    } else {
        panic!("expected Paragraph, got {:?}", block);
    }
}
```

- [ ] **Step 10: Run all tests**

Run: `cargo test -p docmux-reader-docx`
Expected: all tests pass.

- [ ] **Step 11: Run clippy**

Run: `cargo clippy -p docmux-reader-docx --all-targets -- -D warnings`
Expected: no errors.

- [ ] **Step 12: Commit**

```bash
git add crates/docmux-reader-docx/src/document.rs crates/docmux-reader-docx/src/lib.rs
git commit -m "feat(docx): parse <w:drawing> elements into Inline::Image nodes"
```

---

### Task 4: HTML writer — render images with data URIs

**Files:**
- Modify: `crates/docmux-writer-html/Cargo.toml` (add base64 dependency)
- Modify: `crates/docmux-writer-html/src/lib.rs` (thread resources through write methods, render data URIs)

- [ ] **Step 1: Add base64 dependency**

In workspace `Cargo.toml`, add to `[workspace.dependencies]`:

```toml
base64 = "0.22"
```

In `crates/docmux-writer-html/Cargo.toml`, add:

```toml
base64 = { workspace = true }
```

- [ ] **Step 2: Write failing test — image with resources renders data URI**

In `crates/docmux-writer-html/src/lib.rs`, add a test (find the existing `#[cfg(test)] mod tests` block):

```rust
#[test]
fn image_with_resource_renders_data_uri() {
    use base64::Engine;
    use std::collections::HashMap;
    use docmux_ast::ResourceData;

    let png_bytes = b"\x89PNG\r\n\x1a\nfake";
    let expected_b64 = base64::engine::general_purpose::STANDARD.encode(png_bytes);

    let doc = Document {
        resources: HashMap::from([(
            "media/image1.png".to_string(),
            ResourceData {
                mime_type: "image/png".to_string(),
                data: png_bytes.to_vec(),
            },
        )]),
        content: vec![Block::Paragraph {
            content: vec![Inline::Image(docmux_ast::Image {
                url: "media/image1.png".to_string(),
                alt: vec![Inline::Text { value: "A logo".to_string() }],
                title: None,
                attrs: None,
            })],
        }],
        ..Default::default()
    };

    let writer = HtmlWriter::new();
    let output = writer.write(&doc, &WriteOptions::default()).unwrap();
    let expected_src = format!("data:image/png;base64,{expected_b64}");
    assert!(output.contains(&expected_src), "output should contain data URI, got: {output}");
    assert!(output.contains("alt=\"A logo\""));
}

#[test]
fn image_without_resource_renders_path() {
    let doc = Document {
        content: vec![Block::Paragraph {
            content: vec![Inline::Image(docmux_ast::Image {
                url: "images/photo.jpg".to_string(),
                alt: vec![],
                title: None,
                attrs: None,
            })],
        }],
        ..Default::default()
    };

    let writer = HtmlWriter::new();
    let output = writer.write(&doc, &WriteOptions::default()).unwrap();
    assert!(output.contains("src=\"images/photo.jpg\""), "should use path as-is, got: {output}");
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-html image_with_resource -- --nocapture`
Expected: FAIL — writer doesn't check resources yet.

- [ ] **Step 4: Thread Document through write methods**

The `write` method has access to `&Document`, but `write_block`/`write_inline` only receive `&[Block]` / `&Inline`. Add `&Document` parameter to the internal method chain.

Update the method signatures:

```rust
fn write_blocks(&self, blocks: &[Block], opts: &WriteOptions, doc: &Document, out: &mut String)
fn write_block(&self, block: &Block, opts: &WriteOptions, doc: &Document, out: &mut String)
fn write_inlines(&self, inlines: &[Inline], opts: &WriteOptions, doc: &Document, out: &mut String)
fn write_inline(&self, inline: &Inline, opts: &WriteOptions, doc: &Document, out: &mut String)
```

Update the `write` method call:
```rust
fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
    let mut body = String::with_capacity(4096);
    self.write_blocks(&doc.content, opts, doc, &mut body);
    // ... rest unchanged
}
```

Update all internal call sites that call `write_blocks`, `write_block`, `write_inlines`, or `write_inline` to pass the `doc` parameter through.

- [ ] **Step 5: Add data URI rendering for images**

Add import at the top of `lib.rs`:

```rust
use base64::Engine;
```

Create a helper function:

```rust
fn image_src(url: &str, doc: &Document) -> String {
    if let Some(res) = doc.resources.get(url) {
        if !res.data.is_empty() {
            let b64 = base64::engine::general_purpose::STANDARD.encode(&res.data);
            return format!("data:{};base64,{}", res.mime_type, b64);
        }
    }
    url.to_string()
}
```

Update `Inline::Image` rendering (around line 409):

```rust
Inline::Image(img) => {
    let src = image_src(&img.url, doc);
    out.push_str(&format!(
        "<img src=\"{}\" alt=\"{}\"",
        escape_attr(&src),
        escape_attr(&img.alt_text())
    ));
    if let Some(t) = &img.title {
        out.push_str(&format!(" title=\"{}\"", escape_attr(t)));
    }
    out.push('>');
}
```

Update `Block::Figure` rendering (around line 160):

```rust
Block::Figure { image, caption, label, .. } => {
    // ... existing label/figure tag logic ...
    let src = image_src(&image.url, doc);
    out.push_str(&format!(
        "<img src=\"{}\" alt=\"{}\">",
        escape_attr(&src),
        escape_attr(&image.alt_text())
    ));
    // ... rest unchanged
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p docmux-writer-html`
Expected: all tests pass including new ones.

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -p docmux-writer-html --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml crates/docmux-writer-html/Cargo.toml crates/docmux-writer-html/src/lib.rs
git commit -m "feat(html): render embedded images as base64 data URIs"
```

---

### Task 5: Integration test — DOCX with image end-to-end

**Files:**
- Modify: `crates/docmux-reader-docx/src/lib.rs` (add integration test in `mod tests`)

- [ ] **Step 1: Write integration test**

Add in `crates/docmux-reader-docx/src/lib.rs` `mod tests`:

```rust
#[test]
fn read_docx_with_inline_image() {
    let doc_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"
            xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"
            xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"
            xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
  <w:body>
    <w:p>
      <w:r>
        <w:drawing>
          <wp:inline>
            <wp:docPr id="1" name="Test Image"/>
            <a:graphic>
              <a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">
                <pic:pic>
                  <pic:blipFill>
                    <a:blip r:embed="rId5"/>
                  </pic:blipFill>
                </pic:pic>
              </a:graphicData>
            </a:graphic>
          </wp:inline>
        </w:drawing>
      </w:r>
    </w:p>
  </w:body>
</w:document>"#;

    let rels_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId5"
    Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image"
    Target="media/image1.png"/>
</Relationships>"#;

    let fake_png = b"\x89PNG\r\n\x1a\nfake image bytes for test";
    let zip_bytes = make_zip(&[
        ("word/document.xml", doc_xml.as_bytes()),
        ("word/_rels/document.xml.rels", rels_xml.as_bytes()),
        ("word/media/image1.png", fake_png),
    ]);

    let reader = DocxReader;
    let doc = reader.read_bytes(&zip_bytes).unwrap();

    // Verify image inline
    assert_eq!(doc.content.len(), 1);
    if let Block::Paragraph { content } = &doc.content[0] {
        assert_eq!(content.len(), 1);
        if let docmux_ast::Inline::Image(img) = &content[0] {
            assert_eq!(img.url, "media/image1.png");
            assert_eq!(img.alt_text(), "Test Image");
        } else {
            panic!("expected Image, got {:?}", content[0]);
        }
    } else {
        panic!("expected Paragraph");
    }

    // Verify resource loaded
    let res = doc.resources.get("media/image1.png").expect("resource should exist");
    assert_eq!(res.mime_type, "image/png");
    assert_eq!(res.data, fake_png);
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p docmux-reader-docx read_docx_with_inline_image -- --nocapture`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/docmux-reader-docx/src/lib.rs
git commit -m "test(docx): end-to-end integration test for image extraction"
```

---

### Task 6: Rebuild WASM and verify in playground

**Files:**
- No source changes — build + manual verification

- [ ] **Step 1: Run full workspace tests**

Run: `cargo test --workspace`
Expected: all tests pass.

- [ ] **Step 2: Run clippy on full workspace**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 3: Rebuild WASM**

Run: `wasm-pack build crates/docmux-wasm --target web --out-dir ../../playground/wasm-pkg`
Expected: builds successfully.

- [ ] **Step 4: Verify playground type-checks**

Run: `cd playground && pnpm exec tsc --noEmit`
Expected: no errors.

- [ ] **Step 5: Manual verification**

Start playground dev server and open a DOCX file with images. Verify:
- Preview tab shows images inline
- AST tab shows `Inline::Image` nodes with `"media/image1.png"` paths (no base64 in JSON)
- Source tab HTML contains `<img src="data:image/png;base64,...">` tags

- [ ] **Step 6: Commit WASM artifacts if needed**

If wasm-pkg is tracked in git, commit the rebuilt artifacts:

```bash
git add playground/wasm-pkg/
git commit -m "build: rebuild WASM with image extraction support"
```
