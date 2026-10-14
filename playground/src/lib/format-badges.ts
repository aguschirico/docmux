/** Short label and color for each input format, shown as a badge. */
export interface FormatBadge {
  short: string;
  className: string;
}

const BADGES: Record<string, FormatBadge> = {
  latex: { short: "TeX", className: "bg-emerald-500/15 text-emerald-300 ring-emerald-500/30" },
  markdown: { short: "MD", className: "bg-zinc-500/15 text-zinc-200 ring-zinc-500/30" },
  typst: { short: "TYP", className: "bg-cyan-500/15 text-cyan-300 ring-cyan-500/30" },
  docx: { short: "DOCX", className: "bg-blue-500/15 text-blue-300 ring-blue-500/30" },
  html: { short: "HTML", className: "bg-orange-500/15 text-orange-300 ring-orange-500/30" },
};

export function formatBadge(format: string): FormatBadge {
  return BADGES[format] ?? { short: format.toUpperCase(), className: "bg-zinc-500/15 text-zinc-300 ring-zinc-500/30" };
}
