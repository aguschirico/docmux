# docmux vs pandoc — Exhaustive Parity Check

> Generated: 2026-03-25
> Scope: pandoc 3.x feature set vs docmux current state
> Purpose: Identify every gap, classify by impact, assign to phases

---

## 1. Format Coverage

### Input Formats (Readers)

| Format | pandoc | docmux | Gap | Phase |
|--------|--------|--------|-----|-------|
| Markdown (CommonMark + GFM) | `markdown`, `gfm`, `commonmark` | `docmux-reader-markdown` | **ok** — GFM via comrak | — |
| LaTeX | `latex` | `docmux-reader-latex` (53 tests) | **ok** — practical subset | — |
| Typst | `typst` | `docmux-reader-typst` (81 tests) | **ok** | — |
| MyST Markdown | via extensions | Planned (Phase 2) | **planned** | P2 |
| HTML | `html` | **MISSING** | Important for roundtrips, web scraping | P3 |
| DOCX | `docx` | **MISSING** | High demand format | P3 |
| reStructuredText | `rst` | **MISSING** | Python ecosystem staple | P4 |
| Org Mode | `org` | **MISSING** | Emacs ecosystem | P4 |
| EPUB | `epub` | **MISSING** | E-book format | P5 |
| Textile | `textile` | **MISSING** | Legacy format | P5 |
| MediaWiki | `mediawiki` | **MISSING** | Wikipedia markup | P5 |
| AsciiDoc | `asciidoc` (via asciidoctor) | **MISSING** | Technical docs | P4 |
| Jupyter Notebook | `ipynb` | **MISSING** | Data science | P4 |
| JIRA/Confluence | `jira` | **MISSING** | Enterprise wikis | P5 |
| DokuWiki | `dokuwiki` | **MISSING** | Wiki format | P5 |
| man page | `man` | **MISSING** | Unix docs | P5 |
| BibTeX/BibLaTeX | `bibtex`, `biblatex` | **MISSING** (only within LaTeX) | For cite transform | P3 |
| CSL JSON/YAML | `csljson` | **MISSING** | Bibliography interchange | P3 |
| Djot | `djot` | **MISSING** | pandoc's next-gen markdown | P4 |
| Native JSON | `json` | **MISSING** | AST interchange | P2 |
| RTF | `rtf` | **MISSING** | Legacy format | P5 |
| CSV/TSV | `csv`, `tsv` | **MISSING** | Tabular data | P5 |
| FB2 | `fb2` | **MISSING** | Russian e-book | P5+ |
| Creole | `creole` | **MISSING** | Wiki format | P5+ |
| Haddock | `haddock` | **MISSING** | Haskell docs | P5+ |

### Output Formats (Writers)

| Format | pandoc | docmux | Gap | Phase |
|--------|--------|--------|-----|-------|
| HTML5 | `html5` | `docmux-writer-html` (6 tests) | **ok** | — |
| LaTeX | `latex` | `docmux-writer-latex` (10 tests) | **ok** | — |
| Typst | `typst` | Planned (Phase 2) | **planned** | P2 |
| DOCX | `docx` | Planned (Phase 2) | **planned** | P2 |
| Markdown | `markdown`, `gfm`, `commonmark` | **MISSING** | Roundtrip, normalization | P3 |
| PDF | `pdf` (via LaTeX/wkhtmltopdf) | Non-goal (use LaTeX→pdflatex) | **by design** | — |
| EPUB | `epub2`, `epub3` | **MISSING** | E-book output | P4 |
| Beamer | `beamer` | **MISSING** | LaTeX presentations | P4 |
| reveal.js | `revealjs` | **MISSING** | HTML presentations | P4 |
| PowerPoint | `pptx` | **MISSING** | Presentation format | P5 |
| ODT | `odt` | **MISSING** | Open document | P4 |
| reStructuredText | `rst` | **MISSING** | Python docs output | P4 |
| Plain text | `plain` | **MISSING** | Stripped text output | P3 |
| AsciiDoc | `asciidoc`, `asciidoctor` | **MISSING** | Technical docs | P4 |
| Org Mode | `org` | **MISSING** | Emacs output | P5 |
| man page | `man` | **MISSING** | Unix man pages | P5 |
| Texinfo | `texinfo` | **MISSING** | GNU docs | P5+ |
| ConTeXt | `context` | **MISSING** | Alternative to LaTeX | P5+ |
| ICML | `icml` | **MISSING** | InDesign interchange | P5+ |
| TEI | `tei` | **MISSING** | Text Encoding Initiative | P5+ |
| MediaWiki | `mediawiki` | **MISSING** | Wikipedia output | P5 |
| JIRA | `jira` | **MISSING** | Enterprise output | P5 |
| Native JSON | `json` | **MISSING** | AST interchange | P2 |

---

## 2. AST Nodes

### Block-level Nodes

| Node | pandoc | docmux | Gap | Impact | Phase |
|------|--------|--------|-----|--------|-------|
| Paragraph | `Para [Inline]` | `Paragraph { content }` | **ok** | — | — |
| Plain (no `<p>`) | `Plain [Inline]` | **MISSING** | Used for tight lists — items without paragraph wrapping. Without this, all lists render as loose (extra spacing). | HIGH | P2 |
| Heading | `Header Int Attr [Inline]` | `Heading { level, id, content }` | **partial** — has `id` but missing `classes` and `key_values` | MEDIUM | P2 |
| Code block | `CodeBlock Attr String` | `CodeBlock { language, content, caption, label }` | **partial** — has extra fields (good) but no generic `Attr` for classes/kv | MEDIUM | P2 |
| Math block | _(no direct equivalent)_ | `MathBlock { content, label }` | **docmux extra** — pandoc embeds in `Para [Math DisplayMath ...]` | — | — |
| Block quote | `BlockQuote [Block]` | `BlockQuote { content }` | **ok** | — | — |
| Ordered list | `OrderedList ListAttributes [[Block]]` | `List { ordered: true, start, items }` | **partial** — missing `ListNumberStyle` (Decimal, LowerAlpha, UpperAlpha, LowerRoman, UpperRoman) and `ListNumberDelim` (Period, OneParen, TwoParens). Missing tight/loose. | HIGH | P2 |
| Bullet list | `BulletList [[Block]]` | `List { ordered: false, items }` | **partial** — missing tight/loose | MEDIUM | P2 |
| Definition list | `DefinitionList [([Inline], [[Block]])]` | `DefinitionList { items }` | **ok** | — | — |
| Table | `Table Attr Caption [ColSpec] TableHead [TableBody] TableFoot` | `Table { caption, label, columns, header, rows }` | **partial** — see Table section below | MEDIUM | P3 |
| Figure | `Figure Attr Caption [Block]` | `Figure { image, caption, label }` | **partial** — pandoc's figure contains arbitrary blocks (not just image). Missing `Attr`. | MEDIUM | P3 |
| Horizontal rule | `HorizontalRule` | `ThematicBreak` | **ok** | — | — |
| Raw block | `RawBlock Format String` | `RawBlock { format, content }` | **ok** | — | — |
| Div (container) | `Div Attr [Block]` | **MISSING** | Generic block container with attributes. Essential for fenced divs, MyST directives, custom containers. | HIGH | P2 |
| Line block | `LineBlock [[Inline]]` | **MISSING** | Poetry, addresses. Lines with significant line breaks. | LOW | P4 |
| — | — | `Admonition { kind, title, content }` | **docmux extra** — pandoc uses `Div` with classes | — | — |
| — | — | `FootnoteDef { id, content }` | **docmux extra** — pandoc inlines note content via `Note` | — | — |

### Inline-level Nodes

| Node | pandoc | docmux | Gap | Impact | Phase |
|------|--------|--------|-----|--------|-------|
| Text | `Str Text` | `Text { value }` | **ok** | — | — |
| Emphasis | `Emph [Inline]` | `Emphasis { content }` | **ok** | — | — |
| Strong | `Strong [Inline]` | `Strong { content }` | **ok** | — | — |
| Strikethrough | `Strikeout [Inline]` | `Strikethrough { content }` | **ok** | — | — |
| Superscript | `Superscript [Inline]` | `Superscript { content }` | **ok** | — | — |
| Subscript | `Subscript [Inline]` | `Subscript { content }` | **ok** | — | — |
| Small caps | `SmallCaps [Inline]` | `SmallCaps { content }` | **ok** | — | — |
| Inline code | `Code Attr Text` | `Code { value }` | **partial** — missing `Attr` (classes for language hints, key-values) | LOW | P3 |
| Math inline | `Math InlineMath Text` | `MathInline { value }` | **ok** | — | — |
| Link | `Link Attr [Inline] Target` | `Link { url, title, content }` | **partial** — missing `Attr` | LOW | P3 |
| Image | `Image Attr [Inline] Target` | `Image { url, alt, title }` | **partial** — `alt` is `String` not `Vec<Inline>` (loses formatting). Missing `Attr`. | MEDIUM | P3 |
| Citation | `Cite [Citation] [Inline]` | `Citation { keys, prefix, suffix, mode }` | **partial** — pandoc has per-key prefix/suffix and rendered inline fallback. docmux has single prefix/suffix for the whole group. | MEDIUM | P3 |
| Footnote ref | `Note [Block]` | `FootnoteRef { id }` | **different design** — pandoc inlines content, docmux separates. Both valid. | — | — |
| Cross-reference | _(none)_ | `CrossRef { target, form }` | **docmux extra** | — | — |
| Raw inline | `RawInline Format Text` | `RawInline { format, content }` | **ok** | — | — |
| Soft break | `SoftBreak` | `SoftBreak` | **ok** | — | — |
| Hard break | `LineBreak` | `HardBreak` | **ok** | — | — |
| Span | `Span Attr [Inline]` | `Span { content, attrs }` | **ok** | — | — |
| Underline | `Underline [Inline]` | **MISSING** | Added in pandoc-types 1.23. Needed for DOCX, HTML. | MEDIUM | P2 |
| Quoted | `Quoted QuoteType [Inline]` | **MISSING** | Smart quotes: SingleQuote, DoubleQuote. Needed for typographic output. | MEDIUM | P3 |
| Space | `Space` | _(included in Text)_ | **by design** — docmux includes spaces in Text content. Valid choice, simpler AST. | — | — |

---

## 3. Attributes — The Systemic Gap

pandoc attaches `Attr (id, classes, key_values)` to **9 element types**. docmux has `Attributes` only on `Span`.

| Element | pandoc has Attr | docmux has Attr | Phase to fix |
|---------|-----------------|-----------------|-------------|
| Heading | Yes | `id` only (no classes/kv) | P2 |
| CodeBlock | Yes | `language` only (no generic attrs) | P2 |
| Code (inline) | Yes | No | P3 |
| Link | Yes | No | P3 |
| Image | Yes | No | P3 |
| Table | Yes | `label` only | P3 |
| Figure | Yes | `label` only | P3 |
| Div | Yes | N/A (no Div) | P2 |
| Span | Yes | **Yes** | — |

**Proposed solution**: Add `attrs: Option<Attributes>` to Heading, CodeBlock, Table, Figure. Add `Div` block with `Attributes`. Phase 2. Inline attrs (Code, Link, Image) can wait for Phase 3.

---

## 4. Table Structure

### pandoc's table model (pandoc-types 1.23+)

```
Table Attr Caption [ColSpec] TableHead [TableBody] TableFoot
  where
    Caption = (Maybe [Inline], [Block])    -- short caption + long
    ColSpec = (Alignment, ColWidth)
    TableHead = (Attr, [Row])
    TableBody = (Attr, Int, [Row], [Row])  -- header rows + body rows
    TableFoot = (Attr, [Row])
    Row = (Attr, [Cell])
    Cell = (Attr, Alignment, RowSpan, ColSpan, [Block])
```

### docmux's current table

```rust
Table { caption, label, columns, header: Option<Vec<TableCell>>, rows }
TableCell { content, colspan, rowspan }
ColumnSpec { alignment, width }
```

### Gaps

| Feature | pandoc | docmux | Impact | Phase |
|---------|--------|--------|--------|-------|
| Table footer | `TableFoot` | **MISSING** | Needed for proper `<tfoot>` in HTML, DOCX tables | P3 |
| Multiple table bodies | `[TableBody]` with intermediate headers | **MISSING** | Rare but used in complex academic tables | P5 |
| Row attributes | `Row (Attr, [Cell])` | **MISSING** | Useful for highlighting rows | P4 |
| Cell alignment override | `Cell (Attr, Alignment, ...)` | **MISSING** (only per-column) | Per-cell alignment overrides | P4 |
| Cell attributes | `Cell (Attr, ...)` | **MISSING** | For classes, styles on cells | P4 |
| Table attributes | `Table Attr ...` | Only `label` | For classes/id on table element | P3 |
| Short + long caption | `Caption (Maybe [Inline], [Block])` | Single `caption: Option<Vec<Inline>>` | Some formats use short caption in TOT (List of Tables) | P4 |
| Header rows (multiple) | `TableHead (Attr, [Row])` | `header: Option<Vec<TableCell>>` — single row only | Multi-row headers in complex tables | P4 |

---

## 5. List Features

| Feature | pandoc | docmux | Impact | Phase |
|---------|--------|--------|--------|-------|
| Tight vs loose | `Plain` vs `Para` in list items | **MISSING** | Affects HTML rendering significantly | P2 |
| Number style | `Decimal`, `LowerAlpha`, `UpperAlpha`, `LowerRoman`, `UpperRoman`, `DefaultStyle` | Only `ordered: bool` | Academic/legal docs use a), i., A. etc. | P2 |
| Number delimiter | `Period`, `OneParen`, `TwoParens`, `DefaultDelim` | **MISSING** | `1.` vs `1)` vs `(1)` | P2 |
| Example lists | `Example` number style (cross-document numbering) | **MISSING** | Linguistics papers | P5 |
| Task lists | `ListItem { checked: Option<bool> }` | **ok** — same model | — | — |

---

## 6. Metadata

| Feature | pandoc | docmux | Impact | Phase |
|---------|--------|--------|--------|-------|
| Title (plain) | `MetaString` or `MetaInlines` | `title: Option<String>` | **ok** for plain titles | — |
| Title (formatted) | `MetaInlines [Inline]` | **MISSING** — `String` loses `*emphasis*` | LOW | P4 |
| Authors | `MetaList [MetaMap]` | `authors: Vec<Author>` | **ok** — docmux is more structured | — |
| Date | `MetaString` or `MetaInlines` | `date: Option<String>` | **ok** | — |
| Abstract | `MetaBlocks [Block]` | `abstract_text: Option<String>` | **partial** — loses formatting in abstract | MEDIUM | P3 |
| Arbitrary metadata | `MetaMap` (recursive) | `custom: HashMap<String, MetaValue>` | **ok** — equivalent | — |
| MetaInlines | `MetaInlines [Inline]` | **MISSING** from `MetaValue` | LOW | P4 |
| MetaBlocks | `MetaBlocks [Block]` | **MISSING** from `MetaValue` | LOW | P4 |
| Language/lang | `lang` metadata key | Not special-cased | LOW | P3 |
| CSL fields | `csl`, `bibliography`, `nocite` | **MISSING** | Needed for cite transform | P3 |

---

## 7. CLI Features

### Currently implemented in docmux

- `input` (positional)
- `-o, --output`
- `-f, --from`
- `-t, --to`
- `-s, --standalone`

### Missing CLI features (pandoc equivalents)

| Feature | pandoc flag | Impact | Phase |
|---------|------------|--------|-------|
| **Multiple input files** | positional args | Concatenate inputs | P2 |
| **stdin input** | `-` or no file | Common in pipelines | P2 |
| **Metadata** | `-M KEY=VAL` / `--metadata` | Set metadata from CLI | P2 |
| **Variables** | `-V KEY=VAL` / `--variable` | Template variables | P2 |
| **TOC generation** | `--toc` / `--table-of-contents` | Auto-generate table of contents | P3 |
| **TOC depth** | `--toc-depth=N` | Control TOC depth | P3 |
| **Number sections** | `-N` / `--number-sections` | Auto-number headings | P3 |
| **Heading shift** | `--shift-heading-level-by=N` | Shift heading levels | P2 |
| **Template** | `--template=FILE` | Custom output template | P3 |
| **CSS** | `--css=URL` | Stylesheet for HTML output | P2 |
| **Highlight style** | `--highlight-style=STYLE` | Syntax highlighting theme | P3 |
| **No highlight** | `--no-highlight` | Disable syntax highlighting | P3 |
| **Math engine** | `--katex` / `--mathjax` / `--mathml` | Choose math rendering | P2 |
| **Bibliography** | `--bibliography=FILE` | Load .bib file | P3 |
| **CSL style** | `--csl=FILE` | Citation style | P3 |
| **Self-contained** | `--self-contained` / `--embed-resources` | Inline all resources | P4 |
| **Extract media** | `--extract-media=DIR` | Extract embedded media | P4 |
| **Wrap mode** | `--wrap=auto\|none\|preserve` | Output line wrapping | P3 |
| **Columns** | `--columns=N` | Line wrap width | P3 |
| **Tab stop** | `--tab-stop=N` | Tab width | P4 |
| **Filters** | `--filter=PROGRAM` | JSON filter pipeline | P4 |
| **Lua filters** | `--lua-filter=FILE` | Lua AST filter | P4 |
| **List formats** | `--list-input-formats`, `--list-output-formats` | List available formats | P2 |
| **List extensions** | `--list-extensions` | Show supported extensions | P4 |
| **Print template** | `--print-default-template=FORMAT` | Show built-in template | P3 |
| **Verbose/quiet** | `--verbose` / `--quiet` | Output verbosity | P2 |
| **Log file** | `--log=FILE` | Structured logging | P4 |
| **Top-level division** | `--top-level-division=section\|chapter\|part` | LaTeX document structure | P3 |
| **Incremental slides** | `--incremental` | Presentation output | P5 |
| **Slide level** | `--slide-level=N` | Presentation output | P5 |
| **Section divs** | `--section-divs` | Wrap sections in `<section>` | P3 |
| **ID prefix** | `--id-prefix=PREFIX` | Prefix all IDs | P3 |
| **Reference doc** | `--reference-doc=FILE` | Template for DOCX/ODT | P3 |
| **DPI** | `--dpi=N` | Image DPI | P4 |
| **EOL** | `--eol=crlf\|lf\|native` | Line ending style | P3 |
| **Strip comments** | `--strip-comments` | Remove HTML comments | P4 |
| **File scope** | `--file-scope` | Parse each file independently | P4 |
| **Sandbox** | `--sandbox` | Disable external resources | P4 |
| **Trace** | `--trace` | Debug tracing | P4 |
| **Reference links** (markdown output) | `--reference-links` | Use `[text][ref]` style | P3 |
| **ATX headers** (markdown output) | `--atx-headers` | Use `#` headers in output | P3 |
| **DOCX track changes** | `--track-changes=accept\|reject\|all` | DOCX change tracking | P5 |
| **EPUB options** | `--epub-cover-image`, `--epub-metadata`, `--epub-chapter-level` | E-book options | P5 |
| **PDF engine** | `--pdf-engine=PROGRAM` | External PDF engine | Non-goal |

---

## 8. Extensions System

pandoc has ~100+ extensions toggled per format (e.g. `markdown+smart+footnotes-raw_html`). docmux currently has no extension system.

### High-value extensions (used by most people)

| Extension | pandoc | docmux | Phase |
|-----------|--------|--------|-------|
| `smart` | Smart quotes, em-dashes, ellipses | **MISSING** — comrak has no smart punct | P3 |
| `raw_html` | Pass through raw HTML in markdown | **partial** — comrak handles this | — |
| `raw_tex` | Pass through raw LaTeX in markdown | **MISSING** | P3 |
| `footnotes` | `[^id]: ...` syntax | **ok** — comrak extension | — |
| `pipe_tables` | `\| a \| b \|` syntax | **ok** — comrak GFM | — |
| `yaml_metadata_block` | YAML frontmatter | **ok** — first-class | — |
| `fenced_code_blocks` | ``` syntax | **ok** — comrak | — |
| `fenced_code_attributes` | ````{.python .numberLines}``` | **MISSING** — code block attrs | P2 |
| `backtick_code_blocks` | Backtick fences | **ok** — comrak | — |
| `fenced_divs` | `:::` div syntax | **MISSING** — needs `Div` block | P2 |
| `bracketed_spans` | `[text]{.class}` syntax | **MISSING** | P3 |
| `header_attributes` | `# Heading {#id .class}` | **partial** — `id` only, no classes/kv | P2 |
| `auto_identifiers` | Auto-generate heading IDs from text | **MISSING** | P2 |
| `implicit_header_references` | `[Heading text]` links to heading | **MISSING** | P3 |
| `tex_math_dollars` | `$...$` and `$$...$$` | **ok** — comrak extension | — |
| `citations` | `[@key]` syntax | **ok** — in markdown reader | — |
| `task_lists` | `- [x]` / `- [ ]` | **ok** — comrak GFM | — |
| `definition_lists` | Term\n: Definition | **ok** — comrak extension | — |
| `superscript` | `^superscript^` | **MISSING** — comrak doesn't support | P3 |
| `subscript` | `~subscript~` | **MISSING** — comrak doesn't support | P3 |
| `strikeout` | `~~strikeout~~` | **ok** — comrak GFM | — |
| `fancy_lists` | `a.`, `i.`, `A)`, etc. | **MISSING** — needs list styles | P2 |
| `startnum` | Lists starting at specific number | **ok** — `start: Option<u32>` | — |
| `emoji` | `:emoji_name:` | **MISSING** | P4 |
| `link_attributes` | `[text](url){.class}` | **MISSING** | P3 |
| `inline_code_attributes` | `` `code`{.haskell} `` | **MISSING** | P3 |
| `line_blocks` | `\| line 1` syntax | **MISSING** — needs `LineBlock` | P4 |
| `all_symbols_escapable` | `\*` etc. | **ok** — comrak | — |
| `abbreviations` | `*[HTML]: Hyper Text...` | **MISSING** | P5 |
| `example_lists` | `(@)` numbered examples | **MISSING** | P5 |
| `implicit_figures` | Standalone image = figure | **ok** — docmux has Figure block | — |

### Medium-value extensions

| Extension | pandoc | docmux | Phase |
|-----------|--------|--------|-------|
| `grid_tables` | Complex tables with grid syntax | **MISSING** | P4 |
| `multiline_tables` | Multi-line row content | **MISSING** | P4 |
| `simple_tables` | Simple alignment-based tables | **MISSING** | P4 |
| `table_captions` | `Table: caption` syntax | **MISSING** | P3 |
| `pandoc_title_block` | `% Title\n% Author\n% Date` | **MISSING** — we use YAML instead | P5 |
| `mmd_title_block` | MultiMarkdown title block | **MISSING** | P5 |
| `mmd_header_identifiers` | MMD heading IDs | **MISSING** | P5 |
| `ascii_identifiers` | ASCII-only auto IDs | **MISSING** | P4 |
| `gfm_auto_identifiers` | GitHub-style auto IDs | **MISSING** | P2 |
| `raw_attribute` | `` `\foo`{=latex} `` syntax | **MISSING** | P3 |
| `escaped_line_breaks` | `\` at end of line = hard break | **ok** — comrak | — |
| `space_in_atx_header` | Require space after `#` | **ok** — comrak | — |
| `lists_without_preceding_blankline` | Lists without blank line before | **ok** — comrak handles this | — |
| `blank_before_header` | Require blank line before heading | **ok** — comrak option | — |
| `blank_before_blockquote` | Require blank line before quote | **ok** — comrak option | — |

---

## 9. Template System

| Feature | pandoc | docmux | Phase |
|---------|--------|--------|-------|
| Built-in templates per format | Yes (every output format has a default) | **MISSING** — only hardcoded standalone HTML/LaTeX headers | P3 |
| Custom template files | `--template=FILE` | `WriteOptions::template` field exists but unused | P3 |
| Variable interpolation | `$title$`, `$body$`, `$for(author)$...$endfor$` | **MISSING** | P3 |
| Conditionals | `$if(toc)$...$endif$` | **MISSING** | P3 |
| Loops | `$for(x)$...$endfor$` | **MISSING** | P3 |
| Partials | `$partial("header.html")$` | **MISSING** | P4 |
| Default data files | Built-in CSS, templates | **MISSING** | P3 |
| Template variable escaping | Format-aware escaping | **MISSING** | P3 |

---

## 10. Citation Processing

| Feature | pandoc (citeproc) | docmux | Phase |
|---------|-------------------|--------|-------|
| CSL processing engine | Built-in citeproc-hs | `docmux-transform-cite` planned | P3 |
| BibTeX/BibLaTeX parsing | Yes | AST has `Bibliography`/`BibEntry` but no parser | P3 |
| CSL style files | Full CSL 1.0.2 support | **MISSING** | P3 |
| Localization | CSL locales | **MISSING** | P4 |
| Citation linking | Link citations to bibliography | **MISSING** | P3 |
| `nocite` | `nocite: [@*]` metadata | **MISSING** | P3 |
| Multiple bibliographies | Yes | **MISSING** | P5 |
| Citation-key completion | Via editors | N/A | — |

---

## 11. Filter System

| Feature | pandoc | docmux | Phase |
|---------|--------|--------|-------|
| JSON filters | stdin/stdout pipe, any language | **MISSING** | P4 |
| Lua filters | Embedded interpreter, fast | **MISSING** | P4 |
| WASM filters | Not built-in | Natural fit for docmux (WASM-first) | P4 |
| Transform trait | N/A | `Transform` trait exists, crossref implemented | — |
| Filter chaining | `--filter F1 --filter F2` | `Pipeline::with_transform()` exists | — |

**Note**: docmux's `Transform` trait is the foundation. The question is whether to expose it as an external plugin interface (JSON/WASM filters) or keep it Rust-only.

---

## 12. Syntax Highlighting

| Feature | pandoc (skylighting) | docmux | Phase |
|---------|---------------------|--------|-------|
| Built-in highlighting | ~140 languages via Kate definitions | **MISSING** — just passes language tag through | P3 |
| Highlight styles | pygments, kate, monochrome, espresso, zenburn, haddock, tango, breezeDark | **MISSING** | P3 |
| Custom syntax definitions | Kate XML files | **MISSING** | P5 |
| Line numbers | Yes | **MISSING** | P3 |
| Line highlighting | `{.numberLines startFrom="5" .hl-3-5}` | **MISSING** | P4 |

**Note**: For HTML output, highlighting can be deferred to the client (highlight.js, Prism, Shiki). For LaTeX, the `listings` package handles it. This is lower priority than it seems — the writer just needs to emit the right markup for the client-side highlighter.

---

## 13. Media Handling

| Feature | pandoc | docmux | Phase |
|---------|--------|--------|-------|
| Extract media from DOCX/EPUB | `--extract-media=DIR` | **MISSING** | P4 |
| Self-contained output | `--embed-resources` (data URIs) | **MISSING** | P4 |
| Resource path | `--resource-path=PATH` | **MISSING** | P3 |
| Default image extension | `--default-image-extension=EXT` | **MISSING** | P4 |
| DPI setting | `--dpi=N` | **MISSING** | P4 |

---

## 14. Miscellaneous Features

| Feature | pandoc | docmux | Impact | Phase |
|---------|--------|--------|--------|-------|
| AST JSON dump | `pandoc -t json` | **MISSING** — `Document` has serde but no CLI flag | P2 |
| Watch mode | No (use entr/fswatch) | Planned (Phase 4) | P4 |
| Incremental conversion | No | Not planned | — |
| Encoding detection | Auto | Assumes UTF-8 | LOW | P4 |
| BOM handling | Yes | **MISSING** | LOW | P3 |
| Multiple input concatenation | `pandoc a.md b.md` | **MISSING** — single input only | P2 |
| Warnings/diagnostics | `--verbose`, structured | `ParseWarning` exists but limited | P3 |
| WASM bindings | No (Haskell) | **ok** — first-class | — |
| npm package | No | Planned (Phase 3) | P3 |

---

## Phase Plan — Prioritized

### Phase 2 (Current) — Core AST Parity + Essential CLI

**AST changes:**
1. Add `Div` block with `Attributes` — unblocks MyST reader, fenced divs
2. Add `attrs: Option<Attributes>` to `Heading`, `CodeBlock`, `Figure`, `Table`
3. Add `tight: bool` to `List` — tight vs loose rendering
4. Add `ListStyle` enum — number styles and delimiters
5. Add `Underline` inline
6. Add `Plain` block variant (or use `tight` flag — see tradeoffs below)
7. Native JSON reader/writer — `docmux -t json` for AST interchange/debugging

**Reader/Writer:**
8. Typst writer (already planned)
9. Update existing writers for new AST nodes (Div, Underline, attrs)
10. Update existing readers to populate new fields (attrs, tight, list style)

**CLI:**
11. `-M KEY=VAL` for metadata
12. `-V KEY=VAL` for variables
13. `--math=katex|mathjax|mathml|raw` flag
14. `--css=URL` for HTML output
15. `--list-input-formats`, `--list-output-formats`
16. `--dump-ast` or `-t json` for AST debugging
17. Multiple input files / stdin (`-`)
18. `--shift-heading-level-by=N`
19. `--verbose` / `--quiet`

**Extensions (markdown reader):**
20. Auto-generate heading IDs
21. GFM-style auto identifiers
22. Header attributes `{#id .class key=val}` (requires AST attrs)
23. Fenced code attributes ````{.python .numberLines}``` (requires AST attrs)

### Phase 3 — Production Features

**AST changes:**
24. `Quoted` inline (smart quotes: SingleQuote, DoubleQuote)
25. Attributes on inline `Code`, `Link`, `Image`
26. Image `alt` as `Vec<Inline>` instead of `String`
27. Per-key prefix/suffix in `Citation` (matching pandoc's model)
28. `abstract_text` as `Vec<Block>` instead of `String` (formatted abstract)
29. Table footer (`foot: Option<Vec<TableCell>>`)
30. Table attrs + figure attrs

**Reader/Writer:**
31. Markdown writer — critical for roundtrip, AST normalization
32. Plain text writer — stripped output
33. HTML reader — for web content, HTML→LaTeX etc.
34. BibTeX/BibLaTeX parser for bibliography files
35. DOCX writer (already planned)

**Transforms:**
36. `docmux-transform-cite` — CSL citation processing
37. `docmux-transform-toc` — table of contents generation
38. `docmux-transform-number-sections` — heading numbering
39. `docmux-transform-math` (already planned)

**CLI:**
40. `--toc` and `--toc-depth=N`
41. `-N` / `--number-sections`
42. `--template=FILE` with template engine
43. `--bibliography=FILE`, `--csl=FILE`
44. `--highlight-style=STYLE`
45. `--wrap=auto|none|preserve`, `--columns=N`
46. `--section-divs`
47. `--id-prefix=PREFIX`
48. `--eol=crlf|lf|native`
49. `--reference-doc=FILE` (for DOCX output)
50. `--top-level-division=section|chapter|part`
51. `--print-default-template=FORMAT`

**Extensions (markdown reader):**
52. Smart punctuation (`--smart`)
53. Bracketed spans `[text]{.class}`
54. `raw_attribute` syntax
55. `implicit_header_references`
56. Table captions
57. Superscript `^text^` / subscript `~text~` in markdown

**Template system:**
58. Template engine (variable interpolation, conditionals, loops)
59. Built-in default templates per output format

**Syntax highlighting:**
60. Server-side highlighting via `syntect` or `tree-sitter-highlight`
61. Line numbers support
62. Multiple styles

### Phase 4 — Extended Formats + Advanced Features

**AST changes:**
63. `LineBlock` block
64. `MetaInlines` / `MetaBlocks` in `MetaValue`
65. Multi-row table headers
66. Row/cell attributes, cell alignment override
67. Short + long table captions

**Reader/Writer:**
68. DOCX reader
69. EPUB reader + writer
70. reStructuredText reader + writer
71. AsciiDoc reader
72. Djot reader + writer
73. Jupyter notebook reader
74. Beamer output (LaTeX presentations)
75. reveal.js output (HTML presentations)
76. ODT writer

**Transforms:**
77. CSL localization
78. Emoji replacement (`:emoji_name:` → Unicode)

**CLI:**
79. `--self-contained` / `--embed-resources`
80. `--extract-media=DIR`
81. `--filter=PROGRAM` (JSON filter protocol)
82. WASM filter support (docmux-native alternative to JSON/Lua)
83. `--default-image-extension=EXT`
84. `--dpi=N`
85. `--log=FILE`
86. `--list-extensions[=FORMAT]`
87. `--file-scope`
88. `--sandbox`
89. `--trace`

**Extensions (markdown reader):**
90. Grid tables
91. Multiline tables
92. Line blocks
93. Emoji
94. ASCII identifiers

**Other:**
95. Encoding detection / BOM handling
96. Watch mode (already planned)

### Phase 5 — Long Tail

**Reader/Writer:**
97. Org Mode reader + writer
98. MediaWiki reader + writer
99. JIRA/Confluence reader + writer
100. man page reader + writer
101. PowerPoint (PPTX) writer
102. DokuWiki reader + writer
103. RTF reader
104. Textile reader
105. CSV/TSV reader

**Features:**
106. Multiple bibliographies
107. Example lists (`(@)` cross-document numbering)
108. Abbreviations (`*[HTML]: Hyper Text...`)
109. DOCX track changes
110. Custom Kate syntax definitions
111. Line highlighting in code blocks
112. Grid tables in markdown
113. pandoc title block (`% Title`)
114. MultiMarkdown metadata

**CLI:**
115. EPUB options (`--epub-cover-image`, `--epub-metadata`)
116. Slide options (`--slide-level`, `--incremental`)
117. `--reference-links` and `--atx-headers` for markdown output

---

## Tradeoff Notes

### Tight/Loose Lists: `Plain` block vs `tight: bool`

**pandoc approach**: Uses `Plain` (no `<p>`) vs `Para` inside list items.
- Pro: Pure, compositional — the block type determines rendering.
- Con: Adds a new block variant used *only* inside lists. Complicates every writer.

**Alternative**: `tight: bool` on `List`.
- Pro: Simpler — writers check one flag. No new block type.
- Con: Less pure — the list "tells" items how to render. Doesn't compose if you ever want `Plain` outside lists.

**Recommendation**: Use `tight: bool`. The `Plain` block is a pandoc design choice we don't need to copy. No real-world format uses `Plain` outside of tight lists. Writers already switch on list type; adding a flag check is trivial.

### Div vs Admonition

pandoc represents admonitions as `Div` with specific classes (`.note`, `.warning`). docmux has first-class `Admonition`.

**Keep both**: `Admonition` is more ergonomic for typed transforms. `Div` is needed for *generic* containers (custom directives, arbitrary wrappers, fenced divs). They serve different purposes. When reading pandoc markdown, a `Div` with a known admonition class can be upgraded to `Admonition` by a transform.

### Image alt: `String` vs `Vec<Inline>`

Changing `alt: String` to `alt: Vec<Inline>` is a breaking change that propagates to every reader/writer that touches images. Consider adding `alt_inlines: Option<Vec<Inline>>` alongside the existing `alt: String` for a phased migration, or just bite the bullet in Phase 3 since the crate is pre-1.0.

### Attributes: opt-in `Option<Attributes>` vs always-present

Using `Option<Attributes>` means most code can ignore attrs (just pass `None`). Using bare `Attributes` (with `Default`) means every construction site must include `attrs: Attributes::default()`. The `Option` approach is less noisy for simple cases but requires `.unwrap_or_default()` in writers. **Recommendation**: `Option<Attributes>` for the fields we're adding to existing variants. It's more ergonomic and doesn't break existing constructor patterns.

---

## Score Summary

| Category | pandoc features | docmux has | parity % |
|----------|----------------|------------|----------|
| Input formats (core 5) | md, latex, html, docx, typst | 3 | 60% |
| Output formats (core 5) | html, latex, docx, md, typst | 2 | 40% |
| Block nodes | 14 | 13 (+3 extra) | ~85% |
| Inline nodes | 20 | 18 (+1 extra) | ~88% |
| Attributes coverage | 9 element types | 1 | 11% |
| List features | 5 | 2 | 40% |
| Table features | 8 | 4 | 50% |
| CLI flags (essential 20) | 20 | 3 | 15% |
| Template system | Full | Stub | ~5% |
| Citation processing | Full CSL | Not started | 0% |
| Extensions system | ~100+ | None (format-specific) | 0% |
| Syntax highlighting | Built-in | None | 0% |
| Filter system | JSON + Lua | Transform trait only | ~20% |

**Overall estimated parity: ~30%**

The good news: the AST is 85-90% there on *node types*. The biggest gaps are in tooling around the AST (CLI, templates, citations, extensions, highlighting) and the attributes system.
