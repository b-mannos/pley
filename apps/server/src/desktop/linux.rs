//! Linux: a StatusNotifierItem over D-Bus (KDE Plasma, GNOME with the
//! AppIndicator extension, Xfce, Cinnamon, …), in pure Rust. Without a session
//! bus or a tray host the icon simply does not appear.

use std::time::Duration;

use ksni::TrayMethods;

use super::{Command, TrayContext, TrayGuard, autostart};

struct PleyTray {
    ctx: TrayContext,
    status: String,
    autostart: bool,
    icons: Vec<ksni::Icon>,
}

impl ksni::Tray for PleyTray {
    fn id(&self) -> String {
        "pley".into()
    }

    fn title(&self) -> String {
        "pley".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        self.icons.clone()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip { title: "pley".into(), description: self.status.clone(), ..Default::default() }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.ctx.open_web_ui();
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{CheckmarkItem, MenuItem, StandardItem};
        vec![
            StandardItem {
                label: "Open pley".into(),
                activate: Box::new(|tray: &mut Self| tray.ctx.open_web_ui()),
                ..Default::default()
            }
            .into(),
            StandardItem { label: self.status.clone(), enabled: false, ..Default::default() }.into(),
            MenuItem::Separator,
            StandardItem {
                label: "Scan all libraries".into(),
                activate: Box::new(|tray: &mut Self| tray.ctx.send(Command::ScanLibraries)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Show logs".into(),
                activate: Box::new(|tray: &mut Self| tray.ctx.open_logs()),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            CheckmarkItem {
                label: "Start pley at login".into(),
                checked: self.autostart,
                activate: Box::new(|tray: &mut Self| tray.autostart = tray.ctx.toggle_autostart(!tray.autostart)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit pley".into(),
                activate: Box::new(|tray: &mut Self| tray.ctx.send(Command::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

struct Guard(tokio::task::JoinHandle<()>);

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub fn start(ctx: TrayContext) -> Option<TrayGuard> {
    let task = tokio::spawn(async move {
        let tray = PleyTray { status: ctx.status(), autostart: autostart::is_enabled(), icons: icons(), ctx };
        let mut status = tray.status.clone();
        let refresh = tray.ctx.clone();
        let handle = match tray.spawn().await {
            Ok(handle) => handle,
            Err(err) => {
                tracing::debug!("no tray icon: {err}");
                return;
            }
        };
        let mut tick = tokio::time::interval(Duration::from_secs(3));
        loop {
            tick.tick().await;
            let current = refresh.status();
            if current != status {
                status = current.clone();
                if handle.update(|tray: &mut PleyTray| tray.status = current).await.is_none() {
                    break; // the tray service stopped
                }
            }
        }
    });
    Some(TrayGuard(Box::new(Guard(task))))
}

/// The logo as ARGB32 (network byte order) pixmaps.
fn icons() -> Vec<ksni::Icon> {
    [&include_bytes!("../../assets/tray-32.png")[..], &include_bytes!("../../assets/tray-64.png")[..]]
        .into_iter()
        .filter_map(super::rgba)
        .map(|(rgba, width, height)| ksni::Icon {
            width: width as i32,
            height: height as i32,
            data: rgba.chunks_exact(4).flat_map(|p| [p[3], p[0], p[1], p[2]]).collect(),
        })
        .collect()
}
