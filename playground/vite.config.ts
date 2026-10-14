import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import wasm from "vite-plugin-wasm";
import path from "path";

export default defineConfig({
  plugins: [wasm(), react(), tailwindcss()],
  build: {
    rollupOptions: {
      input: {
        landing: path.resolve(__dirname, "index.html"),
        playground: path.resolve(__dirname, "playground/index.html"),
      },
    },
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
