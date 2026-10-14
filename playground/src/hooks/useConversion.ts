import { useEffect, useRef, useState } from "react";
import {
  convert,
  convertStandalone,
  convertBytes,
  convertBytesStandalone,
  convertWithResources,
  convertWithFiles,
  convertToBytes,
  convertBytesToBytes,
  parseToJson,
  parseBytesToJson,
  type ConvertOutcome,
} from "@/wasm/docmux";
import { getFormat, isBinaryFormat } from "@/lib/formats";
import { PREVIEW_TAB } from "@/lib/output-formats";

const DEBOUNCE_MS = 150;

export interface ConversionInput {
  /** Path of the file being converted; its extension gives the format. */
  path: string | null;
  /** Text content (null for binary inputs). */
  content: string | null;
  /** Bytes of a binary input such as a .docx. */
  binary: Uint8Array | null;
  /** The project's other text files (LaTeX `\input`s, `.bbl`), as UTF-8. */
  projectFiles: Map<string, Uint8Array>;
  /** Images, keyed by path. */
  resources: Map<string, Uint8Array>;
}

export interface ConversionState {
  inputFormat: string | null;
  /** Standalone HTML, for the preview tab. */
  preview: string | null;
  /** Converted source, for writer tabs. */
  source: string | null;
  /** Converted bytes, for binary writers (Word). */
  binaryOutput: Uint8Array | null;
  /** Conversion time of the last run, in milliseconds. */
  ms: number | null;
  errors: string[];
  converting: boolean;
}

const EMPTY: ConversionState = {
  inputFormat: null,
  preview: null,
  source: null,
  binaryOutput: null,
  ms: null,
  errors: [],
  converting: false,
};

function outcome(r: ConvertOutcome, errors: string[]): string | null {
  if (r.error) errors.push(r.error);
  return r.output;
}

/** Convert the input to what `tab` shows: preview HTML, a writer's output, or the AST. */
async function run(input: ConversionInput, from: string, tab: string): Promise<ConversionState> {
  const errors: string[] = [];
  const isLatex = from === "latex";
  const binary = input.binary;
  const text = input.content ?? "";
  const started = performance.now();
  let preview: string | null = null;
  let source: string | null = null;
  let binaryOutput: Uint8Array | null = null;

  try {
    if (tab === PREVIEW_TAB) {
      preview = outcome(
        binary
          ? await convertBytesStandalone(binary, from, "html")
          : isLatex
            ? await convertWithFiles(text, from, "html", input.projectFiles, input.resources, true)
            : await convertStandalone(text, from, "html"),
        errors,
      );
    } else if (tab === "ast") {
      source = outcome(binary ? await parseBytesToJson(binary, from) : await parseToJson(text, from), errors);
    } else if (isBinaryFormat(tab)) {
      binaryOutput = binary
        ? await convertBytesToBytes(binary, from, tab, input.resources)
        : await convertToBytes(text, from, tab, input.resources);
    } else {
      source = outcome(
        binary
          ? await convertBytes(binary, from, tab)
          : isLatex
            ? await convertWithFiles(text, from, tab, input.projectFiles, input.resources, false)
            : input.resources.size > 0
              ? await convertWithResources(text, from, tab, input.resources)
              : await convert(text, from, tab),
        errors,
      );
    }
  } catch (e) {
    errors.push(String(e));
  }

  const ms = Math.round(performance.now() - started);
  return { inputFormat: from, preview, source, binaryOutput, ms, errors, converting: false };
}

export function useConversion(input: ConversionInput, tab: string): ConversionState {
  const [state, setState] = useState<ConversionState>(EMPTY);
  const seqRef = useRef(0);
  const from = input.path ? getFormat(input.path) : null;
  const hasInput = from !== null && (input.content !== null || input.binary !== null);

  useEffect(() => {
    if (!hasInput || from === null) return;
    const seq = ++seqRef.current;
    const timer = setTimeout(() => {
      setState((prev) => ({ ...prev, converting: true }));
      run(input, from, tab).then((next) => {
        if (seq === seqRef.current) setState(next);
      });
    }, DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [input, from, tab, hasInput]);

  return hasInput ? state : EMPTY;
}
