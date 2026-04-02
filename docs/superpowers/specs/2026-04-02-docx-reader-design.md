# DOCX Reader Design

**Date:** 2026-04-02
**Status:** Approved
**Scope:** General-purpose DOCX reader — parse DOCX from any source (Word, Google Docs, LibreOffice) into the docmux AST. Closes the DOCX roundtrip. Supports CLI and WASM/playground.

---

## 1. `BinaryReader` trait

New trait in `docmux-core`, parallel to `Reader`:

```rust
pub trait BinaryReader: Send + Sync {
    fn format(&self) -> &str;
    fn extensions(&self) -> &[&str];
    fn read_bytes(&self, input: &[u8]) -> Result<Document>;
}
```

The `FormatRegistry` gets a second table for binary readers. Dispatch logic:

1. Check binary readers first by format/extension.
2. If found, read file as `Vec<u8>`, call `read_bytes()`.
3. Otherwise, fall back to text `Reader` path (`read_to_string` → `read()`).

Existing `Reader` trait and all 5 text readers are untouched.

---

## 2. Crate structure

```
crates/docmux-reader-docx/src/
├── lib.rs           — DocxReader + BinaryReader impl, orchestration
├── archive.rs       — ZIP decompression, extract parts by path
├── relationships.rs — Parse _rels/document.xml.rels (rId → target URL/media)
├── styles.rs        — Parse styles.xml, map styleId → semantic info
├── numbering.rs     — Parse numbering.xml, resolve numId+ilvl → ordered/style/level
├── document.rs      — Parse document.xml → Vec<Block> (core of the reader)
├── footnotes.rs     — Parse footnotes.xml → HashMap<id, Vec<Block>>
└── media.rs         — Extract images from word/media/, resolve rId → bytes
```

**Dependencies:**
- `zip` (already in workspace) — decompression
- `quick-xml` (new) — XML parsing, fast, low memory, WASM-compatible. Used in DOM mode (`quick-xml::reader::Reader` → build in-memory tree per XML part) since document.xml parts are small enough and tree walking is much more ergonomic for recursive AST conversion than SAX events.
- `docmux-ast` + `docmux-core`

**Parsing flow:**

```
ZIP bytes
  → archive.rs: extract entries
  → relationships.rs: rId → target map
  → styles.xml: styleId → {name, basedOn, type}
  → numbering.xml: numId → {ordered, style, levels}
  → footnotes.xml: id → Vec<Block>
  → document.xml: walk <w:body> → Vec<Block>
      ├── <w:p> → classify(style, numPr) → Heading | Paragraph | CodeBlock | ListItem | ...
      ├── <w:tbl> → Table
      ├── <w:drawing> → Figure
      └── inline runs (<w:r>) → Vec<Inline> via run properties
```

---

## 3. Element mapping: OOXML → AST

### Blocks

| OOXML | Signal | AST Node |
|-------|--------|----------|
| `<w:p>` with style `Heading1`–`Heading6` or `Title` | `<w:pStyle>` name match | `Heading { level }` |
| `<w:p>` with `<w:numPr>` | `numId` + `ilvl` → numbering.xml lookup | `List { ordered, items }` |
| `<w:p>` with style `CodeBlock` or monospace font in all runs | Style name or font heuristic | `CodeBlock` |
| `<w:p>` with style containing "Quote" or left borders | Style name or `<w:pBdr>` | `BlockQuote` |
| `<w:p>` with left border color `#4472C4` | Matches writer's admonition pattern | `Admonition` |
| `<w:p>` plain | No special signals | `Paragraph` |
| `<w:tbl>` | Always | `Table` with header/rows/footer, `gridSpan` → colspan |
| `<w:p>` with `<w:drawing>` / `<wp:inline>` | Image relationship | `Figure` |
| `<w:p>` with `<w:pBdr><w:bottom>` and no text | Thematic break pattern | `ThematicBreak` |
| `<w:p>` bold + next indented | DL pattern heuristic | `DefinitionList` |
| `<w:footnoteReference>` + `footnotes.xml` entry | Footnote ID cross-ref | `FootnoteDef` + `FootnoteRef` |
| `<w:p>` with style `MathBlock` | Style name | `MathBlock` |

### Inlines (run properties `<w:rPr>`)

| OOXML | AST Inline |
|-------|------------|
| `<w:b/>` | `Strong` |
| `<w:i/>` | `Emphasis` |
| `<w:strike/>` | `Strikethrough` |
| `<w:u val="single"/>` | `Underline` |
| `<w:vertAlign val="superscript"/>` | `Superscript` |
| `<w:vertAlign val="subscript"/>` | `Subscript` |
| `<w:smallCaps/>` | `SmallCaps` |
| `<w:rFonts>` monospace + `<w:sz>` small | `Code` |
| `<w:hyperlink>` with `r:id` | `Link` (resolve via relationships) |
| `<w:drawing>` inline | `Image` |
| `<w:footnoteReference>` | `FootnoteRef` |
| `<w:br/>` | `HardBreak` |
| Smart quotes `\u{201C}`/`\u{201D}` | `Quoted { DoubleQuote }` |
| `<w:t>` plain | `Text` |

### Metadata

From `docProps/core.xml` (Dublin Core):
- `dc:title` → `metadata.title`
- `dc:creator` → `metadata.authors`
- `dcterms:created` → `metadata.date`
- `dc:subject` / `cp:keywords` → `metadata.keywords`
- Custom properties → `metadata.custom`

### Ignored elements (safe skip)

Headers/footers, comments, revision tracking (`<w:ins>`, `<w:del>`), form fields, embedded OLE objects, VBA macros. Omitted without error — reader focuses on body content.

---

## 4. CLI changes

Symmetric to existing binary output path:

```rust
if let Some(binary_reader) = registry.get_binary_reader(from_format) {
    let bytes = std::fs::read(&input_path)?;
    let doc = binary_reader.read_bytes(&bytes)?;
    // continue to transforms + writer
} else {
    let text = std::fs::read_to_string(&input_path)?;
    let doc = reader.read(&text)?;
}
```

Stdin input (`-`): read as `Vec<u8>`, then dispatch based on format.

---

## 5. WASM changes

New exported functions:

```rust
#[wasm_bindgen]
pub fn convertBytes(input: &[u8], from: &str, to: &str) -> Result<String, JsError>;

#[wasm_bindgen]
pub fn convertBytesStandalone(input: &[u8], from: &str, to: &str) -> Result<String, JsError>;

#[wasm_bindgen]
pub fn parseBytesToJson(input: &[u8], from: &str) -> Result<String, JsError>;
```

TypeScript usage from the playground:

```typescript
const bytes = new Uint8Array(await file.arrayBuffer());
const html = wasm.convertBytes(bytes, "docx", "html");
```

`inputFormats()` automatically includes `"docx"` once the binary reader is registered.

---

## 6. Testing strategy

### Golden file tests

```
tests/
├── fixtures/
│   ├── basic-paragraphs.docx      → basic-paragraphs.expected.json
│   ├── headings.docx              → headings.expected.json
│   ├── lists-nested.docx          → lists-nested.expected.json
│   ├── tables-complex.docx        → tables-complex.expected.json
│   ├── formatting-mixed.docx      → formatting-mixed.expected.json
│   ├── footnotes.docx             → footnotes.expected.json
│   ├── images.docx                → images.expected.json
│   └── metadata.docx              → metadata.expected.json
└── golden.rs
```

Fixtures generated two ways:
- **Roundtrip:** Markdown → docmux DOCX writer → `.docx`
- **External:** Documents from Word/Google Docs saved as fixtures

Updated with `DOCMUX_UPDATE_EXPECTATIONS=1`.

### Unit tests

Per module: `archive.rs`, `styles.rs`, `numbering.rs`, `relationships.rs`, `document.rs` — each tested with minimal XML fragments.

### Roundtrip tests

```rust
#[test]
fn roundtrip_markdown_through_docx() {
    let original = markdown_reader.read(MD_INPUT).unwrap();
    let bytes = docx_writer.write_bytes(&original, &opts).unwrap();
    let recovered = docx_reader.read_bytes(&bytes).unwrap();
    assert_ast_equivalent(&original, &recovered);
}
```

Comparison with tolerance for attributes/raw blocks that DOCX cannot represent.

---

## 7. Style classifier design

The style classifier resolves a `<w:p>` to an AST block type. Priority order:

1. **Exact style ID match** — `Heading1` → `Heading { level: 1 }`, `Title` → `Heading { level: 1 }`, `CodeBlock` → `CodeBlock`
2. **Style name pattern match** — name contains "heading", "quote", "code" (case-insensitive)
3. **Numbering properties** — `<w:numPr>` present → `ListItem` (ordered/unordered from numbering.xml)
4. **Formatting heuristics** — all runs monospace → `CodeBlock`, left border → `BlockQuote`
5. **Fallback** — `Paragraph`

Built-in mappings cover Word (en/es/de/fr style names), Google Docs, and LibreOffice default styles.
