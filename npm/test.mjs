import { strict as assert } from "node:assert";

// Test the Node.js entry point directly (bypass conditional exports for testing)
import {
  convert as wasmConvert,
  inputFormats,
  outputFormats,
  markdownToHtml,
} from "./node/docmux_wasm.js";

// --- Raw bindings work ---

const html = wasmConvert("# Hello\n\nWorld", "markdown", "html");
assert(html.includes("<h1"), "convert should produce h1");
console.log("  ✓ convert (raw)");

const inputs = inputFormats();
assert(inputs.includes("markdown"), "should include markdown");
assert(inputs.includes("latex"), "should include latex");
assert(inputs.includes("docx"), "should include docx");
console.log("  ✓ inputFormats");

const outputs = outputFormats();
assert(outputs.includes("html"), "should include html");
assert(outputs.includes("latex"), "should include latex");
console.log("  ✓ outputFormats");

// --- Wrapper API works ---

const wrapper = await import("./dist/index.node.js");

const result = await wrapper.convert("**bold**", "markdown", "html");
assert.equal(result.error, null, "should not error");
assert(result.output.includes("<strong>"), "should produce strong tag");
console.log("  ✓ wrapper convert");

const mdResult = await wrapper.markdownToHtml("# Test");
assert.equal(mdResult.error, null);
assert(mdResult.output.includes("<h1"), "markdownToHtml should produce h1");
console.log("  ✓ wrapper markdownToHtml");

const badResult = await wrapper.convert("hello", "nonexistent", "html");
assert(badResult.error !== null, "bad format should return error");
assert.equal(badResult.output, null);
console.log("  ✓ wrapper error handling");

const formats = await wrapper.getInputFormats();
assert(Array.isArray(formats), "should return array");
assert(formats.length > 0, "should have formats");
console.log("  ✓ wrapper getInputFormats");

console.log("\nAll npm package tests passed ✓");
