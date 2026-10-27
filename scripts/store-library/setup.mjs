// Sets up the screenshot server: the account, libraries, the made-up metadata, what's being
// watched and My List. Usage: node setup.mjs <base url> <setup code> <out dir>
import { randomBytes } from "node:crypto";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { movies, shows } from "./catalog.mjs";

const [base, code, out] = process.argv.slice(2);
let token = "";
async function api(method, path, body) {
  const res = await fetch(base + path, {
    method,
    headers: { "content-type": "application/json", ...(token ? { authorization: `Bearer ${token}` } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const text = await res.text();
  if (!res.ok) throw new Error(`${method} ${path}: ${res.status} ${text}`);
  return text ? JSON.parse(text) : null;
}
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const password = randomBytes(18).toString("base64url");
const setup = await api("POST", "/api/setup", { username: "alex", password, setup_code: code });
token = setup.token;
writeFileSync(join(out, "admin.json"), JSON.stringify({ username: "alex", password, token }));
await api("PATCH", "/api/me", { display_name: "Alex" });
await api("PUT", "/api/settings", { server_name: "Home", music_metadata: false, detect_markers: false, seek_previews: false });

const libs = {
  movies: await api("POST", "/api/libraries", { name: "Movies", kind: "movies", paths: [join(out, "media", "Movies")] }),
  shows: await api("POST", "/api/libraries", { name: "TV Shows", kind: "shows", paths: [join(out, "media", "Shows")] }),
};
for (;;) {
  await sleep(2000);
  const all = await api("GET", "/api/libraries");
  if (all.every((l) => !l.scanning && l.scanned_at)) break;
}
const items = async (lib) => (await api("GET", `/api/libraries/${lib.id}/items?limit=200`)).items;
const children = async (id) => (await api("GET", `/api/items/${id}/children`)).items;

const studios = ["Lantern Pictures", "Northlight Films", "Tidewater Studios", "Blue Hour Media"];
const day = (y, i) => `${y}-${String(1 + (i * 5) % 12).padStart(2, "0")}-${String(3 + (i * 7) % 25).padStart(2, "0")}`;
const movieItems = await items(libs.movies);
for (const [i, m] of movies.entries()) {
  const it = movieItems.find((x) => x.title === m.title);
  if (!it) throw new Error(`no movie ${m.title}`);
  m.id = it.id;
  await api("PATCH", `/api/items/${it.id}`, { summary: m.summary, tagline: m.tagline, genres: m.genres, content_rating: m.rating, studio: studios[i % 4], release_date: day(m.year, i) });
}
const showItems = await items(libs.shows);
for (const [i, s] of shows.entries()) {
  const it = showItems.find((x) => x.title === s.title);
  if (!it) throw new Error(`no show ${s.title}`);
  s.id = it.id;
  await api("PATCH", `/api/items/${it.id}`, { summary: s.summary, genres: s.genres, content_rating: s.rating, studio: studios[(i + 2) % 4], release_date: day(s.year, i) });
  const seasons = await children(it.id);
  s.episodeIds = [];
  for (const [n, season] of seasons.entries()) {
    await api("PATCH", `/api/items/${season.id}`, { summary: `Season ${n + 1} of ${s.title}.` });
    const eps = await children(season.id);
    for (const [e, ep] of eps.entries()) {
      const [name, summary] = s.seasons[n][e];
      await api("PATCH", `/api/items/${ep.id}`, { title: name, summary, release_date: day(s.year + n, e) });
      s.episodeIds.push(ep.id);
    }
  }
}

// What Alex is in the middle of, has seen, and wants to see.
const progress = async (id, minutes, fraction) => api("POST", `/api/items/${id}/progress`, { position_ms: Math.round(minutes * 60000 * fraction), duration_ms: minutes * 60000, state: "stopped" });
const byTitle = Object.fromEntries(movies.map((m) => [m.title, m]));
await progress(byTitle["Neon Rain"].id, byTitle["Neon Rain"].minutes, 0.42);
await progress(byTitle["Northern Lights"].id, byTitle["Northern Lights"].minutes, 0.68);
await progress(byTitle["Paper Moons"].id, byTitle["Paper Moons"].minutes, 0.18);
const harbor = shows.find((s) => s.title === "Harbor Town");
for (const id of harbor.episodeIds.slice(0, 2)) await api("POST", `/api/items/${id}/watched`);
await progress(harbor.episodeIds[2], 47, 0.55);
const starfall = shows.find((s) => s.title === "Starfall");
for (const id of starfall.episodeIds.slice(0, 5)) await api("POST", `/api/items/${id}/watched`);
await progress(starfall.episodeIds[5], 45, 0.3);
for (const t of ["Saltwater", "Red Desert", "Deep Blue"]) await api("POST", `/api/items/${byTitle[t].id}/watched`);
for (const id of [byTitle["Orbit"].id, byTitle["The Night Train"].id, shows.find((s) => s.title === "The Glasshouse").id, byTitle["Golden Hour"].id, byTitle["Signal Lost"].id]) await api("PUT", `/api/watchlist/${id}`);
writeFileSync(join(out, "ids.json"), JSON.stringify({ movies: movies.map(({ title, id }) => ({ title, id })), shows: shows.map(({ title, id, episodeIds }) => ({ title, id, episodeIds })) }, null, 1));
console.log("set up", movieItems.length, "movies and", showItems.length, "shows");
