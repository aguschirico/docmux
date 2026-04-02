//! DOCX archive module — opens a DOCX ZIP and provides access to XML parts.

use crate::DocxError;
use std::collections::HashMap;
use std::io::Read;
use zip::ZipArchive;

/// A loaded DOCX archive: all parts extracted into memory.
#[allow(dead_code)]
pub(crate) struct DocxArchive {
    parts: HashMap<String, Vec<u8>>,
}

#[allow(dead_code)]
impl DocxArchive {
    /// Open a DOCX archive from raw bytes.
    pub(crate) fn from_bytes(data: &[u8]) -> Result<Self, DocxError> {
        let cursor = std::io::Cursor::new(data);
        let mut zip = ZipArchive::new(cursor).map_err(|e| DocxError::Zip(e.to_string()))?;

        let mut parts: HashMap<String, Vec<u8>> = HashMap::new();

        for i in 0..zip.len() {
            let mut entry = zip.by_index(i).map_err(|e| DocxError::Zip(e.to_string()))?;
            let name = entry.name().to_string();
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| DocxError::Zip(e.to_string()))?;
            parts.insert(name, buf);
        }

        Ok(Self { parts })
    }

    /// Retrieve a part as a UTF-8 string (for XML parts).
    pub(crate) fn get_xml(&self, path: &str) -> Option<Result<String, DocxError>> {
        self.parts.get(path).map(|bytes| {
            String::from_utf8(bytes.clone()).map_err(|e| DocxError::Utf8(e.to_string()))
        })
    }

    /// Retrieve a part as raw bytes (e.g. for media files).
    pub(crate) fn get_bytes(&self, path: &str) -> Option<&[u8]> {
        self.parts.get(path).map(|v| v.as_slice())
    }

    /// List paths of all embedded media files (under `word/media/`).
    pub(crate) fn media_paths(&self) -> Vec<&str> {
        self.parts
            .keys()
            .filter(|k| k.starts_with("word/media/"))
            .map(|k| k.as_str())
            .collect()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use zip::write::{FileOptions, ZipWriter};

    fn make_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        let cursor = std::io::Cursor::new(&mut buf);
        let mut zw = ZipWriter::new(cursor);
        let opts = FileOptions::<()>::default();
        for (name, data) in entries {
            zw.start_file(*name, opts).unwrap();
            zw.write_all(data).unwrap();
        }
        zw.finish().unwrap();
        buf
    }

    #[test]
    fn open_valid_zip() {
        let zip_bytes = make_zip(&[
            ("word/document.xml", b"<hello/>"),
            ("word/media/image1.png", b"\x89PNG"),
        ]);

        let archive = DocxArchive::from_bytes(&zip_bytes).expect("should open");
        let xml = archive
            .get_xml("word/document.xml")
            .expect("part exists")
            .expect("valid utf8");
        assert_eq!(xml, "<hello/>");
    }

    #[test]
    fn open_invalid_bytes() {
        let result = DocxArchive::from_bytes(b"not a zip at all");
        assert!(result.is_err());
    }

    #[test]
    fn media_paths() {
        let zip_bytes = make_zip(&[
            ("word/document.xml", b"<doc/>"),
            ("word/media/image1.png", b"\x89PNG"),
            ("word/media/image2.jpg", b"\xFF\xD8"),
            ("[Content_Types].xml", b"<types/>"),
        ]);

        let archive = DocxArchive::from_bytes(&zip_bytes).expect("should open");
        let mut paths = archive.media_paths();
        paths.sort();
        assert_eq!(
            paths,
            vec!["word/media/image1.png", "word/media/image2.jpg"]
        );
    }
}
