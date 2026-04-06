import {
  convert as wasmConvert,
  convertStandalone as wasmConvertStandalone,
  convertBytes as wasmConvertBytes,
  convertBytesStandalone as wasmConvertBytesStandalone,
  parseToJson as wasmParseToJson,
  parseBytesToJson as wasmParseBytesToJson,
  markdownToHtml as wasmMarkdownToHtml,
  inputFormats as wasmInputFormats,
  outputFormats as wasmOutputFormats,
} from "../bundler/docmux_wasm.js";

// -- Types -------------------------------------------------------------------

export interface ConversionResult {
  output: string;
  error: null;
}

export interface ConversionError {
  output: null;
  error: string;
}

export type ConvertOutcome = ConversionResult | ConversionError;

// -- Internal helper ---------------------------------------------------------

function callWasm<Args extends unknown[]>(
  fn: (...args: Args) => string,
  ...args: Args
): ConvertOutcome {
  try {
    return { output: fn(...args), error: null };
  } catch (e: unknown) {
    return { output: null, error: String(e) };
  }
}

// -- Public API --------------------------------------------------------------

export function convert(
  input: string,
  from: string,
  to: string,
): ConvertOutcome {
  return callWasm(wasmConvert, input, from, to);
}

export function convertStandalone(
  input: string,
  from: string,
  to: string,
): ConvertOutcome {
  return callWasm(wasmConvertStandalone, input, from, to);
}

export function parseToJson(
  input: string,
  from: string,
): ConvertOutcome {
  return callWasm(wasmParseToJson, input, from);
}

export function convertBytes(
  input: Uint8Array,
  from: string,
  to: string,
): ConvertOutcome {
  return callWasm(wasmConvertBytes, input, from, to);
}

export function convertBytesStandalone(
  input: Uint8Array,
  from: string,
  to: string,
): ConvertOutcome {
  return callWasm(wasmConvertBytesStandalone, input, from, to);
}

export function parseBytesToJson(
  input: Uint8Array,
  from: string,
): ConvertOutcome {
  return callWasm(wasmParseBytesToJson, input, from);
}

export function markdownToHtml(input: string): ConvertOutcome {
  return callWasm(wasmMarkdownToHtml, input);
}

export function getInputFormats(): string[] {
  return wasmInputFormats();
}

export function getOutputFormats(): string[] {
  return wasmOutputFormats();
}
