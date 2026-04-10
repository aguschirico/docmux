# MyST Writer Design

## Summary

Add a `docmux-writer-myst` crate that serializes the docmux AST to [MyST (Markedly Structured Text)](https://mystmd.org/) Markdown. This closes the last gap identified in the pandoc-parity audit: the ability to write MyST output.

## Approach

**Option B (chosen):** Use dollar-sign math (`$...$`, `$$...$$`) and MyST directives/roles for features that have no CommonMark equivalent. This matches what MyST users actually write and what tools like JupyterBook expect.

## AST Node Mapping

### Block-level

| AST Node | MyST Output |
|---|---|
| `Paragraph` | Same as Markdown |
| `Heading` (no id) | `# Title` (ATX headings) |
| `Heading` (with id) | `(id)=` on line before, then `# Title` |
| `CodeBlock` (no caption/label) | `` ```lang `` fenced code |
| `CodeBlock` (with caption or label) | `:::{code-block} lang` directive with `:caption:` / `:name:` options |
| `MathBlock` (no label) | `$$...$$` |
| `MathBlock` (with label) | `(label)=` on line before, then `$$...$$` |
| `BlockQuote` | `> ...` (same as Markdown) |
| `List` | Same as Markdown |
| `Table` | GFM pipe tables (same as Markdown) |
| `Figure` | `:::{figure} url` directive with caption as body text, `:name:` for label, `:alt:` for alt text |
| `ThematicBreak` | `---` |
| `RawBlock` | Same as Markdown |
| `Admonition` | `:::{kind}` directive (e.g. `:::{note}`, `:::{warning}`) with optional title |
| `DefinitionList` | Same as Markdown (term + `:   definition`) |
| `FootnoteDef` | `[^id]: ...` (same as Markdown) |
| `Div` | `:::{directive} ...` with attrs mapped to options |

### Inline-level

| AST Node | MyST Output |
|---|---|
| `Text`, `Emphasis`, `Strong`, `Strikethrough` | Same as Markdown |
| `Code` | Same as Markdown |
| `MathInline` | `$...$` |
| `Link`, `Image` | Same as Markdown |
| `Citation` (Normal mode) | `` {cite:p}`key1,key2` `` |
| `Citation` (AuthorOnly mode) | `` {cite:t}`key` `` |
| `Citation` (SuppressAuthor mode) | `` {cite:p}`key` `` (same as Normal — MyST doesn't distinguish) |
| `FootnoteRef` | `[^id]` (same as Markdown) |
| `CrossRef` (Number) | `` {numref}`target` `` |
| `CrossRef` (NumberWithType) | `` {numref}`target` `` |
| `CrossRef` (Page / Custom) | `` {ref}`target` `` |
| `RawInline` | Same as Markdown |
| `Superscript` | `` {sup}`text` `` |
| `Subscript` | `` {sub}`text` `` |
| `SmallCaps` | `[text]{.smallcaps}` (no MyST native — keep pandoc span) |
| `Underline` | `` {u}`text` `` |
| `Span` | `[content]{attrs}` (same as Markdown) |
| `Quoted` | Smart quotes (same as Markdown) |
| `SoftBreak`, `HardBreak` | Same as Markdown |

## Directive Format

MyST directives follow this structure:

```
:::{directive-name} argument
:option: value
:another-option: value

Body content here.
:::
```

### Admonition example

```
:::{warning} Custom Title
Body text of the warning.
:::
```

If no custom title, omit the argument:

```
:::{note}
Default note content.
:::
```

### Figure example

```
:::{figure} photo.png
:alt: A photo
:name: fig-photo

This is the caption text.
:::
```

### Code-block example (only when caption or label present)

```
:::{code-block} python
:caption: Hello world example
:name: code-hello

print("hello")
:::
```

## Label/Target Syntax

MyST uses `(label)=` on the line immediately before the target block:

```
(my-section)=
## My Section

(eq:einstein)=
$$
E = mc^2
$$
```

## Crate Structure

```
crates/docmux-writer-myst/
  Cargo.toml
  src/
    lib.rs          # MystWriter struct, Writer trait impl, all rendering logic
```

Dependencies: `docmux-ast`, `docmux-core`, `docmux-template` (for standalone mode).

## Writer Trait Implementation

```rust
impl Writer for MystWriter {
    fn format(&self) -> &str { "myst" }
    fn default_extension(&self) -> &str { "md" }
    fn write(&self, doc: &Document, opts: &WriteOptions) -> Result<String>;
}
```

Note: `default_extension` returns `"md"` since MyST files use the `.md` extension. The format name `"myst"` distinguishes it from the regular Markdown writer in the registry.

## Registration

Add to `docmux-cli/src/main.rs`:
```rust
reg.add_writer(Box::new(MystWriter::new()));
```

Add to `docmux-wasm` if it has a format registry.

## Testing Strategy

Unit tests in `src/lib.rs` covering:
- Basic blocks (paragraph, heading, code block, math, list, table, blockquote)
- Heading with id (label target syntax)
- Code block with caption/label (directive syntax)
- Math block with label (label target syntax)
- Admonitions (all kinds + custom title)
- Figures (with/without caption, label, alt)
- Cross-references (numref vs ref)
- Citations (cite:p vs cite:t)
- Inline roles (sub, sup, underline)
- Definition list, footnotes, divs
- Standalone mode with frontmatter

## Out of Scope

- MyST-specific frontmatter fields beyond standard metadata (e.g. `kernelspec`, `jupytext`)
- Executable code cells (Jupyter notebook integration)
- MyST cross-document references (`{doc}`, `{download}`)
