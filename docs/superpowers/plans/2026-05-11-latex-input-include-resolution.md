# LaTeX `\input` / `\include` Resolution — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Resolve `\input{X}` and `\include{X}` directives in the LaTeX reader against an in-memory file map (Rust) or filesystem (CLI). Expose a new WASM `convertWithFiles` function. Trait `Reader` is untouched.

**Architecture:** Token-stream pre-pass in `docmux-reader-latex/src/flatten.rs`, called by a new inherent method `LatexReader::read_with_files(&str, &HashMap<String, String>)`. The trait method `read(&self, &str)` delegates with an empty map. WASM exposes `convertWithFiles(input, from, to, files, resources, standalone)`. CLI pre-scans the main LaTeX file for `\input` / `\include` references and loads them from disk before calling `read_with_files`.

**Tech Stack:** Rust 2021, wasm-bindgen, js_sys, clap, standard library.

**Spec:** `docs/superpowers/specs/2026-05-10-latex-input-include-resolution-design.md`

**Issue:** [#4](https://github.com/aguschirico/docmux/issues/4)

---

## File Structure

| File | Purpose | Action |
|---|---|---|
| `crates/docmux-reader-latex/src/flatten.rs` | Token-stream pre-pass for `\input` / `\include` | **Create** |
| `crates/docmux-reader-latex/src/lib.rs` | Add `flatten` module + `read_with_files` method | **Modify** |
| `crates/docmux-wasm/src/lib.rs` | Add `convert_with_files` exported function | **Modify** |
| `crates/docmux-cli/src/main.rs` | Detect single-file LaTeX + pre-scan + load includes from FS | **Modify** |
| `crates/docmux-cli/tests/multi_file_latex.rs` | Integration test using a tempdir multi-file paper | **Create** |
| `tests/fixtures/complex/multi-file/main.tex` etc. | (Optional) golden-style fixture | **Skip** — golden tests load single-file via `reader.read()`; multi-file is covered by the CLI integration test |
| `README.md` | Document multi-file LaTeX usage | **Modify** |
| `.claude/rules/latex.md` | Note `\input/\include` resolution | **Modify** |
| `crates/docmux-reader-latex/src/lib.rs` (rustdoc) | Worked example on `read_with_files` | **Modify** |

---

## Conventions reminder

- `cargo fmt --all` runs automatically via `PostToolUse` hook on `.rs` edits.
- `cargo clippy --workspace --all-targets -- -D warnings` must pass.
- No `unwrap()` / `expect()` in library code (tests are fine).
- New code requires tests. Cover happy path + at least one edge case.
- Use `?` over match-then-return.
- Group imports: std → external crates → internal crates.
- Commits do not use `Co-Authored-By`.

---

## Task 1: Bootstrap `flatten.rs` with the basic `\input` case (happy path)

**Files:**
- Create: `crates/docmux-reader-latex/src/flatten.rs`
- Modify: `crates/docmux-reader-latex/src/lib.rs:9` (add `pub(crate) mod flatten;`)

- [ ] **Step 1.1: Create `flatten.rs` with module skeleton and a single failing test**

Create `crates/docmux-reader-latex/src/flatten.rs`:

```rust
//! Token-stream pre-pass that resolves `\input{X}` and `\include{X}` against
//! an in-memory file map. Runs between the lexer and the parser so the parser
//! sees a single flat token stream with includes already inlined.

use docmux_ast::ParseWarning;
use std::collections::HashMap;

use crate::lexer::{self, Token};

/// Maximum nesting depth before we abort a sub-tree. Defensive: real-world
/// papers nest 2–3 levels.
const MAX_DEPTH: usize = 32;

/// Walk `tokens`, replacing `\input{X}` / `\include{X}` directives with the
/// tokenized contents of `files[X]`. Recurses into included files. Emits
/// warnings on missing files, cycles, and depth-exceeded cases.
pub(crate) fn flatten_includes(
    tokens: Vec<Token>,
    files: &HashMap<String, String>,
    warnings: &mut Vec<ParseWarning>,
) -> Vec<Token> {
    let mut visited: Vec<String> = Vec::new();
    flatten_inner(tokens, files, warnings, &mut visited, 0)
}

fn flatten_inner(
    tokens: Vec<Token>,
    files: &HashMap<String, String>,
    warnings: &mut Vec<ParseWarning>,
    visited: &mut Vec<String>,
    depth: usize,
) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Command { name, line }
                if name == "input" || name == "include" =>
            {
                let line = *line;
                let cmd = name.clone();
                match read_brace_arg(&tokens, i + 1) {
                    Some((arg, consumed)) => {
                        i += 1 + consumed;
                        match files.get(&arg) {
                            Some(content) => {
                                let sub = lexer::tokenize(content);
                                let mut flat =
                                    flatten_inner(sub, files, warnings, visited, depth + 1);
                                out.append(&mut flat);
                            }
                            None => {
                                warnings.push(ParseWarning {
                                    line,
                                    message: format!(
                                        "\\{cmd}{{{arg}}}: file not found in file map"
                                    ),
                                });
                            }
                        }
                    }
                    None => {
                        out.push(tokens[i].clone());
                        i += 1;
                    }
                }
            }
            _ => {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
    }
    out
}

/// Reads a `{...}` brace argument starting at `tokens[start]`. Skips leading
/// whitespace/newlines. Returns the concatenated text inside the braces and
/// the number of tokens consumed (including the braces).
fn read_brace_arg(tokens: &[Token], start: usize) -> Option<(String, usize)> {
    let mut i = start;
    // Skip whitespace before the brace.
    while i < tokens.len() && matches!(tokens[i], Token::Newline) {
        i += 1;
    }
    if i >= tokens.len() || !matches!(tokens[i], Token::BraceOpen) {
        return None;
    }
    i += 1; // consume BraceOpen
    let mut depth: u32 = 1;
    let mut buf = String::new();
    while i < tokens.len() {
        match &tokens[i] {
            Token::BraceOpen => {
                depth += 1;
                buf.push('{');
            }
            Token::BraceClose => {
                depth -= 1;
                if depth == 0 {
                    i += 1;
                    return Some((buf, i - start));
                }
                buf.push('}');
            }
            Token::Text { value } => buf.push_str(value),
            Token::Newline => buf.push(' '),
            _ => {}
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count_text_blocks(tokens: &[Token]) -> usize {
        tokens
            .iter()
            .filter(|t| matches!(t, Token::Text { .. }))
            .count()
    }

    fn text_concat(tokens: &[Token]) -> String {
        tokens
            .iter()
            .filter_map(|t| match t {
                Token::Text { value } => Some(value.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn flatten_basic_input_inlines_referenced_file() {
        let main = "\\input{intro}";
        let mut files = HashMap::new();
        files.insert("intro".to_string(), "hello world".to_string());

        let tokens = lexer::tokenize(main);
        let mut warnings = Vec::new();
        let flat = flatten_includes(tokens, &files, &mut warnings);

        assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
        let concat = text_concat(&flat);
        assert!(
            concat.contains("hello world"),
            "expected included content; got tokens = {flat:?}"
        );
        // The \input command itself should be gone.
        let has_input_cmd = flat.iter().any(|t| {
            matches!(t, Token::Command { name, .. } if name == "input")
        });
        assert!(!has_input_cmd, "\\input command should be removed");
        // Defensive: there must be at least one Text token now.
        assert!(count_text_blocks(&flat) >= 1);
    }
}
```

- [ ] **Step 1.2: Register the new module in `lib.rs`**

Edit `crates/docmux-reader-latex/src/lib.rs` line 9 area, changing:

```rust
pub(crate) mod lexer;
pub(crate) mod parser;
pub(crate) mod unescape;
```

to:

```rust
pub(crate) mod flatten;
pub(crate) mod lexer;
pub(crate) mod parser;
pub(crate) mod unescape;
```

- [ ] **Step 1.3: Run the test to confirm it passes**

Run:

```sh
cargo test -p docmux-reader-latex flatten_basic_input_inlines_referenced_file -- --nocapture
```

Expected: 1 test passed.

- [ ] **Step 1.4: Commit**

```sh
git add crates/docmux-reader-latex/src/flatten.rs crates/docmux-reader-latex/src/lib.rs
git commit -m "feat(reader-latex): bootstrap flatten module with basic \\input case (#4)"
```

---

## Task 2: Extension fallback and `./` path normalization

**Files:**
- Modify: `crates/docmux-reader-latex/src/flatten.rs`

- [ ] **Step 2.1: Add failing tests for extension resolution**

Append to the `tests` module in `flatten.rs`:

```rust
#[test]
fn flatten_resolves_with_tex_extension() {
    let main = "\\input{intro}";
    let mut files = HashMap::new();
    // Note: only the .tex-suffixed key exists.
    files.insert("intro.tex".to_string(), "hello".to_string());

    let tokens = lexer::tokenize(main);
    let mut warnings = Vec::new();
    let flat = flatten_includes(tokens, &files, &mut warnings);

    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    assert!(text_concat(&flat).contains("hello"));
}

#[test]
fn flatten_accepts_explicit_extension() {
    let main = "\\input{intro.tex}";
    let mut files = HashMap::new();
    files.insert("intro.tex".to_string(), "hello".to_string());

    let tokens = lexer::tokenize(main);
    let mut warnings = Vec::new();
    let flat = flatten_includes(tokens, &files, &mut warnings);

    assert!(warnings.is_empty());
    assert!(text_concat(&flat).contains("hello"));
}

#[test]
fn flatten_strips_leading_dot_slash() {
    let main = "\\input{./sec/intro}";
    let mut files = HashMap::new();
    files.insert("sec/intro.tex".to_string(), "deep".to_string());

    let tokens = lexer::tokenize(main);
    let mut warnings = Vec::new();
    let flat = flatten_includes(tokens, &files, &mut warnings);

    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    assert!(text_concat(&flat).contains("deep"));
}
```

- [ ] **Step 2.2: Run to confirm two of three fail (only `flatten_accepts_explicit_extension` passes — the literal key is hit)**

```sh
cargo test -p docmux-reader-latex flatten_ -- --nocapture
```

Expected: `flatten_resolves_with_tex_extension` and `flatten_strips_leading_dot_slash` fail (file not found warning emitted).

- [ ] **Step 2.3: Replace the inline `files.get(&arg)` with a `resolve_target` helper**

In `flatten.rs`, **replace** the matching block in `flatten_inner`:

```rust
match files.get(&arg) {
    Some(content) => {
        let sub = lexer::tokenize(content);
        let mut flat =
            flatten_inner(sub, files, warnings, visited, depth + 1);
        out.append(&mut flat);
    }
    None => {
        warnings.push(ParseWarning {
            line,
            message: format!(
                "\\{cmd}{{{arg}}}: file not found in file map"
            ),
        });
    }
}
```

with:

```rust
match resolve_target(&arg, files) {
    Some((_key, content)) => {
        let sub = lexer::tokenize(content);
        let mut flat =
            flatten_inner(sub, files, warnings, visited, depth + 1);
        out.append(&mut flat);
    }
    None => {
        warnings.push(ParseWarning {
            line,
            message: format!(
                "\\{cmd}{{{arg}}}: file not found in file map"
            ),
        });
    }
}
```

And add this helper at module scope, right above the `#[cfg(test)]` block:

```rust
/// Resolves the `\input` argument against the file map. Strips leading `./`
/// and tries the bare key, then `<key>.tex`, then `<key>.ltx`.
fn resolve_target<'a>(
    arg: &str,
    files: &'a HashMap<String, String>,
) -> Option<(String, &'a str)> {
    let cleaned = arg.trim_start_matches("./");
    for candidate in [
        cleaned.to_string(),
        format!("{cleaned}.tex"),
        format!("{cleaned}.ltx"),
    ] {
        if let Some(content) = files.get(&candidate) {
            return Some((candidate, content.as_str()));
        }
    }
    None
}
```

- [ ] **Step 2.4: Run the three tests — all should pass**

```sh
cargo test -p docmux-reader-latex flatten_ -- --nocapture
```

Expected: all 4 `flatten_*` tests pass.

- [ ] **Step 2.5: Commit**

```sh
git add crates/docmux-reader-latex/src/flatten.rs
git commit -m "feat(reader-latex): extension fallback and ./ stripping for \\input (#4)"
```

---

## Task 3: Recursive includes + cycle detection

**Files:**
- Modify: `crates/docmux-reader-latex/src/flatten.rs`

- [ ] **Step 3.1: Add failing tests**

Append to the `tests` module:

```rust
#[test]
fn flatten_recurses_through_nested_includes() {
    let main = "\\input{intro}";
    let mut files = HashMap::new();
    files.insert("intro".to_string(), "A \\input{deeper} Z".to_string());
    files.insert("deeper".to_string(), "MIDDLE".to_string());

    let tokens = lexer::tokenize(main);
    let mut warnings = Vec::new();
    let flat = flatten_includes(tokens, &files, &mut warnings);

    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
    let concat = text_concat(&flat);
    assert!(concat.contains("MIDDLE"), "expected nested content; got {concat:?}");
}

#[test]
fn flatten_detects_cycles_and_warns() {
    let main = "\\input{a}";
    let mut files = HashMap::new();
    files.insert("a".to_string(), "from-a \\input{b}".to_string());
    files.insert("b".to_string(), "from-b \\input{a}".to_string());

    let tokens = lexer::tokenize(main);
    let mut warnings = Vec::new();
    let flat = flatten_includes(tokens, &files, &mut warnings);

    // Both bodies should appear once. The cycle should be cut, not infinite.
    let concat = text_concat(&flat);
    assert!(concat.contains("from-a"));
    assert!(concat.contains("from-b"));
    assert!(
        warnings.iter().any(|w| w.message.contains("Circular")),
        "expected a Circular warning, got: {warnings:?}"
    );
}
```

- [ ] **Step 3.2: Run — both should fail (cycle test infinite-loops or stack overflows)**

```sh
cargo test -p docmux-reader-latex flatten_recurses_through_nested_includes -- --nocapture
cargo test -p docmux-reader-latex flatten_detects_cycles_and_warns -- --nocapture
```

Note: run the cycle test with a timeout if you suspect infinite recursion:

```sh
timeout 5 cargo test -p docmux-reader-latex flatten_detects_cycles_and_warns -- --nocapture
```

Expected: `flatten_recurses_through_nested_includes` will *actually pass* already — recursion is wired up. `flatten_detects_cycles_and_warns` will fail with stack overflow because the second visit to `a` re-enters indefinitely. That's exactly what cycle-detection prevents.

- [ ] **Step 3.3: Add cycle detection — modify the include arm in `flatten_inner`**

Replace the `Some((_key, content)) => { ... }` arm with:

```rust
Some((key, content)) => {
    if visited.iter().any(|v| v == &key) {
        warnings.push(ParseWarning {
            line,
            message: format!(
                "Circular \\{cmd}{{{arg}}} (already including {key})"
            ),
        });
    } else {
        let sub = lexer::tokenize(content);
        visited.push(key);
        let mut flat = flatten_inner(sub, files, warnings, visited, depth + 1);
        visited.pop();
        out.append(&mut flat);
    }
}
```

- [ ] **Step 3.4: Run both tests — both should pass**

```sh
cargo test -p docmux-reader-latex flatten_ -- --nocapture
```

Expected: all 6 `flatten_*` tests pass.

- [ ] **Step 3.5: Commit**

```sh
git add crates/docmux-reader-latex/src/flatten.rs
git commit -m "feat(reader-latex): recursive includes with cycle detection (#4)"
```

---

## Task 4: Verbatim guard, `\include`, missing file, malformed cases

**Files:**
- Modify: `crates/docmux-reader-latex/src/flatten.rs`

- [ ] **Step 4.1: Add failing tests**

Append to the `tests` module:

```rust
#[test]
fn flatten_skips_inside_verbatim_env() {
    let main = "\\begin{verbatim}\\input{ghost}\\end{verbatim}";
    let files = HashMap::new(); // ghost is not in the map

    let tokens = lexer::tokenize(main);
    let mut warnings = Vec::new();
    let flat = flatten_includes(tokens, &files, &mut warnings);

    // No warning: the \input wasn't even considered (inside verbatim).
    assert!(
        warnings.is_empty(),
        "should not warn for \\input inside verbatim; got {warnings:?}"
    );
    // The \input command must still be present in the output stream.
    let has_input_cmd = flat.iter().any(|t| {
        matches!(t, Token::Command { name, .. } if name == "input")
    });
    assert!(has_input_cmd, "\\input inside verbatim must be preserved literally");
}

#[test]
fn flatten_include_behaves_like_input() {
    let main = "\\include{intro}";
    let mut files = HashMap::new();
    files.insert("intro".to_string(), "INC".to_string());

    let tokens = lexer::tokenize(main);
    let mut warnings = Vec::new();
    let flat = flatten_includes(tokens, &files, &mut warnings);

    assert!(warnings.is_empty());
    assert!(text_concat(&flat).contains("INC"));
}

#[test]
fn flatten_missing_file_emits_warning() {
    let main = "alpha \\input{ghost} omega";
    let files = HashMap::new();

    let tokens = lexer::tokenize(main);
    let mut warnings = Vec::new();
    let flat = flatten_includes(tokens, &files, &mut warnings);

    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].message.contains("ghost"));
    assert!(warnings[0].message.contains("file not found"));
    // \input directive and its arg should be dropped — surrounding text remains.
    let concat = text_concat(&flat);
    assert!(concat.contains("alpha"));
    assert!(concat.contains("omega"));
    assert!(!concat.contains("ghost"));
}

#[test]
fn flatten_no_brace_argument_keeps_command() {
    // Malformed: \input with no following brace at all.
    let main = "\\input";
    let files = HashMap::new();

    let tokens = lexer::tokenize(main);
    let mut warnings = Vec::new();
    let flat = flatten_includes(tokens, &files, &mut warnings);

    // We pass through the \input command — the parser will treat it as an
    // unknown command later.
    let has_input_cmd = flat.iter().any(|t| {
        matches!(t, Token::Command { name, .. } if name == "input")
    });
    assert!(has_input_cmd);
    assert!(warnings.is_empty());
}
```

- [ ] **Step 4.2: Run — three of four fail**

```sh
cargo test -p docmux-reader-latex flatten_ -- --nocapture
```

Expected:
- `flatten_skips_inside_verbatim_env` FAILS (warning emitted because `\input` is processed inside verbatim).
- `flatten_include_behaves_like_input` PASSES (already covered — the match already includes `"include"`).
- `flatten_missing_file_emits_warning` PASSES (covered by Task 1's logic).
- `flatten_no_brace_argument_keeps_command` PASSES (covered by `read_brace_arg` returning `None`).

So really only the verbatim test needs new code. The others are regression coverage.

- [ ] **Step 4.3: Add verbatim depth tracking**

In `flatten_inner`, add a `let mut verbatim_depth: u32 = 0;` at the top of the function (next to `let mut i = 0;`).

Then change the top-level match to track verbatim envs and gate the input handling:

```rust
while i < tokens.len() {
    match &tokens[i] {
        Token::BeginEnv { name, .. } if is_verbatim_env(name) => {
            verbatim_depth += 1;
            out.push(tokens[i].clone());
            i += 1;
        }
        Token::EndEnv { name, .. } if is_verbatim_env(name) => {
            verbatim_depth = verbatim_depth.saturating_sub(1);
            out.push(tokens[i].clone());
            i += 1;
        }
        Token::Command { name, line }
            if verbatim_depth == 0
                && (name == "input" || name == "include") =>
        {
            // ... existing handler unchanged ...
```

Add this helper alongside `resolve_target`:

```rust
fn is_verbatim_env(name: &str) -> bool {
    matches!(
        name,
        "verbatim" | "verbatim*" | "Verbatim" | "lstlisting" | "minted"
    )
}
```

- [ ] **Step 4.4: Run all flatten tests**

```sh
cargo test -p docmux-reader-latex flatten_ -- --nocapture
```

Expected: all 10 `flatten_*` tests pass.

- [ ] **Step 4.5: Commit**

```sh
git add crates/docmux-reader-latex/src/flatten.rs
git commit -m "feat(reader-latex): verbatim guard and edge cases for \\input (#4)"
```

---

## Task 5: Depth guard + `LatexReader::read_with_files`

**Files:**
- Modify: `crates/docmux-reader-latex/src/flatten.rs`
- Modify: `crates/docmux-reader-latex/src/lib.rs`

- [ ] **Step 5.1: Add depth-guard test**

Append to the `tests` module:

```rust
#[test]
fn flatten_max_depth_guard_emits_warning() {
    // Build a chain a → a (self-cycle would short-circuit, so use unique names).
    let mut files = HashMap::new();
    for n in 0..40 {
        let next = n + 1;
        files.insert(format!("f{n}"), format!("level{n} \\input{{f{next}}}"));
    }
    files.insert("f40".to_string(), "deepest".to_string());

    let main = "\\input{f0}";
    let tokens = lexer::tokenize(main);
    let mut warnings = Vec::new();
    let flat = flatten_includes(tokens, &files, &mut warnings);

    assert!(
        warnings.iter().any(|w| w.message.contains("max include depth")),
        "expected a depth warning, got: {warnings:?}"
    );
    // The chain should have stopped before reaching f40.
    let concat = text_concat(&flat);
    assert!(!concat.contains("deepest"), "unexpected deep content: {concat:?}");
}
```

- [ ] **Step 5.2: Run — should fail (no guard yet, will likely stack overflow with a 40-deep chain)**

```sh
timeout 10 cargo test -p docmux-reader-latex flatten_max_depth_guard_emits_warning -- --nocapture
```

Expected: stack overflow or timeout.

- [ ] **Step 5.3: Add the depth guard at the top of `flatten_inner`**

At the very beginning of the function body in `flatten_inner` (before the loop), add:

```rust
if depth > MAX_DEPTH {
    // Cannot synthesize a real line number here; use 0.
    warnings.push(ParseWarning {
        line: 0,
        message: format!("max include depth ({MAX_DEPTH}) exceeded; aborting branch"),
    });
    return Vec::new();
}
```

- [ ] **Step 5.4: Re-run the depth test — should pass**

```sh
cargo test -p docmux-reader-latex flatten_max_depth_guard_emits_warning -- --nocapture
```

Expected: PASS.

- [ ] **Step 5.5: Add `read_with_files` to `LatexReader` and a regression test**

Edit `crates/docmux-reader-latex/src/lib.rs`. Replace the whole file with:

```rust
//! # docmux-reader-latex
//!
//! LaTeX reader for docmux. Parses a practical subset of LaTeX into the
//! docmux AST using a hand-written recursive descent parser.
//!
//! Unrecognized commands and environments are emitted as `RawBlock`/`RawInline`
//! with warnings accumulated in `Document.warnings`.

pub(crate) mod flatten;
pub(crate) mod lexer;
pub(crate) mod parser;
pub(crate) mod unescape;

use docmux_ast::Document;
use docmux_core::{Reader, Result};
use std::collections::HashMap;

/// A LaTeX reader.
#[derive(Debug, Default)]
pub struct LatexReader;

impl LatexReader {
    pub fn new() -> Self {
        Self
    }

    /// Parse a LaTeX document, resolving `\input{}` and `\include{}` directives
    /// against the given file map. Keys are filenames as referenced by the
    /// directive (with or without `.tex` extension).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use docmux_reader_latex::LatexReader;
    /// use std::collections::HashMap;
    ///
    /// let mut files = HashMap::new();
    /// files.insert("intro.tex".to_string(), "Hello!".to_string());
    /// let doc = LatexReader::new()
    ///     .read_with_files("\\input{intro}", &files)
    ///     .unwrap();
    /// ```
    pub fn read_with_files(
        &self,
        input: &str,
        files: &HashMap<String, String>,
    ) -> Result<Document> {
        let tokens = lexer::tokenize(input);
        let mut warnings = Vec::new();
        let flat = flatten::flatten_includes(tokens, files, &mut warnings);
        let mut doc = parser::Parser::new(flat).parse();
        // Surface flatten warnings before parser warnings.
        warnings.extend(std::mem::take(&mut doc.warnings));
        doc.warnings = warnings;
        Ok(doc)
    }
}

impl Reader for LatexReader {
    fn format(&self) -> &str {
        "latex"
    }

    fn extensions(&self) -> &[&str] {
        &["tex", "latex"]
    }

    fn read(&self, input: &str) -> Result<Document> {
        self.read_with_files(input, &HashMap::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use docmux_ast::Block;

    #[test]
    fn reader_trait_metadata() {
        let reader = LatexReader::new();
        assert_eq!(reader.format(), "latex");
        assert!(reader.extensions().contains(&"tex"));
    }

    #[test]
    fn read_simple_document() {
        let reader = LatexReader::new();
        let doc = reader
            .read(
                r"\section{Hello}

Some text.",
            )
            .unwrap();
        assert_eq!(doc.content.len(), 2);
        assert!(matches!(&doc.content[0], Block::Heading { level: 1, .. }));
    }

    #[test]
    fn read_with_files_inlines_input_directive() {
        let reader = LatexReader::new();
        let mut files = HashMap::new();
        files.insert("body.tex".to_string(), "\\section{Body}\nText.".to_string());

        let main = r"\documentclass{article}
\begin{document}
\input{body}
\end{document}";

        let doc = reader.read_with_files(main, &files).unwrap();

        // After flattening, the body must yield a heading and a paragraph.
        let heading_count = doc
            .content
            .iter()
            .filter(|b| matches!(b, Block::Heading { .. }))
            .count();
        assert!(heading_count >= 1, "expected included \\section, got {:?}", doc.content);
        assert!(doc.warnings.is_empty(), "unexpected warnings: {:?}", doc.warnings);
    }

    #[test]
    fn read_unchanged_when_no_files_given() {
        // Regression: existing read() callers should see identical behaviour.
        let reader = LatexReader::new();
        let source = "\\section{Title}\n\nHello world.";
        let doc_via_read = reader.read(source).unwrap();
        let doc_via_with_files = reader.read_with_files(source, &HashMap::new()).unwrap();
        assert_eq!(doc_via_read.content.len(), doc_via_with_files.content.len());
        assert_eq!(doc_via_read.warnings, doc_via_with_files.warnings);
    }
}
```

- [ ] **Step 5.6: Run the new tests**

```sh
cargo test -p docmux-reader-latex -- --nocapture
```

Expected: all tests pass (the existing two plus the three new ones in `lib.rs` plus all `flatten_*` tests).

- [ ] **Step 5.7: Workspace sanity check (clippy + format gate)**

```sh
cargo clippy -p docmux-reader-latex --all-targets -- -D warnings
```

Expected: no warnings. Fix any that appear before moving on.

- [ ] **Step 5.8: Commit**

```sh
git add crates/docmux-reader-latex/
git commit -m "feat(reader-latex): expose read_with_files with \\input/\\include resolution (#4)"
```

---

## Task 6: WASM `convertWithFiles`

**Files:**
- Modify: `crates/docmux-wasm/src/lib.rs`

- [ ] **Step 6.1: Add helper for decoding a JS `Map<string, Uint8Array>` to a Rust `HashMap<String, String>`**

Edit `crates/docmux-wasm/src/lib.rs`. Add a new helper just below `js_map_to_resources` (around line 71):

```rust
/// Convert a JS `Map<string, Uint8Array>` to a Rust `HashMap<String, String>`,
/// decoding each entry as UTF-8. Entries that fail to decode are skipped.
fn js_map_to_text_files(map: &js_sys::Map) -> HashMap<String, String> {
    let mut files = HashMap::new();
    map.for_each(&mut |value, key| {
        if let Some(name) = key.as_string() {
            let arr = js_sys::Uint8Array::from(value);
            let bytes = arr.to_vec();
            if let Ok(text) = String::from_utf8(bytes) {
                files.insert(name, text);
            }
            // else: silently skip — the include will warn "file not found".
        }
    });
    files
}
```

- [ ] **Step 6.2: Add the new exported `convert_with_files` function**

Append below `convert_bytes_to_bytes` (around line 185) — i.e. before `convert_inner`:

```rust
/// Convert a LaTeX document with `\input{}` / `\include{}` resolved against
/// the supplied file map. Currently only meaningful for `from = "latex"`;
/// for other formats, `files` is ignored (use `convertWithResources` instead).
///
/// # Arguments
/// - `input` — main `.tex` source as a string
/// - `from` — input format name (typically `"latex"`)
/// - `to` — output format name (e.g. `"markdown"`)
/// - `files` — `Map<string, Uint8Array>` of included files (UTF-8). Keys are
///   filenames as referenced by `\input{X}` (with or without `.tex`).
/// - `resources` — `Map<string, Uint8Array>` of binary resources for the writer
///   (images embedded in HTML/DOCX/etc.). Pass an empty Map if not needed.
/// - `standalone` — produce a complete output document (HTML head, LaTeX
///   preamble, etc.)
#[wasm_bindgen(js_name = "convertWithFiles")]
pub fn convert_with_files(
    input: &str,
    from: &str,
    to: &str,
    files: &js_sys::Map,
    resources: &js_sys::Map,
    standalone: bool,
) -> Result<String, JsError> {
    let reg = build_registry();
    let writer = reg
        .find_writer(to)
        .ok_or_else(|| JsError::new(&format!("unsupported output format: {to}")))?;

    let mut doc = if from == "latex" || from == "tex" {
        let text_files = js_map_to_text_files(files);
        docmux_reader_latex::LatexReader::new()
            .read_with_files(input, &text_files)
            .map_err(|e| JsError::new(&e.to_string()))?
    } else {
        let reader = reg
            .find_reader(from)
            .ok_or_else(|| JsError::new(&format!("unsupported input format: {from}")))?;
        reader
            .read(input)
            .map_err(|e| JsError::new(&e.to_string()))?
    };

    doc.resources = js_map_to_resources(resources);

    let ctx = docmux_core::TransformContext::default();
    let _ = NumberSectionsTransform::new().transform(&mut doc, &ctx);
    let _ = CrossRefTransform::new().transform(&mut doc, &ctx);

    let opts = WriteOptions {
        standalone,
        highlight_style: if to == "html" {
            Some("InspiredGitHub".into())
        } else {
            None
        },
        ..Default::default()
    };
    writer
        .write(&doc, &opts)
        .map_err(|e| JsError::new(&e.to_string()))
}
```

- [ ] **Step 6.3: Build the workspace and the WASM target**

```sh
cargo build -p docmux-wasm
```

Expected: clean build.

```sh
cargo build --target wasm32-unknown-unknown -p docmux-wasm
```

Expected: clean build. (If `wasm32-unknown-unknown` is not installed, run `rustup target add wasm32-unknown-unknown` once and retry.)

- [ ] **Step 6.4: Clippy check on docmux-wasm**

```sh
cargo clippy -p docmux-wasm --all-targets -- -D warnings
```

Expected: no warnings.

- [ ] **Step 6.5: Commit**

```sh
git add crates/docmux-wasm/src/lib.rs
git commit -m "feat(wasm): convertWithFiles for LaTeX \\input resolution (#4)"
```

---

## Task 7: CLI filesystem-based resolution

**Files:**
- Modify: `crates/docmux-cli/src/main.rs`
- Create: `crates/docmux-cli/tests/multi_file_latex.rs`

- [ ] **Step 7.1: Write the integration test first (will fail with the current binary)**

Create `crates/docmux-cli/tests/multi_file_latex.rs`:

```rust
//! Integration test: docmux must follow \input{} directives when given a
//! single LaTeX file on disk.

use std::path::{Path, PathBuf};
use std::process::Command;

fn docmux_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_docmux"))
}

fn tmp_subdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("docmux-multi-file-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create tmp dir");
    dir
}

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("mkdir -p");
    }
    std::fs::write(path, contents).expect("write file");
}

#[test]
fn cli_resolves_input_directives_against_filesystem() {
    let dir = tmp_subdir("basic");
    let main = dir.join("main.tex");
    write_file(
        &main,
        r#"\documentclass{article}
\begin{document}
\input{intro}
\input{body}
\end{document}
"#,
    );
    write_file(
        &dir.join("intro.tex"),
        "\\section{Introduction}\nThis is the introduction.\n",
    );
    write_file(
        &dir.join("body.tex"),
        "\\section{Body}\nSome body text.\n",
    );

    let output = Command::new(docmux_bin())
        .arg(&main)
        .arg("--to")
        .arg("markdown")
        .output()
        .expect("run docmux");

    assert!(
        output.status.success(),
        "docmux failed: stderr={}",
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("Introduction"), "missing intro heading; got: {stdout}");
    assert!(
        stdout.contains("This is the introduction."),
        "missing intro body; got: {stdout}"
    );
    assert!(stdout.contains("Body"), "missing body heading; got: {stdout}");
    assert!(
        stdout.contains("Some body text."),
        "missing body content; got: {stdout}"
    );
}

#[test]
fn cli_resolves_nested_subdir_includes() {
    let dir = tmp_subdir("nested");
    let main = dir.join("paper.tex");
    write_file(
        &main,
        r#"\documentclass{article}
\begin{document}
\input{sec/0_abs}
\input{sec/1_intro}
\end{document}
"#,
    );
    write_file(
        &dir.join("sec/0_abs.tex"),
        "\\section*{Abstract}\nAbstract content here.\n",
    );
    write_file(
        &dir.join("sec/1_intro.tex"),
        "\\section{Introduction}\nIntro content.\n",
    );

    let output = Command::new(docmux_bin())
        .arg(&main)
        .arg("--to")
        .arg("markdown")
        .output()
        .expect("run docmux");

    assert!(output.status.success(), "stderr={}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Abstract content here."), "got: {stdout}");
    assert!(stdout.contains("Intro content."), "got: {stdout}");
}
```

- [ ] **Step 7.2: Confirm both tests fail with current binary**

```sh
cargo test -p docmux-cli --test multi_file_latex -- --nocapture
```

Expected: FAIL — output is the preamble-only Markdown (no intro/body content) because the CLI doesn't yet resolve `\input`.

- [ ] **Step 7.3: Add a `latex_include_scan` helper to the CLI**

In `crates/docmux-cli/src/main.rs`, near the other helper `fn` definitions (after `apply_metadata_overrides` is a fine spot — find it via `grep -n "fn apply_metadata_overrides" crates/docmux-cli/src/main.rs`), add:

```rust
/// Scan a LaTeX source string for `\input{X}` and `\include{X}` references.
/// Returns the raw filename arguments as written in the source, in
/// occurrence order (duplicates preserved — the caller dedups via HashMap).
fn scan_latex_includes(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    for cmd in ["\\input", "\\include"] {
        let mut search_start = 0;
        while let Some(found) = source[search_start..].find(cmd) {
            let abs = search_start + found;
            let after = abs + cmd.len();
            // Skip whitespace.
            let rest = source[after..].trim_start();
            if let Some(stripped) = rest.strip_prefix('{') {
                if let Some(close) = stripped.find('}') {
                    out.push(stripped[..close].trim().to_string());
                }
            }
            search_start = after;
        }
    }
    out
}

/// Recursively load every file referenced by `\input` / `\include` starting
/// from `source`, resolving paths against `base_dir`. Returns a map keyed by
/// the raw argument as written in the directive (e.g. `"intro"`,
/// `"sec/0_abs"`). Missing files are silently skipped — the reader's
/// flatten pass will warn for them.
fn load_latex_includes_from_disk(
    source: &str,
    base_dir: &std::path::Path,
) -> HashMap<String, String> {
    let mut files: HashMap<String, String> = HashMap::new();
    let mut queue: Vec<String> = scan_latex_includes(source);

    while let Some(arg) = queue.pop() {
        if files.contains_key(&arg) {
            continue;
        }
        let cleaned = arg.trim_start_matches("./");
        let candidates = [
            base_dir.join(cleaned),
            base_dir.join(format!("{cleaned}.tex")),
            base_dir.join(format!("{cleaned}.ltx")),
        ];
        for cand in &candidates {
            if let Ok(content) = std::fs::read_to_string(cand) {
                queue.extend(scan_latex_includes(&content));
                files.insert(arg.clone(), content);
                break;
            }
        }
    }
    files
}
```

- [ ] **Step 7.4: Wire it into the text-reading branch**

In `crates/docmux-cli/src/main.rs`, locate the block around lines 280–320 (the `// Text path — read and concatenate all inputs` block) and replace the `match reader.read(&combined_input)` call (line 313) so that when the reader is the LaTeX reader **and** there is exactly one non-stdin input path, we route through `read_with_files`.

Before the existing `match reader.read(&combined_input) {`, insert:

```rust
        // LaTeX special path: when there's exactly one file on disk, resolve
        // \input / \include against the filesystem so multi-file papers work.
        let is_latex = matches!(from.as_str(), "latex" | "tex");
        let single_disk_path = cli
            .input
            .iter()
            .all(|p| p.to_str() != Some("-"))
            && cli.input.len() == 1;

        if is_latex && single_disk_path {
            // SAFETY: len == 1, so first() is Some; checked path != "-".
            let main_path = &cli.input[0];
            let base_dir = main_path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let files = load_latex_includes_from_disk(&combined_input, base_dir);
            match docmux_reader_latex::LatexReader::new()
                .read_with_files(&combined_input, &files)
            {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("docmux: parse error: {e}");
                    std::process::exit(1);
                }
            }
        } else {
            match reader.read(&combined_input) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("docmux: parse error: {e}");
                    std::process::exit(1);
                }
            }
        }
```

Then **delete** the now-superseded standalone `match reader.read(&combined_input) { ... }` block below the insertion point. The outer assignment is `let mut doc = { ... };` so the `if/else` becomes the block's tail expression.

If the existing code looked like:

```rust
        match reader.read(&combined_input) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("docmux: parse error: {e}");
                std::process::exit(1);
            }
        }
    };
```

It must end up as:

```rust
        let is_latex = matches!(from.as_str(), "latex" | "tex");
        let single_disk_path = cli
            .input
            .iter()
            .all(|p| p.to_str() != Some("-"))
            && cli.input.len() == 1;

        if is_latex && single_disk_path {
            let main_path = &cli.input[0];
            let base_dir = main_path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let files = load_latex_includes_from_disk(&combined_input, base_dir);
            match docmux_reader_latex::LatexReader::new()
                .read_with_files(&combined_input, &files)
            {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("docmux: parse error: {e}");
                    std::process::exit(1);
                }
            }
        } else {
            match reader.read(&combined_input) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("docmux: parse error: {e}");
                    std::process::exit(1);
                }
            }
        }
    };
```

Note: the `reader` variable is still used in the `else` branch for the non-LaTeX text formats, so leave the `let reader = ...;` lookup above untouched.

- [ ] **Step 7.5: Add a unit test for `scan_latex_includes`**

In `crates/docmux-cli/src/main.rs`, at the bottom of the file (find or create a `#[cfg(test)] mod tests {` block — if one already exists, append; otherwise add this new block at end-of-file):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_latex_includes_finds_input_and_include() {
        let src = r#"\documentclass{article}
\begin{document}
\input{intro}
\input{./sec/body}
\include{appendix}
\end{document}"#;
        let refs = scan_latex_includes(src);
        assert!(refs.contains(&"intro".to_string()), "got {refs:?}");
        assert!(refs.contains(&"./sec/body".to_string()), "got {refs:?}");
        assert!(refs.contains(&"appendix".to_string()), "got {refs:?}");
    }

    #[test]
    fn scan_latex_includes_ignores_commands_with_no_brace() {
        let src = "\\input no-brace-here";
        assert!(scan_latex_includes(src).is_empty());
    }
}
```

- [ ] **Step 7.6: Run the integration test + the unit tests**

```sh
cargo test -p docmux-cli --test multi_file_latex -- --nocapture
cargo test -p docmux-cli --lib -- scan_latex_includes
```

Expected: all PASS.

- [ ] **Step 7.7: Clippy + full CLI test sweep**

```sh
cargo clippy -p docmux-cli --all-targets -- -D warnings
cargo test -p docmux-cli
```

Expected: zero warnings, all CLI tests green (existing smoke + golden + new multi-file).

- [ ] **Step 7.8: Commit**

```sh
git add crates/docmux-cli/src/main.rs crates/docmux-cli/tests/multi_file_latex.rs
git commit -m "feat(cli): resolve LaTeX \\input/\\include from filesystem (#4)"
```

---

## Task 8: Documentation

**Files:**
- Modify: `README.md`
- Modify: `.claude/rules/latex.md`

- [ ] **Step 8.1: Add a Multi-file LaTeX section to README.md**

Locate the existing usage section in `README.md` (search for `convertStandalone` or `convert(` to find the JS examples). Append a new subsection:

````markdown
### Multi-file LaTeX (`\input` / `\include`)

For LaTeX papers that split their body across multiple files, pass a `files`
map alongside the main source:

```js
import init, { convertWithFiles } from '@docmux/wasm/web';
await init();

const files = new Map([
  ['intro.tex', new TextEncoder().encode(introSource)],
  ['body.tex', new TextEncoder().encode(bodySource)],
]);

const md = convertWithFiles(
  mainTex,
  'latex',
  'markdown',
  files,
  new Map(), // resources (images) — empty if none
  false,     // standalone
);
```

`\input{intro}` resolves against `intro.tex` (the `.tex` extension is added
automatically if the bare key is missing). The CLI does the same thing
automatically when you point it at a single `.tex` file on disk:

```sh
docmux paper/main.tex --to markdown
```
````

- [ ] **Step 8.2: Update `.claude/rules/latex.md`**

Open `.claude/rules/latex.md` and add a bullet under `## Reader`:

```markdown
- `\input{X}` / `\include{X}` are resolved against an in-memory file map
  via `LatexReader::read_with_files(input, &HashMap<String, String>)`. The
  CLI scans the main file and loads referenced files from the parent
  directory. WASM exposes this via `convertWithFiles(...)`.
```

- [ ] **Step 8.3: Commit**

```sh
git add README.md .claude/rules/latex.md
git commit -m "docs: multi-file LaTeX support (#4)"
```

---

## Task 9: Workspace-wide verification

**Files:** none (validation only)

- [ ] **Step 9.1: Format check**

```sh
cargo fmt --all -- --check
```

Expected: clean. If not, run `cargo fmt --all` and re-commit.

- [ ] **Step 9.2: Workspace clippy**

```sh
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: zero warnings.

- [ ] **Step 9.3: Full workspace test**

```sh
cargo test --workspace
```

Expected: 660 + new tests, all green. (Pre-flight count was 660 — final should be ~675 with the 12 flatten tests, 3 reader integration tests, 2 CLI multi-file tests, 2 CLI scan unit tests.)

- [ ] **Step 9.4: WASM target build**

```sh
cargo build --target wasm32-unknown-unknown -p docmux-wasm
```

Expected: clean build.

- [ ] **Step 9.5: Manual sanity check via CLI on a real-ish layout**

```sh
TMP=$(mktemp -d)
mkdir -p "$TMP/sec"
cat > "$TMP/main.tex" <<'EOF'
\documentclass{article}
\title{Sanity}
\begin{document}
\maketitle
\input{sec/intro}
\input{sec/body}
\end{document}
EOF
cat > "$TMP/sec/intro.tex" <<'EOF'
\section{Introduction}
Some intro text.
EOF
cat > "$TMP/sec/body.tex" <<'EOF'
\section{Body}
Some body text.
EOF
./target/debug/docmux "$TMP/main.tex" --to markdown
```

Expected: stdout contains both section headings and both bodies.

- [ ] **Step 9.6: Final commit if anything needed adjustment**

```sh
git status
# If clean — nothing to commit.
# If any rustfmt-only changes remain: git add -A && git commit -m "chore: rustfmt"
```

---

## Self-Review

**Spec coverage:**

- `flatten_includes` token-stream pre-pass → Tasks 1–5.
- Extension fallback (`X`, `X.tex`, `X.ltx`) → Task 2.
- `./` prefix stripping → Task 2.
- Recursive includes → Task 3.
- Cycle detection → Task 3.
- Verbatim guard → Task 4.
- Missing file warning → Task 4 (test asserts the path covered by Task 1's emit).
- Malformed (no brace) → Task 4.
- Depth guard (>32) → Task 5.
- `\include` same as `\input` → Task 4.
- `read_with_files` inherent method → Task 5.
- `Reader::read` delegating with empty map → Task 5.
- Regression test (existing behaviour unchanged) → Task 5.
- WASM `convertWithFiles` → Task 6.
- UTF-8 decode skip on bad bytes → Task 6.
- `js_map_to_text_files` helper → Task 6.
- CLI single-file LaTeX detection → Task 7.
- CLI filesystem include loading → Task 7.
- CLI integration test → Task 7.
- README docs → Task 8.
- `.claude/rules/latex.md` update → Task 8.
- Rustdoc on `read_with_files` → Task 5 (embedded).

**Placeholder scan:** Each step has concrete code, exact paths, and runnable commands. No "implement later" / "add validation" / "TODO".

**Type consistency:** `flatten_includes`, `flatten_inner`, `resolve_target`, `read_brace_arg`, `is_verbatim_env` signatures are stable from definition through call sites. `read_with_files` signature `(&self, &str, &HashMap<String, String>) -> Result<Document>` is consistent across Rust, WASM, and CLI tasks. `js_map_to_text_files` returns `HashMap<String, String>` matching the reader's expectation.
