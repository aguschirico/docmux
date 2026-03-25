import init, {
  convert as wasmConvert,
  convertStandalone as wasmConvertStandalone,
  parseToJson as wasmParseToJson,
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

export async function convert(
  input: string,
  from: string,
  to: string,
): Promise<ConvertOutcome> {
  await ensureInit();
  try {
    return { output: wasmConvert(input, from, to), error: null };
  } catch (e) {
    return { output: null, error: String(e) };
  }
}

export async function convertStandalone(
  input: string,
  from: string,
  to: string,
): Promise<ConvertOutcome> {
  await ensureInit();
  try {
    return { output: wasmConvertStandalone(input, from, to), error: null };
  } catch (e) {
    return { output: null, error: String(e) };
  }
}

export async function parseToJson(
  input: string,
  from: string,
): Promise<ConvertOutcome> {
  await ensureInit();
  try {
    return { output: wasmParseToJson(input, from), error: null };
  } catch (e) {
    return { output: null, error: String(e) };
  }
}

export async function getInputFormats(): Promise<string[]> {
  await ensureInit();
  return wasmInputFormats();
}

export async function getOutputFormats(): Promise<string[]> {
  await ensureInit();
  return wasmOutputFormats();
}
