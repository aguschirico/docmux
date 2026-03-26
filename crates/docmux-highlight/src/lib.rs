//! # docmux-highlight
//!
//! Syntax highlighting for docmux, powered by [syntect](https://docs.rs/syntect).
//!
//! Provides language-aware tokenisation of source code with theme-based
//! styling. The output is a structured `Vec<Vec<HighlightToken>>` (tokens
//! per line) that writers can consume to emit HTML `<span>` elements,
//! LaTeX `\textcolor` commands, or any other coloured output.

use std::sync::LazyLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

/// A lazily-initialised default syntax set (all built-in grammars).
static SYNTAX_SET: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);

/// A lazily-initialised default theme set (all built-in colour themes).
static THEME_SET: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

// ─── Public types ────────────────────────────────────────────────────────────

/// An RGBA colour with 8-bit channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// Font style flags for a highlighted token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenStyle {
    pub foreground: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// A single highlighted token: a text fragment with its style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HighlightToken {
    pub text: String,
    pub style: TokenStyle,
}

// ─── Public API ──────────────────────────────────────────────────────────────

/// Highlight `code` using the grammar for `language` and the given `theme`.
///
/// Returns a `Vec` of lines, where each line is a `Vec<HighlightToken>`.
///
/// # Errors
///
/// Returns [`docmux_core::ConvertError::Unsupported`] if the language or
/// theme is not recognised.
pub fn highlight(
    code: &str,
    language: &str,
    theme: &str,
) -> docmux_core::Result<Vec<Vec<HighlightToken>>> {
    let syntax = SYNTAX_SET.find_syntax_by_token(language).ok_or_else(|| {
        docmux_core::ConvertError::Unsupported(format!("unknown language: {language}"))
    })?;

    let theme = THEME_SET
        .themes
        .get(theme)
        .ok_or_else(|| docmux_core::ConvertError::Unsupported(format!("unknown theme: {theme}")))?;

    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut result: Vec<Vec<HighlightToken>> = Vec::new();

    for line in syntect::util::LinesWithEndings::from(code) {
        let ranges: Vec<(syntect::highlighting::Style, &str)> = highlighter
            .highlight_line(line, &SYNTAX_SET)
            .map_err(|e| docmux_core::ConvertError::Other(e.to_string()))?;

        let tokens = ranges
            .into_iter()
            .map(|(style, text)| HighlightToken {
                text: text.to_owned(),
                style: convert_style(style),
            })
            .collect();

        result.push(tokens);
    }

    Ok(result)
}

/// Return the names of all available syntax languages.
pub fn available_languages() -> Vec<String> {
    SYNTAX_SET
        .syntaxes()
        .iter()
        .map(|s| s.name.clone())
        .collect()
}

/// Return the names of all available colour themes.
pub fn available_themes() -> Vec<String> {
    THEME_SET.themes.keys().cloned().collect()
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn convert_style(style: syntect::highlighting::Style) -> TokenStyle {
    let fg = style.foreground;
    TokenStyle {
        foreground: Color {
            r: fg.r,
            g: fg.g,
            b: fg.b,
            a: fg.a,
        },
        bold: style.font_style.contains(FontStyle::BOLD),
        italic: style.font_style.contains(FontStyle::ITALIC),
        underline: style.font_style.contains(FontStyle::UNDERLINE),
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_rust_code() {
        let code = "fn main() {\n    println!(\"hello\");\n}\n";
        let tokens = highlight(code, "rs", "base16-ocean.dark").unwrap();

        // Should have 3 lines of code
        assert_eq!(tokens.len(), 3);
        // Each line should have at least one token
        for line in &tokens {
            assert!(!line.is_empty());
        }
        // Verify the text round-trips: concatenating all token texts
        // should reproduce the original code.
        let reconstructed: String = tokens
            .iter()
            .flat_map(|line| line.iter())
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(reconstructed, code);
    }

    #[test]
    fn highlight_python_code() {
        let code = "def greet(name):\n    print(f\"Hello {name}\")\n";
        let tokens = highlight(code, "py", "base16-ocean.dark").unwrap();

        assert_eq!(tokens.len(), 2);
        let reconstructed: String = tokens
            .iter()
            .flat_map(|line| line.iter())
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(reconstructed, code);
    }

    #[test]
    fn highlight_unknown_language_returns_error() {
        let result = highlight("some code", "not-a-real-language-xyz", "base16-ocean.dark");
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unknown language"),
            "expected 'unknown language' in error, got: {msg}"
        );
    }

    #[test]
    fn highlight_unknown_theme_returns_error() {
        let result = highlight("fn main() {}", "rs", "not-a-real-theme-xyz");
        assert!(result.is_err());
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("unknown theme"),
            "expected 'unknown theme' in error, got: {msg}"
        );
    }

    #[test]
    fn available_languages_is_nonempty() {
        let langs = available_languages();
        assert!(!langs.is_empty());
        // Rust should be in the list
        assert!(
            langs.iter().any(|l| l == "Rust"),
            "expected Rust in languages: {langs:?}"
        );
    }

    #[test]
    fn available_themes_is_nonempty() {
        let themes = available_themes();
        assert!(!themes.is_empty());
        // base16-ocean.dark is a built-in theme
        assert!(
            themes.iter().any(|t| t == "base16-ocean.dark"),
            "expected base16-ocean.dark in themes: {themes:?}"
        );
    }

    #[test]
    fn token_style_fields_populated() {
        let code = "fn main() {}\n";
        let tokens = highlight(code, "rs", "base16-ocean.dark").unwrap();
        // At least one token should exist
        let first = &tokens[0][0];
        // The foreground colour alpha should be 255 (fully opaque) for
        // most theme colours.
        assert_eq!(first.style.foreground.a, 255);
    }

    #[test]
    fn highlight_empty_code() {
        let tokens = highlight("", "rs", "base16-ocean.dark").unwrap();
        // Empty input should yield zero lines
        assert!(tokens.is_empty());
    }
}
