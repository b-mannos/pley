# Changelog

Notable changes to Pley for Roku, newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Releases are tagged `roku-v<version>`; [docs/RELEASING.md](../../docs/RELEASING.md)
describes how they are made.

## [Unreleased]

### Added

- Live TV, where the server has channels, as TiviMate lays it out:
  Search (every channel, and the groups whose names match), Favourites,
  Recently watched, All channels, the regions in alphabetical order and
  then the other groups down the left, and the category's channels as a
  guide (Roku's TimeGrid, the programme in focus described above it) or
  as cards, 50 at a time, so lineups of tens of thousands of channels
  open at once. The live player changes channel with up and down within
  the category, lists it at the left, shows what's on with a Live chip
  instead of a seek bar when asked (OK, Right or the Options key), pauses and moves 10 s within the server's
  timeshift, and gives the server's own words when every connection is
  in use. The Options key adds a channel to the favourites or takes it
  out; back from the player, the guide is at the channel watched.

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
