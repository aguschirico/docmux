//! Lookup tables for text-mode commands the parser has no dedicated handler
//! for. Real papers use dozens of layout and font commands; the goal is to
//! keep their text and drop their layout, instead of losing the text.

/// Font and layout switches with no argument and no output of their own
/// (`\small`, `\centering`, `\newpage`). Font switches that start a group
/// are turned into formatting by [`group_format`].
pub(crate) fn is_switch(name: &str) -> bool {
    matches!(
        name,
        "tt" | "bf"
            | "it"
            | "em"
            | "rm"
            | "sf"
            | "sc"
            | "sl"
            | "up"
            | "md"
            | "tiny"
            | "scriptsize"
            | "footnotesize"
            | "small"
            | "normalsize"
            | "large"
            | "Large"
            | "LARGE"
            | "huge"
            | "Huge"
            | "bfseries"
            | "itshape"
            | "ttfamily"
            | "scshape"
            | "mdseries"
            | "upshape"
            | "slshape"
            | "rmfamily"
            | "sffamily"
            | "normalfont"
            | "boldmath"
            | "unboldmath"
            | "selectfont"
            | "ignorespaces"
            | "medskip"
            | "smallskip"
            | "bigskip"
            | "newpage"
            | "clearpage"
            | "cleardoublepage"
            | "pagebreak"
            | "nopagebreak"
            | "linebreak"
            | "centering"
            | "raggedright"
            | "raggedleft"
            | "noindent"
            | "indent"
            | "appendix"
            | "makeatletter"
            | "makeatother"
            | "protect"
            | "relax"
            | "par"
            | "hline"
            | "toprule"
            | "midrule"
            | "bottomrule"
            | "endgroup"
            | "begingroup"
            | "allowdisplaybreaks"
            | "maketitle"
            | "tableofcontents"
            | "listoffigures"
            | "listoftables"
            | "onecolumn"
            | "twocolumn"
            | "null"
            | "hfill"
            | "vfill"
            | "hfil"
            | "vfil"
            | "strut"
            | "xspace"
            | "leavevmode"
            | "unskip"
            | "nobreak"
            | "FloatBarrier"
            | "sloppy"
            | "fussy"
            | "frenchspacing"
            | "phantomsection"
            | "printbibliography"
            | "nolinenumbers"
            | "linenumbers"
            | "displaystyle"
            | "textstyle"
            | "and"
    )
}

/// Formatting applied to the rest of a `{…}` group that starts with a font
/// switch: `{\bf text}`, `{\em text}`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum GroupFormat {
    Emphasis,
    Strong,
    SmallCaps,
    Plain,
}

pub(crate) fn group_format(name: &str) -> Option<GroupFormat> {
    Some(match name {
        "em" | "it" | "itshape" | "sl" | "slshape" => GroupFormat::Emphasis,
        "bf" | "bfseries" | "boldmath" => GroupFormat::Strong,
        "sc" | "scshape" => GroupFormat::SmallCaps,
        _ if is_switch(name) => GroupFormat::Plain,
        _ => return None,
    })
}

/// Commands dropped together with their arguments:
/// `(optional arguments, brace arguments)`.
pub(crate) fn dropped(name: &str) -> Option<(usize, usize)> {
    Some(match name {
        "vspace" | "vspace*" | "hspace" | "hspace*" | "bibliographystyle" | "pagenumbering"
        | "stepcounter" | "refstepcounter" | "pgfplotsset" | "thispagestyle" | "pagestyle"
        | "urlstyle" | "hypersetup" | "usetikzlibrary" | "linespread" | "newlength" | "index"
        | "enlargethispage" | "bibliography" | "cline" | "label" => (0, 1),
        "setlength" | "addtolength" | "setcounter" | "addtocounter" | "fontsize" | "settowidth"
        | "algrenewcommand" => (0, 2),
        "definecolor" | "addcontentsline" | "specialrule" => (0, 3),
        "rule" | "newcolumntype" | "newtheorem" => (1, 2),
        "includepdf" | "captionsetup" | "color" => (1, 1),
        _ => return None,
    })
}

/// Commands whose last brace argument is text to keep:
/// `(optional arguments, brace arguments to skip before the text)`.
pub(crate) fn wrapper(name: &str) -> Option<(usize, usize)> {
    Some(match name {
        "mbox" | "fbox" | "hbox" | "text" | "textnormal" | "textrm" | "textup" | "textmd"
        | "textsf" | "rlap" | "llap" | "ensuremath" | "shortstack" | "makecell" | "framebox"
        | "makebox" | "adjustbox" | "hl" | "mathrm" => (2, 0),
        "textcolor" | "colorbox" | "tabincell" | "scalebox" => (1, 1),
        "multicolumn" | "resizebox" | "fcolorbox" | "multirow" => (1, 2),
        "raisebox" => (2, 1),
        _ => return None,
    })
}

/// Commands that stand for a fixed piece of text.
pub(crate) fn abbreviation(name: &str) -> Option<&'static str> {
    Some(match name {
        "ie" => "i.e.",
        "eg" => "e.g.",
        "etal" => "et al.",
        "vs" => "vs.",
        "cf" => "cf.",
        "wrt" => "w.r.t.",
        "iid" => "i.i.d.",
        "etc" => "etc.",
        "textbackslash" => "\\",
        "textasciitilde" => "~",
        "textasciicircum" => "^",
        "ldots" | "dots" | "textellipsis" => "\u{2026}",
        "textendash" => "\u{2013}",
        "textemdash" => "\u{2014}",
        "textregistered" => "\u{00AE}",
        "copyright" | "textcopyright" => "\u{00A9}",
        "texttrademark" => "\u{2122}",
        "textdegree" => "\u{00B0}",
        "S" => "\u{00A7}",
        "dag" => "\u{2020}",
        "ddag" => "\u{2021}",
        "textbullet" => "\u{2022}",
        "LaTeX" => "LaTeX",
        "TeX" => "TeX",
        "BibTeX" => "BibTeX",
        "quad" => "\u{2003}",
        "qquad" => "\u{2003}\u{2003}",
        "ss" => "\u{00DF}",
        "o" => "\u{00F8}",
        "O" => "\u{00D8}",
        "l" => "\u{0142}",
        "L" => "\u{0141}",
        "ae" => "\u{00E6}",
        "AE" => "\u{00C6}",
        "aa" => "\u{00E5}",
        "AA" => "\u{00C5}",
        "i" => "\u{0131}",
        _ => return None,
    })
}

/// Strip a leading TeX dimension (`1em`, `-2.5pt`) from text that follows
/// `\hskip`, `\vskip` or `\kern`, which take no braces.
pub(crate) fn strip_dimension(text: &str) -> &str {
    let trimmed = text.trim_start();
    let number_len = trimmed
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_digit() || *c == '.' || *c == '-')
        .count();
    if number_len == 0 {
        return text;
    }
    let rest = &trimmed[number_len..];
    let units = ["pt", "em", "ex", "mm", "cm", "in", "mu", "bp", "sp", "pc"];
    match units.iter().find(|u| rest.starts_with(*u)) {
        Some(unit) => &rest[unit.len()..],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_dimension_removes_number_and_unit() {
        assert_eq!(strip_dimension(" 1em text"), " text");
        assert_eq!(strip_dimension("-2.5pt"), "");
        assert_eq!(strip_dimension(" word"), " word");
    }

    #[test]
    fn group_formats() {
        assert_eq!(group_format("bf"), Some(GroupFormat::Strong));
        assert_eq!(group_format("em"), Some(GroupFormat::Emphasis));
        assert_eq!(group_format("small"), Some(GroupFormat::Plain));
        assert_eq!(group_format("frac"), None);
    }
}
