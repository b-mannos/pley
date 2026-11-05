// Writes the native Samsung app's fonts, src/Pley.Tv.Tizen/shared/res/fonts/roboto-<weight>.ttf:
// Roboto (OFL-1.1), the face the Android app's type uses, from the same @fontsource/roboto files
// the web TV app ships (latin, weights 400, 500 and 700), and its licence beside them. NUI loads
// TrueType, so each WOFF is unpacked to the font it holds (scripts/woff.mjs); nothing in the font
// changes. Run again when @fontsource/roboto changes; the output is committed.

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { unpack } from "../../../scripts/woff.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const out = join(root, "apps", "tizen-native", "src", "Pley.Tv.Tizen", "shared", "res", "fonts");
const require = createRequire(join(root, "packages", "tv", "package.json"));
const roboto = dirname(require.resolve("@fontsource/roboto/package.json"));

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
for (const weight of [400, 500, 700]) {
  const woff = readFileSync(join(roboto, "files", `roboto-latin-${weight}-normal.woff`));
  writeFileSync(join(out, `roboto-${weight}.ttf`), unpack(woff));
}
writeFileSync(join(out, "LICENSE.txt"), readFileSync(join(roboto, "LICENSE"), "utf8"));
console.log(`Roboto 400, 500 and 700 in ${out}`);
