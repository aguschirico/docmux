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
    assert!(html.contains("<h1>"), "Expected h1 in output");

    // Clean up
    let _ = std::fs::remove_file(&output_file);
}

#[test]
fn standalone_flag_produces_full_html() {
    let input = fixtures_dir().join("basic/paragraph.md");
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
