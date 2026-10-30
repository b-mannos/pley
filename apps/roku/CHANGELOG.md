# Changelog

Notable changes to Pley for Roku, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Releases are tagged `roku-v<version>`; [docs/RELEASING.md](../../docs/RELEASING.md)
describes how they are made.

## [Unreleased]

### Added

- Live TV, where the server has channels: a guide (Roku's TimeGrid, the
  programme in focus described above it, filtered by group or
  favourites), Channels (Recently watched, Favourites and each group as
  rows of cards with what's on now and next), and a live player with
  up and down to change channel, the channel list at the left, what's on
  with the Live badge, pause and rewind within the server's timeshift,
  and the server's own words when every connection is in use. The
  Options key adds a channel to the favourites or takes it out. Past a
  thousand channels the guide and the list show a group at a time.

### Changed

- "Can't reach" offers Try again, as the other apps do, and the skip
  buttons read "Skip intro" and "Skip credits". Sign out asks "You'll be
  signed out of <server>."

## [0.0.1] - 2026-10-29

### Added

- First release with a version of its own (builds before it carried 0.2.0,
  the version every app shared).

[Unreleased]: https://github.com/hhaidar/pley/compare/roku-v0.0.1...HEAD
[0.0.1]: https://github.com/hhaidar/pley/releases/tag/roku-v0.0.1
