import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const shared = fileURLToPath(new URL("../../packages/tv/src", import.meta.url));

export default defineConfig({
  esbuild: { jsx: "automatic", jsxImportSource: "preact" },
  resolve: { alias: [{ find: /^pley-tv\/(.*)$/, replacement: `${shared}/$1` }] },
  define: { __PLEY_VERSION__: JSON.stringify("test") },
  test: {
    environment: "jsdom",
    globals: true,
    include: ["test/**/*.test.{ts,tsx}"],
    restoreMocks: true,
    unstubGlobals: true,
  },
});
