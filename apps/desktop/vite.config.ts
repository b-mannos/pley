import { defineConfig } from "vitest/config";

// The server picker the app opens with (src-tauri/tauri.conf.json: frontendDist).
export default defineConfig({
  clearScreen: false,
  // The Rust side builds under src-tauri, files Windows locks while they are
  // written: watching them stops the dev server (EBUSY).
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: { outDir: "dist", target: "es2022", emptyOutDir: true },
  test: { environment: "node" },
});
