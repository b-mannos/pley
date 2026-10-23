//! Desktop integration: a tray icon while pley runs in an interactive desktop
//! session (Windows notification area, macOS menu bar, Linux
//! StatusNotifierItem), and starting pley at login.
//!
//! The tray is a small remote control for the server: it opens the web UI,
//! starts library scans, shows the logs, toggles start-at-login and quits. It
//! never appears for services, containers or SSH sessions, and `--no-tray`
//! turns it off.

pub mod autostart;
#[cfg(all(feature = "tray", target_os = "linux"))]
mod linux;
#[cfg(all(feature = "tray", target_os = "macos"))]
pub mod macos;
#[cfg(all(feature = "tray", windows))]
mod windows;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use tokio::sync::mpsc;

use crate::state::AppState;

/// Starts a tray icon for a running server; `None` when there is none.
pub type TrayLauncher = Box<dyn FnOnce(TrayContext) -> Option<TrayGuard> + Send>;

/// A running tray icon. Dropping it removes the icon.
pub struct TrayGuard(#[allow(dead_code)] Box<dyn Send>);

/// What the tray asks the server to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    ScanLibraries,
    Quit,
}

/// Menu item ids of the `tray-icon` based implementations.
#[cfg(all(feature = "tray", any(windows, target_os = "macos")))]
mod item {
    pub const OPEN: &str = "open";
    pub const SCAN: &str = "scan";
    pub const LOGS: &str = "logs";
    pub const AUTOSTART: &str = "autostart";
    pub const QUIT: &str = "quit";
}

/// Everything a tray implementation needs. Cheap to clone, usable from any thread.
#[derive(Clone)]
pub struct TrayContext {
    /// Address of the web UI, e.g. `http://localhost:7539`.
    pub url: String,
    pub logs_dir: PathBuf,
    commands: mpsc::UnboundedSender<Command>,
    /// One line about what the server is doing, refreshed every few seconds.
    status: Arc<RwLock<String>>,
    /// For the setup code, while the server has no account.
    state: AppState,
}

impl TrayContext {
    /// Wire tray commands to the server listening on `addr` and keep the
    /// status line fresh. Must be called inside the server's runtime.
    pub fn new(state: &AppState, addr: SocketAddr) -> Self {
        let (commands, mut rx) = mpsc::unbounded_channel();
        let worker = state.clone();
        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                match command {
                    Command::ScanLibraries => {
                        if let Err(err) = worker.scanner.enqueue_all(false).await {
                            tracing::warn!("could not start a scan: {err}");
                        }
                    }
                    // Stops this server (see `crate::run_with_tray`).
                    Command::Quit => worker.stopping.cancel(),
                }
            }
        });
        let status = Arc::new(RwLock::new(status_line(state)));
        let (refresh, monitored) = (status.clone(), state.clone());
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(3));
            loop {
                tick.tick().await;
                let line = status_line(&monitored);
                if *refresh.read() != line {
                    *refresh.write() = line;
                }
            }
        });
        Self {
            url: web_url(addr),
            logs_dir: state.config.log_file().parent().map(PathBuf::from).unwrap_or_default(),
            commands,
            status,
            state: state.clone(),
        }
    }

    pub fn status(&self) -> String {
        self.status.read().clone()
    }

    /// Tooltip / hover text.
    pub fn tooltip(&self) -> String {
        format!("pley: {}", self.status())
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    /// Opens the web app in the browser: its setup page with the setup
    /// code filled in, while the server has no account.
    pub(crate) fn open_web_ui(&self) {
        match self.state.auth.setup.shown() {
            Some(code) => open(format!("{}/setup#code={code}", self.url)),
            None => open(&self.url),
        }
    }

    #[cfg_attr(not(feature = "tray"), allow(dead_code))]
    fn open_logs(&self) {
        open(&self.logs_dir);
    }

    /// Flip start-at-login; returns the new state, or the old one on failure.
    #[cfg_attr(not(feature = "tray"), allow(dead_code))]
    fn toggle_autostart(&self, enable: bool) -> bool {
        match autostart::set_enabled(enable) {
            Ok(()) => enable,
            Err(err) => {
                tracing::warn!("could not change start at login: {err:#}");
                !enable
            }
        }
    }
}

/// Start the tray for this platform (Windows, Linux). macOS runs its menu bar
/// item on the main thread instead, see [`macos::serve`].
pub fn start_tray(ctx: TrayContext) -> Option<TrayGuard> {
    #[cfg(all(feature = "tray", windows))]
    return windows::start(ctx);
    #[cfg(all(feature = "tray", target_os = "linux"))]
    return linux::start(ctx);
    #[allow(unreachable_code)]
    {
        let _ = ctx;
        None
    }
}

/// Whether this process runs in a desktop session where a tray icon makes
/// sense: not a Windows service, not a container or SSH session.
pub fn interactive_session() -> bool {
    if !cfg!(feature = "tray") {
        return false;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
        use windows_sys::Win32::System::Threading::GetCurrentProcessId;
        let mut session = 0u32;
        // Services run in session 0, users in 1 and up.
        // SAFETY: plain query with a valid out pointer.
        let ok = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session) } != 0;
        ok && session != 0
    }
    #[cfg(target_os = "macos")]
    {
        let var = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
        !var("SSH_CONNECTION") && !var("SSH_TTY") && std::env::var("USER").is_ok_and(|u| u != "root")
    }
    #[cfg(target_os = "linux")]
    {
        let var = |name: &str| std::env::var_os(name).is_some_and(|v| !v.is_empty());
        (var("DISPLAY") || var("WAYLAND_DISPLAY")) && var("DBUS_SESSION_BUS_ADDRESS")
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    false
}

/// "2 streams (1 transcoding), scanning Movies" or "idle".
fn status_line(state: &AppState) -> String {
    if state.stopping.is_cancelled() {
        return "shutting down".to_owned();
    }
    let playing = state.sessions.active().len();
    let transcoding = state.sessions.active_transcodes();
    let mut parts = Vec::new();
    match playing {
        0 => {}
        1 => parts.push("1 stream".to_owned()),
        n => parts.push(format!("{n} streams")),
    }
    if transcoding > 0 {
        let streams = parts.pop().unwrap_or_default();
        parts.push(format!("{streams} ({transcoding} transcoding)").trim().to_owned());
    }
    if let Some(scan) = state.scanner.statuses().first() {
        let name = if scan.library_name.is_empty() { "libraries" } else { scan.library_name.as_str() };
        parts.push(format!("scanning {name}"));
    }
    if parts.is_empty() { "idle".to_owned() } else { parts.join(", ") }
}

fn web_url(bind: SocketAddr) -> String {
    let ip = bind.ip();
    let host = if ip.is_unspecified() || ip.is_loopback() {
        "localhost".to_owned()
    } else if ip.is_ipv6() {
        format!("[{ip}]")
    } else {
        ip.to_string()
    };
    format!("http://{host}:{}", bind.port())
}

#[cfg_attr(not(feature = "tray"), allow(dead_code))]
fn open(target: impl AsRef<std::ffi::OsStr>) {
    #[cfg(feature = "tray")]
    if let Err(err) = open::that_detached(target.as_ref()) {
        tracing::warn!("could not open {}: {err}", target.as_ref().to_string_lossy());
    }
    #[cfg(not(feature = "tray"))]
    let _ = target;
}

/// Decode an embedded PNG into straight RGBA pixels.
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
fn rgba(png: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?.into_rgba8();
    let (width, height) = image.dimensions();
    Some((image.into_raw(), width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_for_bind_addresses() {
        assert_eq!(web_url("0.0.0.0:7539".parse().unwrap()), "http://localhost:7539");
        assert_eq!(web_url("127.0.0.1:8000".parse().unwrap()), "http://localhost:8000");
        assert_eq!(web_url("192.168.1.5:7539".parse().unwrap()), "http://192.168.1.5:7539");
        assert_eq!(web_url("[fe80::1]:7539".parse().unwrap()), "http://[fe80::1]:7539");
    }

    #[test]
    fn embedded_icons_decode() {
        let (pixels, width, height) = rgba(include_bytes!("../../assets/tray-32.png")).unwrap();
        assert_eq!((width, height), (32, 32));
        assert_eq!(pixels.len(), 32 * 32 * 4);
        // Rounded corners are transparent.
        assert_eq!(pixels[3], 0);
    }
}
