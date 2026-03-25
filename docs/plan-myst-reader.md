# Plan: docmux-reader-myst

## Strategy

Delegate base CommonMark parsing to `MarkdownReader`, layer MyST extensions via pre/post-processing. No comrak duplication.

```
Input → preprocess(extract directives, labels, role markers)
      → MarkdownReader::read(cleaned source)
      → postprocess(replace markers → AST nodes, transform roles, attach labels)
      → Document
```

## Steps

### 1. Cargo.toml setup
Add `docmux-reader-markdown = { workspace = true }` to `crates/docmux-reader-myst/Cargo.toml`.

### 2. Pre-processor (~80 lines)
Scan raw source line-by-line:

- **Directives** `:::{name} [argument]` ... `:::` → extract into `Vec<DirectiveBlock>`, replace with `DOCMUX_MYST_DIR_N` marker paragraph
  - Track nesting via colon count (3+ colons, closing fence ≥ opening count)
  - Parse `:key: value` option lines at start of directive body
  - Directive content is the rest (parsed recursively later)

- **Labels** `(my-label)=` → extract into `Vec<String>`, replace with `DOCMUX_MYST_LABEL_N` marker

### 3. Directive → AST mapping (~60 lines)

| Directive | AST Node |
|-----------|----------|
| `note`, `warning`, `tip`, `important`, `caution` | `Block::Admonition { kind, title, content }` |
| `admonition` | `Block::Admonition { kind: Custom(arg), ... }` |
| `figure` | `Block::Figure { image(arg), caption(content), label(option) }` |
| `code`, `code-block` | `Block::CodeBlock { language(arg), content }` |
| `math` | `Block::MathBlock { content, label(option) }` |
| anything else | `Block::Div { attrs: {classes: [name]}, content }` |

Directive content is parsed recursively via `parse_myst_content()` (handles nested directives).

### 4. Role → Inline mapping (~50 lines)

Pattern detection: `Text("...{role}") + Code("text")` (comrak produces this from `` {role}`text` ``).

| Role | AST Node |
|------|----------|
| `ref`, `numref`, `eq` | `CrossRef { target, form }` |
| `doc`, `download` | `Link { url: text }` |
| `math` | `MathInline { value }` |
| `sub` | `Subscript` |
| `sup` | `Superscript` |
| `cite:p`, `cite` | `Citation { mode: Normal }` |
| `cite:t` | `Citation { mode: AuthorOnly }` |
| unknown | `Span { attrs: {classes: ["role-{name}"]} }` |

### 5. Label attachment (~20 lines)
Find `DOCMUX_MYST_LABEL_N` marker paragraphs, apply label to next sibling block (Heading.id, Figure.label, MathBlock.label, etc.), remove marker.

### 6. CLI registration (~3 lines)
Add `MystReader` to `build_registry()` in `docmux-cli/src/main.rs`. Extension: `.myst`.

### 7. Tests (~150 lines)

**Directives:**
- `:::{note}` → Admonition(Note)
- `:::{warning}` with title argument
- Directive with `:key: value` options
- Nested directives (4+ colons)
- `:::{code-block} python` → CodeBlock
- `:::{math}` → MathBlock
- Unknown directive → Div

**Roles:**
- `` {ref}`target` `` → CrossRef
- `` {math}`x^2` `` → MathInline
- `` {sub}`text` `` → Subscript
- Unknown role → Span

**Labels:**
- `(my-label)=` before heading → sets heading ID
- `(fig:x)=` before figure directive → sets figure label

**Integration:**
- Full document with frontmatter + directives + roles + regular markdown
- Content inside directives parsed as markdown (bold, links, etc.)

## Estimated size
~350-400 lines (implementation) + ~150 lines (tests) = ~500-550 total.

## Not in scope (Phase 3+)
- Substitution definitions (`|name|`)
- Target/reference syntax variations (`<target>`)
- MyST-specific frontmatter (jupytext, kernelspec)
- Directive options as YAML blocks
- `eval-rst` directive
