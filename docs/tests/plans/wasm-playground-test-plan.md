# WASM Playground Test Plan

> Executed: 2026-03-26
> Environment: Browser (Chrome), playground at `localhost:5173`, WASM compiled via `wasm-pack build`
> Method: JS console calls against `import('/src/wasm/docmux.ts')` + visual playground UI

---

## 1. Format Support Matrix

All 4 readers x all applicable writers tested.

| From \ To | HTML | LaTeX | Typst | Markdown | Plain Text |
|-----------|------|-------|-------|----------|------------|
| **markdown** | PASS | PASS | PASS | PASS | PASS |
| **latex** | PASS | PASS* | PASS | PASS | PASS |
| **typst** | PASS | PASS | PASS* | PASS | PASS |
| **myst** | PASS | PASS | PASS | PASS* | PASS |

\* Roundtrip (same format in and out) also tested separately — see section 6.

**Total: 17/17 PASS**

---

## 2. Edge Cases

| # | Test | Input | Expected | Result |
|---|------|-------|----------|--------|
| 1 | Empty input | `""` | No error, empty output | PASS (len=0) |
| 2 | Unsupported input format | `"docx"` | Error with message | PASS (`unsupported input format: docx`) |
| 3 | Unsupported output format | `"pdf"` | Error with message | PASS (`unsupported output format: pdf`) |
| 4 | Unicode / CJK | Japanese + emoji | Preserves chars | PASS |
| 5 | Large input (500 paragraphs) | 500x `Para with **bold** and $x^2$` | No error, large output | PASS (43KB output) |
| 6 | Standalone HTML | `# Test` | Has `<!DOCTYPE` / `<html>` | PASS |
| 7 | Standalone LaTeX | `# Test` | Has `\documentclass` | PASS |
| 8 | Standalone Typst | `# Hello` | Valid Typst preamble | PASS (52 bytes) |
| 9 | `parseToJson` | `# Heading\nParagraph` | Valid JSON with `"Heading"` node | PASS |
| 10 | Whitespace only | `"   \n\n  \n"` | No error, empty output | PASS (len=0) |
| 11 | Deeply nested lists (5 levels) | `- L1\n  - L2\n    - L3...` | 5 `<ul>` tags | PASS (5 `<ul>`) |

**Total: 11/11 PASS**

---

## 3. Content Fidelity — Markdown Reader

| # | Feature | Input | Check | Result |
|---|---------|-------|-------|--------|
| 1 | Frontmatter metadata | YAML with title/author/date/keywords | AST has typed fields | PASS |
| 2 | Multi-author frontmatter | List of `{name, email, affiliation}` | 2 authors parsed | PASS |
| 3 | Math inline | `$\alpha + \beta$` | `class="math math-inline"` in HTML | PASS |
| 4 | Math display (block) | `$$\n\sum...\n$$` | `class="math math-display"` in HTML | PASS |
| 5 | Math → LaTeX native | `$$\n y = mx + b \n$$` | `\[...\]` in LaTeX output | PASS |
| 6 | Code block + language | `` ```python `` | `class="language-python"` | PASS |
| 7 | Code block → LaTeX | `` ```python `` | `\begin{lstlisting}[language=python]` | PASS |
| 8 | Inline code | `` `cargo build` `` | `<code>cargo build</code>` | PASS |
| 9 | Links | `[text](url)` | `<a href="...">` | PASS |
| 10 | Images | `![alt](img.png)` | `<img src="img.png">` | PASS |
| 11 | Blockquote | `> quote` | `<blockquote>` | PASS |
| 12 | Horizontal rule | `---` | `<hr>` | PASS |
| 13 | Task lists (GFM) | `- [x] Done` | Has `checked` attribute | PASS |
| 14 | Footnotes | `[^1]` | Contains `footnote` ref | PASS |
| 15 | Strikethrough | `~~text~~` | `<del>` tag | PASS |
| 16 | Subscript | `H~2~O` | `<sub>` tag | PASS |
| 17 | Superscript | `x^2^` | `<sup>` tag | **FINDING** — no `<sup>` produced |
| 18 | Description list | `Term\n: Def` | `<dl>/<dt>` tags | **FINDING** — no `<dl>` produced |
| 19 | LaTeX special char escaping | `% & # $ ~ ^ \` | Escaped in output | PASS |

**Total: 17/19 PASS, 2 findings (see section 8)**

---

## 4. Cross-Reader Feature Checks

### LaTeX Reader → HTML
| Feature | Check | Result |
|---------|-------|--------|
| `\section` | Produces `<h>` | PASS |
| `\textbf` | Produces `<strong>` | PASS |
| `\textit` | Produces `<em>` | PASS |
| `\begin{equation}` | Has math class | PASS |
| `\begin{enumerate}` | Produces `<ol>` | PASS |
| `\begin{figure}` + `\includegraphics` | Has img/figure | PASS |

### Typst Reader → HTML
| Feature | Check | Result |
|---------|-------|--------|
| `= Heading` | Produces `<h>` | PASS |
| `*bold*` | Produces `<strong>` | PASS |
| `_italic_` | Produces `<em>` | PASS |
| `$ math $` | Has math class | PASS |
| `- bullet` | Produces `<ul>` | PASS |
| `+ numbered` | Produces `<ol>` | PASS |
| `` ```code``` `` | Produces `<code>/<pre>` | PASS |

### MyST Reader → HTML
| Feature | Check | Result |
|---------|-------|--------|
| `# Heading` | Produces `<h>` | PASS |
| `` ```{admonition} `` | Has admonition/warning class | PASS |
| `` ```{note} `` | Has note content | PASS |
| `{math}` role | Has math content | PASS |

**Total: 17/17 PASS**

---

## 5. Transform Tests

| # | Transform | Input | Check | Result |
|---|-----------|-------|-------|--------|
| 1 | Number sections | `# A\n## B\n## C\n# D` | Output has `1 A`, `1.1 B`, `1.2 C`, `2 D` | PASS |
| 2 | Table of contents | Multiple headings | `class="toc"` + `href="#..."` links | PASS |
| 3 | Cross-references | `{#intro}` label + `[](#intro)` ref | Heading has `id="intro"`, link points to it | PASS |

**Total: 3/3 PASS**

---

## 6. Roundtrip Tests

| # | Path | Content preserved | Result |
|---|------|-------------------|--------|
| 1 | Markdown → Markdown | Title, bold, list items | PASS |
| 2 | LaTeX → LaTeX | `\section`, `\textbf` | PASS |
| 3 | Typst → Typst | `= Heading`, `*bold*` | PASS |

**Total: 3/3 PASS**

---

## 7. Performance

| Test | Input size | Time | Output size |
|------|-----------|------|-------------|
| 1000 list items (bold + math + code + links) | ~50KB | **15ms** | 127KB |
| 500 paragraphs (bold + math) | ~16KB | < 10ms | 43KB |

WASM performance is excellent — sub-20ms for large documents.

---

## 8. Findings / Known Limitations

### FINDING-1: Superscript syntax `^text^` not producing `<sup>`

**Input**: `x^2^`
**Expected**: `<sup>2</sup>`
**Actual**: No `<sup>` tag in output

**Analysis**: Comrak's `superscript` extension may handle this differently, or the AST mapping for superscript → HTML may be incomplete. Subscript `~text~` works correctly.

**Severity**: Low — math mode `$x^2$` is the standard way to write superscripts in technical docs.

### FINDING-2: Description lists not producing `<dl>` elements

**Input**: `Term\n: Definition here`
**Expected**: `<dl><dt>Term</dt><dd>Definition here</dd></dl>`
**Actual**: No `<dl>` or `<dt>` tags in output

**Analysis**: Comrak has a `description_lists` extension. It may not be enabled, or the AST → HTML writer may not handle this node type yet.

**Severity**: Low — description lists are rarely used in most document workflows.

---

## 9. Playground UI Tests (Visual)

| # | Test | Method | Result |
|---|------|--------|--------|
| 1 | Initial load with Example workspace | Visual | PASS — loads with example.md |
| 2 | Monaco editor renders markdown | Visual | PASS — syntax highlighting |
| 3 | Preview tab: HTML rendering | Visual | PASS — ToC, math (KaTeX), tables, code |
| 4 | Source tab: raw output | Visual | PASS — shows HTML source |
| 5 | Format dropdown switching | Click | PASS — LaTeX, Typst, Markdown, Plain Text all switch |
| 6 | AST tab: JSON tree | Visual | PASS — pretty-printed JSON with metadata + content |
| 7 | Diagnostics tab | Visual | PASS — shows "No errors" |
| 8 | File tree sidebar | Visual | PASS — shows example.md |

**Total: 8/8 PASS**

---

## Summary

| Category | Passed | Total | Notes |
|----------|--------|-------|-------|
| Format conversions | 17 | 17 | All reader→writer combos |
| Edge cases | 11 | 11 | Empty, bad formats, unicode, large, standalone |
| Content fidelity | 17 | 19 | 2 findings: superscript, description lists |
| Cross-reader features | 17 | 17 | LaTeX, Typst, MyST all correct |
| Transforms | 3 | 3 | Number sections, ToC, cross-refs |
| Roundtrips | 3 | 3 | MD, LaTeX, Typst |
| Performance | 2 | 2 | Sub-20ms for 1000 items |
| Playground UI | 8 | 8 | All visual checks pass |
| **Total** | **78** | **80** | **97.5% pass rate** |
