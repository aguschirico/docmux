//! # docmux-core
//!
//! Core traits and conversion pipeline for docmux.
//!
//! This crate defines the [`Reader`], [`Writer`], and [`Transform`] traits
//! that every format plugin must implement, plus the [`Pipeline`] that
//! chains them together.

use docmux_ast::Document;
use std::collections::HashMap;

// ─── Errors ──────────────────────────────────────────────────────────────────

/// Errors that can occur during document conversion.
#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    /// A parse error with location information.
    #[error("parse error at {line}:{col}: {message}")]
    Parse {
        line: usize,
        col: usize,
        message: String,
    },

    /// The input uses a feature that the reader/writer does not support.
    #[error("unsupported feature: {0}")]
    Unsupported(String),

    /// An I/O error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A catch-all for other errors.
    #[error("{0}")]
    Other(String),
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, ConvertError>;

// ─── Reader trait ────────────────────────────────────────────────────────────

/// Parses a source string into a [`Document`] AST.
pub trait Reader: Send + Sync {
    /// A human-readable format name (e.g. `"markdown"`, `"typst"`).
    fn format(&self) -> &str;

    /// File extensions this reader handles (e.g. `["md", "markdown"]`).
    fn extensions(&self) -> &[&str];

    /// Parse `input` into a document AST.
    fn read(&self, input: &str) -> Result<Document>;
}

// ─── BinaryReader trait ─────────────────────────────────────────────────────

/// Parses binary input (e.g. a ZIP archive) into a [`Document`] AST.
pub trait BinaryReader: Send + Sync {
    /// A human-readable format name (e.g. `"docx"`).
    fn format(&self) -> &str;

    /// File extensions this reader handles (e.g. `["docx"]`).
    fn extensions(&self) -> &[&str];

    /// Parse binary `input` into a document AST.
    fn read_bytes(&self, input: &[u8]) -> Result<Document>;
}

// ─── Writer trait ────────────────────────────────────────────────────────────

/// Renders a [`Document`] AST into an output format.
pub trait Writer: Send + Sync {
    /// A human-readable format name (e.g. `"html"`, `"latex"`).
    fn format(&self) -> &str;

    /// The default file extension for this format (e.g. `"html"`).
    fn default_extension(&self) -> &str;

    /// Render the document to a UTF-8 string.
    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String>;

    /// Render the document to bytes (for binary formats like DOCX).
    ///
    /// The default implementation delegates to [`write`](Writer::write)
    /// and encodes the result as UTF-8.
    fn write_bytes(&self, doc: &Document, opts: &WriteOptions) -> Result<Vec<u8>> {
        self.write(doc, opts).map(|s| s.into_bytes())
    }
}

// ─── Transform trait ─────────────────────────────────────────────────────────

/// An AST-to-AST transformation (e.g. resolve citations, number cross-refs).
pub trait Transform: Send + Sync {
    /// A short identifier (e.g. `"crossref"`, `"cite"`).
    fn name(&self) -> &str;

    /// Mutate the document in place.
    fn transform(&self, doc: &mut Document, ctx: &TransformContext) -> Result<()>;
}

/// Contextual data available to transforms.
#[derive(Debug, Clone, Default)]
pub struct TransformContext {
    /// Variables that can influence transform behaviour.
    pub variables: HashMap<String, String>,
}

// ─── Options ─────────────────────────────────────────────────────────────────

/// Options controlling how a [`Writer`] renders the document.
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Which math engine the output should target.
    pub math_engine: MathEngine,
    /// CSL citation style (e.g. `"apa"`).
    pub citation_style: Option<String>,
    /// Whether to produce a complete standalone file (e.g. full `<html>`).
    pub standalone: bool,
    /// Optional template string.
    pub template: Option<String>,
    /// Arbitrary key-value variables passed to templates.
    pub variables: HashMap<String, String>,
    /// How to handle line wrapping in the output.
    pub wrap: WrapMode,
    /// Target column width for `WrapMode::Auto`.
    pub columns: usize,
    /// Line ending style for the output.
    pub eol: Eol,
    /// Syntax highlighting theme name (e.g. `"InspiredGitHub"`).
    /// `None` disables highlighting.
    pub highlight_style: Option<String>,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            math_engine: MathEngine::KaTeX,
            citation_style: None,
            standalone: false,
            template: None,
            variables: HashMap::new(),
            wrap: WrapMode::default(),
            columns: 72,
            eol: Eol::default(),
            highlight_style: None,
        }
    }
}

/// Line-wrapping behaviour for writers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WrapMode {
    /// Wrap at [`WriteOptions::columns`] width.
    Auto,
    /// Do not add any line breaks (each paragraph on one line).
    #[default]
    None,
    /// Preserve existing line breaks from the source.
    Preserve,
}

/// Line-ending style.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Eol {
    /// Unix-style `\n`.
    #[default]
    Lf,
    /// Windows-style `\r\n`.
    Crlf,
    /// Use the platform default.
    Native,
}

/// Target math rendering engine.
#[derive(Debug, Clone, Copy, Default)]
pub enum MathEngine {
    /// Output `<span class="math">` with KaTeX-compatible markup.
    #[default]
    KaTeX,
    /// Output MathJax-compatible markup.
    MathJax,
    /// Leave math source as-is (useful for LaTeX/Typst writers).
    Raw,
}

// ─── Pipeline ────────────────────────────────────────────────────────────────

/// A conversion pipeline: reader → [transforms…] → writer.
pub struct Pipeline {
    reader: Box<dyn Reader>,
    writer: Box<dyn Writer>,
    transforms: Vec<Box<dyn Transform>>,
    write_opts: WriteOptions,
}

impl Pipeline {
    /// Create a new pipeline with the given reader and writer.
    pub fn new(reader: Box<dyn Reader>, writer: Box<dyn Writer>) -> Self {
        Self {
            reader,
            writer,
            transforms: Vec::new(),
            write_opts: WriteOptions::default(),
        }
    }

    /// Append a transform to the pipeline.
    pub fn with_transform(mut self, t: Box<dyn Transform>) -> Self {
        self.transforms.push(t);
        self
    }

    /// Set write options.
    pub fn with_options(mut self, opts: WriteOptions) -> Self {
        self.write_opts = opts;
        self
    }

    /// Run the full conversion: parse → transform → render.
    pub fn convert(&self, input: &str) -> Result<String> {
        let mut doc = self.reader.read(input)?;
        let ctx = TransformContext::default();
        for t in &self.transforms {
            t.transform(&mut doc, &ctx)?;
        }
        self.writer.write(&doc, &self.write_opts)
    }

    /// Run the full conversion and return bytes (for binary formats).
    pub fn convert_bytes(&self, input: &str) -> Result<Vec<u8>> {
        let mut doc = self.reader.read(input)?;
        let ctx = TransformContext::default();
        for t in &self.transforms {
            t.transform(&mut doc, &ctx)?;
        }
        self.writer.write_bytes(&doc, &self.write_opts)
    }
}

// ─── Format registry ─────────────────────────────────────────────────────────

/// A registry that maps format names / extensions to readers and writers.
#[derive(Default)]
pub struct Registry {
    readers: Vec<Box<dyn Reader>>,
    binary_readers: Vec<Box<dyn BinaryReader>>,
    writers: Vec<Box<dyn Writer>>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a reader.
    pub fn add_reader(&mut self, reader: Box<dyn Reader>) {
        self.readers.push(reader);
    }

    /// Register a writer.
    pub fn add_writer(&mut self, writer: Box<dyn Writer>) {
        self.writers.push(writer);
    }

    /// Look up a reader by format name or file extension.
    pub fn find_reader(&self, name_or_ext: &str) -> Option<&dyn Reader> {
        let needle = name_or_ext.trim_start_matches('.');
        self.readers
            .iter()
            .find(|r| r.format() == needle || r.extensions().contains(&needle))
            .map(|r| r.as_ref())
    }

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

    /// Look up a writer by format name or default extension.
    pub fn find_writer(&self, name_or_ext: &str) -> Option<&dyn Writer> {
        let needle = name_or_ext.trim_start_matches('.');
        self.writers
            .iter()
            .find(|w| w.format() == needle || w.default_extension() == needle)
            .map(|w| w.as_ref())
    }

    /// List all available reader format names (text and binary).
    pub fn reader_formats(&self) -> Vec<&str> {
        self.readers
            .iter()
            .map(|r| r.format())
            .chain(self.binary_readers.iter().map(|r| r.format()))
            .collect()
    }

    /// List available writer format names.
    pub fn writer_formats(&self) -> Vec<&str> {
        self.writers.iter().map(|w| w.format()).collect()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_write_options() {
        let opts = WriteOptions::default();
        assert!(!opts.standalone);
        assert!(opts.citation_style.is_none());
        assert!(matches!(opts.math_engine, MathEngine::KaTeX));
    }

    #[test]
    fn registry_empty() {
        let reg = Registry::new();
        assert!(reg.find_reader("markdown").is_none());
        assert!(reg.find_writer("html").is_none());
        assert!(reg.reader_formats().is_empty());
    }

    #[test]
    fn binary_reader_trait_exists() {
        // Verify BinaryReader is a usable trait with Send + Sync bounds
        #[allow(dead_code)]
        fn assert_binary_reader<T: super::BinaryReader + Send + Sync>() {}
        // If this compiles, the trait exists with the right bounds
    }

    #[test]
    fn registry_binary_reader() {
        use super::*;

        struct FakeBinaryReader;
        impl BinaryReader for FakeBinaryReader {
            fn format(&self) -> &str {
                "fake"
            }
            fn extensions(&self) -> &[&str] {
                &["fk"]
            }
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
}
