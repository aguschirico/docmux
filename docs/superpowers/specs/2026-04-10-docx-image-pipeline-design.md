# DOCX Image Pipeline + Playground Binary Output

> Date: 2026-04-10

## Problem

The DOCX writer can embed images, but only from local filesystem paths resolved from CWD. This breaks three scenarios:

1. **Markdown/LaTeX -> DOCX via CLI**: relative image paths (e.g. `![](img/photo.png)`) fail because the writer resolves from CWD, not from the input file's directory.
2. **DOCX -> DOCX roundtrip**: the DOCX reader extracts images to `doc.resources`, but the writer ignores resources — images are lost.
3. **Playground**: no DOCX output at all (WASM functions only return strings), and no way to include images in conversions.

Additionally, all embedded images are hardcoded to 4"x3" regardless of actual dimensions.

## Design

Five coordinated changes across the stack.

### 1. DOCX Writer — Resource-aware image embedding

**File:** `crates/docmux-writer-docx/src/lib.rs`

Change `embed_image` to accept a reference to `Document` (or store `&doc.resources` in the writer struct). Image resolution order:

1. Look up URL as key in `doc.resources` -> use bytes + MIME from `ResourceData`
2. Try filesystem path (existing behavior) -> read bytes from disk
3. Fallback: emit `[Image: url]` as text run

**Dimension parsing** — new internal function `image_dimensions(data: &[u8]) -> Option<(u32, u32)>`:

- **PNG**: read bytes 16-23 of IHDR chunk (width + height as u32 big-endian). Validate PNG magic bytes `89 50 4E 47` first.
- **JPEG**: scan for SOF0 (`FF C0`) or SOF2 (`FF C2`) marker, read height (2 bytes) then width (2 bytes) at offset +3 from marker.
- Convert pixels to EMU: `px * 914400 / 96` (assuming 96 DPI).
- Cap width to 6 inches (5,486,400 EMU). If width exceeds cap, scale both dimensions proportionally.
- If parsing fails, fall back to current 4"x3" default.

**MIME detection** from magic bytes (not extension):

- `89 50 4E 47` -> `image/png`
- `FF D8 FF` -> `image/jpeg`
- Extension-based fallback for other formats.

**Struct change**: the writer needs access to `doc.resources` during `write_inlines`/`write_blocks`. Store a reference or clone of the resources map in `DocxWriter` at the start of `write_bytes`.

### 2. CLI — Path resolution and resource pre-loading

**File:** `crates/docmux-cli/src/main.rs` (or a utility module)

New function `preload_image_resources(doc: &mut Document, base_dir: &Path)`:

1. Walk the AST collecting image URLs from `Inline::Image` and `Block::Figure`.
2. For each URL:
   - If relative path -> resolve against `base_dir` (parent directory of the input file).
   - If the resolved path exists -> read bytes, detect MIME from magic bytes, insert into `doc.resources` with the original URL as key.
   - If path doesn't exist -> skip (writer will show fallback).
3. Call this function after parsing, before writing, for all output formats — the HTML writer already uses `doc.resources` for data URIs, and future writers may too.

This lives in the CLI crate — filesystem I/O is a CLI concern, not a core/writer concern. The writer stays pure (reads from `doc.resources`, never touches the filesystem... except as a fallback for backward compat which we can deprecate later).

### 3. WASM — Binary output and resource support

**File:** `crates/docmux-wasm/src/lib.rs`

Three new `#[wasm_bindgen]` functions:

| Function | Input | Output | Purpose |
|----------|-------|--------|---------|
| `convertWithResources(text, inputFmt, outputFmt, resources)` | `&str` + `&str` + `&str` + `js_sys::Map` | `Result<String, JsError>` | Text input -> string output with image resources (md->html with images) |
| `convertToBytes(text, inputFmt, outputFmt, resources)` | `&str` + `&str` + `&str` + `js_sys::Map` | `Result<js_sys::Uint8Array, JsError>` | Text input -> binary output (md->docx) |
| `convertBytesToBytes(bytes, inputFmt, outputFmt, resources)` | `&[u8]` + `&str` + `&str` + `js_sys::Map` | `Result<js_sys::Uint8Array, JsError>` | Binary input -> binary output (docx->docx) |

The `resources` parameter (`js_sys::Map`):
- Keys: filenames as `JsValue` strings (e.g. `"photo.png"`)
- Values: `js_sys::Uint8Array` with the image bytes
- Iterate the map entries, convert each to `ResourceData { mime_type, data }`, insert into `doc.resources`
- MIME type detected from magic bytes (same logic as writer)

For binary output, use `writer.write_bytes(doc, opts)?` and return the `Vec<u8>` as `js_sys::Uint8Array::from(&bytes[..])`.

### 4. Playground — Image drag-and-drop, DOCX output, resource passing

#### 4a. Image drag-and-drop on editor

**Files:** `playground/src/hooks/useDropZone.ts`, new hook or extension of existing

Extend the drop handler:
- Detect image files by extension (`.png`, `.jpg`, `.jpeg`, `.gif`, `.svg`) or MIME type.
- Store in VFS: `createBinaryFile(workspaceId, filename, arrayBuffer)`.
- Dedup: if a file with the same name exists in the workspace, append a numeric suffix (`photo-1.png`, `photo-2.png`).
- Auto-insert `![](filename)` at the CodeMirror cursor position.
- Show toast confirmation.

#### 4b. DOCX in output format dropdown

**File:** `playground/src/components/OutputTabs.tsx`

- Add `"docx"` to `OUTPUT_FORMATS` array.
- Add `"docx"` to `BINARY_FORMATS` set in `lib/formats.ts`.

#### 4c. Binary download handler

**File:** `playground/src/components/OutputTabs.tsx`

- `handleDownload()` checks if output format is binary.
- If binary: create `Blob` from `Uint8Array` with MIME `application/vnd.openxmlformats-officedocument.wordprocessingml.document`, download as `.docx`.
- Preview tab for DOCX output: show a message like "Binary format — use Download to save" instead of trying to render source text.

#### 4d. Pass VFS resources to WASM

**File:** `playground/src/hooks/useConversion.ts`

- Accept `workspaceId` as parameter (or access it from context).
- Before conversion, query Dexie for all `VfsFile` entries in the workspace that have `binaryContent` and an image extension.
- Build a `Map<string, Uint8Array>` from `path -> new Uint8Array(binaryContent)`.
- Use `convertWithResources` for text->text conversions (so HTML preview shows images as data URIs).
- Use `convertToBytes` for text->binary conversions (md->docx).
- Use `convertBytesToBytes` for binary->binary (docx->docx).
- Existing `convert`/`convertBytes` functions remain for the no-resources case.

### 5. Testing

#### Rust unit tests

**DOCX writer (`crates/docmux-writer-docx/src/lib.rs`):**
- Image from `doc.resources` embeds correctly in ZIP output
- PNG dimension parsing (IHDR chunk) returns correct width/height
- JPEG dimension parsing (SOF marker) returns correct width/height
- Image > 6" wide scales proportionally (e.g. 1200x600 @ 96dpi = 12.5" -> capped to 6" x 3")
- Fallback `[Image: url]` when resource not found and file doesn't exist
- MIME detection from magic bytes (not just extension)

**CLI (`crates/docmux-cli/`):**
- Relative image path resolved and loaded into `doc.resources`
- Missing image file doesn't cause error

**WASM (`crates/docmux-wasm/`):**
- `convertToBytes` with resources produces valid DOCX (ZIP-parseable)
- `convertBytesToBytes` preserves images in DOCX->DOCX roundtrip

#### Playground
- Type check: `tsc --noEmit`
- Lint: `eslint`
- Manual test: drop image on editor -> appears in file tree + markdown inserted -> switch output to DOCX -> download -> open in Word/LibreOffice -> image visible with correct dimensions

## Out of scope

- HTTP/HTTPS image fetching (remote URLs)
- SVG rendering/rasterization for DOCX (SVG not supported in OOXML inline images)
- Image resize attributes in markdown (`{width=50%}`)
- GIF animation in DOCX (only first frame would show)
- `--extract-media=DIR` CLI flag
- `--self-contained` / `--embed-resources` CLI flag

## Dependencies

- `js-sys` (already in docmux-wasm Cargo.toml) for `Map`, `Uint8Array`
- No new external crates needed — PNG/JPEG header parsing is ~30 lines of manual byte reading
