//! Start pley when the user logs in: the `Run` registry key on Windows, a
//! LaunchAgent on macOS and an XDG autostart entry on Linux. The entry runs
//! the current executable with the arguments pley was started with.

use std::ffi::OsString;
use std::path::PathBuf;

use anyhow::Context;

#[cfg(target_os = "macos")]
const AGENT_LABEL: &str = "pley.server";

/// Whether an autostart entry exists for the current user.
pub fn is_enabled() -> bool {
    platform::is_enabled()
}

pub fn set_enabled(enabled: bool) -> anyhow::Result<()> {
    if enabled { platform::enable(&command()?) } else { platform::disable() }
}

/// The current executable and its arguments.
fn command() -> anyhow::Result<(PathBuf, Vec<OsString>)> {
    let exe = std::env::current_exe().context("locating the pley executable")?;
    Ok((exe, std::env::args_os().skip(1).collect()))
}

/// Quote one argument the way `CommandLineToArgvW` and the MSVC runtime
/// parse it back.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn windows_quote(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_owned();
    }
    let mut out = String::from('"');
    let mut backslashes = 0;
    for c in arg.chars() {
        match c {
            '\\' => backslashes += 1,
            '"' => {
                out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                out.push('"');
                backslashes = 0;
            }
            c => {
                out.extend(std::iter::repeat_n('\\', backslashes));
                out.push(c);
                backslashes = 0;
            }
        }
    }
    out.extend(std::iter::repeat_n('\\', backslashes * 2));
    out.push('"');
    out
}

/// Quote one argument for the `Exec` key of a desktop entry.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn desktop_entry_quote(arg: &str) -> String {
    let reserved = |c: char| " \t\n\"'\\><~|&;$*?#()`".contains(c);
    if !arg.is_empty() && !arg.chars().any(reserved) {
        return arg.replace('%', "%%");
    }
    let mut out = String::from('"');
    for c in arg.chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out.replace('%', "%%")
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(windows)]
mod platform {
    use std::ffi::OsString;
    use std::path::Path;
    use std::ptr::null_mut;

    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW, RegOpenKeyExW,
        RegQueryValueExW, RegSetValueExW,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "pley";

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }

    struct Key(HKEY);

    impl Key {
        fn open(access: u32) -> std::io::Result<Self> {
            let mut key: HKEY = null_mut();
            // SAFETY: valid, NUL-terminated strings and out pointer.
            let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, wide(RUN_KEY).as_ptr(), 0, access, &mut key) };
            if status == ERROR_SUCCESS { Ok(Key(key)) } else { Err(std::io::Error::from_raw_os_error(status as i32)) }
        }
    }

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the key was opened by `Key::open`.
            unsafe { RegCloseKey(self.0) };
        }
    }

    pub fn is_enabled() -> bool {
        let Ok(key) = Key::open(KEY_QUERY_VALUE) else { return false };
        // SAFETY: only asks whether the value exists.
        let status =
            unsafe { RegQueryValueExW(key.0, wide(VALUE).as_ptr(), null_mut(), null_mut(), null_mut(), null_mut()) };
        status == ERROR_SUCCESS
    }

    pub fn enable((exe, args): &(std::path::PathBuf, Vec<OsString>)) -> anyhow::Result<()> {
        let line = command_line(exe, args);
        let data = wide(&line);
        let key = Key::open(KEY_SET_VALUE)?;
        // SAFETY: `data` is a NUL-terminated UTF-16 string; the size is in bytes.
        let status = unsafe {
            RegSetValueExW(key.0, wide(VALUE).as_ptr(), 0, REG_SZ, data.as_ptr().cast(), (data.len() * 2) as u32)
        };
        if status != ERROR_SUCCESS {
            return Err(std::io::Error::from_raw_os_error(status as i32).into());
        }
        Ok(())
    }

    pub fn disable() -> anyhow::Result<()> {
        let key = Key::open(KEY_SET_VALUE)?;
        // SAFETY: valid key and value name.
        let status = unsafe { RegDeleteValueW(key.0, wide(VALUE).as_ptr()) };
        if status != ERROR_SUCCESS && status != ERROR_FILE_NOT_FOUND {
            return Err(std::io::Error::from_raw_os_error(status as i32).into());
        }
        Ok(())
    }

    fn command_line(exe: &Path, args: &[OsString]) -> String {
        std::iter::once(exe.as_os_str())
            .chain(args.iter().map(OsString::as_os_str))
            .map(|a| super::windows_quote(&a.to_string_lossy()))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use anyhow::Context;

    fn plist_path() -> Option<PathBuf> {
        Some(dirs::home_dir()?.join("Library/LaunchAgents").join(format!("{}.plist", super::AGENT_LABEL)))
    }

    pub fn is_enabled() -> bool {
        plist_path().is_some_and(|p| p.exists())
    }

    pub fn enable((exe, args): &(PathBuf, Vec<OsString>)) -> anyhow::Result<()> {
        let path = plist_path().context("no home folder")?;
        let arguments: String = std::iter::once(exe.as_os_str())
            .chain(args.iter().map(OsString::as_os_str))
            .map(|a| format!("        <string>{}</string>\n", super::xml_escape(&a.to_string_lossy())))
            .collect();
        let plist = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
{arguments}    </array>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin</string>
    </dict>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#,
            label = super::AGENT_LABEL
        );
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, plist).with_context(|| format!("writing {}", path.display()))
    }

    pub fn disable() -> anyhow::Result<()> {
        match plist_path().map(std::fs::remove_file) {
            Some(Err(err)) if err.kind() != std::io::ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use anyhow::Context;

    fn entry_path() -> Option<PathBuf> {
        Some(dirs::config_dir()?.join("autostart").join("pley.desktop"))
    }

    pub fn is_enabled() -> bool {
        entry_path().is_some_and(|p| p.exists())
    }

    pub fn enable((exe, args): &(PathBuf, Vec<OsString>)) -> anyhow::Result<()> {
        let path = entry_path().context("no config folder")?;
        let exec = std::iter::once(exe.as_os_str())
            .chain(args.iter().map(OsString::as_os_str))
            .map(|a| super::desktop_entry_quote(&a.to_string_lossy()))
            .collect::<Vec<_>>()
            .join(" ");
        let entry = format!(
            "[Desktop Entry]\nType=Application\nName=Pley\nComment=Pley media server\nExec={exec}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
        );
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&path, entry).with_context(|| format!("writing {}", path.display()))
    }

    pub fn disable() -> anyhow::Result<()> {
        match entry_path().map(std::fs::remove_file) {
            Some(Err(err)) if err.kind() != std::io::ErrorKind::NotFound => Err(err.into()),
            _ => Ok(()),
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod platform {
    use std::ffi::OsString;
    use std::path::PathBuf;

    pub fn is_enabled() -> bool {
        false
    }

    pub fn enable(_: &(PathBuf, Vec<OsString>)) -> anyhow::Result<()> {
        anyhow::bail!("starting at login is not supported on this system")
    }

    pub fn disable() -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_arguments_round_trip() {
        assert_eq!(windows_quote("pley.exe"), "pley.exe");
        assert_eq!(windows_quote(r"C:\Program Files\pley\pley.exe"), r#""C:\Program Files\pley\pley.exe""#);
        assert_eq!(windows_quote(r"C:\data dir\"), r#""C:\data dir\\""#);
        assert_eq!(windows_quote(r#"say "hi""#), r#""say \"hi\"""#);
        assert_eq!(windows_quote(""), r#""""#);
    }

    #[test]
    fn desktop_entry_arguments() {
        assert_eq!(desktop_entry_quote("/usr/bin/pley"), "/usr/bin/pley");
        assert_eq!(desktop_entry_quote("/home/me/My Media"), r#""/home/me/My Media""#);
        assert_eq!(desktop_entry_quote("100%"), "100%%");
        assert_eq!(desktop_entry_quote("$HOME"), r#""\$HOME""#);
    }
}
