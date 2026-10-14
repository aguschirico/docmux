//! LaTeX → OMML (Office Math Markup Language), the native math of Word.
//!
//! OMML is a tree: fractions, scripts, radicals and n-ary operators are
//! elements with their arguments as children (`<m:f><m:num>…</m:num>…`).
//! The converter walks the token list and builds a list of [`Atom`]s, so a
//! sub/superscript can take the previous atom as its base, and an n-ary
//! operator (`\sum`, `\int`) can take the atoms that follow as its body, up
//! to the next binary operator or relation.
//!
//! Inside an equation array (`m:eqArr`), a literal `&` is an alignment
//! point (ECMA-376, 22.1.2.34), so `align` rows keep their `&`.

use crate::latex_to_typst::{split_top_level, strip_leading_group, text_units, Separator};
use crate::tables::LATEX_TO_UNICODE;
use crate::tokenizer::{tokenize_latex, Token};

/// Inline math: an `m:oMath` element, placed directly in a `w:p`.
pub fn latex_to_omml_inline(input: &str) -> String {
    format!("<m:oMath>{}</m:oMath>", body(input, false))
}

/// Display math: an `m:oMathPara`, which is the whole content of a `w:p`.
pub fn latex_to_omml_display(input: &str) -> String {
    format!(
        "<m:oMathPara><m:oMath>{}</m:oMath></m:oMathPara>",
        body(input, true)
    )
}

/// Several rows (`a \\ b`) become an equation array; one row is converted as is.
fn body(input: &str, display: bool) -> String {
    let ctx = Ctx {
        style: Style::Italic,
        display,
        in_array: false,
    };
    rows(input, ctx)
}

fn rows(input: &str, ctx: Ctx) -> String {
    let rows: Vec<String> = split_top_level(input, Separator::Row)
        .into_iter()
        .filter(|row| !row.trim().is_empty())
        .collect();
    match rows.as_slice() {
        [] => String::new(),
        [only] => convert(&tokenize_latex(only), ctx),
        _ => eq_array(&rows, ctx),
    }
}

fn eq_array(rows: &[String], ctx: Ctx) -> String {
    let ctx = Ctx {
        in_array: true,
        ..ctx
    };
    let mut out = String::from("<m:eqArr>");
    for row in rows {
        out.push_str(&format!(
            "<m:e>{}</m:e>",
            convert(&tokenize_latex(row), ctx)
        ));
    }
    out.push_str("</m:eqArr>");
    out
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Style {
    Italic,
    Upright,
    Bold,
    BoldItalic,
    DoubleStruck,
    Script,
    Fraktur,
    Sans,
    Mono,
    /// `\text{…}`: ordinary text, spaces kept.
    Text,
}

#[derive(Debug, Clone, Copy)]
struct Ctx {
    style: Style,
    display: bool,
    /// Inside `m:eqArr`, where `&` is an alignment point.
    in_array: bool,
}

/// One element of the output, before n-ary bodies and function arguments
/// are attached.
enum Atom {
    Xml {
        xml: String,
        /// A binary operator, relation, comma or alignment mark: ends the
        /// body of a preceding `\sum` or `\int`.
        breaks: bool,
    },
    Nary {
        chr: &'static str,
        under_over: bool,
        sub: Option<String>,
        sup: Option<String>,
    },
    /// `\log`, `\max`, `\operatorname{…}`: an upright name applied to the
    /// next atom.
    Func {
        name: String,
        limits: bool,
        sub: Option<String>,
        sup: Option<String>,
    },
}

impl Atom {
    fn xml(xml: String) -> Atom {
        Atom::Xml { xml, breaks: false }
    }
}

fn convert(tokens: &[Token], ctx: Ctx) -> String {
    render(atoms(tokens, ctx))
}

fn atoms(tokens: &[Token], ctx: Ctx) -> Vec<Atom> {
    let tokens = split_text(tokens);
    let mut out: Vec<Atom> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        i += 1;
        match token {
            Token::Text(unit) => out.push(text_unit(unit, ctx)),
            Token::Command(name) if name == "left" => out.push(fenced(&tokens, &mut i, ctx)),
            Token::Command(name) => {
                if let Some(atom) = command(name, &tokens, &mut i, ctx) {
                    out.push(atom);
                }
            }
            Token::BraceGroup(inner) => out.push(Atom::xml(convert(inner, ctx))),
            Token::SubScript | Token::SuperScript => {
                let first = script_arg(&tokens, &mut i, ctx);
                // `x_i^2` and `x^2_i` are one element.
                let other = match (token, tokens.get(i)) {
                    (Token::SubScript, Some(Token::SuperScript))
                    | (Token::SuperScript, Some(Token::SubScript)) => {
                        i += 1;
                        Some(script_arg(&tokens, &mut i, ctx))
                    }
                    _ => None,
                };
                let (sub, sup) = if matches!(token, Token::SubScript) {
                    (Some(first), other)
                } else {
                    (other, Some(first))
                };
                attach_scripts(&mut out, sub, sup);
            }
            Token::OptionalArg(arg) => out.push(Atom::xml(run(&format!("[{arg}]"), ctx.style))),
            Token::Environment { name, body } => out.push(Atom::xml(environment(name, body, ctx))),
        }
    }
    out
}

/// Split text into letters, numbers and single characters, like the Typst
/// emitter. A number right after `_`, `^` or a command that takes arguments
/// gives up its first digits as those arguments: `x^10` is `x^1 0` and
/// `\frac12` is `\frac{1}{2}` in LaTeX.
fn split_text(tokens: &[Token]) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    for token in tokens {
        let Token::Text(t) = token else {
            out.push(token.clone());
            continue;
        };
        let wanted = match out.last() {
            Some(Token::SubScript | Token::SuperScript) => 1,
            Some(Token::Command(name)) => arity(name),
            _ => 0,
        };
        for (k, unit) in text_units(t).into_iter().enumerate() {
            let digits = unit.chars().all(|c| c.is_ascii_digit());
            if k == 0 && wanted > 0 && digits && unit.len() > 1 {
                let (head, rest) = unit.split_at(wanted.min(unit.len()));
                out.extend(head.chars().map(|c| Token::Text(c.to_string())));
                if !rest.is_empty() {
                    out.push(Token::Text(rest.to_string()));
                }
            } else {
                out.push(Token::Text(unit));
            }
        }
    }
    out
}

fn text_unit(unit: &str, ctx: Ctx) -> Atom {
    let breaks = matches!(unit, "+" | "-" | "=" | "<" | ">" | "," | ";" | "&");
    let xml = match unit {
        "&" if ctx.in_array => run("&", Style::Italic),
        "&" => String::new(),
        "'" => run("\u{2032}", ctx.style),
        "~" => run("\u{00A0}", ctx.style),
        "-" => run("\u{2212}", ctx.style),
        "*" => run("\u{2217}", ctx.style),
        _ => run(unit, ctx.style),
    };
    Atom::Xml { xml, breaks }
}

/// Attach sub/superscripts to the last atom, or to an empty base.
fn attach_scripts(out: &mut Vec<Atom>, sub: Option<String>, sup: Option<String>) {
    match out.last_mut() {
        Some(Atom::Nary { sub: s, sup: p, .. }) | Some(Atom::Func { sub: s, sup: p, .. }) => {
            if sub.is_some() {
                *s = sub;
            }
            if sup.is_some() {
                *p = sup;
            }
        }
        _ => {
            let base = match out.pop() {
                Some(Atom::Xml { xml, .. }) => xml,
                Some(other) => render(vec![other]),
                None => String::new(),
            };
            out.push(Atom::xml(scripted(&base, sub.as_deref(), sup.as_deref())));
        }
    }
}

fn scripted(base: &str, sub: Option<&str>, sup: Option<&str>) -> String {
    match (sub, sup) {
        (Some(b), Some(p)) => {
            format!("<m:sSubSup><m:e>{base}</m:e><m:sub>{b}</m:sub><m:sup>{p}</m:sup></m:sSubSup>")
        }
        (Some(b), None) => format!("<m:sSub><m:e>{base}</m:e><m:sub>{b}</m:sub></m:sSub>"),
        (None, Some(p)) => format!("<m:sSup><m:e>{base}</m:e><m:sup>{p}</m:sup></m:sSup>"),
        (None, None) => base.to_string(),
    }
}

/// Turn the atom list into XML: n-ary operators take the following atoms
/// up to the next breaking atom as their body, functions take the next atom.
fn render(atoms: Vec<Atom>) -> String {
    let mut out = String::new();
    let mut atoms = atoms.into_iter().peekable();
    while let Some(atom) = atoms.next() {
        match atom {
            Atom::Xml { xml, .. } => out.push_str(&xml),
            Atom::Nary {
                chr,
                under_over,
                sub,
                sup,
            } => {
                let mut body = Vec::new();
                while let Some(next) = atoms.peek() {
                    if matches!(next, Atom::Xml { breaks: true, .. }) {
                        break;
                    }
                    if let Some(next) = atoms.next() {
                        body.push(next);
                    }
                }
                let loc = if under_over { "undOvr" } else { "subSup" };
                let mut props = format!("<m:chr m:val=\"{chr}\"/><m:limLoc m:val=\"{loc}\"/>");
                if sub.is_none() {
                    props.push_str("<m:subHide m:val=\"1\"/>");
                }
                if sup.is_none() {
                    props.push_str("<m:supHide m:val=\"1\"/>");
                }
                out.push_str(&format!(
                    "<m:nary><m:naryPr>{props}</m:naryPr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:nary>",
                    sub.unwrap_or_default(),
                    sup.unwrap_or_default(),
                    render(body)
                ));
            }
            Atom::Func {
                name,
                limits,
                sub,
                sup,
            } => {
                let name_xml = run(&name, Style::Upright);
                let fname = if limits {
                    let mut x = name_xml;
                    if let Some(b) = sub {
                        x = format!("<m:limLow><m:e>{x}</m:e><m:lim>{b}</m:lim></m:limLow>");
                    }
                    if let Some(p) = sup {
                        x = format!("<m:limUpp><m:e>{x}</m:e><m:lim>{p}</m:lim></m:limUpp>");
                    }
                    x
                } else {
                    scripted(&name_xml, sub.as_deref(), sup.as_deref())
                };
                match atoms.peek() {
                    Some(Atom::Xml { breaks: false, .. }) | Some(Atom::Func { .. }) => {
                        let arg = atoms.next().map(|a| render(vec![a])).unwrap_or_default();
                        out.push_str(&format!(
                            "<m:func><m:fName>{fname}</m:fName><m:e>{arg}</m:e></m:func>"
                        ));
                    }
                    _ => out.push_str(&fname),
                }
            }
        }
    }
    out
}

/// The argument of a sub/superscript: one token or a brace group.
fn script_arg(tokens: &[Token], i: &mut usize, ctx: Ctx) -> String {
    let arg = next_arg(tokens, i);
    convert(&arg, ctx)
}

// ─── Runs ───────────────────────────────────────────────────────────────────

fn run(text: &str, style: Style) -> String {
    if text.is_empty() {
        return String::new();
    }
    let props = match style {
        Style::Italic => "",
        Style::Upright => "<m:rPr><m:sty m:val=\"p\"/></m:rPr>",
        Style::Bold => "<m:rPr><m:sty m:val=\"b\"/></m:rPr>",
        Style::BoldItalic => "<m:rPr><m:sty m:val=\"bi\"/></m:rPr>",
        Style::DoubleStruck => {
            "<m:rPr><m:scr m:val=\"double-struck\"/><m:sty m:val=\"p\"/></m:rPr>"
        }
        Style::Script => "<m:rPr><m:scr m:val=\"script\"/><m:sty m:val=\"p\"/></m:rPr>",
        Style::Fraktur => "<m:rPr><m:scr m:val=\"fraktur\"/><m:sty m:val=\"p\"/></m:rPr>",
        Style::Sans => "<m:rPr><m:scr m:val=\"sans-serif\"/><m:sty m:val=\"p\"/></m:rPr>",
        Style::Mono => "<m:rPr><m:scr m:val=\"monospace\"/><m:sty m:val=\"p\"/></m:rPr>",
        Style::Text => "<m:rPr><m:nor/></m:rPr>",
    };
    format!(
        "<m:r>{props}<m:t xml:space=\"preserve\">{}</m:t></m:r>",
        escape(text)
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ─── Commands ───────────────────────────────────────────────────────────────

fn command(name: &str, tokens: &[Token], i: &mut usize, ctx: Ctx) -> Option<Atom> {
    if let Some(sym) = spacing(name) {
        return Some(Atom::xml(run(sym, Style::Text)));
    }
    if let Some((chr, under_over)) = nary(name) {
        return Some(Atom::Nary {
            chr,
            under_over: under_over && ctx.display,
            sub: None,
            sup: None,
        });
    }
    if let Some((limits, label)) = function(name) {
        return Some(Atom::Func {
            name: label.to_string(),
            limits,
            sub: None,
            sup: None,
        });
    }
    if let Some(sym) = symbol(name) {
        return Some(Atom::Xml {
            xml: run(sym, ctx.style),
            breaks: is_relation(name),
        });
    }
    if let Some(style) = font(name) {
        let arg = next_arg(tokens, i);
        return Some(Atom::xml(convert(&arg, Ctx { style, ..ctx })));
    }
    if let Some(xml) = structural(name, tokens, i, ctx) {
        return Some(Atom::xml(xml));
    }
    if let Some(atom) = text_command(name, tokens, i) {
        return Some(atom);
    }
    if let Some(xml) = decoration(name, tokens, i, ctx) {
        return Some(Atom::xml(xml));
    }
    if let Some(xml) = ignored(name, tokens, i, ctx) {
        return (!xml.is_empty()).then(|| Atom::xml(xml));
    }
    // Unknown command: its name, upright, rather than raw LaTeX.
    Some(Atom::xml(run(name, Style::Upright)))
}

fn spacing(name: &str) -> Option<&'static str> {
    Some(match name {
        "," | "thinspace" => "\u{2009}",
        ":" | ">" | "medspace" => "\u{205F}",
        ";" | "thickspace" => "\u{2004}",
        " " => " ",
        "quad" => "\u{2003}",
        "qquad" => "\u{2003}\u{2003}",
        "!" | "negthinspace" => "",
        _ => return None,
    })
}

/// Large operators: (character, limits above and below in display math).
fn nary(name: &str) -> Option<(&'static str, bool)> {
    Some(match name {
        "sum" => ("\u{2211}", true),
        "prod" => ("\u{220F}", true),
        "coprod" => ("\u{2210}", true),
        "bigcup" => ("\u{22C3}", true),
        "bigcap" => ("\u{22C2}", true),
        "bigvee" => ("\u{22C1}", true),
        "bigwedge" => ("\u{22C0}", true),
        "bigoplus" => ("\u{2A01}", true),
        "bigotimes" => ("\u{2A02}", true),
        "bigodot" => ("\u{2A00}", true),
        "biguplus" => ("\u{2A04}", true),
        "bigsqcup" => ("\u{2A06}", true),
        "int" => ("\u{222B}", false),
        "iint" => ("\u{222C}", false),
        "iiint" => ("\u{222D}", false),
        "oint" => ("\u{222E}", false),
        _ => return None,
    })
}

/// Named functions: (limits go under the name, displayed name).
fn function(name: &str) -> Option<(bool, &'static str)> {
    Some(match name {
        "lim" => (true, "lim"),
        "liminf" => (true, "lim inf"),
        "limsup" => (true, "lim sup"),
        "max" => (true, "max"),
        "min" => (true, "min"),
        "sup" => (true, "sup"),
        "inf" => (true, "inf"),
        "det" => (true, "det"),
        "gcd" => (true, "gcd"),
        "Pr" => (true, "Pr"),
        "argmax" => (true, "arg max"),
        "argmin" => (true, "arg min"),
        "log" => (false, "log"),
        "ln" => (false, "ln"),
        "lg" => (false, "lg"),
        "exp" => (false, "exp"),
        "sin" => (false, "sin"),
        "cos" => (false, "cos"),
        "tan" => (false, "tan"),
        "sec" => (false, "sec"),
        "csc" => (false, "csc"),
        "cot" => (false, "cot"),
        "arcsin" => (false, "arcsin"),
        "arccos" => (false, "arccos"),
        "arctan" => (false, "arctan"),
        "sinh" => (false, "sinh"),
        "cosh" => (false, "cosh"),
        "tanh" => (false, "tanh"),
        "coth" => (false, "coth"),
        "dim" => (false, "dim"),
        "ker" => (false, "ker"),
        "hom" => (false, "hom"),
        "arg" => (false, "arg"),
        "deg" => (false, "deg"),
        "bmod" | "mod" => (false, "mod"),
        _ => return None,
    })
}

fn symbol(name: &str) -> Option<&'static str> {
    if let Some(s) = LATEX_TO_UNICODE.get(name) {
        return Some(s);
    }
    Some(match name {
        "{" | "lbrace" => "{",
        "}" | "rbrace" => "}",
        "|" | "Vert" | "lVert" | "rVert" => "\u{2016}",
        "vert" | "lvert" | "rvert" | "mid" => "|",
        "%" => "%",
        "$" => "$",
        "#" => "#",
        "&" => "&",
        "_" => "_",
        "/" => "/",
        "colon" => ":",
        "ast" => "\u{2217}",
        "wedge" => "\u{2227}",
        "vee" => "\u{2228}",
        "lnot" => "\u{00AC}",
        "dots" | "dotsc" | "dotso" | "hdots" => "\u{2026}",
        "dotsb" | "dotsm" | "dotsi" => "\u{22EF}",
        "cdotp" => "\u{00B7}",
        "longrightarrow" => "\u{27F6}",
        "longleftarrow" => "\u{27F5}",
        "Longrightarrow" | "Longleftrightarrow" => "\u{27F9}",
        "longmapsto" => "\u{27FC}",
        "hookrightarrow" => "\u{21AA}",
        "hookleftarrow" => "\u{21A9}",
        "rightleftharpoons" => "\u{21CC}",
        "leftrightarrows" => "\u{21C6}",
        "Uparrow" => "\u{21D1}",
        "Downarrow" => "\u{21D3}",
        "updownarrow" => "\u{2195}",
        "gets" => "\u{2190}",
        "nmid" => "\u{2224}",
        "nleq" => "\u{2270}",
        "ngeq" => "\u{2271}",
        "subsetneq" => "\u{228A}",
        "supsetneq" => "\u{228B}",
        "leqslant" => "\u{2A7D}",
        "geqslant" => "\u{2A7E}",
        "lesssim" => "\u{2272}",
        "gtrsim" => "\u{2273}",
        "asymp" => "\u{224D}",
        "doteq" => "\u{2250}",
        "triangleq" => "\u{225C}",
        "coloneqq" => "\u{2254}",
        "preceq" => "\u{2AAF}",
        "succeq" => "\u{2AB0}",
        "sqsubseteq" => "\u{2291}",
        "sqsupseteq" => "\u{2292}",
        "ni" | "owns" => "\u{220B}",
        "vdash" => "\u{22A2}",
        "dashv" => "\u{22A3}",
        "models" => "\u{22A8}",
        "varnothing" => "\u{2205}",
        "square" | "Box" => "\u{25A1}",
        "triangle" => "\u{25B3}",
        "diamond" | "Diamond" => "\u{22C4}",
        "lozenge" => "\u{25CA}",
        "imath" => "\u{0131}",
        "jmath" => "\u{0237}",
        "wp" => "\u{2118}",
        "complement" => "\u{2201}",
        "sqcup" => "\u{2294}",
        "sqcap" => "\u{2293}",
        "uplus" => "\u{228E}",
        "amalg" => "\u{2A3F}",
        "wr" => "\u{2240}",
        "bigcirc" => "\u{25EF}",
        "checkmark" => "\u{2713}",
        "flat" => "\u{266D}",
        "natural" => "\u{266E}",
        "sharp" => "\u{266F}",
        "S" => "\u{00A7}",
        "P" => "\u{00B6}",
        "varkappa" => "\u{03F0}",
        "digamma" => "\u{03DD}",
        "Varepsilon" => "\u{0395}",
        "therefore" => "\u{2234}",
        "because" => "\u{2235}",
        "lhd" => "\u{22B2}",
        "rhd" => "\u{22B3}",
        "unlhd" => "\u{22B4}",
        "unrhd" => "\u{22B5}",
        "bowtie" | "Join" => "\u{22C8}",
        "smile" => "\u{2323}",
        "frown" => "\u{2322}",
        "nexists" => "\u{2204}",
        "infin" => "\u{221E}",
        _ => return None,
    })
}

/// Relations and arrows end the body of a preceding n-ary operator.
fn is_relation(name: &str) -> bool {
    matches!(
        name,
        "leq"
            | "le"
            | "geq"
            | "ge"
            | "neq"
            | "ne"
            | "approx"
            | "equiv"
            | "sim"
            | "simeq"
            | "cong"
            | "propto"
            | "ll"
            | "gg"
            | "to"
            | "rightarrow"
            | "leftarrow"
            | "Rightarrow"
            | "Leftarrow"
            | "implies"
            | "iff"
            | "mapsto"
            | "pm"
            | "mp"
            | "leqslant"
            | "geqslant"
            | "lesssim"
            | "gtrsim"
            | "coloneqq"
            | "triangleq"
    )
}

fn font(name: &str) -> Option<Style> {
    Some(match name {
        "mathrm" | "mathup" => Style::Upright,
        "mathbf" => Style::Bold,
        "boldsymbol" | "bm" | "pmb" => Style::BoldItalic,
        "mathit" | "mathnormal" => Style::Italic,
        "mathsf" => Style::Sans,
        "mathtt" => Style::Mono,
        "mathcal" | "mathscr" => Style::Script,
        "mathfrak" => Style::Fraktur,
        "mathbb" | "mathbbm" | "mathds" => Style::DoubleStruck,
        _ => return None,
    })
}

/// Fractions, roots, binomials, stacking and fenced groups.
fn structural(name: &str, tokens: &[Token], i: &mut usize, ctx: Ctx) -> Option<String> {
    Some(match name {
        "frac" | "dfrac" | "tfrac" | "cfrac" => {
            let num = arg_xml(tokens, i, ctx);
            let den = arg_xml(tokens, i, ctx);
            format!("<m:f><m:num>{num}</m:num><m:den>{den}</m:den></m:f>")
        }
        "binom" | "dbinom" | "tbinom" => {
            let n = arg_xml(tokens, i, ctx);
            let k = arg_xml(tokens, i, ctx);
            delimited(
                "(",
                ")",
                &[format!(
                    "<m:f><m:fPr><m:type m:val=\"noBar\"/></m:fPr><m:num>{n}</m:num><m:den>{k}</m:den></m:f>"
                )],
                "",
            )
        }
        "sqrt" => {
            let degree = match tokens.get(*i) {
                Some(Token::OptionalArg(n)) => {
                    *i += 1;
                    Some(convert(&tokenize_latex(n), ctx))
                }
                _ => None,
            };
            let body = arg_xml(tokens, i, ctx);
            match degree {
                Some(d) => format!("<m:rad><m:deg>{d}</m:deg><m:e>{body}</m:e></m:rad>"),
                None => format!(
                    "<m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e>{body}</m:e></m:rad>"
                ),
            }
        }
        "overset" | "stackrel" => {
            let top = arg_xml(tokens, i, ctx);
            let base = arg_xml(tokens, i, ctx);
            format!("<m:limUpp><m:e>{base}</m:e><m:lim>{top}</m:lim></m:limUpp>")
        }
        "underset" => {
            let bottom = arg_xml(tokens, i, ctx);
            let base = arg_xml(tokens, i, ctx);
            format!("<m:limLow><m:e>{base}</m:e><m:lim>{bottom}</m:lim></m:limLow>")
        }
        "xrightarrow" | "xleftarrow" => {
            if matches!(tokens.get(*i), Some(Token::OptionalArg(_))) {
                *i += 1;
            }
            let arrow = if name == "xrightarrow" {
                "\u{27F6}"
            } else {
                "\u{27F5}"
            };
            let label = arg_xml(tokens, i, ctx);
            format!(
                "<m:limUpp><m:e>{}</m:e><m:lim>{label}</m:lim></m:limUpp>",
                run(arrow, Style::Italic)
            )
        }
        "not" => negate(tokens, i, ctx),
        "big" | "Big" | "bigg" | "Bigg" | "bigl" | "Bigl" | "biggl" | "Biggl" | "bigr" | "Bigr"
        | "biggr" | "Biggr" | "bigm" | "Bigm" | "biggm" | "Biggm" | "right" | "middle" => {
            let d = delimiter(tokens, i);
            run(d, ctx.style)
        }
        _ => return None,
    })
}

/// `\left( … \right)`: a fenced group that grows with its content.
/// `\middle|` separates the group into parts.
fn fenced(tokens: &[Token], i: &mut usize, ctx: Ctx) -> Atom {
    let open = delimiter(tokens, i);
    let mut depth = 0;
    let mut parts: Vec<Vec<Token>> = vec![Vec::new()];
    let mut separator = "";
    let mut close = "";
    while let Some(token) = tokens.get(*i) {
        *i += 1;
        match token {
            Token::Command(c) if c == "left" => depth += 1,
            Token::Command(c) if c == "right" && depth == 0 => {
                close = delimiter(tokens, i);
                break;
            }
            Token::Command(c) if c == "right" => depth -= 1,
            Token::Command(c) if c == "middle" && depth == 0 => {
                separator = delimiter(tokens, i);
                parts.push(Vec::new());
                continue;
            }
            _ => {}
        }
        if let Some(part) = parts.last_mut() {
            part.push(token.clone());
        }
    }
    let parts: Vec<String> = parts.iter().map(|p| convert(p, ctx)).collect();
    Atom::xml(delimited(open, close, &parts, separator))
}

fn delimited(open: &str, close: &str, parts: &[String], separator: &str) -> String {
    let mut props = format!(
        "<m:begChr m:val=\"{}\"/><m:endChr m:val=\"{}\"/>",
        escape(open),
        escape(close)
    );
    if !separator.is_empty() {
        props.push_str(&format!("<m:sepChr m:val=\"{}\"/>", escape(separator)));
    }
    let body: String = parts.iter().map(|p| format!("<m:e>{p}</m:e>")).collect();
    format!("<m:d><m:dPr>{props}</m:dPr>{body}</m:d>")
}

/// The delimiter after `\left`, `\right`, `\big`, …; `.` is no delimiter.
fn delimiter(tokens: &[Token], i: &mut usize) -> &'static str {
    let Some(token) = tokens.get(*i) else {
        return "";
    };
    *i += 1;
    match token {
        Token::Text(unit) => match unit.as_str() {
            "(" => "(",
            ")" => ")",
            "[" => "[",
            "]" => "]",
            "|" => "|",
            "/" => "/",
            "<" => "\u{27E8}",
            ">" => "\u{27E9}",
            _ => "",
        },
        Token::Command(name) => match name.as_str() {
            "{" | "lbrace" => "{",
            "}" | "rbrace" => "}",
            "|" | "Vert" | "lVert" | "rVert" => "\u{2016}",
            "vert" | "lvert" | "rvert" => "|",
            "langle" => "\u{27E8}",
            "rangle" => "\u{27E9}",
            "lfloor" => "\u{230A}",
            "rfloor" => "\u{230B}",
            "lceil" => "\u{2308}",
            "rceil" => "\u{2309}",
            "uparrow" => "\u{2191}",
            "downarrow" => "\u{2193}",
            "backslash" => "\\",
            _ => "",
        },
        _ => {
            *i -= 1;
            ""
        }
    }
}

/// `\not` followed by a relation: the negated symbol.
fn negate(tokens: &[Token], i: &mut usize, ctx: Ctx) -> String {
    let negated = match tokens.get(*i) {
        Some(Token::Text(u)) => match u.as_str() {
            "=" => Some("\u{2260}"),
            "<" => Some("\u{226E}"),
            ">" => Some("\u{226F}"),
            _ => None,
        },
        Some(Token::Command(c)) => match c.as_str() {
            "in" => Some("\u{2209}"),
            "subset" => Some("\u{2284}"),
            "subseteq" => Some("\u{2288}"),
            "equiv" => Some("\u{2262}"),
            "exists" => Some("\u{2204}"),
            "parallel" => Some("\u{2226}"),
            "mid" => Some("\u{2224}"),
            "sim" => Some("\u{2241}"),
            "approx" => Some("\u{2249}"),
            "le" | "leq" => Some("\u{2270}"),
            "ge" | "geq" => Some("\u{2271}"),
            _ => None,
        },
        _ => None,
    };
    match negated {
        Some(s) => {
            *i += 1;
            run(s, ctx.style)
        }
        None => String::new(),
    }
}

/// `\text{…}`, `\operatorname{…}` and friends.
fn text_command(name: &str, tokens: &[Token], i: &mut usize) -> Option<Atom> {
    let style = match name {
        "text" | "textrm" | "textnormal" | "textup" | "textmd" | "textsc" | "mbox" | "hbox"
        | "textit" | "textsl" | "emph" | "texttt" | "textsf" => Style::Text,
        "textbf" => Style::Bold,
        "operatorname" | "operatorname*" => {
            let label = text_of(&next_arg(tokens, i));
            return Some(Atom::Func {
                name: label.trim().to_string(),
                limits: name.ends_with('*'),
                sub: None,
                sup: None,
            });
        }
        _ => return None,
    };
    let text = text_of(&next_arg(tokens, i)).replace('$', "");
    Some(Atom::xml(run(&text, style)))
}

/// Accents and over/under decorations.
fn decoration(name: &str, tokens: &[Token], i: &mut usize, ctx: Ctx) -> Option<String> {
    let accent = match name {
        "hat" | "widehat" => Some("\u{0302}"),
        "tilde" | "widetilde" => Some("\u{0303}"),
        "bar" => Some("\u{0305}"),
        "vec" | "overrightarrow" => Some("\u{20D7}"),
        "dot" => Some("\u{0307}"),
        "ddot" => Some("\u{0308}"),
        "acute" => Some("\u{0301}"),
        "grave" => Some("\u{0300}"),
        "breve" => Some("\u{0306}"),
        "check" => Some("\u{030C}"),
        "mathring" => Some("\u{030A}"),
        _ => None,
    };
    if let Some(chr) = accent {
        let body = arg_xml(tokens, i, ctx);
        return Some(format!(
            "<m:acc><m:accPr><m:chr m:val=\"{chr}\"/></m:accPr><m:e>{body}</m:e></m:acc>"
        ));
    }
    Some(match name {
        "overline" => format!(
            "<m:bar><m:barPr><m:pos m:val=\"top\"/></m:barPr><m:e>{}</m:e></m:bar>",
            arg_xml(tokens, i, ctx)
        ),
        "underline" => format!(
            "<m:bar><m:barPr><m:pos m:val=\"bot\"/></m:barPr><m:e>{}</m:e></m:bar>",
            arg_xml(tokens, i, ctx)
        ),
        "overbrace" => format!(
            "<m:groupChr><m:groupChrPr><m:chr m:val=\"\u{23DE}\"/><m:pos m:val=\"top\"/><m:vertJc m:val=\"bot\"/></m:groupChrPr><m:e>{}</m:e></m:groupChr>",
            arg_xml(tokens, i, ctx)
        ),
        "underbrace" => format!(
            "<m:groupChr><m:groupChrPr><m:chr m:val=\"\u{23DF}\"/></m:groupChrPr><m:e>{}</m:e></m:groupChr>",
            arg_xml(tokens, i, ctx)
        ),
        "boxed" => format!(
            "<m:borderBox><m:e>{}</m:e></m:borderBox>",
            arg_xml(tokens, i, ctx)
        ),
        _ => return None,
    })
}

/// Commands with no visible output, or that only wrap their argument.
fn ignored(name: &str, tokens: &[Token], i: &mut usize, ctx: Ctx) -> Option<String> {
    match name {
        "\\" | "newline" | "cr" => {
            if matches!(tokens.get(*i), Some(Token::OptionalArg(_))) {
                *i += 1;
            }
            Some(String::new())
        }
        "nonumber" | "notag" | "displaystyle" | "textstyle" | "scriptstyle"
        | "scriptscriptstyle" | "limits" | "nolimits" | "allowbreak" | "nobreak" | "qedhere"
        | "relax" | "strut" | "mathstrut" | "protect" | "hline" | "centering" | "small"
        | "footnotesize" | "scriptsize" | "normalsize" | "large" | "rm" | "tt" | "bf" | "it"
        | "sf" | "cal" | "left" => Some(String::new()),
        "addtocounter" | "setcounter" => {
            next_arg(tokens, i);
            next_arg(tokens, i);
            Some(String::new())
        }
        "label" | "tag" | "hspace" | "vspace" | "color" | "cline" | "mspace" | "hskip" | "kern"
        | "mkern" | "phantom" | "hphantom" | "vphantom" => {
            next_arg(tokens, i);
            Some(String::new())
        }
        "textcolor" => {
            next_arg(tokens, i);
            Some(arg_xml(tokens, i, ctx))
        }
        "mathop" | "mathbin" | "mathrel" | "mathord" | "mathopen" | "mathclose" | "ensuremath"
        | "cancel" | "bcancel" | "xcancel" | "sout" => Some(arg_xml(tokens, i, ctx)),
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
            let end = (*i + 1 + arity(name)).min(tokens.len());
            let arg = tokens[*i..end].to_vec();
            *i = end;
            arg
        }
        Some(t @ Token::Text(_)) => {
            *i += 1;
            vec![t.clone()]
        }
        Some(t @ Token::Environment { .. }) => {
            *i += 1;
            vec![t.clone()]
        }
        _ => Vec::new(),
    }
}

fn arity(name: &str) -> usize {
    match name {
        "frac" | "dfrac" | "tfrac" | "cfrac" | "binom" | "overset" | "underset" | "stackrel"
        | "textcolor" => 2,
        "sqrt" | "hat" | "widehat" | "tilde" | "widetilde" | "bar" | "vec" | "dot" | "ddot"
        | "overline" | "underline" | "text" | "operatorname" | "mathrm" | "mathbf" | "mathbb"
        | "mathcal" | "mathit" | "boldsymbol" | "mathsf" | "mathtt" | "mathfrak" => 1,
        _ => 0,
    }
}

fn arg_xml(tokens: &[Token], i: &mut usize, ctx: Ctx) -> String {
    convert(&next_arg(tokens, i), ctx)
}

/// Plain text content of a token list, for `\text{…}` and operator names.
fn text_of(tokens: &[Token]) -> String {
    let mut out = String::new();
    for token in tokens {
        match token {
            Token::Text(s) => out.push_str(s),
            Token::BraceGroup(inner) => out.push_str(&text_of(inner)),
            Token::Command(c) => match c.as_str() {
                "\\" | "," | ";" | ":" | ">" | " " | "quad" => out.push(' '),
                "!" => {}
                "{" | "}" | "%" | "&" | "#" | "_" | "$" => out.push_str(c),
                _ => {}
            },
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

// ─── Environments ───────────────────────────────────────────────────────────

fn environment(name: &str, body: &str, ctx: Ctx) -> String {
    let base = name.trim_end_matches('*');
    let delims = match base {
        "pmatrix" => Some(("(", ")")),
        "bmatrix" => Some(("[", "]")),
        "Bmatrix" => Some(("{", "}")),
        "vmatrix" => Some(("|", "|")),
        "Vmatrix" => Some(("\u{2016}", "\u{2016}")),
        "matrix" | "smallmatrix" | "array" | "tabular" => Some(("", "")),
        _ => None,
    };
    if let Some((open, close)) = delims {
        let body = if matches!(base, "array" | "tabular") {
            strip_leading_group(body)
        } else {
            body
        };
        let m = matrix(body, ctx);
        return if open.is_empty() {
            m
        } else {
            delimited(open, close, &[m], "")
        };
    }
    match base {
        "cases" | "dcases" => delimited("{", "", &[cases(body, ctx)], ""),
        "rcases" => delimited("", "}", &[cases(body, ctx)], ""),
        "alignat" | "alignedat" => rows(strip_leading_group(body), ctx),
        // align, aligned, gather, split, equation, multline, eqnarray, unknown.
        _ => rows(body, ctx),
    }
}

fn matrix(body: &str, ctx: Ctx) -> String {
    let mut out = String::from("<m:m>");
    for row in split_top_level(body, Separator::Row) {
        if row.trim().is_empty() {
            continue;
        }
        out.push_str("<m:mr>");
        for cell in split_top_level(&row, Separator::Cell) {
            out.push_str(&format!(
                "<m:e>{}</m:e>",
                convert(&tokenize_latex(&cell), ctx)
            ));
        }
        out.push_str("</m:mr>");
    }
    out.push_str("</m:m>");
    out
}

/// `cases` rows as an equation array, so `&` aligns the conditions.
fn cases(body: &str, ctx: Ctx) -> String {
    let rows: Vec<String> = split_top_level(body, Separator::Row)
        .into_iter()
        .filter(|row| !row.trim().is_empty())
        .collect();
    eq_array(&rows, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn omml(latex: &str) -> String {
        body(latex, true)
    }

    #[test]
    fn letters_numbers_and_operators_are_runs() {
        assert_eq!(
            omml("x+1"),
            "<m:r><m:t xml:space=\"preserve\">x</m:t></m:r>\
             <m:r><m:t xml:space=\"preserve\">+</m:t></m:r>\
             <m:r><m:t xml:space=\"preserve\">1</m:t></m:r>"
        );
    }

    #[test]
    fn fraction() {
        let out = omml(r"\frac{a}{b}");
        assert!(out.starts_with("<m:f><m:num>"), "{out}");
        assert!(out.contains("<m:den><m:r><m:t xml:space=\"preserve\">b</m:t></m:r></m:den>"));
    }

    #[test]
    fn sub_and_superscript_combine() {
        let out = omml("x_i^2");
        assert!(out.starts_with("<m:sSubSup><m:e>"), "{out}");
        assert_eq!(out.matches("<m:sSubSup>").count(), 1);
    }

    #[test]
    fn sum_takes_its_body_up_to_the_next_operator() {
        let out = omml(r"\sum_{i=1}^{n} x_i + y");
        assert!(out.starts_with("<m:nary>"), "{out}");
        assert!(out.contains("<m:limLoc m:val=\"undOvr\"/>"));
        let body = &out[out.find("<m:e>").unwrap_or(0)..out.find("</m:nary>").unwrap_or(0)];
        assert!(body.contains("<m:sSub>"), "{body}");
        assert!(!body.contains('y'), "{body}");
        assert!(out.ends_with(">y</m:t></m:r>"), "{out}");
    }

    #[test]
    fn integrals_put_limits_beside() {
        let out = omml(r"\int_0^1 f");
        assert!(out.contains("<m:limLoc m:val=\"subSup\"/>"), "{out}");
    }

    #[test]
    fn left_right_is_a_growing_delimiter() {
        let out = omml(r"\left( \frac{a}{b} \right]");
        assert!(
            out.starts_with(
                "<m:d><m:dPr><m:begChr m:val=\"(\"/><m:endChr m:val=\"]\"/></m:dPr><m:e><m:f>"
            ),
            "{out}"
        );
    }

    #[test]
    fn rows_become_an_equation_array_with_alignment() {
        let out = omml(r"a &= b \\ c &= d");
        assert!(out.starts_with("<m:eqArr><m:e>"), "{out}");
        assert_eq!(out.matches("<m:e>").count(), 2);
        assert!(out.contains(">&amp;</m:t>"), "{out}");
    }

    #[test]
    fn ampersand_outside_arrays_is_dropped() {
        assert!(!body("a & b", false).contains("&amp;"));
    }

    #[test]
    fn fonts_set_run_styles() {
        assert!(omml(r"\mathbb{R}").contains("<m:scr m:val=\"double-struck\"/>"));
        assert!(omml(r"\mathbf{x}").contains("<m:sty m:val=\"b\"/>"));
        assert!(omml(r"\mathrm{KL}").contains("<m:sty m:val=\"p\"/>"));
    }

    #[test]
    fn functions_apply_to_the_next_atom() {
        let out = omml(r"\log p");
        assert!(out.starts_with("<m:func><m:fName>"), "{out}");
        assert!(out.contains(">log</m:t>"));
    }

    #[test]
    fn limit_operators_put_the_subscript_below() {
        let out = omml(r"\operatorname*{arg\,min}_{\theta} L");
        assert!(out.contains("<m:limLow>"), "{out}");
        assert!(out.contains(">arg min</m:t>"), "{out}");
    }

    #[test]
    fn greek_and_symbols_are_unicode() {
        assert!(omml(r"\alpha \leq \infty").contains("\u{03B1}"));
        assert!(omml(r"\alpha \leq \infty").contains("\u{2264}"));
    }

    #[test]
    fn accents_and_roots() {
        assert!(omml(r"\hat{\theta}").starts_with("<m:acc>"));
        assert!(omml(r"\sqrt{x}").contains("<m:degHide m:val=\"1\"/>"));
        assert!(omml(r"\sqrt[3]{x}").contains("<m:deg><m:r>"));
    }

    #[test]
    fn matrices_and_cases() {
        let m = omml(r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}");
        assert!(m.contains("<m:m><m:mr><m:e>"), "{m}");
        assert_eq!(m.matches("<m:mr>").count(), 2);
        let c = omml(r"\begin{cases} 1 & x > 0 \\ 0 & \text{otherwise} \end{cases}");
        assert!(c.contains("<m:begChr m:val=\"{\"/>"), "{c}");
        assert!(c.contains("<m:eqArr>"), "{c}");
        assert!(c.contains("<m:nor/>"), "{c}");
    }

    #[test]
    fn text_is_escaped() {
        assert!(omml(r"a < b").contains("&lt;"));
        assert!(omml(r"\text{x & y}").contains("x &amp; y"));
    }

    #[test]
    fn unknown_commands_keep_their_name_upright() {
        assert!(omml(r"\foo").contains(">foo</m:t>"));
        assert!(!omml(r"\foo").contains('\\'));
    }

    #[test]
    fn digits_after_scripts_and_commands_split_like_latex() {
        let frac = omml(r"\frac12");
        assert!(
            frac.contains("<m:num><m:r><m:t xml:space=\"preserve\">1</m:t></m:r></m:num>"),
            "{frac}"
        );
        assert!(
            frac.contains("<m:den><m:r><m:t xml:space=\"preserve\">2</m:t></m:r></m:den>"),
            "{frac}"
        );
        let power = omml("x^10");
        assert!(
            power.contains("<m:sup><m:r><m:t xml:space=\"preserve\">1</m:t></m:r></m:sup>"),
            "{power}"
        );
        assert!(power.ends_with(">0</m:t></m:r>"), "{power}");
        assert!(omml("x^{10}").contains(">10</m:t>"));
    }

    #[test]
    fn wrappers() {
        assert!(latex_to_omml_inline("x").starts_with("<m:oMath><m:r>"));
        assert!(latex_to_omml_display("x").starts_with("<m:oMathPara><m:oMath>"));
    }
}
