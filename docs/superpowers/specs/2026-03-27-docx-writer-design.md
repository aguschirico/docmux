# DOCX Writer Design

> Date: 2026-03-27

## Overview

Implement `docmux-writer-docx` — a DOCX writer that produces valid Office Open XML files from the docmux AST. The writer generates raw XML + ZIP (no external DOCX library), consistent with how other docmux writers work.

## Decisions

- **Fidelity**: Functional — correct structure, editable in Word, no premium styling
- **Images**: Local files embedded in ZIP, remote URLs rendered as hyperlinks
- **Implementation**: Raw XML generation + `zip` crate, monolithic `lib.rs`
- **Math**: Plain text for now. OMML (Office Math Markup Language) is a planned future improvement
- **Dependencies**: Only `zip` added to workspace. XML generated via `format!()`/`write!()`

## OOXML Structure

Minimal ZIP layout:

```
[Content_Types].xml              — MIME types for each part
_rels/.rels                      — Root relationships (points to document.xml)
word/document.xml                — Document content
word/styles.xml                  — Style definitions (Heading1-6, Normal, Code, etc.)
word/footnotes.xml               — Footnotes (if any)
word/numbering.xml               — List numbering definitions
word/_rels/document.xml.rels     — Relationships from document (images, footnotes, hyperlinks)
word/media/imageN.ext            — Embedded images
```

## AST to OOXML Mapping

### Blocks

| AST Block | OOXML Element |
|-----------|---------------|
| `Paragraph` | `<w:p>` with runs |
| `Heading { level }` | `<w:p>` with `<w:pPr><w:pStyle w:val="Heading{N}"/>` |
| `CodeBlock` | `<w:p>` with style "CodeBlock", Courier New font, `<w:shd>` background |
| `MathBlock` | `<w:p>` with style "MathBlock", content as plain text |
| `BlockQuote` | `<w:p>` with left indent + style "BlockQuote" |
| `List` | `<w:p>` with `<w:numPr>` referencing numbering.xml |
| `Table` | `<w:tbl>` with `<w:tr>` / `<w:tc>`, colspan via `<w:gridSpan>` |
| `Figure` | Image as `<w:drawing>` + caption as `<w:p>` with style "Caption" |
| `ThematicBreak` | `<w:p>` with bottom border |
| `RawBlock { format: "docx" }` | Passthrough XML. Other formats skipped |
| `Admonition` | `<w:p>` with left border + bold title |
| `DefinitionList` | Term in bold + indented definitions |
| `FootnoteDef` | `<w:footnote>` in footnotes.xml |
| `Div` | Transparent — recurse into children |

### Inlines

| AST Inline | OOXML Element |
|------------|---------------|
| `Text` | `<w:r><w:t>` |
| `Emphasis` | `<w:rPr><w:i/>` |
| `Strong` | `<w:rPr><w:b/>` |
| `Strikethrough` | `<w:rPr><w:strike/>` |
| `Underline` | `<w:rPr><w:u w:val="single"/>` |
| `Code` | `<w:rPr>` with Courier New font |
| `MathInline` | Plain text with delimiters (OMML future) |
| `Link` | `<w:hyperlink r:id="rIdN">` with relationship |
| `Image` | `<w:drawing><wp:inline>` with relationship to media/ |
| `Citation` | `[key]` as text |
| `FootnoteRef` | `<w:footnoteReference w:id="N"/>` |
| `CrossRef` | Label text |
| `Superscript` | `<w:rPr><w:vertAlign w:val="superscript"/>` |
| `Subscript` | `<w:rPr><w:vertAlign w:val="subscript"/>` |
| `SmallCaps` | `<w:rPr><w:smallCaps/>` |
| `Quoted` | Literal quote characters around content |
| `Span` | Apply attrs, recurse |
| `SoftBreak` | Space character |
| `HardBreak` | `<w:br/>` |
| `RawInline` | Passthrough if format is "docx", skip otherwise |

## Metadata

- `title` → `<w:p>` with style "Title" at document start
- `authors` → `<w:p>` with style "Author" below title
- `date` → `<w:p>` with style "Date"
- `abstract_text` → Block with style "Abstract" (indented, italic)
- `keywords` → Skipped (no standard Word body representation)

## Images

1. Read file from path relative to input
2. Detect format (PNG/JPEG/GIF) by extension
3. Add to `word/media/imageN.ext` in ZIP
4. Create relationship in `word/_rels/document.xml.rels`
5. Reference with `<w:drawing><wp:inline>` in document.xml
6. Size: use actual image dimensions, capped to page width (~6 inches / 5486400 EMUs)

## Footnotes

- Each `FootnoteDef` generates `<w:footnote w:id="N">` in footnotes.xml
- `FootnoteRef` generates `<w:footnoteReference w:id="N"/>` inline
- IDs 0 and 1 are reserved by Word (separator/continuation separator)
- User footnotes start at ID 2

## Lists / numbering.xml

- Each unique combination of (ordered, style, delimiter) generates an `<w:abstractNum>` in numbering.xml
- Nesting handled via `<w:ilvl>` (indent level)
- Bullets: bullet character
- Ordered: Decimal, LowerAlpha, UpperAlpha, LowerRoman, UpperRoman mapped to `w:numFmt`

## Styles (styles.xml)

Built-in style definitions:

- **Paragraph styles**: Normal, Heading1–Heading6, Title, Author, Date, CodeBlock, BlockQuote, Caption, Abstract, FootnoteText, MathBlock
- **Character styles**: CodeChar (inline code), FootnoteReference, Hyperlink

## Dependencies

```toml
[dependencies]
docmux-ast = { workspace = true }
docmux-core = { workspace = true }
zip = { version = "2", default-features = false, features = ["deflate"] }
```

## Writer Trait Implementation

```rust
impl Writer for DocxWriter {
    fn format(&self) -> &str { "docx" }
    fn default_extension(&self) -> &str { "docx" }

    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String> {
        // DOCX is binary — return error directing to write_bytes()
        Err(...)
    }

    fn write_bytes(&self, doc: &Document, opts: &WriteOptions) -> Result<Vec<u8>> {
        // Build ZIP in memory, return bytes
    }
}
```

Internal state during generation:

```rust
struct DocxBuilder {
    relationships: Vec<Relationship>,  // hyperlinks, images, footnotes
    footnotes: Vec<(u32, String)>,     // (id, xml_content)
    media: Vec<(String, Vec<u8>)>,     // (filename, bytes)
    numbering_defs: Vec<NumberingDef>, // list definitions
    next_rel_id: u32,
    next_footnote_id: u32,             // starts at 2
    next_image_id: u32,
}
```

## CLI Integration

Register in `build_registry()` in `crates/docmux-cli/src/main.rs`:

```rust
reg.add_writer(Box::new(DocxWriter::new()));
```

The CLI already handles `write_bytes()` for binary output.

## WASM Integration

Register in `docmux-wasm` crate. Note: image embedding from local filesystem won't work in WASM context — images will be skipped with a warning.

## Testing Strategy

- **Unit tests**: Each block/inline type → verify generated XML fragments
- **Integration tests**: `.md` → parse → write DOCX → decompress ZIP → compare `document.xml` content
- **No binary golden files**: DOCX files contain timestamps and may vary. Compare extracted XML instead.
- **CLI smoke tests**: `docmux input.md -o output.docx` → verify valid ZIP with correct structure
- **Validation**: Open generated files in LibreOffice/Word to verify rendering (manual spot check)

## Future Improvements (out of scope)

- OMML math (LaTeX → Office Math XML) — tracked as priority upgrade
- `--reference-doc=FILE` (template DOCX for inheriting styles)
- Headers/footers
- Native Word Table of Contents
- Syntax highlighting in code blocks (colored runs)
- Track changes
- Page numbering
