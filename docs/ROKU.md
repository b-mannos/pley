# Pley for Roku

A native SceneGraph app for Roku players and Roku TVs, in `apps/roku`
(paths below are relative to it). It is not in Roku's channel store: you
sideload it in developer mode.
It talks only to the Pley server, so it works without internet: pictures
come from the server (Discover's from TMDB through it). Discover needs the
server to reach TMDB.

- [Install](#install)
- [Using it](#using-it)
- [Building and testing](#building-and-testing)
- [Releases](#releases)
- [Known limits](#known-limits)

## Install

1. Turn on developer mode: on the Roku remote press Home three times, Up
   twice, then Right, Left, Right, Left, Right. Note the address shown,
   choose a password and let the Roku restart.
2. Get the zip: the one attached to a Roku release
   (`pley-roku-<version>.zip`), or `bun run package`, which writes
   `out/pley-roku.zip`.
3. Open the Roku's address in a browser, sign in as `rokudev` with that
   password, upload the zip and press Install. Pley appears on the Home
   screen; installing again replaces it.

`bunx bsc --deploy --host <address> --password <password>` packages and
installs in one go. The debug console (`telnet <address> 8085`) shows what
the app prints and its errors.

## Using it

The app finds Pley servers on the local network (or takes an address) and
signs in with a code shown on the TV, approved on the server's `/link` page
or by its QR code. Choose your server lists the saved servers first, then
those that answer the broadcast. A saved server the broadcast didn't find
is asked directly (`GET /api/server`, without a token), and its row says
"Can't reach it" when it doesn't answer as Pley; the row chosen says
"Connecting to …". Search this network asks every other address of the
Roku's network ("Checking 192.168.1.1-254…"), 24 at a time, a second
each, without a token, for servers that never hear the broadcast (Docker
on a bridge network). It does so only on a private network (10/8,
172.16/12, 192.168/16, or a link-local address), so a Roku on a
provider's shared addresses never asks its neighbours; elsewhere it
sends the broadcast again. An answer longer than 4 KB isn't read, and a
server's id is kept only as up to 64 letters, digits and dashes. An address typed on the keyboard that can't be used says why
on the keyboard, shown again: one over plain HTTP beyond the home network
is refused, as SECURITY.md says of the TV apps. The sign-in screen is laid out as Android TV's: three
numbered steps (scan the code, or go to the address, and enter the code,
in two fields), the QR code at the right in a light tile that is dimmed
once the code expires, and Change server under it. An expired code gives
way to a new one without a word, three times (half an hour of codes);
then OK gets the next. When the server doesn't answer, the screen says
when it tries again, counting down the seconds. When the server requires
two-step sign-in, the sign-in screen says so: the phone that approves the
code sets it up for whoever hasn't yet. It starts straight into Home as the profile it was left on when
that profile has no PIN. In a household of more than one profile, it
asks "Who's watching?" at start when that profile has a PIN, where
going on as it asks the PIN, as on the other TVs; with Always start
as, the start opens on that profile's PIN pad, and Use another
profile shows the other profiles.
Choosing a saved server (Settings' Switch server too) is a start on it:
it asks the same, whatever was answered on the server before. A PIN is
typed on Roku's own PIN pad ("Enter Anna's PIN"): a wrong one, or too
many, asks again saying why. Where Android TV dims its pad (a PIN locked
after too many wrong tries, a profile that can't be used from the app),
Roku's pad can't be dimmed: a message under the same title says why, with
Use another profile. A profile that needs its password opens the sign-in
screen as "Approve on your phone": "Robin's profile needs its password:
approve this code on a phone signed in to Pley, and pick Robin there.",
with Use another profile back to the profiles. A sign-in that can't
switch profiles offers Sign in as another profile, the same way.

### Home

Home's rail, at the left, is Android TV's: the profile's avatar (its
picture, or its initial on its colour), which opens "Who's watching?",
then Search, Home, one way into requests (where the server has them:
Discover where the profile may ask, else Your requests, a manager's
Requests, with the number of what waits there), Live TV, the libraries,
Playlists, Collections (every library's collections, a row each), Now
Playing while music plays, USB drive while one is plugged in
([below](#videos-on-a-usb-drive)), and Settings, each with its icon. With more
than six libraries, as on Android TV, the rail lists the first six, then
Libraries in place of Collections: every library, with what it holds
("Movies · 412 movies"). While the focus is in the page the rail shows
the icons alone, Home's in the text colour with an accent mark beside it
and a dot on the requests' icon while some wait. Left or Back brings the
focus to it and it opens over the page, which dims: the names fade in
beside the icons, the focused entry on the accent pill, Home on a raised
one, and the requests' number after their name.

Beside it, as on Android TV and the native Samsung app, a hero shows the
first of Continue Watching, else the newest title in the rows, over its
backdrop: its title (a show's name over an episode's "S2 · E5 · Title"),
year, length, content rating, stars and quality ("4K", "DV"), the time
left, its summary, then Resume (or Play) on a pill, and From start, Info
and the watched mark as round buttons, whose names show under them while
they have the focus; a show's (or a season's) Play plays its next
episode, as on Android TV, and "Nothing to play" says when it has none.
Resume has the focus as Home first shows. Down goes to the server's rows:
there an info block about the title focus rests on (0.4 s) takes the
hero's place, the rows come up under it, the focused row staying just
under it as the others move, and the backdrop follows it. Up from the
first row goes back to the hero; Left or Back, to the rail, the hero
shown again.
The hero's summary, content rating and quality come with its details
(`GET /api/items/{id}`, which the API task reduces to what Home says), as
do the info block's. With nothing to show, Home says why: No libraries
yet, Scanning your libraries…, or Nothing here yet.

Tiles are captioned as on the other TVs, in rows and grids: a title in
progress says the time left ("26 min left"), else what it is: a movie's
year, a show's seasons ("3 seasons"), a season's episodes ("10
episodes"), an album's artist and year, a track's artist and album. An
episode's tile is its show's, with "S1 · E3 · 26 min left" (its title
when not started). A show's or a season's tile has the number of its
episodes left to watch where the watched mark goes, the mark only once
every one is watched. Continue Watching, and any row whose first title
is an episode, has 16:9 stills.

What can't be loaded says so in the page, as on Android TV: what didn't
load ("Couldn't load your home screen"), the server's sentence, and Retry,
which has the focus. So do a library's tabs, Collections, a collection,
an album, an artist, the playlists, a playlist and the watch history. A
server that doesn't answer at all has a screen of its own, with
Retry.

### The server out of reach

When the server answers nothing, or another device answers at its
address, "Pley is offline" shows at the top right for 6 s, as on Android
TV (not over the players, which say "Reconnecting…" themselves). A page
that needed the server gives way to "Can't reach your server": why, then
"Check that the server is on and that this device is on its network.",
with Retry and Switch server. Meanwhile the app asks every 5 s whether the
server answers (`GET /api/server`, signed in with the token, so the
answer counts only once the server there has proved itself), and that
page comes back by itself once it does.

### How it looks

The app has the Android TV app's look, as far as Roku allows:

- **Type:** Roboto, the face of the Android app and the other TVs, in
  weights 400, 500 and 700, at Android TV's sizes (its 960 × 540 dp
  doubled to Roku's 1920 × 1080): 76 pixels for the hero's title, 56 for
  a page's, 36 for row headers, 28 for buttons and the metadata line, 26
  for tile captions. Roku draws a label in one font, and the Roboto files
  hold the Latin alphabets of Western Europe: a title or a name the
  server sends in another script (Greek, Cyrillic, Polish, Japanese…) is
  drawn in Roku's own font, which has them, and so is a line with an
  arrow. A rating's star is an icon in the line.
- **Icons:** Lucide's, as every other app draws them: the rail's, the
  hero's buttons, Live TV's tabs and its Search, Favourites, Recently
  watched and All channels, a region's chevron, and favourite channels'
  stars.
- **Tiles**, a title's or a playlist's, have rounded corners, and a
  profile's avatar is round. The focused tile grows to 1.05 with a
  ring around it and its title turns bold, the look coming in over 150 ms
  eased out and going over 120 ms eased in; only the tile's scale and the
  ring's opacity change, which a Roku draws without laying anything out.
  A Roku crops a list's items at its edges, so each tile is drawn inside
  a cell with the room its look takes, and the look is a little smaller
  than Android TV's 1.08, its ring 2 dp out rather than 3, to keep the
  tiles as close together.
  The rail and the page move the same way. A player Roku counts as slow
  skips these animations (they are `optional`), and the looks change at
  once. A row keeps the focus at its left edge and slides under it.
- **Buttons, tabs and chips** are rounded pills, the focused one the
  accent pill at 1.05.
- **Backdrops** (Home, a title's page, a TMDB title's, a collection's,
  Discover) fill the screen under Android TV's two gradients of the
  page's colour, from the left and from below, drawn as stretched
  pictures.

Roku's own player bar, keyboards, PIN pad and dialogs keep Roku's look.

### Remote keys

| Key | Where | What it does |
|---|---|---|
| Options (\*) | A title | As on Android TV: Play (a show's or a season's next episode), or Resume and From start; the watched mark, Remove from Continue Watching, My List, Add to playlist, an episode's Go to show and Go to season, a season's Go to show; music's Open and Add to playlist. What it came to ("Added to Road trip", or why not) shows in a toast at the top right |
| Options (\*) | Watch history | Removes the play at once, as Android TV's cross does; a toast says so, and the focus goes to the next |
| Options (\*) | A playlist entry | Move (Up and Down carry it, OK puts it there, Back puts it back), Remove from playlist, Go to its title's page |
| Down | The player | Pley's menu: Subtitles, Audio, Quality, Subtitle sync, Chapters, Previous chapter, Next chapter, Next episode (or Next in the queue), Sleep timer, Find subtitles…, Playback details |
| Options (\*) | The player | Roku's own options: subtitles and audio tracks |
| OK | Skip Intro, Skip Credits | Skips; fast forward and rewind jump 30 s, instant replay 10 s back |
| Options (\*) | Live TV | A programme's details (a card's: what's on), Watch, Add to or remove from favourites |
| OK, Options (\*) | The live player | The channel list, at the right |
| OK held | The live player | What's on, with Channels and Add to or remove from favourites |
| Options (\*) | The live player's channel list | Adds the channel in focus to the favourites or removes it |
| Replay | Live TV, the live player's channel list | Search (Roku remotes have no search key for apps) |

### A title's page

A movie's, a show's, a season's or an episode's page looks like Android
TV's: its backdrop fills the screen under the page's colour from the
left and from below, as Home's hero's does, with no poster, and the text
is in Android TV's column, 928 pixels wide from the safe area's edge. It
says what Android TV's does, in its order: the title, as big as Home's
hero's (an episode's or a season's show, with "S1 · E3 · Pilot" or
"Season 1" under it), then a line of what it is: a
movie's year, an episode's air day, a show's seasons and years ("3
seasons · 2019-2024"), a season's episodes, the version's
edition, the runtime, the content rating, the stars and the quality of
the version Play plays ("4K · DV P8.1 · Atmos"). Under it, a title that
was asked for shows its request's stage, with its download's bar beside
it. A title resumed says
how much is left ("1 h 33 min left"); the genres are followed by the
collections it is part of ("Drama · Part of Dune").

Its buttons are one row, drawn as Home's hero's: Resume (or Play; a
show's or a season's next episode, "Resume S1 · E3" or "Play S1 · E3")
on a pill with Play's icon, then round buttons with Android TV's icons,
whose names show under them while they have the focus: From start,
Request more seasons for a show with seasons left to ask for, the
watched mark, Audio and subtitles, My List, Trailer (where its extras
have one: it plays from its start), Add to playlist and More. Left and
Right go between them. With nothing to play the pill says Unavailable,
faint, and the focus starts on the next and never goes to it. Holding
OK on the pill doesn't open the title's actions as Android TV's long
press does: the Options key does, from anywhere on the buttons. Audio
and subtitles and More open a panel at the right; Back closes it. Over
the summary, as on Android TV,
a line says who made it and who is in it ("Directed by Grace Hopper ·
Starring Ada Lovelace, Alan Turing"; a show's "Created by"). The text
keeps above the buttons and the rows: the summary has three lines at
most, as on Android TV, and gives lines up while the text would reach
lower, and the tagline goes when the summary would keep fewer than two.

Under the buttons, their top in view, a show's or a season's page has
its seasons' tabs (when there are two or more), then a row of the
season's episodes: 16:9 stills, "5. Glasshouse" and the time left (else
the runtime) under each, as on Android TV. A show's page opens on its
next episode's season, a season's on its own, the focus coming into the
row on the next episode; an episode's page has its own season's
episodes, without tabs, the focus on itself. A tab the focus rests on
(0.35 s) shows its season, as OK does. A season tile opens the season's
page.

The rows under them are in Android TV's order: Extras, then the people,
titled by who is there (Cast, Crew or Cast & Crew; an episode's Guest
Stars), More like this and the collections the title is part of. Down
from the buttons goes to the tabs, the episodes and the rows, Up back:
once the focus is there, an info block at the top keeps the title in
view, with its line and, in the tabs and the episodes, the episode the
focus rests on (0.4 s): its title, "Season 2 · Episode 5 · 55 min · Aired
Mar 4, 2024" and two lines of its summary, else the title's. The part
with the focus comes up under it. Back leaves the page from anywhere on
it, as on Android TV. A page that doesn't load says why, with Retry
(Back for a title that is gone, or the profile can't see); one the server
doesn't answer for says it can't be reached, as any screen does.

- **Audio and subtitles** (a movie or an episode with several files, or
  whose file has two audio tracks or more, or subtitles):
  - Version, for several files: one line each ("Director's Cut · 4K ·
    HEVC · DV P8.1 · TrueHD Atmos 7.1 · 62.0 GB"). Play plays the one
    chosen (`file_id` in `POST /api/playback`), with the tracks the server
    picks; it isn't saved, as on the other apps.
  - Audio, Automatic first, named by the language the profile's defaults
    pick ("Automatic (English)"), then the file's tracks ("French (AC3
    5.1)"); Subtitles, Off first, then the file's ("English – SDH (SRT)").
    A choice is saved at once for the show or movie (`PUT
    /api/items/{id}/streams`) and Play starts with it. The audio goes with
    it only when picked here, Automatic as none, as on the other TVs.
- **More**, what is used less, as on the other TVs: Rate (Your rating,
  once given: No rating, or 1 to 5 stars, kept for the profile on the
  server), an episode's or a season's Go to show (the show's page) and
  an episode's Go to season (the season's page), Media info and Details.
  The Options key on an episode's tile goes to them alike.
- **Media info** (a movie or an episode with a file): each file, under its
  edition or "VERSION 2" when there are several, and every stream in
  full, in the other apps' words (codec, profile and level, size, frame
  rate, bit depth, HDR and Dolby Vision, colour, channels, sample rate,
  language, title, flags), as text that Up and Down scroll.
- **Details**, Android TV's section of them: the whole summary, which the
  page cuts, then who created, directed and wrote it (four names a role,
  then "and 2 more"), the original title, when it came out, the studio (a
  show's network) and how many times the profile played it. It is in More
  when there is something the page doesn't say.

### Collections, people, artists and playlists

A collection's page has its backdrop, "Collection" with a TMDB or Smart
badge, its title, what it holds ("5 movies · Movies · By release date";
a smart collection's filter in words), its summary, Play and Shuffle (two
titles or more), then its posters, which Down slides the page up to. Play
plays a collection of movies in its order, Up Next bringing on each next,
and Shuffle in any order; a collection of shows plays the first show's
next episode, and Shuffle a random show's.

A person's page has their photo, their name and what of theirs is in the
libraries ("3 movies · 2 shows in your libraries"), then a row for each
thing they did (Acting, Directing, Creating, Writing, Producing) of their
titles, newest first, each with its year and the part ("2019 · as Neo").

An artist's page, as on Android TV, has their picture, beside it their
name, albums, tracks and genres and their summary, then Play and
Shuffle, which hand every track of theirs to the music player, album by
album, and open Now Playing (Now playing goes back to it while music
plays); under them, their albums in a row of square tiles, newest first,
each with its year.

Playlists shows each playlist as a square cover, five across, as on
Android TV: a mosaic of its first four entries' pictures (decoded at a
quarter of the cover each, for a Roku Express's texture memory), or the
first one's alone, under it "12 items · 3 h 12 min" (and "Smart"). New
playlist, at the right of its heading, asks what it is for
(Movies & TV or Music), then its name on the Roku's keyboard. The new
playlist opens, empty; Add to playlist, the Options key on a title, fills
it. A playlist's page, as on Android TV, has its first entry's backdrop,
"Video playlist", "Music playlist" or "Smart playlist" over its name,
what it holds (a smart one's filter in words, after a Smart badge), and
Play and Shuffle (one entry, or two, at least), Rename (not for a smart
playlist, which its filter fills) and Delete. Its entries move with the
Options key's Move; an entry taken out, or a change refused, says so in a
toast.

### Libraries

A library has the tabs Recommended, Library and Collections (a music
library has no Collections), and opens on the tab, sort and filters it was
left on, kept on the Roku for each server, profile and library.

- **Library** is a grid of posters, 70 at a time as the focus comes near
  the next ones; places a jump passes over show empty frames until their
  titles come.
- **Quality badges:** sorted by bitrate, or filtered by resolution or
  dynamic range, the posters say what sets their best file apart at their
  foot: "4K" (no other resolution), then "DV", "HDR+", "HDR" or "HLG".
  Sorted by bitrate, its bitrate and size take the year's place under the
  poster ("42 Mbps · 23.4 GB"). Otherwise, and in rows, collections,
  playlists and search, posters have no quality badges.
- **Recommended** is the library's rows, then, as on Android TV, its
  collections (the first 20), whose See all opens the Collections tab.
- **See all** ends the rows of a library's newest, newly released and top
  rated titles, on Home and on a library's Recommended tab. It shows the
  library's Library tab in the row's order (date added, release date or
  rating, newest or best first), nothing filtered, for that visit: the
  sort and filters kept stay as they were until changed there.
- **A to Z:** sorted by title with more than 30 titles, "#" and A to Z
  stand in a column right of the grid, the letter of the title in focus
  marked and the letters no title starts with dimmed. Right from the
  grid's last column reaches it, Up and Down pick a letter, OK goes to its
  first title (or the next letter's) and Left goes back. The server has no
  index of letters: the app reads the listing's titles a thousand at a
  time and keeps only where each letter starts
  (`components/lib/Library.brs`).
- **Chips**, at the right of the tabs, as on Android TV: the sort (its
  order in words), Filter, Unwatched (one press turns it on or off),
  Genre and Year once the server says what the titles have, and the
  resolution or dynamic range chosen ("4K", "Dolby Vision"); a filter in
  force is lit. Genre, Year and those two open their choices, and the
  pick goes back to the chips. A music library has the sort alone. When
  they don't fit beside the tabs, the chips slide to keep the one in
  focus in view. Filtered, the count at the top right says "23 of 342
  movies".
- **Filter** opens a panel: Show (all titles, unwatched or watched),
  Genre and Year, then Resolution (4K, 1080p, 720p, SD) and Dynamic range
  (Any HDR, HDR, which takes in Dolby Vision and HDR+, Dolby Vision, HDR+,
  HLG, SDR). Those two show once the server says the titles differ in
  them, offering the choices it lists, or while one is chosen; a title
  counts by its best file, a show by its best episode. Filters that
  nothing matches show No matches, with Clear filters, which takes the
  focus.
- **Save as…:** once filters are set, the Filter panel ends with Clear
  filters and Save as…, which saves them and the order as a smart playlist
  (`POST /api/playlists` with `library_id` and `smart_filter`, written as
  the other apps write it), named on the Roku's keyboard, and opens it.

### Search

Search is typed on the Roku's keyboard and asked once typing rests half a
second. What the libraries have comes in rows by kind, in the other apps'
order: Movies, Shows, Seasons, Episodes, Artists, Albums, Tracks, Live TV
(cards: OK watches the channel, and Up and Down in the player go through
what was found) and People, the closest matches first in each; then
Discover for those who may ask (below). Each row holds all that was found
of its kind, up to 200 (`limit=200`), and scrolls sideways.
Right from the keyboard's last column reaches them.

Before anything is typed it lists the recent searches: the last 10 whose
results were opened, newest first, kept on the Roku for each server and
profile. OK searches one again at once and moves it to the top; Clear
forgets them.

### Music

Music plays on while other screens show, and Home's rail has Now Playing
while it does. An album's page lists its tracks under "Disc 1", "Disc 2"
when they are on several discs (the tracks' `parent_index`), each
numbered on its disc; the focus passes over the headings. An album or a
music playlist plays whole, from the track chosen, so Previous goes back
past it; its Shuffle starts on any track and plays the others after it in
a random order. As on Android TV, the track playing is marked in the
list, OK on it pauses or goes on, and the profile's rating shows under
the album's line ("Your rating: 4 stars"). As on the other TVs, an
album's page also has Go to artist, and Now playing while music plays.

As on the other apps, the queue stays until other music replaces it (or
another profile, sign-in or server comes): at its end the last track
stays, and Play starts it again; a track that can't be played stays,
saying why ("Can't reach the server.", "This track can't be played."),
and Play asks for it again. A video or a channel pauses the music, which
lets go of its session (its paused reports would keep it through a whole
film); Play goes on from where it was, in a new session.

Now Playing shows, as on Android TV, the track, "Artist · Album", its
cover and how far along it is, a line saying Loading… or why the track
doesn't play, Shuffle, Previous, Pause, Next (while a track comes after
it) and the sleep timer, and Up next: the queue, numbered, the track
playing in bold, which stays in view unless the focus is on the list. OK
on one plays it, and the queue goes on from there. Up from the buttons
goes to the scrubber, where Left and Right move 10 s and OK plays or
pauses, as on Android TV. After two minutes without a key it dims to 30 %,
to spare the screen, and the next key wakes it (the remote's rest is
Roku's `TimeSinceLastKeypress`, looked at each second, four times a
second while dimmed). A long queue is listed 250 tracks at a time, from
50 before the one playing, the next part as playing nears the end of this
one: each row is a node the Roku draws, and a playlist holds up to
5,000. Shuffle plays the other tracks after the one playing in a random
order; Stop shuffling puts them back in the album's or playlist's order,
at the same track. Play pauses or goes on; fast forward and rewind go to the next
track or back.

The sleep timer belongs to the music, not to Now Playing: it runs while
other screens show. Each OK on its button steps Off, 15, 30, 45, 60 and 90
min, End of this track, and the button says what is left ("Sleep timer:
23 min left"). In its last 30 s Now Playing says "Pausing in 12 s" and the
button is Keep listening, which adds the same time again (or lets this
track play to its end). Then the music pauses until Play, as on the other
apps; at the end of a track, the next one waits, paused. The queue's
end turns the timer off.

### The player

As each item begins (the first, the next episode, a playlist's or a
collection's next), its title shows at the top for 4 s, a show's name over
the episode's "S1 · E2 · Pilot", or a movie's "Dune (2021)", as on Android
TV. Skip Intro and Skip Credits are named as on the other apps.

Paused, or while Roku's bar shows, the title stays at the top with, at
the right as on Android TV, how it plays (Direct play, Direct stream or
Transcoding), the clock (on the Roku's 12- or 24-hour clock) and, paused,
when it ends ("Ends at 10:42 PM"). Skip Intro and Skip
Credits stay while paused, above Roku's bar. The bar itself is Roku's,
with its seeking (fast forward and rewind speeds, seek previews, instant
replay): there is no control row or scrubber of Pley's own, and Back while
paused leaves the player, as in Roku's own apps.

- **Up Next** is Android TV's card at the bottom right: the next one's
  still, "Up next · S2 · E6" and the episode's title, then Play now and
  Cancel with the countdown beside them ("Next in 7"). A playlist's next
  entry is asked for as its item begins, for its episode's name. It shows
  at the credits marker, else at the end, as on Android TV and the native
  Samsung app. Shown at the credits, it goes on counting down past the
  end. Cancelled, or with Play the next episode by itself off, it waits at
  the end, paused: Play now (or Play) plays the next one, Cancel or Back
  leaves. With the setting on, the countdown starts again there.
- **Waiting** for the video, as it opens or once it plays, shows a spinner
  after 500 ms, as Android TV's ring.
- **The server out of reach:** once something played, the player keeps its
  place and its queue and asks again every 2 s, saying "Reconnecting…"
  under the spinner, for 90 s, as long as the server keeps a session it
  doesn't hear from; then, or when the first playback can't start, it says
  "Can't reach your server", with Try again. A refusal says why, with Try
  again where asking again may help (not for a title gone or out of reach
  of the profile). What went wrong shows as on Android TV: centred on
  black, the title, the reason in up to four lines, then Try again
  (focused) beside Back.
- **Stopped** by the server's administrator or from another device, the
  player says so itself, as Android TV's: "Playback stopped", who stopped
  it and their message, with Back; it leaves by itself 20 s later.
- **Stalls:** a playhead that stands still 12 s while meant to play is
  looked at, as the other apps do: a file played as it is that has its
  data is converted once a title (see [Quality](#quality)); a stream asks
  the server how fast it converts, and below real time says "The server
  can't keep up with this video" (Automatic goes lower by itself);
  otherwise the stream opens again where it was.
- **Media that ends short** (moments after it started, a minute or more
  before the title's end, as a file resumed past what it holds) isn't the
  end: a file played as it is is converted, and a stream opened again;
  falling short again within the minute, the player says so.
- **Music** gives way to a video or a channel: it pauses, its session
  ends, and its queue stays for Now Playing.
- **Notices** (a lower quality, subtitles added, a server that can't keep
  up) show in the app's toast at the top right, under the title at the
  top, and go by themselves.
- **Progress** goes to the server every 10 s while playing, every 30 s
  paused, and at once after a seek, in Roku's own bar too.
- **Playback details** in the menu say "Asking the server…" until a
  conversion's details come, are asked for again every 2 s while they
  show, and scroll with Up and Down when they don't fit.
- **Settings → Playback details** (kept on the Roku, off at first) keeps a
  short readout at the top left while nothing else shows, in the place of
  the title, which then doesn't show as an item begins, as on Android TV:
  how it plays, the file's video and audio (and, converted, what the Roku
  decodes), the subtitles, the bitrate (the file's, and a conversion's),
  and the network as the Roku measures it. The setting is read as the
  player opens; the readout follows the video each second.
- **Chapters** are in the menu for a file with two or more, each named by
  its title, or "Chapter 3" when it has none or only its time ("00:06:19",
  as mkvmerge names them). With them come Previous chapter (the start of
  the chapter playing, or of the one before within its first 3 s) and,
  before the last, Next chapter, as Android TV's Previous and Next.
- **Next episode** (Next in the queue for a playlist or a collection) is in
  the menu while something comes next, as Android TV's Next: the episode
  keeps its place.

Fast forward and rewind show seek previews in Roku's own bar: the server's
previews of the file as a BIF file, which the Video node fetches beside the
media (`previews.bif`). While the server is still making them, the bar
shows the time alone.

The sleep timer (Down, then Sleep timer) pauses after 15 to 90 minutes
or at the end of what plays, as on the other apps. In its last
30 s it says "Pausing in 12 s" with Keep watching, which adds the same time
again (or lets this title play to its end). Paused by it for 10 minutes,
the player leaves; it isn't kept from one time the player opens to the
next.

### Audio and subtitles

- Roku shows a subtitle track only with captions on, a Roku-wide setting.
  When Pley picks a track it turns captions on for that playback and
  restores them after.
- Subtitles look as the Roku's caption settings say (Settings →
  Accessibility → Captions style). The app never sets `captionStyle`,
  which would change them for the whole Roku.
- Text subtitles go to the Video node as WebVTT.
- **PGS subtitles** are drawn by the app over the video
  (`components/lib/Pictures.brs`), so the file plays as it is and HDR stays
  HDR. The app reports `hdmv_pgs_subtitle` in `subtitle_codecs` with
  `picture_subtitles_sideload`, and asks for no `subtitle_index` for them.
  The session's key opens the track's pictures:
  `/api/media/{key}/subtitles-{index}.json?at={ms}` lists each one's time,
  its place on the subtitle plane and its PNG beside it (named with the
  list's `at`). The server reads the pictures around `at` first: while the
  list isn't whole, the app asks again every 3 s from where the video is
  (at once after a seek), and keeps what it has. The subtitle plane goes
  over the picture as the web player puts it: a DVD-sized plane (576 lines
  or fewer) stretches over it, and an HD plane of another shape than the
  video (a film cropped to 1920×800) keeps its own, fitted to the screen.
  The file says only how its video is stored, so a video of 576 lines or
  fewer is taken to show as 16:9. A picture shows while the video plays,
  under the skip button and the player's panels; paused or seeking, Roku's
  bar has the bottom of the screen. The next picture loads in a second
  frame while one shows, and one that fails to load loads again 3 s later.
  The Video node tells its position every 0.2 s while pictures are drawn,
  else every second.
- Roku's options list only text tracks, and a conversion's one audio.
  Pley's menu (Down) lists every track of the file, as the title's page
  does: Subtitles (Off, then each, PGS too) and Audio (Automatic, then
  each). A pick there is saved for the show or movie, and the stream opens
  again where it was with it. The remote on the profile's other devices
  picks them the same way ([Remote control](#remote-control)).
  A text track picked in the options takes the place of the pictures, and
  captions turned off there turn them off; captions turned on without a
  text track leave them be.
- A server that can't make the pictures has them burned in: the stream
  reopens where it was with `subtitle_index`, and caps without PGS in
  `subtitle_codecs` or `picture_subtitles_sideload` (to the server, a
  listed codec is the player's to show, never burned in), as for VobSub
  and DVB, which are always burned in. The app keeps burning them in until
  the player closes. That is an older server (`404` "unknown file", or
  "unknown playlist file" for a stream, as its keyed media knows no such
  name), a transcoder too old to copy the track (`503` "update it"), or a
  track that isn't PGS. Anything else (a disk asleep, the transcoder
  restarting or out of reach, the network) is asked again 3 s later:
  burning them in would need the same server.
- **Subtitle sync:** Left and Right move subtitles 0.1 s, up to 10 s either
  way. The server shifts text cues (`offset_ms`) and the stream reopens
  where it was; pictures drawn by the app move as the offset changes, and
  the stream plays on. The offset is kept per file per server (the latest
  50).
- **Find subtitles…** searches OpenSubtitles in the profile's subtitle
  language, or another that Language, above the results, picks
  (`components/lib/SubtitleSearch.brs`). OK on a result adds it on the
  server and shows it.
- The server picks the audio this Roku plays best in the profile's
  language. Any audio but the file's default makes the file a stream with
  its video copied.
- Picks in Roku's options are saved for the show or movie, so the next
  episode starts with the same tracks. Roku's audio tracks are matched to
  the file's only when their count and languages agree.

### Quality

The menu, Original and Automatic follow [QUALITY.md](QUALITY.md#the-menu);
the code is `components/lib/AutoQuality.brs`. On Roku:

- **Settings → Quality** is kept on the Roku. One chosen in the player's
  menu, or by the remote on another device, becomes the setting, and the
  title reopens at it, played as that quality plays (the original as it
  is), whatever the player converted before.
- **Conversions the player chose:** a file that doesn't play on this Roku,
  or ends short, is converted, and Quality says so above its hints ("This
  Roku couldn't play the original file."). One that has its data but
  stands still is converted once a title, as on Android TV, and says so
  as the conversion plays: "The original didn't play smoothly on this
  Roku. Play original is in Quality." Quality then offers Play original
  above its choices (Automatic's menu has Original instead).
- **Several sizes:** in an adaptive session the Video node moves between
  sizes itself. Other streams come in one size, so the app steps down.
- **Signals:** a wait is the Video node's `buffering` after at least 2 s of
  playing, and only when `bufferingStatus.isUnderrun` says it is for want
  of data. Network speed comes from the HLS segments' sizes and download
  times. Roku reports neither read-ahead nor dropped frames.
- Roku follows the rules for
  [players that go black between streams](QUALITY.md#players-that-go-black-between-streams):
  a conversion whose bitrate can change is lowered in place; the original,
  or a fixed-bitrate conversion, moves to a new session at least one
  quality lower after a wait of 2 s or more.
- **Memory:** each step down is kept 30 minutes per server and told to the
  server. When the server asks for a speed test, the app times 8 MiB of
  `GET /api/playback/speedtest`, giving up after about 3 s.

### Live TV

Live TV works as in the [Android app](ANDROID.md#live-tv). On Roku
(`components/screens/LiveTvScreen`, `LivePlayerScreen`,
`components/lib/Live.brs`):

- **Paging:** channels come 50 at a time, and the guide only for those 50
  (`GET /api/live/guide?ids=`). A page of channels or a part of the guide
  that didn't load is asked for again once 10 s have passed, as the list
  moves (in the player, as the list moves or Up or Down is pressed). With
  nothing to show, the screen says why in Android's words, a title and a
  line under it: "Live TV didn't load" with the server's words and Try
  again, "Live TV isn't open to this profile", "No channels yet", "No
  favourites yet", "Nothing watched yet", "No channels match", "No
  channels in this group." The player's channel list says the same of a
  category or search without channels.
- **Categories** run down the left under Search. Rewind and fast forward
  move a screen at a time. The category, the tab and the last channel
  watched are kept per server and profile (registry section `liveViews`):
  the list opens at that channel, back from the player and after a
  restart.
- **Search** (the entry over the categories, or Replay) is typed on
  Roku's keyboard, the groups it names listed under "Groups found" at the
  top of the categories; Clear in it, or Back, goes back to the category
  shown before. A channel watched from a search (here, in the player's
  list or in the app's Search) makes Up and Down in the player go through
  every channel, as on Android.
- **Guide:** Roku's TimeGrid, 50 channels by two hours. Rewind and fast
  forward move two hours, Play goes back to now. OK on what's on watches
  the channel; on a later programme it shows its details, as Options (\*)
  does on any programme (on a card, what's on). Up and Down past a page's
  ends bring the page before or after; Up from the category's first
  channel goes to the Guide and Channels tabs, and Down stops at its last,
  as on Android (the TimeGrid goes round its rows, so at the category's
  ends the focus is put back at once). Time the guide says nothing of (a
  channel without a guide) is cut at the half hours, so Left and Right go
  through it half an hour at a time, as on Android. TimeGrid draws a
  programme's title alone, and no time over its now line: Android's cells
  show their times too, and the time now sits on its line.
- **A programme's details** show in a panel at the right, as Android's
  side panel: the channel, the programme's picture, title, episode, day
  and times with On now, Live and New and its rating, a bar of how far it
  is and the time left while it airs, then what the guide says of it (or
  that it says nothing more, and "It has ended." once it has). Under them
  are Watch now (Watch channel for a programme not on now) and the
  favourite, which changes in place. Back or Left closes it.
- **The guide's channels** are TimeGrid's own column: a channel's logo,
  or where it has none its number and name, a star before them for a
  favourite; Android's cell shows the logo, the number and the name
  together. TimeGrid's `channelInfoComponentName` could draw all three,
  but Roku's documentation doesn't say from which Roku OS it is there (the
  app runs from Roku OS 9), and brs-node, which the smoke run and the
  preview use, doesn't draw it.
- **Badges:** LIVE, else NEW, is a label of its own before the
  programme's title, above the guide and in the player's bar, as on
  Android; the rating stays in the line under the title.
- **Channels:** the other tab, as cards with what's on and what's next.
- **Favourites:** a channel's star changes at once, on its card and in
  the guide, and goes back if the server refuses, a toast saying so, as
  on Android. The counts follow, and Favourites itself, when it shows,
  is asked for again.
- **What's on moves on** each 15 s: the programme above the guide, the
  cards in view and the player's bar and list show how far along it is. Once a programme listed as on now has ended, the cards in view are
  asked for again, and in the player the channel playing
  (`GET /api/live/channels/{id}`), each at most once a minute. The guide's
  programmes come six hours ahead, so the guide itself needs nothing new.
- **Back** goes up a step at a time: a programme's details closed, the
  guide back to now (when another time shows, or a programme not on now
  has focus), a search back to the category before, the guide or the
  cards to the categories, then out of Live TV.
- **The player** tunes with `POST …/watch`, waits for the server's live HLS
  playlist (up to 50 s), then plays it with `Live = true`. Up and Down
  change channel, tuning 0.7 s after the last press: a Roku starts its
  Video node again for each channel, so a run of presses tunes only the
  channel it stops on, where Android tunes each at once. OK (as it is let
  go) and Options (\*), Roku's menu key, open the channel list, as
  Android's OK, Menu and Guide do. Options (\*) doesn't change the
  favourite by itself: OK held (half a second) shows a bar of what's on
  and next with Channels and Add to (or Remove from) favourites, which
  changes in place; the bar goes 6 s after it was last touched, or with
  Back. Left and Right do nothing. Roku remotes have no Info key, so
  what's on shows only in that bar, never by itself as a channel starts.
  The favourite's star changes at once, and goes back if the server
  refuses, a toast saying so. Leaving, or changing channel, sends
  `DELETE /api/live/sessions/{session}`.
- **The channel list** opens at the right, on the category Up and Down go
  through, at the channel playing. Options (\*) in it adds the channel in
  focus to the favourites or removes it. Left in it lists the categories
  as Live TV does (OK on one lists its channels instead, Back goes back),
  and Replay searches every channel (Back leaves the search). A channel
  chosen from another category makes Up and Down go through that one;
  from a search, through every channel.
- **Failures:** a playlist the server doesn't give (a `503` while the
  provider sends nothing, or another failure of its), or a stream that
  breaks while its playlist is still there, tunes the channel again twice,
  after 1 s and then 3 s, before the card says why; not when the server
  refused it (`409` every connection in use, `410`, `403`, `404`). Try
  again starts the count again. When the server doesn't answer at all,
  the app says it can't be reached, as on any screen.
- **The card** of what went wrong is Android TV's, in the middle of the
  screen: the channel's logo, why, and Try again, Other channels and Back
  to the guide, Left and Right between them. Every connection in use, it
  lists what each watches and who (three at most), with Watch here instead
  on each this account may take over, which has the focus first; Up and
  Down go between those and the buttons.
- **Taken over:** the server's `live_taken_over` event for the channel
  playing (the scene hands it to the player) stops it at once, before its
  stream breaks off. A stream that breaks asks its playlist again, and a
  `410` there stops it too. Either way the card says "Live TV stopped"
  with the server's words, and Watch again in place of Try again. Taken
  while Up or Down changes channel, the change goes on, with no card.
- **Radio:** a channel with no picture (3 s after it plays, the Video
  node says it has audio and no video) keeps its card, as on Android: its
  logo, "Sound only", and what's on.

### Remote control

The Roku obeys the remote on the profile's other devices that have one
([API.md](API.md#remote-control)), and plays what it casts here (Casts,
below); it shows no remote of its own and casts nothing. Its
players say what they obey when they start (`controls`), that they play
on a TV (`device_kind: "tv"`) and the name given to the Roku in its
settings (`device_name`, "Living room Roku"; a Roku nobody named goes by
its model, without the serial number Roku adds). Commands come on the
event stream, so like the other live updates they need an `http://`
server: from an `https://` one the players say they obey nothing, and
remotes offer only Stop. They obey:

- **Movies and episodes:** play, pause, seek and skip, and next where
  something can come next (an episode, a playlist's or collection's next
  entry). Next moves on without counting the title as watched; next with
  nothing after it does nothing; a video has no previous. The audio and
  subtitles, where the file has them (`audio`, `subtitles`), change as a
  pick in Pley's menu changes them: saved for the show or movie, and the
  stream opened again where it was with them. The quality (`quality`), one
  of those the player's Quality menu offers for the file, changes as a
  pick there does: kept as the Roku's, and the title opened again where it
  was at it.
- **Music:** play, pause, seek, skip, next and previous, as Now Playing's
  buttons and scrubber do, while other screens show too: previous more
  than 3 s into a track, or on the first, goes back to its start. Music
  stopped from another device or by an administrator stops, as a video
  does, its queue kept: Now Playing's line says who stopped it (over
  another screen on top, a message does), and Play goes on.
- **Live TV:** next and previous, channel up and down through the category
  Up and Down go through, round its ends. It has no pause or seek. Stopped
  from another device or by an administrator (`session_stopped`), it
  shows "Live TV stopped", saying who stopped it and, on a line of its
  own, their message, with Watch again, as for a connection taken over;
  a channel change on its way ends too.

Only the player of the session named acts on a command. A title or track
acts only while it plays or is paused: not while it starts, after it
stopped, or while it says what went wrong. Live TV changes channel while a
channel is still tuning too, as Up and Down do, but not after it stopped
or while it says what went wrong. A seek stays within the title and a
skip counts from where it is. Each change is reported at once, so the
remote sees it; another title, track or channel tells the server with its
own session, and the stream opened again for an audio or subtitle track,
or a quality, tells it as it plays. A play on a player already playing, a
pause on one already paused, or the audio, subtitles or quality playing
picked again, says so again at once, so a remote a little behind settles;
a track the file doesn't have, or a quality its menu doesn't offer, does
nothing. Every report of a video says which audio and subtitles play
(`audio_index`, `subtitle_index`, `-1` for none) and the Quality menu's
pick (`quality`: the setting, or Original for a file that doesn't offer
it, which then plays as it is), so the remote ticks them; music's say
none of these. A channel step that waits for the
next page of the category goes on only if the channel is still the one it
started from and none was picked meanwhile (from the list, or Watch
again), the same channel again too; tuning a channel again after its
stream broke off isn't a pick. A seek or skip while Up Next counts down
puts Up Next away, as Rewind does. The rule is
`components/lib/Remote.brs` (the step's, `components/lib/Live.brs`),
pinned by `packages/tv/test/fixtures/remote.json`, which the tests read
(the tracks and the quality by cases of their own).

A command never brings up the player's controls. Paused, the Video node
shows its title, clock and bar whoever paused it, so for the remote's
pause the player turns them off (`enableUI`) until the video plays again,
from either side, or the person presses a key: Play plays on, and the
Video node's other keys are its own again from the next press. That
first OK, left, right or skip only brings its controls back, where the
other TVs act on it at once: the Video node hasn't had its controls to
take it. What the
remote did shows instead for 2 s in the player's notice at the top:
"Paused", "Playing", the skip asked for ("+30 s", "−10 s"), where a seek
went ("Jumped to 12:34"), and the audio, subtitles ("Subtitles off") or
quality picked, named as the player's menus name them
(`remoteNotice` and its neighbours in `components/lib/Remote.brs`). A
command that changes nothing shows nothing; next shows the title as it
begins, and Live TV's channel up and down the channel's card, as Up and
Down do. Music shows nothing of a command: Now Playing is its controls.

**Casts.** While its event stream is open (an `http://` server, as for
the commands), the Roku is one of the devices the profile's remotes play
things on ([API.md](API.md#casting)): the stream names it a receiver by
the `client_id` its players send, a TV with the name its players give,
taking titles and playlists (`casts=item`) and channels while Home's rail
has Live TV (`live`), and switching to the profile of a remote of another
profile that casts to it (`switch`), and opens again when Live TV comes or goes. What a
remote sends plays as the Roku's own Play would, in place of what plays:

- a movie or an episode in the player, from the place sent, else where
  its page's Play starts it, with the audio and subtitles picked on the
  remote's page in place of the profile's choices for that play (asked
  for with the play request's `audio_index`, shown as the menu's pick, and
  not saved), where its first file has them; a show or a season its next
  episode;
- an album or an artist as the music's queue, in order or shuffled, with
  Now Playing; a track its album from it, from the place sent;
- a playlist as its page plays it: music as the queue, videos in the
  player one after the other, shuffled as its Shuffle does;
- a channel in the live player, Up and Down going through All channels.

A video or a channel playing gives way to it, as does music to a video.
"Playing from" and the remote's device ("another device" when it has no
name) shows for 4 s at the top, as the players' notices do (over the
video player, under the title it shows as an item begins); a show,
album, artist or playlist with nothing to play says "Nothing to play",
and what couldn't be fetched says why. A cast for another device, or one
that comes while the Roku is signed out, signing in or asking who's
watching, does nothing (a cast never gets past a profile's PIN), and of
two sent close together the later plays. The rules are
`components/lib/Cast.brs`; the scene (`components/MainScene.brs`)
fetches what a cast needs and plays it.

A remote on another profile of the household casts here too
([API.md](API.md#casting)): its cast names that profile (`profile_id`)
and carries a ticket (`switch_ticket`), and the Roku switches to the
remote's profile before it plays, with the switch-profile grant and the
ticket in place of a PIN (the remote gave its PIN, if it has one). What
plays stops first, and the switch waits for its last progress, its
stopped report and its session's end to reach the server as the profile
that watched it, within 5 s: what hasn't gone by then is dropped, never
sent as the remote's profile. Once switched, the Roku keeps the new
sign-in as a switch from "Who's watching?" does, starts again from Home
as that profile, and the cast plays over it, once. A switch the server
refuses (the ticket expired or was used: `ticket_spent`) or that fails
says "Couldn't switch to the remote's profile" for 4 s, and nothing
plays. A cast naming another profile without a ticket does nothing, and
any other cast that comes while the Roku switches is dropped; once
switched, casts play as ever, the new profile's own too. "Who's
watching?" still takes no casts: the person at the TV is choosing who
watches.

### Requests and Discover

Where an administrator has turned requests on, people ask for movies and
shows found on TMDB, administrators approve them, and Radarr and Sonarr
fetch them ([API.md](API.md#requests)). An administrator's Roku manages
them (a manager, below) once its sign-in allows `approvals`; one signed in
before is offered Sign in, which asks "Sign in again?" first (this TV
shows a code to link it to the server's address again; Sign in and
Cancel). As on Android TV, the rail has one way in, for what the
profile may do: Discover for those who may ask (the requests are at
its top), else Your requests (a manager's Requests). Its number counts
what waits: a manager's pending and failed requests (red while some
failed) and their own news, anyone else's news. What an action did, or
why it didn't work, shows in a toast at the top right, as on the other
TVs: "Asked for “Dune”: an administrator decides.", or the server's
words for one approved as it was asked.

- **Discover**, for those who may ask: a hero for the first trending title
  (its facts, a line of its overview, its main action and Details); when
  trending has nothing the profile may ask for, every row loads at once
  and the hero is the first title of the next one with any. The hero's
  title is read whole for its main action, as its page has it: Request,
  Request more seasons, Open, Tell me too with "Someone asked for it
  already.", Requested or Following beside its request's stage (Following
  with Stop following), or the line that says why there is none (the
  limit and when it frees up, declined and why, a restriction). A movie's
  Request and Tell me too ask at once; a show's Request opens its page with
  Which seasons?. Then rows of posters with their badges ("In library",
  "Partly in library", "Requested", "Declined"): Trending this week,
  Popular movies, Popular shows, Coming to cinemas, Popular anime, each
  loaded as it comes into view and ending with See all (a grid, a page at
  a time; after five pages in a row that leave it short of a screenful,
  as an age limit can, Look further asks for more, and after a page that
  failed, a tile says so, Select asking for it again); then Your requests,
  from the first one asked for. A list with nothing the profile may ask
  for leaves; with nothing in any, the page says "Nothing here right now"
  (search for a title instead), and with every list failed, "Couldn't
  load Discover" with Try again. Down goes to the rows: there an info
  block about the tile focus rests on (0.4 s) takes the hero's place, as on
  Home, and the backdrop follows it: a title's facts and two lines of its
  overview, or a request's stage beside the seasons asked for, who asked
  and when, and its stage's line. At the top, the way to the requests,
  with the rail's number of what waits there. TMDB is searched from
  Search. A poster opens the title's page, or, when a library the account
  can open has the title whole, the library's title, where it plays (a
  show partly there opens its page, where more seasons can be asked
  for).
- **A TMDB title's page:** backdrop and poster, year, runtime or seasons,
  certification, rating; its network (a movie's studio) and genres;
  tagline and four lines of overview; one main action (Open, Request,
  Request more seasons, Tell me too) or the line that says why there is
  none (the limit and when it frees up, declined, not out yet, "Ready, in a
  library you can't open: ask an administrator"); its request's stage and
  what can be done with it; Overview where the four lines cut the
  overview, a panel with the tagline and all of it; rows of seasons, each
  saying how it stands, and the cast. A movie's Request asks at once; a
  show's opens **Which seasons?**, those left to ask for ticked, OK ticking
  or unticking one, then Request 3 seasons. A title gone from TMDB (404)
  says "This title isn't on TMDB", with Back.
- **Your requests** (a manager's **Requests**): tabs by status, each
  with how many it holds (the statuses' counts come with a tab's list,
  and are asked for beside Needs attention's and Yours', which count
  only what they hold). A manager's open on Needs attention (pending,
  and failed ones the server won't retry, oldest first), Yours holds
  their own, and Filter keeps movies or shows, one person, or titles with
  some words (the Roku's keyboard). A card per request: poster, seasons,
  a stage pill (amber waiting for approval; blue approved, searching, or
  downloading with a bar and the time left; green in library; red
  declined or failed; gray gone), who asked and when, the reason or
  note. OK opens a panel at the right, as on the other TVs: at its head
  the title and year, its stage, what was asked for and who asked when,
  what to know of it and, for a manager, where it was sent ("Radarr ·
  HD-1080p · /movies"); then Open where a library has it for the profile,
  what the server lets it do in its order (Withdraw, Remove, Stop
  following, Request again, and to managers Approve, Try again, Decline,
  Decline with a reason…, Mark as added by hand and Delete), and Details,
  the title's page. Withdraw, Remove and Delete ask first in the panel,
  Cancel first; Decline with a reason… opens the keyboard ("Decline
  “Dune”?", with "The person who asked sees it." on someone else's).
  Back closes the panel. With none to list, the page says why: "Nothing
  asked for yet", with Find something to ask for (Discover) for those who
  may ask and don't manage; "Nothing needs you" on Needs attention; "No
  requests match" on a status's tab, Yours or with filters. Opening the
  account's own requests marks their news seen.
- **Search** shows up to 20 titles the libraries lack, read from at most
  3 of TMDB's pages, in a **Discover** row after the libraries' own, for
  those who may ask; OK opens a title's page (the library's title, as in
  Discover, for one a library has whole). When TMDB isn't answering,
  refuses the server's key or isn't open to the account, a tile says so,
  with Select to try again where that helps. TMDB is searched for the
  first 200 characters of the words. See all ends the row: a grid titled
  for the words ("Results for “dune”") of every title TMDB has for them,
  with its badge, a page at a time as See all's are; Back goes back to
  Search. A library title that was asked for shows its request's stage
  on its page, under the line of what it is, and a show with seasons left
  to ask for has Request more seasons among its buttons.
- **Ready to watch:** a requested title that arrives is told of in a
  card at the bottom right: its poster, "“The Last Lighthouse” is ready
  to watch." (a show's seasons once each batch is ready; "3 of your
  requests have news" for several that came while the app wasn't
  listening), Open and Close. It shows once the remote has rested for
  1.5 s and nothing else holds the focus: never over playing video, a
  sign-in, "Who's watching?", a dialog, a panel or a keyboard, nor on
  the requests, Which seasons? or Sort libraries; it waits for them. It
  takes the focus and gives it back when it closes: Close, Back, Up or
  Down, or 10 s untouched. Open opens the title (the requests, for
  several). A dialog over it leaves it waiting under the dialog.

Not offered: Approve's choices of profile, folder and seasons (on the web
and phones; Approve here approves what was asked), bulk approve and
decline, and the requests' settings (the web).

### Videos on a USB drive

On a Roku with a USB port (Roku Ultra, some Roku TVs), while a USB drive
is plugged in, Home's menu has **USB drive** before Settings, as Android
TV and the native Samsung app have it
([ANDROID.md](ANDROID.md#videos-on-a-usb-drive),
[TV_PLATFORMS.md](TV_PLATFORMS.md#videos-on-a-usb-drive)). Its page lists
the drive's folders, then its videos, each sorted as a person sorts names
("Part 2" before "Part 10"); hidden files, what Windows and macOS leave on
a drive, and other files aren't shown. The videos are those Roku's Video
node plays as they are: MKV and WebM (`streamFormat` `mkv`), MP4, M4V and
MOV (`mp4`). The page's title is the name the drive was given (its volume
label), else "USB drive"; under it, the folder's path ("/Films/2019"), at
the drive's top "/" and how full the drive is. A folder opens over the one
before, with ".." first to go back up. A video's line gives its kind, or
where it resumes with a bar of how far through it is, and its size. No
drive, an empty folder and one that can't be read say so in the middle of
the page, under an icon. The page and the menu follow the drive as it is
plugged in or pulled out.

A video plays in Pley's player as it is, with no server, also while the
server is out of reach: the screen that says so has **USB drive** beside
Try again and Switch server while a drive is plugged in, and Back from
the drive's page comes back to it. Seeking, the sleep timer and Playback
details are the same as for the server's videos. What only a server does
isn't there: converting, Quality, seek previews, Find subtitles…, saving
track choices, the next episode, Continue Watching and the remote on
other devices.

- **The drive** is the first of Roku's USB volumes (`ext1:`, `ext2:`…,
  `roFileSystem`'s `GetVolumeList`), read by the drive task
  (`components/tasks/DriveTask`): `roFileSystem` works only in a Task, and
  a large folder takes a while. It lists a folder with
  `GetDirectoryListing`, then `Stat` for each name that shows, 5,000
  entries at most, folders first. Roku tells of a drive plugged in or
  pulled out (`roFileSystemEvent`), and the task looks again as Home, the
  screen saying the server is out of reach or the drive's page shows, for
  a Roku that tells nothing. Roku mounts USB drives read-only.
- **The Video node** is given the file's path as its address
  (`ext1:/Films/Ad Astra.mkv`, no `file://`), as Roku Media Player plays
  a drive's files; Roku's documentation of the Video node names no local
  address, only that `mp4` covers MOV and M4V and `mkv` WebM.
- **Pley's menu** (Down) has Subtitles: Off, the file's own tracks as the
  Video node lists them, which the Roku shows (captions on for them), then
  the `.srt` and `.vtt` files beside the video, named after it and then a
  language, "forced" and SDH in any order (`Film.en.forced.srt`;
  `Film 2.srt` is Film 2's); Audio, the Video node's tracks, where it has
  two or more; Subtitle sync while a file beside it shows; Sleep timer;
  Playback details. Subtitles start as the Roku's captions have them; a
  pick turns captions on or off for the playback, and they go back as
  they were after. Roku's options (\*) offer the file's own tracks too.
- **Subtitle files** are drawn by the app over the video, as Android draws
  them: the drive task reads the file (2 MB at most) as UTF-16 or UTF-32
  when it starts with that byte-order mark, UTF-16 without one when its
  NUL bytes say so, else UTF-8, else Windows-1252, as the other apps do,
  and hands the player its cues without their styling
  (`components/lib/Drive.brs`). They show while the video plays, at the
  foot of the picture, Roku's captions off meanwhile, in the text size,
  colour, opacity and edge the Roku's caption settings give
  (`GetCaptionsOption`; white over a shadow by default), with no
  background behind them. Subtitle sync moves them at once, and is kept by
  the file's path; it doesn't move the file's own tracks, as on Android
  TV.
- **Where it stopped** is kept on the Roku by the file's path (registry
  section `driveResume`, the latest 50 videos) as the server's progress
  would go: as it pauses, once a minute while it plays (Android TV keeps
  it every 10 s; the registry is the Roku's flash) and as the player
  closes. Past 95 % it was watched, and it starts over next time.
- **What doesn't play** says so: a file the Roku can't play as it is ("a
  file on a drive has no server to convert it"), a picture that doesn't
  start in 12 s, and a video that stops, or stands still 12 s, once it
  played (the drive pulled out).

### Settings

Settings is in two panes, as on Android TV: the categories at the left,
the focused category's rows at the right, each with its value on the line
under its label. Right or OK goes into the rows, at their first, and Left
back; choices open in a panel at the right, and switches turn On or Off
with OK. What a change did shows as a toast ("Saved"), and so does why it
didn't.

- **Profile:** who is signed in (the picture, the name, "Managed profile ·
  192.168.1.20:7539"), then Switch profile (for a sign-in that may switch
  them), Language ([Language](#language)), the audio and subtitle defaults, Watch history, Sort libraries,
  Always start as (a household of several) and Manage your account on the
  web, which shows the address to type on a phone or computer. The other
  TVs draw its QR code too; the server draws QR codes of sign-in codes
  only, so Roku shows the address alone.
- **Playback:** Quality, Play the next episode by itself (Up Next counts
  down only while it is on) and Playback details, which has the player
  show how a video plays over it (the registry's `playback` section,
  `showDetails`).
- **Subtitles and display:** subtitles look as the Roku's own caption
  settings say, which the row says.
- **Server:** the server this TV uses, and Switch server.
- **Diagnostics:** Open diagnostics: under "This TV" the app, the model,
  the Roku OS and the Roku's name, then under "What it plays" what it
  tells the server it plays (`capsLines` in `components/lib/Caps.brs`),
  as on Android TV a column of labels and one of values, a line each,
  which Up and Down go through.
- **About:** TMDB's notice, the version, and Open-source licences: Pley's
  licence and the notices of what the package bundles.
- **Sign out:** "Sign this TV out of 192.168.1.20:7539. You'll need a new
  link code to sign in again.", and Sign out, which asks first.

Android TV's Audio (passthrough by format) has no place: the Roku's own
settings decide what goes to a receiver.

Always start as, for a household of several profiles, names the profile
the app starts as, or Profile used last. As on the other TVs, the app
starts as the profile it was left on, but never past its PIN: without a
PIN it goes straight in either way, and with one Always start as opens
its PIN pad first, where Profile used last shows the profiles. Whether it
has one is kept in the registry section `household` each time `GET
/api/auth/me` answers, and forgotten when a sign-in or a switch changes
the profile; until the Roku knows, the start asks "Who's watching?". A
profile since gone from the household (deleted, or another account's,
signed in since) counts as Profile used last, as Settings then shows it.
It is kept on the Roku for each server.

### Language

The app is in English, French and Spanish ([API.md](API.md#language)),
as the other TVs: the account's language, else the one picked on this
Roku, else the Roku's own (`roDeviceInfo`'s `GetCurrentLocale`, where
`fr_CA` counts as French), else the server's (`GET /api/server`), else
English. `packages/tv/test/fixtures/language.json` pins that order, and
the unit tests run against it.

- **The globe:** Choose your server has a small globe at the bottom
  right, on the forget hint's line; the sign-in code has it at the top
  right, on the logo's line. It names the language shown by its two
  letters ("FR"), filled with the accent only while it has the focus.
  Down reaches it on Choose your server and Up on the sign-in code; Left
  and Right never do. OK opens a panel: Automatic, with what that is on
  this Roku (its language, else the server's, else English), then
  English, Français and Español, the choice in force checked. A choice
  shows at once and is kept on the Roku (registry section `language`);
  Automatic forgets it. Typing an address is Roku's own keyboard dialog,
  which has no room for the globe.
- **At a start** the app shows the language it showed last, never another
  while it waits, which would flash. Signed in, it changes once the
  account has been heard (`GET /api/auth/me`); on a Roku in a language
  Pley doesn't have with none picked, once the server has said its own,
  asked again when the event stream comes up if it didn't answer.
- **A change** (another profile, signing out, Settings, the globe) draws
  the sign-in screens, "Who's watching?" and Settings again in place, the
  focus where it was. Any other screen is made again, the one on top at
  once and those under it as they show again, so Home's and the
  libraries' rows, whose titles the server writes, are asked for anew.
  The players play on, and their words come in the new language as they
  show them.
- **Settings → Language:** Automatic, with what that is on this Roku (the
  one picked with the globe, else its language, else the server's), then
  English, Français and Español, each in itself. A choice is the
  account's, on all its devices (`PUT /api/me/language`), and shows once
  the server has it. A Pley from before languages keeps no account's
  language (its `GET /api/server` names none, and the route answers 404
  or 405): there the row is this Roku's, as the globe picks it, with
  Automatic the Roku's own language, and a choice shows at once and is
  kept on the Roku. A server that answers 404 or 405 though it names a
  language is taken as one for as long as it is the server in use.
- **Requests** to the server carry `Accept-Language` with the language
  shown. What the server writes in its own words, such as an error's
  message the app has none for, comes in English.
- **Dates, times and numbers** are written by the app in the language
  shown (`components/lib/I18n.brs`, `Format.brs`): "12,345", "12 345" or
  "12.345"; "Nov 4" or "4 nov."; a 12-hour clock in English when the Roku
  uses one. Counts take CLDR's plural forms: one and other in English,
  one, many and other in French and Spanish.
- **The words** are in `locales/<language>/<area>.json`, in the other
  apps' format (i18next's: `{{name}}`, `_one`, `_many`, `_other`), read
  with `ParseJson` an area at a time and looked up with the app's own
  `t()`: Roku's `tr()` follows the device's language only, so it can't
  show the account's. English stands in for a word missing elsewhere. The
  tests check that every language has every key, with the same values in
  it, that every key the code names is there, and that every word is
  shown somewhere.

## Building and testing

### How it works

| Path | What |
|---|---|
| `source/main.brs` | Starts the scene |
| `components/MainScene.*` | The screen stack, the HTTP agent for pictures, sign-in when the server ends it, "Pley is offline" and the question whether the server answers again, what the user's remotes cast here |
| `components/MusicPlayer.*` | The music queue on the scene's Audio node |
| `components/screens/` | One component per screen |
| `components/shared/` | Screen helpers (`Ui.brs`), title actions (`Titles.brs`), the language's globe and panel |
| `components/items/` | Tiles (a playlist's mosaic too), rows, a page's message with its Retry, the player's skip button, the ready-to-watch card |
| `components/tasks/ApiTask` | The only code that talks to the server |
| `components/tasks/DiscoveryTask` | `pley?` to UDP port 7539 (`apps/server/src/discovery.rs`) |
| `components/tasks/EventsTask` | The server's event stream, on a socket |
| `components/tasks/DriveTask` | The USB drive: which one is plugged in, a folder's lines, subtitle files' cues |
| `components/lib/` | Plain BrightScript (no SceneGraph): the logic the unit tests cover |
| `tests/` | Unit tests of `components/lib` |
| `components/shared/Type.brs`, `components/lib/Type.brs` | Roboto at Android TV's sizes, and Roku's font for text Roboto can't draw |
| `fonts/` | Roboto 400, 500 and 700 (`scripts/fonts.mjs`) |
| `images/` | The shapes and gradients (`scripts/make_images.py`), Lucide's icons in `images/icons` (`scripts/icons.mjs`) |
| `scripts/` | Test runner, smoke run, preview, bench, fonts, icons, images, the words' check (`locales.mjs`) |
| `locales/` | The words, in English, French and Spanish (`<language>/<area>.json`) |
| `licenses/` | Notices for what the package bundles: BrighterScript's `bslib.brs`, Roboto (OFL-1.1) and Lucide (ISC, and MIT for the icons from Feather) |

Rules the code keeps:

- All HTTP goes through `ApiTask`; the render thread never makes a request.
  The USB drive is read only in `DriveTask`.
- The access token goes only in `Authorization: Bearer`, never in an
  address, including the Video node's and the pictures' HTTP agents.
- No token goes to the server's address before the server there proves it
  is the one signed in to, at that address
  ([API.md](API.md#telling-servers-apart)). The access token waits for a
  proof made on this network within the half hour, and the scene (its
  players, pictures and event stream) has the token only while one holds:
  `ApiTask` asks for the next proof before it lapses, and joining or
  leaving a network (Roku's link status) takes the token away until the
  server proves itself again. A request that gets no answer, or an event
  stream that fails when the server doesn't answer a request after it,
  takes the token away too until a new proof. A proof that fails is asked
  again every 30 seconds. A refresh, a profile switch with its PIN and
  a sign-out wait for a proof made then. What can't prove itself gets no
  token, and the app says "Another device answers there." Roku's
  transfers follow redirects, but a proof made at any other address than
  the one asked doesn't verify.
- Videos play from [keyed addresses](API.md#keyed-urls): Roku fetches a
  BIF file of seek previews without its HTTP agent's headers, so the
  previews come from the session's key, beside its media. So do the
  pictures of PGS subtitles and their list, fetched without the token.
- Refreshes run one at a time per server, and the new token pair is saved
  before use (a refresh token works once).
- A request goes with the tokens of the profile it was made for, or not
  at all: one waiting for a refresh or a proof, or refused while a profile
  switch was under way, is dropped once the switch has changed the
  profile, never sent with the new profile's pair.
- The User-Agent is `pleyRoku/<version> (Roku OS <version>; <model>)`; the
  server's `/link` page shows it.
- The event stream (`GET /api/events`) works with `http://` servers only:
  Roku's sockets have no TLS.
- Settings and views are kept in registry sections: `playback`,
  `subtitleSync`, `libraryViews`, `liveViews`, `household`, `startAs`,
  `requests`, `searches`, `driveResume`, `language`.
- Sign-in asks for `library play playlists profiles`, and `requests
  approvals` when the server's metadata lists them in `scopes_supported`
  (an older server refuses scopes it doesn't know). A sign-in from before
  requests learns it from the summary (`sign_in_again`), on Discover and
  the requests.
- **Requests' summary** (`GET /api/requests/summary`;
  `components/lib/Requests.brs`) decides the rail, its number and what the
  screens offer. It is asked with each profile, when the event stream
  connects, a second after request events (once for those that come
  together), after the person acts on a request, and each minute without
  the event stream. An older server's `404`, or a `403` from one built
  before apps could ask, turns requests off until the stream connects
  again or the app starts.
- **Request changes:** request events give every tile of their titles, on
  covered screens too, its new badge: the title's status for the profile
  (`GET /api/discover/{kind}/{tmdb_id}`, once a title for events that come
  together), never one request's. The screen on top that shows requests
  asks again; nothing reloads whole.
  Without the stream the requests, Discover's Your requests and a TMDB
  title's page showing a request ask each 30 s; while something
  downloads, title pages (TMDB's and the libraries'), the requests and
  Discover's Your requests ask each 10 s.
- **Notices** are kept per server and profile (registry section
  `requests`, the newest 40): what was told of each request once its
  card closed, so a title is told of once, on this Roku. The remote's
  rest comes from `TimeSinceLastKeypress` (whole seconds), looked at
  four times a second while a notice waits; a screen changing and a
  card closing count as presses.
- **Times** come as unix seconds, and the app writes them ("in 20 min",
  "3 days ago", "Nov 4", "9:05 PM") in the Roku's time zone and the
  language shown; TMDB's days
  ("2026-11-04") are written as they are.
- **Discover's pictures** come through the server at the size they are
  drawn (`w`), with the scene's HTTP agent like library artwork.
- Launch beacons: `AppLaunchComplete` once the first screen is usable;
  `AppDialogInitiate` and `AppDialogComplete` around a sign-in at launch.

### Commands

In `apps/roku`, with Bun and Node 22 or newer. Bun runs the scripts, and
the tools (bsc, bslint) run on Node, since bslint's plugin loader needs
Node's `module.require`; so `apps/roku` has no `bunfig.toml`.

```powershell
bun install
bun run check     # bsc's compile check, bslint, the unit tests (CI runs this)
bun run smoke     # the whole app in brs-node, against a fake server
bun run bench     # the list work timed in brs-node
bun run preview   # the app in a browser, the keyboard as its remote
bun run images    # the shapes, gradients, watched mark and globe
bun run icons     # Lucide's icons, drawn by Chrome
bun run fonts     # Roboto, from @fontsource/roboto
```

- **Preview** serves the app on http://127.0.0.1:8086/. Arrows, Enter
  (OK), Esc or Backspace (Back), Space (play/pause), `[` and `]` (rewind,
  fast forward) and `*` (options) are the remote. It uses the smoke run's
  fake server, which approves the code itself; `--server
  http://<address>:7539` uses a real one; `--port` changes the port.
- **Unit tests** run `components/lib` and `tests/*.test.brs` in brs-node;
  each `sub test_…()` is a test.
- **Smoke run** starts a fake server (`scripts/fixture-server.mjs`; its
  requests and TMDB titles, all made up, in
  `scripts/fixture-requests.mjs`) and walks every screen, saving each in
  `out/smoke/`. It fails on a BrightScript error, a request without the
  User-Agent, an API request without `Accept-Language`, a token in an
  address, a token before the server proved
  itself (a refresh token without a proof made then), or a refresh token
  used twice. On the way it plays an episode whose PGS subtitles the app
  draws (their list asked for where it starts, then again until whole, and
  a picture loaded), then the same episode on a server without their
  pictures, which burns them in, and picks its subtitles from the
  player's menu; it jumps down a library with its A to Z,
  sorts it by bitrate and filters it by resolution, saves another's
  filters as a smart playlist, plays a collection of movies and a smart
  one of shows, and an artist's tracks, makes a playlist, plays a track
  from Now Playing's Up next, shuffles the queue and puts its order back,
  steps the music's sleep timer through its choices, plays a track from
  under an album's "Disc 2", opens the See all of Search's Discover row,
  asks for a movie and three seasons from Discover, approves a request,
  opens a title from its ready-to-watch card on Home (which waited while
  the requests showed), chooses Español in Settings → Language for the
  account (Home asked for again in it) and Automatic again, plugs in a
  USB drive (a zip brs-node mounts as `ext1:`) and walks its page, a
  folder, an empty one and a video's menu, then pulls it out, opens the
  drive's page from the screen saying the server is out of reach, and
  checks that a server
  without requests is asked nothing but the summary. The fake server also
  checks that a video's reports say which audio and subtitles play, and
  music's don't.
- **brs-node limits:** its own font in place of Roku's (Roboto draws as
  on a Roku), no video or audio (Skip Intro, Up
  Next and its wait at the end, the title as an item begins, reconnecting,
  stalls, media that ends short, the sleep timers' ends, picture
  subtitles shown in time and the remote's commands, which a player obeys
  only once it plays, have only unit tests, as do a drive's subtitle files
  drawn in time and a drive's video's place kept), a drive named "Zip
  Storage", sockets that connect but send
  nothing (the event stream, and the casts it brings, have only unit
  tests), one fetch at a time, the odd dropped key, and no measure of a
  Roku's speed.
- **Images:** `bun run images` runs `scripts/make_images.py` (Python
  standard library): the nine-patch shapes Roku tints (pills, a tile's
  well, the corners that round it and its focus ring), the gradients over
  backdrops, the watched mark and the guide's cells. `bun run icons`
  draws Lucide's icons through Chrome (`scripts/svg-render.mjs` at the
  repository root), white for a Poster's `blendColor` to tint, the USB
  drive page's at a message's size, and `bun
  run fonts` unpacks Roboto from the web TV app's `@fontsource/roboto`;
  both need the repository's packages installed. The home screen icons
  and splash screens come from `bun scripts/render-icons.mjs` at the
  repository root. All of these are committed.

## Releases

The app is the `roku` unit in [RELEASING.md](RELEASING.md); its version is
the manifest's `major_version`, `minor_version` and `build_version`.

The manifest asks for SceneGraph 1.2, which every Roku OS from 9 runs.

## Known limits

- No playback speed or audio delay: the Video node has neither.
- Live updates need an `http://` server; with `https://`, Home and the
  requests' summary refresh once a minute, the requests, Discover's Your
  requests and a title page with a request each 30 s. The remote on other
  devices reaches the Roku only through them; with `https://`, the Roku
  offers remotes only Stop and takes no casts, and a film or music
  stopped from another device stops at its next report (within 10 s while
  playing, 30 s while paused), a channel when its stream breaks off.
- Audio other than the file's default isn't played as it is.
- Live TV has no channel numbers to type: Roku remotes have no number keys.
- A USB drive's TS, M2TS, AVI and MPEG files aren't listed: the Video node
  has no stream format for them (a transport stream only as HLS's
  segments). The drive's make and model aren't given to apps, so its page
  is titled with the drive's label. A size `Stat` gives as no positive
  number isn't shown. A drive's video has no Chapters: the Video node
  doesn't tell a file's chapters.
- Not yet: an editable music queue.
- Text in scripts other than the Latin alphabets of Western Europe is in
  Roku's font, not Roboto: Roku has no fallback from one font to another.

