# Cite Transform + Bibliography CLI Flags

**Date**: 2026-04-07
**Status**: Approved
**Priority**: High — largest remaining gap for pandoc parity

## Overview

Implement citation resolution and bibliography generation using `hayagriva` as the CSL engine. This covers four coordinated pieces: markdown reader citation parsing, the cite transform crate, CLI flags, and WASM compatibility.

## Architecture

### Pipeline

```
Input file → Reader → AST (Inline::Citation unresolved)
                        ↓
                 CLI: --bibliography=FILE --csl=FILE
                        ↓
                 CLI: load .bib/.yml → hayagriva::Library
                        ↓
                 docmux-transform-cite
                   1. Walk AST: resolve each Citation → Inline::Str
                   2. Insert bibliography block at Div#refs or end
                        ↓
                 AST (citations resolved, bibliography inserted)
                        ↓
                 Writer → Output file
```

Resolved citations become `Inline::Str` (e.g., `"(Smith, 2020)"`). The writer no longer needs citation awareness — it just renders text.

## Piece 1 — Markdown Reader: `[@key]` Parsing

Comrak has no native pandoc citation support. Post-process comrak's AST by walking text nodes and matching pandoc citation patterns.

### Syntax supported

| Pattern | Meaning | CitationMode |
|---------|---------|-------------|
| `[@key]` | Parenthetical citation | `Normal` |
| `[@k1; @k2]` | Multi-cite | `Normal` (multiple items) |
| `[@key, p. 10]` | With suffix | `Normal` + suffix |
| `[see @key]` | With prefix | `Normal` + prefix |
| `[-@key]` | Suppress author (year only) | `SuppressAuthor` |
| `@key` (inline, no brackets) | Narrative / author-only | `AuthorOnly` |

### Implementation

- Regex-based extraction on text nodes after comrak parse
- Bracketed: `\[(-?)@[\w:.#$%&\-+?<>~/]+(;\s*(-?)@[\w:.#$%&\-+?<>~/]+)*\]`
- Inline narrative: `@[\w:.#$%&\-+?<>~/]+`
- Each match produces `Inline::Citation` with parsed `CiteItem` entries (key, prefix, suffix)

## Piece 2 — `docmux-transform-cite` Crate

Currently a placeholder. Becomes the core citation processor.

### Config

```rust
pub struct CiteTransformConfig {
    pub bibliography: hayagriva::Library,   // pre-parsed library
    pub csl_style: Option<String>,          // CSL XML content (None → chicago-author-date)
    pub locale: Option<String>,             // e.g. "es-ES" (None → "en-US")
}
```

Note: the config receives a pre-parsed `Library`, not file paths. File I/O lives in the CLI layer. This keeps the crate WASM-compatible.

### Transform steps

1. **Receive** pre-parsed `hayagriva::Library` and optional CSL style
2. **Walk AST** for each `Inline::Citation`:
   - Lookup each `CiteItem.key` in the library
   - Key not found → warning to stderr + replace with `Inline::Str("[?key]")`
   - Key found → format with hayagriva according to style and `CitationMode` → replace with `Inline::Str("(Smith, 2020)")` or equivalent
   - Track all cited keys for bibliography generation
3. **Insert bibliography**:
   - Format all cited entries via hayagriva
   - If AST contains a `Block::Div` with `id = "refs"` → replace its content with bibliography entries
   - Otherwise → append bibliography block at end of document

### Behavior without `--bibliography`

No-op. The transform is not registered in the pipeline. Citations remain as `Inline::Citation` nodes and writers render them as keys (current behavior). No warning emitted.

### Unknown citation keys

Warning to stderr: `warning: citation key 'foo' not found in bibliography`
Rendered as `[?foo]` in output — visible signal that the entry is missing.

## Piece 3 — CLI Flags

### New flags

| Flag | Type | Description |
|------|------|-------------|
| `--bibliography=FILE` | `Vec<PathBuf>`, repeatable | Load .bib or .yml bibliography file |
| `--csl=FILE` | `Option<PathBuf>` | CSL style file (default: chicago-author-date) |

### Wiring

- `--bibliography` present → load file(s), parse with hayagriva, register cite transform in pipeline
- `--bibliography` absent → cite transform not registered (no-op)
- **Metadata fallback**: `bibliography` and `csl` fields from document YAML frontmatter are respected. CLI flags take priority over metadata.

### Supported bibliography formats

| Format | Extension | Via |
|--------|-----------|-----|
| BibTeX | `.bib` | `hayagriva::io::from_biblatex_str()` |
| BibLaTeX | `.bib` | `hayagriva::io::from_biblatex_str()` |
| Hayagriva YAML | `.yml` | `hayagriva::io::from_yaml_str()` |

Format detected by extension (`.bib` → BibTeX/BibLaTeX, `.yml`/`.yaml` → Hayagriva YAML).

## Piece 4 — WASM Compatibility

### Constraint

`docmux-wasm` compiles to `wasm32-unknown-unknown`. The cite transform must not break this.

### Strategy

- **File I/O in CLI only**: the CLI reads `.bib` files from disk and passes content as `&str` to the transform. The transform crate has zero filesystem access.
- **Transform receives parsed data**: `CiteTransformConfig` holds a `hayagriva::Library` (already parsed), not file paths.
- **Feature gate if needed**: if hayagriva has any non-WASM-compatible transitive deps, gate it behind a cargo feature (`csl`) that is enabled in CLI but disabled in WASM. Fallback: WASM build skips cite transform entirely (citations pass through unresolved).

### Validation spike

Before full implementation, add hayagriva as a dep and run:
```sh
cargo build --target wasm32-unknown-unknown -p docmux-transform-cite
```
If this fails, apply feature gating.

## Testing

### Unit tests — Markdown reader
- Parse `[@key]` → single `CiteItem`
- Parse `[@k1; @k2]` → two `CiteItem`s
- Parse `[-@key]` → `CitationMode::SuppressAuthor`
- Parse `@key` inline → `CitationMode::AuthorOnly`
- Parse prefix/suffix: `[see @key, p. 10]`
- No false positives on email addresses (`user@example.com`)

### Unit tests — Cite transform
- Resolve known key → correct formatted string
- Unknown key → `[?key]` + warning
- Multiple citations in one node
- Bibliography inserted at `Div#refs` when present
- Bibliography appended at end when no `Div#refs`
- Empty library + citations → all `[?key]`

### Golden file tests
- `citations.md` + `refs.bib` → HTML with resolved cites + bibliography
- `citations.md` without `--bibliography` → cites unresolved
- `citations.md` with `Div#refs` placeholder → bibliography at correct position

### WASM smoke test
- `cargo build --target wasm32-unknown-unknown -p docmux-transform-cite` succeeds

### CLI integration test
- `docmux input.md -o output.html --bibliography=refs.bib` end-to-end
- `docmux input.md -o output.html --bibliography=refs.bib --csl=ieee.csl` with custom style

## Dependencies

### New crate dependency
- `hayagriva` (MIT OR Apache-2.0) — CSL processing, BibTeX/YAML parsing
  - Brings: `citationberg`, `biblatex`, `serde_yaml`, `url`, `unicode-segmentation`
  - All permissively licensed, no GPL/LGPL contamination

### Crate boundary rules
- `docmux-transform-cite` depends on: `docmux-ast`, `docmux-core`, `hayagriva`
- `docmux-cli` depends on: `docmux-transform-cite` (already does)
- No cross-dependencies between transform and readers/writers

## Out of scope

- CSL JSON / CSL YAML bibliography input (future)
- `--nocite` flag (future)
- Footnote-style citations (future — requires writer awareness)
- DOCX/HTML reader citation parsing (future)
- Localized CSL styles bundled in binary (future — download or provide via `--csl`)
