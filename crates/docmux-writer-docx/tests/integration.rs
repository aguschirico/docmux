//! Integration tests: parse Markdown → write DOCX → verify ZIP structure and XML content.

use docmux_core::{Reader, WriteOptions, Writer};
use docmux_reader_markdown::MarkdownReader;
use docmux_writer_docx::DocxWriter;

fn write_docx(markdown: &str) -> Vec<u8> {
    let reader = MarkdownReader::new();
    let doc = reader.read(markdown).expect("parse failed");
    let writer = DocxWriter::new();
    let opts = WriteOptions {
        standalone: true,
        ..Default::default()
    };
    writer.write_bytes(&doc, &opts).expect("write failed")
}

fn extract_xml(bytes: &[u8], name: &str) -> String {
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut file = archive.by_name(name).unwrap();
    let mut contents = String::new();
    std::io::Read::read_to_string(&mut file, &mut contents).unwrap();
    contents
}

#[test]
fn full_document_structure() {
    let md = r#"---
title: Test Document
author: Alice
date: 2026-01-01
---

# Introduction

This is a paragraph with **bold** and *italic* text.

## Lists

- Item one
- Item two

1. First
2. Second

## Code

```rust
fn main() {
    println!("hello");
}
```

## Table

| Name  | Score |
|-------|-------|
| Alice | 95    |
| Bob   | 87    |

---

> A blockquote with *emphasis*.

Inline math: $E = mc^2$ and a footnote[^1].

[^1]: This is a footnote.
"#;

    let bytes = write_docx(md);

    // Verify ZIP structure
    let cursor = std::io::Cursor::new(&bytes);
    let mut archive = zip::ZipArchive::new(cursor).unwrap();
    let mut names = Vec::new();
    for i in 0..archive.len() {
        names.push(archive.by_index(i).unwrap().name().to_string());
    }

    assert!(names.contains(&"[Content_Types].xml".to_string()));
    assert!(names.contains(&"word/document.xml".to_string()));
    assert!(names.contains(&"word/styles.xml".to_string()));
    assert!(names.contains(&"word/numbering.xml".to_string()));
    assert!(names.contains(&"word/footnotes.xml".to_string()));

    // Verify document.xml content
    let doc_xml = extract_xml(&bytes, "word/document.xml");

    // Metadata
    assert!(
        doc_xml.contains("<w:t>Test Document</w:t>"),
        "missing title"
    );
    assert!(doc_xml.contains("<w:t>Alice</w:t>"), "missing author");

    // Headings
    assert!(doc_xml.contains(r#"<w:pStyle w:val="Heading1"/>"#));
    assert!(doc_xml.contains(r#"<w:pStyle w:val="Heading2"/>"#));
    assert!(doc_xml.contains("<w:t>Introduction</w:t>"));

    // Inline formatting
    assert!(doc_xml.contains("<w:b/>"), "missing bold");
    assert!(doc_xml.contains("<w:i/>"), "missing italic");

    // Lists
    assert!(doc_xml.contains("<w:numPr>"), "missing list numbering");

    // Code block
    assert!(doc_xml.contains(r#"<w:pStyle w:val="CodeBlock"/>"#));

    // Table
    assert!(doc_xml.contains("<w:tbl>"), "missing table");

    // Blockquote
    assert!(doc_xml.contains(r#"<w:pStyle w:val="BlockQuote"/>"#));

    // Thematic break
    assert!(doc_xml.contains("<w:pBdr>"), "missing thematic break");

    // Footnote reference
    assert!(
        doc_xml.contains("<w:footnoteReference"),
        "missing footnote ref"
    );

    // Footnotes file
    let fn_xml = extract_xml(&bytes, "word/footnotes.xml");
    assert!(
        fn_xml.contains("This is a footnote"),
        "missing footnote content"
    );
}
