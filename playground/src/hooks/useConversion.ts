import { useEffect, useRef, useState } from "react";
import {
  convert,
  convertStandalone,
  parseToJson,
  type ConvertOutcome,
} from "@/wasm/docmux";

const DEBOUNCE_MS = 200;

export interface ConversionState {
  /** HTML preview (standalone mode for full rendering) */
  preview: string | null;
  /** Source output in the selected target format */
  source: string | null;
  /** AST as pretty-printed JSON */
  ast: string | null;
  /** Any conversion errors */
  errors: string[];
  /** Whether a conversion is in progress */
  converting: boolean;
}

const INITIAL: ConversionState = {
  preview: null,
  source: null,
  ast: null,
  errors: [],
  converting: false,
};

export function useConversion(
  content: string | null,
  inputFormat: string | null,
  outputFormat: string,
): ConversionState {
  const [state, setState] = useState<ConversionState>(INITIAL);
  const timerRef = useRef<ReturnType<typeof setTimeout>>();
  const seqRef = useRef(0);

  const hasInput = content !== null && inputFormat !== null;

  useEffect(() => {
    if (!hasInput) return;

    clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      const seq = ++seqRef.current;
      setState((prev) => ({ ...prev, converting: true }));

      Promise.all([
        convertStandalone(content, inputFormat, "html"),
        convert(content, inputFormat, outputFormat),
        parseToJson(content, inputFormat),
      ]).then(([previewResult, sourceResult, astResult]) => {
        if (seq !== seqRef.current) return;

        const errors: string[] = [];
        const collectError = (label: string, r: ConvertOutcome) => {
          if (r.error) errors.push(`[${label}] ${r.error}`);
        };
        collectError("preview", previewResult);
        collectError("source", sourceResult);
        collectError("ast", astResult);

        setState({
          preview: previewResult.output,
          source: sourceResult.output,
          ast: astResult.output,
          errors,
          converting: false,
        });
      });
    }, DEBOUNCE_MS);

    return () => clearTimeout(timerRef.current);
  }, [content, inputFormat, outputFormat, hasInput]);

  if (!hasInput) return INITIAL;
  return state;
}
