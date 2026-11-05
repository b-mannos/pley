// Writes the Roku app's fonts, fonts/roboto-<weight>.ttf: Roboto (OFL-1.1), the face of the Android
// app and the other TVs, from the same @fontsource/roboto files the web TV app ships (latin, weights
// 400, 500 and 700), unpacked to the TrueType Roku's Font nodes load (scripts/woff.mjs at the
// repository's root), and its licence in licenses/roboto.txt. Run again when @fontsource/roboto
// changes; the output is committed. Roboto's Latin files lack other scripts and Roku draws a label in
// one font, so components/lib/Type.brs knows which characters they have (typeCovers): change it with
// them.
//
//   node scripts/fonts.mjs

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { unpack } from "../../../scripts/woff.mjs";

const app = join(dirname(fileURLToPath(import.meta.url)), "..");
const root = join(app, "..", "..");
const out = join(app, "fonts");
const require = createRequire(join(root, "packages", "tv", "package.json"));
const roboto = dirname(require.resolve("@fontsource/roboto/package.json"));

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
for (const weight of [400, 500, 700]) {
  const woff = readFileSync(join(roboto, "files", `roboto-latin-${weight}-normal.woff`));
  writeFileSync(join(out, `roboto-${weight}.ttf`), unpack(woff));
}
writeFileSync(join(app, "licenses", "roboto.txt"), readFileSync(join(roboto, "LICENSE"), "utf8"));
console.log(`Roboto 400, 500 and 700 in ${out}`);
