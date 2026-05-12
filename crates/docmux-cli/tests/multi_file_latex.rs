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
    write_file(&dir.join("body.tex"), "\\section{Body}\nSome body text.\n");

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

    assert!(
        stdout.contains("Introduction"),
        "missing intro heading; got: {stdout}"
    );
    assert!(
        stdout.contains("This is the introduction."),
        "missing intro body; got: {stdout}"
    );
    assert!(
        stdout.contains("Body"),
        "missing body heading; got: {stdout}"
    );
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

    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Abstract content here."), "got: {stdout}");
    assert!(stdout.contains("Intro content."), "got: {stdout}");
}
