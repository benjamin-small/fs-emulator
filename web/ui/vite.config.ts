import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

// GitHub Pages serves the app under /fs-emulator/; the Pages workflow sets VITE_BASE.
export default defineConfig({
  base: process.env.VITE_BASE ?? "/",
  plugins: [svelte(), wasm(), topLevelAwait()],
  build: { target: "esnext" },
  // Both packages load their .wasm via `new URL(..., import.meta.url)`; pre-bundling would break that.
  optimizeDeps: { exclude: ["fs-emulator-wasm", "@benjamin-small/browser-terminal"] },
});
