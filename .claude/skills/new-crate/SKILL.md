---
name: new-crate
description: Scaffold a new docmux crate (reader, writer, or transform) with standard workspace structure, trait implementation, and tests
disable-model-invocation: true
argument-hint: "reader|writer|transform <name>"
---

# Scaffold a New docmux Crate

Usage: `/new-crate <type> <name>`
- `type`: `reader`, `writer`, or `transform`
- `name`: format or transform name (e.g., `epub`, `rst`, `normalize`)

Example: `/new-crate writer epub`

## Steps

### 1. Create the crate directory

```
crates/docmux-{type}-{name}/
├── Cargo.toml
└── src/
    └── lib.rs
```

### 2. Cargo.toml template

```toml
[package]
name = "docmux-{type}-{name}"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
description = "{Description} for docmux"
rust-version.workspace = true

[dependencies]
docmux-ast = { workspace = true }
docmux-core = { workspace = true }
```

Add external dependencies only if strictly needed for the format.

### 3. src/lib.rs template

**For a Reader:**
```rust
//! # docmux-reader-{name}
//!
//! {Format} reader for docmux.

use docmux_ast::Document;
use docmux_core::{ReadOptions, Reader, Result};

#[derive(Debug, Default)]
pub struct {Name}Reader;

impl {Name}Reader {
    pub fn new() -> Self {
        Self
    }
}

impl Reader for {Name}Reader {
    fn read(&self, input: &str, options: &ReadOptions) -> Result<Document> {
        todo!("Implement {name} reader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let reader = {Name}Reader::new();
        let doc = reader.read("", &ReadOptions::default()).unwrap();
        assert!(doc.content.is_empty());
    }
}
```

**For a Writer:**
```rust
//! # docmux-writer-{name}
//!
//! {Format} writer for docmux.

use docmux_ast::*;
use docmux_core::{Result, WriteOptions, Writer};

#[derive(Debug, Default)]
pub struct {Name}Writer;

impl {Name}Writer {
    pub fn new() -> Self {
        Self
    }
}

impl Writer for {Name}Writer {
    fn write(&self, doc: &Document, options: &WriteOptions) -> Result<String> {
        let mut out = String::new();
        for block in &doc.content {
            self.write_block(block, &mut out);
        }
        Ok(out)
    }
}

impl {Name}Writer {
    fn write_block(&self, block: &Block, out: &mut String) {
        todo!("Implement block writing")
    }

    fn write_inlines(&self, inlines: &[Inline], out: &mut String) {
        todo!("Implement inline writing")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_document() {
        let writer = {Name}Writer::new();
        let doc = Document::default();
        let result = writer.write(&doc, &WriteOptions::default()).unwrap();
        assert!(result.is_empty());
    }
}
```

**For a Transform:**
```rust
//! # docmux-transform-{name}
//!
//! {Description} transform for docmux.

use docmux_ast::Document;
use docmux_core::{Result, Transform};

#[derive(Debug, Default)]
pub struct {Name}Transform;

impl {Name}Transform {
    pub fn new() -> Self {
        Self
    }
}

impl Transform for {Name}Transform {
    fn transform(&self, doc: &mut Document) -> Result<()> {
        todo!("Implement {name} transform")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_document() {
        let mut doc = Document::default();
        let transform = {Name}Transform::new();
        transform.transform(&mut doc).unwrap();
        assert!(doc.content.is_empty());
    }
}
```

### 4. Register in workspace

Add to root `Cargo.toml`:

1. In `[workspace] members`: `"crates/docmux-{type}-{name}"`
2. In `[workspace.dependencies]`: `docmux-{type}-{name} = { path = "crates/docmux-{type}-{name}" }`

### 5. Verify

Run:
```sh
cargo check -p docmux-{type}-{name}
cargo test -p docmux-{type}-{name}
cargo clippy -p docmux-{type}-{name} -- -D warnings
```

### 6. Update documentation

After scaffolding, update these files to reflect the new crate:
- `CLAUDE.md` — update crate count in the Architecture section if mentioned
- `memory/project_status.md` — update crate count and test count
- If the crate introduces format-specific architecture, create a `.claude/rules/{name}.md` with path-scoped rules
