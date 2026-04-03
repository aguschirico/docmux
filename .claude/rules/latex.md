---
paths:
  - "crates/docmux-reader-latex/**"
  - "crates/docmux-writer-latex/**"
---

# LaTeX Architecture

## Reader
- Parses a **practical subset** of LaTeX (not Turing-complete TeX).
- Goal: roundtrip fidelity for academic papers — `\section`, `\begin{figure}`, `\cite`, math environments, etc.

## Writer
- Full coverage of all AST node types.
- Standalone mode: `\documentclass{article}` with amsmath, graphicx, hyperref, listings, ulem.
- Math is native LaTeX: `$...$` / `\[...\]` / `\begin{equation}`.
- 10 special chars escaped: `# $ % & ~ _ ^ \ { }`.
