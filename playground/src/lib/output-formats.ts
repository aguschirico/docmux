/** Output tabs: the rendered preview, then each writer docmux has. */
export interface OutputTab {
  /** Tab id; for writers, the docmux format name. */
  id: string;
  label: string;
  /** Monaco language for the source view. */
  language: string;
  /** File extension for downloads. */
  extension: string;
}

export const PREVIEW_TAB = "preview";

export const MAIN_TABS: OutputTab[] = [
  { id: PREVIEW_TAB, label: "Preview", language: "html", extension: "html" },
  { id: "typst", label: "Typst", language: "typst", extension: "typ" },
  { id: "html", label: "HTML", language: "html", extension: "html" },
  { id: "latex", label: "LaTeX", language: "latex", extension: "tex" },
  { id: "markdown", label: "Markdown", language: "markdown", extension: "md" },
  { id: "docx", label: "Word", language: "plaintext", extension: "docx" },
];

export const MORE_TABS: OutputTab[] = [
  { id: "myst", label: "MyST", language: "markdown", extension: "md" },
  { id: "plain", label: "Plain text", language: "plaintext", extension: "txt" },
  { id: "ast", label: "AST (JSON)", language: "json", extension: "json" },
];

export const ALL_TABS = [...MAIN_TABS, ...MORE_TABS];

export function findTab(id: string): OutputTab {
  return ALL_TABS.find((t) => t.id === id) ?? MAIN_TABS[0]!;
}

/** Human name of an input format, for the status line. */
export function formatName(format: string): string {
  const names: Record<string, string> = {
    latex: "LaTeX",
    markdown: "Markdown",
    typst: "Typst",
    html: "HTML",
    docx: "Word",
    myst: "MyST",
  };
  return names[format] ?? format;
}
