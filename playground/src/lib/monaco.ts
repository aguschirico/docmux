import type { Monaco } from "@monaco-editor/react";
import type { editor, languages } from "monaco-editor";

export const MONACO_THEME = "docmux";

const latex: languages.IMonarchLanguage = {
  tokenizer: {
    root: [
      [/%.*$/, "comment"],
      [/\$\$/, { token: "string.math", next: "@displayMath" }],
      [/\\\[/, { token: "string.math", next: "@displayBracket" }],
      [/\$/, { token: "string.math", next: "@inlineMath" }],
      [
        /(\\begin)(\{)((?:equation|align|gather|multline|eqnarray|displaymath|math)\*?)(\})/,
        ["keyword", "delimiter", "type", { token: "delimiter", next: "@mathEnv" }],
      ],
      [/(\\(?:begin|end))(\{)([^}]*)(\})/, ["keyword", "delimiter", "type", "delimiter"]],
      [
        /(\\(?:section|subsection|subsubsection|paragraph|chapter|title|caption)\*?)(\{)/,
        ["keyword", { token: "delimiter", next: "@heading" }],
      ],
      [
        /(\\(?:label|ref|eqref|cref|autoref|cite[a-z]*\*?))((?:\[[^\]]*\])*)(\{)([^}]*)(\})/,
        ["keyword", "delimiter", "delimiter", "variable", "delimiter"],
      ],
      [/\\[a-zA-Z@]+\*?/, "keyword"],
      [/\\./, "keyword"],
      [/[{}]/, "delimiter.bracket"],
      [/[[\]]/, "delimiter.square"],
      [/&|~/, "operator"],
    ],
    mathEnv: [
      [/(\\end)(\{)([^}]*)(\})/, ["keyword", "delimiter", "type", { token: "delimiter", next: "@pop" }]],
      [/%.*$/, "comment"],
      [/\\label\{[^}]*\}/, "variable"],
      [/\\[a-zA-Z]+/, "string.math.command"],
      [/[^\\%]+/, "string.math"],
      [/\\./, "string.math"],
    ],
    heading: [
      [/\}/, { token: "delimiter", next: "@pop" }],
      [/\\[a-zA-Z@]+/, "keyword"],
      [/[^}\\]+/, "strong"],
    ],
    inlineMath: [
      [/\$/, { token: "string.math", next: "@pop" }],
      [/\\[a-zA-Z]+/, "string.math.command"],
      [/[^$\\]+/, "string.math"],
      [/\\./, "string.math"],
    ],
    displayMath: [
      [/\$\$/, { token: "string.math", next: "@pop" }],
      [/\\[a-zA-Z]+/, "string.math.command"],
      [/[^$\\]+/, "string.math"],
      [/./, "string.math"],
    ],
    displayBracket: [
      [/\\\]/, { token: "string.math", next: "@pop" }],
      [/\\[a-zA-Z]+/, "string.math.command"],
      [/[^\\]+/, "string.math"],
      [/\\./, "string.math"],
    ],
  },
};

const typst: languages.IMonarchLanguage = {
  tokenizer: {
    root: [
      [/\/\/.*$/, "comment"],
      [/\/\*/, { token: "comment", next: "@blockComment" }],
      [/^\s*=+ .*$/, "strong"],
      [/\$/, { token: "string.math", next: "@math" }],
      [/#(?:let|set|show|import|include|if|else|for|while|return)\b/, "keyword"],
      [/#[a-zA-Z_][\w-]*(?:\.[a-zA-Z_][\w-]*)*/, "keyword"],
      [/<[\w:.-]+>/, "variable"],
      [/@[\w:.-]+/, "variable"],
      [/"(?:[^"\\]|\\.)*"/, "string"],
      [/`[^`]*`/, "string"],
      [/\*[^*\n]+\*/, "strong"],
      [/_[^_\n]+_/, "emphasis"],
      [/\b\d+(?:\.\d+)?(?:pt|em|cm|mm|in|%|fr)?\b/, "number"],
      [/^\s*[-+] /, "operator"],
      [/[{}()[\]]/, "delimiter.bracket"],
    ],
    blockComment: [
      [/\*\//, { token: "comment", next: "@pop" }],
      [/./, "comment"],
    ],
    math: [
      [/\$/, { token: "string.math", next: "@pop" }],
      [/[a-zA-Z]{2,}(?:\.[a-z]+)*/, "string.math.command"],
      [/[^$a-zA-Z]+/, "string.math"],
      [/./, "string.math"],
    ],
  },
};

let ready = false;

/** Register LaTeX and Typst highlighting and the playground theme, once. */
export function setupMonaco(monaco: Monaco) {
  if (ready) return;
  ready = true;

  for (const [id, grammar] of [
    ["latex", latex],
    ["typst", typst],
  ] as const) {
    monaco.languages.register({ id });
    monaco.languages.setMonarchTokensProvider(id, grammar);
  }

  monaco.editor.defineTheme(MONACO_THEME, {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "comment", foreground: "6b7280", fontStyle: "italic" },
      { token: "keyword", foreground: "7dd3fc" },
      { token: "type", foreground: "c4b5fd" },
      { token: "variable", foreground: "fca5a5" },
      { token: "string", foreground: "86efac" },
      { token: "string.math", foreground: "fcd34d" },
      { token: "string.math.command", foreground: "fdba74" },
      { token: "strong", foreground: "f4f4f5", fontStyle: "bold" },
      { token: "emphasis", fontStyle: "italic" },
      { token: "number", foreground: "f9a8d4" },
      { token: "operator", foreground: "a1a1aa" },
      { token: "delimiter", foreground: "a1a1aa" },
      { token: "delimiter.bracket", foreground: "a1a1aa" },
      { token: "delimiter.square", foreground: "a1a1aa" },
    ],
    colors: {
      "editor.background": "#09090b",
      "editor.foreground": "#d4d4d8",
      "editorLineNumber.foreground": "#3f3f46",
      "editorLineNumber.activeForeground": "#a1a1aa",
      "editor.lineHighlightBackground": "#18181b",
      "editor.lineHighlightBorder": "#00000000",
      "editor.selectionBackground": "#0c4a6e",
      "editor.inactiveSelectionBackground": "#0c4a6e80",
      "editorCursor.foreground": "#38bdf8",
      "editorIndentGuide.background1": "#27272a",
      "editorBracketMatch.background": "#0c4a6e60",
      "editorBracketMatch.border": "#00000000",
      "scrollbarSlider.background": "#3f3f4660",
      "scrollbarSlider.hoverBackground": "#52525b80",
      "editorWidget.background": "#18181b",
      "editorWidget.border": "#27272a",
    },
  });
}

/** Options shared by the input editor and the read-only output views. */
export const EDITOR_OPTIONS: editor.IStandaloneEditorConstructionOptions = {
  minimap: { enabled: false },
  fontFamily: "'JetBrains Mono', ui-monospace, monospace",
  fontLigatures: true,
  lineHeight: 21,
  scrollBeyondLastLine: false,
  wordWrap: "on",
  padding: { top: 16, bottom: 16 },
  lineNumbersMinChars: 3,
  overviewRulerLanes: 0,
  hideCursorInOverviewRuler: true,
  scrollbar: { verticalScrollbarSize: 10, horizontalScrollbarSize: 10, useShadows: false },
  guides: { indentation: false },
  // Converted text has non-breaking spaces (LaTeX `~`) on purpose.
  unicodeHighlight: { invisibleCharacters: false, ambiguousCharacters: false, nonBasicASCII: false },
};
