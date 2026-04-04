//! # CLI smoke tests
//!
//! Tests the `docmux` binary end-to-end: does it run, does it produce
//! correct output, does it handle errors gracefully?

use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("could not resolve workspace root")
        .to_path_buf()
}

fn fixtures_dir() -> PathBuf {
    workspace_root().join("tests/fixtures")
}

/// Build the binary once and return the path.
/// `cargo test` already builds the workspace, so the binary should exist.
fn docmux_bin() -> PathBuf {
    // The binary is built by cargo in the target directory
    let bin = env!("CARGO_BIN_EXE_docmux");
    PathBuf::from(bin)
}

// ─── Success cases ──────────────────────────────────────────────────────────

#[test]
fn converts_markdown_to_html_stdout() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let output = Command::new(docmux_bin())
        .arg(&input)
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success(), "docmux exited with error");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("<p>"),
        "Expected HTML output, got: {stdout}"
    );
    assert!(
        stdout.contains("<strong>bold</strong>"),
        "Expected bold markup in output"
    );
}

#[test]
fn converts_to_file_output() {
    let input = fixtures_dir().join("basic/heading.md");
    let tmp_dir = std::env::temp_dir().join("docmux-test");
    std::fs::create_dir_all(&tmp_dir).ok();
    let output_file = tmp_dir.join("heading-output.html");

    // Clean up any previous run
    let _ = std::fs::remove_file(&output_file);

    let result = Command::new(docmux_bin())
        .arg(&input)
        .arg("--output")
        .arg(&output_file)
        .output()
        .expect("failed to run docmux");

    assert!(result.status.success(), "docmux exited with error");
    assert!(output_file.exists(), "Output file was not created");

    let html = std::fs::read_to_string(&output_file).expect("read output");
    assert!(html.contains("<h1"), "Expected h1 in output");

    // Clean up
    let _ = std::fs::remove_file(&output_file);
}

#[test]
fn standalone_flag_produces_full_html() {
    // Use frontmatter.md so the document has a title (template only emits
    // <title> when the variable is set).
    let input = fixtures_dir().join("basic/frontmatter.md");
    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("--standalone")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("<!DOCTYPE html>"),
        "Expected full HTML document"
    );
    assert!(stdout.contains("<title>"), "Expected <title> tag");
    assert!(
        stdout.contains("katex"),
        "Expected KaTeX include in standalone mode"
    );
}

#[test]
fn explicit_format_flags() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("--from")
        .arg("markdown")
        .arg("--to")
        .arg("html")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("<p>"));
}

#[test]
fn converts_latex_to_html_stdout() {
    let input = fixtures_dir().join("basic/latex-paragraph.tex");
    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("--to")
        .arg("html")
        .output()
        .expect("failed to run docmux");

    assert!(
        output.status.success(),
        "docmux exited with error: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("<strong>bold</strong>"),
        "Expected bold in output, got: {stdout}"
    );
    assert!(
        stdout.contains("<em>italic</em>"),
        "Expected italic in output, got: {stdout}"
    );
}

#[test]
fn latex_auto_detects_format_by_extension() {
    let input = fixtures_dir().join("basic/latex-paragraph.tex");
    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("--to")
        .arg("html")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("<p>"), "Expected HTML paragraph output");
}

// ─── Typst smoke tests ──────────────────────────────────────────────────────

#[test]
fn converts_typst_to_html_stdout() {
    let tmp = std::env::temp_dir().join("docmux-test");
    std::fs::create_dir_all(&tmp).ok();
    let input_file = tmp.join("test.typ");
    std::fs::write(&input_file, "= Hello\n\nWorld.").unwrap();

    let output = Command::new(docmux_bin())
        .arg(&input_file)
        .arg("--to")
        .arg("html")
        .output()
        .expect("failed to run docmux");

    assert!(
        output.status.success(),
        "docmux exited with error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("<h1>"),
        "Expected heading in output, got: {stdout}"
    );
}

#[test]
fn converts_typst_to_latex_stdout() {
    let tmp = std::env::temp_dir().join("docmux-test");
    std::fs::create_dir_all(&tmp).ok();
    let input_file = tmp.join("test.typ");
    std::fs::write(&input_file, "= Hello\n\n*Bold* and _italic_.").unwrap();

    let output = Command::new(docmux_bin())
        .arg(&input_file)
        .arg("--to")
        .arg("latex")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\\section"), "Expected LaTeX section");
}

#[test]
fn typst_format_autodetected() {
    let tmp = std::env::temp_dir().join("docmux-test");
    std::fs::create_dir_all(&tmp).ok();
    let input_file = tmp.join("autodetect.typ");
    std::fs::write(&input_file, "Hello world.").unwrap();

    let output = Command::new(docmux_bin())
        .arg(&input_file)
        .output()
        .expect("failed to run docmux");

    assert!(
        output.status.success(),
        "Typst format should be auto-detected from .typ extension"
    );
}

#[test]
fn converts_typst_to_typst_stdout() {
    let tmp = std::env::temp_dir().join("docmux-test");
    std::fs::create_dir_all(&tmp).ok();
    let input_file = tmp.join("roundtrip.typ");
    std::fs::write(&input_file, "= Hello\n\n*Bold* and _italic_.").unwrap();

    let output = Command::new(docmux_bin())
        .arg(&input_file)
        .arg("--to")
        .arg("typst")
        .output()
        .expect("failed to run docmux");

    assert!(
        output.status.success(),
        "docmux exited with error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("= Hello"),
        "Expected Typst heading in output"
    );
    assert!(stdout.contains("*Bold*"), "Expected bold markup");
}

// ─── Error cases ────────────────────────────────────────────────────────────

#[test]
fn errors_on_missing_input_file() {
    let output = Command::new(docmux_bin())
        .arg("nonexistent-file.md")
        .output()
        .expect("failed to run docmux");

    assert!(
        !output.status.success(),
        "Expected non-zero exit for missing file"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error reading"),
        "Expected error message on stderr, got: {stderr}"
    );
}

#[test]
fn errors_on_unsupported_input_format() {
    // Create a temp file with an unknown extension
    let tmp = std::env::temp_dir().join("docmux-test-unsupported.xyz");
    std::fs::write(&tmp, "some content").ok();

    let output = Command::new(docmux_bin())
        .arg(&tmp)
        .output()
        .expect("failed to run docmux");

    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported input format"),
        "Expected unsupported format error, got: {stderr}"
    );

    let _ = std::fs::remove_file(&tmp);
}

#[test]
fn version_flag() {
    let output = Command::new(docmux_bin())
        .arg("--version")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("docmux"),
        "Expected version string, got: {stdout}"
    );
}

#[test]
fn help_flag() {
    let output = Command::new(docmux_bin())
        .arg("--help")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Universal document converter"));
    assert!(stdout.contains("--output"));
    assert!(stdout.contains("--from"));
    assert!(stdout.contains("--to"));
}

#[test]
fn converts_markdown_to_docx_file() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let tmp_dir = std::env::temp_dir().join("docmux-test");
    std::fs::create_dir_all(&tmp_dir).ok();
    let output_file = tmp_dir.join("paragraph.docx");
    let _ = std::fs::remove_file(&output_file);

    let output = Command::new(docmux_bin())
        .args([input.to_str().unwrap(), "-o", output_file.to_str().unwrap()])
        .output()
        .expect("failed to run docmux");

    assert!(
        output.status.success(),
        "docmux exited with error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output_file.exists(), "output file should exist");

    // Verify it's a valid ZIP
    let bytes = std::fs::read(&output_file).unwrap();
    let cursor = std::io::Cursor::new(&bytes);
    let archive = zip::ZipArchive::new(cursor);
    assert!(archive.is_ok(), "output should be a valid ZIP file");

    let _ = std::fs::remove_file(&output_file);
}

#[test]
fn docx_to_stdout_errors() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let output = Command::new(docmux_bin())
        .args([input.to_str().unwrap(), "-t", "docx"])
        .output()
        .expect("failed to run docmux");

    assert!(!output.status.success(), "docx to stdout should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("requires -o FILE") || stderr.contains("binary format"),
        "expected binary format error, got: {stderr}"
    );
}

// ─── Template tests ─────────────────────────────────────────────────────────

#[test]
fn print_default_template_html() {
    let output = Command::new(docmux_bin())
        .arg("--print-default-template=html")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success(), "exit code was non-zero");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("$body$"),
        "expected $body$ in template, got: {stdout}"
    );
    assert!(
        stdout.contains("<!DOCTYPE html>"),
        "expected HTML doctype in template"
    );
}

#[test]
fn print_default_template_latex() {
    let output = Command::new(docmux_bin())
        .arg("--print-default-template=latex")
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\\documentclass"));
    assert!(stdout.contains("$body$"));
}

#[test]
fn print_default_template_unknown_format_fails() {
    let output = Command::new(docmux_bin())
        .arg("--print-default-template=unknown")
        .output()
        .expect("failed to run docmux");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("no default template"));
}

#[test]
fn custom_template_flag() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let tmp_dir = std::env::temp_dir().join("docmux-template-test");
    std::fs::create_dir_all(&tmp_dir).ok();

    // Write a minimal custom template
    let template_file = tmp_dir.join("custom.html");
    std::fs::write(&template_file, "<custom>$body$</custom>").expect("failed to write template");

    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("-t")
        .arg("html")
        .arg("--template")
        .arg(&template_file)
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success(), "exit code was non-zero");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("<custom>"),
        "expected custom template wrapper, got: {stdout}"
    );
    assert!(
        stdout.contains("<p>"),
        "expected rendered body content in output"
    );

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn template_implies_standalone() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let tmp_dir = std::env::temp_dir().join("docmux-template-test-standalone");
    std::fs::create_dir_all(&tmp_dir).ok();

    let template_file = tmp_dir.join("minimal.html");
    std::fs::write(&template_file, "HEADER\n$body$\nFOOTER").expect("write template");

    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("-t")
        .arg("html")
        .arg("--template")
        .arg(&template_file)
        // Note: no --standalone flag, but --template implies it
        .output()
        .expect("failed to run docmux");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("HEADER"), "template wrapper expected");
    assert!(stdout.contains("FOOTER"), "template wrapper expected");

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn template_nonexistent_file_fails() {
    let input = fixtures_dir().join("basic/paragraph.md");
    let output = Command::new(docmux_bin())
        .arg(&input)
        .arg("-t")
        .arg("html")
        .arg("--template")
        .arg("/nonexistent/template.html")
        .output()
        .expect("failed to run docmux");

    assert!(!output.status.success());
}
