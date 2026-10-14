# Real-corpus hardening (LaTeX to Typst first): design

**Date:** 2026-10-14
**Status:** LaTeX → Typst done (8/8 papers clean), citations from `.bbl` done; playground next

## Problem

All unit and golden tests pass, but real documents break. Converting 8 arXiv
papers (LaTeX sources) to Typst with docmux v0.5.0:

| Paper | docmux: Typst errors | pandoc 3.x: Typst errors |
|---|---|---|
| 1312.6114 (VAE) | 2 | pandoc times out |
| 1412.6980 (Adam) | 0 | 0 |
| 1512.03385 (ResNet) | 2 | parse failure on cvpr.sty |
| 1706.03762 (Transformer) | 4 | 1 (missing image) |
| 1810.04805 (BERT) | 2 | 1 (missing image) |
| 2005.14165 (GPT-3) | 6 + 591 raw LaTeX leaks | 1 (missing image) |
| 2006.11239 (DDPM) | 7 + 34 leaks | 1 (missing image) |
| 2106.09685 (LoRA) | 9 | 1 (missing image) |

Typst reports parse errors first and stops, so these counts are a floor:
semantic errors (`unknown variable: xy`) appear once parsing succeeds.

## Root causes

1. **Typst writer**
   - `#table(` emitted inside `#figure(` (code mode, `#` is invalid there).
   - Labels with spaces (`<figure:eval strategies>`) are not valid Typst labels.
   - Text escaping misses `[`, `]`, line-initial `/ `, `- `, `+ `, `= `, and `//`.
2. **LaTeX → Typst math** (`docmux-transform-math`)
   - Multi-letter runs pass through (`xy`, `KL`): Typst reads them as variables.
   - Symbol commands (`\{`, `\,`, `\|`, `\\`) tokenize as an empty command.
   - `\left`, `\right`, `\big…` sizing commands leak.
   - Sub/superscript brace groups lose their grouping (`x_{ij}` → `x_ij`).
   - Empty bases (`$^\dagger$`, `{}^*`) produce `unexpected hat`.
   - Converted names glue to neighbours (`256times256`, `alpha.`).
   - Unknown commands pass through as `\name`, which Typst reads as an escape.
   - Some table entries are stale for Typst 0.13+ (`angle.l` → `chevron.l`).
3. **LaTeX reader**
   - User macros (`\newcommand`, `\def`, `\DeclareMathOperator`) are not
     expanded, so `\bz`, `\qPhi`, `\kl{…}{…}` leak into math and text.
   - ``` ``…'' ``` quotes are not turned into `Quoted`.
   - `\iftoggle`/`\if…` conditionals and some `\includegraphics` forms leak.

## Bar

For every corpus paper and every output format:

1. Typst output compiles with `typst compile`, except for missing image files
   (EPS/PDF figures and paths outside the corpus are expected).
2. No raw LaTeX commands outside math in Markdown/HTML/Typst output.
3. Side-by-side with pandoc, docmux is at least as good on (1) and (2).

## Approach

- **Corpus harness** (`scripts/corpus/`): a manifest of arXiv IDs and main
  files, a fetch script that downloads sources into a gitignored cache, and a
  check script that converts, compiles Typst and counts leaks, with an
  optional pandoc column. Sources are not committed (arXiv licenses vary).
- **Regression fixtures**: every bug found in the corpus gets a minimal,
  self-written snippet as a unit or golden test in the crate that owns it.
- Fix order: Typst writer (small, unblocks parsing) → math converter →
  reader macro expansion → quotes/conditionals → other formats (HTML, MD).

## Non-goals

- Full TeX macro semantics (`\expandafter`, catcodes, conditionals evaluation
  beyond picking the true branch of known toggles).
- Rendering missing figures (EPS/PDF to PNG).

## Results (2026-10-14)

`scripts/corpus/check.py --pandoc`, Typst output, images stubbed:

| Paper | docmux before | docmux now | pandoc |
|---|---|---|---|
| VAE | 2 errors | 0 / 0 leaks | times out |
| Adam | 0 | 0 / 0 | 0 / 0 |
| ResNet | 2 | 0 / 0 | parse failure |
| Transformer | 4 | 0 / 0 | 79 errors |
| BERT | 2 (+121 hidden) | 0 / 0 | 112 errors, 107 leaks |
| GPT-3 | 6 + 591 leaks | 0 / 0 | 4 leaks |
| DDPM | 7 + 34 leaks | 0 / 0 | 29 leaks |
| LoRA | 9 | 0 / 0 | 3 leaks |

What changed:

- Typst writer: `table(` without `#` inside figures; sanitized labels on both
  ends; escapes for `[ ] //` and line-start markers; `#strong[…]`/`#emph[…]`
  when glued to a word; `#raw("…")` for code with backticks; heading and
  equation numbering when they are referenced; dangling `@ref`s written as
  `??` and unresolved citations as `[key]` instead of compile errors;
  `#ref(<…>)` when a label character follows.
- Math: rewritten LaTeX → Typst emitter (atoms, grouping, call escaping,
  sizing delimiters, `op("…")` fallback, environments, arity-aware
  arguments); symbol table validated against Typst 0.15 (all 267 entries
  compile); `\epsilon`/`\phi` were swapped.
- LaTeX reader: user macro expansion (`macros.rs`), `\iftoggle`, `\%`/`\$`
  no longer start comments/math, `\( \)`, accents, TeX ligatures and quotes,
  natbib/biblatex citation commands with optional arguments, `\eqref`,
  `\cref`, labels after a space, `\texttt` unescaping.

## Citations and references (2026-10-14)

- The LaTeX reader replaces `\bibliography{x}` with `x.bbl` (the CLI loads
  `<main>.bbl` and `x.bbl`; WASM callers pass them in `files`) and parses
  `thebibliography` into `Div#refs` with one `Div#ref-<key>.csl-entry` per
  entry, carrying the `\bibitem` label.
- `docmux-transform-references` resolves citations against that list:
  numeric `[1, 2]`, alphabetic `[BCV13]`, or natbib author-year
  `(Kingma and Ba, 2015)` / `Kingma and Ba (2015)`, each a link.
- `docmux-pipeline` runs references, cross-references and math notation
  for every conversion, in the CLI and in WASM (which before skipped math
  conversion entirely).
- Unknown environments and commands keep their content; layout is dropped.

## Next

1. Playground redesign (spec to follow).
2. More corpus: Typst and DOCX inputs, non-ML LaTeX (math, physics).
3. biblatex `.bbl` (`\entry` data format) is not read yet.
