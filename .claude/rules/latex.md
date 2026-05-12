---
paths:
  - "crates/docmux-reader-latex/**"
  - "crates/docmux-writer-latex/**"
---

# LaTeX Architecture

## Reader
- Parses a **practical subset** of LaTeX (not Turing-complete TeX).
- Goal: roundtrip fidelity for academic papers — `\section`, `\begin{figure}`, `\cite`, math environments, etc.
- `\input{X}` / `\include{X}` are resolved against an in-memory file map
  via `LatexReader::read_with_files(input, &HashMap<String, String>)`. The
  CLI scans the main file and loads referenced files from the parent
  directory. WASM exposes this via `convertWithFiles(...)`.

## Writer
- Full coverage of all AST node types.
- Standalone mode: `\documentclass{article}` with amsmath, graphicx, hyperref, listings, ulem.
- Math is native LaTeX: `$...$` / `\[...\]` / `\begin{equation}`.
- 10 special chars escaped: `# $ % & ~ _ ^ \ { }`.
