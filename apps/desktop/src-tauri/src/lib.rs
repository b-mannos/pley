//! pley for Windows, macOS and Linux: a window on a pley server's web app.
//!
//! It opens on a server picker (the page in `../src`), which finds servers on
//! the local network and remembers the ones opened, then shows the chosen
//! server's web app in the same window. That web app leads back to the picker
//! through `/__app/servers`, which the window catches; links to other sites
//! open in the browser. Pages from servers get no access to the app's
//! commands: only the picker, bundled with the app, has it.

mod discovery;
mod servers;

use std::sync::Mutex;

use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_opener::OpenerExt;
use url::Url;

/// Where the web app sends people who want another server.
const SWITCH_SERVER: &str = "/__app/servers";
/// Tells the web app it runs in this app (src/lib/shell.ts over there).
const INIT_SCRIPT: &str = "window.__PLEY_APP__ = 'desktop';";

/// The picker's address, and the server shown now.
#[derive(Default)]
struct Pages {
    picker: Mutex<Option<Url>>,
    server: Mutex<Option<Url>>,
}

fn same_origin(a: &Url, b: &Url) -> bool {
    a.origin() == b.origin()
}

/// What the window does with a navigation to `url`.
#[derive(Debug, PartialEq)]
enum Navigation {
    Allow,
    /// Back to the picker.
    Picker,
    /// Another site: the browser opens it.
    Browser,
}

fn navigation(url: &Url, picker: Option<&Url>, server: Option<&Url>) -> Navigation {
    if picker.is_some_and(|p| same_origin(p, url)) {
        return Navigation::Allow;
    }
    match server {
        Some(server) if same_origin(server, url) => {
            if url.path().starts_with(SWITCH_SERVER) {
                Navigation::Picker
            } else {
                Navigation::Allow
            }
        }
        _ if matches!(url.scheme(), "http" | "https" | "mailto") => Navigation::Browser,
        _ => Navigation::Allow,
    }
}

fn show_picker<R: Runtime>(window: &WebviewWindow<R>) {
    let pages = window.state::<Pages>();
    let Some(mut picker) = pages.picker.lock().unwrap().clone() else { return };
    *pages.server.lock().unwrap() = None;
    picker.set_query(Some("pick"));
    let _ = window.navigate(picker);
}

fn open_in_browser<R: Runtime>(app: &AppHandle<R>, url: &Url) {
    if let Err(err) = app.opener().open_url(url.as_str(), None::<&str>) {
        eprintln!("cannot open {url}: {err}");
    }
}

/// Shows the server at `url` in the window (the picker calls this once the
/// server answered).
#[tauri::command]
fn open_server(window: WebviewWindow, url: String) -> Result<(), String> {
    let origin = servers::server_origin(&url)?;
    *window.state::<Pages>().server.lock().unwrap() = Some(origin.clone());
    window.navigate(origin).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_opener::init())
        .manage(Pages::default())
        .invoke_handler(tauri::generate_handler![
            servers::saved_servers,
            servers::save_server,
            servers::forget_server,
            servers::check_server,
            discovery::discover,
            open_server
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let opener = app.handle().clone();
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .title("pley")
                .inner_size(1280.0, 800.0)
                .min_inner_size(720.0, 480.0)
                .initialization_script(INIT_SCRIPT)
                .on_navigation(move |url| {
                    let pages = handle.state::<Pages>();
                    let picker = pages.picker.lock().unwrap().clone();
                    let server = pages.server.lock().unwrap().clone();
                    match navigation(url, picker.as_ref(), server.as_ref()) {
                        Navigation::Allow => true,
                        Navigation::Picker => {
                            if let Some(window) = handle.get_webview_window("main") {
                                show_picker(&window);
                            }
                            false
                        }
                        Navigation::Browser => {
                            open_in_browser(&handle, url);
                            false
                        }
                    }
                })
                // Links that open a new window (`target="_blank"`) go to the browser.
                .on_new_window(move |url, _features| {
                    open_in_browser(&opener, &url);
                    NewWindowResponse::Deny
                })
                .build()?;
            let picker = window.url()?;
            *window.state::<Pages>().picker.lock().unwrap() = Some(picker);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("pley could not start");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn the_window_keeps_to_the_server_and_its_picker() {
        let picker = url("http://tauri.localhost/index.html");
        let server = url("http://192.168.1.20:7539");
        let at = |u: &str| navigation(&url(u), Some(&picker), Some(&server));
        assert_eq!(at("http://tauri.localhost/index.html?pick"), Navigation::Allow);
        assert_eq!(at("http://192.168.1.20:7539/item/kx7bq2m"), Navigation::Allow);
        assert_eq!(at("http://192.168.1.20:7539/__app/servers"), Navigation::Picker);
        assert_eq!(at("https://www.themoviedb.org/movie/603"), Navigation::Browser);
        assert_eq!(at("http://192.168.1.20:8080/"), Navigation::Browser, "another port is another site");
        assert_eq!(at("about:blank"), Navigation::Allow);
        // Before a server is open, only the picker is at home.
        assert_eq!(navigation(&url("http://192.168.1.20:7539/"), Some(&picker), None), Navigation::Browser);
        assert_eq!(
            navigation(&url("http://192.168.1.20:7539/__app/servers"), Some(&picker), None),
            Navigation::Browser,
            "only the open server can lead back"
        );
    }
}
