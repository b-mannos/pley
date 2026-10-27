# The store screenshots' library

A made-up library for the stores' screenshots (`deploy/stores/screenshots`):
twelve films and four shows with their own posters, backdrops, stills, plots,
genres and ratings, so no real title's artwork appears. Each title is a
video of its picture at its running time (1080p, silent 5.1), small enough
to make in a few minutes.

```sh
node scripts/store-library/art.mjs <dir>          # the pictures, through a headless Chrome
node scripts/store-library/media.mjs <dir>        # Movies/ and Shows/, and the videos to make
tr '\n' '\0' < <dir>/jobs.txt | xargs -0 -P 6 -I{} bash scripts/store-library/encode.sh "{}"
```

Then a server of its own: a new data folder, nothing else on it, and no
internet, so the metadata providers can't match the made-up titles to real
ones (`HTTP_PROXY`, `HTTPS_PROXY` and `ALL_PROXY` at a closed port,
`NO_PROXY=127.0.0.1,localhost`):

```sh
pley serve --data-dir <dir>/data --bind 127.0.0.1:17560 --plex-bind off --no-discovery --no-tray
node scripts/store-library/setup.mjs http://127.0.0.1:17560 <setup code> <dir>
```

`setup.mjs` makes the account (Alex, its password in `<dir>/admin.json`),
the libraries, the metadata, something to resume, things watched and My
List. Emulators reach the server at `10.0.2.2:17560`. Phone screenshots are
1080 × 1920 (`adb shell wm size 1080x1920`): Google Play takes nothing
longer than 2:1.
