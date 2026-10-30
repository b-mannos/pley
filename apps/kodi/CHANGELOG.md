# Changelog

Notable changes to Pley for Kodi, the add-on `plugin.video.pley`, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Releases are tagged `kodi-v<version>`; [docs/RELEASING.md](../../docs/RELEASING.md)
describes how they are made.

## [Unreleased]

### Added

- Live TV, where the server has channels, laid out as TiviMate does: in
  the rail after Home, the categories (Search, Favourites, Recently
  watched, All channels, then the groups, a region's gathered under it),
  the channels of the one shown by number with what's on now and how far
  into it, a page at a time, and the focused one's programme and what
  comes next beside them; a channel's schedule; number keys to go to a
  channel. Kodi's lists have the same categories. A channel plays in
  Kodi's own player as the provider sends it (Kodi deinterlaces; surround
  sound passes through), Next and Previous change channel within its
  category, and every connection in use says so in the server's words.
  Lineups of tens of thousands of channels open at once. Needs a server
  with Live TV.

### Security

- Plain http goes only to addresses on the home network, checked on the
  connection: sign-in codes and tokens never cross the internet
  unencrypted. Use the server's https address from elsewhere.
- The proxy refuses a negative or unreadable Content-Length, and checks
  its secret in constant time.

### Changed

- PGS subtitles come to Kodi as a `.sup` file beside a converted stream,
  or for a `.sup` file next to the media, rather than burned in: the
  picture keeps its quality and HDR. Needs a server that makes them.
- Buttons and rows read in sentence case, as in the other apps: "Skip intro",
  "Skip credits", "More like this", "Cast & crew", "Guest stars". Sign out
  asks "You'll be signed out of <server>."

## [0.0.1] - 2026-10-29

### Added

- First release with a version of its own (builds before it carried 0.2.0,
  the version every app shared).

[Unreleased]: https://github.com/hhaidar/pley/compare/kodi-v0.0.1...HEAD
[0.0.1]: https://github.com/hhaidar/pley/releases/tag/kodi-v0.0.1
