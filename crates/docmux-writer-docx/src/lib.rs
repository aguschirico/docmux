//! # docmux-writer-docx
//!
//! DOCX (Office Open XML) writer for docmux. Produces `.docx` files as byte
//! vectors — the text-based [`Writer::write`] method returns an error because
//! DOCX is a binary (ZIP) format.

use docmux_ast::Document;
use docmux_core::{ConvertError, Result, WriteOptions, Writer};

/// A DOCX writer that produces Office Open XML `.docx` files.
#[derive(Debug, Default)]
pub struct DocxWriter;

impl DocxWriter {
    pub fn new() -> Self {
        Self
    }
}

impl Writer for DocxWriter {
    fn format(&self) -> &str {
        "docx"
    }

    fn default_extension(&self) -> &str {
        "docx"
    }

    fn write(&self, _doc: &Document, _opts: &WriteOptions) -> Result<String> {
        Err(ConvertError::Unsupported(
            "DOCX is a binary format \u{2014} use write_bytes() instead".into(),
        ))
    }

    fn write_bytes(&self, _doc: &Document, _opts: &WriteOptions) -> Result<Vec<u8>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trait_metadata() {
        let w = DocxWriter::new();
        assert_eq!(w.format(), "docx");
        assert_eq!(w.default_extension(), "docx");
    }

    #[test]
    fn write_returns_unsupported() {
        let w = DocxWriter::new();
        let doc = Document::default();
        let opts = WriteOptions::default();
        let err = w.write(&doc, &opts).unwrap_err();
        assert!(
            matches!(err, ConvertError::Unsupported(_)),
            "expected Unsupported, got {err:?}"
        );
    }
}
