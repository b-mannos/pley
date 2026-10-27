# Store listings

The pictures and words the app stores ask for besides the app itself.
`node scripts/render-icons.mjs` renders the pictures from the logo
(`apps/web/public/favicon.svg`), with every other icon.

| File | Size | For |
|---|---|---|
| `icon-512.png` | 512 × 512 | Google Play's app icon (Play rounds its corners), the Amazon Appstore's large icon |
| `icon-114.png` | 114 × 114 | The Amazon Appstore's small icon |
| `feature-graphic-1024x500.png` | 1024 × 500 | Google Play's feature graphic |
| `tv-banner-1280x720.png` | 1280 × 720 | Google Play's Android TV banner, the Amazon Appstore's Fire TV icon |

Screenshots are taken from the apps: phones and tablets at 1080 × 1920 or
larger, TVs at 1920 × 1080.

## The app

- **Package**: `com.formandelement.pley` (Google Play and the Amazon Appstore),
  the bundle from the release, `pley-android-<version>-play.aab`
  (docs/ANDROID.md, Releases).
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
