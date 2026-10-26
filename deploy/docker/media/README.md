# Your media

This folder is mounted read-only at `/media` in the container when
`PLEY_MEDIA_DIR` is `./media` (the default). Put your files in the three
folders below, or point `PLEY_MEDIA_DIR` in `.env` at the folder that holds
your media already, laid out the same way. Git ignores everything you add
here.

In pley, add one library per kind (**Settings → Libraries**): a Movies library
on `/media/Movies`, a TV Shows library on `/media/TV Shows`, a Music library on
`/media/Music`. pley never changes or moves these files.

## How to name files

pley reads names the way Plex does. The year makes matches certain.

```
media/
├── Movies/
│   ├── Blade Runner (1982)/
│   │   ├── Blade Runner (1982) {edition-Final Cut}.mkv
│   │   ├── Blade Runner (1982) {edition-Final Cut}.en.srt     subtitles
│   │   ├── poster.jpg                                          your own artwork wins
│   │   └── Trailers/
│   │       └── Official Trailer.mkv
│   ├── Arrival (2016).mkv                                      no folder needed
│   ├── Arrival (2016)-trailer.mkv                              extras by suffix too
│   └── Dune Part Two (2024) {tmdb-693134}.mkv                  a TMDB id settles it
├── TV Shows/
│   └── Severance (2022)/
│       ├── poster.jpg
│       ├── Season 01/
│       │   ├── Severance (2022) - S01E01 - Good News About Hell.mkv
│       │   ├── Severance (2022) - S01E01 - Good News About Hell.eng.forced.srt
│       │   └── Severance (2022) - S01E02 - Half Loop.mkv
│       └── Season 02/
│           └── Severance - 2x01 - Hello, Ms. Cobel.mkv          1x02 works as well
└── Music/
    └── Daft Punk/
        └── Discovery (2001)/
            ├── cover.jpg
            ├── 01 - One More Time.flac
            └── 02 - Aerodynamic.flac
```

- **Movies:** `Title (Year)`, alone or in a folder of the same name. Release
  names (`Title.2019.2160p.WEB-DL...`) work too. `{edition-...}` names an
  edition; `{tmdb-123}` fixes the match.
- **TV shows:** a folder per show, `Season NN` folders, and `S01E02` (or
  `1x02`, or `02 - Title` inside a season folder) in each file name.
  Date-based shows (`Show - 2024-03-01.mkv`) and anime numbering work too.
- **Music:** `Artist/Album/track`, with the tags in the files. Covers come
  from `cover.jpg` or `folder.jpg`, or from the files.
- **Subtitles** sit next to the video with the same name, plus a language
  and optionally `forced` or `sdh`: `Movie (2019).en.srt`,
  `Movie (2019).eng.forced.srt`. A `Subs/` folder works too.
- **Artwork** next to the media wins over downloaded artwork: `poster.jpg`,
  `fanart.jpg`, `season01-poster.jpg`, `Movie (2019)-fanart.jpg`.
