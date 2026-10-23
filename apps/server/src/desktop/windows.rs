//! Windows: the notification-area icon lives on its own thread with a Win32
//! message loop. Left click opens the web UI, right click shows the menu, and
//! a thread timer keeps the tooltip current.

use std::ptr::null_mut;
use std::sync::mpsc;
use std::thread::JoinHandle;

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use windows_sys::Win32::System::Console::{
    FreeConsole, GetConsoleProcessList, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle,
};
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, GetSystemMetrics, KillTimer, MSG, PostThreadMessageW, SM_CXSMICON, SetTimer,
    TranslateMessage, WM_QUIT, WM_TIMER,
};

use super::{Command, TrayContext, TrayGuard, autostart, item};

/// Resource id of the application icon (see build.rs).
const ICON_RESOURCE: u16 = 1;
const REFRESH_MS: u32 = 3000;

struct Guard {
    thread_id: u32,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: posting to a thread id we own; harmless if it already exited.
        unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn start(ctx: TrayContext) -> Option<TrayGuard> {
    let (ready_tx, ready_rx) = mpsc::channel();
    let thread = std::thread::Builder::new().name("pley-tray".into()).spawn(move || run(ctx, ready_tx)).ok()?;
    match ready_rx.recv() {
        Ok(Ok(thread_id)) => {
            detach_console();
            let _ = std::thread::Builder::new().name("pley-tray-promote".into()).spawn(promote_once);
            Some(TrayGuard(Box::new(Guard { thread_id, thread: Some(thread) })))
        }
        Ok(Err(err)) => {
            tracing::warn!("no tray icon: {err}");
            let _ = thread.join();
            None
        }
        Err(_) => None,
    }
}

struct Items {
    open: MenuItem,
    scan: MenuItem,
    logs: MenuItem,
    autostart: CheckMenuItem,
    quit: MenuItem,
}

fn build(ctx: &TrayContext) -> Result<(TrayIcon, Items), String> {
    let items = Items {
        open: MenuItem::with_id(item::OPEN, "&Open pley", true, None),
        scan: MenuItem::with_id(item::SCAN, "&Scan all libraries", true, None),
        logs: MenuItem::with_id(item::LOGS, "Show &logs", true, None),
        autostart: CheckMenuItem::with_id(item::AUTOSTART, "Start pley at log&in", true, autostart::is_enabled(), None),
        quit: MenuItem::with_id(item::QUIT, "&Quit pley", true, None),
    };
    let menu = Menu::new();
    menu.append_items(&[
        &items.open,
        &PredefinedMenuItem::separator(),
        &items.scan,
        &items.logs,
        &PredefinedMenuItem::separator(),
        &items.autostart,
        &PredefinedMenuItem::separator(),
        &items.quit,
    ])
    .map_err(|e| e.to_string())?;
    let tray = TrayIconBuilder::new()
        .with_id("pley")
        .with_icon(icon())
        .with_tooltip(ctx.tooltip())
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|e| e.to_string())?;
    Ok((tray, items))
}

/// The exe's icon resource at the notification area's size (crisp at any
/// DPI), or the embedded PNG when the resource is missing.
fn icon() -> Icon {
    // SAFETY: plain metric query.
    let size = unsafe { GetSystemMetrics(SM_CXSMICON) }.clamp(16, 64) as u32;
    Icon::from_resource(ICON_RESOURCE, Some((size, size))).unwrap_or_else(|_| {
        let (pixels, width, height) = super::rgba(include_bytes!("../../assets/tray-32.png")).unwrap_or_default();
        Icon::from_rgba(pixels, width, height).expect("embedded tray icon")
    })
}

fn run(ctx: TrayContext, ready: mpsc::Sender<Result<u32, String>>) {
    let (tray, items) = match build(&ctx) {
        Ok(built) => built,
        Err(err) => {
            let _ = ready.send(Err(err));
            return;
        }
    };
    // SAFETY: plain Win32 calls on this thread's message queue.
    let thread_id = unsafe { GetCurrentThreadId() };
    let timer = unsafe { SetTimer(null_mut(), 0, REFRESH_MS, None) };
    let _ = ready.send(Ok(thread_id));

    let mut tooltip = ctx.tooltip();
    // SAFETY: MSG is plain data; GetMessageW fills it in.
    let mut msg: MSG = unsafe { std::mem::zeroed() };
    loop {
        // SAFETY: standard message loop on the thread that owns the icon's window.
        let got = unsafe { GetMessageW(&mut msg, null_mut(), 0, 0) };
        if got == 0 || got == -1 {
            break; // WM_QUIT (server stopped) or error
        }
        if msg.message == WM_TIMER && msg.hwnd.is_null() {
            let current = ctx.tooltip();
            if current != tooltip {
                let _ = tray.set_tooltip(Some(&current));
                tooltip = current;
            }
            continue;
        }
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        // Clicks are queued while the message above is dispatched.
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            handle_menu(&ctx, &items, &event);
        }
        while let Ok(event) = TrayIconEvent::receiver().try_recv() {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                ctx.open_web_ui();
            }
        }
    }
    // SAFETY: the timer was created on this thread.
    unsafe { KillTimer(null_mut(), timer) };
    drop(tray); // removes the icon right away instead of leaving a ghost
}

fn handle_menu(ctx: &TrayContext, items: &Items, event: &MenuEvent) {
    let id = &event.id;
    if id == items.open.id() {
        ctx.open_web_ui();
    } else if id == items.scan.id() {
        ctx.send(Command::ScanLibraries);
    } else if id == items.logs.id() {
        ctx.open_logs();
    } else if id == items.autostart.id() {
        // The menu already flipped the checkmark.
        let enabled = ctx.toggle_autostart(items.autostart.is_checked());
        items.autostart.set_checked(enabled);
    } else if id == items.quit.id() {
        items.quit.set_enabled(false);
        ctx.send(Command::Quit);
    }
}

/// Windows 11 puts new notification icons in the hidden overflow. Show pley's
/// on the taskbar the first time: only while Windows has no choice recorded
/// for it, so hiding it in the taskbar settings sticks.
fn promote_once() {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE, REG_DWORD, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegCloseKey,
        RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, RegSetValueExW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let exe = exe.to_string_lossy().to_lowercase();
    // Explorer creates the entry shortly after the icon appears.
    for _ in 0..10 {
        std::thread::sleep(std::time::Duration::from_secs(1));
        // SAFETY: registry calls with valid NUL-terminated strings, sized
        // buffers and keys that are closed before returning.
        unsafe {
            let mut root: HKEY = null_mut();
            let path = wide(r"Control Panel\NotifyIconSettings");
            if RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_READ, &mut root) != ERROR_SUCCESS {
                return; // not Windows 11
            }
            let mut index = 0;
            let mut found = false;
            loop {
                let mut name = [0u16; 64];
                let mut len = name.len() as u32;
                let status = RegEnumKeyExW(
                    root,
                    index,
                    name.as_mut_ptr(),
                    &mut len,
                    null_mut(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                );
                if status != ERROR_SUCCESS {
                    break;
                }
                index += 1;
                let mut buf = [0u16; 1024];
                let mut size = (buf.len() * 2) as u32;
                let value = wide("ExecutablePath");
                let status = RegGetValueW(
                    root,
                    name.as_ptr(),
                    value.as_ptr(),
                    RRF_RT_REG_SZ,
                    null_mut(),
                    buf.as_mut_ptr().cast(),
                    &mut size,
                );
                if status != ERROR_SUCCESS {
                    continue;
                }
                let path = String::from_utf16_lossy(&buf[..(size as usize / 2).saturating_sub(1)]);
                if path.to_lowercase() != exe {
                    continue;
                }
                found = true;
                let promoted = wide("IsPromoted");
                let mut current = 0u32;
                let mut size = 4u32;
                let recorded = RegGetValueW(
                    root,
                    name.as_ptr(),
                    promoted.as_ptr(),
                    RRF_RT_REG_DWORD,
                    null_mut(),
                    (&raw mut current).cast(),
                    &mut size,
                ) == ERROR_SUCCESS;
                if !recorded {
                    let mut entry: HKEY = null_mut();
                    if RegOpenKeyExW(root, name.as_ptr(), 0, KEY_SET_VALUE, &mut entry) == ERROR_SUCCESS {
                        let one = 1u32;
                        RegSetValueExW(entry, promoted.as_ptr(), 0, REG_DWORD, (&raw const one).cast(), 4);
                        RegCloseKey(entry);
                    }
                }
            }
            RegCloseKey(root);
            if found {
                return;
            }
        }
    }
}

/// Close the console window pley got when it was started from Explorer or
/// at login (it is the only process attached): the tray is the interface and
/// logs go to the log file. A console shared with a shell is left alone.
fn detach_console() {
    let mut processes = [0u32; 4];
    // SAFETY: the buffer outlives the call and its length is passed.
    let attached = unsafe { GetConsoleProcessList(processes.as_mut_ptr(), processes.len() as u32) };
    if attached == 1 {
        tracing::info!("running in the notification area; logs are in the data folder");
        crate::logging::console_detached();
        // FreeConsole closes the console's handles but leaves their values as
        // the standard handles, and Windows gives the same values to files
        // opened later. Anything still writing to stdout or stderr (a panic,
        // a library) would write into one of those files: database files
        // opened later were corrupted that way. Point them at NUL first.
        let nul = std::fs::OpenOptions::new().read(true).write(true).open("NUL").ok();
        // Kept open for the life of the process (null makes writes fail
        // quietly if NUL can't be opened).
        let handle = nul.map(std::os::windows::io::IntoRawHandle::into_raw_handle).unwrap_or(null_mut());
        // SAFETY: replacing our own standard handles, then detaching from
        // our own console.
        unsafe {
            SetStdHandle(STD_INPUT_HANDLE, handle);
            SetStdHandle(STD_OUTPUT_HANDLE, handle);
            SetStdHandle(STD_ERROR_HANDLE, handle);
            FreeConsole();
        }
    }
}
