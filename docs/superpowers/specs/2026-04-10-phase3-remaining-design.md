# Phase 3 Remaining — Design Spec

> Date: 2026-04-10
> Scope: Math transform, highlight enhancements, cite completion
> Order: Math → Highlight → Cite

## 1. Math Transform (`docmux-transform-math`)

### 1.1 Overview

A transform that converts math notation between formats. The AST stores math as opaque strings (typically LaTeX). This transform rewrites those strings when source and target notations differ.

Three conversion paths:
- **LaTeX → Typst**: when output format is Typst
- **Typst → LaTeX**: when input is Typst and output is HTML/LaTeX/Markdown
- **LaTeX → MathML**: when `--math=mathml` (server-side rendering, no JS)

When source and target use the same notation, the transform is a no-op.

### 1.2 Tokenizer (shared)

A lightweight LaTeX math tokenizer (not a full LaTeX parser). Token types:
- `Command(name)` — `\frac`, `\alpha`, `\sqrt`, etc.
- `BraceGroup(tokens)` — `{...}` content
- `Text(s)` — letters, numbers, operators, whitespace
- `SubScript` / `SuperScript` — `_` and `^`
- `Environment(name, body)` — `\begin{name}...\end{name}`

The tokenizer is reused by all three conversion paths.

### 1.3 LaTeX → Typst mapper

Command mapping table:

| Category | LaTeX | Typst |
|----------|-------|-------|
| Greek | `\alpha`, `\beta`, ... | `alpha`, `beta`, ... (strip `\`) |
| Functions | `\frac{a}{b}` | `(a)/(b)` |
| | `\sqrt{x}` | `sqrt(x)` |
| | `\sqrt[n]{x}` | `root(n, x)` |
| Operators | `\sum`, `\prod`, `\int` | `sum`, `product`, `integral` |
| Decorations | `\hat{x}`, `\bar{x}`, `\vec{x}` | `hat(x)`, `overline(x)`, `arrow(x)` |
| Environments | `\begin{pmatrix}...\end{pmatrix}` | `mat(delim: "(", ...)` |
| | `\begin{cases}...\end{cases}` | `cases(...)` |
| Sets | `\mathbb{R}`, `\mathbb{N}` | `RR`, `NN` |
| Spacing | `\quad`, `\,` | `quad`, `thin` |
| Pass-through | Unrecognized commands | Emitted as-is |

### 1.4 Typst → LaTeX mapper

Inverse of the table above. Typst function-call syntax `func(args)` mapped back to LaTeX `\func{args}`.

### 1.5 LaTeX → MathML emitter

Converts LaTeX math tokens to MathML XML:

| LaTeX | MathML |
|-------|--------|
| `\frac{a}{b}` | `<mfrac><mi>a</mi><mi>b</mi></mfrac>` |
| `\sqrt{x}` | `<msqrt><mi>x</mi></msqrt>` |
| `x^2` | `<msup><mi>x</mi><mn>2</mn></msup>` |
| `x_i` | `<msub><mi>x</mi><mi>i</mi></msub>` |
| `\alpha` | `<mi>α</mi>` (Unicode mapping) |
| Display math | `<math display="block">...</math>` |
| Inline math | `<math display="inline">...</math>` |
| Unrecognized | `<mtext>\command</mtext>` (graceful degradation) |

### 1.6 Transform API

```rust
pub enum MathTarget {
    Typst,
    LaTeX,
    MathML,
    None, // no-op
}

pub enum MathNotation {
    LaTeX,
    Typst,
}

pub struct MathTransform {
    pub target_format: MathTarget,
    pub source_notation: MathNotation,
}

impl Transform for MathTransform {
    fn transform(&self, doc: &mut Document) -> Result<(), ConvertError>;
}
```

The CLI determines `source_notation` from the input format and `target_format` from the output format + `--math` flag.

### 1.7 Crate structure

```
crates/docmux-transform-math/src/
├── lib.rs              # Transform impl, orchestration
├── tokenizer.rs        # LaTeX math tokenizer
├── latex_to_typst.rs   # LaTeX → Typst mapper
├── typst_to_latex.rs   # Typst → LaTeX mapper
├── latex_to_mathml.rs  # LaTeX → MathML emitter
└── tables.rs           # Command mapping tables
```

### 1.8 Core changes

Add `MathML` variant to `MathEngine` enum in `docmux-core`:

```rust
pub enum MathEngine {
    KaTeX,
    MathJax,
    MathML,  // new
    Raw,
}
```

### 1.9 HTML writer changes

When `math_engine == MathML`:
- Do not inject KaTeX/MathJax scripts
- Emit `MathInline`/`MathBlock` content directly (already converted to MathML by the transform)

### 1.10 CLI changes

- Accept `mathml` in `--math` flag (already listed in value_parser, just needs MathML variant)
- Register `MathTransform` in the pipeline when output=Typst or `--math=mathml`

### 1.11 Testing

- Unit tests per module: tokenizer, each mapper, MathML emitter
- Golden files: `math-latex-to-typst.md → .typ`, `math-typst-to-html.typ → .html` (with mathml), `math-latex-to-mathml.md → .html`
- Edge cases: nested fractions, empty groups, unrecognized commands (pass-through)

---

## 2. Highlight — Line Numbers + Line Highlighting

### 2.1 Line numbers

Activated via `.numberLines` class on fenced code blocks (pandoc convention):

```markdown
```{.python .numberLines}
code here
```
```

Optional `startFrom` attribute: `{.python .numberLines startFrom="5"}`.

**HTML output:**
```html
<pre><code class="language-python sourceCode numberLines">
<span class="line-number">1</span> <span style="...">def</span> hello():
<span class="line-number">2</span>     print("world")
</code></pre>
```

Default CSS for `.line-number`: gray color, right padding, `user-select: none`.

**LaTeX output:** line counter in `alltt` environment, numbers in left margin.

### 2.2 Line highlighting

Activated via `highlight` attribute:

```markdown
```{.python highlight="2,4-6"}
code here
```
```

Attribute value format: comma-separated line numbers and ranges (e.g., `"2,4-6,10"`).

**HTML output:** highlighted lines wrapped in `<span class="highlight-line">...</span>` with subtle background color (CSS default included).

**LaTeX output:** `\colorbox` or background color for marked lines.

### 2.3 Range parser

A helper function that parses the highlight attribute value:

```rust
fn parse_line_ranges(attr: &str) -> Vec<std::ops::RangeInclusive<u32>>
```

Parses `"2,4-6,10"` into `[2..=2, 4..=6, 10..=10]`.

Lives in `docmux-highlight`.

### 2.4 API changes to `docmux-highlight`

```rust
pub struct LineOptions {
    pub number_lines: bool,
    pub start_from: u32,         // default 1
    pub highlighted_lines: Vec<RangeInclusive<u32>>,
}
```

The `highlight()` function signature does not change — line options are consumed by the writers when rendering the token output. The writers read `CodeBlock.attrs` to extract `.numberLines`, `startFrom`, and `highlight`.

### 2.5 Crate changes

- **`docmux-highlight`**: add `LineOptions`, `parse_line_ranges()` helper
- **`docmux-writer-html`**: read attrs, render line numbers and highlight spans
- **`docmux-writer-latex`**: read attrs, render line numbers and highlight with colorbox
- **No changes** to AST or `docmux-core`

### 2.6 Testing

- Unit tests: `parse_line_ranges` with various inputs (single, range, mixed, empty, invalid)
- Golden files: code block with numberLines, code block with highlight, combined
- Both HTML and LaTeX output verified

---

## 3. Cite — Prefix/Suffix + nocite

### 3.1 Prefix/suffix forwarding

The markdown reader already parses `[see @smith2020, p. 42]` into:
```rust
CiteItem { key: "smith2020", prefix: Some("see"), suffix: Some("p. 42") }
```

The transform (line 315 TODO) discards these. Fix:

- Map `CiteItem.prefix` to hayagriva's `CitationItem` prefix API
- Map `CiteItem.suffix` to hayagriva's `CitationItem` suffix API
- If hayagriva doesn't support affixes directly on `CitationItem`, prepend prefix and append suffix to the formatted output as fallback

### 3.2 `--nocite` flag

Pandoc semantics: include bibliography entries without in-text citation.

**CLI:**
```
--nocite KEY    (repeatable)
```

Special value `@*` includes all entries from the bibliography.

**Metadata fallback:** `nocite: [@key1, @key2]` in YAML frontmatter.

**Implementation:**
- After the "collect" pass, add nocite keys to the `BibliographyDriver` as entries that appear in the bibliography but not in the text
- `@*` adds all entries from the loaded `Library`
- Nocite entries appear in the final bibliography div but produce no inline text

### 3.3 Crate changes

- **`docmux-transform-cite`**: forward prefix/suffix, handle nocite entries
- **`docmux-cli`**: add `--nocite` flag (repeatable), parse from metadata
- **`docmux-ast`**: no changes (CiteItem already has prefix/suffix fields)

### 3.4 Testing

- Golden files: citation with prefix/suffix visible in output (e.g., "(see Smith 2020, p. 42)")
- Golden file: nocite entry appears in bibliography but not in text
- Golden file: `@*` includes all bibliography entries
- Unit tests: nocite with unknown key produces warning

---

## 4. Roadmap Updates

### Add to Phase 4

- OMML math output in DOCX writer (LaTeX → Office Math Markup Language)
- Highlight: load custom `.tmTheme` theme files
- Highlight: per-code-block theme selection

### Phase 3 completion

Mark as done when implemented:
- `docmux-transform-math` — LaTeX ↔ Typst + MathML
- Highlight line numbers + line highlighting
- Cite prefix/suffix + `--nocite`

---

## Implementation Order

1. **Math transform** — new crate implementation, core enum change, CLI + writer integration
2. **Highlight** — additive features on existing crate, writer changes
3. **Cite** — focused changes in existing transform + CLI flag

## Out of Scope

- OMML math (Phase 4)
- Custom highlight themes (Phase 4)
- Per-block highlight themes (Phase 4)
- Full LaTeX math parser (only token-level mapping)
- Typst math syntax validation
- MathML → LaTeX (reverse direction for HTML reader)
