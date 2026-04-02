# DOCX Image Extraction — Design Spec

## Problem

The DOCX reader parses text, tables, hyperlinks, and formatting, but silently drops images. DOCX files embed images as binary entries in the ZIP archive, referenced via OOXML `<w:drawing>` elements. Users see broken or missing images in output.

## Approach

Resource map on the Document (pandoc-style media bag). The reader extracts image bytes from the ZIP and stores them in a `HashMap` on the `Document` struct. The AST references images by path. Writers decide how to materialize the bytes (data URI, write to disk, re-embed).

## Design

### AST Changes (`docmux-ast`)

New struct:

```rust
#[derive(Debug, Clone)]
pub struct ResourceData {
    pub mime_type: String,  // "image/png", "image/jpeg", etc.
    #[serde(skip)]
    pub data: Vec<u8>,
}
```

`data` is `#[serde(skip)]` so AST JSON serialization stays clean (no giant byte arrays). The paths in `Image.url` already indicate which images exist.

New field on `Document`:

```rust
pub struct Document {
    pub metadata: Metadata,
    pub content: Vec<Block>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub resources: HashMap<String, ResourceData>,
}
```

`skip_serializing_if` keeps the JSON unchanged for documents without images.

### DOCX Reader (`docmux-reader-docx`)

#### Resource loading

Before parsing the document body, the reader loads all media files from the ZIP into `Document.resources`:

```
for path in archive.media_paths():
    bytes = archive.get_bytes("word/" + path)
    mime  = infer from file extension
    resources[path] = ResourceData { mime_type, data: bytes }
```

This avoids threading mutable state through the parser. `archive.media_paths()` and `archive.get_bytes()` already exist.

#### ParseContext change

Add archive reference so the parser can resolve relationship IDs to media paths:

```rust
pub(crate) struct ParseContext<'a> {
    pub(crate) styles: &'a StyleMap,
    pub(crate) numbering: &'a NumberingMap,
    pub(crate) rels: &'a RelMap,
    pub(crate) archive: &'a DocxArchive,  // new
}
```

The archive reference is used only for rId resolution (confirming the target file exists), not for reading bytes (that happens upfront).

#### Drawing parser

In `parse_runs()`, detect `<w:drawing>` elements inside `<w:r>`. Collect the full drawing XML, then extract:

1. **Image type**: `<wp:inline>` or `<wp:anchor>` — both treated as inline in the AST
2. **Alt text**: `<wp:docPr name="...">` attribute
3. **Relationship ID**: `<a:blip r:embed="rId3">` — the rId referencing the image
4. Resolution: `rels[rId3].target` → `"media/image1.png"`

Emit `Inline::Image(Image { url: "media/image1.png", alt: [...], title: None })`.

OOXML structure reference:

```xml
<w:r>
  <w:drawing>
    <wp:inline>                              <!-- or <wp:anchor> -->
      <wp:extent cx="1828800" cy="1371600"/>
      <wp:docPr id="1" name="logo.png"/>
      <a:graphic>
        <a:graphicData>
          <pic:pic>
            <pic:blipFill>
              <a:blip r:embed="rId3"/>       <!-- key: relationship ID -->
            </pic:blipFill>
          </pic:pic>
        </a:graphicData>
      </a:graphic>
    </wp:inline>
  </w:drawing>
</w:r>
```

#### MIME type inference

From file extension, covering common DOCX image formats:

| Extension | MIME type |
|-----------|-----------|
| png | image/png |
| jpg, jpeg | image/jpeg |
| gif | image/gif |
| svg | image/svg+xml |
| bmp | image/bmp |
| tiff, tif | image/tiff |
| wmf | image/x-wmf |
| emf | image/x-emf |

Fallback: `application/octet-stream`.

### HTML Writer (`docmux-writer-html`)

When rendering `Inline::Image` or `Block::Figure`, check `doc.resources` for the image path:

```
if let Some(resource) = doc.resources.get(&image.url):
    src = format!("data:{};base64,{}", resource.mime_type, base64_encode(resource.data))
else:
    src = image.url  // fallback: use path as-is
```

The `write` method already receives `&Document`, so resources are accessible. Add `base64` crate as dependency.

No changes needed for LaTeX, Typst, Markdown, or plaintext writers.

### WASM + Playground

No changes. The existing pipeline already works:

1. Playground sends DOCX bytes via `convertBytes()` / `parseBytesToJson()`
2. WASM calls `DocxReader::read_bytes()` → `Document` with populated `resources`
3. HTML writer emits `<img src="data:...">` → browser renders natively
4. AST tab shows `Image` nodes with clean paths (no base64), since `ResourceData.data` is `#[serde(skip)]`

## Scope

### In scope

- `<wp:inline>` and `<wp:anchor>` drawing elements
- Image formats: PNG, JPEG, GIF, SVG, BMP, TIFF, WMF, EMF
- Alt text from `<wp:docPr name="...">`
- Data URI output in HTML writer
- Resource map on Document struct

### Out of scope

- VML legacy images (`<v:imagedata>`)
- Image dimensions/sizing in output
- Floating image positioning (anchors rendered as inline)
- Extracting images to disk in CLI
- Changes to non-HTML writers

## Testing

- Unit test: `parse_drawing_inline` — XML with `<w:drawing><wp:inline>` produces `Inline::Image` with correct path and alt text
- Unit test: `parse_drawing_anchor` — XML with `<wp:anchor>` also produces `Inline::Image`
- Unit test: resource loading from archive — media paths populated in `Document.resources`
- Unit test: HTML writer data URI — image with resources produces `<img src="data:...">`, image without resources produces `<img src="path">`
- Integration test: round-trip a DOCX with an embedded image, verify HTML output contains data URI
