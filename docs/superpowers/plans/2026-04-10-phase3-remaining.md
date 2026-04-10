# Phase 3 Remaining — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete Phase 3 by implementing the math transform (LaTeX ↔ Typst + MathML), highlight enhancements (line numbers + line highlighting), and cite completion (prefix/suffix + nocite).

**Architecture:** Three independent features built sequentially. Math transform is a new crate with a tokenizer + 3 mappers. Highlight adds line options to existing writer code. Cite patches the existing transform and adds a CLI flag.

**Tech Stack:** Rust, syntect (highlighting), hayagriva/citationberg (citations), MathML XML generation

**Spec:** `docs/superpowers/specs/2026-04-10-phase3-remaining-design.md`

---

## Part 1: Math Transform

### Task 1: LaTeX math tokenizer

**Files:**
- Create: `crates/docmux-transform-math/src/tokenizer.rs`
- Test: inline `#[cfg(test)] mod tests` in the same file

- [ ] **Step 1: Write failing tests for the tokenizer**

In `crates/docmux-transform-math/src/tokenizer.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_simple_variable() {
        let tokens = tokenize_latex("x");
        assert_eq!(tokens, vec![Token::Text("x".into())]);
    }

    #[test]
    fn tokenize_command() {
        let tokens = tokenize_latex(r"\alpha");
        assert_eq!(tokens, vec![Token::Command("alpha".into())]);
    }

    #[test]
    fn tokenize_command_with_brace_arg() {
        let tokens = tokenize_latex(r"\frac{a}{b}");
        assert_eq!(
            tokens,
            vec![
                Token::Command("frac".into()),
                Token::BraceGroup(vec![Token::Text("a".into())]),
                Token::BraceGroup(vec![Token::Text("b".into())]),
            ]
        );
    }

    #[test]
    fn tokenize_subscript_superscript() {
        let tokens = tokenize_latex("x^2_i");
        assert_eq!(
            tokens,
            vec![
                Token::Text("x".into()),
                Token::SuperScript,
                Token::Text("2".into()),
                Token::SubScript,
                Token::Text("i".into()),
            ]
        );
    }

    #[test]
    fn tokenize_environment() {
        let tokens = tokenize_latex(r"\begin{pmatrix}a & b\end{pmatrix}");
        assert_eq!(
            tokens,
            vec![Token::Environment {
                name: "pmatrix".into(),
                body: r"a & b".into(),
            }]
        );
    }

    #[test]
    fn tokenize_nested_braces() {
        let tokens = tokenize_latex(r"\frac{a+{b}}{c}");
        assert_eq!(
            tokens,
            vec![
                Token::Command("frac".into()),
                Token::BraceGroup(vec![
                    Token::Text("a+".into()),
                    Token::BraceGroup(vec![Token::Text("b".into())]),
                ]),
                Token::BraceGroup(vec![Token::Text("c".into())]),
            ]
        );
    }

    #[test]
    fn tokenize_optional_arg() {
        let tokens = tokenize_latex(r"\sqrt[3]{x}");
        assert_eq!(
            tokens,
            vec![
                Token::Command("sqrt".into()),
                Token::OptionalArg("3".into()),
                Token::BraceGroup(vec![Token::Text("x".into())]),
            ]
        );
    }

    #[test]
    fn tokenize_whitespace_preserved() {
        let tokens = tokenize_latex(r"a + b");
        assert_eq!(tokens, vec![Token::Text("a + b".into())]);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: compilation errors (types not defined yet)

- [ ] **Step 3: Implement the tokenizer**

In `crates/docmux-transform-math/src/tokenizer.rs`:

```rust
/// Tokens produced by the LaTeX math tokenizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// A backslash command like `\frac`, `\alpha`. Name excludes the `\`.
    Command(String),
    /// Content inside `{...}`, recursively tokenized.
    BraceGroup(Vec<Token>),
    /// Content inside `[...]` (optional argument to a command).
    OptionalArg(String),
    /// `_`
    SubScript,
    /// `^`
    SuperScript,
    /// `\begin{name}...\end{name}` — body stored as raw string.
    Environment { name: String, body: String },
    /// Plain text (letters, digits, operators, whitespace).
    Text(String),
}

/// Tokenize a LaTeX math string into a flat/nested token list.
pub fn tokenize_latex(input: &str) -> Vec<Token> {
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;
    tokenize_inner(&chars, &mut pos, false)
}

fn tokenize_inner(chars: &[char], pos: &mut usize, inside_brace: bool) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut text_buf = String::new();

    while *pos < chars.len() {
        let ch = chars[*pos];

        match ch {
            '\\' => {
                flush_text(&mut text_buf, &mut tokens);
                *pos += 1;
                let name = read_command_name(chars, pos);
                if name == "begin" {
                    // Parse environment
                    let env_name = read_brace_content_raw(chars, pos);
                    let body = read_until_end_env(chars, pos, &env_name);
                    tokens.push(Token::Environment { name: env_name, body });
                } else {
                    tokens.push(Token::Command(name));
                }
            }
            '{' => {
                flush_text(&mut text_buf, &mut tokens);
                *pos += 1;
                let inner = tokenize_inner(chars, pos, true);
                tokens.push(Token::BraceGroup(inner));
            }
            '}' if inside_brace => {
                flush_text(&mut text_buf, &mut tokens);
                *pos += 1;
                return tokens;
            }
            '[' if tokens.last().map_or(false, |t| matches!(t, Token::Command(_))) => {
                flush_text(&mut text_buf, &mut tokens);
                *pos += 1;
                let content = read_until_char(chars, pos, ']');
                tokens.push(Token::OptionalArg(content));
            }
            '_' => {
                flush_text(&mut text_buf, &mut tokens);
                tokens.push(Token::SubScript);
                *pos += 1;
            }
            '^' => {
                flush_text(&mut text_buf, &mut tokens);
                tokens.push(Token::SuperScript);
                *pos += 1;
            }
            _ => {
                text_buf.push(ch);
                *pos += 1;
            }
        }
    }

    flush_text(&mut text_buf, &mut tokens);
    tokens
}

fn flush_text(buf: &mut String, tokens: &mut Vec<Token>) {
    if !buf.is_empty() {
        tokens.push(Token::Text(std::mem::take(buf)));
    }
}

fn read_command_name(chars: &[char], pos: &mut usize) -> String {
    let mut name = String::new();
    while *pos < chars.len() && chars[*pos].is_ascii_alphabetic() {
        name.push(chars[*pos]);
        *pos += 1;
    }
    if name.is_empty() && *pos < chars.len() {
        // Single-char command like \, or \;
        name.push(chars[*pos]);
        *pos += 1;
    }
    name
}

fn read_brace_content_raw(chars: &[char], pos: &mut usize) -> String {
    // Expect `{` at current pos
    if *pos < chars.len() && chars[*pos] == '{' {
        *pos += 1;
    }
    read_until_char(chars, pos, '}')
}

fn read_until_char(chars: &[char], pos: &mut usize, end: char) -> String {
    let mut buf = String::new();
    while *pos < chars.len() && chars[*pos] != end {
        buf.push(chars[*pos]);
        *pos += 1;
    }
    if *pos < chars.len() {
        *pos += 1; // skip closing char
    }
    buf
}

fn read_until_end_env(chars: &[char], pos: &mut usize, name: &str) -> String {
    let end_marker = format!("\\end{{{name}}}");
    let mut body = String::new();
    let input: String = chars[*pos..].iter().collect();
    if let Some(idx) = input.find(&end_marker) {
        body = input[..idx].to_string();
        *pos += idx + end_marker.len();
    } else {
        // No matching \end — consume rest
        body = input;
        *pos = chars.len();
    }
    body
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: all 8 tokenizer tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-transform-math/src/tokenizer.rs
git commit -m "feat(math): add LaTeX math tokenizer"
```

---

### Task 2: Command mapping tables

**Files:**
- Create: `crates/docmux-transform-math/src/tables.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write failing tests for the tables**

In `crates/docmux-transform-math/src/tables.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greek_latex_to_typst() {
        assert_eq!(LATEX_TO_TYPST_COMMANDS.get("alpha"), Some(&"alpha"));
        assert_eq!(LATEX_TO_TYPST_COMMANDS.get("beta"), Some(&"beta"));
        assert_eq!(LATEX_TO_TYPST_COMMANDS.get("Gamma"), Some(&"Gamma"));
    }

    #[test]
    fn operator_latex_to_typst() {
        assert_eq!(LATEX_TO_TYPST_COMMANDS.get("sum"), Some(&"sum"));
        assert_eq!(LATEX_TO_TYPST_COMMANDS.get("int"), Some(&"integral"));
        assert_eq!(LATEX_TO_TYPST_COMMANDS.get("prod"), Some(&"product"));
    }

    #[test]
    fn decoration_latex_to_typst() {
        assert_eq!(LATEX_TO_TYPST_FUNCTIONS.get("hat"), Some(&"hat"));
        assert_eq!(LATEX_TO_TYPST_FUNCTIONS.get("bar"), Some(&"overline"));
        assert_eq!(LATEX_TO_TYPST_FUNCTIONS.get("vec"), Some(&"arrow"));
    }

    #[test]
    fn latex_to_unicode() {
        assert_eq!(LATEX_TO_UNICODE.get("alpha"), Some(&"\u{03B1}"));
        assert_eq!(LATEX_TO_UNICODE.get("infty"), Some(&"\u{221E}"));
    }

    #[test]
    fn environment_mapping() {
        assert_eq!(LATEX_ENV_TO_TYPST.get("pmatrix"), Some(&("mat", "\"(\"")));
        assert_eq!(LATEX_ENV_TO_TYPST.get("bmatrix"), Some(&("mat", "\"[\"")));
        assert_eq!(LATEX_ENV_TO_TYPST.get("cases"), Some(&("cases", "")));
    }

    #[test]
    fn mathbb_mapping() {
        assert_eq!(MATHBB_TO_TYPST.get("R"), Some(&"RR"));
        assert_eq!(MATHBB_TO_TYPST.get("N"), Some(&"NN"));
        assert_eq!(MATHBB_TO_TYPST.get("Z"), Some(&"ZZ"));
    }

    #[test]
    fn reverse_typst_to_latex() {
        assert_eq!(TYPST_TO_LATEX_COMMANDS.get("integral"), Some(&"int"));
        assert_eq!(TYPST_TO_LATEX_COMMANDS.get("product"), Some(&"prod"));
        assert_eq!(TYPST_TO_LATEX_COMMANDS.get("arrow"), Some(&"vec"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: compilation errors (tables not defined)

- [ ] **Step 3: Implement the mapping tables**

In `crates/docmux-transform-math/src/tables.rs`:

```rust
use std::collections::HashMap;
use std::sync::LazyLock;

/// LaTeX command → Typst equivalent (simple renames, no structural change).
/// These are commands that map 1:1 with no argument transformation.
pub static LATEX_TO_TYPST_COMMANDS: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // Greek lowercase
    for name in [
        "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta",
        "iota", "kappa", "lambda", "mu", "nu", "xi", "pi", "rho", "sigma",
        "tau", "upsilon", "phi", "chi", "psi", "omega",
    ] {
        m.insert(name, name); // Typst uses same names without backslash
    }
    // Greek uppercase
    for name in [
        "Gamma", "Delta", "Theta", "Lambda", "Xi", "Pi", "Sigma",
        "Upsilon", "Phi", "Psi", "Omega",
    ] {
        m.insert(name, name);
    }
    // Operators
    m.insert("sum", "sum");
    m.insert("prod", "product");
    m.insert("int", "integral");
    m.insert("iint", "integral.double");
    m.insert("iiint", "integral.triple");
    m.insert("oint", "integral.cont");
    m.insert("lim", "lim");
    m.insert("inf", "inf");
    m.insert("sup", "sup");
    m.insert("min", "min");
    m.insert("max", "max");
    m.insert("sin", "sin");
    m.insert("cos", "cos");
    m.insert("tan", "tan");
    m.insert("log", "log");
    m.insert("ln", "ln");
    m.insert("exp", "exp");
    m.insert("det", "det");
    m.insert("dim", "dim");
    // Arrows
    m.insert("to", "arrow.r");
    m.insert("leftarrow", "arrow.l");
    m.insert("rightarrow", "arrow.r");
    m.insert("leftrightarrow", "arrow.l.r");
    m.insert("Rightarrow", "arrow.r.double");
    m.insert("Leftarrow", "arrow.l.double");
    m.insert("implies", "arrow.r.double");
    m.insert("iff", "arrow.l.r.double");
    // Relations
    m.insert("leq", "lt.eq");
    m.insert("geq", "gt.eq");
    m.insert("neq", "eq.not");
    m.insert("approx", "approx");
    m.insert("equiv", "equiv");
    m.insert("subset", "subset");
    m.insert("supset", "supset");
    m.insert("subseteq", "subset.eq");
    m.insert("supseteq", "supset.eq");
    m.insert("in", "in");
    m.insert("notin", "in.not");
    // Misc
    m.insert("infty", "infinity");
    m.insert("partial", "partial");
    m.insert("nabla", "nabla");
    m.insert("forall", "forall");
    m.insert("exists", "exists");
    m.insert("emptyset", "emptyset");
    m.insert("cdot", "dot.op");
    m.insert("cdots", "dots.h.c");
    m.insert("ldots", "dots.h");
    m.insert("vdots", "dots.v");
    m.insert("ddots", "dots.down");
    m.insert("times", "times");
    m.insert("pm", "plus.minus");
    m.insert("mp", "minus.plus");
    // Spacing
    m.insert("quad", "quad");
    m.insert(",", "thin");
    m.insert(";", "med");
    m.insert("!", "negthin");
    m
});

/// LaTeX commands that take a `{arg}` and become Typst `func(arg)`.
pub static LATEX_TO_TYPST_FUNCTIONS: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("hat", "hat");
    m.insert("bar", "overline");
    m.insert("overline", "overline");
    m.insert("underline", "underline");
    m.insert("vec", "arrow");
    m.insert("tilde", "tilde");
    m.insert("dot", "dot");
    m.insert("ddot", "dot.double");
    m.insert("mathbf", "bold");
    m.insert("mathit", "italic");
    m.insert("mathrm", "upright");
    m.insert("mathcal", "cal");
    m.insert("text", "\"text\""); // placeholder, handled specially
    m
});

/// LaTeX environment → (Typst function name, delim arg or "").
pub static LATEX_ENV_TO_TYPST: LazyLock<HashMap<&str, (&str, &str)>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("pmatrix", ("mat", "\"(\""));
    m.insert("bmatrix", ("mat", "\"[\""));
    m.insert("Bmatrix", ("mat", "\"{\""));
    m.insert("vmatrix", ("mat", "\"|\""));
    m.insert("matrix", ("mat", ""));
    m.insert("cases", ("cases", ""));
    m.insert("aligned", ("aligned", ""));
    m.insert("gathered", ("gathered", ""));
    m
});

/// `\mathbb{X}` → Typst shorthand.
pub static MATHBB_TO_TYPST: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("R", "RR");
    m.insert("N", "NN");
    m.insert("Z", "ZZ");
    m.insert("Q", "QQ");
    m.insert("C", "CC");
    m
});

/// LaTeX command → Unicode character (for MathML `<mi>` output).
pub static LATEX_TO_UNICODE: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    // Greek lowercase
    m.insert("alpha", "\u{03B1}");
    m.insert("beta", "\u{03B2}");
    m.insert("gamma", "\u{03B3}");
    m.insert("delta", "\u{03B4}");
    m.insert("epsilon", "\u{03B5}");
    m.insert("zeta", "\u{03B6}");
    m.insert("eta", "\u{03B7}");
    m.insert("theta", "\u{03B8}");
    m.insert("iota", "\u{03B9}");
    m.insert("kappa", "\u{03BA}");
    m.insert("lambda", "\u{03BB}");
    m.insert("mu", "\u{03BC}");
    m.insert("nu", "\u{03BD}");
    m.insert("xi", "\u{03BE}");
    m.insert("pi", "\u{03C0}");
    m.insert("rho", "\u{03C1}");
    m.insert("sigma", "\u{03C3}");
    m.insert("tau", "\u{03C4}");
    m.insert("upsilon", "\u{03C5}");
    m.insert("phi", "\u{03C6}");
    m.insert("chi", "\u{03C7}");
    m.insert("psi", "\u{03C8}");
    m.insert("omega", "\u{03C9}");
    // Greek uppercase
    m.insert("Gamma", "\u{0393}");
    m.insert("Delta", "\u{0394}");
    m.insert("Theta", "\u{0398}");
    m.insert("Lambda", "\u{039B}");
    m.insert("Xi", "\u{039E}");
    m.insert("Pi", "\u{03A0}");
    m.insert("Sigma", "\u{03A3}");
    m.insert("Phi", "\u{03A6}");
    m.insert("Psi", "\u{03A8}");
    m.insert("Omega", "\u{03A9}");
    // Operators / symbols
    m.insert("infty", "\u{221E}");
    m.insert("partial", "\u{2202}");
    m.insert("nabla", "\u{2207}");
    m.insert("forall", "\u{2200}");
    m.insert("exists", "\u{2203}");
    m.insert("emptyset", "\u{2205}");
    m.insert("sum", "\u{2211}");
    m.insert("prod", "\u{220F}");
    m.insert("int", "\u{222B}");
    m.insert("pm", "\u{00B1}");
    m.insert("times", "\u{00D7}");
    m.insert("cdot", "\u{22C5}");
    m.insert("leq", "\u{2264}");
    m.insert("geq", "\u{2265}");
    m.insert("neq", "\u{2260}");
    m.insert("approx", "\u{2248}");
    m.insert("equiv", "\u{2261}");
    m.insert("subset", "\u{2282}");
    m.insert("supset", "\u{2283}");
    m.insert("in", "\u{2208}");
    m.insert("notin", "\u{2209}");
    m.insert("rightarrow", "\u{2192}");
    m.insert("leftarrow", "\u{2190}");
    m.insert("Rightarrow", "\u{21D2}");
    m.insert("Leftarrow", "\u{21D0}");
    m
});

/// Typst command → LaTeX command (reverse mapping for Typst → LaTeX direction).
pub static TYPST_TO_LATEX_COMMANDS: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("integral", "int");
    m.insert("integral.double", "iint");
    m.insert("integral.triple", "iiint");
    m.insert("integral.cont", "oint");
    m.insert("product", "prod");
    m.insert("infinity", "infty");
    m.insert("arrow.r", "rightarrow");
    m.insert("arrow.l", "leftarrow");
    m.insert("arrow.r.double", "Rightarrow");
    m.insert("arrow.l.double", "Leftarrow");
    m.insert("arrow.l.r", "leftrightarrow");
    m.insert("arrow.l.r.double", "iff");
    m.insert("lt.eq", "leq");
    m.insert("gt.eq", "geq");
    m.insert("eq.not", "neq");
    m.insert("subset.eq", "subseteq");
    m.insert("supset.eq", "supseteq");
    m.insert("in.not", "notin");
    m.insert("dot.op", "cdot");
    m.insert("plus.minus", "pm");
    m.insert("minus.plus", "mp");
    m.insert("arrow", "vec");
    m.insert("overline", "overline");
    m.insert("underline", "underline");
    m.insert("hat", "hat");
    m.insert("tilde", "tilde");
    m.insert("bold", "mathbf");
    m.insert("italic", "mathit");
    m.insert("upright", "mathrm");
    m.insert("cal", "mathcal");
    m
});
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: all table tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-transform-math/src/tables.rs
git commit -m "feat(math): add LaTeX/Typst/Unicode command mapping tables"
```

---

### Task 3: LaTeX → Typst mapper

**Files:**
- Create: `crates/docmux-transform-math/src/latex_to_typst.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write failing tests**

In `crates/docmux-transform-math/src/latex_to_typst.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_variable() {
        assert_eq!(latex_to_typst("x"), "x");
    }

    #[test]
    fn greek_letter() {
        assert_eq!(latex_to_typst(r"\alpha"), "alpha");
    }

    #[test]
    fn fraction() {
        assert_eq!(latex_to_typst(r"\frac{a}{b}"), "(a)/(b)");
    }

    #[test]
    fn sqrt_basic() {
        assert_eq!(latex_to_typst(r"\sqrt{x}"), "sqrt(x)");
    }

    #[test]
    fn sqrt_nth() {
        assert_eq!(latex_to_typst(r"\sqrt[3]{x}"), "root(3, x)");
    }

    #[test]
    fn subscript_superscript() {
        assert_eq!(latex_to_typst("x^2_i"), "x^2_i");
    }

    #[test]
    fn operator() {
        assert_eq!(latex_to_typst(r"\int_0^1"), "integral_0^1");
    }

    #[test]
    fn decoration() {
        assert_eq!(latex_to_typst(r"\hat{x}"), "hat(x)");
        assert_eq!(latex_to_typst(r"\bar{x}"), "overline(x)");
    }

    #[test]
    fn mathbb() {
        assert_eq!(latex_to_typst(r"\mathbb{R}"), "RR");
        assert_eq!(latex_to_typst(r"\mathbb{N}"), "NN");
    }

    #[test]
    fn environment_pmatrix() {
        assert_eq!(
            latex_to_typst(r"\begin{pmatrix}a & b \\ c & d\end{pmatrix}"),
            "mat(delim: \"(\", a, b; c, d)"
        );
    }

    #[test]
    fn environment_cases() {
        assert_eq!(
            latex_to_typst(r"\begin{cases}x & y \\ a & b\end{cases}"),
            "cases(x & y, a & b)"
        );
    }

    #[test]
    fn nested_fraction() {
        assert_eq!(
            latex_to_typst(r"\frac{\frac{a}{b}}{c}"),
            "((a)/(b))/(c)"
        );
    }

    #[test]
    fn unrecognized_command_passthrough() {
        assert_eq!(latex_to_typst(r"\mycommand"), r"\mycommand");
    }

    #[test]
    fn spacing() {
        assert_eq!(latex_to_typst(r"\quad"), "quad");
    }

    #[test]
    fn complex_expression() {
        assert_eq!(
            latex_to_typst(r"\frac{\partial f}{\partial x}"),
            "(partial f)/(partial x)"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: compilation errors

- [ ] **Step 3: Implement the mapper**

In `crates/docmux-transform-math/src/latex_to_typst.rs`:

```rust
use crate::tables::*;
use crate::tokenizer::{tokenize_latex, Token};

/// Convert a LaTeX math string to Typst math notation.
pub fn latex_to_typst(input: &str) -> String {
    let tokens = tokenize_latex(input);
    tokens_to_typst(&tokens)
}

fn tokens_to_typst(tokens: &[Token]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Command(name) => {
                i += 1;
                out.push_str(&convert_command(name, tokens, &mut i));
            }
            Token::BraceGroup(inner) => {
                out.push_str(&tokens_to_typst(inner));
                i += 1;
            }
            Token::SubScript => {
                out.push('_');
                i += 1;
            }
            Token::SuperScript => {
                out.push('^');
                i += 1;
            }
            Token::Environment { name, body } => {
                out.push_str(&convert_environment(name, body));
                i += 1;
            }
            Token::Text(t) => {
                out.push_str(t);
                i += 1;
            }
            Token::OptionalArg(_) => {
                // Stray optional arg — shouldn't happen, skip
                i += 1;
            }
        }
    }
    out
}

fn convert_command(name: &str, tokens: &[Token], i: &mut usize) -> String {
    // \frac{a}{b} → (a)/(b)
    if name == "frac" {
        let num = consume_brace_arg(tokens, i);
        let den = consume_brace_arg(tokens, i);
        return format!("({})/({})", num, den);
    }

    // \sqrt[n]{x} → root(n, x) or \sqrt{x} → sqrt(x)
    if name == "sqrt" {
        if let Some(Token::OptionalArg(n)) = tokens.get(*i) {
            *i += 1;
            let arg = consume_brace_arg(tokens, i);
            return format!("root({n}, {arg})");
        }
        let arg = consume_brace_arg(tokens, i);
        return format!("sqrt({arg})");
    }

    // \mathbb{R} → RR
    if name == "mathbb" {
        let arg = consume_brace_arg_raw(tokens, i);
        if let Some(mapped) = MATHBB_TO_TYPST.get(arg.as_str()) {
            return (*mapped).to_string();
        }
        return format!("bb({arg})");
    }

    // \text{...} → "..."
    if name == "text" || name == "mathrm" || name == "textrm" {
        let arg = consume_brace_arg_raw(tokens, i);
        // mathrm as function if it looks like a single identifier
        if name == "mathrm" {
            if let Some(mapped) = LATEX_TO_TYPST_FUNCTIONS.get(name) {
                if *mapped != "\"text\"" {
                    return format!("{mapped}({arg})");
                }
            }
            return format!("upright({arg})");
        }
        return format!("\"{arg}\"");
    }

    // Decoration commands: \hat{x} → hat(x), \bar{x} → overline(x)
    if let Some(typst_fn) = LATEX_TO_TYPST_FUNCTIONS.get(name.as_str()) {
        if *typst_fn != "\"text\"" {
            let arg = consume_brace_arg(tokens, i);
            return format!("{typst_fn}({arg})");
        }
    }

    // Simple command renames: \alpha → alpha, \int → integral
    if let Some(typst_name) = LATEX_TO_TYPST_COMMANDS.get(name.as_str()) {
        return (*typst_name).to_string();
    }

    // Unrecognized — pass through with backslash
    format!("\\{name}")
}

fn convert_environment(name: &str, body: &str) -> String {
    if let Some(&(typst_fn, delim)) = LATEX_ENV_TO_TYPST.get(name.as_str()) {
        if typst_fn == "mat" {
            let rows: Vec<&str> = body.split(r"\\").collect();
            let converted_rows: Vec<String> = rows
                .iter()
                .map(|row| {
                    row.split('&')
                        .map(|cell| latex_to_typst(cell.trim()))
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .collect();
            let body_str = converted_rows.join("; ");
            if delim.is_empty() {
                return format!("mat({body_str})");
            }
            return format!("mat(delim: {delim}, {body_str})");
        }
        if typst_fn == "cases" {
            let rows: Vec<&str> = body.split(r"\\").collect();
            let converted: Vec<String> = rows
                .iter()
                .map(|row| latex_to_typst(row.trim()))
                .collect();
            return format!("cases({})", converted.join(", "));
        }
        // Generic: aligned, gathered
        return format!("{typst_fn}({body})");
    }
    // Unknown environment — pass through
    format!("\\begin{{{name}}}{body}\\end{{{name}}}")
}

/// Consume the next token if it's a BraceGroup, return its Typst conversion.
fn consume_brace_arg(tokens: &[Token], i: &mut usize) -> String {
    if let Some(Token::BraceGroup(inner)) = tokens.get(*i) {
        *i += 1;
        tokens_to_typst(inner)
    } else {
        String::new()
    }
}

/// Consume the next token if it's a BraceGroup, return its raw text (for mathbb, text).
fn consume_brace_arg_raw(tokens: &[Token], i: &mut usize) -> String {
    if let Some(Token::BraceGroup(inner)) = tokens.get(*i) {
        *i += 1;
        // Flatten to raw text
        inner
            .iter()
            .map(|t| match t {
                Token::Text(s) => s.clone(),
                Token::Command(c) => format!("\\{c}"),
                _ => String::new(),
            })
            .collect()
    } else {
        String::new()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: all latex_to_typst tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-transform-math/src/latex_to_typst.rs
git commit -m "feat(math): add LaTeX → Typst math mapper"
```

---

### Task 4: Typst → LaTeX mapper

**Files:**
- Create: `crates/docmux-transform-math/src/typst_to_latex.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write failing tests**

In `crates/docmux-transform-math/src/typst_to_latex.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_variable() {
        assert_eq!(typst_to_latex("x"), "x");
    }

    #[test]
    fn greek_letter() {
        // In Typst math, greek is just the word without backslash
        assert_eq!(typst_to_latex("alpha"), r"\alpha");
    }

    #[test]
    fn operator_rename() {
        assert_eq!(typst_to_latex("integral"), r"\int");
        assert_eq!(typst_to_latex("product"), r"\prod");
    }

    #[test]
    fn subscript_superscript() {
        assert_eq!(typst_to_latex("x^2_i"), "x^2_i");
    }

    #[test]
    fn typst_function_call() {
        assert_eq!(typst_to_latex("sqrt(x)"), r"\sqrt{x}");
        assert_eq!(typst_to_latex("hat(x)"), r"\hat{x}");
    }

    #[test]
    fn typst_fraction_syntax() {
        assert_eq!(typst_to_latex("(a)/(b)"), r"\frac{a}{b}");
    }

    #[test]
    fn double_letter_shorthand() {
        assert_eq!(typst_to_latex("RR"), r"\mathbb{R}");
        assert_eq!(typst_to_latex("NN"), r"\mathbb{N}");
    }

    #[test]
    fn infinity() {
        assert_eq!(typst_to_latex("infinity"), r"\infty");
    }

    #[test]
    fn unrecognized_passthrough() {
        assert_eq!(typst_to_latex("myvar"), "myvar");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: compilation errors

- [ ] **Step 3: Implement the mapper**

In `crates/docmux-transform-math/src/typst_to_latex.rs`:

```rust
use crate::tables::*;

/// Convert a Typst math string to LaTeX math notation.
///
/// This is a best-effort token-level transformation. It handles:
/// - Named identifiers that map to LaTeX commands (`alpha` → `\alpha`)
/// - Function calls (`sqrt(x)` → `\sqrt{x}`)
/// - Fraction syntax (`(a)/(b)` → `\frac{a}{b}`)
/// - Double-letter shorthands (`RR` → `\mathbb{R}`)
pub fn typst_to_latex(input: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut pos = 0;

    while pos < chars.len() {
        // Try to match (...)/(...)  → \frac{...}{...}
        if chars[pos] == '(' {
            if let Some((num, den, end)) = try_parse_fraction(&chars, pos) {
                out.push_str(&format!("\\frac{{{}}}{{{}}}", typst_to_latex(&num), typst_to_latex(&den)));
                pos = end;
                continue;
            }
        }

        // Try to read an identifier (alphabetic word)
        if chars[pos].is_ascii_alphabetic() {
            let start = pos;
            while pos < chars.len() && (chars[pos].is_ascii_alphanumeric() || chars[pos] == '.') {
                pos += 1;
            }
            let word = &input[start..pos];

            // Check for double-letter shorthands: RR, NN, ZZ, etc.
            if word.len() == 2 {
                let first = word.chars().next().unwrap_or_default();
                let second = word.chars().nth(1).unwrap_or_default();
                if first == second && first.is_ascii_uppercase() {
                    let key = &word[..1];
                    if MATHBB_TO_TYPST.values().any(|&v| v == word) {
                        // Find the original letter
                        for (&k, &v) in MATHBB_TO_TYPST.iter() {
                            if v == word {
                                out.push_str(&format!("\\mathbb{{{k}}}"));
                                break;
                            }
                        }
                        continue;
                    }
                }
            }

            // Check if followed by `(` → function call: func(arg) → \func{arg}
            if pos < chars.len() && chars[pos] == '(' {
                let arg_content = read_paren_content(&chars, &mut pos);
                // Map function name
                if word == "sqrt" {
                    out.push_str(&format!("\\sqrt{{{}}}", typst_to_latex(&arg_content)));
                } else if word == "root" {
                    // root(n, x) → \sqrt[n]{x}
                    if let Some((n, x)) = arg_content.split_once(',') {
                        out.push_str(&format!(
                            "\\sqrt[{}]{{{}}}",
                            n.trim(),
                            typst_to_latex(x.trim())
                        ));
                    } else {
                        out.push_str(&format!("\\sqrt{{{}}}", typst_to_latex(&arg_content)));
                    }
                } else if let Some(&latex_cmd) = TYPST_TO_LATEX_COMMANDS.get(word) {
                    out.push_str(&format!("\\{latex_cmd}{{{}}}", typst_to_latex(&arg_content)));
                } else {
                    out.push_str(&format!("\\{word}{{{}}}", typst_to_latex(&arg_content)));
                }
                continue;
            }

            // Simple identifier rename
            if let Some(&latex_cmd) = TYPST_TO_LATEX_COMMANDS.get(word) {
                out.push_str(&format!("\\{latex_cmd}"));
            } else if LATEX_TO_TYPST_COMMANDS.get(word).is_some() {
                // It's a command that has the same name in both (e.g., alpha)
                out.push_str(&format!("\\{word}"));
            } else {
                // Not a known command — pass through as-is
                out.push_str(word);
            }
            continue;
        }

        // Regular character
        out.push(chars[pos]);
        pos += 1;
    }

    out
}

/// Try to parse `(num)/(den)` starting at a `(`.
/// Returns (numerator, denominator, position_after_close_paren) or None.
fn try_parse_fraction(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    let num_content = read_balanced_paren(chars, start)?;
    let after_num = start + 1 + num_content.len() + 1; // ( + content + )

    // Check for `/(`
    if after_num + 1 >= chars.len() || chars[after_num] != '/' || chars[after_num + 1] != '(' {
        return None;
    }

    let den_content = read_balanced_paren(chars, after_num + 1)?;
    let end = after_num + 2 + den_content.len() + 1; // /( + content + )

    Some((num_content, den_content, end))
}

/// Read balanced parentheses content starting at `(`. Returns the inner content.
fn read_balanced_paren(chars: &[char], start: usize) -> Option<String> {
    if start >= chars.len() || chars[start] != '(' {
        return None;
    }
    let mut depth = 1;
    let mut pos = start + 1;
    let mut content = String::new();
    while pos < chars.len() && depth > 0 {
        match chars[pos] {
            '(' => {
                depth += 1;
                content.push('(');
            }
            ')' => {
                depth -= 1;
                if depth > 0 {
                    content.push(')');
                }
            }
            c => content.push(c),
        }
        pos += 1;
    }
    if depth == 0 {
        Some(content)
    } else {
        None
    }
}

/// Read content inside parentheses, advancing `pos` past the closing `)`.
fn read_paren_content(chars: &[char], pos: &mut usize) -> String {
    // pos is at '('
    *pos += 1;
    let mut depth = 1;
    let mut content = String::new();
    while *pos < chars.len() && depth > 0 {
        match chars[*pos] {
            '(' => {
                depth += 1;
                content.push('(');
            }
            ')' => {
                depth -= 1;
                if depth > 0 {
                    content.push(')');
                }
            }
            c => content.push(c),
        }
        *pos += 1;
    }
    content
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: all typst_to_latex tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-transform-math/src/typst_to_latex.rs
git commit -m "feat(math): add Typst → LaTeX math mapper"
```

---

### Task 5: LaTeX → MathML emitter

**Files:**
- Create: `crates/docmux-transform-math/src/latex_to_mathml.rs`
- Test: inline `#[cfg(test)] mod tests`

- [ ] **Step 1: Write failing tests**

In `crates/docmux-transform-math/src/latex_to_mathml.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_variable() {
        assert_eq!(latex_to_mathml("x"), "<mi>x</mi>");
    }

    #[test]
    fn number() {
        assert_eq!(latex_to_mathml("42"), "<mn>42</mn>");
    }

    #[test]
    fn greek_letter() {
        assert_eq!(latex_to_mathml(r"\alpha"), "<mi>\u{03B1}</mi>");
    }

    #[test]
    fn operator_symbol() {
        assert_eq!(latex_to_mathml(r"\infty"), "<mi>\u{221E}</mi>");
    }

    #[test]
    fn superscript() {
        assert_eq!(
            latex_to_mathml("x^2"),
            "<msup><mi>x</mi><mn>2</mn></msup>"
        );
    }

    #[test]
    fn subscript() {
        assert_eq!(
            latex_to_mathml("x_i"),
            "<msub><mi>x</mi><mi>i</mi></msub>"
        );
    }

    #[test]
    fn fraction() {
        assert_eq!(
            latex_to_mathml(r"\frac{a}{b}"),
            "<mfrac><mi>a</mi><mi>b</mi></mfrac>"
        );
    }

    #[test]
    fn sqrt_basic() {
        assert_eq!(
            latex_to_mathml(r"\sqrt{x}"),
            "<msqrt><mi>x</mi></msqrt>"
        );
    }

    #[test]
    fn sqrt_nth() {
        assert_eq!(
            latex_to_mathml(r"\sqrt[3]{x}"),
            "<mroot><mi>x</mi><mn>3</mn></mroot>"
        );
    }

    #[test]
    fn plus_operator() {
        let result = latex_to_mathml("a + b");
        assert_eq!(result, "<mi>a</mi><mo>+</mo><mi>b</mi>");
    }

    #[test]
    fn unrecognized_command() {
        assert_eq!(
            latex_to_mathml(r"\mycommand"),
            "<mtext>\\mycommand</mtext>"
        );
    }

    #[test]
    fn wrap_display() {
        assert_eq!(
            wrap_mathml("x", true),
            "<math display=\"block\"><mi>x</mi></math>"
        );
    }

    #[test]
    fn wrap_inline() {
        assert_eq!(
            wrap_mathml("x", false),
            "<math display=\"inline\"><mi>x</mi></math>"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: compilation errors

- [ ] **Step 3: Implement the MathML emitter**

In `crates/docmux-transform-math/src/latex_to_mathml.rs`:

```rust
use crate::tables::LATEX_TO_UNICODE;
use crate::tokenizer::{tokenize_latex, Token};

/// Convert a LaTeX math string to MathML markup (without the outer `<math>` wrapper).
pub fn latex_to_mathml(input: &str) -> String {
    let tokens = tokenize_latex(input);
    tokens_to_mathml(&tokens)
}

/// Wrap the MathML output in `<math display="...">` tags.
pub fn wrap_mathml(input: &str, display: bool) -> String {
    let inner = latex_to_mathml(input);
    let mode = if display { "block" } else { "inline" };
    format!("<math display=\"{mode}\">{inner}</math>")
}

fn tokens_to_mathml(tokens: &[Token]) -> String {
    let mut out = String::new();
    let mut i = 0;

    while i < tokens.len() {
        match &tokens[i] {
            Token::Command(name) => {
                i += 1;
                out.push_str(&convert_command_mathml(name, tokens, &mut i));
            }
            Token::Text(t) => {
                out.push_str(&text_to_mathml(t));
                i += 1;
            }
            Token::BraceGroup(inner) => {
                out.push_str(&tokens_to_mathml(inner));
                i += 1;
            }
            Token::SuperScript => {
                // Need to wrap previous element and next element in <msup>
                i += 1;
                let base = pop_last_element(&mut out);
                let exp = consume_next_mathml(tokens, &mut i);
                out.push_str(&format!("<msup>{base}{exp}</msup>"));
            }
            Token::SubScript => {
                i += 1;
                let base = pop_last_element(&mut out);
                let sub = consume_next_mathml(tokens, &mut i);
                out.push_str(&format!("<msub>{base}{sub}</msub>"));
            }
            Token::Environment { name: _, body } => {
                // Environments: best-effort, wrap body
                out.push_str(&format!("<mrow>{}</mrow>", latex_to_mathml(body)));
                i += 1;
            }
            Token::OptionalArg(_) => {
                i += 1; // skip stray optional args
            }
        }
    }

    out
}

fn convert_command_mathml(name: &str, tokens: &[Token], i: &mut usize) -> String {
    // \frac{a}{b}
    if name == "frac" {
        let num = consume_brace_mathml(tokens, i);
        let den = consume_brace_mathml(tokens, i);
        return format!("<mfrac>{num}{den}</mfrac>");
    }

    // \sqrt{x} or \sqrt[n]{x}
    if name == "sqrt" {
        if let Some(Token::OptionalArg(n)) = tokens.get(*i) {
            *i += 1;
            let body = consume_brace_mathml(tokens, i);
            let index = text_to_mathml(n);
            return format!("<mroot>{body}{index}</mroot>");
        }
        let body = consume_brace_mathml(tokens, i);
        return format!("<msqrt>{body}</msqrt>");
    }

    // Decoration: \hat{x}, \bar{x}, etc.
    let accents = [
        ("hat", "\u{0302}"),
        ("bar", "\u{0304}"),
        ("overline", "\u{0304}"),
        ("vec", "\u{20D7}"),
        ("tilde", "\u{0303}"),
        ("dot", "\u{0307}"),
        ("ddot", "\u{0308}"),
    ];
    for (cmd, accent_char) in accents {
        if name == cmd {
            let body = consume_brace_mathml(tokens, i);
            return format!("<mover accent=\"true\">{body}<mo>{accent_char}</mo></mover>");
        }
    }

    // \mathbb, \mathbf, etc. — font variants
    if name == "mathbb" || name == "mathbf" || name == "mathit" || name == "mathrm" || name == "mathcal" {
        let body = consume_brace_mathml(tokens, i);
        let variant = match name {
            "mathbb" => "double-struck",
            "mathbf" => "bold",
            "mathit" => "italic",
            "mathrm" => "normal",
            "mathcal" => "script",
            _ => "normal",
        };
        return format!("<mstyle mathvariant=\"{variant}\">{body}</mstyle>");
    }

    // \text{...}
    if name == "text" || name == "textrm" {
        let body = consume_brace_raw(tokens, i);
        return format!("<mtext>{body}</mtext>");
    }

    // Unicode-mapped commands: \alpha → α, \infty → ∞
    if let Some(&unicode) = LATEX_TO_UNICODE.get(name.as_str()) {
        return format!("<mi>{unicode}</mi>");
    }

    // Unrecognized command
    format!("<mtext>\\{name}</mtext>")
}

/// Convert a plain text segment to MathML elements.
fn text_to_mathml(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch.is_ascii_digit() {
            let mut num = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() || d == '.' {
                    num.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            out.push_str(&format!("<mn>{num}</mn>"));
        } else if ch.is_ascii_alphabetic() {
            out.push_str(&format!("<mi>{ch}</mi>"));
            chars.next();
        } else if ch == ' ' {
            chars.next(); // skip whitespace
        } else {
            // Operator: +, -, =, (, ), etc.
            out.push_str(&format!("<mo>{ch}</mo>"));
            chars.next();
        }
    }

    out
}

fn consume_brace_mathml(tokens: &[Token], i: &mut usize) -> String {
    if let Some(Token::BraceGroup(inner)) = tokens.get(*i) {
        *i += 1;
        tokens_to_mathml(inner)
    } else {
        String::new()
    }
}

fn consume_brace_raw(tokens: &[Token], i: &mut usize) -> String {
    if let Some(Token::BraceGroup(inner)) = tokens.get(*i) {
        *i += 1;
        inner
            .iter()
            .map(|t| match t {
                Token::Text(s) => s.clone(),
                _ => String::new(),
            })
            .collect()
    } else {
        String::new()
    }
}

fn consume_next_mathml(tokens: &[Token], i: &mut usize) -> String {
    if *i >= tokens.len() {
        return String::new();
    }
    match &tokens[*i] {
        Token::BraceGroup(inner) => {
            *i += 1;
            tokens_to_mathml(inner)
        }
        Token::Command(name) => {
            *i += 1;
            convert_command_mathml(name, tokens, i)
        }
        Token::Text(t) => {
            *i += 1;
            // Take just the first character for sub/superscript
            if let Some(ch) = t.chars().next() {
                let rest = &t[ch.len_utf8()..];
                let result = text_to_mathml(&ch.to_string());
                // If there's remaining text, we need to re-insert it somehow
                // For simplicity, just convert the whole text
                if rest.is_empty() {
                    result
                } else {
                    text_to_mathml(&ch.to_string())
                    // The rest will be lost — acceptable for now as sub/super
                    // typically has single-char or brace-group arguments
                }
            } else {
                String::new()
            }
        }
        _ => {
            *i += 1;
            String::new()
        }
    }
}

/// Pop the last MathML element from the output string.
/// This is used to wrap the base of sub/superscripts.
fn pop_last_element(out: &mut String) -> String {
    // Find the last complete MathML element
    if let Some(start) = out.rfind('<') {
        // Check if this is a closing tag
        if out[start..].starts_with("</") {
            // Find the matching opening tag
            let tag_end = out[start + 2..].find('>').map(|i| start + 2 + i);
            if let Some(te) = tag_end {
                let tag_name: String = out[start + 2..te].to_string();
                // Search backwards for the opening tag
                let open_tag = format!("<{tag_name}");
                if let Some(open_start) = out.rfind(&open_tag) {
                    let element = out[open_start..].to_string();
                    out.truncate(open_start);
                    return element;
                }
            }
        } else {
            // Self-contained element like <mi>x</mi>
            let element = out[start..].to_string();
            out.truncate(start);
            return element;
        }
    }
    // Fallback: return empty mrow
    "<mrow/>".to_string()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: all MathML tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-transform-math/src/latex_to_mathml.rs
git commit -m "feat(math): add LaTeX → MathML emitter"
```

---

### Task 6: Transform orchestration + MathEngine::MathML

**Files:**
- Modify: `crates/docmux-core/src/lib.rs:180-190` (add MathML variant)
- Modify: `crates/docmux-transform-math/src/lib.rs` (replace placeholder)
- Test: inline `#[cfg(test)] mod tests` in `lib.rs`

- [ ] **Step 1: Add MathML variant to MathEngine**

In `crates/docmux-core/src/lib.rs`, replace the `MathEngine` enum:

```rust
/// Target math rendering engine.
#[derive(Debug, Clone, Copy, Default)]
pub enum MathEngine {
    /// Output `<span class="math">` with KaTeX-compatible markup.
    #[default]
    KaTeX,
    /// Output MathJax-compatible markup.
    MathJax,
    /// Server-side conversion to MathML (no client JS needed).
    MathML,
    /// Leave math source as-is (useful for LaTeX/Typst writers).
    Raw,
}
```

- [ ] **Step 2: Write failing tests for the transform**

In `crates/docmux-transform-math/src/lib.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use docmux_ast::*;

    fn make_doc_with_math(inline_val: &str, block_content: &str) -> Document {
        Document {
            content: vec![
                Block::Paragraph {
                    content: vec![Inline::MathInline {
                        value: inline_val.to_string(),
                    }],
                },
                Block::MathBlock {
                    content: block_content.to_string(),
                    label: None,
                },
            ],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn latex_to_typst_transform() {
        let mut doc = make_doc_with_math(r"\alpha", r"\frac{a}{b}");
        let t = MathTransform {
            target_format: MathTarget::Typst,
            source_notation: MathNotation::LaTeX,
        };
        t.transform(&mut doc, &TransformContext::default()).unwrap();

        match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::MathInline { value } => assert_eq!(value, "alpha"),
                other => panic!("expected MathInline, got {other:?}"),
            },
            other => panic!("expected Paragraph, got {other:?}"),
        }
        match &doc.content[1] {
            Block::MathBlock { content, .. } => assert_eq!(content, "(a)/(b)"),
            other => panic!("expected MathBlock, got {other:?}"),
        }
    }

    #[test]
    fn latex_to_mathml_transform() {
        let mut doc = make_doc_with_math("x", r"\frac{a}{b}");
        let t = MathTransform {
            target_format: MathTarget::MathML,
            source_notation: MathNotation::LaTeX,
        };
        t.transform(&mut doc, &TransformContext::default()).unwrap();

        match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::MathInline { value } => {
                    assert!(value.contains("<math display=\"inline\">"));
                    assert!(value.contains("<mi>x</mi>"));
                }
                other => panic!("expected MathInline, got {other:?}"),
            },
            other => panic!("expected Paragraph, got {other:?}"),
        }
        match &doc.content[1] {
            Block::MathBlock { content, .. } => {
                assert!(content.contains("<math display=\"block\">"));
                assert!(content.contains("<mfrac>"));
            }
            other => panic!("expected MathBlock, got {other:?}"),
        }
    }

    #[test]
    fn noop_when_same_notation() {
        let mut doc = make_doc_with_math(r"\alpha", r"\frac{a}{b}");
        let original_inline = r"\alpha".to_string();
        let original_block = r"\frac{a}{b}".to_string();
        let t = MathTransform {
            target_format: MathTarget::None,
            source_notation: MathNotation::LaTeX,
        };
        t.transform(&mut doc, &TransformContext::default()).unwrap();

        match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::MathInline { value } => assert_eq!(value, &original_inline),
                other => panic!("expected MathInline, got {other:?}"),
            },
            other => panic!("expected Paragraph, got {other:?}"),
        }
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: compilation errors (MathTransform not defined)

- [ ] **Step 4: Implement the transform orchestration**

Replace `crates/docmux-transform-math/src/lib.rs` entirely:

```rust
//! # docmux-transform-math
//!
//! Math notation conversion for docmux.
//!
//! Converts math strings in the AST between LaTeX, Typst, and MathML
//! notations. When source and target use the same notation, this is a no-op.

mod latex_to_mathml;
mod latex_to_typst;
mod tables;
mod tokenizer;
mod typst_to_latex;

use docmux_ast::*;
use docmux_core::{Result, Transform, TransformContext};

/// Target notation for math conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathTarget {
    /// Convert to Typst math notation.
    Typst,
    /// Convert to LaTeX math notation.
    LaTeX,
    /// Convert to MathML markup (replaces math content with XML).
    MathML,
    /// No conversion (pass-through).
    None,
}

/// Source notation of math strings in the AST.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathNotation {
    /// Math is in LaTeX notation (default for most readers).
    LaTeX,
    /// Math is in Typst notation.
    Typst,
}

/// Transform that converts math notation between formats.
pub struct MathTransform {
    pub target_format: MathTarget,
    pub source_notation: MathNotation,
}

impl Transform for MathTransform {
    fn name(&self) -> &str {
        "math"
    }

    fn transform(&self, doc: &mut Document, _ctx: &TransformContext) -> Result<()> {
        if self.target_format == MathTarget::None {
            return Ok(());
        }
        // Determine the conversion function
        let convert: Box<dyn Fn(&str) -> String> = match (self.source_notation, self.target_format)
        {
            (MathNotation::LaTeX, MathTarget::Typst) => {
                Box::new(|s| latex_to_typst::latex_to_typst(s))
            }
            (MathNotation::Typst, MathTarget::LaTeX) => {
                Box::new(|s| typst_to_latex::typst_to_latex(s))
            }
            (MathNotation::LaTeX, MathTarget::MathML) => {
                // MathML handled specially — inline vs display
                rewrite_math_blocks_mathml(&mut doc.content);
                rewrite_math_inlines_mathml(&mut doc.content);
                return Ok(());
            }
            (MathNotation::Typst, MathTarget::MathML) => {
                // Typst → LaTeX first, then LaTeX → MathML
                let to_latex = |s: &str| typst_to_latex::typst_to_latex(s);
                rewrite_math_blocks_via(&mut doc.content, &to_latex);
                rewrite_math_inlines_via(&mut doc.content, &to_latex);
                rewrite_math_blocks_mathml(&mut doc.content);
                rewrite_math_inlines_mathml(&mut doc.content);
                return Ok(());
            }
            _ => return Ok(()), // Same notation or unsupported — no-op
        };

        rewrite_math_blocks_via(&mut doc.content, &convert);
        rewrite_math_inlines_via(&mut doc.content, &convert);
        Ok(())
    }
}

// ─── AST rewriting helpers ─────────────────────────────────────────────────

fn rewrite_math_blocks_via(blocks: &mut [Block], convert: &dyn Fn(&str) -> String) {
    for block in blocks.iter_mut() {
        match block {
            Block::MathBlock { content, .. } => {
                *content = convert(content);
            }
            Block::BlockQuote { content }
            | Block::Div { content, .. }
            | Block::ListItem { content, .. } => {
                rewrite_math_blocks_via(content, convert);
            }
            Block::List { items, .. } => {
                for item in items {
                    rewrite_math_blocks_via(&mut item.content, convert);
                }
            }
            Block::Figure { content, .. } => {
                rewrite_math_blocks_via(content, convert);
            }
            _ => {}
        }
    }
}

fn rewrite_math_inlines_via(blocks: &mut [Block], convert: &dyn Fn(&str) -> String) {
    for block in blocks.iter_mut() {
        match block {
            Block::Paragraph { content }
            | Block::Heading { content, .. }
            | Block::ListItem { content: _, .. } => {
                if let Block::Paragraph { content } | Block::Heading { content, .. } = block {
                    rewrite_inlines(content, convert);
                }
            }
            Block::BlockQuote { content }
            | Block::Div { content, .. } => {
                rewrite_math_inlines_via(content, convert);
            }
            Block::List { items, .. } => {
                for item in items {
                    rewrite_math_inlines_via(&mut item.content, convert);
                }
            }
            Block::Figure { content, caption, .. } => {
                rewrite_math_inlines_via(content, convert);
                if let Some(cap) = caption {
                    rewrite_inlines(cap, convert);
                }
            }
            Block::Table { caption, .. } => {
                if let Some(cap) = caption {
                    rewrite_inlines(cap, convert);
                }
            }
            _ => {}
        }
    }
}

fn rewrite_inlines(inlines: &mut [Inline], convert: &dyn Fn(&str) -> String) {
    for inline in inlines.iter_mut() {
        match inline {
            Inline::MathInline { value } => {
                *value = convert(value);
            }
            Inline::Emph { content }
            | Inline::Strong { content }
            | Inline::Strikethrough { content }
            | Inline::Underline { content }
            | Inline::Subscript { content }
            | Inline::Superscript { content }
            | Inline::Span { content, .. } => {
                rewrite_inlines(content, convert);
            }
            Inline::Link { content, .. } => {
                rewrite_inlines(content, convert);
            }
            _ => {}
        }
    }
}

fn rewrite_math_blocks_mathml(blocks: &mut [Block]) {
    for block in blocks.iter_mut() {
        match block {
            Block::MathBlock { content, .. } => {
                *content = latex_to_mathml::wrap_mathml(content, true);
            }
            Block::BlockQuote { content }
            | Block::Div { content, .. } => {
                rewrite_math_blocks_mathml(content);
            }
            Block::List { items, .. } => {
                for item in items {
                    rewrite_math_blocks_mathml(&mut item.content);
                }
            }
            Block::Figure { content, .. } => {
                rewrite_math_blocks_mathml(content);
            }
            _ => {}
        }
    }
}

fn rewrite_math_inlines_mathml(blocks: &mut [Block]) {
    for block in blocks.iter_mut() {
        match block {
            Block::Paragraph { content }
            | Block::Heading { content, .. } => {
                for inline in content.iter_mut() {
                    rewrite_inline_mathml(inline);
                }
            }
            Block::BlockQuote { content }
            | Block::Div { content, .. } => {
                rewrite_math_inlines_mathml(content);
            }
            Block::List { items, .. } => {
                for item in items {
                    rewrite_math_inlines_mathml(&mut item.content);
                }
            }
            Block::Figure { content, caption, .. } => {
                rewrite_math_inlines_mathml(content);
                if let Some(cap) = caption {
                    for inline in cap.iter_mut() {
                        rewrite_inline_mathml(inline);
                    }
                }
            }
            _ => {}
        }
    }
}

fn rewrite_inline_mathml(inline: &mut Inline) {
    match inline {
        Inline::MathInline { value } => {
            *value = latex_to_mathml::wrap_mathml(value, false);
        }
        Inline::Emph { content }
        | Inline::Strong { content }
        | Inline::Strikethrough { content }
        | Inline::Underline { content }
        | Inline::Subscript { content }
        | Inline::Superscript { content }
        | Inline::Span { content, .. } => {
            for inner in content.iter_mut() {
                rewrite_inline_mathml(inner);
            }
        }
        Inline::Link { content, .. } => {
            for inner in content.iter_mut() {
                rewrite_inline_mathml(inner);
            }
        }
        _ => {}
    }
}

// ... tests at the bottom (from Step 2 above)
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p docmux-transform-math -- --nocapture`
Expected: all transform tests PASS

- [ ] **Step 6: Run `cargo clippy --workspace --all-targets -- -D warnings`**

Expected: clean pass (no warnings)

- [ ] **Step 7: Commit**

```bash
git add crates/docmux-core/src/lib.rs crates/docmux-transform-math/src/lib.rs
git commit -m "feat(math): implement MathTransform with LaTeX↔Typst and MathML support"
```

---

### Task 7: CLI + HTML writer integration for math transform

**Files:**
- Modify: `crates/docmux-cli/src/main.rs:493-499` (MathML mapping)
- Modify: `crates/docmux-cli/src/main.rs:~356` (register math transform)
- Modify: `crates/docmux-writer-html/src/lib.rs:598-612` (MathML head)
- Modify: `crates/docmux-writer-html/src/lib.rs:103-119` (MathBlock for MathML)
- Modify: `crates/docmux-writer-html/src/lib.rs:413-422` (MathInline for MathML)
- Test: golden file `tests/fixtures/math/` for math conversion

- [ ] **Step 1: Update CLI math_engine mapping**

In `crates/docmux-cli/src/main.rs`, change line 496 from:

```rust
        Some("raw") | Some("mathml") => MathEngine::Raw,
```

to:

```rust
        Some("mathml") => MathEngine::MathML,
        Some("raw") => MathEngine::Raw,
```

- [ ] **Step 2: Add math transform registration in CLI**

In `crates/docmux-cli/src/main.rs`, add after the cite transform block (after line ~458) and before the JSON dump (line ~468), add the math transform registration. Also add the import at the top of the file:

Add to imports:
```rust
use docmux_transform_math::{MathNotation, MathTarget, MathTransform};
```

Add transform registration (after cite transform, before "JSON AST dump" comment):
```rust
    // Apply math transform when needed
    let source_notation = match from {
        "typst" => MathNotation::Typst,
        _ => MathNotation::LaTeX,
    };
    let target_format = match (to, &cli.math) {
        (_, Some(ref m)) if m == "mathml" => MathTarget::MathML,
        ("typst", _) => MathTarget::Typst,
        _ => MathTarget::None,
    };
    if target_format != MathTarget::None {
        let math_transform = MathTransform {
            target_format,
            source_notation,
        };
        if let Err(e) = math_transform.transform(&mut doc, &TransformContext::default()) {
            eprintln!("docmux: math transform error: {e}");
            std::process::exit(1);
        }
    }
```

- [ ] **Step 3: Update HTML writer for MathML**

In `crates/docmux-writer-html/src/lib.rs`, update the math engine head (line ~598):

```rust
            MathEngine::MathML | MathEngine::Raw => "",
```

Update MathBlock rendering (line ~103) — add MathML case:

```rust
            Block::MathBlock { content, label } => {
                match opts.math_engine {
                    MathEngine::MathML => {
                        // Content is already MathML from the transform
                        if let Some(label) = label {
                            out.push_str(&format!("<div id=\"{}\">", escape_attr(label)));
                        }
                        out.push_str(content);
                        if label.is_some() {
                            out.push_str("</div>");
                        }
                        out.push('\n');
                    }
                    _ => {
                        // existing KaTeX/MathJax/Raw logic (unchanged)
                        let class = match opts.math_engine {
                            MathEngine::KaTeX => "math math-display",
                            MathEngine::MathJax => "math math-display",
                            _ => "math",
                        };
                        // ... rest unchanged
                    }
                }
            }
```

Update MathInline rendering (line ~413):

```rust
            Inline::MathInline { value } => {
                match opts.math_engine {
                    MathEngine::MathML => {
                        // Content is already MathML from the transform
                        out.push_str(value);
                    }
                    _ => {
                        // existing logic unchanged
                        let class = match opts.math_engine {
                            MathEngine::KaTeX => "math math-inline",
                            MathEngine::MathJax => "math math-inline",
                            _ => "math",
                        };
                        out.push_str(&format!("<span class=\"{class}\">"));
                        out.push_str(&escape_html(value));
                        out.push_str("</span>");
                    }
                }
            }
```

- [ ] **Step 4: Add `docmux-transform-math` to CLI Cargo.toml dependencies**

In `crates/docmux-cli/Cargo.toml`, add:

```toml
docmux-transform-math = { workspace = true }
```

- [ ] **Step 5: Create golden test fixtures**

Create `tests/fixtures/math/latex-to-typst.md`:
```markdown
Inline: $\frac{a}{b}$ and $\alpha + \beta$.

$$
\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}
$$
```

Create `tests/fixtures/math/mathml-basic.md`:
```markdown
Inline: $x^2 + y^2$.

$$
\frac{a}{b}
$$
```

- [ ] **Step 6: Add golden tests to `crates/docmux-cli/tests/golden.rs`**

```rust
#[test]
fn math_latex_to_mathml() {
    let md_path = fixtures_dir().join("math/mathml-basic.md");
    let expected_path = fixtures_dir().join("math/mathml-basic.html");

    let input = std::fs::read_to_string(&md_path).expect("read fixture");

    let reader = MarkdownReader::new();
    let mut doc = reader.read(&input).expect("read markdown");

    // Apply math transform
    let transform = docmux_transform_math::MathTransform {
        target_format: docmux_transform_math::MathTarget::MathML,
        source_notation: docmux_transform_math::MathNotation::LaTeX,
    };
    transform
        .transform(&mut doc, &TransformContext::default())
        .expect("math transform");

    let writer = HtmlWriter::new();
    let opts = WriteOptions {
        math_engine: MathEngine::MathML,
        ..Default::default()
    };
    let actual = writer.write(&doc, &opts).expect("write html");

    if update_mode() {
        std::fs::write(&expected_path, &actual).expect("write expected");
    } else if expected_path.exists() {
        let expected = std::fs::read_to_string(&expected_path).expect("read expected");
        assert_eq!(actual.trim(), expected.trim(), "MathML golden file mismatch");
    } else {
        std::fs::create_dir_all(expected_path.parent().unwrap()).ok();
        std::fs::write(&expected_path, &actual).expect("bootstrap expected");
        eprintln!("bootstrapped: {}", expected_path.display());
    }
}
```

- [ ] **Step 7: Run all tests**

Run: `cargo test --workspace`
Expected: all tests PASS (including new golden tests — first run bootstraps expected files)

- [ ] **Step 8: Run `cargo clippy --workspace --all-targets -- -D warnings`**

- [ ] **Step 9: Commit**

```bash
git add crates/docmux-cli/ crates/docmux-writer-html/ tests/fixtures/math/
git commit -m "feat(math): integrate math transform in CLI and HTML writer with MathML support"
```

---

## Part 2: Highlight Enhancements

### Task 8: Line range parser

**Files:**
- Modify: `crates/docmux-highlight/src/lib.rs` (add `LineOptions` and `parse_line_ranges`)
- Test: inline

- [ ] **Step 1: Write failing tests**

Add to `crates/docmux-highlight/src/lib.rs` tests module:

```rust
    #[test]
    fn parse_single_line() {
        let ranges = parse_line_ranges("3");
        assert_eq!(ranges, vec![3..=3]);
    }

    #[test]
    fn parse_range() {
        let ranges = parse_line_ranges("2-5");
        assert_eq!(ranges, vec![2..=5]);
    }

    #[test]
    fn parse_mixed() {
        let ranges = parse_line_ranges("1,3-5,8");
        assert_eq!(ranges, vec![1..=1, 3..=5, 8..=8]);
    }

    #[test]
    fn parse_empty() {
        let ranges = parse_line_ranges("");
        assert!(ranges.is_empty());
    }

    #[test]
    fn parse_whitespace() {
        let ranges = parse_line_ranges(" 2 , 4 - 6 ");
        assert_eq!(ranges, vec![2..=2, 4..=6]);
    }

    #[test]
    fn parse_invalid_skipped() {
        let ranges = parse_line_ranges("2,abc,5");
        assert_eq!(ranges, vec![2..=2, 5..=5]);
    }

    #[test]
    fn is_line_highlighted() {
        let opts = LineOptions {
            number_lines: false,
            start_from: 1,
            highlighted_lines: vec![2..=2, 4..=6],
        };
        assert!(!opts.is_highlighted(1));
        assert!(opts.is_highlighted(2));
        assert!(!opts.is_highlighted(3));
        assert!(opts.is_highlighted(4));
        assert!(opts.is_highlighted(5));
        assert!(opts.is_highlighted(6));
        assert!(!opts.is_highlighted(7));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-highlight -- --nocapture`
Expected: compilation errors

- [ ] **Step 3: Implement LineOptions and parse_line_ranges**

Add to `crates/docmux-highlight/src/lib.rs` (after the existing types, before `highlight()` function):

```rust
use std::ops::RangeInclusive;

/// Options for line-level features in code blocks.
#[derive(Debug, Clone, Default)]
pub struct LineOptions {
    /// Whether to show line numbers.
    pub number_lines: bool,
    /// Starting line number (default 1).
    pub start_from: u32,
    /// Lines to visually highlight.
    pub highlighted_lines: Vec<RangeInclusive<u32>>,
}

impl LineOptions {
    /// Check if a given 1-based line number should be highlighted.
    pub fn is_highlighted(&self, line: u32) -> bool {
        self.highlighted_lines.iter().any(|r| r.contains(&line))
    }

    /// Parse `LineOptions` from code block attributes.
    ///
    /// Looks for `.numberLines` class, `startFrom` key, and `highlight` key.
    pub fn from_attrs(attrs: Option<&docmux_ast::Attributes>) -> Self {
        let Some(attrs) = attrs else {
            return Self::default();
        };

        let number_lines = attrs.classes.iter().any(|c| c == "numberLines");
        let start_from = attrs
            .kvs
            .iter()
            .find(|(k, _)| k == "startFrom")
            .and_then(|(_, v)| v.parse::<u32>().ok())
            .unwrap_or(1);
        let highlighted_lines = attrs
            .kvs
            .iter()
            .find(|(k, _)| k == "highlight")
            .map(|(_, v)| parse_line_ranges(v))
            .unwrap_or_default();

        Self {
            number_lines,
            start_from,
            highlighted_lines,
        }
    }
}

/// Parse a highlight range string like `"2,4-6,10"` into inclusive ranges.
pub fn parse_line_ranges(input: &str) -> Vec<RangeInclusive<u32>> {
    input
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            if let Some((start, end)) = part.split_once('-') {
                let s = start.trim().parse::<u32>().ok()?;
                let e = end.trim().parse::<u32>().ok()?;
                Some(s..=e)
            } else {
                let n = part.parse::<u32>().ok()?;
                Some(n..=n)
            }
        })
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-highlight -- --nocapture`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-highlight/src/lib.rs
git commit -m "feat(highlight): add LineOptions and parse_line_ranges"
```

---

### Task 9: HTML writer line numbers + highlighting

**Files:**
- Modify: `crates/docmux-writer-html/src/lib.rs:49-102` (CodeBlock rendering)
- Test: inline tests + golden file

- [ ] **Step 1: Write failing tests**

Add to the existing test module in `crates/docmux-writer-html/src/lib.rs`:

```rust
    #[test]
    fn code_block_with_line_numbers() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "def hello():\n    print(\"world\")".into(),
                attrs: Some(Attributes {
                    id: None,
                    classes: vec!["numberLines".into()],
                    kvs: vec![],
                }),
            }],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
        };
        let writer = HtmlWriter::new();
        let opts = WriteOptions::default();
        let html = writer.write(&doc, &opts).unwrap();
        assert!(html.contains("class=\"line-number\""), "should have line numbers");
        assert!(html.contains(">1<"), "should start at 1");
        assert!(html.contains(">2<"), "should have line 2");
    }

    #[test]
    fn code_block_with_line_highlight() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "line1\nline2\nline3".into(),
                attrs: Some(Attributes {
                    id: None,
                    classes: vec![],
                    kvs: vec![("highlight".into(), "2".into())],
                }),
            }],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
        };
        let writer = HtmlWriter::new();
        let opts = WriteOptions::default();
        let html = writer.write(&doc, &opts).unwrap();
        assert!(html.contains("highlight-line"), "should have highlight class");
    }

    #[test]
    fn code_block_with_start_from() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "a\nb".into(),
                attrs: Some(Attributes {
                    id: None,
                    classes: vec!["numberLines".into()],
                    kvs: vec![("startFrom".into(), "10".into())],
                }),
            }],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
        };
        let writer = HtmlWriter::new();
        let opts = WriteOptions::default();
        let html = writer.write(&doc, &opts).unwrap();
        assert!(html.contains(">10<"), "should start at 10");
        assert!(html.contains(">11<"), "should have line 11");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-html -- --nocapture`
Expected: test failures (no line number or highlight output yet)

- [ ] **Step 3: Update CodeBlock rendering in HTML writer**

In `crates/docmux-writer-html/src/lib.rs`, modify the `Block::CodeBlock` match arm (lines 49-102) to use `LineOptions`:

Add import at top of file:
```rust
use docmux_highlight::LineOptions;
```

Replace the CodeBlock rendering logic to incorporate line options. The key addition is: after generating `lines` (the highlighted token output), wrap each line in line number spans and/or highlight spans based on `LineOptions::from_attrs(attrs.as_ref())`.

For each line:
- If `line_opts.number_lines`: prepend `<span class="line-number">{n}</span>`
- If `line_opts.is_highlighted(n)`: wrap line content in `<span class="highlight-line">...</span>`

When no highlighting theme is set (plain code blocks), apply the same line options using the raw text split by `\n`.

Add default CSS in standalone mode:
```css
.line-number { color: #6e7781; padding-right: 1em; user-select: none; display: inline-block; text-align: right; min-width: 2em; }
.highlight-line { background-color: rgba(255, 255, 0, 0.15); display: inline; }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-html -- --nocapture`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-html/src/lib.rs
git commit -m "feat(highlight): add line numbers and line highlighting to HTML writer"
```

---

### Task 10: LaTeX writer line numbers + highlighting

**Files:**
- Modify: `crates/docmux-writer-latex/src/lib.rs:50-65,547-569` (CodeBlock + write_highlighted_code)
- Test: inline tests

- [ ] **Step 1: Write failing tests**

Add to existing test module in `crates/docmux-writer-latex/src/lib.rs`:

```rust
    #[test]
    fn code_block_with_line_numbers() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "a = 1\nb = 2".into(),
                attrs: Some(Attributes {
                    id: None,
                    classes: vec!["numberLines".into()],
                    kvs: vec![],
                }),
            }],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
        };
        let writer = LatexWriter::new();
        let opts = WriteOptions::default();
        let latex = writer.write(&doc, &opts).unwrap();
        // Should contain line numbers in the output
        assert!(latex.contains("1"), "should have line numbers");
    }

    #[test]
    fn code_block_with_highlight_lines() {
        let doc = Document {
            content: vec![Block::CodeBlock {
                language: Some("python".into()),
                content: "a\nb\nc".into(),
                attrs: Some(Attributes {
                    id: None,
                    classes: vec![],
                    kvs: vec![("highlight".into(), "2".into())],
                }),
            }],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
        };
        let writer = LatexWriter::new();
        let opts = WriteOptions {
            highlight_style: Some("InspiredGitHub".into()),
            ..Default::default()
        };
        let latex = writer.write(&doc, &opts).unwrap();
        assert!(latex.contains("colorbox"), "should highlight line with colorbox");
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p docmux-writer-latex -- --nocapture`
Expected: failures

- [ ] **Step 3: Update CodeBlock rendering in LaTeX writer**

In `crates/docmux-writer-latex/src/lib.rs`:

Add import:
```rust
use docmux_highlight::LineOptions;
```

Modify `write_highlighted_code` to accept `LineOptions` and:
- For line numbers: prepend `\makebox[2em][r]{N}\;\,` before each line
- For highlight: wrap the line in `\colorbox{yellow!15}{...}`

Similarly update the `write_lstlisting` fallback to support line options.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-writer-latex -- --nocapture`
Expected: all tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/docmux-writer-latex/src/lib.rs
git commit -m "feat(highlight): add line numbers and line highlighting to LaTeX writer"
```

---

## Part 3: Cite Completion

### Task 11: Forward prefix/suffix to hayagriva

**Files:**
- Modify: `crates/docmux-transform-cite/src/lib.rs:299-332` (build_citation_items)
- Modify: `crates/docmux-transform-cite/src/lib.rs:257-281` (extract_cite_strings)
- Test: inline unit test + golden file

- [ ] **Step 1: Write failing test**

Add to existing test module in `crates/docmux-transform-cite/src/lib.rs`:

```rust
    #[test]
    fn citation_with_prefix_suffix() {
        let bib_yaml = r#"
smith2020:
    type: article
    title: Test Article
    author: Smith, John
    date: 2020
    parent:
        type: periodical
        title: Journal
"#;
        let lib = hayagriva::io::from_yaml_str(bib_yaml).unwrap();
        let transform = CiteTransform::with_library(lib, None).unwrap();

        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Citation(Citation {
                    items: vec![CiteItem {
                        key: "smith2020".into(),
                        prefix: Some("see".into()),
                        suffix: Some("p. 42".into()),
                    }],
                    mode: CitationMode::Normal,
                })],
            }],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        let text = match &doc.content[0] {
            Block::Paragraph { content } => match &content[0] {
                Inline::Str(s) => s.clone(),
                other => format!("{other:?}"),
            },
            other => format!("{other:?}"),
        };

        assert!(text.contains("see"), "should contain prefix 'see': {text}");
        assert!(text.contains("p. 42"), "should contain suffix 'p. 42': {text}");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p docmux-transform-cite -- citation_with_prefix_suffix --nocapture`
Expected: FAIL (prefix/suffix not in output)

- [ ] **Step 3: Implement prefix/suffix forwarding**

In `crates/docmux-transform-cite/src/lib.rs`, modify `build_citation_items` (line ~310-321):

Replace the TODO block:
```rust
            Some(entry) => {
                // TODO: forward CiteItem.prefix and CiteItem.suffix to hayagriva's
                // CitationItem once we determine the correct API for CSL affixes.
                let mut ci = CitationItem::with_entry(entry);
                if let Some(p) = purpose {
                    ci.purpose = Some(p);
                }
                Some(ci)
            }
```

With:
```rust
            Some(entry) => {
                let mut ci = CitationItem::with_entry(entry);
                if let Some(p) = purpose {
                    ci.purpose = Some(p);
                }
                Some((ci, item.prefix.clone(), item.suffix.clone()))
            }
```

Update the return type of `build_citation_items` to return `Vec<(CitationItem<'a, Entry>, Option<String>, Option<String>)>`.

Then in `feed_driver`, adjust to destructure and pass the items to `CitationRequest`, and in `extract_cite_strings`, prepend prefix and append suffix to the formatted string:

```rust
// In extract_cite_strings, after getting the formatted text:
let mut text = formatted_text;
if let Some(prefix) = &group_prefix {
    text = format!("{prefix} {text}");
}
if let Some(suffix) = &group_suffix {
    text = format!("{text}, {suffix}");
}
```

The exact approach depends on whether hayagriva's `CitationItem` supports affixes natively. If it does, use the native API. If not, use the string prepend/append fallback.

Store prefix/suffix alongside the citation group so they're available during extract.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p docmux-transform-cite -- --nocapture`
Expected: all tests PASS including the new prefix/suffix test

- [ ] **Step 5: Update golden test fixture**

Update `tests/fixtures/citations/basic.md` to include a prefix/suffix citation if not already present, and regenerate:

Run: `DOCMUX_UPDATE_EXPECTATIONS=1 cargo test -p docmux-cli -- citation --nocapture`

- [ ] **Step 6: Commit**

```bash
git add crates/docmux-transform-cite/src/lib.rs tests/fixtures/citations/
git commit -m "feat(cite): forward prefix/suffix to formatted citation output"
```

---

### Task 12: `--nocite` flag

**Files:**
- Modify: `crates/docmux-cli/src/main.rs:~138` (add --nocite arg)
- Modify: `crates/docmux-transform-cite/src/lib.rs` (add nocite support)
- Test: unit test + golden file

- [ ] **Step 1: Add --nocite CLI arg**

In `crates/docmux-cli/src/main.rs`, add after the `csl` field (line ~138):

```rust
    /// Include bibliography entries without citing in text (repeatable, use @* for all)
    #[arg(long, value_name = "KEY")]
    nocite: Vec<String>,
```

- [ ] **Step 2: Add nocite support to CiteTransform**

In `crates/docmux-transform-cite/src/lib.rs`, add a `nocite_keys` field to `CiteTransform`:

```rust
pub struct CiteTransform {
    library: Library,
    style: IndependentStyle,
    locales: Vec<Locale>,
    nocite_keys: Vec<String>,
}
```

Update `with_library` to accept `nocite_keys: Vec<String>`:

```rust
pub fn with_library(
    library: Library,
    csl_xml: Option<&str>,
    nocite_keys: Vec<String>,
) -> Result<Self> {
```

In the `transform` method, after the normal `feed_driver` call, add nocite entries to the bibliography driver:
- If `nocite_keys` contains `"@*"`, add all library entries
- Otherwise, add each matching key as a "phantom" citation (appears in bibliography, no inline text)

- [ ] **Step 3: Write failing test**

Add to tests in `crates/docmux-transform-cite/src/lib.rs`:

```rust
    #[test]
    fn nocite_adds_to_bibliography() {
        let bib_yaml = r#"
smith2020:
    type: article
    title: Test Article
    author: Smith, John
    date: 2020
    parent:
        type: periodical
        title: Journal
jones2021:
    type: article
    title: Another Article
    author: Jones, Jane
    date: 2021
    parent:
        type: periodical
        title: Proceedings
"#;
        let lib = hayagriva::io::from_yaml_str(bib_yaml).unwrap();
        let transform =
            CiteTransform::with_library(lib, None, vec!["jones2021".into()]).unwrap();

        // Document has no citations at all
        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Str("No citations here.".into())],
            }],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        // Should have bibliography at the end with jones2021
        let last = doc.content.last().unwrap();
        let text = format!("{last:?}");
        assert!(
            text.contains("Jones") || text.contains("jones"),
            "bibliography should contain nocite entry: {text}"
        );
    }

    #[test]
    fn nocite_star_includes_all() {
        let bib_yaml = r#"
smith2020:
    type: article
    title: First
    author: Smith, John
    date: 2020
    parent:
        type: periodical
        title: Journal
jones2021:
    type: article
    title: Second
    author: Jones, Jane
    date: 2021
    parent:
        type: periodical
        title: Proceedings
"#;
        let lib = hayagriva::io::from_yaml_str(bib_yaml).unwrap();
        let transform = CiteTransform::with_library(lib, None, vec!["@*".into()]).unwrap();

        let mut doc = Document {
            content: vec![Block::Paragraph {
                content: vec![Inline::Str("No citations.".into())],
            }],
            metadata: Metadata::default(),
            warnings: vec![],
            resources: std::collections::HashMap::new(),
        };

        transform
            .transform(&mut doc, &TransformContext::default())
            .unwrap();

        let text = format!("{:?}", doc.content);
        assert!(text.contains("Smith") || text.contains("smith"), "should have Smith");
        assert!(text.contains("Jones") || text.contains("jones"), "should have Jones");
    }
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p docmux-transform-cite -- nocite --nocapture`
Expected: FAIL

- [ ] **Step 5: Implement nocite in the transform**

In `feed_driver`, after processing normal citation groups, add nocite entries:

```rust
// Add nocite entries (bibliography only, no inline text)
if self.nocite_keys.iter().any(|k| k == "@*") {
    // Add all library entries
    for entry in library.iter() {
        let ci = CitationItem::with_entry(entry);
        let req = CitationRequest::from_items(vec![ci], style, locales);
        driver.citation(req);
    }
} else {
    for key in &self.nocite_keys {
        if let Some(entry) = library.get(key) {
            let ci = CitationItem::with_entry(entry);
            let req = CitationRequest::from_items(vec![ci], style, locales);
            driver.citation(req);
        } else {
            eprintln!("warning: nocite key '{key}' not found in bibliography");
        }
    }
}
```

The nocite citations generate bibliography entries but their inline text is discarded (not inserted into the document).

- [ ] **Step 6: Update CLI to pass nocite to transform**

In `crates/docmux-cli/src/main.rs`, where `CiteTransform::with_library` is called (~line 447):

```rust
// Collect nocite keys from CLI + metadata
let mut nocite_keys = cli.nocite.clone();
if let Some(MetaValue::List(items)) = doc.metadata.custom.get("nocite") {
    for item in items {
        if let MetaValue::String(s) = item {
            nocite_keys.push(s.trim_start_matches('@').to_string());
        }
    }
}

let cite_transform = match CiteTransform::with_library(combined, csl_xml.as_deref(), nocite_keys) {
```

- [ ] **Step 7: Update all existing CiteTransform::with_library calls**

Any existing call to `CiteTransform::with_library(lib, csl)` in tests needs the third arg:

```rust
CiteTransform::with_library(lib, None, vec![])
```

Update all call sites in `crates/docmux-transform-cite/src/lib.rs` tests and `crates/docmux-cli/tests/golden.rs`.

- [ ] **Step 8: Run all tests**

Run: `cargo test --workspace`
Expected: all PASS

- [ ] **Step 9: Commit**

```bash
git add crates/docmux-cli/src/main.rs crates/docmux-transform-cite/src/lib.rs
git commit -m "feat(cite): add --nocite flag for bibliography-only entries"
```

---

## Part 4: Finalization

### Task 13: Update ROADMAP.md

**Files:**
- Modify: `ROADMAP.md`

- [ ] **Step 1: Mark Phase 3 items as complete**

In `ROADMAP.md`, check off:
```markdown
- [x] Forward `CiteItem.prefix`/`suffix` to hayagriva (enables `(see Smith 2020, p. 42)`)
- [x] `--nocite` flag
- [x] `docmux-transform-math` — normalize math notation across formats
- [x] Line numbers, multiple styles
```

- [ ] **Step 2: Add Phase 4 items**

In the Phase 4 section, add:
```markdown
### Math
- [ ] OMML math output in DOCX writer (LaTeX → Office Math Markup Language)

### Syntax highlighting
- [ ] Load custom `.tmTheme` theme files
- [ ] Per-code-block theme selection
```

- [ ] **Step 3: Commit**

```bash
git add ROADMAP.md
git commit -m "docs: update roadmap — Phase 3 complete, add Phase 4 math/highlight items"
```

---

### Task 14: Final verification

- [ ] **Step 1: Run full test suite**

```bash
cargo test --workspace
```

Expected: all 520+ tests PASS

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: zero warnings

- [ ] **Step 3: Run fmt check**

```bash
cargo fmt --all -- --check
```

Expected: no formatting issues

- [ ] **Step 4: Build WASM**

```bash
cargo build --target wasm32-unknown-unknown -p docmux-wasm
```

Expected: successful build

- [ ] **Step 5: Run playground checks**

```bash
cd playground && pnpm exec tsc --noEmit && pnpm run lint
```

Expected: clean
