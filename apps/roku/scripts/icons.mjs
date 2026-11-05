// Draws the Roku app's icons, images/icons/<name>.png, from Lucide (ISC, and MIT for those Lucide took
// from Feather: the icons the Android app and the other TVs draw), and Lucide's licence in
// licenses/lucide.txt. Each is white on transparent, 40 pixels square (Android TV's 20 dp at 1080p),
// and a Poster tints it with its blendColor. Those Android fills (Play, the star) are drawn filled,
// as <name>-fill.png. Chrome draws them, as it draws the other apps' icons (scripts/svg-render.mjs at
// the repository's root). Run again when the list or Lucide changes; the output is committed.
//
//   node scripts/icons.mjs

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { renderer } from "../../../scripts/svg-render.mjs";

const app = join(dirname(fileURLToPath(import.meta.url)), "..");
const root = join(app, "..", "..");
const out = join(app, "images", "icons");
const require = createRequire(join(root, "packages", "tv", "package.json"));
const lucide = dirname(require.resolve("lucide/package.json"));

const SIZE = 40;
// [Lucide's name, filled]: Home's rail (Android TV's RailItems.kt), the hero's and a title's buttons,
// Live TV's, and the star of a rating.
const ICONS = [
  ["Search"], ["House"], ["Compass"], ["ListVideo"], ["TvMinimal"], ["Clapperboard"], ["Tv"],
  ["Music"], ["Film"], ["Layers"], ["ListMusic"], ["AudioLines"], ["Settings"],
  ["Play", true], ["RotateCcw"], ["Info"], ["CircleCheck"],
  ["CalendarDays"], ["LayoutGrid"], ["Clock"], ["Star"], ["Star", true], ["ChevronRight"], ["ChevronDown"],
];

const kebab = (name) => name.replace(/([a-z0-9])([A-Z])/g, "$1-$2").replace(/([A-Za-z])([0-9])/g, "$1-$2").toLowerCase();
const attrs = (o) => Object.entries(o).map(([k, v]) => ` ${k}="${String(v).replace(/"/g, "&quot;")}"`).join("");

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
const all = await import(pathToFileURL(require.resolve("lucide")).href);
const r = await renderer();
try {
  for (const [name, filled] of ICONS) {
    const node = all[name];
    if (!Array.isArray(node)) throw new Error(`Lucide has no ${name}`);
    const body = node.map(([tag, a]) => `<${tag}${attrs(a)}/>`).join("");
    const fill = filled ? "#ffffff" : "none";
    const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="${fill}" stroke="#ffffff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;
    writeFileSync(join(out, `${kebab(name)}${filled ? "-fill" : ""}.png`), await r.png(svg, SIZE));
  }
} finally {
  r.close();
}
writeFileSync(join(app, "licenses", "lucide.txt"), readFileSync(join(lucide, "LICENSE"), "utf8"));
console.log(`${ICONS.length} icons in ${out}`);
