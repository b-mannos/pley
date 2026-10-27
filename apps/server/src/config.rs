//! Static configuration: command line > environment > config file > defaults.
//!
//! Settings that admins change at runtime (TMDB key, hardware acceleration, …)
//! live in the database instead, see [`crate::settings`].

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};
use serde::Deserialize;

/// Default HTTP port ("PLEY" on a phone keypad).
pub const DEFAULT_PORT: u16 = 7539;
/// Default port of the Plex-compatible API (Plex Media Server's).
pub const PLEX_PORT: u16 = 32400;
/// How long requests in progress may take to finish once the server is told
/// to stop. Streams (a film playing directly) are cut after that. Short
/// enough to leave time for stopping the transcoder and closing the database
/// before Docker's default stop timeout (10 s) kills the process.
pub const SHUTDOWN_GRACE: Duration = Duration::from_secs(5);
/// How often an open event stream checks that its token still works.
pub const EVENT_RECHECK: Duration = Duration::from_secs(30);

#[derive(Debug, Parser)]
#[command(name = "pley", version = crate::VERSION_AND_COMMIT, about = "pley — a fast, self-hosted media server")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Path to a TOML config file (default: <data-dir>/pley.toml when it exists)
    #[arg(long, short, env = "PLEY_CONFIG", global = true)]
    pub config: Option<PathBuf>,

    /// Directory holding the database, caches, artwork and transcodes
    #[arg(long, env = "PLEY_DATA_DIR", global = true)]
    pub data_dir: Option<PathBuf>,

    /// Address to listen on, e.g. `0.0.0.0:7539` or just a port
    #[arg(long, env = "PLEY_BIND", global = true)]
    pub bind: Option<String>,

    /// Address of the Plex-compatible API (default `0.0.0.0:32400`, `off` to disable)
    #[arg(long, env = "PLEY_PLEX_BIND", global = true)]
    pub plex_bind: Option<String>,

    /// Path to the ffmpeg binary
    #[arg(long, env = "PLEY_FFMPEG", global = true)]
    pub ffmpeg: Option<PathBuf>,

    /// Path to the ffprobe binary
    #[arg(long, env = "PLEY_FFPROBE", global = true)]
    pub ffprobe: Option<PathBuf>,

    /// Serve the web UI from this directory instead of the copy embedded in the binary
    #[arg(long, env = "PLEY_WEB_DIR", global = true)]
    pub web_dir: Option<PathBuf>,

    /// pley-transcoder executable to spawn (default: next to the pley binary)
    #[arg(long, env = "PLEY_TRANSCODER_PATH", global = true)]
    pub transcoder_path: Option<PathBuf>,

    /// Use an already running transcoder at this URL instead of spawning one
    #[arg(long, env = "PLEY_TRANSCODER_URL", global = true)]
    pub transcoder_url: Option<String>,

    /// Shared secret for an external transcoder (`--transcoder-url`)
    #[arg(long, env = "PLEY_TRANSCODER_TOKEN", global = true, hide_env_values = true)]
    pub transcoder_token: Option<String>,

    /// Log filter, e.g. `info` or `debug,sqlx=warn`
    #[arg(long, env = "PLEY_LOG", default_value = "info", global = true)]
    pub log: String,

    /// Never show the tray icon (it only appears in interactive desktop sessions anyway)
    #[arg(long, env = "PLEY_NO_TRAY", global = true)]
    pub no_tray: bool,

    /// Don't answer the pley apps looking for servers on the local network
    #[arg(long, env = "PLEY_NO_DISCOVERY", global = true)]
    pub no_discovery: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the media server (default)
    Serve,
    /// Set a user's password, creating the user if it does not exist
    ResetPassword {
        username: String,
        password: String,
        /// Grant administrator rights
        #[arg(long)]
        admin: bool,
    },
    /// Exit with status 0 when the server on this machine answers (for container health checks)
    Health,
    /// Save the options given before it (--bind, --plex-bind, --ffmpeg, --ffprobe,
    /// --no-discovery) in pley.toml in the data folder, for every later start
    SaveConfig {
        /// Whether to answer the pley apps looking for servers on the local network
        #[arg(long, value_name = "true|false")]
        discovery: Option<bool>,
    },
    /// Run pley as a Windows service: install, start, stop or remove it
    #[cfg(windows)]
    #[command(subcommand)]
    Service(crate::service::ServiceCommand),
}

/// Shape of `pley.toml`. Every key is optional.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct FileConfig {
    bind: Option<String>,
    plex_bind: Option<String>,
    data_dir: Option<PathBuf>,
    transcode_dir: Option<PathBuf>,
    ffmpeg: Option<PathBuf>,
    ffprobe: Option<PathBuf>,
    web_dir: Option<PathBuf>,
    transcoder_path: Option<PathBuf>,
    transcoder_url: Option<String>,
    transcoder_token: Option<String>,
    transcoder_max_jobs: Option<u32>,
    discovery: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bind: SocketAddr,
    /// Plex-compatible API listener (`None`: disabled).
    pub plex_bind: Option<SocketAddr>,
    pub data_dir: PathBuf,
    /// Work directory handed to the managed transcoder (segments, caches).
    pub transcode_dir: PathBuf,
    /// ffmpeg/ffprobe are only passed through to the managed transcoder.
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub web_dir: Option<PathBuf>,
    pub transcoder_path: Option<PathBuf>,
    pub transcoder_url: Option<String>,
    pub transcoder_token: Option<String>,
    pub transcoder_max_jobs: u32,
    /// Where TVmaze, TheTVDB, MusicBrainz and the Cover Art Archive are
    /// reached. `None` (tests) never contacts them.
    pub metadata_endpoints: Option<crate::providers::Endpoints>,
    /// See [`SHUTDOWN_GRACE`] (tests shorten it).
    pub shutdown_grace: Duration,
    /// Answer apps looking for servers on the local network (see [`crate::discovery`]).
    pub discovery: bool,
    /// See [`EVENT_RECHECK`] (tests shorten it).
    pub event_recheck: Duration,
    /// Where the client metadata documents of other publishers' apps come
    /// from: the internet, or nowhere in tests (which may also give fixed
    /// ones). pley's own apps are built in either way.
    pub client_documents: crate::oauth::fetch::Source,
}

impl Config {
    pub fn load(cli: &Cli) -> anyhow::Result<Self> {
        let default_data_dir = cli.data_dir.clone().unwrap_or_else(default_data_dir);
        let config_path = match &cli.config {
            Some(p) => Some(p.clone()),
            None => {
                let candidate = default_data_dir.join("pley.toml");
                candidate.exists().then_some(candidate)
            }
        };
        let file: FileConfig = match &config_path {
            Some(path) => {
                let text =
                    std::fs::read_to_string(path).with_context(|| format!("reading config file {}", path.display()))?;
                toml::from_str(&text).with_context(|| format!("parsing config file {}", path.display()))?
            }
            None => FileConfig::default(),
        };

        let data_dir = cli.data_dir.clone().or(file.data_dir).unwrap_or(default_data_dir);
        let data_dir = absolutize(&data_dir);
        let bind = cli.bind.clone().or(file.bind).map(|s| parse_bind(&s)).transpose()?;
        let plex_bind = match cli.plex_bind.clone().or(file.plex_bind) {
            None => Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), PLEX_PORT)),
            Some(value) if is_off(&value) => None,
            Some(value) => Some(parse_bind(&value)?),
        };
        let transcode_dir = file.transcode_dir.map(|p| absolutize(&p)).unwrap_or_else(|| data_dir.join("transcode"));

        Ok(Self {
            bind: bind.unwrap_or(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DEFAULT_PORT)),
            plex_bind,
            ffmpeg: cli.ffmpeg.clone().or(file.ffmpeg).unwrap_or_else(|| PathBuf::from("ffmpeg")),
            ffprobe: cli.ffprobe.clone().or(file.ffprobe).unwrap_or_else(|| PathBuf::from("ffprobe")),
            web_dir: cli.web_dir.clone().or(file.web_dir),
            transcoder_path: cli.transcoder_path.clone().or(file.transcoder_path),
            transcoder_url: cli.transcoder_url.clone().or(file.transcoder_url).filter(|u| !u.trim().is_empty()),
            transcoder_token: cli.transcoder_token.clone().or(file.transcoder_token),
            transcoder_max_jobs: file.transcoder_max_jobs.unwrap_or(8).max(1),
            metadata_endpoints: Some(crate::providers::Endpoints::default()),
            shutdown_grace: SHUTDOWN_GRACE,
            discovery: !cli.no_discovery && file.discovery.unwrap_or(true),
            event_recheck: EVENT_RECHECK,
            client_documents: crate::oauth::fetch::Source::Internet,
            data_dir,
            transcode_dir,
        })
    }

    /// Configuration rooted in `data_dir`, used by tests. Metadata providers
    /// that need no key are not contacted.
    pub fn for_data_dir(data_dir: &Path) -> Self {
        Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0),
            plex_bind: None,
            data_dir: data_dir.to_path_buf(),
            transcode_dir: data_dir.join("transcode"),
            ffmpeg: PathBuf::from("ffmpeg"),
            ffprobe: PathBuf::from("ffprobe"),
            web_dir: None,
            transcoder_path: None,
            transcoder_url: None,
            transcoder_token: None,
            transcoder_max_jobs: 8,
            metadata_endpoints: None,
            shutdown_grace: SHUTDOWN_GRACE,
            discovery: false,
            event_recheck: EVENT_RECHECK,
            client_documents: crate::oauth::fetch::Source::Off,
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("pley.db")
    }

    /// Server log, rotated by size (see [`crate::logging`]).
    pub fn log_file(&self) -> PathBuf {
        log_file(&self.data_dir)
    }

    /// The server log when the configuration can't be loaded: in the data
    /// directory of the command line, or the default one (where `pley.toml`
    /// is looked for).
    pub fn fallback_log_file(cli: &Cli) -> PathBuf {
        log_file(&absolutize(&cli.data_dir.clone().unwrap_or_else(default_data_dir)))
    }

    /// Resized images, extracted subtitles, …: safe to delete at any time.
    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join("cache")
    }

    /// Downloaded / generated artwork referenced from the database.
    pub fn metadata_dir(&self) -> PathBuf {
        self.data_dir.join("metadata")
    }

    /// Files generated from media files, by fingerprint (see [`crate::store`]).
    pub fn media_dir(&self) -> PathBuf {
        self.data_dir.join("media")
    }
}

impl Cli {
    /// The data directory of the command line, or the default one.
    pub fn data_dir_or_default(&self) -> PathBuf {
        absolutize(&self.data_dir.clone().unwrap_or_else(default_data_dir))
    }
}

/// `pley save-config`: the options given (on the command line or in the
/// environment) written into the config file `path`, whose other lines and
/// comments stay. `discovery` overrides `--no-discovery`.
pub fn save_config(cli: &Cli, path: &Path, discovery: Option<bool>) -> anyhow::Result<()> {
    let mut values: Vec<(&str, toml::Value)> = Vec::new();
    if let Some(bind) = &cli.bind {
        parse_bind(bind)?;
        values.push(("bind", bind.trim().into()));
    }
    if let Some(plex_bind) = &cli.plex_bind {
        if !is_off(plex_bind) {
            parse_bind(plex_bind)?;
        }
        values.push(("plex_bind", plex_bind.trim().into()));
    }
    for (key, program) in [("ffmpeg", &cli.ffmpeg), ("ffprobe", &cli.ffprobe)] {
        if let Some(program) = program {
            values.push((key, program.to_string_lossy().into_owned().into()));
        }
    }
    if let Some(discovery) = discovery.or(cli.no_discovery.then_some(false)) {
        values.push(("discovery", discovery.into()));
    }
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err).with_context(|| format!("reading config file {}", path.display())),
    };
    let text = set_keys(&text, &values);
    toml::from_str::<FileConfig>(&text).with_context(|| format!("checking the new {}", path.display()))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    }
    std::fs::write(path, text).with_context(|| format!("writing config file {}", path.display()))
}

/// `text` with each key of `values` set: on its line, or a new one at the
/// end. pley.toml has top-level keys only, so the end is never in a table.
fn set_keys(text: &str, values: &[(&str, toml::Value)]) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    for (key, value) in values {
        let line = format!("{key} = {value}");
        let mut found = false;
        // A second line for the key would make the file invalid.
        lines.retain_mut(|existing| {
            if !defines(existing, key) {
                return true;
            }
            if found {
                return false;
            }
            existing.clone_from(&line);
            found = true;
            true
        });
        if !found {
            lines.push(line);
        }
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Whether `line` gives `key` a value (`key = …`, not `# key = …` nor `key2 = …`).
fn defines(line: &str, key: &str) -> bool {
    line.trim_start().strip_prefix(key).is_some_and(|rest| rest.trim_start().starts_with('='))
}

/// A Plex-compatible API address that turns it off.
fn is_off(value: &str) -> bool {
    matches!(value.trim(), "" | "off" | "none" | "false" | "0")
}

fn default_data_dir() -> PathBuf {
    dirs::data_local_dir().map(|d| d.join("pley")).unwrap_or_else(|| PathBuf::from("pley-data"))
}

fn log_file(data_dir: &Path) -> PathBuf {
    data_dir.join("logs").join("pley.log")
}

fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().map(|cwd| cwd.join(path)).unwrap_or_else(|_| path.to_path_buf())
    }
}

/// Accepts `host:port`, `:port` or a bare `port`.
fn parse_bind(value: &str) -> anyhow::Result<SocketAddr> {
    let value = value.trim();
    if let Ok(port) = value.trim_start_matches(':').parse::<u16>() {
        return Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port));
    }
    value.parse().with_context(|| format!("invalid bind address '{value}'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_forms() {
        assert_eq!(parse_bind("8080").unwrap().port(), 8080);
        assert_eq!(parse_bind(":9000").unwrap().port(), 9000);
        assert_eq!(parse_bind("127.0.0.1:1234").unwrap(), "127.0.0.1:1234".parse().unwrap());
        assert!(parse_bind("nope").is_err());
        assert_eq!(
            parse_bind(" 8080 ").unwrap().ip(),
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            "a bare port listens everywhere"
        );
        assert_eq!(parse_bind("[::1]:7539").unwrap(), "[::1]:7539".parse().unwrap());
        assert!(parse_bind("70000").is_err());
    }

    /// Command line options as if none were given (built directly, so the
    /// `PLEY_*` environment of the test run cannot leak in).
    fn cli(data_dir: &Path) -> Cli {
        Cli {
            command: None,
            config: None,
            data_dir: Some(data_dir.to_path_buf()),
            bind: None,
            plex_bind: None,
            ffmpeg: None,
            ffprobe: None,
            web_dir: None,
            transcoder_path: None,
            transcoder_url: None,
            transcoder_token: None,
            log: "info".into(),
            no_tray: false,
            no_discovery: false,
        }
    }

    #[test]
    fn defaults_without_a_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(&cli(dir.path())).unwrap();
        assert_eq!(config.bind, SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), DEFAULT_PORT));
        assert_eq!(config.plex_bind, Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), PLEX_PORT)));
        assert_eq!(config.data_dir, dir.path());
        assert_eq!(config.transcode_dir, dir.path().join("transcode"));
        assert_eq!(config.db_path(), dir.path().join("pley.db"));
        assert_eq!((config.ffmpeg.as_path(), config.ffprobe.as_path()), (Path::new("ffmpeg"), Path::new("ffprobe")));
        assert_eq!(config.transcoder_max_jobs, 8);
        assert_eq!(config.transcoder_url, None);
    }

    #[test]
    fn the_config_file_in_the_data_dir_is_read_and_the_command_line_wins() {
        let dir = tempfile::tempdir().unwrap();
        let transcode = dir.path().join("scratch");
        std::fs::write(
            dir.path().join("pley.toml"),
            format!(
                "bind = \"127.0.0.1:8000\"\nplex_bind = \"off\"\nffmpeg = \"/opt/ff/ffmpeg\"\n\
                 transcode_dir = {:?}\ntranscoder_url = \"  \"\ntranscoder_max_jobs = 0\n",
                transcode.to_string_lossy()
            ),
        )
        .unwrap();
        let config = Config::load(&cli(dir.path())).unwrap();
        assert_eq!(config.bind, "127.0.0.1:8000".parse().unwrap());
        assert_eq!(config.plex_bind, None, "`off` disables the Plex-compatible API");
        assert_eq!(config.ffmpeg, PathBuf::from("/opt/ff/ffmpeg"));
        assert_eq!(config.transcode_dir, transcode);
        assert_eq!(config.transcoder_url, None, "a blank URL means: spawn the transcoder");
        assert_eq!(config.transcoder_max_jobs, 1, "at least one job");

        let mut args = cli(dir.path());
        args.bind = Some("9000".into());
        args.plex_bind = Some("127.0.0.1:32401".into());
        args.ffmpeg = Some("ffmpeg7".into());
        let config = Config::load(&args).unwrap();
        assert_eq!(config.bind, SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 9000));
        assert_eq!(config.plex_bind, Some("127.0.0.1:32401".parse().unwrap()));
        assert_eq!(config.ffmpeg, PathBuf::from("ffmpeg7"));
    }

    #[test]
    fn an_explicit_config_file_can_move_the_data_dir() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("elsewhere");
        let file = dir.path().join("custom.toml");
        std::fs::write(&file, format!("data_dir = {:?}\n", data.to_string_lossy())).unwrap();
        let mut args = cli(dir.path());
        args.data_dir = None;
        args.config = Some(file);
        let config = Config::load(&args).unwrap();
        assert_eq!(config.data_dir, data);
        assert_eq!(config.transcode_dir, data.join("transcode"));
    }

    #[test]
    fn set_keys_replaces_lines_keeps_the_rest_and_appends_new_keys() {
        let text = "# pley\nbind = \"0.0.0.0:7539\"\n# plex_bind = \"off\"\nbind_extra = 1\nbind=\"x\"\n";
        let out = set_keys(text, &[("bind", "127.0.0.1:7539".into()), ("plex_bind", "off".into())]);
        assert_eq!(
            out,
            "# pley\nbind = \"127.0.0.1:7539\"\n# plex_bind = \"off\"\nbind_extra = 1\nplex_bind = \"off\"\n"
        );
        assert_eq!(set_keys("", &[("discovery", false.into())]), "discovery = false\n");
    }

    #[test]
    fn saved_options_are_read_back_and_other_lines_stay() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("pley.toml");
        std::fs::write(&file, "# mine\ntranscoder_max_jobs = 3\nbind = \"9000\"\n").unwrap();
        let mut args = cli(dir.path());
        args.bind = Some("127.0.0.1:7540".into());
        args.plex_bind = Some("off".into());
        args.ffmpeg = Some(PathBuf::from(r"C:\Program Files\pley\ffmpeg\ffmpeg.exe"));
        save_config(&args, &file, Some(false)).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.starts_with("# mine\ntranscoder_max_jobs = 3\n"), "{text}");

        let config = Config::load(&cli(dir.path())).unwrap();
        assert_eq!(config.bind, "127.0.0.1:7540".parse().unwrap());
        assert_eq!(config.plex_bind, None);
        assert_eq!(config.ffmpeg, PathBuf::from(r"C:\Program Files\pley\ffmpeg\ffmpeg.exe"));
        assert!(!config.discovery);
        assert_eq!(config.transcoder_max_jobs, 3);

        args = cli(dir.path());
        args.bind = Some("nope".into());
        assert!(save_config(&args, &file, None).is_err(), "an unreadable address isn't saved");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), text);
    }

    #[test]
    fn bad_config_files_are_reported() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("pley.toml"), "bindd = \"8080\"\n").unwrap();
        let err = Config::load(&cli(dir.path())).unwrap_err();
        assert!(format!("{err:#}").contains("pley.toml"), "{err:#}");

        std::fs::write(dir.path().join("pley.toml"), "plex_bind = \"somewhere\"\n").unwrap();
        assert!(Config::load(&cli(dir.path())).is_err(), "an unreadable address is an error, not a silent default");

        let mut args = cli(dir.path());
        args.config = Some(dir.path().join("missing.toml"));
        assert!(Config::load(&args).is_err());
    }
}
