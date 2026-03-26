const EXTENSION_TO_FORMAT: Record<string, string> = {
  md: "markdown",
  markdown: "markdown",
  tex: "latex",
  latex: "latex",
  typ: "typst",
  html: "html",
  htm: "html",
  json: "json",
  xml: "xml",
  yaml: "yaml",
  yml: "yaml",
  myst: "myst",
  bib: "bibtex",
};

const EXTENSION_TO_MONACO: Record<string, string> = {
  md: "markdown",
  markdown: "markdown",
  tex: "latex",
  latex: "latex",
  typ: "typescript",
  html: "html",
  htm: "html",
  json: "json",
  xml: "xml",
  yaml: "yaml",
  yml: "yaml",
  css: "css",
  js: "javascript",
  ts: "typescript",
  myst: "markdown",
  bib: "plaintext",
};

export function getExtension(path: string): string {
  const dot = path.lastIndexOf(".");
  return dot >= 0 ? path.slice(dot + 1).toLowerCase() : "";
}

export function getFormat(path: string): string {
  return EXTENSION_TO_FORMAT[getExtension(path)] ?? "plaintext";
}

export function getMonacoLanguage(path: string): string {
  return EXTENSION_TO_MONACO[getExtension(path)] ?? "plaintext";
}
