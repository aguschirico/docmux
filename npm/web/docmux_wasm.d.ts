/* tslint:disable */
/* eslint-disable */

/**
 * Convert a document from one format to another (fragment mode).
 *
 * # Arguments
 * - `input` — the source document as a string
 * - `from` — input format name or extension (e.g. `"markdown"`, `"md"`)
 * - `to` — output format name or extension (e.g. `"html"`)
 */
export function convert(input: string, from: string, to: string): string;

/**
 * Convert binary input (e.g. DOCX bytes) to another format (fragment mode).
 *
 * # Arguments
 * - `input` — the raw binary content (e.g. DOCX bytes from `FileReader`)
 * - `from` — input format name or extension (e.g. `"docx"`)
 * - `to`   — output format name or extension (e.g. `"html"`)
 */
export function convertBytes(input: Uint8Array, from: string, to: string): string;

/**
 * Convert binary input producing a standalone file (full HTML, LaTeX with preamble, etc.).
 */
export function convertBytesStandalone(input: Uint8Array, from: string, to: string): string;

/**
 * Convert binary input to binary output (e.g. DOCX → DOCX), with additional resources.
 */
export function convertBytesToBytes(input: Uint8Array, from: string, to: string, resources: Map<any, any>): Uint8Array;

/**
 * Convert a document producing a standalone file (full HTML document, LaTeX with preamble, etc.).
 */
export function convertStandalone(input: string, from: string, to: string): string;

/**
 * Convert text input to binary output (e.g. markdown → DOCX), with image resources.
 */
export function convertToBytes(input: string, from: string, to: string, resources: Map<any, any>): Uint8Array;

/**
 * Convert text input to string output, with image resources for embedding.
 */
export function convertWithResources(input: string, from: string, to: string, resources: Map<any, any>): string;

/**
 * Return a list of supported input format names.
 */
export function inputFormats(): string[];

/**
 * Convert markdown to HTML (convenience wrapper).
 */
export function markdownToHtml(input: string): string;

/**
 * Return a list of supported output format names.
 */
export function outputFormats(): string[];

/**
 * Parse binary input and return the AST as pretty-printed JSON.
 */
export function parseBytesToJson(input: Uint8Array, from: string): string;

/**
 * Parse a document and return the AST as pretty-printed JSON.
 */
export function parseToJson(input: string, from: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly convert: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly convertBytes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly convertBytesStandalone: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly convertBytesToBytes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly convertStandalone: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly convertToBytes: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly convertWithResources: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number) => void;
    readonly inputFormats: (a: number) => void;
    readonly markdownToHtml: (a: number, b: number, c: number) => void;
    readonly outputFormats: (a: number) => void;
    readonly parseBytesToJson: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly parseToJson: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly __wasm_bindgen_func_elem_312: (a: number, b: number, c: number, d: number) => void;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export3: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
