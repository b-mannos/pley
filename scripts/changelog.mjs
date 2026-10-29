// A unit's CHANGELOG.md, in Keep a Changelog's form
// (https://keepachangelog.com/en/1.1.0/): "## [Unreleased]" on top, then a
// "## [1.2.3] - 2026-10-29" section for each version, newest first, and the
// links the headings name at the end. scripts/release.mjs uses these.

const HEADING = /^## \[?(Unreleased|\d+\.\d+\.\d+)\]?(?:\s+-\s+(\d{4}-\d{2}-\d{2}))?[ \t]*$/gm;
const LINK = /^\[[^\]\n]+\]:[ \t]/m;

/** Every section: { name, date, start (of the heading), bodyStart, end }. */
export function sections(text) {
  const found = [...text.matchAll(HEADING)].map((m) => ({
    name: m[1],
    date: m[2] ?? null,
    start: m.index,
    bodyStart: m.index + m[0].length,
  }));
  const links = text.slice(found.at(-1)?.bodyStart ?? 0).search(LINK);
  const linksAt = links < 0 ? text.length : (found.at(-1)?.bodyStart ?? 0) + links;
  return found.map((s, i) => ({ ...s, end: found[i + 1]?.start ?? linksAt }));
}

/** The section named `name` ("Unreleased" or a version), with its body, or null. */
export function findSection(text, name) {
  const s = sections(text).find((x) => x.name.toLowerCase() === name.toLowerCase());
  return s ? { ...s, body: text.slice(s.bodyStart, s.end).trim() } : null;
}

/** A version's notes: its section's body, as the GitHub release shows them. */
export function notes(text, version) {
  const s = findSection(text, version);
  if (!s) throw new Error(`no section for ${version}`);
  if (!s.body) throw new Error(`the section for ${version} is empty`);
  return s.body;
}

/**
 * The changelog with Unreleased's entries moved under a new version's
 * heading, an empty Unreleased above it, and the headings' links updated.
 */
export function release(text, { version, date, tag, repository }) {
  const unreleased = findSection(text, "Unreleased");
  if (!unreleased) throw new Error('there is no "## [Unreleased]" section');
  if (!unreleased.body) throw new Error("Unreleased is empty");
  if (findSection(text, version)) throw new Error(`there is a section for ${version} already`);
  const moved = `## [Unreleased]\n\n## [${version}] - ${date}\n\n${unreleased.body}\n\n`;
  const out = text.slice(0, unreleased.start) + moved + text.slice(unreleased.end).replace(/^\n+/, "");
  return withLinks(out, version, tag, repository);
}

/** Unreleased compared from the new tag, and the new version's link above the others. */
function withLinks(text, version, tag, repository) {
  const unreleasedLink = `[Unreleased]: ${repository}/compare/${tag}...HEAD`;
  const versionLink = `[${version}]: ${repository}/releases/tag/${tag}`;
  const lines = text.replace(/\n+$/, "").split("\n");
  const at = lines.findIndex((l) => /^\[Unreleased\]:/i.test(l));
  if (at >= 0) lines.splice(at, 1, unreleasedLink, versionLink);
  else lines.push("", unreleasedLink, versionLink);
  return `${lines.join("\n")}\n`;
}

/**
 * The changelog with `entries` added to Unreleased under "### Changed" (made
 * when missing), leaving out those it has already. Returns { text, added }.
 */
export function addEntries(text, entries) {
  const unreleased = findSection(text, "Unreleased");
  if (!unreleased) throw new Error('there is no "## [Unreleased]" section');
  const have = new Set(unreleased.body.split("\n").map((l) => l.replace(/^[-*]\s+/, "").trim().toLowerCase()));
  const added = [...new Set(entries)].filter((e) => !have.has(e.trim().toLowerCase()));
  if (!added.length) return { text, added };
  const bullets = added.map((e) => `- ${e}`).join("\n");
  let body = unreleased.body;
  const changed = /^### Changed[ \t]*$/m.exec(body);
  if (changed) {
    const after = changed.index + changed[0].length;
    const next = body.slice(after).search(/^### /m);
    const end = next < 0 ? body.length : after + next;
    const block = body.slice(changed.index, end).replace(/\s+$/, "");
    body = `${body.slice(0, changed.index)}${block}\n${bullets}${end < body.length ? `\n\n${body.slice(end)}` : ""}`;
  } else {
    body = `${body ? `${body}\n\n` : ""}### Changed\n\n${bullets}`;
  }
  const out = `${text.slice(0, unreleased.bodyStart)}\n\n${body}\n\n${text.slice(unreleased.end).replace(/^\n+/, "")}`;
  return { text: out, added };
}

/** "kodi, roku: Automatic settles" is "Automatic settles"; null for a release commit's subject. */
export function entryOf(subject) {
  if (/^[\w ,/-]+: release \d+\.\d+\.\d+$/.test(subject)) return null;
  const what = subject.replace(/^[\w ,/.-]+:\s+/, "").trim();
  return what ? what[0].toUpperCase() + what.slice(1) : null;
}

/** A section as plain text, for places that show no Markdown (Kodi's add-on news). */
export function plainText(version, date, body) {
  const lines = body
    .replace(/\n[ \t]+(?=\S)/g, " ")
    .split("\n")
    .map((l) => l.replace(/^### (.+)$/, "$1:").replace(/^\* /, "- ").replace(/\[([^\]]+)\]\([^)]+\)/g, "$1").replace(/`|\*\*/g, ""))
    .filter((l, i, all) => l.trim() || (i > 0 && all[i - 1].trim()));
  return `${version} (${date})\n${lines.join("\n").trim()}`;
}
