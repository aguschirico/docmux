# Playground redesign: the 10-second demo

**Date:** 2026-10-14
**Status:** in progress

## Problem

The playground is an IDE shell: a file tree, a workspace dropdown and a
narrow output column. A first-time visitor sees a generic Markdown example,
no statement of what docmux is, and has to find a format selector inside a
tab to see a conversion. It also had correctness gaps:

- LaTeX projects were converted file by file, so `\input` and the `.bbl`
  never reached the reader (`convertWithFiles` existed but was unused).
- The preview iframe used `allow-scripts allow-same-origin`, which lets a
  document's scripts reach the playground's origin (IndexedDB with the
  user's files). Raw HTML in a Markdown document is passed through, as in
  pandoc, so the sandbox has to hold.

## Goal

Someone landing on the page understands what docmux does in ten seconds,
without reading or clicking: a real paper is already open on the left and
already converted on the right.

## Layout

```
docmux  Convert LaTeX, Markdown, Typst, Word and HTML, in your browser.   GitHub  npm
        Rust compiled to WebAssembly · nothing leaves your machine · MIT
Try:  [Research paper · LaTeX]  [README · Markdown]  [Report · Typst]   [Open file]
┌─ INPUT ─ main.tex  refs.bbl ──────────┬─ Preview │ Typst │ HTML │ Markdown │ LaTeX │ Word ┐
│ editor                                │ output (rendered or source)       [Copy] [Save] │
└───────────────────────────────────────┴───────────────────────────────────────────────────┘
LaTeX → Typst · 14 ms · 0 warnings
```

- 50/50 split, resizable. The file tree is hidden behind a "Files" toggle;
  multi-file examples show their files as tabs above the editor.
- Output formats are top-level tabs. Preview renders the document as HTML
  (KaTeX for math); the other tabs show the converted source; Word
  downloads a `.docx`. AST and diagnostics move to a small "Inspect" menu.
- A status line shows the conversion direction, time in ms and warning
  count: the speed is part of the pitch.
- First visit opens the paper example with the Preview tab selected (the rendered paper is the strongest first impression; Typst is one click away).

## Examples

Written for the playground (no third-party papers, no license questions):

1. **Research paper (LaTeX)**: `main.tex` + `refs.bbl`. User macros,
   `align`, a theorem, a table with `\multicolumn`, natbib citations,
   `\ref`s. Shows the things that break other converters.
2. **README (Markdown)**: GFM tables, task lists, code, math, footnotes.
3. **Report (Typst)**: headings, math, figure, table.

Loading an example replaces a dedicated "Examples" workspace, so user
workspaces are never overwritten.

## Non-goals (this iteration)

- Compiling Typst to PDF in the browser.
- Shareable URLs (later: example + output tab in the hash).
- Deploy (Cloudflare Pages) and launch, which follow this work.
