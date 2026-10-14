import paperMain from "./paper/main.tex?raw";
import paperRefs from "./paper/refs.bbl?raw";
import readme from "./readme/README.md?raw";
import report from "./report/report.typ?raw";

export interface ExampleFile {
  path: string;
  content: string;
}

export interface Example {
  id: string;
  /** Button label, e.g. "Research paper". */
  label: string;
  /** Input format shown next to the label. */
  format: string;
  files: ExampleFile[];
  /** Path of the file to open and convert. */
  main: string;
  /** Output tab to show first. */
  output: string;
}

const PAPER: Example = {
  id: "paper",
  label: "Research paper",
  format: "LaTeX",
  files: [
    { path: "main.tex", content: paperMain },
    { path: "refs.bbl", content: paperRefs },
  ],
  main: "main.tex",
  output: "preview",
};

export const EXAMPLES: Example[] = [
  PAPER,
  {
    id: "readme",
    label: "README",
    format: "Markdown",
    files: [{ path: "README.md", content: readme }],
    main: "README.md",
    output: "typst",
  },
  {
    id: "report",
    label: "Report",
    format: "Typst",
    files: [{ path: "report.typ", content: report }],
    main: "report.typ",
    output: "latex",
  },
];

export const DEFAULT_EXAMPLE = PAPER;
