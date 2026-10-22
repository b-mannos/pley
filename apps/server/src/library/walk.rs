//! Directory walking: finds media files plus the sidecar files (artwork,
//! subtitles) next to them, skipping system folders, extras and samples.

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
/// Bonus-material folders (not imported as library items yet).
const EXTRAS_DIRS: &[&str] = &[
    "extras",
    "featurettes",
    "behind the scenes",
    "deleted scenes",
    "interviews",
    "scenes",
    "shorts",
    "trailers",
    "other",
    "sample",
    "samples",
    "bonus",
    "specials features",
];

#[derive(Debug, Clone)]
pub struct FoundFile {
    pub path: PathBuf,
    pub root: PathBuf,
    pub size: i64,
    pub mtime: i64,
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

/// Release samples and trailers: `sample.mkv`, `sample-movie.mkv`,
/// `movie-sample.mkv`, `movie.trailer.mkv` (small files only for samples).
fn is_sample(path: &Path, size: u64) -> bool {
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if ["-trailer", ".trailer", "_trailer"].iter().any(|s| stem.ends_with(s)) {
        return true;
    }
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
                if kind != LibraryKind::Music && EXTRAS_DIRS.contains(&name.as_str()) {
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
        assert!(is_sample(Path::new("/m/x/Movie-trailer.mp4"), 4_000_000_000));
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
        assert_eq!(walked.media.len(), 1);
        assert!(walked.media[0].path.ends_with("Movie (2020).mkv"));
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
}
