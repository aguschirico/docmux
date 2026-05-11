# LaTeX `\input` / `\include` resolution — Design

**Issue:** [#4 — LaTeX reader: doesn't follow `\input{}` / `\include{}` directives](https://github.com/aguschirico/docmux/issues/4)
**Date:** 2026-05-10
**Status:** approved, ready for implementation plan

## Problem

`@docmux/wasm` 0.4.7 reads only the main `.tex` file passed to `convert()` /
`convertStandalone()`. `\input{}` and `\include{}` directives are silently
dropped: their bodies never enter the IR, so a 2k-line arXiv paper round-trips
to a few hundred bytes of Markdown.

Real-world impact (reported in #4): 3 of 8 arXiv papers tested produce
near-empty Markdown because of multi-file structure. Single-file papers in the
same suite produce 60–114 KB of Markdown.

The same gap exists in the CLI: `docmux main.tex` only sees `main.tex`'s body,
not the `sec/*.tex` files it includes. The issue only flagged WASM but the
fix is shared.

## Goals

1. LaTeX reader resolves `\input{X}` and `\include{X}` against an in-memory
   file map (Rust `HashMap<String, String>`).
2. WASM exposes this via a new `convertWithFiles(...)` function accepting a
   JS `Map<string, Uint8Array>`.
3. CLI auto-resolves `\input` / `\include` against the filesystem when given
   a single LaTeX path.
4. Existing single-file behaviour is unchanged (backwards-compatible).
5. The `Reader` trait in `docmux-core` is **not** modified.

## Non-goals

- `\InputIfFileExists`, `\subfile{}`, `subfiles` package — out of scope. Stay
  unknown commands with warnings.
- Honouring `\include`'s implicit `\clearpage` — the AST has no page-break
  node and it does not affect Markdown / MyST / HTML output.
- Surfacing `Document.warnings` through the WASM `convert*` JSON return — the
  API shape change is bigger than this PR. Tracked as separate follow-up.
- Pandoc-style include of non-`.tex` files (e.g. `\verbatiminput`).

## Architecture

```
Caller (CLI / WASM / future binding)
            │
            ▼
LatexReader::read_with_files(input, files: &HashMap<String, String>)
  ├── lexer::tokenize(input)                  ← unchanged
  ├── flatten::flatten_includes(tokens, files, &mut warnings)
  │     ├── walks token stream
  │     ├── tracks verbatim depth (BeginEnv/EndEnv)
  │     ├── on Token::Command{name="input"|"include"}:
  │     │     ├── read brace argument
  │     │     ├── resolve filename (extension fallback)
  │     │     ├── tokenize sub-file
  │     │     └── recursively flatten + splice
  │     └── cycle detection via visited stack
  └── Parser::new(flat_tokens).parse()        ← unchanged
```

**All new logic lives in `docmux-reader-latex`.** A new module `flatten.rs`
implements the token-stream pre-pass. `lib.rs` gains the
`read_with_files()` inherent method. The trait `Reader::read(&str)` keeps its
signature and delegates to `read_with_files(input, &HashMap::new())`.

### Why token-stream level

| Approach | Verdict | Why |
|---|---|---|
| Textual regex pre-pass on source | Rejected | Would replace inside `\verb|\input{x}|` and `\begin{verbatim}` — silently incorrect. |
| Token-stream pre-pass (chosen) | Selected | Lexer already classifies verbatim envs; we trivially track depth and skip expansion inside them. Tokens are flat — no nested state to thread through the parser. |
| Parser-level (parse sub-doc, splice blocks) | Rejected | Requires threading the file map through ~3000 lines of parser. Sub-parsing is supported (lines 740–763, 1466–1493) but the granularity is wrong: `\input` can paste a partial environment, which only makes sense at token level. |

## Algorithm: `flatten_includes`

```rust
pub(crate) fn flatten_includes(
    tokens: Vec<Token>,
    files: &HashMap<String, String>,
    warnings: &mut Vec<ParseWarning>,
) -> Vec<Token>
```

**Walk the token vector once.** Maintain `verbatim_depth: u32` and
`visited: Vec<String>` (currently-flattening filenames, for cycle detection).

For each token:

- `BeginEnv { name }` where `name ∈ { verbatim, verbatim*, Verbatim, lstlisting, minted }`
  → `verbatim_depth += 1`, push token, advance.
- `EndEnv { name }` of the above → `saturating_sub(1)`, push, advance.
- `Command { name: "input" | "include", line }` **AND** `verbatim_depth == 0`:
  1. Skip whitespace tokens.
  2. Read brace argument: `BraceOpen … BraceClose` → collect `Text` content.
     Fallback: if no brace, read next `Text` and split on whitespace (LaTeX
     allows `\input intro` separated by space).
  3. Resolve filename via `resolve_target(arg, files)`:
     - Strip leading `./`.
     - Try keys in order: `arg`, `arg.tex`, `arg.ltx`.
     - Return `(canonical_key, content)` on hit.
  4. If hit:
     - If `canonical_key` is in `visited`: warn "Circular `\X{Y}`", drop.
     - Else: push key, recurse on `tokenize(content)`, splice result, pop key.
  5. If miss: warn "`\X{Y}`: file not found in file map", drop.
- Anything else → push, advance.

**Recursion depth cap:** 32. If exceeded, emit warning and abort that
sub-tree. Defensive guard against bugs in `visited` tracking, not a real
limit.

## Data flow

`Document.warnings: Vec<ParseWarning>` is the existing channel. The
flatten pass appends warnings *before* parser warnings so users see include
errors first when triaging.

## API surface

### Rust — `LatexReader`

```rust
impl LatexReader {
    /// Parse a LaTeX document, resolving `\input{}` and `\include{}` against
    /// the given file map. Keys are filenames as referenced by the directive,
    /// with or without `.tex` extension.
    pub fn read_with_files(
        &self,
        input: &str,
        files: &HashMap<String, String>,
    ) -> Result<Document>;
}

// existing — unchanged signature, delegates to read_with_files:
impl Reader for LatexReader {
    fn read(&self, input: &str) -> Result<Document> {
        self.read_with_files(input, &HashMap::new())
    }
}
```

### WASM — `convertWithFiles`

```rust
#[wasm_bindgen(js_name = "convertWithFiles")]
pub fn convert_with_files(
    input: &str,
    from: &str,
    to: &str,
    files: &js_sys::Map,      // Map<string, Uint8Array> — source includes
    resources: &js_sys::Map,  // Map<string, Uint8Array> — binary resources
    standalone: bool,
) -> Result<String, JsError>;
```

- `files` is decoded as UTF-8. Bytes that fail decoding are skipped (the
  `\input` referencing them then fails with "file not found").
- `resources` parallels `convertWithResources`'s map (image embedding).
- Both maps may be empty.
- Existing functions (`convert`, `convertStandalone`, `convertWithResources`,
  `convertToBytes`, `convertBytesStandalone`) are **not modified**.

### CLI

When the input format is `latex` and there is exactly one input path (not
stdin), the CLI:

1. Reads `main.tex` from disk.
2. Pre-scans it for `\input{X}` / `\include{X}` directives.
3. Recursively reads each referenced file from the filesystem, relative to
   `main.tex`'s parent directory, applying the same extension fallback rules.
4. Builds a `HashMap<String, String>` keyed by the directive argument.
5. Calls `LatexReader::read_with_files(main_text, &files)`.

If the user passes multiple input paths (`docmux a.tex b.tex c.tex`), the
existing concatenation behaviour is preserved (no resolution).
If input is stdin, no resolution is attempted.

Files referenced but missing on disk produce a warning printed to stderr,
matching the existing parser-warning output path.

## Resolution rules

| Directive | Tries (in order) |
|---|---|
| `\input{intro}` | `intro`, `intro.tex`, `intro.ltx` |
| `\input{intro.tex}` | `intro.tex`, `intro.tex.tex`, `intro.tex.ltx` |
| `\input{./sec/body}` | `sec/body`, `sec/body.tex`, `sec/body.ltx` |
| `\input{../shared/preamble}` | `../shared/preamble`, `../shared/preamble.tex`, … |

`\include{X}` uses identical rules; the implicit `\clearpage` is dropped.

## Error handling

| Case | Action | Severity |
|---|---|---|
| File not in map / not on disk | Warning + drop directive | Soft |
| Cycle detected | Warning + drop second include | Soft |
| UTF-8 decode failure (WASM) | Warning + behave as "not found" | Soft |
| Map empty + `\input{X}` | Warning "file not found" | Soft |
| `\input` with no argument | Pass through; parser handles as unknown command | Soft |
| Recursion depth > 32 | Warning + abort that sub-tree | Soft |

Nothing aborts the `read()` call. Convert what you can, report what failed
— matches the parser's existing `RawBlock`-with-warning policy.

## Tests

### Unit (`crates/docmux-reader-latex/src/flatten.rs`)

- `flatten_basic_input` — `\input{intro}` with `intro.tex` in map produces
  inlined tokens.
- `flatten_extension_resolution` — bare `intro` resolves to `intro.tex`.
- `flatten_explicit_extension` — `intro.tex` works directly.
- `flatten_leading_dot_slash` — `./sec/intro` strips `./`.
- `flatten_recursive_includes` — three levels deep.
- `flatten_cycle_detection` — `a → b → a` warns and stops.
- `flatten_missing_file_warns` — unknown key produces warning, drops cmd.
- `flatten_inside_verbatim_preserved` — `\begin{verbatim}\input{x}\end{verbatim}`
  is left literal.
- `flatten_include_same_as_input` — `\include` mirrors `\input` behaviour.
- `flatten_no_brace_argument` — `\input` alone is passed through.
- `flatten_max_depth_guard` — recursion past 32 emits warning.
- `read_without_files_unchanged` — regression test: existing single-file
  documents produce identical AST.

### Integration (`crates/docmux-cli/tests/multi_file_latex.rs`)

Creates a tempdir with `main.tex` + `sec/intro.tex` + `sec/body.tex`, runs
the CLI binary, asserts the Markdown output contains content from both
included files.

### Golden fixture (`crates/docmux-cli/tests/fixtures/multi-file-paper/`)

Mimics arXiv layout:

```
multi-file-paper/
├── main.tex
├── sec/
│   ├── 0_abs.tex
│   ├── 1_intro.tex
│   └── 2_body.tex
└── expected/
    └── main.md
```

Updated with `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test`.

### WASM

No automated test runtime exists. We verify
`cargo build --target wasm32-unknown-unknown -p docmux-wasm` compiles. Manual
check via the playground (or a small Node.js script) is documented in the PR.

## Documentation

- `README.md` — add "Multi-file LaTeX" section with JS + CLI example.
- `crates/docmux-reader-latex/src/lib.rs` — docstring on `read_with_files`
  with a worked example.
- `.claude/rules/latex.md` — bullet on include resolution.

## Surface summary

| Layer | Diff |
|---|---|
| `docmux-core` | None |
| `docmux-reader-latex` | +1 module (`flatten.rs`), +1 inherent method |
| `docmux-wasm` | +1 exported function |
| `docmux-cli` | +~30 lines for filesystem resolution |
| `playground` | None |

## Open questions

- **Should `convertWithFiles` also accept text input bytes** (i.e. let JS pass
  the main `.tex` as `Uint8Array` too)? Current decision: no — keep `input`
  as `&str` to match all other text-mode entry points. Users decode upstream.
- **Should we propagate `Document.warnings` through `convertWithFiles`'s
  return value?** Current decision: not in this PR. The whole `convert*`
  family currently throws away warnings; changing that needs a separate API
  shape decision.

## Out-of-scope follow-ups

- Surface `Document.warnings` through the WASM `convert*` family.
- Markdown reader analogue (`{!include ...!}` / `<!-- include foo.md -->`).
- `\InputIfFileExists`, `\subfile`, `subfiles` package.
