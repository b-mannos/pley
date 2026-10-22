//! External subtitle files next to videos: `Movie.srt`, `Movie.en.srt`,
//! `Movie.eng.forced.srt`, `Movie.English.sdh.srt`, `Subs/2_English.srt`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use pley_transcoder_api::{MediaStream, StreamKind, is_text_subtitle_codec};
use serde::{Deserialize, Serialize};

use super::walk::SUBTITLE_EXTENSIONS;

/// External subtitle streams get indexes from this value up.
pub const EXTERNAL_INDEX_BASE: i64 = 1000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExternalSub {
    pub index: i64,
    pub path: String,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    #[serde(default)]
    pub forced: bool,
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub sdh: bool,
}

impl ExternalSub {
    pub fn to_stream(&self) -> MediaStream {
        MediaStream {
            index: self.index,
            kind: StreamKind::Subtitle,
            codec: self.codec.clone(),
            language: self.language.clone(),
            title: self.title.clone(),
            default: self.default,
            forced: self.forced,
            text_based: Some(is_text_subtitle_codec(&self.codec)),
            external: true,
            ..Default::default()
        }
    }
}

fn codec_for(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "srt" => "subrip",
        "ass" => "ass",
        "ssa" => "ssa",
        "vtt" => "webvtt",
        "sub" => "microdvd",
        "sup" => "hdmv_pgs_subtitle",
        "smi" => "sami",
        _ => return None,
    })
}

/// Subtitle sidecars belonging to `video`.
pub fn external_subtitles(video: &Path, sidecars: &HashMap<PathBuf, Vec<String>>) -> Vec<ExternalSub> {
    let Some(dir) = video.parent() else { return Vec::new() };
    let stem = video.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let mut found: Vec<(PathBuf, String)> = Vec::new(); // (path, tag part)

    if let Some(files) = sidecars.get(dir) {
        for name in files {
            let lower = name.to_lowercase();
            let Some((base, ext)) = lower.rsplit_once('.') else { continue };
            if !SUBTITLE_EXTENSIONS.contains(&ext) {
                continue;
            }
            if base == stem {
                found.push((dir.join(name), String::new()));
            } else if let Some(tags) = base.strip_prefix(&stem).and_then(|r| r.strip_prefix('.')) {
                found.push((dir.join(name), tags.to_owned()));
            }
        }
    }
    // "Subs" folders (common for single-movie folders).
    for sub_dir_name in ["subs", "subtitles"] {
        for (sub_dir, files) in sidecars {
            let Some(parent) = sub_dir.parent() else { continue };
            let name = sub_dir.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
            let matches_dir = (parent == dir && name == sub_dir_name)
                || (parent.parent() == Some(dir)
                    && parent.file_name().is_some_and(|p| p.to_string_lossy().to_lowercase() == sub_dir_name)
                    && name == stem);
            if !matches_dir {
                continue;
            }
            for file in files {
                let lower = file.to_lowercase();
                let Some((base, ext)) = lower.rsplit_once('.') else { continue };
                if SUBTITLE_EXTENSIONS.contains(&ext) {
                    // "2_English" → "english"
                    let tags =
                        base.trim_start_matches(|c: char| c.is_ascii_digit() || c == '_' || c == ' ').replace('_', ".");
                    found.push((sub_dir.join(file), tags));
                }
            }
        }
    }

    found.sort_by(|a, b| a.0.cmp(&b.0));
    found.dedup_by(|a, b| a.0 == b.0);
    found
        .into_iter()
        .enumerate()
        .filter_map(|(i, (path, tags))| {
            let ext = path.extension()?.to_string_lossy().to_lowercase();
            let codec = codec_for(&ext)?;
            let mut language = None;
            let mut forced = false;
            let mut default = false;
            let mut sdh = false;
            let mut extra = Vec::new();
            for token in tags.split(['.', ' ', '-']).filter(|t| !t.is_empty()) {
                match token {
                    "forced" | "foreign" => forced = true,
                    "default" => default = true,
                    "sdh" | "cc" | "hi" => sdh = true,
                    t => match normalize_language(t) {
                        Some(lang) if language.is_none() => language = Some(lang),
                        _ => extra.push(t.to_owned()),
                    },
                }
            }
            let title = (!extra.is_empty()).then(|| extra.join(" "));
            Some(ExternalSub {
                index: EXTERNAL_INDEX_BASE + i as i64,
                path: path.to_string_lossy().into_owned(),
                codec: codec.to_owned(),
                language,
                title,
                forced,
                default,
                sdh,
            })
        })
        .collect()
}

/// ISO 639-1/639-2 codes and English names → ISO 639-2/B.
pub fn normalize_language(token: &str) -> Option<String> {
    const TABLE: &[(&str, &[&str])] = &[
        ("eng", &["en", "eng", "english"]),
        ("fre", &["fr", "fre", "fra", "french", "francais", "français"]),
        ("ger", &["de", "ger", "deu", "german", "deutsch"]),
        ("spa", &["es", "spa", "spanish", "espanol", "español", "castellano"]),
        ("ita", &["it", "ita", "italian", "italiano"]),
        ("por", &["pt", "por", "portuguese", "pt-br", "ptbr", "brazilian"]),
        ("dut", &["nl", "dut", "nld", "dutch"]),
        ("swe", &["sv", "swe", "swedish"]),
        ("nor", &["no", "nor", "nob", "norwegian"]),
        ("dan", &["da", "dan", "danish"]),
        ("fin", &["fi", "fin", "finnish"]),
        ("pol", &["pl", "pol", "polish"]),
        ("cze", &["cs", "cze", "ces", "czech"]),
        ("hun", &["hu", "hun", "hungarian"]),
        ("rum", &["ro", "rum", "ron", "romanian"]),
        ("gre", &["el", "gre", "ell", "greek"]),
        ("tur", &["tr", "tur", "turkish"]),
        ("rus", &["ru", "rus", "russian"]),
        ("ukr", &["uk", "ukr", "ukrainian"]),
        ("ara", &["ar", "ara", "arabic"]),
        ("heb", &["he", "heb", "hebrew"]),
        ("hin", &["hi-in", "hin", "hindi"]),
        ("jpn", &["ja", "jpn", "japanese"]),
        ("kor", &["ko", "kor", "korean"]),
        ("chi", &["zh", "chi", "zho", "chinese", "chs", "cht"]),
        ("tha", &["th", "tha", "thai"]),
        ("vie", &["vi", "vie", "vietnamese"]),
        ("ind", &["id", "ind", "indonesian"]),
        ("may", &["ms", "may", "msa", "malay"]),
        ("bul", &["bg", "bul", "bulgarian"]),
        ("hrv", &["hr", "hrv", "croatian"]),
        ("srp", &["sr", "srp", "serbian"]),
        ("slv", &["sl", "slv", "slovenian"]),
        ("slo", &["sk", "slo", "slk", "slovak"]),
        ("est", &["et", "est", "estonian"]),
        ("lav", &["lv", "lav", "latvian"]),
        ("lit", &["lt", "lit", "lithuanian"]),
        ("ice", &["is", "ice", "isl", "icelandic"]),
        ("per", &["fa", "per", "fas", "persian", "farsi"]),
        ("cat", &["ca", "cat", "catalan"]),
    ];
    let t = token.trim().to_lowercase();
    TABLE.iter().find(|(_, names)| names.contains(&t.as_str())).map(|(code, _)| (*code).to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_sidecar_subtitles() {
        let dir = PathBuf::from("/m/Movie (2020)");
        let mut sidecars = HashMap::new();
        sidecars.insert(
            dir.clone(),
            vec![
                "Movie (2020).srt".to_owned(),
                "Movie (2020).en.forced.srt".to_owned(),
                "Movie (2020).French.sdh.ass".to_owned(),
                "Other Movie.srt".to_owned(),
                "poster.jpg".to_owned(),
            ],
        );
        sidecars.insert(dir.join("Subs"), vec!["2_English.srt".to_owned()]);
        let subs = external_subtitles(&dir.join("Movie (2020).mkv"), &sidecars);
        assert_eq!(subs.len(), 4);
        let forced = subs.iter().find(|s| s.forced).unwrap();
        assert_eq!(forced.language.as_deref(), Some("eng"));
        let french = subs.iter().find(|s| s.codec == "ass").unwrap();
        assert_eq!(french.language.as_deref(), Some("fre"));
        assert!(french.sdh);
        let from_subs_dir = subs.iter().find(|s| s.path.contains("Subs")).unwrap();
        assert_eq!(from_subs_dir.language.as_deref(), Some("eng"));
        assert!(subs.iter().all(|s| s.index >= EXTERNAL_INDEX_BASE));
        let stream = subs[0].to_stream();
        assert!(stream.external);
        assert_eq!(stream.text_based, Some(true));
    }

    #[test]
    fn languages() {
        assert_eq!(normalize_language("EN").as_deref(), Some("eng"));
        assert_eq!(normalize_language("deu").as_deref(), Some("ger"));
        assert_eq!(normalize_language("Japanese").as_deref(), Some("jpn"));
        assert_eq!(normalize_language("xx"), None);
    }
}
