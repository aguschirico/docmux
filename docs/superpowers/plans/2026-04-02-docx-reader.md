# DOCX Reader Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a general-purpose DOCX reader that parses Office Open XML files from any source (Word, Google Docs, LibreOffice) into the docmux AST, with a new `BinaryReader` trait for binary input formats.

**Architecture:** New `BinaryReader` trait in `docmux-core` for binary formats. `DocxReader` implements it by unzipping the archive, parsing XML parts with `quick-xml`, and walking `<w:body>` to produce AST nodes. A style classifier maps Word styles + formatting heuristics to semantic AST types. CLI and WASM gain binary input paths.

**Tech Stack:** Rust, `zip` crate (v2, already in workspace), `quick-xml` (new), Office Open XML (ECMA-376)

**Spec:** `docs/superpowers/specs/2026-04-02-docx-reader-design.md`

**Deferred to follow-up iterations:**
- List assembly (grouping consecutive `<w:p>` with `<w:numPr>` into `List` blocks)
- `<w:drawing>` / `<wp:inline>` → `Figure` parsing in document.rs
- DefinitionList heuristic (bold term + indented definition)
- Smart quote detection (`\u{201C}` → `Quoted`)
- Golden file test infrastructure with `.docx` fixtures
- `media.rs` as separate module (covered by `archive.get_bytes()` for now)

These features build on the skeleton established by this plan and can be added incrementally.

---

### Task 1: Add `BinaryReader` trait to `docmux-core`

**Files:**
- Modify: `crates/docmux-core/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add at the bottom of `crates/docmux-core/src/lib.rs`, inside `mod tests`:

```rust
#[test]
fn binary_reader_trait_exists() {
    // Verify BinaryReader is a usable trait with Send + Sync bounds
    fn assert_binary_reader<T: super::BinaryReader + Send + Sync>() {}
    // If this compiles, the trait exists with the right bounds
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p docmux-core -- binary_reader_trait_exists`
Expected: FAIL — `BinaryReader` not found

- [ ] **Step 3: Add the `BinaryReader` trait**

In `crates/docmux-core/src/lib.rs`, add after the `Reader` trait (after line 53):

```rust
// ─── BinaryReader trait ─────────────────────────────────────────────────

/// Parses binary input (e.g. a ZIP archive) into a [`Document`] AST.
pub trait BinaryReader: Send + Sync {
    /// A human-readable format name (e.g. `"docx"`).
    fn format(&self) -> &str;

    /// File extensions this reader handles (e.g. `["docx"]`).
    fn extensions(&self) -> &[&str];

    /// Parse binary `input` into a document AST.
    fn read_bytes(&self, input: &[u8]) -> Result<Document>;
}
```

- [ ] **Step 4: Add `BinaryReader` support to `Registry`**

In `crates/docmux-core/src/lib.rs`, modify the `Registry` struct to add a `binary_readers` field:

```rust
#[derive(Default)]
pub struct Registry {
    readers: Vec<Box<dyn Reader>>,
    binary_readers: Vec<Box<dyn BinaryReader>>,
    writers: Vec<Box<dyn Writer>>,
}
```

Add these methods to `impl Registry`:

```rust
/// Register a binary reader.
pub fn add_binary_reader(&mut self, reader: Box<dyn BinaryReader>) {
    self.binary_readers.push(reader);
}

/// Look up a binary reader by format name or file extension.
pub fn find_binary_reader(&self, name_or_ext: &str) -> Option<&dyn BinaryReader> {
    let needle = name_or_ext.trim_start_matches('.');
    self.binary_readers
        .iter()
        .find(|r| r.format() == needle || r.extensions().contains(&needle))
        .map(|r| r.as_ref())
}

/// List available binary reader format names.
pub fn binary_reader_formats(&self) -> Vec<&str> {
    self.binary_readers.iter().map(|r| r.format()).collect()
}
```

Modify `reader_formats()` to include binary reader formats:

```rust
/// List all available reader format names (text and binary).
pub fn reader_formats(&self) -> Vec<&str> {
    self.readers
        .iter()
        .map(|r| r.format())
        .chain(self.binary_readers.iter().map(|r| r.format()))
        .collect()
}
```

- [ ] **Step 5: Add Registry test for binary readers**

Add inside `mod tests`:

```rust
#[test]
fn registry_binary_reader() {
    use super::*;

    struct FakeBinaryReader;
    impl BinaryReader for FakeBinaryReader {
        fn format(&self) -> &str { "fake" }
        fn extensions(&self) -> &[&str] { &["fk"] }
        fn read_bytes(&self, _input: &[u8]) -> Result<Document> {
            Ok(Document::default())
        }
    }

    let mut reg = Registry::new();
    assert!(reg.find_binary_reader("fake").is_none());

    reg.add_binary_reader(Box::new(FakeBinaryReader));
    assert!(reg.find_binary_reader("fake").is_some());
    assert!(reg.find_binary_reader("fk").is_some());
    assert!(reg.reader_formats().contains(&"fake"));
}
```

- [ ] **Step 6: Run all tests**

Run: `cargo test -p docmux-core`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-core/src/lib.rs
git commit -m "feat(core): add BinaryReader trait and Registry support"
```

---

### Task 2: Scaffold `docmux-reader-docx` crate with archive module

**Files:**
- Create: `crates/docmux-reader-docx/Cargo.toml`
- Create: `crates/docmux-reader-docx/src/lib.rs`
- Create: `crates/docmux-reader-docx/src/archive.rs`
- Modify: `Cargo.toml` (workspace root — add member + workspace dep)

- [ ] **Step 1: Add `quick-xml` to workspace dependencies**

In root `Cargo.toml`, add after the `zip` line:

```toml
quick-xml = "0.37"
```

- [ ] **Step 2: Add workspace member and dependency**

In root `Cargo.toml`, add `"crates/docmux-reader-docx"` to `[workspace] members` (after `docmux-reader-html`).

Add to `[workspace.dependencies]`:

```toml
docmux-reader-docx = { path = "crates/docmux-reader-docx" }
```

- [ ] **Step 3: Create `crates/docmux-reader-docx/Cargo.toml`**

```toml
[package]
name = "docmux-reader-docx"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "DOCX reader for docmux — universal document converter"
rust-version.workspace = true

[dependencies]
docmux-ast = { workspace = true }
docmux-core = { workspace = true }
zip = { workspace = true }
quick-xml = { workspace = true }

[dev-dependencies]
docmux-writer-docx = { workspace = true }
docmux-reader-markdown = { workspace = true }
```

- [ ] **Step 4: Create `archive.rs` with tests**

Create `crates/docmux-reader-docx/src/archive.rs`:

```rust
//! ZIP archive extraction for DOCX files.

use docmux_core::{ConvertError, Result};
use std::collections::HashMap;
use std::io::{Cursor, Read};
use zip::ZipArchive;

/// Extracted contents of a DOCX ZIP archive.
pub struct DocxArchive {
    /// Raw XML/binary content keyed by path inside the ZIP.
    parts: HashMap<String, Vec<u8>>,
}

impl DocxArchive {
    /// Open a DOCX archive from raw bytes.
    pub fn open(input: &[u8]) -> Result<Self> {
        let cursor = Cursor::new(input);
        let mut archive = ZipArchive::new(cursor).map_err(|e| {
            ConvertError::Parse {
                line: 0,
                col: 0,
                message: format!("invalid DOCX archive: {e}"),
            }
        })?;

        let mut parts = HashMap::new();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| {
                ConvertError::Other(format!("zip entry error: {e}"))
            })?;
            let name = file.name().to_string();
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map_err(ConvertError::Io)?;
            parts.insert(name, buf);
        }

        Ok(Self { parts })
    }

    /// Get a part as a UTF-8 string, or `None` if not found.
    pub fn get_xml(&self, path: &str) -> Option<&str> {
        self.parts.get(path).and_then(|b| std::str::from_utf8(b).ok())
    }

    /// Get a part as raw bytes, or `None` if not found.
    pub fn get_bytes(&self, path: &str) -> Option<&[u8]> {
        self.parts.get(path).map(|b| b.as_slice())
    }

    /// List all media file paths (e.g. `word/media/image1.png`).
    pub fn media_paths(&self) -> Vec<&str> {
        self.parts
            .keys()
            .filter(|k| k.starts_with("word/media/"))
            .map(|k| k.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    /// Build a minimal valid DOCX ZIP for testing.
    fn build_test_zip(entries: &[(&str, &str)]) -> Vec<u8> {
        let buf = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        for (name, content) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(content.as_bytes()).unwrap();
        }

        zip.finish().unwrap().into_inner()
    }

    #[test]
    fn open_valid_zip() {
        let bytes = build_test_zip(&[
            ("word/document.xml", "<w:document/>"),
            ("word/styles.xml", "<w:styles/>"),
        ]);
        let archive = DocxArchive::open(&bytes).unwrap();
        assert_eq!(archive.get_xml("word/document.xml"), Some("<w:document/>"));
        assert_eq!(archive.get_xml("word/styles.xml"), Some("<w:styles/>"));
        assert!(archive.get_xml("word/missing.xml").is_none());
    }

    #[test]
    fn open_invalid_bytes() {
        let result = DocxArchive::open(b"not a zip file");
        assert!(result.is_err());
    }

    #[test]
    fn media_paths() {
        let bytes = build_test_zip(&[
            ("word/document.xml", ""),
            ("word/media/image1.png", "PNG"),
            ("word/media/image2.jpeg", "JPEG"),
        ]);
        let archive = DocxArchive::open(&bytes).unwrap();
        let mut paths = archive.media_paths();
        paths.sort();
        assert_eq!(paths, vec!["word/media/image1.png", "word/media/image2.jpeg"]);
    }
}
```

- [ ] **Step 5: Create stub `lib.rs`**

Create `crates/docmux-reader-docx/src/lib.rs`:

```rust
//! # docmux-reader-docx
//!
//! DOCX (Office Open XML) reader for docmux. Parses `.docx` files from
//! any source (Word, Google Docs, LibreOffice) into the docmux AST.

use docmux_ast::Document;
use docmux_core::{BinaryReader, Result};

mod archive;

/// A DOCX reader.
#[derive(Debug, Default)]
pub struct DocxReader;

impl DocxReader {
    pub fn new() -> Self {
        Self
    }
}

impl BinaryReader for DocxReader {
    fn format(&self) -> &str {
        "docx"
    }

    fn extensions(&self) -> &[&str] {
        &["docx"]
    }

    fn read_bytes(&self, input: &[u8]) -> Result<Document> {
        let _archive = archive::DocxArchive::open(input)?;
        // Stub: return empty document for now
        Ok(Document::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_impl_exists() {
        let reader = DocxReader::new();
        assert_eq!(reader.format(), "docx");
        assert_eq!(reader.extensions(), &["docx"]);
    }
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p docmux-reader-docx`
Expected: All PASS (archive tests + trait test)

- [ ] **Step 7: Run clippy**

Run: `cargo clippy -p docmux-reader-docx -- -D warnings`
Expected: No warnings

- [ ] **Step 8: Commit**

```bash
git add crates/docmux-reader-docx/ Cargo.toml
git commit -m "feat(docx): scaffold docx reader crate with archive module"
```

---

### Task 3: Relationships parser

**Files:**
- Create: `crates/docmux-reader-docx/src/relationships.rs`
- Modify: `crates/docmux-reader-docx/src/lib.rs` (add `mod relationships;`)

- [ ] **Step 1: Write the test**

Create `crates/docmux-reader-docx/src/relationships.rs`:

```rust
//! Parse OOXML relationship files (`_rels/*.rels`, `word/_rels/document.xml.rels`).

use docmux_core::{ConvertError, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

/// A parsed relationship: rId → (type, target).
#[derive(Debug, Clone)]
pub struct Relationship {
    pub rel_type: String,
    pub target: String,
    pub target_mode: Option<String>,
}

/// Map of rId → Relationship.
pub type RelMap = HashMap<String, Relationship>;

/// Parse an OOXML `.rels` XML file into a map of rId → Relationship.
pub fn parse_relationships(xml: &str) -> Result<RelMap> {
    let mut reader = Reader::from_str(xml);
    let mut map = HashMap::new();

    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e)) | Ok(Event::Start(ref e)) => {
                if e.local_name().as_ref() == b"Relationship" {
                    let mut id = None;
                    let mut rel_type = None;
                    let mut target = None;
                    let mut target_mode = None;

                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"Id" => id = Some(String::from_utf8_lossy(&attr.value).to_string()),
                            b"Type" => {
                                rel_type =
                                    Some(String::from_utf8_lossy(&attr.value).to_string())
                            }
                            b"Target" => {
                                target = Some(String::from_utf8_lossy(&attr.value).to_string())
                            }
                            b"TargetMode" => {
                                target_mode =
                                    Some(String::from_utf8_lossy(&attr.value).to_string())
                            }
                            _ => {}
                        }
                    }

                    if let (Some(id), Some(rel_type), Some(target)) = (id, rel_type, target) {
                        map.insert(
                            id,
                            Relationship {
                                rel_type,
                                target,
                                target_mode,
                            },
                        );
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ConvertError::Parse {
                    line: 0,
                    col: 0,
                    message: format!("XML parse error in .rels: {e}"),
                });
            }
            _ => {}
        }
    }

    Ok(map)
}

/// Check if a relationship type string indicates a hyperlink.
pub fn is_hyperlink(rel_type: &str) -> bool {
    rel_type.contains("hyperlink")
}

/// Check if a relationship type string indicates an image.
pub fn is_image(rel_type: &str) -> bool {
    rel_type.contains("image")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_document_rels() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rIdStyles" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="https://example.com" TargetMode="External"/>
<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
</Relationships>"#;

        let rels = parse_relationships(xml).unwrap();
        assert_eq!(rels.len(), 3);

        let link = &rels["rId1"];
        assert!(is_hyperlink(&link.rel_type));
        assert_eq!(link.target, "https://example.com");
        assert_eq!(link.target_mode.as_deref(), Some("External"));

        let img = &rels["rId2"];
        assert!(is_image(&img.rel_type));
        assert_eq!(img.target, "media/image1.png");
    }

    #[test]
    fn parse_empty_rels() {
        let xml = r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#;
        let rels = parse_relationships(xml).unwrap();
        assert!(rels.is_empty());
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/docmux-reader-docx/src/lib.rs`, add after `mod archive;`:

```rust
mod relationships;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p docmux-reader-docx`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-reader-docx/src/relationships.rs crates/docmux-reader-docx/src/lib.rs
git commit -m "feat(docx): relationships parser for hyperlinks and images"
```

---

### Task 4: Styles parser and classifier

**Files:**
- Create: `crates/docmux-reader-docx/src/styles.rs`
- Modify: `crates/docmux-reader-docx/src/lib.rs` (add `mod styles;`)

- [ ] **Step 1: Create the styles module**

Create `crates/docmux-reader-docx/src/styles.rs`:

```rust
//! Parse `word/styles.xml` and classify paragraph styles to AST block types.

use docmux_core::{ConvertError, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

/// What AST block type a paragraph style maps to.
#[derive(Debug, Clone, PartialEq)]
pub enum StyleKind {
    Heading(u8),
    CodeBlock,
    BlockQuote,
    MathBlock,
    Caption,
    Title,
    Author,
    Date,
    Abstract,
    Normal,
}

/// Parsed style information.
#[derive(Debug, Clone)]
pub struct StyleInfo {
    pub style_id: String,
    pub name: String,
    pub based_on: Option<String>,
    pub style_type: String,
    pub kind: StyleKind,
}

/// A map of styleId → StyleInfo.
pub type StyleMap = HashMap<String, StyleInfo>;

/// Parse `styles.xml` and build a style map.
pub fn parse_styles(xml: &str) -> Result<StyleMap> {
    let mut reader = Reader::from_str(xml);
    let mut map = HashMap::new();

    let mut in_style = false;
    let mut current_id = String::new();
    let mut current_name = String::new();
    let mut current_based_on: Option<String> = None;
    let mut current_type = String::new();
    let mut current_outline_lvl: Option<u8> = None;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"style" => {
                        in_style = true;
                        current_id.clear();
                        current_name.clear();
                        current_based_on = None;
                        current_type.clear();
                        current_outline_lvl = None;

                        for attr in e.attributes().flatten() {
                            match attr.key.local_name().as_ref() {
                                b"styleId" => {
                                    current_id =
                                        String::from_utf8_lossy(&attr.value).to_string();
                                }
                                b"type" => {
                                    current_type =
                                        String::from_utf8_lossy(&attr.value).to_string();
                                }
                                _ => {}
                            }
                        }
                    }
                    b"name" if in_style => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                current_name =
                                    String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    b"basedOn" if in_style => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                current_based_on = Some(
                                    String::from_utf8_lossy(&attr.value).to_string(),
                                );
                            }
                        }
                    }
                    b"outlineLvl" if in_style => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                let val = String::from_utf8_lossy(&attr.value);
                                current_outline_lvl = val.parse::<u8>().ok();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().as_ref() == b"style" && in_style {
                    let kind = classify_style(
                        &current_id,
                        &current_name,
                        current_outline_lvl,
                    );
                    map.insert(
                        current_id.clone(),
                        StyleInfo {
                            style_id: current_id.clone(),
                            name: current_name.clone(),
                            based_on: current_based_on.clone(),
                            style_type: current_type.clone(),
                            kind,
                        },
                    );
                    in_style = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ConvertError::Parse {
                    line: 0,
                    col: 0,
                    message: format!("XML parse error in styles.xml: {e}"),
                });
            }
            _ => {}
        }
    }

    Ok(map)
}

/// Classify a style based on its ID, name, and outline level.
fn classify_style(id: &str, name: &str, outline_lvl: Option<u8>) -> StyleKind {
    let id_lower = id.to_lowercase();
    let name_lower = name.to_lowercase();

    // Exact ID matches (Word, docmux writer)
    match id {
        "Title" => return StyleKind::Title,
        "Author" => return StyleKind::Author,
        "Date" => return StyleKind::Date,
        "Abstract" => return StyleKind::Abstract,
        "CodeBlock" => return StyleKind::CodeBlock,
        "BlockQuote" => return StyleKind::BlockQuote,
        "MathBlock" => return StyleKind::MathBlock,
        "Caption" => return StyleKind::Caption,
        _ => {}
    }

    // Heading by style ID: Heading1..Heading6, Titre1 (fr), Überschrift1 (de)
    if let Some(level) = extract_heading_level_from_id(&id_lower) {
        return StyleKind::Heading(level);
    }

    // Heading by outline level (Word internal)
    if let Some(lvl) = outline_lvl {
        if lvl < 6 {
            return StyleKind::Heading(lvl + 1);
        }
    }

    // Name-based patterns
    if name_lower.contains("heading") || name_lower.contains("título")
        || name_lower.contains("titre") || name_lower.contains("überschrift")
    {
        if let Some(level) = extract_trailing_digit(&name_lower) {
            return StyleKind::Heading(level);
        }
    }

    if name_lower == "title" || name_lower == "título" || name_lower == "titre" {
        return StyleKind::Title;
    }

    if name_lower.contains("code") || name_lower.contains("source code")
        || name_lower.contains("preformatted")
    {
        return StyleKind::CodeBlock;
    }

    if name_lower.contains("quote") || name_lower.contains("cita")
        || name_lower.contains("citation")
    {
        return StyleKind::BlockQuote;
    }

    if name_lower.contains("abstract") || name_lower.contains("resumen") {
        return StyleKind::Abstract;
    }

    if name_lower.contains("caption") || name_lower.contains("leyenda") {
        return StyleKind::Caption;
    }

    StyleKind::Normal
}

/// Extract heading level from style IDs like "Heading1", "heading2", "titre3".
fn extract_heading_level_from_id(id_lower: &str) -> Option<u8> {
    for prefix in &["heading", "titre", "überschrift", "título", "titolo"] {
        if let Some(rest) = id_lower.strip_prefix(prefix) {
            if let Ok(n) = rest.parse::<u8>() {
                if (1..=6).contains(&n) {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// Extract trailing digit from a string, e.g. "heading 2" → 2.
fn extract_trailing_digit(s: &str) -> Option<u8> {
    s.chars()
        .last()
        .and_then(|c| c.to_digit(10))
        .and_then(|n| {
            if (1..=6).contains(&n) {
                Some(n as u8)
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_docmux_styles() {
        assert_eq!(classify_style("Heading1", "heading 1", Some(0)), StyleKind::Heading(1));
        assert_eq!(classify_style("Heading3", "heading 3", Some(2)), StyleKind::Heading(3));
        assert_eq!(classify_style("Title", "Title", None), StyleKind::Title);
        assert_eq!(classify_style("CodeBlock", "Code Block", None), StyleKind::CodeBlock);
        assert_eq!(classify_style("BlockQuote", "Block Quote", None), StyleKind::BlockQuote);
        assert_eq!(classify_style("MathBlock", "Math Block", None), StyleKind::MathBlock);
        assert_eq!(classify_style("Abstract", "Abstract", None), StyleKind::Abstract);
        assert_eq!(classify_style("Caption", "caption", None), StyleKind::Caption);
        assert_eq!(classify_style("Author", "Author", None), StyleKind::Author);
        assert_eq!(classify_style("Date", "Date", None), StyleKind::Date);
    }

    #[test]
    fn classify_by_outline_level() {
        assert_eq!(classify_style("CustomStyle", "My Style", Some(0)), StyleKind::Heading(1));
        assert_eq!(classify_style("CustomStyle", "My Style", Some(2)), StyleKind::Heading(3));
    }

    #[test]
    fn classify_by_name_pattern() {
        assert_eq!(classify_style("Custom1", "Source Code", None), StyleKind::CodeBlock);
        assert_eq!(classify_style("Custom2", "Block Quote", None), StyleKind::BlockQuote);
        assert_eq!(classify_style("Custom3", "Preformatted Text", None), StyleKind::CodeBlock);
    }

    #[test]
    fn classify_i18n_styles() {
        // French
        assert_eq!(classify_style("Titre1", "Titre 1", None), StyleKind::Heading(1));
        // Spanish
        assert_eq!(classify_style("Título2", "Título 2", None), StyleKind::Heading(2));
    }

    #[test]
    fn classify_unknown_as_normal() {
        assert_eq!(classify_style("ListParagraph", "List Paragraph", None), StyleKind::Normal);
        assert_eq!(classify_style("BodyText", "Body Text", None), StyleKind::Normal);
    }

    #[test]
    fn parse_styles_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Normal">
    <w:name w:val="Normal"/>
  </w:style>
</w:styles>"#;

        let styles = parse_styles(xml).unwrap();
        assert_eq!(styles.len(), 2);

        let h1 = &styles["Heading1"];
        assert_eq!(h1.kind, StyleKind::Heading(1));
        assert_eq!(h1.based_on.as_deref(), Some("Normal"));

        let normal = &styles["Normal"];
        assert_eq!(normal.kind, StyleKind::Normal);
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/docmux-reader-docx/src/lib.rs`, add after `mod relationships;`:

```rust
mod styles;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p docmux-reader-docx`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-reader-docx/src/styles.rs crates/docmux-reader-docx/src/lib.rs
git commit -m "feat(docx): style parser with i18n heading classification"
```

---

### Task 5: Numbering parser

**Files:**
- Create: `crates/docmux-reader-docx/src/numbering.rs`
- Modify: `crates/docmux-reader-docx/src/lib.rs` (add `mod numbering;`)

- [ ] **Step 1: Create the numbering module**

Create `crates/docmux-reader-docx/src/numbering.rs`:

```rust
//! Parse `word/numbering.xml` to resolve list numbering definitions.

use docmux_ast::ListStyle;
use docmux_core::{ConvertError, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

/// A resolved numbering definition for a given numId + indent level.
#[derive(Debug, Clone)]
pub struct NumberingInfo {
    pub ordered: bool,
    pub style: Option<ListStyle>,
}

/// Maps (numId, ilvl) → NumberingInfo.
pub type NumberingMap = HashMap<(u32, u32), NumberingInfo>;

/// Parse `numbering.xml` and build a lookup map.
pub fn parse_numbering(xml: &str) -> Result<NumberingMap> {
    let mut reader = Reader::from_str(xml);
    let mut map = NumberingMap::new();

    // First pass: parse abstractNum definitions
    // abstractNumId → Vec<(ilvl, numFmt)>
    let mut abstract_defs: HashMap<u32, Vec<(u32, String)>> = HashMap::new();
    // numId → abstractNumId
    let mut num_to_abstract: HashMap<u32, u32> = HashMap::new();

    let mut in_abstract_num = false;
    let mut current_abstract_id: u32 = 0;
    let mut in_lvl = false;
    let mut current_ilvl: u32 = 0;
    let mut current_num_fmt = String::new();
    let mut in_num = false;
    let mut current_num_id: u32 = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"abstractNum" => {
                        in_abstract_num = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"abstractNumId" {
                                let val = String::from_utf8_lossy(&attr.value);
                                current_abstract_id = val.parse().unwrap_or(0);
                            }
                        }
                    }
                    b"lvl" if in_abstract_num => {
                        in_lvl = true;
                        current_num_fmt.clear();
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"ilvl" {
                                let val = String::from_utf8_lossy(&attr.value);
                                current_ilvl = val.parse().unwrap_or(0);
                            }
                        }
                    }
                    b"numFmt" if in_lvl => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                current_num_fmt =
                                    String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    b"num" if !in_abstract_num => {
                        in_num = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"numId" {
                                let val = String::from_utf8_lossy(&attr.value);
                                current_num_id = val.parse().unwrap_or(0);
                            }
                        }
                    }
                    b"abstractNumId" if in_num => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                let val = String::from_utf8_lossy(&attr.value);
                                num_to_abstract.insert(
                                    current_num_id,
                                    val.parse().unwrap_or(0),
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"lvl" if in_lvl => {
                        abstract_defs
                            .entry(current_abstract_id)
                            .or_default()
                            .push((current_ilvl, current_num_fmt.clone()));
                        in_lvl = false;
                    }
                    b"abstractNum" => in_abstract_num = false,
                    b"num" => in_num = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ConvertError::Parse {
                    line: 0,
                    col: 0,
                    message: format!("XML parse error in numbering.xml: {e}"),
                });
            }
            _ => {}
        }
    }

    // Resolve: for each numId, look up its abstractNum levels
    for (num_id, abstract_id) in &num_to_abstract {
        if let Some(levels) = abstract_defs.get(abstract_id) {
            for (ilvl, fmt) in levels {
                let (ordered, style) = num_fmt_to_list_style(fmt);
                map.insert((*num_id, *ilvl), NumberingInfo { ordered, style });
            }
        }
    }

    Ok(map)
}

/// Convert a `w:numFmt` value to (ordered, ListStyle).
fn num_fmt_to_list_style(fmt: &str) -> (bool, Option<ListStyle>) {
    match fmt {
        "bullet" => (false, None),
        "decimal" => (true, Some(ListStyle::Decimal)),
        "lowerLetter" => (true, Some(ListStyle::LowerAlpha)),
        "upperLetter" => (true, Some(ListStyle::UpperAlpha)),
        "lowerRoman" => (true, Some(ListStyle::LowerRoman)),
        "upperRoman" => (true, Some(ListStyle::UpperRoman)),
        "none" => (false, None),
        // Default: treat unknown numbered formats as decimal ordered
        _ => (true, Some(ListStyle::Decimal)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_numbering_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:abstractNum w:abstractNumId="1">
  <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/></w:lvl>
  <w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/></w:lvl>
</w:abstractNum>
<w:abstractNum w:abstractNumId="2">
  <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/></w:lvl>
</w:abstractNum>
<w:num w:numId="1"><w:abstractNumId w:val="1"/></w:num>
<w:num w:numId="2"><w:abstractNumId w:val="2"/></w:num>
</w:numbering>"#;

        let map = parse_numbering(xml).unwrap();

        let info = &map[&(1, 0)];
        assert!(info.ordered);
        assert_eq!(info.style, Some(ListStyle::Decimal));

        let info = &map[&(1, 1)];
        assert!(info.ordered);
        assert_eq!(info.style, Some(ListStyle::LowerAlpha));

        let info = &map[&(2, 0)];
        assert!(!info.ordered);
        assert_eq!(info.style, None);
    }

    #[test]
    fn parse_empty_numbering() {
        let xml = r#"<?xml version="1.0"?>
<w:numbering xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#;
        let map = parse_numbering(xml).unwrap();
        assert!(map.is_empty());
    }

    #[test]
    fn num_fmt_mapping() {
        assert_eq!(num_fmt_to_list_style("bullet"), (false, None));
        assert_eq!(num_fmt_to_list_style("decimal"), (true, Some(ListStyle::Decimal)));
        assert_eq!(num_fmt_to_list_style("lowerRoman"), (true, Some(ListStyle::LowerRoman)));
        assert_eq!(num_fmt_to_list_style("upperLetter"), (true, Some(ListStyle::UpperAlpha)));
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/docmux-reader-docx/src/lib.rs`, add after `mod styles;`:

```rust
mod numbering;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p docmux-reader-docx`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-reader-docx/src/numbering.rs crates/docmux-reader-docx/src/lib.rs
git commit -m "feat(docx): numbering parser for list style resolution"
```

---

### Task 6: Footnotes parser

**Files:**
- Create: `crates/docmux-reader-docx/src/footnotes.rs`
- Modify: `crates/docmux-reader-docx/src/lib.rs` (add `mod footnotes;`)

- [ ] **Step 1: Create the footnotes module**

Create `crates/docmux-reader-docx/src/footnotes.rs`:

```rust
//! Parse `word/footnotes.xml` into footnote content blocks.

use docmux_ast::{Block, Inline};
use docmux_core::{ConvertError, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;

/// Map of footnote ID → text content (simplified: plain text for now,
/// will be enriched when document.rs inline parser is available).
pub type FootnoteMap = HashMap<String, Vec<Block>>;

/// Parse `footnotes.xml` and extract footnote content.
///
/// Skips special footnotes (separator, continuationSeparator) with IDs -1 and 0.
pub fn parse_footnotes(xml: &str) -> Result<FootnoteMap> {
    let mut reader = Reader::from_str(xml);
    let mut map = FootnoteMap::new();

    let mut in_footnote = false;
    let mut current_id = String::new();
    let mut skip_footnote = false;
    let mut text_buf = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"footnote" => {
                        in_footnote = true;
                        current_id.clear();
                        text_buf.clear();
                        skip_footnote = false;

                        for attr in e.attributes().flatten() {
                            match attr.key.local_name().as_ref() {
                                b"id" => {
                                    current_id =
                                        String::from_utf8_lossy(&attr.value).to_string();
                                }
                                b"type" => {
                                    let val = String::from_utf8_lossy(&attr.value);
                                    if val == "separator" || val == "continuationSeparator" {
                                        skip_footnote = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_footnote && !skip_footnote => {
                if let Ok(text) = e.unescape() {
                    text_buf.push_str(&text);
                }
            }
            Ok(Event::End(ref e)) => {
                if e.local_name().as_ref() == b"footnote" && in_footnote {
                    if !skip_footnote && !current_id.is_empty() {
                        let trimmed = text_buf.trim().to_string();
                        if !trimmed.is_empty() {
                            map.insert(
                                current_id.clone(),
                                vec![Block::Paragraph {
                                    content: vec![Inline::Text { value: trimmed }],
                                }],
                            );
                        }
                    }
                    in_footnote = false;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ConvertError::Parse {
                    line: 0,
                    col: 0,
                    message: format!("XML parse error in footnotes.xml: {e}"),
                });
            }
            _ => {}
        }
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_footnotes_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>
<w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote>
<w:footnote w:id="2"><w:p><w:pPr><w:pStyle w:val="FootnoteText"/></w:pPr><w:r><w:t>This is a footnote.</w:t></w:r></w:p></w:footnote>
<w:footnote w:id="3"><w:p><w:r><w:t>Another footnote.</w:t></w:r></w:p></w:footnote>
</w:footnotes>"#;

        let notes = parse_footnotes(xml).unwrap();
        assert_eq!(notes.len(), 2);
        assert!(notes.contains_key("2"));
        assert!(notes.contains_key("3"));
        // Separator and continuation separator are skipped
        assert!(!notes.contains_key("-1"));
        assert!(!notes.contains_key("0"));
    }

    #[test]
    fn parse_empty_footnotes() {
        let xml = r#"<?xml version="1.0"?>
<w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#;
        let notes = parse_footnotes(xml).unwrap();
        assert!(notes.is_empty());
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/docmux-reader-docx/src/lib.rs`, add after `mod numbering;`:

```rust
mod footnotes;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p docmux-reader-docx`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-reader-docx/src/footnotes.rs crates/docmux-reader-docx/src/lib.rs
git commit -m "feat(docx): footnotes parser with separator filtering"
```

---

### Task 7: Metadata parser (Dublin Core)

**Files:**
- Create: `crates/docmux-reader-docx/src/metadata.rs`
- Modify: `crates/docmux-reader-docx/src/lib.rs` (add `mod metadata;`)

- [ ] **Step 1: Create the metadata module**

Create `crates/docmux-reader-docx/src/metadata.rs`:

```rust
//! Parse `docProps/core.xml` (Dublin Core) for document metadata.

use docmux_ast::{Author, Metadata};
use docmux_core::{ConvertError, Result};
use quick_xml::events::Event;
use quick_xml::Reader;

/// Parse Dublin Core metadata from `docProps/core.xml`.
pub fn parse_core_properties(xml: &str) -> Result<Metadata> {
    let mut reader = Reader::from_str(xml);
    let mut metadata = Metadata::default();

    let mut current_element = String::new();
    let mut in_element = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = String::from_utf8_lossy(e.local_name().as_ref()).to_string();
                match local.as_str() {
                    "title" | "creator" | "created" | "subject" | "keywords"
                    | "description" => {
                        current_element = local;
                        in_element = true;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_element => {
                if let Ok(text) = e.unescape() {
                    let text = text.trim().to_string();
                    if !text.is_empty() {
                        match current_element.as_str() {
                            "title" => metadata.title = Some(text),
                            "creator" => {
                                // Multiple authors may be separated by ";"
                                for name in text.split(';') {
                                    let name = name.trim();
                                    if !name.is_empty() {
                                        metadata.authors.push(Author {
                                            name: name.to_string(),
                                            ..Default::default()
                                        });
                                    }
                                }
                            }
                            "created" => metadata.date = Some(text),
                            "subject" | "keywords" => {
                                for kw in text.split([',', ';']) {
                                    let kw = kw.trim();
                                    if !kw.is_empty() {
                                        metadata.keywords.push(kw.to_string());
                                    }
                                }
                            }
                            "description" => {
                                metadata.abstract_text = Some(vec![
                                    docmux_ast::Block::text(&text),
                                ]);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::End(_)) => {
                in_element = false;
                current_element.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ConvertError::Parse {
                    line: 0,
                    col: 0,
                    message: format!("XML parse error in core.xml: {e}"),
                });
            }
            _ => {}
        }
    }

    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_core_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
  xmlns:dc="http://purl.org/dc/elements/1.1/"
  xmlns:dcterms="http://purl.org/dc/terms/">
  <dc:title>My Document</dc:title>
  <dc:creator>Alice; Bob</dc:creator>
  <dcterms:created>2026-04-01</dcterms:created>
  <dc:subject>Rust, WASM, Documents</dc:subject>
  <dc:description>An abstract about this doc.</dc:description>
</cp:coreProperties>"#;

        let meta = parse_core_properties(xml).unwrap();
        assert_eq!(meta.title.as_deref(), Some("My Document"));
        assert_eq!(meta.authors.len(), 2);
        assert_eq!(meta.authors[0].name, "Alice");
        assert_eq!(meta.authors[1].name, "Bob");
        assert_eq!(meta.date.as_deref(), Some("2026-04-01"));
        assert_eq!(meta.keywords, vec!["Rust", "WASM", "Documents"]);
        assert!(meta.abstract_text.is_some());
    }

    #[test]
    fn parse_empty_core_xml() {
        let xml = r#"<?xml version="1.0"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"/>"#;
        let meta = parse_core_properties(xml).unwrap();
        assert!(meta.title.is_none());
        assert!(meta.authors.is_empty());
    }

    #[test]
    fn parse_keywords_with_semicolons() {
        let xml = r#"<?xml version="1.0"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties"
  xmlns:cp2="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <cp:keywords>alpha; beta; gamma</cp:keywords>
</cp:coreProperties>"#;
        let meta = parse_core_properties(xml).unwrap();
        assert_eq!(meta.keywords, vec!["alpha", "beta", "gamma"]);
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/docmux-reader-docx/src/lib.rs`, add after `mod footnotes;`:

```rust
mod metadata;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p docmux-reader-docx`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-reader-docx/src/metadata.rs crates/docmux-reader-docx/src/lib.rs
git commit -m "feat(docx): Dublin Core metadata parser"
```

---

### Task 8: Document body parser — inline runs

**Files:**
- Create: `crates/docmux-reader-docx/src/document.rs`
- Modify: `crates/docmux-reader-docx/src/lib.rs` (add `mod document;`)

This is the largest module. We build it in two tasks: inlines first (this task), then blocks (next task).

- [ ] **Step 1: Create `document.rs` with inline parsing**

Create `crates/docmux-reader-docx/src/document.rs`:

```rust
//! Parse `word/document.xml` — the main body of a DOCX file.

use crate::numbering::{NumberingInfo, NumberingMap};
use crate::relationships::{self, RelMap};
use crate::styles::{StyleKind, StyleMap};
use docmux_ast::*;
use docmux_core::{ConvertError, Result};
use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// Context needed while parsing the document body.
pub struct ParseContext<'a> {
    pub styles: &'a StyleMap,
    pub numbering: &'a NumberingMap,
    pub rels: &'a RelMap,
}

/// Parse `<w:body>` content into AST blocks.
pub fn parse_body(xml: &str, ctx: &ParseContext<'_>) -> Result<Vec<Block>> {
    let mut reader = Reader::from_str(xml);
    let mut blocks = Vec::new();
    let mut depth = 0;
    let mut in_body = false;

    // Buffer for collecting the raw XML of individual elements inside <w:body>
    let mut element_buf = String::new();
    let mut element_depth: i32 = 0;
    let mut collecting = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                if local.as_ref() == b"body" {
                    in_body = true;
                    continue;
                }
                if !in_body {
                    continue;
                }
                depth += 1;
                if depth == 1 {
                    // Top-level element inside <w:body>
                    collecting = true;
                    element_buf.clear();
                    element_depth = 0;
                }
                if collecting {
                    element_depth += 1;
                    append_start_tag(e, &mut element_buf);
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                if local.as_ref() == b"body" {
                    in_body = false;
                    continue;
                }
                if !in_body {
                    continue;
                }
                if collecting {
                    element_depth -= 1;
                    append_end_tag(e, &mut element_buf);
                    if element_depth == 0 {
                        // We have a complete top-level element
                        collecting = false;
                        let tag = e.local_name();
                        match tag.as_ref() {
                            b"p" => {
                                if let Some(block) = parse_paragraph(&element_buf, ctx) {
                                    blocks.push(block);
                                }
                            }
                            b"tbl" => {
                                blocks.push(parse_table(&element_buf, ctx));
                            }
                            _ => {
                                // Skip sectPr, bookmarkStart, etc.
                            }
                        }
                    }
                }
                depth -= 1;
            }
            Ok(Event::Empty(ref e)) => {
                if collecting {
                    append_empty_tag(e, &mut element_buf);
                }
            }
            Ok(Event::Text(ref e)) => {
                if collecting {
                    if let Ok(text) = e.unescape() {
                        element_buf.push_str(&quick_xml_escape(&text));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ConvertError::Parse {
                    line: 0,
                    col: 0,
                    message: format!("XML parse error in document.xml: {e}"),
                });
            }
            _ => {}
        }
    }

    Ok(blocks)
}

// ─── Inline (run) parsing ───────────────────────────────────────────────

/// Parse inline runs from a paragraph's raw XML.
pub fn parse_runs(xml: &str, ctx: &ParseContext<'_>) -> Vec<Inline> {
    let mut reader = Reader::from_str(xml);
    let mut inlines = Vec::new();

    // Run state
    let mut in_run = false;
    let mut bold = false;
    let mut italic = false;
    let mut strike = false;
    let mut underline = false;
    let mut superscript = false;
    let mut subscript = false;
    let mut small_caps = false;
    let mut is_code_font = false;
    let mut in_hyperlink = false;
    let mut hyperlink_rel_id = String::new();
    let mut in_footnote_ref = false;
    let mut footnote_ref_id = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"r" => {
                        in_run = true;
                        bold = false;
                        italic = false;
                        strike = false;
                        underline = false;
                        superscript = false;
                        subscript = false;
                        small_caps = false;
                        is_code_font = false;
                    }
                    b"hyperlink" => {
                        in_hyperlink = true;
                        hyperlink_rel_id.clear();
                        for attr in e.attributes().flatten() {
                            // r:id attribute (may have namespace prefix)
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            if key == "r:id" || key.ends_with(":id") {
                                hyperlink_rel_id =
                                    String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"b" if in_run => bold = true,
                    b"i" if in_run => italic = true,
                    b"strike" if in_run => strike = true,
                    b"smallCaps" if in_run => small_caps = true,
                    b"u" if in_run => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                let val = String::from_utf8_lossy(&attr.value);
                                if val != "none" {
                                    underline = true;
                                }
                            }
                        }
                    }
                    b"vertAlign" if in_run => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                match attr.value.as_ref() {
                                    b"superscript" => superscript = true,
                                    b"subscript" => subscript = true,
                                    _ => {}
                                }
                            }
                        }
                    }
                    b"rFonts" if in_run => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"ascii" {
                                let val = String::from_utf8_lossy(&attr.value);
                                if is_monospace_font(&val) {
                                    is_code_font = true;
                                }
                            }
                        }
                    }
                    b"br" if in_run => {
                        inlines.push(Inline::HardBreak);
                    }
                    b"footnoteReference" if in_run => {
                        in_footnote_ref = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"id" {
                                footnote_ref_id =
                                    String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        if !footnote_ref_id.is_empty() {
                            inlines.push(Inline::FootnoteRef {
                                id: footnote_ref_id.clone(),
                            });
                        }
                        in_footnote_ref = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_run => {
                if let Ok(text) = e.unescape() {
                    let text = text.to_string();
                    if text.is_empty() {
                        continue;
                    }

                    let base = if is_code_font {
                        Inline::Code {
                            value: text.clone(),
                            attrs: None,
                        }
                    } else {
                        Inline::Text {
                            value: text.clone(),
                        }
                    };

                    // Wrap in formatting inlines from innermost to outermost
                    let wrapped = wrap_inline(
                        base, bold, italic, strike, underline, superscript, subscript,
                        small_caps,
                    );

                    if in_hyperlink && !hyperlink_rel_id.is_empty() {
                        // Resolve hyperlink URL from relationships
                        let url = ctx
                            .rels
                            .get(&hyperlink_rel_id)
                            .filter(|r| relationships::is_hyperlink(&r.rel_type))
                            .map(|r| r.target.clone())
                            .unwrap_or_default();

                        if !url.is_empty() {
                            inlines.push(Inline::Link {
                                url,
                                title: None,
                                content: vec![wrapped],
                                attrs: None,
                            });
                        } else {
                            inlines.push(wrapped);
                        }
                    } else {
                        inlines.push(wrapped);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"r" => in_run = false,
                    b"hyperlink" => in_hyperlink = false,
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
    }

    inlines
}

/// Wrap a base inline with formatting layers.
fn wrap_inline(
    base: Inline,
    bold: bool,
    italic: bool,
    strike: bool,
    underline: bool,
    superscript: bool,
    subscript: bool,
    small_caps: bool,
) -> Inline {
    let mut result = base;
    if small_caps {
        result = Inline::SmallCaps {
            content: vec![result],
        };
    }
    if subscript {
        result = Inline::Subscript {
            content: vec![result],
        };
    }
    if superscript {
        result = Inline::Superscript {
            content: vec![result],
        };
    }
    if underline {
        result = Inline::Underline {
            content: vec![result],
        };
    }
    if strike {
        result = Inline::Strikethrough {
            content: vec![result],
        };
    }
    if italic {
        result = Inline::Emphasis {
            content: vec![result],
        };
    }
    if bold {
        result = Inline::Strong {
            content: vec![result],
        };
    }
    result
}

/// Check if a font name is monospace.
fn is_monospace_font(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("courier")
        || lower.contains("consolas")
        || lower.contains("mono")
        || lower.contains("menlo")
        || lower.contains("lucida console")
        || lower.contains("source code")
        || lower.contains("fira code")
        || lower.contains("jetbrains")
        || lower.contains("inconsolata")
}

// ─── Paragraph and table parsing (stubs — completed in Task 9) ──────

fn parse_paragraph(_xml: &str, _ctx: &ParseContext<'_>) -> Option<Block> {
    // Stub — will be implemented in Task 9
    None
}

fn parse_table(_xml: &str, _ctx: &ParseContext<'_>) -> Block {
    // Stub — will be implemented in Task 9
    Block::ThematicBreak
}

// ─── XML reconstruction helpers ─────────────────────────────────────

fn append_start_tag(e: &BytesStart<'_>, buf: &mut String) {
    buf.push('<');
    buf.push_str(&String::from_utf8_lossy(e.name().as_ref()));
    for attr in e.attributes().flatten() {
        buf.push(' ');
        buf.push_str(&String::from_utf8_lossy(attr.key.as_ref()));
        buf.push_str("=\"");
        buf.push_str(&String::from_utf8_lossy(&attr.value));
        buf.push('"');
    }
    buf.push('>');
}

fn append_end_tag(e: &quick_xml::events::BytesEnd<'_>, buf: &mut String) {
    buf.push_str("</");
    buf.push_str(&String::from_utf8_lossy(e.name().as_ref()));
    buf.push('>');
}

fn append_empty_tag(e: &BytesStart<'_>, buf: &mut String) {
    buf.push('<');
    buf.push_str(&String::from_utf8_lossy(e.name().as_ref()));
    for attr in e.attributes().flatten() {
        buf.push(' ');
        buf.push_str(&String::from_utf8_lossy(attr.key.as_ref()));
        buf.push_str("=\"");
        buf.push_str(&String::from_utf8_lossy(&attr.value));
        buf.push('"');
    }
    buf.push_str("/>");
}

fn quick_xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::numbering::NumberingMap;
    use crate::relationships::RelMap;
    use crate::styles::StyleMap;
    use std::collections::HashMap;

    fn empty_ctx() -> ParseContext<'static> {
        // Leaked for test convenience — these are small and test-only
        let styles: &'static StyleMap = Box::leak(Box::new(HashMap::new()));
        let numbering: &'static NumberingMap = Box::leak(Box::new(HashMap::new()));
        let rels: &'static RelMap = Box::leak(Box::new(HashMap::new()));
        ParseContext {
            styles,
            numbering,
            rels,
        }
    }

    #[test]
    fn parse_plain_text_run() {
        let xml = r#"<w:r><w:t>Hello world</w:t></w:r>"#;
        let ctx = empty_ctx();
        let inlines = parse_runs(xml, &ctx);
        assert_eq!(inlines.len(), 1);
        match &inlines[0] {
            Inline::Text { value } => assert_eq!(value, "Hello world"),
            other => panic!("expected Text, got {other:?}"),
        }
    }

    #[test]
    fn parse_bold_run() {
        let xml = r#"<w:r><w:rPr><w:b/></w:rPr><w:t>Bold</w:t></w:r>"#;
        let ctx = empty_ctx();
        let inlines = parse_runs(xml, &ctx);
        assert_eq!(inlines.len(), 1);
        match &inlines[0] {
            Inline::Strong { content } => {
                assert_eq!(content.len(), 1);
                match &content[0] {
                    Inline::Text { value } => assert_eq!(value, "Bold"),
                    other => panic!("expected Text, got {other:?}"),
                }
            }
            other => panic!("expected Strong, got {other:?}"),
        }
    }

    #[test]
    fn parse_bold_italic_run() {
        let xml = r#"<w:r><w:rPr><w:b/><w:i/></w:rPr><w:t>Both</w:t></w:r>"#;
        let ctx = empty_ctx();
        let inlines = parse_runs(xml, &ctx);
        assert_eq!(inlines.len(), 1);
        // Should be Strong wrapping Emphasis
        match &inlines[0] {
            Inline::Strong { content } => match &content[0] {
                Inline::Emphasis { content } => match &content[0] {
                    Inline::Text { value } => assert_eq!(value, "Both"),
                    other => panic!("expected Text, got {other:?}"),
                },
                other => panic!("expected Emphasis, got {other:?}"),
            },
            other => panic!("expected Strong, got {other:?}"),
        }
    }

    #[test]
    fn parse_code_font_run() {
        let xml =
            r#"<w:r><w:rPr><w:rFonts w:ascii="Courier New"/><w:sz w:val="20"/></w:rPr><w:t>code</w:t></w:r>"#;
        let ctx = empty_ctx();
        let inlines = parse_runs(xml, &ctx);
        assert_eq!(inlines.len(), 1);
        match &inlines[0] {
            Inline::Code { value, .. } => assert_eq!(value, "code"),
            other => panic!("expected Code, got {other:?}"),
        }
    }

    #[test]
    fn parse_hyperlink() {
        let mut rels = RelMap::new();
        rels.insert(
            "rId1".to_string(),
            crate::relationships::Relationship {
                rel_type: "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink".to_string(),
                target: "https://example.com".to_string(),
                target_mode: Some("External".to_string()),
            },
        );
        let styles = StyleMap::new();
        let numbering = NumberingMap::new();
        let ctx = ParseContext {
            styles: &styles,
            numbering: &numbering,
            rels: &rels,
        };

        let xml = r#"<w:hyperlink r:id="rId1"><w:r><w:t>Click here</w:t></w:r></w:hyperlink>"#;
        let inlines = parse_runs(xml, &ctx);
        assert_eq!(inlines.len(), 1);
        match &inlines[0] {
            Inline::Link { url, content, .. } => {
                assert_eq!(url, "https://example.com");
                assert_eq!(content.len(), 1);
            }
            other => panic!("expected Link, got {other:?}"),
        }
    }

    #[test]
    fn parse_hard_break() {
        let xml = r#"<w:r><w:br/></w:r>"#;
        let ctx = empty_ctx();
        let inlines = parse_runs(xml, &ctx);
        assert_eq!(inlines.len(), 1);
        assert!(matches!(inlines[0], Inline::HardBreak));
    }

    #[test]
    fn parse_footnote_reference() {
        let xml = r#"<w:r><w:rPr><w:rStyle w:val="FootnoteReference"/></w:rPr><w:footnoteReference w:id="2"/></w:r>"#;
        let ctx = empty_ctx();
        let inlines = parse_runs(xml, &ctx);
        assert_eq!(inlines.len(), 1);
        match &inlines[0] {
            Inline::FootnoteRef { id } => assert_eq!(id, "2"),
            other => panic!("expected FootnoteRef, got {other:?}"),
        }
    }

    #[test]
    fn is_monospace_detects_common_fonts() {
        assert!(is_monospace_font("Courier New"));
        assert!(is_monospace_font("Consolas"));
        assert!(is_monospace_font("JetBrains Mono"));
        assert!(is_monospace_font("Source Code Pro"));
        assert!(!is_monospace_font("Calibri"));
        assert!(!is_monospace_font("Times New Roman"));
    }
}
```

- [ ] **Step 2: Register the module**

In `crates/docmux-reader-docx/src/lib.rs`, add after `mod metadata;`:

```rust
mod document;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p docmux-reader-docx`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-reader-docx/src/document.rs crates/docmux-reader-docx/src/lib.rs
git commit -m "feat(docx): inline run parser with formatting, links, footnotes"
```

---

### Task 9: Document body parser — blocks (paragraphs, tables, lists)

**Files:**
- Modify: `crates/docmux-reader-docx/src/document.rs` (replace stubs with real implementations)

- [ ] **Step 1: Implement `parse_paragraph`**

In `crates/docmux-reader-docx/src/document.rs`, replace the `parse_paragraph` stub with:

```rust
/// Parse a `<w:p>` element into an AST block.
fn parse_paragraph(xml: &str, ctx: &ParseContext<'_>) -> Option<Block> {
    let mut reader = Reader::from_str(xml);

    let mut style_id: Option<String> = None;
    let mut num_id: Option<u32> = None;
    let mut ilvl: u32 = 0;
    let mut has_bottom_border = false;
    let mut has_left_border = false;
    let mut left_border_color: Option<String> = None;

    // First pass: extract paragraph properties
    let mut in_ppr = false;
    let mut ppr_depth = 0;

    // We need to scan for pPr before parsing runs
    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"pPr" => {
                        in_ppr = true;
                        ppr_depth = 1;
                    }
                    _ if in_ppr => ppr_depth += 1,
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) if in_ppr => {
                let local = e.local_name();
                match local.as_ref() {
                    b"pStyle" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                style_id = Some(
                                    String::from_utf8_lossy(&attr.value).to_string(),
                                );
                            }
                        }
                    }
                    b"numId" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                let val = String::from_utf8_lossy(&attr.value);
                                num_id = val.parse().ok();
                            }
                        }
                    }
                    b"ilvl" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                let val = String::from_utf8_lossy(&attr.value);
                                ilvl = val.parse().unwrap_or(0);
                            }
                        }
                    }
                    b"bottom" => {
                        has_bottom_border = true;
                    }
                    b"left" => {
                        has_left_border = true;
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"color" {
                                left_border_color = Some(
                                    String::from_utf8_lossy(&attr.value).to_string(),
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) if in_ppr => {
                ppr_depth -= 1;
                if ppr_depth == 0 {
                    in_ppr = false;
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
    }

    // Parse inlines from the full paragraph XML
    let inlines = parse_runs(xml, ctx);

    // Classify the paragraph
    let kind = if let Some(ref sid) = style_id {
        ctx.styles
            .get(sid)
            .map(|s| s.kind.clone())
            .unwrap_or(StyleKind::Normal)
    } else {
        StyleKind::Normal
    };

    // Check for numbering (list item)
    if let Some(nid) = num_id {
        if nid > 0 {
            // This is a list item — return as a tagged paragraph for list assembly
            return Some(Block::Paragraph { content: inlines });
        }
    }

    // ThematicBreak: bottom border with no text content
    if has_bottom_border && inlines.is_empty() {
        return Some(Block::ThematicBreak);
    }

    // Admonition: left border with specific color
    if has_left_border {
        if let Some(ref color) = left_border_color {
            if color.eq_ignore_ascii_case("4472C4") {
                let text = extract_plain_text(&inlines);
                return Some(Block::Admonition {
                    kind: AdmonitionKind::Note,
                    title: Some(vec![Inline::Text { value: text }]),
                    content: Vec::new(),
                });
            }
        }
    }

    match kind {
        StyleKind::Heading(level) => Some(Block::Heading {
            level,
            id: None,
            content: inlines,
            attrs: None,
        }),
        StyleKind::Title => Some(Block::Heading {
            level: 1,
            id: None,
            content: inlines,
            attrs: None,
        }),
        StyleKind::CodeBlock => {
            let text = extract_plain_text(&inlines);
            Some(Block::CodeBlock {
                language: None,
                content: text,
                caption: None,
                label: None,
                attrs: None,
            })
        }
        StyleKind::MathBlock => {
            let text = extract_plain_text(&inlines);
            Some(Block::MathBlock {
                content: text,
                label: None,
            })
        }
        StyleKind::BlockQuote => Some(Block::BlockQuote {
            content: vec![Block::Paragraph { content: inlines }],
        }),
        StyleKind::Caption | StyleKind::Author | StyleKind::Date | StyleKind::Abstract => {
            // These are metadata-like — emit as paragraphs
            if inlines.is_empty() {
                None
            } else {
                Some(Block::Paragraph { content: inlines })
            }
        }
        StyleKind::Normal => {
            if inlines.is_empty() {
                None
            } else {
                Some(Block::Paragraph { content: inlines })
            }
        }
    }
}

/// Extract plain text from a list of inlines (for code blocks, math, etc.).
fn extract_plain_text(inlines: &[Inline]) -> String {
    let mut text = String::new();
    for inline in inlines {
        match inline {
            Inline::Text { value } => text.push_str(value),
            Inline::Code { value, .. } => text.push_str(value),
            Inline::SoftBreak => text.push(' '),
            Inline::HardBreak => text.push('\n'),
            Inline::Strong { content }
            | Inline::Emphasis { content }
            | Inline::Strikethrough { content }
            | Inline::Underline { content }
            | Inline::Superscript { content }
            | Inline::Subscript { content }
            | Inline::SmallCaps { content } => {
                text.push_str(&extract_plain_text(content));
            }
            _ => {}
        }
    }
    text
}
```

- [ ] **Step 2: Implement `parse_table`**

Replace the `parse_table` stub with:

```rust
/// Parse a `<w:tbl>` element into an AST Table block.
fn parse_table(xml: &str, ctx: &ParseContext<'_>) -> Block {
    let mut reader = Reader::from_str(xml);
    let mut rows: Vec<Vec<TableCell>> = Vec::new();
    let mut header: Option<Vec<TableCell>> = None;
    let mut columns: Vec<ColumnSpec> = Vec::new();

    let mut in_row = false;
    let mut is_header_row = false;
    let mut current_row: Vec<TableCell> = Vec::new();
    let mut in_cell = false;
    let mut cell_content = String::new();
    let mut cell_depth: i32 = 0;
    let mut cell_colspan: u32 = 1;
    let mut cell_rowspan: u32 = 1;
    let mut grid_col_count: usize = 0;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"tr" => {
                        in_row = true;
                        is_header_row = false;
                        current_row.clear();
                    }
                    b"tc" if in_row => {
                        in_cell = true;
                        cell_content.clear();
                        cell_depth = 1;
                        cell_colspan = 1;
                        cell_rowspan = 1;
                    }
                    _ if in_cell => {
                        cell_depth += 1;
                        append_start_tag(e, &mut cell_content);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"gridCol" => grid_col_count += 1,
                    b"tblHeader" if in_row => is_header_row = true,
                    b"gridSpan" if in_cell => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                let val = String::from_utf8_lossy(&attr.value);
                                cell_colspan = val.parse().unwrap_or(1);
                            }
                        }
                    }
                    b"vMerge" if in_cell => {
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"val" {
                                let val = String::from_utf8_lossy(&attr.value);
                                if val == "restart" {
                                    cell_rowspan = 2; // Approximate
                                }
                            }
                        }
                    }
                    _ if in_cell => {
                        append_empty_tag(e, &mut cell_content);
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_cell => {
                if let Ok(text) = e.unescape() {
                    cell_content.push_str(&quick_xml_escape(&text));
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.local_name();
                match local.as_ref() {
                    b"tc" if in_cell => {
                        // Parse cell content as blocks
                        let cell_inlines = parse_runs(&cell_content, ctx);
                        let content = if cell_inlines.is_empty() {
                            Vec::new()
                        } else {
                            vec![Block::Paragraph {
                                content: cell_inlines,
                            }]
                        };
                        current_row.push(TableCell {
                            content,
                            colspan: cell_colspan,
                            rowspan: cell_rowspan,
                        });
                        in_cell = false;
                    }
                    b"tr" if in_row => {
                        if is_header_row && header.is_none() {
                            header = Some(current_row.clone());
                        } else {
                            rows.push(current_row.clone());
                        }
                        in_row = false;
                    }
                    _ if in_cell => {
                        cell_depth -= 1;
                        append_end_tag(e, &mut cell_content);
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
    }

    // Build column specs
    if grid_col_count == 0 {
        // Infer from first row
        grid_col_count = header
            .as_ref()
            .or(rows.first())
            .map(|r| r.len())
            .unwrap_or(0);
    }
    columns = (0..grid_col_count)
        .map(|_| ColumnSpec {
            alignment: Alignment::Default,
            width: None,
        })
        .collect();

    Block::Table(Table {
        caption: None,
        label: None,
        columns,
        header,
        rows,
        foot: None,
        attrs: None,
    })
}
```

- [ ] **Step 3: Add block-level tests**

Add to the `tests` module in `document.rs`:

```rust
#[test]
fn parse_heading_paragraph() {
    let mut styles = StyleMap::new();
    styles.insert(
        "Heading1".to_string(),
        crate::styles::StyleInfo {
            style_id: "Heading1".to_string(),
            name: "heading 1".to_string(),
            based_on: None,
            style_type: "paragraph".to_string(),
            kind: StyleKind::Heading(1),
        },
    );
    let numbering = NumberingMap::new();
    let rels = RelMap::new();
    let ctx = ParseContext {
        styles: &styles,
        numbering: &numbering,
        rels: &rels,
    };

    let xml = r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p>"#;
    let block = parse_paragraph(xml, &ctx);
    match block {
        Some(Block::Heading { level, content, .. }) => {
            assert_eq!(level, 1);
            assert_eq!(content.len(), 1);
        }
        other => panic!("expected Heading, got {other:?}"),
    }
}

#[test]
fn parse_thematic_break() {
    let styles = StyleMap::new();
    let numbering = NumberingMap::new();
    let rels = RelMap::new();
    let ctx = ParseContext {
        styles: &styles,
        numbering: &numbering,
        rels: &rels,
    };

    let xml = r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6" w:space="1" w:color="auto"/></w:pBdr></w:pPr></w:p>"#;
    let block = parse_paragraph(xml, &ctx);
    assert!(matches!(block, Some(Block::ThematicBreak)));
}

#[test]
fn parse_code_block_paragraph() {
    let mut styles = StyleMap::new();
    styles.insert(
        "CodeBlock".to_string(),
        crate::styles::StyleInfo {
            style_id: "CodeBlock".to_string(),
            name: "Code Block".to_string(),
            based_on: None,
            style_type: "paragraph".to_string(),
            kind: StyleKind::CodeBlock,
        },
    );
    let numbering = NumberingMap::new();
    let rels = RelMap::new();
    let ctx = ParseContext {
        styles: &styles,
        numbering: &numbering,
        rels: &rels,
    };

    let xml = r#"<w:p><w:pPr><w:pStyle w:val="CodeBlock"/></w:pPr><w:r><w:t xml:space="preserve">fn main() {}</w:t></w:r></w:p>"#;
    let block = parse_paragraph(xml, &ctx);
    match block {
        Some(Block::CodeBlock { content, .. }) => {
            assert_eq!(content, "fn main() {}");
        }
        other => panic!("expected CodeBlock, got {other:?}"),
    }
}

#[test]
fn parse_simple_table() {
    let styles = StyleMap::new();
    let numbering = NumberingMap::new();
    let rels = RelMap::new();
    let ctx = ParseContext {
        styles: &styles,
        numbering: &numbering,
        rels: &rels,
    };

    let xml = r#"<w:tbl>
<w:tblGrid><w:gridCol/><w:gridCol/></w:tblGrid>
<w:tr><w:trPr><w:tblHeader/></w:trPr>
  <w:tc><w:tcPr><w:tcW w:w="0" w:type="auto"/></w:tcPr><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc>
  <w:tc><w:tcPr><w:tcW w:w="0" w:type="auto"/></w:tcPr><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc>
</w:tr>
<w:tr>
  <w:tc><w:tcPr><w:tcW w:w="0" w:type="auto"/></w:tcPr><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc>
  <w:tc><w:tcPr><w:tcW w:w="0" w:type="auto"/></w:tcPr><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc>
</w:tr>
</w:tbl>"#;

    let block = parse_table(xml, &ctx);
    match block {
        Block::Table(table) => {
            assert_eq!(table.columns.len(), 2);
            assert!(table.header.is_some());
            assert_eq!(table.header.as_ref().unwrap().len(), 2);
            assert_eq!(table.rows.len(), 1);
            assert_eq!(table.rows[0].len(), 2);
        }
        other => panic!("expected Table, got {other:?}"),
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p docmux-reader-docx`
Expected: All PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-reader-docx/src/document.rs
git commit -m "feat(docx): paragraph classifier and table parser"
```

---

### Task 10: Wire up `DocxReader` with all modules

**Files:**
- Modify: `crates/docmux-reader-docx/src/lib.rs`

- [ ] **Step 1: Implement the full `read_bytes` method**

Replace `crates/docmux-reader-docx/src/lib.rs` with:

```rust
//! # docmux-reader-docx
//!
//! DOCX (Office Open XML) reader for docmux. Parses `.docx` files from
//! any source (Word, Google Docs, LibreOffice) into the docmux AST.

use docmux_ast::Document;
use docmux_core::{BinaryReader, Result};

mod archive;
mod document;
mod footnotes;
mod metadata;
mod numbering;
mod relationships;
mod styles;

/// A DOCX reader.
#[derive(Debug, Default)]
pub struct DocxReader;

impl DocxReader {
    pub fn new() -> Self {
        Self
    }
}

impl BinaryReader for DocxReader {
    fn format(&self) -> &str {
        "docx"
    }

    fn extensions(&self) -> &[&str] {
        &["docx"]
    }

    fn read_bytes(&self, input: &[u8]) -> Result<Document> {
        let archive = archive::DocxArchive::open(input)?;

        // Parse relationships
        let rels = archive
            .get_xml("word/_rels/document.xml.rels")
            .map(relationships::parse_relationships)
            .transpose()?
            .unwrap_or_default();

        // Parse styles
        let style_map = archive
            .get_xml("word/styles.xml")
            .map(styles::parse_styles)
            .transpose()?
            .unwrap_or_default();

        // Parse numbering
        let numbering_map = archive
            .get_xml("word/numbering.xml")
            .map(numbering::parse_numbering)
            .transpose()?
            .unwrap_or_default();

        // Parse footnotes
        let footnote_map = archive
            .get_xml("word/footnotes.xml")
            .map(footnotes::parse_footnotes)
            .transpose()?
            .unwrap_or_default();

        // Parse metadata from Dublin Core
        let mut doc_metadata = archive
            .get_xml("docProps/core.xml")
            .map(metadata::parse_core_properties)
            .transpose()?
            .unwrap_or_default();

        // Parse document body
        let document_xml = archive
            .get_xml("word/document.xml")
            .ok_or_else(|| {
                docmux_core::ConvertError::Parse {
                    line: 0,
                    col: 0,
                    message: "missing word/document.xml in DOCX archive".to_string(),
                }
            })?;

        let ctx = document::ParseContext {
            styles: &style_map,
            numbering: &numbering_map,
            rels: &rels,
        };

        let mut content = document::parse_body(document_xml, &ctx)?;

        // Append footnote definitions as blocks at the end
        for (id, blocks) in footnote_map {
            content.push(docmux_ast::Block::FootnoteDef {
                id,
                content: blocks,
            });
        }

        // If metadata wasn't in core.xml, try to extract from styled paragraphs
        // (Title, Author, Date styles at the start of the document)
        if doc_metadata.title.is_none() {
            extract_metadata_from_body(&mut content, &mut doc_metadata, &style_map);
        }

        Ok(Document {
            metadata: doc_metadata,
            content,
            bibliography: None,
            warnings: Vec::new(),
        })
    }
}

/// Extract metadata from styled paragraphs at the beginning of the document.
/// Removes consumed paragraphs from `content`.
fn extract_metadata_from_body(
    content: &mut Vec<docmux_ast::Block>,
    metadata: &mut docmux_ast::Metadata,
    style_map: &styles::StyleMap,
) {
    // This is a simple heuristic: if the first few paragraphs have
    // Title/Author/Date styles, extract them as metadata.
    // We leave the implementation simple for now and can enhance later.
    let _ = (content, metadata, style_map);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_impl_exists() {
        let reader = DocxReader::new();
        assert_eq!(reader.format(), "docx");
        assert_eq!(reader.extensions(), &["docx"]);
    }

    #[test]
    fn read_empty_docx() {
        // Build a minimal DOCX with just document.xml
        use std::io::{Cursor, Write};
        use zip::write::SimpleFileOptions;
        use zip::{CompressionMethod, ZipWriter};

        let buf = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(buf);
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        zip.start_file("word/document.xml", opts).unwrap();
        zip.write_all(
            br#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body></w:body>
</w:document>"#,
        )
        .unwrap();

        let bytes = zip.finish().unwrap().into_inner();

        let reader = DocxReader::new();
        let doc = reader.read_bytes(&bytes).unwrap();
        assert!(doc.content.is_empty());
    }

    #[test]
    fn reject_invalid_bytes() {
        let reader = DocxReader::new();
        let result = reader.read_bytes(b"not a zip file");
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p docmux-reader-docx`
Expected: All PASS

- [ ] **Step 3: Run clippy on the whole crate**

Run: `cargo clippy -p docmux-reader-docx -- -D warnings`
Expected: No warnings

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-reader-docx/src/lib.rs
git commit -m "feat(docx): wire up DocxReader with all parser modules"
```

---

### Task 11: CLI integration — binary input path

**Files:**
- Modify: `crates/docmux-cli/Cargo.toml` (add `docmux-reader-docx`)
- Modify: `crates/docmux-cli/src/main.rs`

- [ ] **Step 1: Add dependency**

In `crates/docmux-cli/Cargo.toml`, add to `[dependencies]`:

```toml
docmux-reader-docx = { workspace = true }
```

- [ ] **Step 2: Register binary reader in CLI**

In `crates/docmux-cli/src/main.rs`, add import:

```rust
use docmux_reader_docx::DocxReader;
```

In `build_registry()`, add after the last `add_reader` call:

```rust
reg.add_binary_reader(Box::new(DocxReader::new()));
```

- [ ] **Step 3: Add binary input dispatch to `main()`**

In `crates/docmux-cli/src/main.rs`, replace the input reading section (lines 178–198) and the reader lookup (lines 222–241) with a unified approach. After format detection, before the JSON/writer sections:

Replace the "Look up reader" + "Parse" block (lines 222–241) with:

```rust
// Try binary reader first, then text reader
let mut doc = if let Some(binary_reader) = registry.find_binary_reader(from) {
    // Binary input: read raw bytes
    let mut combined_bytes = Vec::new();
    for path in &cli.input {
        if path.to_str() == Some("-") {
            use std::io::Read;
            std::io::stdin()
                .read_to_end(&mut combined_bytes)
                .unwrap_or_else(|e| {
                    eprintln!("docmux: error reading stdin: {e}");
                    std::process::exit(1);
                });
        } else {
            match std::fs::read(path) {
                Ok(bytes) => combined_bytes.extend(bytes),
                Err(e) => {
                    eprintln!("docmux: error reading {}: {e}", path.display());
                    std::process::exit(1);
                }
            }
        }
    }
    match binary_reader.read_bytes(&combined_bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("docmux: parse error: {e}");
            std::process::exit(1);
        }
    }
} else {
    // Text input: existing path
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
    match reader.read(&combined_input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("docmux: parse error: {e}");
            std::process::exit(1);
        }
    }
};
```

Note: the `combined_input` String reading (lines 178–198) stays for text readers. For binary readers, we read bytes separately. This means the existing `combined_input` code still runs but is only used when the binary reader path is not taken.

- [ ] **Step 4: Run CLI tests**

Run: `cargo test -p docmux-cli`
Expected: All PASS (existing tests unaffected)

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-cli/Cargo.toml crates/docmux-cli/src/main.rs
git commit -m "feat(cli): add binary reader dispatch for DOCX input"
```

---

### Task 12: WASM integration — `convertBytes` API

**Files:**
- Modify: `crates/docmux-wasm/Cargo.toml` (add `docmux-reader-docx`)
- Modify: `crates/docmux-wasm/src/lib.rs`

- [ ] **Step 1: Add dependency**

In `crates/docmux-wasm/Cargo.toml`, add to `[dependencies]` (readers section):

```toml
docmux-reader-docx = { workspace = true }
```

- [ ] **Step 2: Register binary reader and add `convertBytes` functions**

In `crates/docmux-wasm/src/lib.rs`, add import:

```rust
use docmux_reader_docx::DocxReader;
```

In `build_registry()`, add after the last `add_reader` call:

```rust
reg.add_binary_reader(Box::new(DocxReader::new()));
```

Add these new exported functions after the existing ones:

```rust
/// Convert a binary document (e.g. DOCX) to a text format (fragment mode).
#[wasm_bindgen(js_name = "convertBytes")]
pub fn convert_bytes(input: &[u8], from: &str, to: &str) -> Result<String, JsError> {
    convert_bytes_inner(input, from, to, false)
}

/// Convert a binary document producing a standalone file.
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

/// Parse a binary document and return the AST as pretty-printed JSON.
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
```

Update `input_formats()` — no change needed since `Registry::reader_formats()` already includes binary reader formats.

- [ ] **Step 3: Check WASM build**

Run: `cargo build --target wasm32-unknown-unknown -p docmux-wasm`
Expected: Build succeeds

- [ ] **Step 4: Commit**

```bash
git add crates/docmux-wasm/Cargo.toml crates/docmux-wasm/src/lib.rs
git commit -m "feat(wasm): add convertBytes API for DOCX input"
```

---

### Task 13: Roundtrip test — Markdown → DOCX → AST

**Files:**
- Modify: `crates/docmux-reader-docx/src/lib.rs` (add integration test)

- [ ] **Step 1: Add roundtrip test**

Add to the `#[cfg(test)] mod tests` in `crates/docmux-reader-docx/src/lib.rs`:

```rust
#[test]
fn roundtrip_markdown_basic() {
    use docmux_core::{WriteOptions, Writer};
    use docmux_reader_markdown::MarkdownReader;
    use docmux_writer_docx::DocxWriter;
    use docmux_core::Reader;

    let md = "# Hello\n\nThis is a **bold** paragraph with *emphasis*.\n\n## Subheading\n\nAnother paragraph.\n";

    let md_reader = MarkdownReader::new();
    let docx_writer = DocxWriter::new();
    let docx_reader = DocxReader::new();

    // Markdown → AST → DOCX bytes
    let original = md_reader.read(md).unwrap();
    let bytes = docx_writer
        .write_bytes(&original, &WriteOptions::default())
        .unwrap();

    // DOCX bytes → AST
    let recovered = docx_reader.read_bytes(&bytes).unwrap();

    // Verify structural equivalence
    // Should have: Heading(1), Paragraph, Heading(2), Paragraph
    let block_types: Vec<&str> = recovered
        .content
        .iter()
        .map(|b| match b {
            docmux_ast::Block::Heading { .. } => "heading",
            docmux_ast::Block::Paragraph { .. } => "paragraph",
            docmux_ast::Block::CodeBlock { .. } => "code",
            docmux_ast::Block::ThematicBreak => "hr",
            _ => "other",
        })
        .collect();

    assert!(
        block_types.contains(&"heading"),
        "should contain headings, got: {block_types:?}"
    );
    assert!(
        block_types.contains(&"paragraph"),
        "should contain paragraphs, got: {block_types:?}"
    );
}

#[test]
fn roundtrip_code_block() {
    use docmux_core::{WriteOptions, Writer};
    use docmux_reader_markdown::MarkdownReader;
    use docmux_writer_docx::DocxWriter;
    use docmux_core::Reader;

    let md = "```rust\nfn main() {}\n```\n";

    let md_reader = MarkdownReader::new();
    let docx_writer = DocxWriter::new();
    let docx_reader = DocxReader::new();

    let original = md_reader.read(md).unwrap();
    let bytes = docx_writer
        .write_bytes(&original, &WriteOptions::default())
        .unwrap();
    let recovered = docx_reader.read_bytes(&bytes).unwrap();

    let has_code = recovered
        .content
        .iter()
        .any(|b| matches!(b, docmux_ast::Block::CodeBlock { .. }));
    assert!(has_code, "should have a code block in: {:?}", recovered.content);
}

#[test]
fn roundtrip_table() {
    use docmux_core::{WriteOptions, Writer};
    use docmux_reader_markdown::MarkdownReader;
    use docmux_writer_docx::DocxWriter;
    use docmux_core::Reader;

    let md = "| A | B |\n|---|---|\n| 1 | 2 |\n";

    let md_reader = MarkdownReader::new();
    let docx_writer = DocxWriter::new();
    let docx_reader = DocxReader::new();

    let original = md_reader.read(md).unwrap();
    let bytes = docx_writer
        .write_bytes(&original, &WriteOptions::default())
        .unwrap();
    let recovered = docx_reader.read_bytes(&bytes).unwrap();

    let has_table = recovered
        .content
        .iter()
        .any(|b| matches!(b, docmux_ast::Block::Table(_)));
    assert!(has_table, "should have a table in: {:?}", recovered.content);
}
```

- [ ] **Step 2: Run roundtrip tests**

Run: `cargo test -p docmux-reader-docx -- roundtrip`
Expected: All PASS

- [ ] **Step 3: Run full workspace tests**

Run: `cargo test --workspace`
Expected: All PASS

- [ ] **Step 4: Run clippy on workspace**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: No warnings

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-reader-docx/src/lib.rs
git commit -m "test(docx): roundtrip tests markdown → DOCX → AST"
```

---

### Task 14: WASM build verification

**Files:** None (verification only)

- [ ] **Step 1: Build WASM target**

Run: `cargo build --target wasm32-unknown-unknown -p docmux-wasm`
Expected: Build succeeds

- [ ] **Step 2: Verify `quick-xml` works in WASM**

The `quick-xml` crate is pure Rust with no system dependencies, so it should compile to WASM without issues. If the build fails, check for any `std::fs` or `std::net` usage in the reader crate that wouldn't be available in WASM.

- [ ] **Step 3: Run full CI checks**

Run all verification commands:

```bash
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build --target wasm32-unknown-unknown -p docmux-wasm
```

Expected: All pass

- [ ] **Step 4: Commit (if any fixes were needed)**

```bash
git add -A
git commit -m "fix(wasm): ensure DOCX reader compiles to wasm32"
```
