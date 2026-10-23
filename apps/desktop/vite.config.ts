import { defineConfig } from "vitest/config";

// The server picker the app opens with (src-tauri/tauri.conf.json: frontendDist).
export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { outDir: "dist", target: "es2022", emptyOutDir: true },
  test: { environment: "node" },
});
