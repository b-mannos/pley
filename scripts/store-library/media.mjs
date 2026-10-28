// Lays out <out>/media (Movies, Shows) from <out>/art, and writes <out>/jobs.txt: one ffmpeg
// job per line, "<image>|<seconds>|<video>", for the videos still to make. Usage: bun media.mjs <out>
import { copyFileSync, existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { movies, shows } from "./catalog.mjs";

const out = process.argv[2];
const art = (n) => join(out, "art", n);
const slug = (s) => s.toLowerCase().replace(/[^a-z0-9]+/g, "-");
const jobs = [];
const job = (image, seconds, video) => existsSync(video) || jobs.push(`${image}|${seconds}|${video}`);

for (const m of movies) {
  const dir = join(out, "media", "Movies", `${m.title} (${m.year})`);
  mkdirSync(dir, { recursive: true });
  copyFileSync(art(`${slug(m.title)}-poster.jpg`), join(dir, "poster.jpg"));
  copyFileSync(art(`${slug(m.title)}-fanart.jpg`), join(dir, "fanart.jpg"));
  job(art(`${slug(m.title)}-fanart.jpg`), m.minutes * 60, join(dir, `${m.title} (${m.year}).mkv`));
}
for (const s of shows) {
  const dir = join(out, "media", "Shows", `${s.title} (${s.year})`);
  mkdirSync(dir, { recursive: true });
  copyFileSync(art(`${slug(s.title)}-poster.jpg`), join(dir, "poster.jpg"));
  copyFileSync(art(`${slug(s.title)}-fanart.jpg`), join(dir, "fanart.jpg"));
  s.seasons.forEach((eps, n) => {
    const sn = String(n + 1).padStart(2, "0");
    copyFileSync(art(`${slug(s.title)}-season${n + 1}.jpg`), join(dir, `season${sn}-poster.jpg`));
    const sdir = join(dir, `Season ${sn}`);
    mkdirSync(sdir, { recursive: true });
    eps.forEach(([name], e) => {
      const en = String(e + 1).padStart(2, "0");
      job(art(`${slug(s.title)}-s${n + 1}e${e + 1}.jpg`), (42 + ((e * 7 + n * 3) % 12)) * 60, join(sdir, `${s.title} - S${sn}E${en} - ${name}.mkv`));
    });
  });
}
writeFileSync(join(out, "jobs.txt"), jobs.join("\n") + "\n");
console.log(`${jobs.length} videos to make`);
