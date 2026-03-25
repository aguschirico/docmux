//! # Golden file tests for the docmux pipeline
//!
//! Each fixture is a pair: `<name>.md` (input) + `<name>.html` (expected output).
//!
//! ## How it works
//!
//! 1. The harness discovers all `.md` files under `tests/fixtures/` (workspace root)
//! 2. For each `.md`, it runs the Markdown → HTML pipeline
//! 3. If a matching `.html` exists, the output is compared against it
//! 4. If no `.html` exists, one is auto-generated (first run bootstrap)
//!
//! ## Updating expectations
//!
//! When you intentionally change the HTML output, run:
//!
//! ```sh
//! DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden
//! ```
//!
//! This overwrites all `.html` files. Review the diff with `git diff` before committing.

use docmux_core::{Reader, WriteOptions, Writer};
use docmux_reader_latex::LatexReader;
use docmux_reader_markdown::MarkdownReader;
use docmux_reader_typst::TypstReader;
use docmux_writer_html::HtmlWriter;
use docmux_writer_latex::LatexWriter;
use docmux_writer_typst::TypstWriter;
use std::path::{Path, PathBuf};

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Workspace root, computed from this crate's manifest dir (crates/docmux-cli).
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent() // crates/
        .and_then(Path::parent) // workspace root
        .expect("could not resolve workspace root")
        .to_path_buf()
}

fn fixtures_dir() -> PathBuf {
    workspace_root().join("tests/fixtures")
}

fn update_mode() -> bool {
    std::env::var("DOCMUX_UPDATE_EXPECTATIONS").is_ok()
}

/// Recursively discover all `.md` files under `dir`.
fn discover_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if !dir.is_dir() {
        return results;
    }
    for entry in std::fs::read_dir(dir).expect("read fixtures dir") {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        if path.is_dir() {
            results.extend(discover_fixtures(&path));
        } else if path.extension().is_some_and(|ext| ext == "md") {
            results.push(path);
        }
    }
    results.sort();
    results
}

/// Convert Markdown → HTML using the pipeline (fragment mode, no standalone wrapper).
fn convert_md_to_html(input: &str) -> String {
    let reader = MarkdownReader::new();
    let writer = HtmlWriter::new();
    let opts = WriteOptions::default();

    let doc = reader
        .read(input)
        .expect("markdown reader should not fail on fixture");
    writer
        .write(&doc, &opts)
        .expect("html writer should not fail")
}

/// Convert Markdown → LaTeX using the pipeline (fragment mode, no standalone wrapper).
fn convert_md_to_latex(input: &str) -> String {
    let reader = MarkdownReader::new();
    let writer = LatexWriter::new();
    let opts = WriteOptions::default();

    let doc = reader
        .read(input)
        .expect("markdown reader should not fail on fixture");
    writer
        .write(&doc, &opts)
        .expect("latex writer should not fail")
}

/// Human-readable test name from a fixture path.
///
/// `tests/fixtures/basic/heading.md` → `basic/heading`
fn test_name(fixture_path: &Path, base: &Path) -> String {
    fixture_path
        .strip_prefix(base)
        .unwrap_or(fixture_path)
        .with_extension("")
        .to_string_lossy()
        .replace('\\', "/")
}

// ─── Golden test ────────────────────────────────────────────────────────────

#[test]
fn golden_md_to_html() {
    let base = fixtures_dir();
    let fixtures = discover_fixtures(&base);

    assert!(
        !fixtures.is_empty(),
        "No .md fixtures found under {}",
        base.display()
    );

    let mut failures: Vec<String> = Vec::new();
    let mut generated = 0u32;
    let mut updated = 0u32;

    for fixture_path in &fixtures {
        let name = test_name(fixture_path, &base);
        let expected_path = fixture_path.with_extension("html");

        let input = std::fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read input: {e}"));

        let actual = convert_md_to_html(&input);

        if update_mode() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            updated += 1;
            eprintln!("  updated: {name}");
            continue;
        }

        if !expected_path.exists() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            generated += 1;
            eprintln!("  generated: {name} (new fixture — review the .html file)");
            continue;
        }

        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read expected: {e}"));

        if actual != expected {
            failures.push(format!(
                "━━━ MISMATCH: {name} ━━━\n\
                 --- expected ({path})\n\
                 +++ actual\n\n\
                 {diff}\n\
                 Hint: run `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden` to update.\n",
                path = expected_path.display(),
                diff = line_diff(&expected, &actual),
            ));
        }
    }

    if generated > 0 {
        eprintln!(
            "\n  {} new expectation file(s) generated. Review and commit them.",
            generated
        );
    }

    if updated > 0 {
        eprintln!("\n  {} expectation file(s) updated.", updated);
    }

    if !failures.is_empty() {
        panic!(
            "\n\n{count} golden file(s) mismatched:\n\n{details}",
            count = failures.len(),
            details = failures.join("\n"),
        );
    }
}

#[test]
fn golden_md_to_latex() {
    let base = fixtures_dir();
    let fixtures = discover_fixtures(&base);

    assert!(
        !fixtures.is_empty(),
        "No .md fixtures found under {}",
        base.display()
    );

    let mut failures: Vec<String> = Vec::new();
    let mut generated = 0u32;
    let mut updated = 0u32;

    for fixture_path in &fixtures {
        let name = test_name(fixture_path, &base);
        let expected_path = fixture_path.with_extension("tex");

        let input = std::fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read input: {e}"));

        let actual = convert_md_to_latex(&input);

        if update_mode() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            updated += 1;
            eprintln!("  updated: {name}.tex");
            continue;
        }

        if !expected_path.exists() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            generated += 1;
            eprintln!("  generated: {name}.tex (new fixture — review the .tex file)");
            continue;
        }

        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read expected: {e}"));

        if actual != expected {
            failures.push(format!(
                "━━━ MISMATCH: {name}.tex ━━━\n\
                 --- expected ({path})\n\
                 +++ actual\n\n\
                 {diff}\n\
                 Hint: run `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden` to update.\n",
                path = expected_path.display(),
                diff = line_diff(&expected, &actual),
            ));
        }
    }

    if generated > 0 {
        eprintln!(
            "\n  {} new LaTeX expectation file(s) generated. Review and commit them.",
            generated
        );
    }

    if updated > 0 {
        eprintln!("\n  {} LaTeX expectation file(s) updated.", updated);
    }

    if !failures.is_empty() {
        panic!(
            "\n\n{count} LaTeX golden file(s) mismatched:\n\n{details}",
            count = failures.len(),
            details = failures.join("\n"),
        );
    }
}

// ─── LaTeX → HTML golden tests ──────────────────────────────────────────────

fn convert_tex_to_html(input: &str) -> String {
    let reader = LatexReader::new();
    let writer = HtmlWriter::new();
    let opts = WriteOptions::default();
    let doc = reader
        .read(input)
        .expect("latex reader should not fail on fixture");
    writer
        .write(&doc, &opts)
        .expect("html writer should not fail")
}

fn discover_tex_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if !dir.is_dir() {
        return results;
    }
    for entry in std::fs::read_dir(dir).expect("read fixtures dir") {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        if path.is_dir() {
            results.extend(discover_tex_fixtures(&path));
        } else if path.extension().is_some_and(|ext| ext == "tex") {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            if stem.starts_with("latex-") {
                results.push(path);
            }
        }
    }
    results.sort();
    results
}

#[test]
fn golden_tex_to_html() {
    let base = fixtures_dir();
    let fixtures = discover_tex_fixtures(&base);

    if fixtures.is_empty() {
        eprintln!("No .tex fixtures found (skipping golden_tex_to_html)");
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut generated = 0u32;
    let mut updated = 0u32;

    for fixture_path in &fixtures {
        let name = test_name(fixture_path, &base);
        let expected_path = fixture_path.with_extension("tex.html");

        let input = std::fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read input: {e}"));
        let actual = convert_tex_to_html(&input);

        if update_mode() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            updated += 1;
            eprintln!("  updated: {name}.tex.html");
            continue;
        }

        if !expected_path.exists() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            generated += 1;
            eprintln!("  generated: {name}.tex.html (new — review the file)");
            continue;
        }

        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read expected: {e}"));

        if actual != expected {
            failures.push(format!(
                "━━━ MISMATCH: {name}.tex.html ━━━\n--- expected ({path})\n+++ actual\n\n{diff}\nHint: run `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden` to update.\n",
                path = expected_path.display(),
                diff = line_diff(&expected, &actual),
            ));
        }
    }

    if generated > 0 {
        eprintln!("\n  {} new .tex.html expectation(s) generated.", generated);
    }
    if updated > 0 {
        eprintln!("\n  {} .tex.html expectation(s) updated.", updated);
    }

    if !failures.is_empty() {
        panic!(
            "\n\n{count} .tex→.html golden file(s) mismatched:\n\n{details}",
            count = failures.len(),
            details = failures.join("\n"),
        );
    }
}

// ─── Typst → HTML / LaTeX golden tests ─────────────────────────────────────

fn convert_typ_to_html(input: &str) -> String {
    let reader = TypstReader::new();
    let writer = HtmlWriter::new();
    let opts = WriteOptions::default();
    let doc = reader
        .read(input)
        .expect("typst reader should not fail on fixture");
    writer
        .write(&doc, &opts)
        .expect("html writer should not fail")
}

fn convert_typ_to_latex(input: &str) -> String {
    let reader = TypstReader::new();
    let writer = LatexWriter::new();
    let opts = WriteOptions::default();
    let doc = reader
        .read(input)
        .expect("typst reader should not fail on fixture");
    writer
        .write(&doc, &opts)
        .expect("latex writer should not fail")
}

fn discover_typ_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if !dir.is_dir() {
        return results;
    }
    for entry in std::fs::read_dir(dir).expect("read fixtures dir") {
        let entry = entry.expect("read dir entry");
        let path = entry.path();
        if path.is_dir() {
            results.extend(discover_typ_fixtures(&path));
        } else if path.extension().is_some_and(|ext| ext == "typ") {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            // Only match primary fixture files: stem must start with "typst-"
            // and must not contain a dot (ruling out derived files like typst-foo.typ.typ).
            if stem.starts_with("typst-") && !stem.contains('.') {
                results.push(path);
            }
        }
    }
    results.sort();
    results
}

#[test]
fn golden_typ_to_html() {
    let base = fixtures_dir();
    let fixtures = discover_typ_fixtures(&base);

    if fixtures.is_empty() {
        eprintln!("No .typ fixtures found (skipping golden_typ_to_html)");
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut generated = 0u32;
    let mut updated = 0u32;

    for fixture_path in &fixtures {
        let name = test_name(fixture_path, &base);
        let expected_path = fixture_path.with_extension("typ.html");

        let input = std::fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read input: {e}"));
        let actual = convert_typ_to_html(&input);

        if update_mode() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            updated += 1;
            eprintln!("  updated: {name}.typ.html");
            continue;
        }

        if !expected_path.exists() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            generated += 1;
            eprintln!("  generated: {name}.typ.html (new — review the file)");
            continue;
        }

        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read expected: {e}"));

        if actual != expected {
            failures.push(format!(
                "━━━ MISMATCH: {name}.typ.html ━━━\n--- expected ({path})\n+++ actual\n\n{diff}\nHint: run `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden` to update.\n",
                path = expected_path.display(),
                diff = line_diff(&expected, &actual),
            ));
        }
    }

    if generated > 0 {
        eprintln!("\n  {} new .typ.html expectation(s) generated.", generated);
    }
    if updated > 0 {
        eprintln!("\n  {} .typ.html expectation(s) updated.", updated);
    }

    if !failures.is_empty() {
        panic!(
            "\n\n{count} .typ→.html golden file(s) mismatched:\n\n{details}",
            count = failures.len(),
            details = failures.join("\n"),
        );
    }
}

#[test]
fn golden_typ_to_latex() {
    let base = fixtures_dir();
    let fixtures = discover_typ_fixtures(&base);

    if fixtures.is_empty() {
        eprintln!("No .typ fixtures found (skipping golden_typ_to_latex)");
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut generated = 0u32;
    let mut updated = 0u32;

    for fixture_path in &fixtures {
        let name = test_name(fixture_path, &base);
        let expected_path = fixture_path.with_extension("typ.tex");

        let input = std::fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read input: {e}"));
        let actual = convert_typ_to_latex(&input);

        if update_mode() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            updated += 1;
            eprintln!("  updated: {name}.typ.tex");
            continue;
        }

        if !expected_path.exists() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            generated += 1;
            eprintln!("  generated: {name}.typ.tex (new — review the file)");
            continue;
        }

        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read expected: {e}"));

        if actual != expected {
            failures.push(format!(
                "━━━ MISMATCH: {name}.typ.tex ━━━\n--- expected ({path})\n+++ actual\n\n{diff}\nHint: run `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden` to update.\n",
                path = expected_path.display(),
                diff = line_diff(&expected, &actual),
            ));
        }
    }

    if generated > 0 {
        eprintln!("\n  {} new .typ.tex expectation(s) generated.", generated);
    }
    if updated > 0 {
        eprintln!("\n  {} .typ.tex expectation(s) updated.", updated);
    }

    if !failures.is_empty() {
        panic!(
            "\n\n{count} .typ→.tex golden file(s) mismatched:\n\n{details}",
            count = failures.len(),
            details = failures.join("\n"),
        );
    }
}

fn convert_typ_to_typst(input: &str) -> String {
    let reader = TypstReader::new();
    let writer = TypstWriter::new();
    let opts = WriteOptions::default();
    let doc = reader
        .read(input)
        .expect("typst reader should not fail on fixture");
    writer
        .write(&doc, &opts)
        .expect("typst writer should not fail")
}

#[test]
fn golden_typ_to_typst() {
    let base = fixtures_dir();
    let fixtures = discover_typ_fixtures(&base);

    if fixtures.is_empty() {
        eprintln!("No .typ fixtures found (skipping golden_typ_to_typst)");
        return;
    }

    let mut failures: Vec<String> = Vec::new();
    let mut generated = 0u32;
    let mut updated = 0u32;

    for fixture_path in &fixtures {
        let name = test_name(fixture_path, &base);
        let expected_path = fixture_path.with_extension("typ.typ");

        let input = std::fs::read_to_string(fixture_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read input: {e}"));
        let actual = convert_typ_to_typst(&input);

        if update_mode() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            updated += 1;
            eprintln!("  updated: {name}.typ.typ");
            continue;
        }

        if !expected_path.exists() {
            std::fs::write(&expected_path, &actual)
                .unwrap_or_else(|e| panic!("[{name}] failed to write expected: {e}"));
            generated += 1;
            eprintln!("  generated: {name}.typ.typ (new — review the file)");
            continue;
        }

        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|e| panic!("[{name}] failed to read expected: {e}"));

        if actual != expected {
            failures.push(format!(
                "━━━ MISMATCH: {name}.typ.typ ━━━\n--- expected ({path})\n+++ actual\n\n{diff}\nHint: run `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli --test golden` to update.\n",
                path = expected_path.display(),
                diff = line_diff(&expected, &actual),
            ));
        }
    }

    if generated > 0 {
        eprintln!("\n  {} new .typ.typ expectation(s) generated.", generated);
    }
    if updated > 0 {
        eprintln!("\n  {} .typ.typ expectation(s) updated.", updated);
    }

    if !failures.is_empty() {
        panic!(
            "\n\n{count} .typ→.typ golden file(s) mismatched:\n\n{details}",
            count = failures.len(),
            details = failures.join("\n"),
        );
    }
}

// ─── Diff helper ────────────────────────────────────────────────────────────

/// Minimal line-by-line diff for readable test output.
fn line_diff(expected: &str, actual: &str) -> String {
    let exp_lines: Vec<&str> = expected.lines().collect();
    let act_lines: Vec<&str> = actual.lines().collect();
    let mut out = String::new();
    let max = exp_lines.len().max(act_lines.len());

    for i in 0..max {
        let exp = exp_lines.get(i).copied().unwrap_or("");
        let act = act_lines.get(i).copied().unwrap_or("");
        if exp != act {
            out.push_str(&format!("  L{line}:\n", line = i + 1));
            out.push_str(&format!("    - {exp}\n"));
            out.push_str(&format!("    + {act}\n"));
        }
    }

    if exp_lines.len() != act_lines.len() {
        out.push_str(&format!(
            "  (expected {} lines, got {} lines)\n",
            exp_lines.len(),
            act_lines.len()
        ));
    }

    if out.is_empty() {
        "  (trailing whitespace or newline difference)\n".to_string()
    } else {
        out
    }
}
