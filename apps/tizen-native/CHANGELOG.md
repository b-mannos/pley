# Changelog

Notable changes to Pley for Samsung TV, native (Tizen .NET), newest first.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
It has no releases yet: it is built and installed by hand
([docs/TV_PLATFORMS.md](../../docs/TV_PLATFORMS.md#the-native-samsung-app)).

## [Unreleased]

### Added

- A native app for Samsung TVs with Tizen 9, beside the web app: so far the
  Android TV app's "Choose your server" screen, with servers found on the
  network, an address pad, the TV's keyboard and forgetting a server; its
  sign-in with a code and a QR code, the tokens kept sealed by the TV's key
  manager; Who's watching? with PINs; and Home, with the hero, the
  server's rows and the rail; item pages, with seasons and episodes, Extras,
  Cast, More Like This and the side panels; in Roboto as on Android, driven
  by the remote, a held key sliding a row along under a still highlight;
  pictures load as rows come near the screen, and a tile without one shows
  its title on a gradient of its own, as on Android.
- Library pages in the native app: the name and tabs, the chips (Sort,
  Filter, Unwatched, Genre, Year) and their side panels, the grid of
  posters with the A to Z jump, Recommended and Collections; and the
  Libraries page, where the profile can sort them. The rail's libraries open
  them.
- A tile's actions in the native app: OK held or the Menu key on a tile on
  Home or a library page opens them in a side panel (Play, the watched
  marks, Continue Watching, My List and the pages they lead to).
- Item pages in the native app end with Details: who made it and the
  facts, the versions (OK chooses which plays) and Media info, which OK
  opens: each file and every stream in full.
- Rows with more than they show end with See all, which opens the library
  sorted and filtered as the row is (Recommended's collections: the
  Collections tab).
- Playing in the native app: a session on the server with the TV's
  capabilities, as the web Samsung app reports them, on the TV's own
  player; Left and Right seek 10 s, presses adding up and a held key
  scrubbing; the title, clock, scrubber and Back 10 · Play/Pause ·
  Forward 10 over a scrim; where it stopped goes to the server.
- Collections in the native app: the rail's Collections lists every movie
  and show library's collections, a row each; a collection's page has its
  backdrop, TMDB or Smart badge, what it holds, Play and Shuffle, and its
  titles in a grid that scrolls up over the backdrop.
