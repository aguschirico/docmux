import { useEffect, useRef, useState } from "react";
import {
  convert,
  convertStandalone,
  convertBytes,
  convertBytesStandalone,
  parseToJson,
  parseBytesToJson,
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
  binaryContent?: Uint8Array | null,
): ConversionState {
  const [state, setState] = useState<ConversionState>(INITIAL);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const seqRef = useRef(0);

  const hasBinary = binaryContent != null && binaryContent.length > 0 && inputFormat !== null;
  const hasText = content !== null && inputFormat !== null;
  const hasInput = hasBinary || hasText;

  useEffect(() => {
    if (!hasInput) return;

    clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      const seq = ++seqRef.current;
      setState((prev) => ({ ...prev, converting: true }));

      const conversions: Promise<[ConvertOutcome, ConvertOutcome, ConvertOutcome]> =
        hasBinary
          ? Promise.all([
              convertBytesStandalone(binaryContent, inputFormat!, "html"),
              convertBytes(binaryContent, inputFormat!, outputFormat),
              parseBytesToJson(binaryContent, inputFormat!),
            ])
          : Promise.all([
              convertStandalone(content!, inputFormat!, "html"),
              convert(content!, inputFormat!, outputFormat),
              parseToJson(content!, inputFormat!),
            ]);

      conversions.then(([previewResult, sourceResult, astResult]) => {
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
  }, [content, binaryContent, inputFormat, outputFormat, hasInput, hasBinary]);

  if (!hasInput) return INITIAL;
  return state;
}
