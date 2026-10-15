//! LaTeX → Typst math notation converter.
//!
//! Tokenizes a LaTeX math string and walks the token list, emitting Typst
//! math. Typst parses math differently from LaTeX in ways that break real
//! documents if the conversion is purely textual:
//!
//! - a run of letters is one identifier (`xy` is the variable `xy`), so
//!   letters are emitted as separate atoms (`x y`);
//! - `name(` starts a function call whose arguments are separated by `,` and
//!   `;`, so content placed inside generated calls escapes them, and
//!   unbalanced delimiters are escaped, inside a call or anywhere else;
//! - a sub/superscript needs a base and a single (or parenthesized) argument,
//!   so empty bases become `""` and multi-token scripts are wrapped in `(…)`.
//!
//! Unknown commands become `op("name")`, which always compiles.

use crate::tables::{
    LATEX_ENV_TO_TYPST, LATEX_TO_TYPST_COMMANDS, LATEX_TO_TYPST_FUNCTIONS, MATHBB_TO_TYPST,
};
use crate::tokenizer::{tokenize_latex, Token};

/// Convert a LaTeX math string to Typst math notation.
pub fn latex_to_typst(input: &str) -> String {
    // Half-open intervals like `(0, 1]` are unbalanced on purpose, and Typst
    // rejects an unclosed delimiter anywhere in a formula.
    let converted = balance(&convert(&tokenize_latex(input), false));
    let mut out = converted.trim();
    // A trailing `\\` would become a dangling `\` that escapes the closing `$`.
    while let Some(rest) = out.strip_suffix('\\') {
        out = rest.trim_end();
    }
    out.to_string()
}

/// Convert a token slice. `in_call` is true when the result will sit inside
/// the argument list of a generated Typst function call.
fn convert(tokens: &[Token], in_call: bool) -> String {
    let tokens = split_text(tokens);
    let mut emitter = Emitter {
        out: String::new(),
        in_call,
        script_end: None,
    };
    let mut i = 0;
    while i < tokens.len() {
        emitter.token(&tokens, &mut i);
    }
    emitter.out
}

/// Split `Text` tokens into units: one letter, one number, or one other
/// character. Whitespace is dropped (it has no meaning in LaTeX math).
fn split_text(tokens: &[Token]) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    for token in tokens {
        match token {
            Token::Text(t) => out.extend(text_units(t).into_iter().map(Token::Text)),
            other => out.push(other.clone()),
        }
    }
    out
}

pub(crate) fn text_units(t: &str) -> Vec<String> {
    let chars: Vec<char> = t.chars().collect();
    let mut units = Vec::new();
    let mut k = 0;
    while k < chars.len() {
        let start = k;
        if chars[k].is_ascii_digit() {
            while k < chars.len()
                && (chars[k].is_ascii_digit()
                    || (chars[k] == '.' && chars.get(k + 1).is_some_and(char::is_ascii_digit)))
            {
                k += 1;
            }
        } else {
            k += 1;
        }
        if !chars[start].is_whitespace() {
            units.push(chars[start..k].iter().collect());
        }
    }
    units
}

struct Emitter {
    out: String,
    in_call: bool,
    /// Output length right after a sub/superscript argument: a `(` here
    /// would turn `f_theta(x)` into a call inside the subscript.
    script_end: Option<usize>,
}

impl Emitter {
    fn token(&mut self, tokens: &[Token], i: &mut usize) {
        let token = &tokens[*i];
        *i += 1;
        match token {
            Token::Command(name) => {
                let s = command(name, tokens, i, self.in_call);
                self.atom(&s);
            }
            Token::Text(unit) => self.unit(unit),
            Token::SubScript => self.script('_', tokens, i),
            Token::SuperScript => self.script('^', tokens, i),
            Token::BraceGroup(inner) => {
                let s = convert(inner, self.in_call);
                self.atom(&s);
            }
            Token::OptionalArg(arg) => {
                let s = convert(&tokenize_latex(&format!("[{arg}]")), self.in_call);
                self.atom(&s);
            }
            Token::Environment { name, body } => {
                let s = environment(name, body);
                self.atom(&s);
            }
        }
    }

    /// Append a converted atom, inserting a space when it would otherwise
    /// fuse with the previous one (`x` `y` → `x y`, not the identifier `xy`).
    fn atom(&mut self, s: &str) {
        let Some(first) = s.chars().next() else {
            return;
        };
        let last = self.out.chars().last();
        let word_end = last.is_some_and(|c| c.is_alphanumeric() || matches!(c, '.' | '"' | ')'));
        // `log (a)/(b)`, not the call `log(a)` over `b`.
        let fuses = word_end && (first.is_alphanumeric() || first == '"' || first == '(')
            || self.script_end == Some(self.out.len()) && first == '(';
        if fuses || last.is_some_and(|c| matches!(c, '&' | '=' | '+' | '<' | '>')) {
            self.out.push(' ');
        }
        self.out.push_str(s);
    }

    /// Append a binary operator or alignment mark with spaces around it, for
    /// readable output (`a &= b + c`).
    fn spaced(&mut self, c: char) {
        let last = self.out.chars().last();
        let glued = c == '=' && last == Some('&');
        if !glued && last.is_some_and(|l| !l.is_whitespace() && !"([{".contains(l)) {
            self.out.push(' ');
        }
        self.out.push(c);
    }

    /// Append one text unit (a letter, a number or a single character).
    fn unit(&mut self, unit: &str) {
        let mut chars = unit.chars();
        let (Some(c), None) = (chars.next(), chars.next()) else {
            self.atom(unit);
            return;
        };
        if matches!(c, '(' | '[')
            && (self.script_end == Some(self.out.len()) || ends_with_name(&self.out))
        {
            self.out.push(' ');
        }
        match c {
            '\'' => self.out.push('\''),
            '/' => self.out.push_str("\\/"),
            '#' | '$' | '"' | '@' => {
                self.out.push('\\');
                self.out.push(c);
            }
            ',' | ';' => {
                if self.in_call {
                    self.out.push('\\');
                }
                self.out.push(c);
                self.out.push(' ');
            }
            '&' | '=' | '+' | '<' | '>' => self.spaced(c),
            '~' => self.atom("space"),
            c if c.is_alphanumeric() => self.atom(unit),
            _ => self.out.push(c),
        }
    }

    fn script(&mut self, op: char, tokens: &[Token], i: &mut usize) {
        let kept = self.out.trim_end().len();
        self.out.truncate(kept);
        if needs_base(&self.out) {
            self.out.push_str("\"\"");
        }
        self.out.push(op);
        self.out.push_str(&script_arg(tokens, i));
        self.script_end = Some(self.out.len());
    }
}

/// True when the output ends with a multi-letter name (`in`, `tilde.op`),
/// which a `(` right after it would turn into a function call.
fn ends_with_name(out: &str) -> bool {
    let name: String = out
        .chars()
        .rev()
        .take_while(|c| c.is_alphabetic() || *c == '.')
        .collect();
    name.chars().filter(|c| c.is_alphabetic()).count() > 1
}

/// True when a `_`/`^` here would have nothing to attach to in Typst.
fn needs_base(out: &str) -> bool {
    match out.chars().last() {
        None => true,
        Some(c) => matches!(c, '(' | '[' | '{' | '&' | ',' | ';' | '\n'),
    }
}

/// The argument of a sub/superscript: one token, grouped if needed.
fn script_arg(tokens: &[Token], i: &mut usize) -> String {
    let Some(token) = tokens.get(*i) else {
        return "\"\"".into();
    };
    *i += 1;
    let s = match token {
        Token::BraceGroup(inner) => convert(inner, false),
        Token::Text(_) => convert(std::slice::from_ref(token), false),
        Token::Command(name) => command(name, tokens, i, false),
        Token::Environment { name, body } => environment(name, body),
        Token::SubScript | Token::SuperScript | Token::OptionalArg(_) => {
            *i -= 1;
            String::new()
        }
    };
    group(&s)
}

/// Wrap a script argument in parentheses unless it is a single atom.
fn group(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        "\"\"".into()
    } else if s.chars().all(|c| c.is_alphanumeric() || c == '.') {
        s.to_string()
    } else {
        format!("({})", balance(s))
    }
}

// ─── Commands ───────────────────────────────────────────────────────────────

/// Convert a command token (already consumed) and its arguments.
fn command(name: &str, tokens: &[Token], i: &mut usize, in_call: bool) -> String {
    if name == "\\" || name == "newline" {
        if matches!(tokens.get(*i), Some(Token::OptionalArg(_))) {
            *i += 1;
        }
        return " \\\n".into();
    }
    if let Some(sym) = control_symbol(name) {
        return sym.to_string();
    }
    if let Some(sym) = LATEX_TO_TYPST_COMMANDS.get(name) {
        return (*sym).to_string();
    }
    structural(name, tokens, i)
        .or_else(|| text_command(name, tokens, i))
        .or_else(|| font(name, tokens, i))
        .or_else(|| decoration(name, tokens, i))
        .or_else(|| ignored(name, tokens, i, in_call))
        .unwrap_or_else(|| format!("op(\"{name}\")"))
}

/// Control symbols: `\{`, `\,`, `\|`, …
fn control_symbol(name: &str) -> Option<&'static str> {
    Some(match name {
        "{" => "{",
        "}" => "}",
        "|" => "||",
        "%" => "%",
        "$" => "\\$",
        "#" => "\\#",
        "&" => "\\&",
        "_" => "\\_",
        "/" => "\\/",
        "," => "thin",
        ":" | ">" => "med",
        ";" => "thick",
        " " => "space",
        "!" => "",
        _ => return None,
    })
}

/// Fractions, roots, stacking, delimiters and other commands with structure.
fn structural(name: &str, tokens: &[Token], i: &mut usize) -> Option<String> {
    Some(match name {
        "frac" | "dfrac" | "tfrac" | "cfrac" => {
            let num = call_arg(tokens, i);
            let den = call_arg(tokens, i);
            format!("({num})/({den})")
        }
        "binom" | "dbinom" | "tbinom" => {
            let n = call_arg(tokens, i);
            let k = call_arg(tokens, i);
            format!("binom({n}, {k})")
        }
        "sqrt" => sqrt(tokens, i),
        "overset" | "stackrel" => {
            let top = call_arg(tokens, i);
            format!("limits({})^({top})", call_arg(tokens, i))
        }
        "underset" => {
            let bottom = call_arg(tokens, i);
            format!("limits({})_({bottom})", call_arg(tokens, i))
        }
        "xrightarrow" | "xleftarrow" => {
            if matches!(tokens.get(*i), Some(Token::OptionalArg(_))) {
                *i += 1;
            }
            let arrow = if name == "xrightarrow" {
                "arrow.r.long"
            } else {
                "arrow.l.long"
            };
            format!("limits({arrow})^({})", call_arg(tokens, i))
        }
        "not" => negate(tokens, i),
        _ if is_sizing(name) => delimiter(tokens, i),
        _ => return None,
    })
}

/// `\left`, `\right`, `\middle`, `\big`, `\Bigl`, … Typst sizes delimiters
/// automatically, so only the delimiter itself is kept.
fn is_sizing(name: &str) -> bool {
    matches!(name, "left" | "right" | "middle")
        || ["big", "Big", "bigg", "Bigg"].iter().any(|base| {
            name.strip_prefix(base)
                .is_some_and(|rest| matches!(rest, "" | "l" | "r" | "m"))
        })
}

fn delimiter(tokens: &[Token], i: &mut usize) -> String {
    let Some(token) = tokens.get(*i) else {
        return String::new();
    };
    *i += 1;
    match token {
        Token::Text(unit) => match unit.as_str() {
            "." => "",
            "<" => "chevron.l",
            ">" => "chevron.r",
            "/" => "\\/",
            other => other,
        }
        .to_string(),
        Token::Command(name) => control_symbol(name)
            .or_else(|| LATEX_TO_TYPST_COMMANDS.get(name.as_str()).copied())
            .unwrap_or("")
            .to_string(),
        _ => {
            *i -= 1;
            String::new()
        }
    }
}

fn sqrt(tokens: &[Token], i: &mut usize) -> String {
    let index = match tokens.get(*i) {
        Some(Token::OptionalArg(n)) => {
            *i += 1;
            Some(balance(&convert(&tokenize_latex(n), true)))
        }
        _ => None,
    };
    let body = call_arg(tokens, i);
    match index {
        Some(n) => format!("root({n}, {body})"),
        None => format!("sqrt({body})"),
    }
}

/// `\not` followed by a relation: use Typst's negated symbol when there is one.
fn negate(tokens: &[Token], i: &mut usize) -> String {
    let negated = match tokens.get(*i) {
        Some(Token::Text(u)) if u == "=" => Some("eq.not"),
        Some(Token::Text(u)) if u == "<" => Some("lt.not"),
        Some(Token::Text(u)) if u == ">" => Some("gt.not"),
        Some(Token::Command(c)) => match c.as_str() {
            "in" => Some("in.not"),
            "subset" => Some("subset.not"),
            "subseteq" => Some("subset.eq.not"),
            "equiv" => Some("equiv.not"),
            "exists" => Some("exists.not"),
            "parallel" => Some("parallel.not"),
            "mid" => Some("divides.not"),
            "sim" => Some("tilde.not"),
            "approx" => Some("approx.not"),
            "le" | "leq" => Some("lt.eq.not"),
            "ge" | "geq" => Some("gt.eq.not"),
            _ => None,
        },
        _ => None,
    };
    match negated {
        Some(s) => {
            *i += 1;
            s.to_string()
        }
        None => String::new(),
    }
}

fn text_command_wrap(name: &str) -> Option<&'static str> {
    Some(match name {
        "text" | "textrm" | "textnormal" | "textup" | "textmd" | "textsc" | "mbox" | "hbox" => "",
        "textbf" => "bold",
        "textit" | "textsl" | "emph" => "italic",
        "texttt" => "mono",
        "textsf" => "sans",
        "operatorname" => "op",
        "operatorname*" => "op*",
        _ => return None,
    })
}

/// `\text{…}` and friends: literal text as a Typst string.
fn text_command(name: &str, tokens: &[Token], i: &mut usize) -> Option<String> {
    let wrap = text_command_wrap(name)?;
    let lit = string_lit(&text_of(&next_arg(tokens, i)));
    Some(match wrap {
        "" => lit,
        "op*" => format!("op({lit}, limits: #true)"),
        func => format!("{func}({lit})"),
    })
}

fn font_func(name: &str) -> Option<&'static str> {
    Some(match name {
        "mathrm" | "mathup" => "upright",
        "mathbf" | "boldsymbol" | "bm" | "pmb" => "bold",
        "mathit" | "mathnormal" => "italic",
        "mathsf" => "sans",
        "mathtt" => "mono",
        "mathcal" | "mathscr" => "cal",
        "mathfrak" => "frak",
        "mathbb" | "mathbbm" | "mathds" => "bb",
        _ => return None,
    })
}

/// `\mathbf{x}`, `\mathrm{KL}`, `\mathbb{R}`, …
fn font(name: &str, tokens: &[Token], i: &mut usize) -> Option<String> {
    let func = font_func(name)?;
    let arg = next_arg(tokens, i);
    let raw = text_of(&arg);
    let word = raw.trim();
    if func == "bb" {
        if let Some(s) = MATHBB_TO_TYPST.get(word) {
            return Some((*s).to_string());
        }
    }
    let is_word = word.chars().count() > 1 && word.chars().all(char::is_alphabetic);
    if is_word && matches!(func, "upright" | "bold" | "sans" | "mono") {
        // A multi-letter word (`\mathrm{KL}`) is one word, not a product.
        let lit = string_lit(word);
        return Some(if func == "upright" {
            lit
        } else {
            format!("{func}({lit})")
        });
    }
    Some(format!("{func}({})", balance(&convert(&arg, true))))
}

/// Accents and stretchy decorations: `\hat{x}` → `hat(x)`.
fn decoration(name: &str, tokens: &[Token], i: &mut usize) -> Option<String> {
    let func = match name {
        "overrightarrow" => "arrow",
        "mathring" => "circle",
        "cancel" | "bcancel" | "xcancel" | "sout" => "cancel",
        // `hide` is not a math function: it takes content, so the phantom
        // gets its own (inline) equation.
        "phantom" | "hphantom" | "vphantom" => {
            let inner = convert(&next_arg(tokens, i), false);
            let inner = inner.trim();
            return Some(if inner.is_empty() {
                String::new()
            } else {
                format!("#hide(${inner}$)")
            });
        }
        _ => LATEX_TO_TYPST_FUNCTIONS.get(name).copied()?,
    };
    Some(format!("{func}({})", call_arg(tokens, i)))
}

/// Commands with no visible Typst output, or that only wrap their argument.
fn ignored(name: &str, tokens: &[Token], i: &mut usize, in_call: bool) -> Option<String> {
    match name {
        "nonumber" | "notag" | "displaystyle" | "textstyle" | "scriptstyle"
        | "scriptscriptstyle" | "limits" | "nolimits" | "allowbreak" | "nobreak" | "qedhere"
        | "relax" | "strut" | "mathstrut" | "protect" | "hline" | "centering" | "small"
        | "footnotesize" | "scriptsize" | "normalsize" | "large" | "rm" | "tt" | "bf" | "it"
        | "sf" | "cal" => Some(String::new()),
        "addtocounter" | "setcounter" => {
            next_arg(tokens, i);
            next_arg(tokens, i);
            Some(String::new())
        }
        "label" | "tag" | "hspace" | "vspace" | "color" | "cline" | "mspace" | "hskip" | "kern"
        | "mkern" => {
            next_arg(tokens, i);
            Some(String::new())
        }
        "textcolor" => {
            next_arg(tokens, i);
            Some(convert(&next_arg(tokens, i), in_call))
        }
        "boxed" | "mathop" | "mathbin" | "mathrel" | "mathord" | "mathopen" | "mathclose"
        | "ensuremath" => Some(convert(&next_arg(tokens, i), in_call)),
        _ => None,
    }
}

// ─── Arguments ──────────────────────────────────────────────────────────────

/// The next argument: a brace group's tokens, or a single token
/// (`\frac12`, `\hat x`).
fn next_arg(tokens: &[Token], i: &mut usize) -> Vec<Token> {
    match tokens.get(*i) {
        Some(Token::BraceGroup(inner)) => {
            *i += 1;
            inner.clone()
        }
        Some(Token::Command(name)) => {
            // `\hat\mathbf{x}`: the argument is the command with its own
            // arguments, not the bare command.
            let end = (*i + 1 + arity(name)).min(tokens.len());
            let arg = tokens[*i..end].to_vec();
            *i = end;
            arg
        }
        Some(t @ Token::Text(_)) => {
            *i += 1;
            vec![t.clone()]
        }
        _ => Vec::new(),
    }
}

/// Number of brace arguments a command takes, for commands used as an
/// undelimited argument of another command.
fn arity(name: &str) -> usize {
    match name {
        "frac" | "dfrac" | "tfrac" | "cfrac" | "binom" | "overset" | "underset" | "stackrel"
        | "textcolor" => 2,
        "sqrt" => 1,
        _ if LATEX_TO_TYPST_FUNCTIONS.contains_key(name) => 1,
        _ if text_command_wrap(name).is_some() || font_func(name).is_some() => 1,
        _ => 0,
    }
}

/// The next argument converted for use inside a generated function call.
fn call_arg(tokens: &[Token], i: &mut usize) -> String {
    balance(&convert(&next_arg(tokens, i), true))
}

/// Plain text content of a token list, for `\text{…}` and font words.
fn text_of(tokens: &[Token]) -> String {
    let mut out = String::new();
    for token in tokens {
        match token {
            Token::Text(s) => out.push_str(s),
            Token::BraceGroup(inner) => out.push_str(&text_of(inner)),
            Token::Command(c) if !c.chars().any(char::is_alphabetic) => {
                out.push_str(match c.as_str() {
                    "\\" | "," | ";" | ":" | ">" | " " => " ",
                    "!" => "",
                    other => other,
                })
            }
            Token::Command(_) => {}
            Token::SubScript => out.push('_'),
            Token::SuperScript => out.push('^'),
            Token::OptionalArg(a) => {
                out.push('[');
                out.push_str(a);
                out.push(']');
            }
            Token::Environment { body, .. } => out.push_str(body),
        }
    }
    out
}

/// A Typst string literal. Inline `$…$` inside `\text` is flattened.
fn string_lit(s: &str) -> String {
    let escaped = s
        .replace('$', "")
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\"{escaped}\"")
}

/// Escape delimiters that have no partner, so they cannot close (or leave
/// open) a generated function call: `[0, 1)` → `\[0, 1\)`.
fn balance(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut open: Vec<usize> = Vec::new();
    let mut unmatched: Vec<usize> = Vec::new();
    let mut in_string = false;
    let mut k = 0;
    while k < chars.len() {
        match chars[k] {
            '\\' => k += 1,
            '"' => in_string = !in_string,
            '(' | '[' if !in_string => open.push(k),
            c @ (')' | ']') if !in_string => {
                let partner = if c == ')' { '(' } else { '[' };
                match open.last() {
                    Some(&o) if chars[o] == partner => {
                        open.pop();
                    }
                    _ => unmatched.push(k),
                }
            }
            _ => {}
        }
        k += 1;
    }
    unmatched.extend(open);
    let mut out = String::with_capacity(s.len() + unmatched.len());
    for (k, c) in chars.iter().enumerate() {
        if unmatched.contains(&k) {
            out.push('\\');
        }
        out.push(*c);
    }
    out
}

// ─── Environments ───────────────────────────────────────────────────────────

/// Convert a `\begin{env}…\end{env}` environment to Typst.
fn environment(name: &str, body: &str) -> String {
    let base = name.trim_end_matches('*');
    match base {
        "cases" | "dcases" | "rcases" => cases(body),
        "array" | "tabular" => matrix("#none", strip_leading_group(body)),
        "alignat" | "alignedat" => convert(&tokenize_latex(strip_leading_group(body)), false),
        _ => match LATEX_ENV_TO_TYPST.get(base) {
            Some(("mat", delim)) => matrix(if delim.is_empty() { "#none" } else { delim }, body),
            // align, aligned, gather, split, equation, multline, eqnarray, unknown:
            // Typst handles `&` alignment and `\` line breaks natively.
            _ => convert(&tokenize_latex(body), false),
        },
    }
}

/// Matrix-like environments: rows split by `\\`, cells by `&`.
fn matrix(delim: &str, body: &str) -> String {
    let rows: Vec<String> = split_top_level(body, Separator::Row)
        .iter()
        .filter(|row| !row.trim().is_empty())
        .map(|row| {
            split_top_level(row, Separator::Cell)
                .iter()
                .map(|cell| matrix_cell(cell))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect();
    format!("mat(delim: {delim}, {})", rows.join("; "))
}

fn matrix_cell(cell: &str) -> String {
    let converted = balance(&convert(&tokenize_latex(cell), true));
    match converted.trim() {
        "" => "\"\"".to_string(),
        c => c.to_string(),
    }
}

/// `cases`: rows separated by `,`; Typst keeps `&` alignment inside each.
fn cases(body: &str) -> String {
    let rows: Vec<String> = split_top_level(body, Separator::Row)
        .iter()
        .filter(|row| !row.trim().is_empty())
        .map(|row| {
            balance(&convert(&tokenize_latex(row), true))
                .trim()
                .to_string()
        })
        .collect();
    format!("cases({})", rows.join(", "))
}

/// Drop a leading `{…}` group (the column spec of `array`, the count of
/// `alignat`).
pub(crate) fn strip_leading_group(body: &str) -> &str {
    let trimmed = body.trim_start();
    if !trimmed.starts_with('{') {
        return body;
    }
    let mut depth = 0usize;
    for (k, c) in trimmed.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return &trimmed[k + 1..];
                }
            }
            _ => {}
        }
    }
    body
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Separator {
    /// `\\` (optionally followed by `[length]`)
    Row,
    /// `&`
    Cell,
}

/// Split on `\\` or `&` outside braces and nested environments.
pub(crate) fn split_top_level(body: &str, sep: Separator) -> Vec<String> {
    let chars: Vec<char> = body.chars().collect();
    let mut parts = vec![String::new()];
    let mut depth = 0i32;
    let mut k = 0;
    while k < chars.len() {
        depth += env_depth_change(&chars[k..]);
        let c = chars[k];
        if depth == 0 && is_separator(&chars, k, sep) {
            k = if sep == Separator::Row {
                skip_row_spacing(&chars, k + 2)
            } else {
                k + 1
            };
            parts.push(String::new());
            continue;
        }
        let Some(last) = parts.last_mut() else {
            break;
        };
        last.push(c);
        if c == '\\' {
            if let Some(&next) = chars.get(k + 1) {
                last.push(next);
                k += 1;
            }
        }
        match c {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        k += 1;
    }
    parts
}

fn env_depth_change(rest: &[char]) -> i32 {
    let starts = |s: &str| s.chars().zip(rest).all(|(a, &b)| a == b) && rest.len() >= s.len();
    if starts("\\begin{") {
        1
    } else if starts("\\end{") {
        -1
    } else {
        0
    }
}

fn is_separator(chars: &[char], k: usize, sep: Separator) -> bool {
    match sep {
        Separator::Row => chars[k] == '\\' && chars.get(k + 1) == Some(&'\\'),
        Separator::Cell => chars[k] == '&',
    }
}

/// Skip an optional `[2pt]` after a `\\` row break.
fn skip_row_spacing(chars: &[char], k: usize) -> usize {
    let mut j = k;
    while chars.get(j).is_some_and(|c| c.is_whitespace()) {
        j += 1;
    }
    if chars.get(j) != Some(&'[') {
        return k;
    }
    match chars[j..].iter().position(|&c| c == ']') {
        Some(end) => j + end + 1,
        None => k,
    }
}

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
        assert_eq!(
            latex_to_typst(r"\epsilon \varepsilon"),
            "epsilon.alt epsilon"
        );
    }

    #[test]
    fn fraction() {
        assert_eq!(latex_to_typst(r"\frac{a}{b}"), "(a)/(b)");
        assert_eq!(latex_to_typst(r"\frac{\frac{a}{b}}{c}"), "((a)/(b))/(c)");
    }

    #[test]
    fn sqrt_basic_and_nth() {
        assert_eq!(latex_to_typst(r"\sqrt{x}"), "sqrt(x)");
        assert_eq!(latex_to_typst(r"\sqrt[3]{x}"), "root(3, x)");
    }

    #[test]
    fn subscript_superscript() {
        assert_eq!(latex_to_typst("x^2_i"), "x^2_i");
        assert_eq!(latex_to_typst("x_{ij}"), "x_(i j)");
        assert_eq!(latex_to_typst("e^{-1}"), "e^(-1)");
        assert_eq!(latex_to_typst(r"\sum_{i=1}^{n} x_i"), "sum_(i = 1)^n x_i");
    }

    #[test]
    fn empty_script_base() {
        assert_eq!(latex_to_typst(r"^\dagger"), "\"\"^dagger");
        assert_eq!(latex_to_typst(r"{}^{*}"), "\"\"^(*)");
    }

    #[test]
    fn letters_are_separate_atoms() {
        assert_eq!(latex_to_typst("xy + 2ab"), "x y + 2 a b");
        assert_eq!(latex_to_typst(r"256\times256"), "256 times 256");
        assert_eq!(latex_to_typst(r"\alpha\beta"), "alpha beta");
    }

    #[test]
    fn parenthesis_after_script_is_not_a_call() {
        assert_eq!(latex_to_typst(r"f_\theta(x)"), "f_theta (x)");
        assert_eq!(latex_to_typst(r"\log\frac{a}{b}"), "log (a)/(b)");
        assert_eq!(latex_to_typst(r"x_{i}(t)"), "x_i (t)");
    }

    #[test]
    fn operator() {
        assert_eq!(latex_to_typst(r"\int_0^1"), "integral_0^1");
    }

    #[test]
    fn decoration() {
        assert_eq!(latex_to_typst(r"\hat\mathbf{x}_0"), "hat(bold(x))_0");
        assert_eq!(latex_to_typst(r"\frac\alpha{2}"), "(alpha)/(2)");
        assert_eq!(latex_to_typst(r"\hat{x}"), "hat(x)");
        assert_eq!(latex_to_typst(r"\bar{x}"), "overline(x)");
        assert_eq!(latex_to_typst(r"\hat x"), "hat(x)");
    }

    #[test]
    fn fonts() {
        assert_eq!(latex_to_typst(r"\mathbb{R}"), "RR");
        assert_eq!(latex_to_typst(r"\mathbb{E}"), "bb(E)");
        assert_eq!(latex_to_typst(r"\mathrm{KL}"), "\"KL\"");
        assert_eq!(latex_to_typst(r"\mathbf{x}"), "bold(x)");
        assert_eq!(latex_to_typst(r"\mathbf{ReLU}"), "bold(\"ReLU\")");
        assert_eq!(latex_to_typst(r"\mathcal{N}(0, I)"), "cal(N)(0, I)");
    }

    #[test]
    fn text_and_operatorname() {
        assert_eq!(latex_to_typst(r"x \text{ if } y"), "x \" if \" y");
        assert_eq!(
            latex_to_typst(r"\operatorname{softmax}(x)"),
            "op(\"softmax\")(x)"
        );
        assert_eq!(
            latex_to_typst(r"\operatorname*{arg\,max}_x"),
            "op(\"arg max\", limits: #true)_x"
        );
    }

    #[test]
    fn sizing_delimiters_are_dropped() {
        assert_eq!(latex_to_typst(r"\left( \frac{1}{2} \right)"), "((1)/(2))");
        assert_eq!(latex_to_typst(r"\Bigg[ x \Bigg]"), "[x]");
        assert_eq!(latex_to_typst(r"\left\{ x \right."), "{x");
        assert_eq!(
            latex_to_typst(r"\left\langle x \right\rangle"),
            "chevron.l x chevron.r"
        );
    }

    #[test]
    fn control_symbols() {
        assert_eq!(latex_to_typst(r"a\,b\;c\!d"), "a thin b thick c d");
        assert_eq!(latex_to_typst(r"\{a\}"), "{a}");
        assert_eq!(latex_to_typst(r"\|x\|"), "||x||");
        assert_eq!(latex_to_typst(r"50\%"), "50%");
    }

    #[test]
    fn commas_inside_calls_are_escaped() {
        assert_eq!(latex_to_typst(r"\hat{a, b}"), r"hat(a\, b)");
        assert_eq!(latex_to_typst(r"f(a, b)"), "f(a, b)");
    }

    #[test]
    fn unbalanced_delimiters_are_escaped() {
        assert_eq!(latex_to_typst(r"\sqrt{[0, 1)}"), r"sqrt(\[0\, 1\))");
        assert_eq!(latex_to_typst(r"[0, 1)"), r"\[0, 1\)");
        // After a symbol, `(` would open call arguments that never close.
        assert_eq!(latex_to_typst(r"\alpha \in (0, 1]"), r"alpha in \(0, 1\]");
    }

    #[test]
    fn a_paren_after_a_symbol_name_is_not_a_call() {
        assert_eq!(
            latex_to_typst(r"(u, i) \sim (v, j)"),
            "(u, i) tilde.op (v, j)"
        );
        assert_eq!(latex_to_typst(r"\sin(x)"), "sin (x)");
    }

    #[test]
    fn phantom_hides_an_inline_equation() {
        assert_eq!(latex_to_typst(r"a \phantom{b} c"), "a#hide($b$) c");
        assert_eq!(latex_to_typst(r"\phantom{}"), "");
        assert_eq!(latex_to_typst(r"f(a, b)"), "f(a, b)");
    }

    #[test]
    fn slash_is_literal() {
        assert_eq!(latex_to_typst("a/b"), r"a\/b");
    }

    #[test]
    fn unknown_command_compiles() {
        assert_eq!(latex_to_typst(r"\mycommand x"), "op(\"mycommand\") x");
    }

    #[test]
    fn spacing() {
        assert_eq!(latex_to_typst(r"\quad"), "quad");
    }

    #[test]
    fn line_breaks() {
        assert_eq!(latex_to_typst("a &= b \\\\\nc &= d"), "a &= b \\\nc &= d");
        assert_eq!(latex_to_typst(r"a = b \\"), "a = b");
    }

    #[test]
    fn negations() {
        assert_eq!(latex_to_typst(r"a \not= b"), "a eq.not b");
        assert_eq!(latex_to_typst(r"x \not\in A"), "x in.not A");
    }

    #[test]
    fn environment_pmatrix() {
        assert_eq!(
            latex_to_typst(r"\begin{pmatrix}a & b \\ c & d\end{pmatrix}"),
            "mat(delim: \"(\", a, b; c, d)"
        );
    }

    #[test]
    fn environment_array_drops_column_spec() {
        assert_eq!(
            latex_to_typst(r"\begin{array}{cc}a & b \\ c & \end{array}"),
            "mat(delim: #none, a, b; c, \"\")"
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
    fn environment_aligned_is_native() {
        assert_eq!(
            latex_to_typst(r"\begin{aligned}a &= b \\ c &= d\end{aligned}"),
            "a &= b \\\nc &= d"
        );
    }

    #[test]
    fn complex_expression() {
        assert_eq!(
            latex_to_typst(r"\frac{\partial f}{\partial x}"),
            "(partial f)/(partial x)"
        );
    }
}
