//! # docmux-reader-docx
//!
//! DOCX reader for docmux — parses a DOCX ZIP archive into the docmux AST.

mod archive;

use archive::DocxArchive;
use docmux_ast::Document;
use docmux_core::{BinaryReader, Result as CoreResult};

// ─── Error type ──────────────────────────────────────────────────────────────

/// Errors specific to DOCX parsing.
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub(crate) enum DocxError {
    #[error("zip error: {0}")]
    Zip(String),
    #[error("utf-8 error: {0}")]
    Utf8(String),
    #[error("xml error: {0}")]
    Xml(String),
    #[error("missing part: {0}")]
    MissingPart(String),
}

impl From<DocxError> for docmux_core::ConvertError {
    fn from(e: DocxError) -> Self {
        docmux_core::ConvertError::Other(e.to_string())
    }
}

// ─── DocxReader ───────────────────────────────────────────────────────────────

/// A reader that parses DOCX binary input into a [`Document`].
pub struct DocxReader;

impl BinaryReader for DocxReader {
    fn format(&self) -> &str {
        "docx"
    }

    fn extensions(&self) -> &[&str] {
        &["docx"]
    }

    fn read_bytes(&self, input: &[u8]) -> CoreResult<Document> {
        let _archive = DocxArchive::from_bytes(input)?;
        Ok(Document::default())
    }
}
