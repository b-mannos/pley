// Draws the icons of the USB drive's page into images/icons/<name>.png,
// white on clear (the app tints them), from Lucide's icons as the native
// Samsung app has them (apps/tizen-native/src/Pley.Tv.Tizen/shared/res/
// icons, written by its scripts/icons.mjs), and puts Lucide's licence in
// the package (licenses/lucide.txt), which Settings → Open-source
// licences shows. Renders with the canvas brs-node brings, as the smoke
// run does. Run again when the list or those icons change; the output is
// committed.
//
//   node scripts/icons.mjs

import { createRequire } from "node:module";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const { createCanvas, loadImage } = createRequire(require.resolve("brs-node"))("canvas");

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const source = join(root, "..", "tizen-native", "src", "Pley.Tv.Tizen", "shared", "res", "icons");
const out = join(root, "images", "icons");

// A folder, a video, ".." (up a folder), and the page's messages: no drive,
// an empty folder, one that can't be read.
const names = ["folder", "file-video", "corner-left-up", "usb", "folder-open", "circle-alert"];
// Drawn at most this size (a message's icon); smaller ones are scaled down.
const SIZE = 128;

mkdirSync(out, { recursive: true });
for (const name of names) {
  const svg = readFileSync(join(source, `${name}.svg`), "utf8").replace('width="24" height="24"', `width="${SIZE}" height="${SIZE}"`);
  const image = await loadImage(Buffer.from(svg));
  const canvas = createCanvas(SIZE, SIZE);
  canvas.getContext("2d").drawImage(image, 0, 0, SIZE, SIZE);
  writeFileSync(join(out, `${name}.png`), canvas.toBuffer("image/png"));
  console.log(`images/icons/${name}.png`, SIZE, "x", SIZE);
}
const licence = readFileSync(join(source, "LICENSE.txt"), "utf8");
writeFileSync(join(root, "licenses", "lucide.txt"), `Pley for Roku draws icons from Lucide (https://lucide.dev), under this licence:\n\n${licence}`);
console.log("licenses/lucide.txt");
