//! Remembered audio and subtitle choices, and each user's defaults.
//!
//! When a user picks an audio track or turns subtitles on or off, the choice
//! is stored for the show (episodes) or the movie, as track properties
//! (language, title, forced). Every episode then starts with the matching
//! tracks of its own file, so subtitles don't have to be re-enabled episode
//! after episode. Titles without a choice start with the user's default
//! languages ([`Defaults`]).

use pley_transcoder_api::{MediaStream, StreamKind};
use serde::{Deserialize, Serialize};

use crate::catalog::ItemRow;
use crate::db::Db;
use crate::util::now;

#[derive(Debug, Clone, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct StreamPreference {
    pub audio_language: Option<String>,
    pub audio_title: Option<String>,
    pub subtitles: bool,
    pub subtitle_language: Option<String>,
    pub subtitle_title: Option<String>,
    pub subtitle_forced: bool,
}

/// When subtitles come on by themselves, for titles without a choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleMode {
    /// Never.
    Off,
    /// Forced subtitles (foreign dialogue) in the language of the audio.
    #[default]
    Forced,
    /// Full subtitles in the subtitle language when the audio is in another.
    Foreign,
    /// Always, in the subtitle language.
    Always,
}

impl SubtitleMode {
    pub fn as_str(self) -> &'static str {
        match self {
            SubtitleMode::Off => "off",
            SubtitleMode::Forced => "forced",
            SubtitleMode::Foreign => "foreign",
            SubtitleMode::Always => "always",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "off" => SubtitleMode::Off,
            "foreign" => SubtitleMode::Foreign,
            "always" => SubtitleMode::Always,
            _ => SubtitleMode::Forced,
        }
    }
}

/// A user's default tracks (account settings).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Defaults {
    /// Preferred audio language; `None`: the file's default track.
    pub audio_language: Option<String>,
    /// Language subtitles are read in; `None`: the audio language.
    pub subtitle_language: Option<String>,
    pub subtitle_mode: SubtitleMode,
}

pub async fn defaults(db: &Db, user_id: i64) -> sqlx::Result<Defaults> {
    let row: Option<(Option<String>, Option<String>, String)> =
        sqlx::query_as("SELECT audio_language, subtitle_language, subtitle_mode FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&db.read)
            .await?;
    Ok(row
        .map(|(audio_language, subtitle_language, mode)| Defaults {
            audio_language,
            subtitle_language,
            subtitle_mode: SubtitleMode::parse(&mode),
        })
        .unwrap_or_default())
}

/// Whether `code` looks like an ISO 639 language code (`en`, `eng`).
pub fn valid_language(code: &str) -> bool {
    (2..=3).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_lowercase())
}

/// Tracks to start a file with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Selection {
    pub audio: Option<i64>,
    pub subtitle: Option<i64>,
}

/// The item a preference is stored on: the show for episodes, else the item.
pub fn scope(row: &ItemRow) -> i64 {
    match row.kind.as_str() {
        "episode" => row.grandparent_id.unwrap_or(row.id),
        _ => row.id,
    }
}

pub async fn load(db: &Db, user_id: i64, scope_id: i64) -> sqlx::Result<Option<StreamPreference>> {
    sqlx::query_as(
        "SELECT audio_language, audio_title, subtitles, subtitle_language, subtitle_title, subtitle_forced
         FROM stream_preferences WHERE user_id = ? AND item_id = ?",
    )
    .bind(user_id)
    .bind(scope_id)
    .fetch_optional(&db.read)
    .await
}

pub async fn save(db: &Db, user_id: i64, scope_id: i64, pref: &StreamPreference) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO stream_preferences (user_id, item_id, audio_language, audio_title, subtitles, subtitle_language,
                                         subtitle_title, subtitle_forced, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id, item_id) DO UPDATE SET
            audio_language = excluded.audio_language, audio_title = excluded.audio_title,
            subtitles = excluded.subtitles, subtitle_language = excluded.subtitle_language,
            subtitle_title = excluded.subtitle_title, subtitle_forced = excluded.subtitle_forced,
            updated_at = excluded.updated_at",
    )
    .bind(user_id)
    .bind(scope_id)
    .bind(&pref.audio_language)
    .bind(&pref.audio_title)
    .bind(pref.subtitles)
    .bind(&pref.subtitle_language)
    .bind(&pref.subtitle_title)
    .bind(pref.subtitle_forced)
    .bind(now())
    .execute(&db.write)
    .await?;
    Ok(())
}

/// Describe the chosen tracks of a file as a preference.
pub fn from_selection(streams: &[MediaStream], audio: Option<i64>, subtitle: Option<i64>) -> StreamPreference {
    let find = |index: Option<i64>, kind: StreamKind| {
        index.and_then(|i| streams.iter().find(|s| s.index == i && s.kind == kind))
    };
    let audio = find(audio, StreamKind::Audio);
    let subtitle = find(subtitle, StreamKind::Subtitle);
    StreamPreference {
        audio_language: audio.and_then(|a| a.language.clone()),
        audio_title: audio.and_then(|a| a.title.clone()),
        subtitles: subtitle.is_some(),
        subtitle_language: subtitle.and_then(|s| s.language.clone()),
        subtitle_title: subtitle.and_then(|s| s.title.clone()),
        subtitle_forced: subtitle.is_some_and(|s| s.forced),
    }
}

/// Pick the tracks of `streams` that best match `pref`, or without one the
/// user's `defaults` (see [`by_default`]).
pub fn resolve(pref: Option<&StreamPreference>, defaults: &Defaults, streams: &[MediaStream]) -> Selection {
    let audio_streams: Vec<&MediaStream> = streams.iter().filter(|s| s.kind == StreamKind::Audio).collect();
    let subtitle_streams: Vec<&MediaStream> = streams.iter().filter(|s| s.kind == StreamKind::Subtitle).collect();
    let default_audio = audio_streams.iter().copied().find(|s| s.default).or(audio_streams.first().copied());

    let Some(pref) = pref else {
        return by_default(defaults, &audio_streams, &subtitle_streams, default_audio);
    };

    let audio = pref.audio_language.as_deref().and_then(|language| {
        audio_streams
            .iter()
            .filter(|s| same_language(s.language.as_deref(), Some(language)))
            .max_by_key(|s| {
                let commentary = is_commentary(s.title.as_deref());
                let wanted_commentary = is_commentary(pref.audio_title.as_deref());
                (
                    same_title(s.title.as_deref(), pref.audio_title.as_deref()),
                    commentary == wanted_commentary,
                    s.default,
                    // Prefer the earliest track among equals.
                    std::cmp::Reverse(s.index),
                )
            })
            .map(|s| s.index)
    });
    // Only pin the audio when it differs from what the file plays anyway.
    let audio = audio.filter(|&i| default_audio.is_none_or(|d| d.index != i));

    let subtitle = if pref.subtitles {
        subtitle_streams
            .iter()
            .filter(|s| match pref.subtitle_language.as_deref() {
                Some(language) => same_language(s.language.as_deref(), Some(language)),
                // An untagged track was chosen: only a track with the same title will do.
                None => s.language.is_none() && same_title(s.title.as_deref(), pref.subtitle_title.as_deref()),
            })
            .max_by_key(|s| {
                (
                    s.forced == pref.subtitle_forced,
                    same_title(s.title.as_deref(), pref.subtitle_title.as_deref()),
                    is_sdh(s.title.as_deref()) == is_sdh(pref.subtitle_title.as_deref()),
                    // Text is shown by the player; images need a transcode.
                    s.text_based.unwrap_or(true),
                    s.default,
                    std::cmp::Reverse(s.index),
                )
            })
            .map(|s| s.index)
    } else {
        None
    };
    Selection { audio, subtitle }
}

/// Tracks for a title the user picked none for: the main track in their audio
/// language (the file's default without one), and subtitles as their mode
/// says. Forced subtitles in the language of the audio translate foreign
/// dialogue the viewer is meant to read: every mode but `off` shows them
/// when nothing else is shown.
fn by_default(
    defaults: &Defaults,
    audio_streams: &[&MediaStream],
    subtitle_streams: &[&MediaStream],
    default_audio: Option<&MediaStream>,
) -> Selection {
    let preferred = defaults.audio_language.as_deref().and_then(|language| {
        audio_streams
            .iter()
            .copied()
            .filter(|s| same_language(s.language.as_deref(), Some(language)))
            .max_by_key(|s| (!is_commentary(s.title.as_deref()), s.default, std::cmp::Reverse(s.index)))
    });
    let playing = preferred.or(default_audio);
    // Only pin the audio when it differs from what the file plays anyway.
    let audio = preferred.filter(|p| default_audio.is_none_or(|d| d.index != p.index)).map(|s| s.index);

    let heard = playing.and_then(|a| a.language.as_deref());
    let read = defaults.subtitle_language.as_deref().or(defaults.audio_language.as_deref());
    let forced = || {
        subtitle_streams
            .iter()
            .filter(|s| s.forced && heard.is_some() && same_language(s.language.as_deref(), heard))
            .max_by_key(|s| s.text_based.unwrap_or(true))
            .map(|s| s.index)
    };
    let full = |language: &str| {
        subtitle_streams
            .iter()
            .filter(|s| same_language(s.language.as_deref(), Some(language)))
            .max_by_key(|s| {
                (
                    !s.forced,
                    !is_sdh(s.title.as_deref()),
                    // Text is shown by the player; images need a transcode.
                    s.text_based.unwrap_or(true),
                    s.default,
                    std::cmp::Reverse(s.index),
                )
            })
            .map(|s| s.index)
    };
    let subtitle = match defaults.subtitle_mode {
        SubtitleMode::Off => None,
        SubtitleMode::Forced => forced(),
        // An untagged audio track isn't taken for foreign.
        SubtitleMode::Foreign => match read {
            Some(read) if heard.is_some_and(|h| !same_language(Some(h), Some(read))) => full(read).or_else(forced),
            _ => forced(),
        },
        SubtitleMode::Always => read.and_then(full).or_else(forced),
    };
    Selection { audio, subtitle }
}

/// Language codes as ffprobe reports them (`eng`, sometimes `en`), compared
/// on their ISO 639-2 form.
fn same_language(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => normalize_language(a) == normalize_language(b),
        _ => false,
    }
}

fn normalize_language(code: &str) -> String {
    let code = code.trim().to_ascii_lowercase();
    let code = code.split(['-', '_']).next().unwrap_or_default();
    match code {
        "en" => "eng",
        "fr" | "fra" => "fre",
        "de" | "deu" => "ger",
        "es" => "spa",
        "it" => "ita",
        "pt" => "por",
        "ja" => "jpn",
        "ko" => "kor",
        "zh" | "zho" => "chi",
        "ru" => "rus",
        "nl" | "nld" => "dut",
        "sv" => "swe",
        "no" | "nb" | "nob" => "nor",
        "da" => "dan",
        "fi" => "fin",
        "pl" => "pol",
        "ar" => "ara",
        "he" => "heb",
        "hi" => "hin",
        "tr" => "tur",
        "bg" => "bul",
        "ca" => "cat",
        "cs" | "ces" => "cze",
        "el" | "ell" => "gre",
        "fa" | "fas" => "per",
        "hr" => "hrv",
        "hu" => "hun",
        "id" => "ind",
        "ro" | "ron" => "rum",
        "sk" | "slk" => "slo",
        "sr" => "srp",
        "th" => "tha",
        "uk" => "ukr",
        "vi" => "vie",
        other => other,
    }
    .to_owned()
}

fn same_title(a: Option<&str>, b: Option<&str>) -> bool {
    let norm = |t: Option<&str>| t.map(|t| t.trim().to_lowercase()).filter(|t| !t.is_empty());
    norm(a) == norm(b)
}

fn is_sdh(title: Option<&str>) -> bool {
    title.is_some_and(|t| {
        let t = t.to_lowercase();
        t.contains("sdh") || t.contains("hearing") || t.split(|c: char| !c.is_alphanumeric()).any(|w| w == "cc")
    })
}

fn is_commentary(title: Option<&str>) -> bool {
    title.is_some_and(|t| t.to_lowercase().contains("commentary"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A user who set no defaults.
    const NONE: Defaults =
        Defaults { audio_language: None, subtitle_language: None, subtitle_mode: SubtitleMode::Forced };

    fn audio(index: i64, language: &str, title: Option<&str>, default: bool) -> MediaStream {
        MediaStream {
            index,
            kind: StreamKind::Audio,
            codec: "aac".into(),
            language: Some(language.into()),
            title: title.map(Into::into),
            default,
            ..Default::default()
        }
    }

    fn sub(index: i64, language: Option<&str>, title: Option<&str>, forced: bool, text: bool) -> MediaStream {
        MediaStream {
            index,
            kind: StreamKind::Subtitle,
            codec: if text { "subrip" } else { "hdmv_pgs_subtitle" }.into(),
            language: language.map(Into::into),
            title: title.map(Into::into),
            forced,
            text_based: Some(text),
            ..Default::default()
        }
    }

    /// Episode 1 and episode 2 of a show, with different track layouts.
    fn episode(shift: i64) -> Vec<MediaStream> {
        vec![
            MediaStream { index: 0, kind: StreamKind::Video, codec: "h264".into(), ..Default::default() },
            audio(1, "jpn", None, true),
            audio(2 + shift, "eng", Some("English 5.1"), false),
            audio(3 + shift, "eng", Some("Director's Commentary"), false),
            sub(4 + shift, Some("eng"), Some("Signs & Songs"), true, true),
            sub(5 + shift, Some("eng"), Some("Full"), false, true),
            sub(6 + shift, Some("eng"), Some("SDH"), false, false),
            sub(7 + shift, Some("fre"), None, false, true),
        ]
    }

    #[test]
    fn choices_carry_over_to_the_next_episode() {
        let first = episode(0);
        let pref = from_selection(&first, Some(1), Some(5));
        assert_eq!(pref.subtitle_language.as_deref(), Some("eng"));
        assert!(pref.subtitles && !pref.subtitle_forced);

        // Next episode: tracks moved; the full English subtitles are found again.
        let next = episode(10);
        assert_eq!(resolve(Some(&pref), &NONE, &next), Selection { audio: None, subtitle: Some(15) });
    }

    #[test]
    fn forced_and_sdh_choices_are_kept_apart() {
        let pref = from_selection(&episode(0), None, Some(4));
        assert!(pref.subtitle_forced);
        assert_eq!(resolve(Some(&pref), &NONE, &episode(10)).subtitle, Some(14));

        let pref = from_selection(&episode(0), None, Some(6));
        assert_eq!(resolve(Some(&pref), &NONE, &episode(10)).subtitle, Some(16));
    }

    #[test]
    fn audio_language_is_followed_but_commentary_avoided() {
        let pref = from_selection(&episode(0), Some(2), None);
        assert_eq!(pref.audio_language.as_deref(), Some("eng"));
        assert_eq!(resolve(Some(&pref), &NONE, &episode(10)), Selection { audio: Some(12), subtitle: None });
        // Choosing the default track pins nothing.
        let pref = from_selection(&episode(0), Some(1), None);
        assert_eq!(resolve(Some(&pref), &NONE, &episode(10)).audio, None);
    }

    #[test]
    fn subtitles_off_stays_off_and_missing_languages_are_skipped() {
        let pref = from_selection(&episode(0), None, None);
        assert!(!pref.subtitles);
        assert_eq!(resolve(Some(&pref), &NONE, &episode(10)).subtitle, None);

        let pref = StreamPreference { subtitles: true, subtitle_language: Some("ger".into()), ..Default::default() };
        assert_eq!(resolve(Some(&pref), &NONE, &episode(0)).subtitle, None);
    }

    #[test]
    fn language_codes_are_normalized() {
        let pref = StreamPreference { subtitles: true, subtitle_language: Some("en".into()), ..Default::default() };
        assert_eq!(resolve(Some(&pref), &NONE, &episode(0)).subtitle, Some(5));
        assert!(same_language(Some("fra"), Some("fre")));
        assert!(same_language(Some("en-US"), Some("eng")));
    }

    #[test]
    fn a_chosen_commentary_track_is_followed() {
        let pref = from_selection(&episode(0), Some(3), None);
        assert_eq!(pref.audio_title.as_deref(), Some("Director's Commentary"));
        assert_eq!(resolve(Some(&pref), &NONE, &episode(10)).audio, Some(13));
    }

    #[test]
    fn missing_audio_languages_keep_the_default_track() {
        let pref = StreamPreference { audio_language: Some("ger".into()), ..Default::default() };
        assert_eq!(resolve(Some(&pref), &NONE, &episode(0)), Selection::default());
        // Indexes that are not audio (or don't exist) describe nothing.
        let pref = from_selection(&episode(0), Some(4), Some(1));
        assert_eq!(pref, StreamPreference::default());
    }

    #[test]
    fn untagged_subtitles_are_found_again_by_title_only() {
        let streams = |shift: i64| {
            vec![
                audio(1, "eng", None, true),
                sub(2 + shift, None, Some("Commentary"), false, true),
                sub(3 + shift, None, Some("Karaoke"), false, true),
                sub(4 + shift, Some("eng"), Some("Karaoke"), false, true),
            ]
        };
        let pref = from_selection(&streams(0), None, Some(3));
        assert_eq!((pref.subtitle_language.as_deref(), pref.subtitle_title.as_deref()), (None, Some("Karaoke")));
        assert_eq!(resolve(Some(&pref), &NONE, &streams(10)).subtitle, Some(13), "the untagged track with that title");
        // No untagged track with that title: subtitles stay off.
        let pref = StreamPreference { subtitles: true, subtitle_title: Some("Signs".into()), ..Default::default() };
        assert_eq!(resolve(Some(&pref), &NONE, &streams(0)).subtitle, None);
    }

    #[test]
    fn text_subtitles_win_over_images_and_sdh_is_recognized() {
        let streams = vec![
            audio(1, "eng", None, true),
            sub(2, Some("eng"), None, false, false),
            sub(3, Some("eng"), None, false, true),
            sub(4, Some("eng"), Some("English [CC]"), false, true),
        ];
        let pref = StreamPreference { subtitles: true, subtitle_language: Some("eng".into()), ..Default::default() };
        assert_eq!(resolve(Some(&pref), &NONE, &streams).subtitle, Some(3), "text, no SDH, earliest");
        let sdh = StreamPreference { subtitle_title: Some("SDH".into()), ..pref };
        assert_eq!(resolve(Some(&sdh), &NONE, &streams).subtitle, Some(4), "`CC` marks SDH too");
        assert!(!is_sdh(Some("Accent")), "a word, not two letters inside one");
    }

    #[test]
    fn choices_are_kept_per_show_or_movie() {
        let mut episode = ItemRow::test_row(3, "episode", "Pilot");
        (episode.parent_id, episode.grandparent_id) = (Some(2), Some(1));
        assert_eq!(scope(&episode), 1);
        assert_eq!(scope(&ItemRow::test_row(9, "movie", "Alien")), 9);
        assert_eq!(scope(&ItemRow::test_row(8, "episode", "Orphan")), 8, "an episode without a show");
    }

    #[test]
    fn without_a_preference_only_matching_forced_subtitles_show() {
        let mut streams = episode(0);
        assert_eq!(resolve(None, &NONE, &streams), Selection::default());
        // English audio by default: the English forced track is shown.
        streams[1].default = false;
        streams[2].default = true;
        assert_eq!(resolve(None, &NONE, &streams).subtitle, Some(4));
    }

    #[test]
    fn titles_without_a_choice_start_with_the_users_defaults() {
        // A Japanese film: English 5.1 and a commentary, full and forced
        // English subtitles (and SDH), French ones.
        let film = episode(10);
        let with = |audio: Option<&str>, subtitles: Option<&str>, mode| Defaults {
            audio_language: audio.map(Into::into),
            subtitle_language: subtitles.map(Into::into),
            subtitle_mode: mode,
        };
        // English audio, its main track, not the commentary.
        let english = resolve(None, &with(Some("en"), None, SubtitleMode::Forced), &film);
        assert_eq!(english, Selection { audio: Some(12), subtitle: Some(14) }, "and its forced subtitles");
        // The default track is already Japanese: not pinned.
        assert_eq!(resolve(None, &with(Some("jpn"), None, SubtitleMode::Off), &film), Selection::default());
        // Foreign audio, subtitles in the reader's language: the full track, not SDH or forced.
        let reader = with(None, Some("eng"), SubtitleMode::Foreign);
        assert_eq!(resolve(None, &reader, &film).subtitle, Some(15));
        // The audio is in the reader's language: only forced ones.
        let at_home = with(Some("eng"), Some("eng"), SubtitleMode::Foreign);
        assert_eq!(resolve(None, &at_home, &film), Selection { audio: Some(12), subtitle: Some(14) });
        // Always, in French; in a language the file lacks, forced ones at most.
        assert_eq!(resolve(None, &with(None, Some("fr"), SubtitleMode::Always), &film).subtitle, Some(17));
        assert_eq!(resolve(None, &with(None, Some("ger"), SubtitleMode::Always), &film).subtitle, None);
        assert_eq!(resolve(None, &with(Some("eng"), Some("ger"), SubtitleMode::Always), &film).subtitle, Some(14));
        // Off is off, forced subtitles included.
        assert_eq!(resolve(None, &with(Some("eng"), None, SubtitleMode::Off), &film).subtitle, None);
        // A remembered choice wins over the defaults.
        let chosen = StreamPreference { subtitles: false, ..Default::default() };
        assert_eq!(resolve(Some(&chosen), &with(None, Some("fre"), SubtitleMode::Always), &film).subtitle, None);
        assert!(valid_language("eng") && valid_language("fr") && !valid_language("English") && !valid_language("EN"));
        // Both ISO 639-2 forms and the two-letter code are one language.
        assert!(same_language(Some("ces"), Some("cze")) && same_language(Some("el"), Some("gre")));
    }
}
