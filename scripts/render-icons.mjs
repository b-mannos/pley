#!/usr/bin/env bun
// Renders the icons the web app, the desktop app and the TV apps (Samsung,
// LG, Kodi, Roku) ship from the logo (apps/web/public/favicon.svg), through
// a headless Chrome or Edge:
//
//   apps/web/public/icon-maskable-512.png   full bleed, the P as big as the native
//                                           Android app's once Chrome masks it
//   apps/web/public/favicon-32.png          for browsers without SVG favicons
//   apps/desktop/src-tauri/icons/icon.ico   32 px first: Windows takes the first
//                                           for the window, the taskbar and Alt-Tab
//   apps/desktop/src-tauri/icons/icon.icns  on Apple's grid (an 824 plate in 1024,
//                                           a soft shadow), as macOS draws its apps
//   apps/tizen/assets/icon.png              Samsung's 512×423, full bleed, as a
//                                           24-bit RGB PNG at 72 dpi (Samsung's spec)
//   apps/webos/assets/icon.png, largeIcon.png
//                                           LG's 80 and 130 px, full bleed on the
//                                           tile's colour, which appinfo.json's
//                                           iconColor gives the TV's own tile
//   apps/webos/assets/splash.png            LG's launch picture, 1920×1080
//   apps/kodi/plugin.video.pley/resources/icon.png
//                                           512 square, the tile's colour out to the
//                                           corners: Kodi's add-on checker refuses
//                                           transparency
//   apps/kodi/plugin.video.pley/resources/fanart.jpg
//                                           1920×1080, the TV banner's picture
//   apps/roku/images/icon_focus_{fhd,hd}.png
//                                           540×405 and 290×218, Roku's home screen
//                                           tiles, as the Android TV app's banner
//   apps/roku/images/splash_{fhd,hd,sd}.png the logo on the Roku app's background
//   apps/android/app/src/main/res/drawable-{xhdpi,xxhdpi,xxxhdpi}/tv_banner.png
//                                           the Android TV app's banner, 320×180 dp,
//                                           on Android TV's home screen
//   deploy/windows/setup-small-*.png        the logo in the corner of the Windows
//                                           installer's pages, inset in the square
//                                           Inno Setup gives it, one per DPI
//   deploy/stores/                          the stores' listing pictures (README.md
//                                           there): icons, Play's feature graphic,
//                                           the TV banner, Fire TV's background
//
// Chrome comes from $CHROME, else the usual place on Windows, macOS or Linux
// (svg-render.mjs).
// The banner's wordmark is in the system's bold sans (Roboto, else Segoe UI).
//
//   bun scripts/render-icons.mjs

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { crc32, deflateSync, inflateSync } from "node:zlib";
import { renderer } from "./svg-render.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const logo = readFileSync(join(root, "apps/web/public/favicon.svg"), "utf8");
const defs = logo.match(/<defs>[\s\S]*<\/defs>/)[0];
const glyph = logo.match(/<path[\s\S]*?\/>/)[0];
const TILE = "#0a1622";

/** The P alone on the tile's colour to the edges, scaled [s] about the 64-unit logo's centre. */
function maskable(size, s) {
  const t = size / 2 - 32 * s;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}">${defs}<rect width="${size}" height="${size}" fill="${TILE}"/><g transform="translate(${t} ${t}) scale(${s})">${glyph}</g></svg>`;
}

/** The P on the tile's colour to the edges of a width × height picture, the 64-unit logo at [s] with its top left at (x, y). */
function onTile(width, height, s, x, y) {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}">${defs}<rect width="${width}" height="${height}" fill="${TILE}"/><g transform="translate(${x} ${y}) scale(${s})">${glyph}</g></svg>`;
}

/**
 * A TV home screen's tile, as the Android TV app's banner (320 × 180): the P in its gradient and the
 * wordmark on the tile's colour, the brand's deep blue glowing from the lower right corner. Other sizes
 * get the same picture scaled to fit, the colours out to the edges.
 */
function banner(width, height) {
  const s = Math.min(width / 320, height / 180);
  const g = 81 / 41; // the P 81 high, from (68, 54)
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}">${defs}
  <radialGradient id="corner" cx="${width}" cy="${height}" r="${width * 0.52}" gradientUnits="userSpaceOnUse">
    <stop offset="0" stop-color="#0369a1" stop-opacity="0.3"/><stop offset="1" stop-color="#0369a1" stop-opacity="0"/>
  </radialGradient>
  <rect width="${width}" height="${height}" fill="${TILE}"/><rect width="${width}" height="${height}" fill="url(#corner)"/>
  <g transform="translate(${(width - 320 * s) / 2} ${(height - 180 * s) / 2}) scale(${s})">
    <g transform="translate(${68 - 17 * g} ${54 - 14 * g}) scale(${g})">${glyph}</g>
    <text x="160" y="107" font-family="Roboto, 'Segoe UI', system-ui, sans-serif" font-size="63" font-weight="700" letter-spacing="-1" fill="#e9ebef">Pley</text>
  </g></svg>`;
}

/**
 * The Amazon Appstore's Fire TV background, 1920 × 1080: the store's words and buttons cover its left
 * and bottom, so the P stands alone in the top right (the 882 × 448 Amazon keeps clear), on the banner's
 * colours.
 */
function fireTvBackground() {
  const s = 300 / 41; // the P 300 high, its middle (33, 34.5) at (1480, 260)
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1920 1080">${defs}
  <radialGradient id="corner" cx="1920" cy="0" r="1100" gradientUnits="userSpaceOnUse">
    <stop offset="0" stop-color="#0369a1" stop-opacity="0.3"/><stop offset="1" stop-color="#0369a1" stop-opacity="0"/>
  </radialGradient>
  <rect width="1920" height="1080" fill="${TILE}"/><rect width="1920" height="1080" fill="url(#corner)"/>
  <g transform="translate(${1480 - 33 * s} ${260 - 34.5 * s}) scale(${s})">${glyph}</g></svg>`;
}

/** The logo [inset] of a [size] square's side, in its middle, transparent around it. */
function inset(size, inset) {
  const side = size * inset;
  const placed = logo.replace("<svg", `<svg x="${(size - side) / 2}" y="${(size - side) / 2}" width="${side}" height="${side}"`);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${size} ${size}">${placed}</svg>`;
}

/** A splash screen: the logo, a fifth of the height, in the middle of the app's background [bg]. */
function splash(width, height, bg) {
  const size = height / 5;
  const placed = logo.replace("<svg", `<svg x="${(width - size) / 2}" y="${(height - size) / 2}" width="${size}" height="${size}"`);
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${width} ${height}"><rect width="${width}" height="${height}" fill="${bg}"/>${placed}</svg>`;
}

/** Apple's macOS grid: the plate 824 of 1024 at (100, 100), corners about 185, a shadow 10 down. */
const MAC = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">${defs}
  <filter id="shadow" x="-10%" y="-10%" width="120%" height="125%"><feDropShadow dx="0" dy="10" stdDeviation="10" flood-color="#000" flood-opacity="0.3"/></filter>
  <g filter="url(#shadow)"><g transform="translate(100 100) scale(12.875)"><rect width="64" height="64" rx="14.4" fill="${TILE}"/>${glyph}</g></g></svg>`;

/** A .ico of PNG entries, in the order given. */
function ico(images) {
  const head = Buffer.alloc(6 + 16 * images.length);
  head.writeUInt16LE(1, 2);
  head.writeUInt16LE(images.length, 4);
  let offset = head.length;
  images.forEach(({ size, png }, i) => {
    const at = 6 + 16 * i;
    head.writeUInt8(size >= 256 ? 0 : size, at);
    head.writeUInt8(size >= 256 ? 0 : size, at + 1);
    head.writeUInt16LE(1, at + 4);
    head.writeUInt16LE(32, at + 6);
    head.writeUInt32LE(png.length, at + 8);
    head.writeUInt32LE(offset, at + 12);
    offset += png.length;
  });
  return Buffer.concat([head, ...images.map((i) => i.png)]);
}

/** A .icns of PNG chunks. */
function icns(chunks) {
  const parts = chunks.map(({ type, png }) => {
    const head = Buffer.alloc(8);
    head.write(type, 0, "ascii");
    head.writeUInt32BE(png.length + 8, 4);
    return Buffer.concat([head, png]);
  });
  const body = Buffer.concat(parts);
  const head = Buffer.alloc(8);
  head.write("icns", 0, "ascii");
  head.writeUInt32BE(body.length + 8, 4);
  return Buffer.concat([head, body]);
}

/**
 * An opaque PNG from the browser (8-bit RGB or RGBA) again as a 24-bit RGB
 * PNG marked 72 dpi, as Samsung asks for its icons. Refuses a picture with
 * any pixel that isn't opaque, rather than dropping it.
 */
function rgb72(png) {
  let at = 8;
  let width = 0;
  let height = 0;
  let channels = 0;
  const data = [];
  while (at < png.length) {
    const length = png.readUInt32BE(at);
    const type = png.toString("ascii", at + 4, at + 8);
    const body = png.subarray(at + 8, at + 8 + length);
    if (type === "IHDR") {
      width = body.readUInt32BE(0);
      height = body.readUInt32BE(4);
      if (body[8] !== 8 || body[12] !== 0 || (body[9] !== 2 && body[9] !== 6)) throw new Error("expected an 8-bit RGB or RGBA PNG, not interlaced");
      channels = body[9] === 6 ? 4 : 3;
    }
    if (type === "IDAT") data.push(body);
    at += 12 + length;
  }
  const raw = inflateSync(Buffer.concat(data));
  const stride = width * channels;
  const pixels = Buffer.alloc(stride * height);
  // Each row undoes its filter (PNG has five), from the rows above it.
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    for (let x = 0; x < stride; x++) {
      const left = x >= channels ? pixels[y * stride + x - channels] : 0;
      const up = y > 0 ? pixels[(y - 1) * stride + x] : 0;
      const corner = x >= channels && y > 0 ? pixels[(y - 1) * stride + x - channels] : 0;
      const p = left + up - corner;
      const [dl, du, dc] = [Math.abs(p - left), Math.abs(p - up), Math.abs(p - corner)];
      const paeth = dl <= du && dl <= dc ? left : du <= dc ? up : corner;
      const predicted = [0, left, up, (left + up) >> 1, paeth][filter];
      pixels[y * stride + x] = (raw[y * (stride + 1) + 1 + x] + predicted) & 255;
    }
  }
  if (channels === 4) {
    for (let i = 3; i < pixels.length; i += 4) if (pixels[i] !== 255) throw new Error("a picture meant to be opaque has see-through pixels");
  }
  // RGB rows, each as differences from the pixel on its left (filter 1), which suits flat colour.
  const rowBytes = width * 3 + 1;
  const rows = Buffer.alloc(rowBytes * height);
  for (let y = 0; y < height; y++) {
    rows[y * rowBytes] = 1;
    for (let x = 0; x < width; x++) {
      for (let c = 0; c < 3; c++) {
        const v = pixels[y * stride + x * channels + c];
        const left = x > 0 ? pixels[y * stride + (x - 1) * channels + c] : 0;
        rows[y * rowBytes + 1 + x * 3 + c] = (v - left) & 255;
      }
    }
  }
  const chunk = (type, body) => {
    const head = Buffer.alloc(8);
    head.writeUInt32BE(body.length, 0);
    head.write(type, 4, "ascii");
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(Buffer.concat([head.subarray(4), body])), 0);
    return Buffer.concat([head, body, crc]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0);
  ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  // 72 dpi: 2835 pixels a metre.
  const phys = Buffer.alloc(9);
  phys.writeUInt32BE(2835, 0);
  phys.writeUInt32BE(2835, 4);
  phys[8] = 1;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("pHYs", phys),
    chunk("IDAT", deflateSync(rows, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

const r = await renderer();
try {
  const web = join(root, "apps/web/public");
  // Chrome fits the 80 % safe circle to Android's 66 dp safe zone: at 7.449 the P is the native app's 49 dp.
  writeFileSync(join(web, "icon-maskable-512.png"), await r.png(maskable(512, 7.449), 512));
  writeFileSync(join(web, "favicon-32.png"), await r.png(logo, 32));

  const icons = join(root, "apps/desktop/src-tauri/icons");
  const windows = [];
  for (const size of [32, 16, 20, 24, 40, 48, 64, 128, 256]) windows.push({ size, png: await r.png(logo, size) });
  writeFileSync(join(icons, "icon.ico"), ico(windows));

  const mac = {};
  for (const size of [16, 32, 64, 128, 256, 512, 1024]) mac[size] = await r.png(MAC, size);
  writeFileSync(
    join(icons, "icon.icns"),
    icns([
      { type: "icp4", png: mac[16] },
      { type: "icp5", png: mac[32] },
      { type: "icp6", png: mac[64] },
      { type: "ic07", png: mac[128] },
      { type: "ic08", png: mac[256] },
      { type: "ic09", png: mac[512] },
      { type: "ic10", png: mac[1024] },
      { type: "ic11", png: mac[32] },
      { type: "ic12", png: mac[64] },
      { type: "ic13", png: mac[256] },
      { type: "ic14", png: mac[512] },
    ]),
  );

  // Samsung's 512×423: the P where a 360 px logo centred in it puts it, the tile's colour to the edges.
  const tizen = join(root, "apps/tizen/assets");
  writeFileSync(join(tizen, "icon.png"), rgb72(await r.png(onTile(512, 423, 360 / 64, 76, 31.5), 512, 423)));

  // LG's: the logo at full size without its rounded corners, on the colour appinfo.json's iconColor
  // gives the TV's tile; and the launch picture with the P's own middle (33, 34.5 of the logo's 64)
  // in the middle, 205 px tall.
  const webos = join(root, "apps/webos/assets");
  writeFileSync(join(webos, "icon.png"), rgb72(await r.png(maskable(80, 80 / 64), 80)));
  writeFileSync(join(webos, "largeIcon.png"), rgb72(await r.png(maskable(130, 130 / 64), 130)));
  writeFileSync(join(webos, "splash.png"), rgb72(await r.png(onTile(1920, 1080, 5, 960 - 33 * 5, 540 - 34.5 * 5), 1920, 1080)));

  // Kodi's: the favicon's P where it is, without the rounded corners; the fanart a JPEG (Kodi's
  // checker takes up to 1000 KB).
  const kodi = join(root, "apps/kodi/plugin.video.pley/resources");
  writeFileSync(join(kodi, "icon.png"), rgb72(await r.png(maskable(512, 8), 512)));
  writeFileSync(join(kodi, "fanart.jpg"), await r.jpeg(banner(1920, 1080), 1920, 1080, 90));

  // Roku's home screen tiles, and splash screens on the app's background (components/shared/Ui.brs),
  // as the manifest's splash_color.
  const roku = join(root, "apps/roku/images");
  writeFileSync(join(roku, "icon_focus_fhd.png"), rgb72(await r.png(banner(540, 405), 540, 405)));
  writeFileSync(join(roku, "icon_focus_hd.png"), rgb72(await r.png(banner(290, 218), 290, 218)));
  for (const [name, width, height] of [["splash_fhd.png", 1920, 1080], ["splash_hd.png", 1280, 720], ["splash_sd.png", 720, 480]]) {
    writeFileSync(join(roku, name), rgb72(await r.png(splash(width, height, "#16191e"), width, height)));
  }
  // The Android TV app's banner, sharp on a 4K TV's home screen too.
  const res = join(root, "apps/android/app/src/main/res");
  for (const [density, scale] of [["xhdpi", 1], ["xxhdpi", 1.5], ["xxxhdpi", 2]]) {
    const [width, height] = [320 * scale, 180 * scale];
    mkdirSync(join(res, `drawable-${density}`), { recursive: true });
    writeFileSync(join(res, `drawable-${density}`, "tv_banner.png"), rgb72(await r.png(banner(width, height), width, height)));
  }
  // The stores': Google Play rounds the square icon's corners itself, so the P sits as on the
  // Android app's adaptive icon; the feature graphic and TV banner are the banner, opaque.
  const stores = join(root, "deploy/stores");
  writeFileSync(join(stores, "icon-512.png"), await r.png(maskable(512, 7.449), 512));
  writeFileSync(join(stores, "icon-114.png"), rgb72(await r.png(maskable(114, (114 / 512) * 7.449), 114)));
  writeFileSync(join(stores, "feature-graphic-1024x500.png"), rgb72(await r.png(banner(1024, 500), 1024, 500)));
  writeFileSync(join(stores, "tv-banner-1280x720.png"), rgb72(await r.png(banner(1280, 720), 1280, 720)));
  writeFileSync(join(stores, "fire-tv-background-1920x1080.jpg"), await r.jpeg(fireTvBackground(), 1920, 1080, 90));
  // Inno Setup's square in the top right of the installer's pages, at 100 to 250 % (its help's
  // WizardSmallImageFile): the logo flush in it would touch the window's edge, so it's inset.
  const setup = join(root, "deploy/windows");
  for (const size of [58, 77, 97, 116, 124, 143, 159]) writeFileSync(join(setup, `setup-small-${size}.png`), await r.png(inset(size, 0.7), size));
  console.log("rendered the web's maskable icon and PNG favicon, the desktop's .ico and .icns, the Samsung and LG apps' icons and LG's splash, Kodi's icon and fanart, Roku's icons and splash screens, the Android TV banner, the stores' pictures, and the Windows installer's");
} finally {
  r.close();
}
