import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import wasm from "vite-plugin-wasm";
import path from "path";

export default defineConfig({
  plugins: [wasm(), react(), tailwindcss()],
  resolve: {
    // @docx-editor.dev is hoisted to the workspace root; without this it
    // resolves a second copy of React from there.
    dedupe: ["react", "react-dom"],
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
