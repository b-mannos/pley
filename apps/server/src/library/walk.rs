//! Directory walking: finds media files plus the sidecar files (artwork,
//! subtitles) next to them, skipping system folders and samples. Bonus
//! material (trailers, featurettes, …) is found with Plex's conventions:
//! folders such as `Trailers` or `Behind The Scenes` next to the movie or in
//! the show folder, or a suffix such as `Movie (2020)-trailer.mkv`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use super::LibraryKind;
use crate::util::system_time_secs;

const VIDEO_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "m4v", "avi", "mov", "wmv", "ts", "m2ts", "mts", "webm", "flv", "mpg", "mpeg", "ogv", "3gp", "divx",
    "asf",
];
const AUDIO_EXTENSIONS: &[&str] =
    &["mp3", "flac", "m4a", "aac", "ogg", "oga", "opus", "wav", "wma", "aiff", "aif", "ape", "wv", "mka", "m4b", "dsf"];
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "tbn"];
pub const SUBTITLE_EXTENSIONS: &[&str] = &["srt", "ass", "ssa", "vtt", "sub", "sup", "smi"];

const SKIP_DIRS: &[&str] = &[
    "@eadir",
    "#recycle",
    "$recycle.bin",
    "system volume information",
    "lost+found",
    ".trickplay",
    "#snapshot",
    ".grab",
];
/// Release samples: never imported.
const SAMPLE_DIRS: &[&str] = &["sample", "samples"];

/// Kinds of bonus material (Plex's extra types).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtraKind {
    Trailer,
    BehindTheScenes,
    DeletedScene,
    Featurette,
    Interview,
    Scene,
    Short,
    Other,
}

impl ExtraKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ExtraKind::Trailer => "trailer",
            ExtraKind::BehindTheScenes => "behind_the_scenes",
            ExtraKind::DeletedScene => "deleted_scene",
            ExtraKind::Featurette => "featurette",
            ExtraKind::Interview => "interview",
            ExtraKind::Scene => "scene",
            ExtraKind::Short => "short",
            ExtraKind::Other => "other",
        }
    }

    /// A bonus-material folder name (lowercase).
    fn from_dir(name: &str) -> Option<Self> {
        Some(match name {
            "trailers" => ExtraKind::Trailer,
            "behind the scenes" => ExtraKind::BehindTheScenes,
            "deleted scenes" => ExtraKind::DeletedScene,
            "featurettes" => ExtraKind::Featurette,
            "interviews" => ExtraKind::Interview,
            "scenes" => ExtraKind::Scene,
            "shorts" => ExtraKind::Short,
            "extras" | "other" | "bonus" | "special features" | "specials features" => ExtraKind::Other,
            _ => return None,
        })
    }

    /// A file named with a bonus-material suffix (`…-trailer`), and its stem
    /// without the suffix. Only `trailer` is matched in any case and after
    /// `.` or `_` too; the others must be Plex's lowercase `-suffix`, so that
    /// release groups (`…x264-SCENE`) are not mistaken for extras.
    fn from_stem(stem: &str) -> Option<(Self, &str)> {
        const SUFFIXES: &[(&str, ExtraKind)] = &[
            ("-behindthescenes", ExtraKind::BehindTheScenes),
            ("-deleted", ExtraKind::DeletedScene),
            ("-featurette", ExtraKind::Featurette),
            ("-interview", ExtraKind::Interview),
            ("-scene", ExtraKind::Scene),
            ("-short", ExtraKind::Short),
            ("-other", ExtraKind::Other),
        ];
        let lower = stem.to_ascii_lowercase();
        for tail in ["-trailer", ".trailer", "_trailer"] {
            if lower.ends_with(tail) {
                return Some((ExtraKind::Trailer, &stem[..stem.len() - tail.len()]));
            }
        }
        SUFFIXES
            .iter()
            .find(|(tail, _)| stem.ends_with(tail))
            .map(|(tail, kind)| (*kind, &stem[..stem.len() - tail.len()]))
    }
}

/// Bonus material and what it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extra {
    pub kind: ExtraKind,
    /// The movie's or show's folder (folder extras), or the file's own folder.
    pub owner_dir: PathBuf,
    /// For `Movie (2020)-trailer.mkv`: `Movie (2020)`, the owner's file stem.
    pub owner_stem: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FoundFile {
    pub path: PathBuf,
    pub root: PathBuf,
    pub size: i64,
    pub mtime: i64,
    /// Set for bonus material (trailers, featurettes, …).
    pub extra: Option<Extra>,
}

/// Whether `path` is bonus material: inside a bonus folder below `root`, or
/// named with a bonus suffix.
pub fn extra_of(path: &Path, root: &Path) -> Option<Extra> {
    let relative = path.strip_prefix(root).ok()?;
    let mut dir = root.to_path_buf();
    let components: Vec<_> = relative.components().collect();
    // Folders between the root and the file, nearest last.
    let mut found = None;
    for component in components.iter().take(components.len().saturating_sub(1)) {
        let name = component.as_os_str().to_string_lossy().to_ascii_lowercase();
        if let Some(kind) = ExtraKind::from_dir(&name) {
            found = Some(Extra { kind, owner_dir: dir.clone(), owner_stem: None });
            break;
        }
        dir.push(component);
    }
    if found.is_some() {
        return found;
    }
    let stem = path.file_stem()?.to_string_lossy();
    let (kind, owner) = ExtraKind::from_stem(&stem)?;
    Some(Extra { kind, owner_dir: path.parent()?.to_path_buf(), owner_stem: Some(owner.to_owned()) })
}

#[derive(Debug, Default)]
pub struct Walked {
    pub media: Vec<FoundFile>,
    /// Sidecar file names (images, subtitles) per directory.
    pub sidecars: HashMap<PathBuf, Vec<String>>,
    /// Library roots that could not be read (unmounted drive?): files below
    /// them must not be treated as deleted.
    pub offline_roots: Vec<PathBuf>,
    /// Folders, links and files inside the roots that failed to read (an
    /// offline network share behind a symlink, a permission problem, an I/O
    /// error): files below them must not be treated as deleted either.
    pub unreadable: Vec<PathBuf>,
}

impl Walked {
    /// Whether a known file at `path` may be missing only because it could
    /// not be read this time.
    pub fn maybe_unavailable(&self, path: &Path) -> bool {
        self.offline_roots.iter().chain(&self.unreadable).any(|p| path.starts_with(p))
    }
}

fn extension(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

pub fn is_media_file(path: &Path, kind: LibraryKind) -> bool {
    let ext = extension(path);
    match kind {
        LibraryKind::Music => AUDIO_EXTENSIONS.contains(&ext.as_str()),
        _ => VIDEO_EXTENSIONS.contains(&ext.as_str()),
    }
}

/// Release samples: `sample.mkv`, `sample-movie.mkv`, `movie-sample.mkv`
/// (small files only).
fn is_sample(path: &Path, size: u64) -> bool {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let sample_name = stem == "sample"
        || ["sample-", "sample.", "sample_"].iter().any(|p| stem.starts_with(p))
        || ["-sample", ".sample", "_sample"].iter().any(|s| stem.ends_with(s));
    sample_name && size < 500 * 1024 * 1024
}

pub fn walk(roots: &[PathBuf], kind: LibraryKind) -> Walked {
    let mut out = Walked::default();
    for root in roots {
        if std::fs::read_dir(root).is_err() {
            tracing::warn!("library folder {} is not accessible; skipping", root.display());
            out.offline_roots.push(root.clone());
            continue;
        }
        let walker = WalkDir::new(root).follow_links(true).into_iter().filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if name.starts_with('.') {
                return false;
            }
            if entry.file_type().is_dir() {
                if SKIP_DIRS.contains(&name.as_str()) {
                    return false;
                }
                if kind != LibraryKind::Music && SAMPLE_DIRS.contains(&name.as_str()) {
                    return false;
                }
            }
            true
        });
        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    tracing::warn!("cannot read {}: {err}", err.path().unwrap_or(root).display());
                    out.unreadable.push(err.path().unwrap_or(root).to_path_buf());
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let ext = extension(path);
            if IMAGE_EXTENSIONS.contains(&ext.as_str()) || SUBTITLE_EXTENSIONS.contains(&ext.as_str()) {
                if let Some(dir) = path.parent() {
                    out.sidecars
                        .entry(dir.to_path_buf())
                        .or_default()
                        .push(entry.file_name().to_string_lossy().into_owned());
                }
                continue;
            }
            if !is_media_file(path, kind) {
                continue;
            }
            let Ok(meta) = entry.metadata() else {
                out.unreadable.push(path.to_path_buf());
                continue;
            };
            if kind != LibraryKind::Music && is_sample(path, meta.len()) {
                continue;
            }
            out.media.push(FoundFile {
                path: path.to_path_buf(),
                root: root.clone(),
                size: meta.len() as i64,
                mtime: meta.modified().map(system_time_secs).unwrap_or(0),
                extra: if kind == LibraryKind::Music { None } else { extra_of(path, root) },
            });
        }
    }
    out
}

/// Find an image named `<base>.<image ext>` (case-insensitive) in `files`.
pub fn find_image(files: &[String], bases: &[String]) -> Option<String> {
    for base in bases {
        for name in files {
            let lower = name.to_ascii_lowercase();
            if let Some((stem, ext)) = lower.rsplit_once('.') {
                if IMAGE_EXTENSIONS.contains(&ext) && stem == base.to_ascii_lowercase() {
                    return Some(name.clone());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_images_case_insensitively() {
        let files = vec!["Poster.JPG".to_owned(), "fanart.png".to_owned(), "movie.srt".to_owned()];
        assert_eq!(find_image(&files, &["poster".into()]), Some("Poster.JPG".into()));
        assert_eq!(find_image(&files, &["folder".into(), "fanart".into()]), Some("fanart.png".into()));
        assert_eq!(find_image(&files, &["movie".into()]), None);
    }

    #[test]
    fn samples_and_trailers() {
        let small = 10 * 1024 * 1024;
        assert!(is_sample(Path::new("/m/x/sample.mkv"), small));
        assert!(is_sample(Path::new("/m/x/movie.2019.1080p-sample.mkv"), small));
        assert!(is_sample(Path::new("/m/x/sample-movie.mkv"), small));
        assert!(!is_sample(Path::new("/m/x/Movie-trailer.mp4"), 4_000_000_000), "trailers are extras now");
        assert!(!is_sample(Path::new("/m/x/HDR Sample (2023).mkv"), small));
        assert!(!is_sample(Path::new("/m/x/Free Samples (2012).mkv"), small));
        assert!(!is_sample(Path::new("/m/x/movie-sample.mkv"), 2_000_000_000));
    }

    #[test]
    fn walks_a_tree() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("Movie (2020)/Extras")).unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        std::fs::write(root.join("Movie (2020)/Movie (2020).mkv"), b"x").unwrap();
        std::fs::write(root.join("Movie (2020)/poster.jpg"), b"x").unwrap();
        std::fs::write(root.join("Movie (2020)/Movie (2020).en.srt"), b"x").unwrap();
        std::fs::write(root.join("Movie (2020)/Extras/Behind.mkv"), b"x").unwrap();
        std::fs::write(root.join(".hidden/secret.mkv"), b"x").unwrap();
        std::fs::write(root.join("Movie (2020)/Movie-sample.mkv"), b"x").unwrap();
        std::fs::write(root.join("notes.txt"), b"x").unwrap();

        let walked = walk(&[root.to_path_buf(), root.join("missing")], LibraryKind::Movies);
        let mut media: Vec<_> = walked.media.iter().collect();
        media.sort_by_key(|f| f.extra.is_some());
        assert_eq!(media.len(), 2);
        assert!(media[0].path.ends_with("Movie (2020).mkv"));
        assert!(media[0].extra.is_none());
        let extra = media[1].extra.as_ref().unwrap();
        assert_eq!((extra.kind, extra.owner_dir.clone()), (ExtraKind::Other, root.join("Movie (2020)")));
        let sidecars = &walked.sidecars[&root.join("Movie (2020)")];
        assert_eq!(sidecars.len(), 2);
        assert_eq!(walked.offline_roots, vec![root.join("missing")]);
        assert!(walked.maybe_unavailable(&root.join("missing/Film (1999)/Film.mkv")));
        assert!(!walked.maybe_unavailable(&root.join("Gone (2001)/Gone.mkv")));
    }

    #[test]
    fn unreadable_folders_protect_their_files() {
        let walked = Walked { unreadable: vec![PathBuf::from("/m/nas-link")], ..Default::default() };
        assert!(walked.maybe_unavailable(Path::new("/m/nas-link/Movie (2020)/Movie.mkv")));
        assert!(walked.maybe_unavailable(Path::new("/m/nas-link")));
        assert!(!walked.maybe_unavailable(Path::new("/m/other/Movie.mkv")));
    }

    #[test]
    fn finds_extras() {
        let root = Path::new("/m");
        let e = extra_of(Path::new("/m/Movie (2020)/Trailers/Teaser.mkv"), root).unwrap();
        assert_eq!((e.kind, e.owner_dir, e.owner_stem), (ExtraKind::Trailer, PathBuf::from("/m/Movie (2020)"), None));
        let e = extra_of(Path::new("/m/Movie (2020)/Movie (2020)-behindthescenes.mkv"), root).unwrap();
        assert_eq!((e.kind, e.owner_stem.as_deref()), (ExtraKind::BehindTheScenes, Some("Movie (2020)")));
        let e = extra_of(Path::new("/m/Movie (2020)/Movie (2020).Trailer.mp4"), root).unwrap();
        assert_eq!(e.kind, ExtraKind::Trailer);
        let e = extra_of(Path::new("/m/Show (2010)/Featurettes/Making Of.mkv"), root).unwrap();
        assert_eq!((e.kind, e.owner_dir), (ExtraKind::Featurette, PathBuf::from("/m/Show (2010)")));
        assert_eq!(extra_of(Path::new("/m/Show (2010)/Season 1/Show - S01E01.mkv"), root), None);
        assert_eq!(extra_of(Path::new("/m/Movie.2019.1080p.x264-SCENE.mkv"), root), None, "a release group");
        assert_eq!(extra_of(Path::new("/m/Crime Scene (2001)/Crime Scene (2001).mkv"), root), None);
    }
}
