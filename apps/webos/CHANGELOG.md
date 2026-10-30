# Changelog

Notable changes to Pley for LG TV (webOS), newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Releases are tagged `lg-v<version>`; [docs/RELEASING.md](../../docs/RELEASING.md)
describes how they are made.

## [Unreleased]

### Security

- The token goes only to the server's own origin, never to an address
  elsewhere that the server hands out.

### Added

- Live TV, where the server has IPTV channels for the profile: a guide
  that moves through time, channel cards with what's on and next, groups,
  favourites and the channels watched last, and a programme's details. The
  player plays a channel through LG's media pipeline from its live edge,
  with what's on, the Live badge, pause and rewind in the server's
  timeshift, the channel list, and CH+ and CH−, Up and Down, or the
  channel's number to change channels. When every connection is in use,
  the server's words say which channels hold them. Needs a server that
  gives the TV's player keyed live addresses.

### Fixed

- Picture subtitles the server is still copying out of a big file show
  as far as they are, without a picture cut in two.
- Switching to another server no longer shows the last server's Home,
  libraries or titles, and asks "Who's watching?" as that server's
  household needs.
- Automatic quality no longer leaves the original for a wait the server
  caused (a slow disk, a remux catching up): a wait of 2 s or more counts
  only on a network the TV measured below 1.5 times the file's bitrate in
  the last 2 minutes, or, where it measured nothing, the second such wait
  within 5 minutes.

## [0.0.1] - 2026-10-29

### Added

- First release with a version of its own (builds before it carried 0.2.0,
  the version every app shared).

[Unreleased]: https://github.com/hhaidar/pley/compare/lg-v0.0.1...HEAD
[0.0.1]: https://github.com/hhaidar/pley/releases/tag/lg-v0.0.1
