import { useEffect, useRef, useState } from "react";
import {
  convert,
  convertStandalone,
  convertBytes,
  convertBytesStandalone,
  convertWithResources,
  convertToBytes,
  convertBytesToBytes,
  parseToJson,
  parseBytesToJson,
  type ConvertOutcome,
} from "@/wasm/docmux";
import { isBinaryFormat } from "@/lib/formats";

const DEBOUNCE_MS = 200;

export interface ConversionState {
  /** HTML preview (standalone mode for full rendering) */
  preview: string | null;
  /** Source output in the selected target format (null for binary outputs) */
  source: string | null;
  /** Binary output for binary target formats */
  binaryOutput: Uint8Array | null;
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
  binaryOutput: null,
  ast: null,
  errors: [],
  converting: false,
};

export function useConversion(
  content: string | null,
  inputFormat: string | null,
  outputFormat: string,
  binaryContent?: Uint8Array | null,
  resources?: Map<string, Uint8Array> | null,
): ConversionState {
  const [state, setState] = useState<ConversionState>(INITIAL);
  const timerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const seqRef = useRef(0);

  const hasBinary = binaryContent != null && binaryContent.length > 0 && inputFormat !== null;
  const hasText = content !== null && inputFormat !== null;
  const hasInput = hasBinary || hasText;
  const hasResources = resources != null && resources.size > 0;
  const isBinaryOutput = isBinaryFormat(outputFormat);

  useEffect(() => {
    if (!hasInput) return;

    clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => {
      const seq = ++seqRef.current;
      setState((prev) => ({ ...prev, converting: true }));

      const runConversion = async () => {
        const errors: string[] = [];
        let preview: string | null = null;
        let source: string | null = null;
        let binaryOutput: Uint8Array | null = null;
        let ast: string | null = null;

        // Build JS Map for WASM
        const jsResources = new Map<string, Uint8Array>();
        if (hasResources) {
          resources!.forEach((v, k) => jsResources.set(k, v));
        }

        // Preview (always string HTML)
        try {
          let r: ConvertOutcome;
          if (hasBinary) {
            r = await convertBytesStandalone(binaryContent, inputFormat!, "html");
          } else if (hasResources) {
            r = await convertWithResources(content!, inputFormat!, "html", jsResources);
          } else {
            r = await convertStandalone(content!, inputFormat!, "html");
          }
          if (r.error) errors.push(`[preview] ${r.error}`);
          preview = r.output;
        } catch (e) {
          errors.push(`[preview] ${e}`);
        }

        // Source / binary output
        try {
          if (isBinaryOutput) {
            if (hasBinary) {
              binaryOutput = await convertBytesToBytes(binaryContent, inputFormat!, outputFormat, jsResources);
            } else {
              binaryOutput = await convertToBytes(content!, inputFormat!, outputFormat, jsResources);
            }
          } else if (hasBinary) {
            const r = await convertBytes(binaryContent, inputFormat!, outputFormat);
            if (r.error) errors.push(`[source] ${r.error}`);
            source = r.output;
          } else if (hasResources) {
            const r = await convertWithResources(content!, inputFormat!, outputFormat, jsResources);
            if (r.error) errors.push(`[source] ${r.error}`);
            source = r.output;
          } else {
            const r = await convert(content!, inputFormat!, outputFormat);
            if (r.error) errors.push(`[source] ${r.error}`);
            source = r.output;
          }
        } catch (e) {
          errors.push(`[source] ${e}`);
        }

        // AST
        try {
          let r: ConvertOutcome;
          if (hasBinary) {
            r = await parseBytesToJson(binaryContent, inputFormat!);
          } else {
            r = await parseToJson(content!, inputFormat!);
          }
          if (r.error) errors.push(`[ast] ${r.error}`);
          ast = r.output;
        } catch (e) {
          errors.push(`[ast] ${e}`);
        }

        if (seq !== seqRef.current) return;
        setState({ preview, source, binaryOutput, ast, errors, converting: false });
      };

      runConversion();
    }, DEBOUNCE_MS);

    return () => clearTimeout(timerRef.current);
  }, [content, binaryContent, inputFormat, outputFormat, hasInput, hasBinary, hasResources, isBinaryOutput, resources]);

  if (!hasInput) return INITIAL;
  return state;
}
