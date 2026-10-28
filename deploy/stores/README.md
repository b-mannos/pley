# Store listings

The pictures and words the app stores ask for besides the app itself.
`bun scripts/render-icons.mjs` renders the pictures from the logo
(`apps/web/public/favicon.svg`), with every other icon.

| File | Size | For |
|---|---|---|
| `icon-512.png` | 512 × 512 | Google Play's app icon (Play rounds its corners), the Amazon Appstore's large icon |
| `icon-114.png` | 114 × 114 | The Amazon Appstore's small icon |
| `feature-graphic-1024x500.png` | 1024 × 500 | Google Play's feature graphic |
| `tv-banner-1280x720.png` | 1280 × 720 | Google Play's Android TV banner, the Amazon Appstore's Fire TV icon |

`screenshots/` has the apps' screenshots, phones' at 1080 × 1920 (the
player 1920 × 1080) and TVs' at 1920 × 1080, JPEGs as Google Play takes
them. They show a made-up library, with no real title's artwork, which
`scripts/store-library` makes again (its README says how).

| Phone | TV |
|---|---|
| `phone-1-home` Home: Continue Watching, My List | `tv-1-home` Home |
| `phone-2-movie` A film's page | `tv-2-library` The Movies library |
| `phone-3-library` The Movies library | `tv-3-movie` A film's page |
| `phone-4-show` A show's page | `tv-4-episodes` A season's episodes |
| `phone-5-episodes` A season's episodes, with downloads | `tv-5-player` The player |
| `phone-6-player` The player | |
| `phone-7-downloads` Downloads, for watching offline | |

## The app

- **Package**: `com.formandelement.pley` (Google Play and the Amazon Appstore),
  the bundle from the release, `pley-android-<version>-play.aab`
  (docs/ANDROID.md, Releases).
- **Version code**: the version's (0.2.0 is 20000). The stores take each
  code once, so a second upload of the same version is built with
  `PLEY_BUILD` set, 1 to 99, which is added to it (`PLEY_BUILD=1` makes
  20001): `./gradlew :app:bundleStoreRelease` in `apps/android`, with the
  release key.
- **Category**: Video Players & Editors.
- **Name**: pley

**Short description** (80 characters at most):

> Your own movies, shows and music, from your pley server, on phones and TVs.

**Full description**:

> pley plays the movies, shows and music on your own pley media server, at
> home or away.
>
> - Continue Watching, My List and every library, with posters, plots and
>   episode stills.
> - Plays files as they are whenever the device can, in its own player;
>   your server converts the rest.
> - HDR and Dolby Vision on the TVs that show them.
> - Skip Intro and Up Next.
> - Downloads for watching offline, which report what you watched when
>   you're back.
> - One app for phones, tablets, Android TV, Google TV and Fire TV.
> - Signs in with a code shown on the TV, typed on your phone or computer.
>
> pley needs a pley server, which runs on Windows, macOS, Linux and Docker.
