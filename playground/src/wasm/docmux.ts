import init, {
  convert as wasmConvert,
  convertStandalone as wasmConvertStandalone,
  convertBytes as wasmConvertBytes,
  convertBytesStandalone as wasmConvertBytesStandalone,
  parseToJson as wasmParseToJson,
  parseBytesToJson as wasmParseBytesToJson,
  inputFormats as wasmInputFormats,
  outputFormats as wasmOutputFormats,
} from "../../wasm-pkg/docmux_wasm.js";

let initPromise: Promise<void> | null = null;

function ensureInit(): Promise<void> {
  if (!initPromise) {
    initPromise = init().then(() => undefined);
  }
  return initPromise;
}

export interface ConversionResult {
  output: string;
  error: null;
}

export interface ConversionError {
  output: null;
  error: string;
}

export type ConvertOutcome = ConversionResult | ConversionError;

async function callWasm<Args extends unknown[]>(
  fn: (...args: Args) => string,
  ...args: Args
): Promise<ConvertOutcome> {
  await ensureInit();
  try {
    return { output: fn(...args), error: null };
  } catch (e) {
    return { output: null, error: String(e) };
  }
}

export function convert(input: string, from: string, to: string): Promise<ConvertOutcome> {
  return callWasm(wasmConvert, input, from, to);
}

export function convertStandalone(input: string, from: string, to: string): Promise<ConvertOutcome> {
  return callWasm(wasmConvertStandalone, input, from, to);
}

export function parseToJson(input: string, from: string): Promise<ConvertOutcome> {
  return callWasm(wasmParseToJson, input, from);
}

export function convertBytes(input: Uint8Array, from: string, to: string): Promise<ConvertOutcome> {
  return callWasm(wasmConvertBytes, input, from, to);
}

export function convertBytesStandalone(input: Uint8Array, from: string, to: string): Promise<ConvertOutcome> {
  return callWasm(wasmConvertBytesStandalone, input, from, to);
}

export function parseBytesToJson(input: Uint8Array, from: string): Promise<ConvertOutcome> {
  return callWasm(wasmParseBytesToJson, input, from);
}

export async function getInputFormats(): Promise<string[]> {
  await ensureInit();
  return wasmInputFormats();
}

export async function getOutputFormats(): Promise<string[]> {
  await ensureInit();
  return wasmOutputFormats();
}
