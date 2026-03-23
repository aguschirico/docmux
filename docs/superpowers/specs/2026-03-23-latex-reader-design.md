# docmux-reader-latex — Design Spec

> Date: 2026-03-23
> Status: Approved
> Crate: `docmux-reader-latex`

## Goal

Parse a practical subset of LaTeX into the docmux AST. Not a Turing-complete TeX interpreter — a best-effort parser targeting academic papers, with graceful degradation for unrecognized constructs.

**Use cases:**
- Roundtrip fidelity for academic papers (`\section`, `\begin{figure}`, `\cite`, math environments)
- Parse back what the LaTeX writer produces
- Migrate existing LaTeX documents to HTML/Markdown (lossy is acceptable)

## Design Decisions

1. **Recursive descent parser** — manual lexer + parser, no external parsing dependencies. LaTeX is context-sensitive; a hand-written parser gives full control over error recovery and ambiguous constructs.
2. **Best-effort + warnings** — unrecognized commands/environments emit `RawBlock`/`RawInline { format: "latex" }` and accumulate `ParseWarning` entries. The document always parses successfully.
3. **Warnings in `Document`** — add `warnings: Vec<ParseWarning>` field to `Document` in `docmux-ast`. Non-breaking: existing readers return `warnings: vec![]`.
4. **Preamble handling** — extract `\title`, `\author`, `\date`, `\begin{abstract}` into typed `Metadata`. Preserve full preamble text in `Metadata.custom["latex_preamble"]` as `MetaValue::String`.

## AST Change

```rust
// docmux-ast — add to Document
pub struct Document {
    pub metadata: Metadata,
    pub content: Vec<Block>,
    pub bibliography: Option<Bibliography>,
    pub warnings: Vec<ParseWarning>,  // NEW
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParseWarning {
    pub line: usize,
    pub message: String,
}
```

## Architecture

```
Input &str
   |
   v
+----------+   Vec<Token>   +----------+
|  Lexer   | -------------> |  Parser  | -------> Document
| lexer.rs |                | parser.rs|
+----------+                +----------+
                                 |
                            Preamble
                            extraction
```

### Lexer (`lexer.rs`)

Converts input `&str` into `Vec<Token>`. Token variants:

| Token | Matches |
|-------|---------|
| `Command { name: String, line: usize }` | `\section`, `\textbf`, etc. |
| `BeginEnv { name: String, line: usize }` | `\begin{...}` |
| `EndEnv { name: String, line: usize }` | `\end{...}` |
| `BraceOpen` / `BraceClose` | `{` / `}` |
| `BracketOpen` / `BracketClose` | `[` / `]` |
| `Text { value: String }` | Plain text between commands |
| `MathInline { value: String }` | Content between `$...$` |
| `MathDisplay { value: String }` | Content between `$$...$$` or `\[...\]` |
| `Comment { value: String }` | Lines starting with `%` |
| `BlankLine` | Empty line (paragraph separator) |
| `Tilde` | `~` (non-breaking space) |
| `Ampersand` | `&` (table cell separator) |
| `DoubleBackslash { line: usize }` | `\\` (line break / table row separator) |

### Parser (`parser.rs`)

Recursive descent consuming tokens to produce AST nodes.

Key functions:
- `parse_document()` — extract preamble, parse body between `\begin{document}` and `\end{document}`
- `parse_blocks()` — collect blocks until a terminator (EOF, `\end{...}`)
- `parse_block()` — dispatch by command/environment to specific handler
- `parse_inlines()` — parse inline content until delimiter (closing brace, `\end`, etc.)
- `parse_environment()` — match `\begin{X}` ... `\end{X}`, dispatch by environment name
- `parse_brace_argument()` — consume balanced `{...}` content
- `parse_optional_argument()` — consume `[...]` if immediately following

### Unescape (`unescape.rs`)

Reverts LaTeX special character escaping:

| Input | Output |
|-------|--------|
| `\textbackslash{}` | `\` |
| `\{` / `\}` | `{` / `}` |
| `\#` | `#` |
| `\$` | `$` |
| `\%` | `%` |
| `\&` | `&` |
| `\textasciitilde{}` | `~` |
| `\_` | `_` |
| `\textasciicircum{}` | `^` |

## Supported Constructs

### Block-level

| LaTeX | AST Node |
|-------|----------|
| `\section{...}` through `\subparagraph{...}` | `Heading { level: 1..5 }` |
| `\section*{...}` (starred) | `Heading` without `id` |
| Text separated by blank lines | `Paragraph` |
| `\begin{itemize}` | `List { ordered: false }` |
| `\begin{enumerate}` | `List { ordered: true }` |
| `\begin{quote}` / `\begin{quotation}` | `BlockQuote` |
| `\begin{verbatim}` / `\begin{lstlisting}` | `CodeBlock` (`lstlisting[language=X]` extracts language; `verbatim` has `language: None`) |
| `\begin{figure}` | `Figure` (extracts `\includegraphics`, `\caption`, `\label`) |
| `\begin{table}` + `\begin{tabular}` | `Table` (parses `&`, `\\`, `\hline`) |
| `\begin{equation}` / `\begin{align}` / `\begin{gather}` | `MathBlock` |
| `\[ ... \]` / `$$ ... $$` | `MathBlock` |
| `\begin{abstract}` | Stored in `Metadata.abstract_text` |
| `\begin{description}` | `DefinitionList` |
| `\footnotetext[N]{...}` | `FootnoteDef` (numeric `[N]` converted to string id, e.g. `"1"`) |
| `\hrule` / `\noindent\rule{...}` | `ThematicBreak` |
| Unknown environment | `RawBlock { format: "latex" }` + warning |

**Not produced by this reader:** `Admonition` (no standard LaTeX equivalent).

**List parsing:** `\item` is lexed as a generic `Command { name: "item" }` token. Inside `\begin{itemize}` and `\begin{enumerate}`, the parser uses `\item` commands to split content into `ListItem` nodes.

### Inline-level

| LaTeX | AST Node |
|-------|----------|
| `\emph{...}` / `\textit{...}` | `Emphasis` |
| `\textbf{...}` | `Strong` |
| `\sout{...}` | `Strikethrough` |
| `\texttt{...}` | `Code` |
| `\textsc{...}` | `SmallCaps` |
| `\textsuperscript{...}` | `Superscript` |
| `\textsubscript{...}` | `Subscript` |
| `$...$` | `MathInline` |
| `\href{url}{text}` / `\url{...}` | `Link` |
| `\includegraphics{...}` (standalone inline) | `Image` |
| `\cite{...}` / `\citet{...}` / `\citeyear{...}` | `Citation` with `CitationMode` |
| `\ref{...}` / `\autoref{...}` / `\pageref{...}` | `CrossRef` with `RefForm` |
| `\label{...}` | Captured by parent block |
| `\footnote{...}` | Generates `FootnoteDef` + `FootnoteRef` inline |
| `\\` | `HardBreak` |
| `~` | `Text { value: "\u{00A0}" }` (non-breaking space) |
| Single newline within paragraph | `SoftBreak` |
| Unknown command | `RawInline { format: "latex" }` + warning |

**Not produced by this reader:** `Span` (no standard LaTeX equivalent).

### Preamble

| LaTeX | Destination |
|-------|-------------|
| `\title{...}` | `Metadata.title` |
| `\author{...}` | `Metadata.authors` (split by `\and`) |
| `\date{...}` | `Metadata.date` |
| Full preamble text | `Metadata.custom["latex_preamble"]` |

### Silently ignored (no warning)

- `\documentclass{...}`
- `\usepackage{...}`
- `\newcommand` / `\renewcommand`
- `\maketitle`
- `\tableofcontents`
- `\bibliographystyle{...}`
- Comments (`%`)

## Edge Cases

- **No `\begin{document}`** — parse everything as body (snippet mode).
- **Nested environments** — parser maintains a stack. Recursive parsing.
- **Nested math** — `\begin{equation}` containing `\begin{aligned}` → all content is raw math string, not recursively parsed.
- **Balanced braces** — brace counting when reading command arguments, supports `\textbf{text with {braces} inside}`.
- **Optional `[...]` arguments** — consumed only when immediately after command. `\section[short]{long}` → uses `long` for content, `short` discarded.
- **Multiple `\author`** — split by `\and` for multiple `Author` entries. Affiliations via `\\` within author block.

## File Structure

```
crates/docmux-reader-latex/
├── Cargo.toml          (add: thiserror workspace dep)
├── src/
│   ├── lib.rs          (LatexReader + Reader impl + re-exports)
│   ├── lexer.rs        (Tokenizer: &str -> Vec<Token>)
│   ├── parser.rs       (Recursive descent: Vec<Token> -> Document)
│   └── unescape.rs     (LaTeX special char unescaping)
```

### Changes outside the crate

- `docmux-ast/src/lib.rs` — add `warnings: Vec<ParseWarning>` to `Document`, add `ParseWarning` struct
- `docmux-reader-markdown/src/lib.rs` — add `warnings: vec![]` to constructed `Document`
- `docmux-cli/src/main.rs` — register `LatexReader` in `build_registry()`
- `docmux-cli/tests/cli_smoke.rs` — add smoke test for `.tex` -> `.html`
- `tests/fixtures/` — add `.tex` fixtures for golden tests

## Testing Strategy

### Unit tests (~15-20 in `lib.rs`)

- Each block type in isolation (heading, paragraph, list, figure, table, math, code, blockquote, definition list, footnote)
- Inline types (emphasis, strong, code, link, citation, crossref, math inline)
- Preamble metadata extraction
- Raw preamble preservation in `custom["latex_preamble"]`
- Character unescaping
- Unknown environment -> `RawBlock` + warning
- Unknown command -> `RawInline` + warning
- Document without `\begin{document}` (snippet mode)
- Nested braces in arguments

### Golden file tests

- Create `.tex` input fixtures in `tests/fixtures/basic/` and `tests/fixtures/complex/`
- Use existing `.tex` files generated by the LaTeX writer as starting points (roundtrip test)
- Add a complex academic paper fixture in native LaTeX

### CLI smoke tests

- `docmux input.tex -o output.html` works
- Auto-detection of format by `.tex` extension
- Error on malformed input still produces partial output
