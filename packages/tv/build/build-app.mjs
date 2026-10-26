// Builds a TV app from its entry: one script and one stylesheet for the
// TV's old engine, the fonts, and the page that loads them. Everything the
// app needs ships inside the package: no CDN, no remote font, so it works
// on a home network without the internet.

import { build } from "esbuild";
import { copyFileSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { commit } from "../../../scripts/commit.mjs";

const require = createRequire(import.meta.url);

/**
 * @param {{
 *   entry: string,
 *   outdir: string,
 *   target: string,
 *   version: string,
 *   head?: string,
 *   files?: { from: string, to: string }[],
 *   minify?: boolean,
 *   tsconfig?: string,
 * }} o
 */
export async function buildApp(o) {
  rmSync(o.outdir, { recursive: true, force: true });
  mkdirSync(o.outdir, { recursive: true });
  const result = await build({
    entryPoints: { app: o.entry },
    bundle: true,
    outdir: o.outdir,
    format: "iife",
    target: o.target,
    minify: o.minify !== false,
    tsconfig: o.tsconfig,
    sourcemap: false,
    legalComments: "none",
    jsx: "automatic",
    jsxImportSource: "preact",
    // The fonts are copied next to the stylesheet as they are.
    external: ["*.woff2"],
    define: { __PLEY_VERSION__: JSON.stringify(o.version), __PLEY_COMMIT__: JSON.stringify(commit()) },
    logLevel: "warning",
    metafile: true,
  });
  const fonts = join(dirname(require.resolve("@fontsource/roboto/package.json")), "files");
  mkdirSync(join(o.outdir, "fonts"), { recursive: true });
  for (const weight of [400, 500, 700]) {
    const name = `roboto-latin-${weight}-normal.woff2`;
    copyFileSync(join(fonts, name), join(o.outdir, "fonts", name));
  }
  for (const f of o.files ?? []) {
    mkdirSync(dirname(join(o.outdir, f.to)), { recursive: true });
    copyFileSync(f.from, join(o.outdir, f.to));
  }
  writeFileSync(
    join(o.outdir, "index.html"),
    [
      "<!DOCTYPE html>",
      '<html lang="en">',
      "<head>",
      '<meta charset="utf-8">',
      '<meta name="viewport" content="width=1920, height=1080, initial-scale=1">',
      "<title>pley</title>",
      '<link rel="stylesheet" href="app.css">',
      o.head ?? "",
      "</head>",
      "<body>",
      '<div id="app"></div>',
      '<script src="app.js"></script>',
      "</body>",
      "</html>",
      "",
    ]
      .filter((l) => l !== "")
      .join("\n") + "\n",
  );
  const bytes = Object.values(result.metafile.outputs).reduce((n, out) => n + out.bytes, 0);
  return { bytes };
}

/** The app's version, from its package.json. */
export function versionOf(packageJson) {
  return JSON.parse(readFileSync(packageJson, "utf8")).version;
}
