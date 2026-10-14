//! User macro expansion.
//!
//! Real papers define their own notation in the preamble (`\newcommand{\bz}{\mathbf{z}}`,
//! `\DeclareMathOperator*{\argmax}{arg\,max}`) and use it everywhere, including
//! inside math. Without expansion those commands reach the writers as unknown
//! names. This module runs on the source text, before tokenizing:
//!
//! 1. [`collect`] reads definitions from every source file and removes them;
//! 2. [`expand`] substitutes uses, with arguments, recursively.
//!
//! It is deliberately not TeX: definitions whose body relies on TeX internals
//! (`@` names, `\csname`, `\if…`) are dropped without being expanded, and
//! structural commands (`\section`, `\cite`, …) are never redefined, so a
//! class-file style redefinition cannot destroy the document structure.
//! `\iftoggle` (etoolbox) is evaluated, since papers use it to pick content.

use std::collections::{HashMap, HashSet};

/// A user-defined macro.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Macro {
    /// Number of parameters (`#1` … `#9`).
    params: usize,
    /// Default for the first parameter, which is then optional (`[…]`).
    default: Option<String>,
    body: String,
}

/// Macro definitions and toggle states gathered from the sources.
#[derive(Debug, Default)]
pub(crate) struct Macros {
    defs: HashMap<String, Macro>,
    toggles: HashMap<String, bool>,
}

/// Never redefined: the parser gives these document structure.
const PROTECTED: &[&str] = &[
    "section",
    "subsection",
    "subsubsection",
    "paragraph",
    "subparagraph",
    "chapter",
    "part",
    "caption",
    "label",
    "ref",
    "eqref",
    "autoref",
    "cref",
    "Cref",
    "cite",
    "citep",
    "citet",
    "item",
    "begin",
    "end",
    "input",
    "include",
    "footnote",
    "emph",
    "textbf",
    "textit",
    "texttt",
    "url",
    "href",
    "includegraphics",
    "title",
    "author",
    "maketitle",
    "date",
    "frac",
    "mathbf",
    "mathrm",
    "text",
    "left",
    "right",
];

/// Bodies that need a real TeX engine.
const UNEXPANDABLE: &[&str] = &[
    "@",
    "\\csname",
    "\\expandafter",
    "\\if",
    "\\else",
    "\\fi",
    "\\def",
    "\\newcommand",
    "\\let",
    "\\futurelet",
    "\\afterassignment",
    "\\global",
];

const DEFINERS: &[&str] = &[
    "newcommand",
    "renewcommand",
    "providecommand",
    "DeclareRobustCommand",
    "def",
    "DeclareMathOperator",
    "newtoggle",
    "toggletrue",
    "togglefalse",
];

/// Read every definition in `source` into `macros` and return the source
/// with the definitions removed.
pub(crate) fn collect(source: &str, macros: &mut Macros) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        if let Some(skip) = skip_literal(&chars, i, &mut out) {
            i = skip;
            continue;
        }
        if chars[i] == '\\' {
            let (name, after) = control_word(&chars, i + 1);
            if DEFINERS.contains(&name.as_str()) {
                if let Some(end) = definition(&name, &chars, after, macros) {
                    i = end;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Parse one definition starting after the definer's name. Returns the index
/// after the definition, or `None` if it is not one we understand (the text is
/// then left in place).
fn definition(definer: &str, chars: &[char], start: usize, macros: &mut Macros) -> Option<usize> {
    let mut i = start;
    if chars.get(i) == Some(&'*') {
        i += 1;
    }
    match definer {
        "newtoggle" | "toggletrue" | "togglefalse" => {
            let (name, end) = group(chars, skip_spaces(chars, i))?;
            let state = definer == "toggletrue";
            macros.toggles.insert(name.trim().to_string(), state);
            Some(end)
        }
        "def" => def_definition(chars, i, macros),
        "DeclareMathOperator" => {
            let starred = chars.get(start) == Some(&'*');
            let (name, i) = macro_name(chars, skip_spaces(chars, i))?;
            let (text, end) = group(chars, skip_spaces(chars, i))?;
            let op = if starred {
                "operatorname*"
            } else {
                "operatorname"
            };
            let body = format!("\\{op}{{{text}}}");
            insert(
                macros,
                definer,
                name,
                Macro {
                    params: 0,
                    default: None,
                    body,
                },
            );
            Some(end)
        }
        _ => command_definition(definer, chars, i, macros),
    }
}

/// `\newcommand{\name}[n][default]{body}` and its variants.
fn command_definition(
    definer: &str,
    chars: &[char],
    start: usize,
    macros: &mut Macros,
) -> Option<usize> {
    let (name, mut i) = macro_name(chars, skip_spaces(chars, start))?;
    let mut params = 0;
    let mut default = None;
    if let Some((n, end)) = bracket(chars, skip_spaces(chars, i)) {
        params = n.trim().parse().ok()?;
        i = end;
        if let Some((d, end)) = bracket(chars, skip_spaces(chars, i)) {
            default = Some(d);
            i = end;
        }
    }
    let (body, end) = group(chars, skip_spaces(chars, i))?;
    insert(
        macros,
        definer,
        name,
        Macro {
            params,
            default,
            body,
        },
    );
    Some(end)
}

/// `\def\name#1#2{body}`
fn def_definition(chars: &[char], start: usize, macros: &mut Macros) -> Option<usize> {
    let (name, mut i) = macro_name(chars, start)?;
    let mut params = 0;
    while chars.get(i) == Some(&'#') && chars.get(i + 1).is_some_and(char::is_ascii_digit) {
        params += 1;
        i += 2;
    }
    if chars.get(i) != Some(&'{') {
        return None;
    }
    let (body, end) = group(chars, i)?;
    insert(
        macros,
        "def",
        name,
        Macro {
            params,
            default: None,
            body,
        },
    );
    Some(end)
}

fn insert(macros: &mut Macros, definer: &str, name: String, mac: Macro) {
    let expandable = !UNEXPANDABLE.iter().any(|bad| mac.body.contains(bad));
    if !expandable || PROTECTED.contains(&name.as_str()) || name.starts_with("the") {
        return;
    }
    if definer == "providecommand" && macros.defs.contains_key(&name) {
        return;
    }
    macros.defs.insert(name, mac);
}

/// Expand every macro use in `source`.
pub(crate) fn expand(source: &str, macros: &Macros) -> String {
    let mut seen = HashSet::new();
    expand_depth(source, macros, 0, &mut seen)
}

const MAX_DEPTH: usize = 16;

fn expand_depth(source: &str, macros: &Macros, depth: usize, seen: &mut HashSet<String>) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    while i < chars.len() {
        if let Some(skip) = skip_literal(&chars, i, &mut out) {
            i = skip;
            continue;
        }
        if chars[i] != '\\' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let (name, after) = control_word(&chars, i + 1);
        match use_site(&name, &chars, after, macros) {
            Some((replacement, end)) if depth < MAX_DEPTH && !seen.contains(&name) => {
                seen.insert(name.clone());
                out.push_str(&expand_depth(&replacement, macros, depth + 1, seen));
                seen.remove(&name);
                i = end;
            }
            _ => {
                out.push('\\');
                out.push_str(&name);
                i = after.max(i + 1);
            }
        }
    }
    out
}

/// If `\name` at `start` is a macro use (or `\iftoggle`), return its
/// replacement text and the index after its arguments.
fn use_site(name: &str, chars: &[char], start: usize, macros: &Macros) -> Option<(String, usize)> {
    if name == "iftoggle" {
        let (toggle, i) = group(chars, skip_spaces(chars, start))?;
        let (yes, i) = group(chars, skip_spaces(chars, i))?;
        let (no, end) = group(chars, skip_spaces(chars, i))?;
        let on = macros.toggles.get(toggle.trim()).copied().unwrap_or(false);
        return Some((if on { yes } else { no }, end));
    }
    let mac = macros.defs.get(name)?;
    let mut i = start;
    let mut args = Vec::with_capacity(mac.params);
    if let Some(default) = &mac.default {
        match bracket(chars, skip_spaces(chars, i)) {
            Some((arg, end)) => {
                args.push(arg);
                i = end;
            }
            None => args.push(default.clone()),
        }
    }
    while args.len() < mac.params {
        let (arg, end) = argument(chars, skip_spaces(chars, i))?;
        args.push(arg);
        i = end;
    }
    Some(substitute(mac, &args, chars, i))
}

/// Fill `#1…#9` and apply TeX's spacing rule: a control word without
/// arguments swallows the spaces after it, unless its body ends in
/// `\xspace`, which keeps one before a letter.
fn substitute(mac: &Macro, args: &[String], chars: &[char], end: usize) -> (String, usize) {
    let mut body = mac.body.clone();
    for (k, arg) in args.iter().enumerate() {
        body = body.replace(&format!("#{}", k + 1), arg);
    }
    if let Some(stripped) = body.strip_suffix("\\xspace") {
        let next = chars.get(end).copied();
        let needs_space = next.is_some_and(|c| c.is_alphanumeric());
        return (
            format!("{stripped}{}", if needs_space { " " } else { "" }),
            end,
        );
    }
    if mac.params == 0 && mac.default.is_none() {
        return (body, skip_spaces(chars, end));
    }
    (body, end)
}

// ─── Scanning helpers ───────────────────────────────────────────────────────

/// Copy comments, `\verb|…|` and verbatim environments untouched. Returns
/// the index after the copied text, or `None` if `i` starts none of them.
fn skip_literal(chars: &[char], i: usize, out: &mut String) -> Option<usize> {
    let end = match chars[i] {
        '%' => chars[i..]
            .iter()
            .position(|&c| c == '\n')
            .map_or(chars.len(), |p| i + p),
        '\\' if chars.get(i + 1) == Some(&'\\') || chars.get(i + 1) == Some(&'%') => i + 2,
        '\\' => {
            let (name, after) = control_word(chars, i + 1);
            match name.as_str() {
                "verb" => verb_end(chars, after)?,
                "begin" => verbatim_env_end(chars, after)?,
                _ => return None,
            }
        }
        _ => return None,
    };
    out.extend(&chars[i..end]);
    Some(end)
}

fn verb_end(chars: &[char], start: usize) -> Option<usize> {
    let i = if chars.get(start) == Some(&'*') {
        start + 1
    } else {
        start
    };
    let delim = *chars.get(i)?;
    let close = chars[i + 1..].iter().position(|&c| c == delim)?;
    Some(i + 1 + close + 1)
}

fn verbatim_env_end(chars: &[char], start: usize) -> Option<usize> {
    let (env, after) = group(chars, start)?;
    let literal = matches!(
        env.as_str(),
        "verbatim" | "verbatim*" | "Verbatim" | "lstlisting" | "minted" | "comment"
    );
    if !literal {
        return None;
    }
    let marker: Vec<char> = format!("\\end{{{env}}}").chars().collect();
    let found = chars[after..]
        .windows(marker.len())
        .position(|w| w == marker.as_slice())?;
    Some(after + found + marker.len())
}

/// Read a control word (letters) or control symbol (one other character)
/// starting at `start`. Returns the name and the index after it.
fn control_word(chars: &[char], start: usize) -> (String, usize) {
    let mut i = start;
    while chars.get(i).is_some_and(|c| c.is_ascii_alphabetic()) {
        i += 1;
    }
    if i == start {
        return match chars.get(start) {
            Some(c) => (c.to_string(), start + 1),
            None => (String::new(), start),
        };
    }
    (chars[start..i].iter().collect(), i)
}

/// `\name` or `{\name}` as the target of a definition.
fn macro_name(chars: &[char], start: usize) -> Option<(String, usize)> {
    match chars.get(start)? {
        '\\' => {
            let (name, end) = control_word(chars, start + 1);
            (!name.is_empty()).then_some((name, end))
        }
        '{' => {
            let (inner, end) = group(chars, start)?;
            let inner = inner.trim();
            let name = inner.strip_prefix('\\')?;
            Some((name.to_string(), end))
        }
        _ => None,
    }
}

/// A macro argument: a brace group's content, or a single token.
fn argument(chars: &[char], start: usize) -> Option<(String, usize)> {
    match chars.get(start)? {
        '{' => group(chars, start),
        '\\' => {
            let (name, end) = control_word(chars, start + 1);
            Some((format!("\\{name}"), end))
        }
        c => Some((c.to_string(), start + 1)),
    }
}

/// The content of a balanced `{…}` group at `start`, and the index after it.
fn group(chars: &[char], start: usize) -> Option<(String, usize)> {
    delimited(chars, start, '{', '}')
}

/// The content of a `[…]` group at `start`, and the index after it.
fn bracket(chars: &[char], start: usize) -> Option<(String, usize)> {
    delimited(chars, start, '[', ']')
}

fn delimited(chars: &[char], start: usize, open: char, close: char) -> Option<(String, usize)> {
    if chars.get(start) != Some(&open) {
        return None;
    }
    let mut depth = 0usize;
    let mut i = start;
    while i < chars.len() {
        match chars[i] {
            '\\' => i += 1,
            c if c == open => depth += 1,
            c if c == close => {
                depth -= 1;
                if depth == 0 {
                    return Some((chars[start + 1..i].iter().collect(), i + 1));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

fn skip_spaces(chars: &[char], start: usize) -> usize {
    let mut i = start;
    while chars
        .get(i)
        .is_some_and(|c| *c == ' ' || *c == '\t' || *c == '\n')
    {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(source: &str) -> String {
        let mut macros = Macros::default();
        let body = collect(source, &mut macros);
        expand(&body, &macros)
    }

    #[test]
    fn zero_argument_macro_in_math() {
        let out = run(r"\newcommand{\bz}{\mathbf{z}}$p(\bz)$");
        assert_eq!(out, r"$p(\mathbf{z})$");
    }

    #[test]
    fn definition_forms() {
        let out = run(
            r"\newcommand\R{\mathbb{R}}\renewcommand*{\N}{\mathbb{N}}\def\bx{\mathbf{x}}\providecommand{\C}{\mathbb{C}}$\R\N\bx\C$",
        );
        assert_eq!(out, r"$\mathbb{R}\mathbb{N}\mathbf{x}\mathbb{C}$");
    }

    #[test]
    fn arguments_and_optional_default() {
        let src = r"\newcommand{\kl}[2]{D(#1\|#2)}\newcommand{\E}[1][q]{\mathbb{E}_{#1}}$\kl{q}{p} \E x \E[p] y$";
        assert_eq!(run(src), r"$D(q\|p) \mathbb{E}_{q} x \mathbb{E}_{p} y$");
    }

    #[test]
    fn single_token_arguments() {
        assert_eq!(
            run(r"\newcommand{\vv}[1]{\mathbf{#1}}$\vv x + \vv\alpha$"),
            r"$\mathbf{x} + \mathbf{\alpha}$"
        );
    }

    #[test]
    fn nested_macros_expand() {
        assert_eq!(
            run(r"\newcommand{\bz}{\mathbf{z}}\newcommand{\q}{q(\bz)}$\q$"),
            r"$q(\mathbf{z})$"
        );
    }

    #[test]
    fn declare_math_operator() {
        assert_eq!(
            run(
                r"\DeclareMathOperator*{\argmax}{arg\,max}\DeclareMathOperator{\tr}{tr}$\argmax_x \tr A$"
            ),
            r"$\operatorname*{arg\,max}_x \operatorname{tr}A$"
        );
    }

    #[test]
    fn text_macro_with_xspace() {
        let src = r"\newcommand{\lora}{LoRA\xspace}We use \lora with \lora.";
        assert_eq!(run(src), "We use LoRA with LoRA.");
    }

    #[test]
    fn recursive_definition_does_not_loop() {
        let out = run(r"\newcommand{\a}{x\a}$\a$");
        assert!(out.starts_with("$x"));
    }

    #[test]
    fn structural_and_internal_definitions_are_ignored() {
        let src = r"\renewcommand\section{\@startsection{section}{1}{0pt}}\renewcommand{\thefootnote}{\alph{footnote}}\section{A}";
        assert_eq!(run(src), r"\section{A}");
    }

    #[test]
    fn comments_and_verbatim_are_untouched() {
        let src =
            "\\newcommand{\\x}{X}% \\x here\n\\begin{verbatim}\\x\\end{verbatim} \\verb|\\x| \\x";
        assert_eq!(
            run(src),
            "% \\x here\n\\begin{verbatim}\\x\\end{verbatim} \\verb|\\x| X"
        );
    }

    #[test]
    fn iftoggle_picks_the_branch() {
        let src =
            r"\newtoggle{hq}\toggletrue{hq}\iftoggle{hq}{high}{low} \iftoggle{other}{yes}{no}";
        assert_eq!(run(src), "high no");
    }

    #[test]
    fn escaped_percent_is_not_a_comment() {
        assert_eq!(run(r"\newcommand{\x}{X}50\% \x"), r"50\% X");
    }
}
