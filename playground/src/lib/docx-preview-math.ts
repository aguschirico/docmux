/**
 * docx-preview draws Word math (OMML) as MathML, with gaps: it skips
 * accents, combined sub/superscripts and upper limits, and writes every run
 * as a string literal (`<ms>`). These helpers adapt the copy that the
 * playground displays; the .docx a visitor downloads is left as is.
 */
import JSZip from "jszip";

const OMML = "http://schemas.openxmlformats.org/officeDocument/2006/math";
const MATHML = "http://www.w3.org/1998/Math/MathML";

/** Accent marks as spacing characters, which can sit above a base. */
const ACCENTS: Record<string, string> = {
  "̂": "ˆ",
  "̃": "˜",
  "̅": "¯",
  "⃗": "→",
  "̇": "˙",
  "̈": "¨",
  "́": "´",
  "̀": "`",
  "̆": "˘",
  "̌": "ˇ",
  "̊": "˚",
};

/**
 * Mathematical alphanumeric symbols: (first capital, first small letter,
 * first digit) per style, and letters that live elsewhere in Unicode.
 */
const ALPHABETS: Record<string, { upper: number; lower: number; digit?: number; holes: Record<string, number> }> = {
  b: { upper: 0x1d400, lower: 0x1d41a, digit: 0x1d7ce, holes: {} },
  bi: { upper: 0x1d468, lower: 0x1d482, holes: {} },
  "double-struck": {
    upper: 0x1d538,
    lower: 0x1d552,
    digit: 0x1d7d8,
    holes: { C: 0x2102, H: 0x210d, N: 0x2115, P: 0x2119, Q: 0x211a, R: 0x211d, Z: 0x2124 },
  },
  script: {
    upper: 0x1d49c,
    lower: 0x1d4b6,
    holes: { B: 0x212c, E: 0x2130, F: 0x2131, H: 0x210b, I: 0x2110, L: 0x2112, M: 0x2133, R: 0x211b, e: 0x212f, g: 0x210a, o: 0x2134 },
  },
  fraktur: {
    upper: 0x1d504,
    lower: 0x1d51e,
    holes: { C: 0x212d, H: 0x210c, I: 0x2111, R: 0x211c, Z: 0x2128 },
  },
  "sans-serif": { upper: 0x1d5a0, lower: 0x1d5ba, digit: 0x1d7e2, holes: {} },
  monospace: { upper: 0x1d670, lower: 0x1d68a, digit: 0x1d7f6, holes: {} },
};

function styled(text: string, style: string): string {
  const alphabet = ALPHABETS[style];
  if (!alphabet) return text;
  return Array.from(text)
    .map((c) => {
      const hole = alphabet.holes[c];
      if (hole) return String.fromCodePoint(hole);
      if (c >= "A" && c <= "Z") return String.fromCodePoint(alphabet.upper + c.charCodeAt(0) - 65);
      if (c >= "a" && c <= "z") return String.fromCodePoint(alphabet.lower + c.charCodeAt(0) - 97);
      if (c >= "0" && c <= "9" && alphabet.digit) return String.fromCodePoint(alphabet.digit + c.charCodeAt(0) - 48);
      return c;
    })
    .join("");
}

function child(el: Element, name: string): Element | null {
  return Array.from(el.children).find((c) => c.localName === name) ?? null;
}

function make(doc: XMLDocument, name: string, ...children: (Element | null)[]): Element {
  const el = doc.createElementNS(OMML, `m:${name}`);
  for (const c of children) if (c) el.appendChild(c);
  return el;
}

/** `m:e` wrapping `el`. */
function base(doc: XMLDocument, el: Element): Element {
  return make(doc, "e", el);
}

function groupChr(doc: XMLDocument, chr: string, body: Element | null): Element {
  const props = make(doc, "groupChrPr");
  const c = doc.createElementNS(OMML, "m:chr");
  c.setAttributeNS(OMML, "m:val", chr);
  const jc = doc.createElementNS(OMML, "m:vertJc");
  // docx-preview draws vertJc="bot" as <mover>: the character on top.
  jc.setAttributeNS(OMML, "m:val", "bot");
  props.append(c, jc);
  return make(doc, "groupChr", props, body);
}

function rewrite(doc: XMLDocument) {
  // Run styles (\mathbb, \mathbf, …) as styled characters.
  for (const run of Array.from(doc.getElementsByTagNameNS(OMML, "r"))) {
    const props = child(run, "rPr");
    if (!props) continue;
    const scr = child(props, "scr")?.getAttributeNS(OMML, "val");
    const sty = child(props, "sty")?.getAttributeNS(OMML, "val");
    const style = scr && scr !== "roman" ? scr : sty === "b" || sty === "bi" ? sty : null;
    if (!style) continue;
    for (const t of Array.from(run.getElementsByTagNameNS(OMML, "t"))) {
      t.textContent = styled(t.textContent ?? "", style);
    }
  }
  for (const el of Array.from(doc.getElementsByTagNameNS(OMML, "sSubSup"))) {
    const sub = make(doc, "sSub", child(el, "e"), child(el, "sub"));
    el.replaceWith(make(doc, "sSup", base(doc, sub), child(el, "sup")));
  }
  for (const el of Array.from(doc.getElementsByTagNameNS(OMML, "acc"))) {
    const chr = child(child(el, "accPr") ?? el, "chr")?.getAttributeNS(OMML, "val") ?? "̂";
    el.replaceWith(groupChr(doc, ACCENTS[chr] ?? chr, child(el, "e")));
  }
  for (const el of Array.from(doc.getElementsByTagNameNS(OMML, "limUpp"))) {
    const lim = child(el, "lim");
    const sup = make(doc, "sup", ...Array.from(lim?.childNodes ?? []).filter((n): n is Element => n instanceof Element));
    el.replaceWith(make(doc, "sSup", child(el, "e"), sup));
  }
}

/** Rewrite the math in `bytes` into elements docx-preview can draw. */
export async function adaptMath(bytes: Uint8Array): Promise<Uint8Array> {
  const zip = await JSZip.loadAsync(bytes);
  const file = zip.file("word/document.xml");
  if (!file) return bytes;
  const xml = await file.async("string");
  if (!xml.includes("<m:")) return bytes;
  const doc = new DOMParser().parseFromString(xml, "application/xml");
  if (doc.getElementsByTagName("parsererror").length > 0) return bytes;
  rewrite(doc);
  zip.file("word/document.xml", new XMLSerializer().serializeToString(doc));
  return zip.generateAsync({ type: "uint8array" });
}

/** The MathML element a run of text should be: number, identifier or operator. */
function tagFor(text: string): string {
  if (/^\d+(\.\d+)?$/.test(text)) return "mn";
  if (/^\p{L}+$/u.test(text)) return "mi";
  return "mo";
}

/** Fix the MathML docx-preview produced inside `root`. */
export function fixRenderedMath(root: HTMLElement) {
  // Token elements (mi, mn, mo, ms) may only hold text; docx-preview puts
  // whole scripts and function names in them. Those become rows.
  for (const token of Array.from(root.querySelectorAll("mi, mn, mo, ms, mtext"))) {
    if (token.children.length === 0) continue;
    const row = document.createElementNS(MATHML, "mrow");
    row.append(...Array.from(token.childNodes));
    token.replaceWith(row);
  }
  for (const ms of Array.from(root.querySelectorAll("ms"))) {
    const text = ms.textContent ?? "";
    // `&` marks alignment in a Word equation array; it is not shown.
    if (text === "&") {
      ms.remove();
      continue;
    }
    const tag = tagFor(text);
    const el = document.createElementNS(MATHML, tag);
    el.textContent = text;
    // A bracket in a run is a plain character; growing brackets are m:d.
    if (tag === "mo") el.setAttribute("stretchy", "false");
    ms.replaceWith(el);
  }
  // An equation alone in its paragraph is display math.
  for (const math of Array.from(root.querySelectorAll("math"))) {
    const paragraph = math.closest("p");
    if (paragraph && paragraph.textContent?.trim() === math.textContent?.trim()) {
      math.setAttribute("display", "block");
    }
  }
}
