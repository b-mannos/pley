//! Windows: embed the application icon (Explorer, taskbar, tray), version
//! information and a manifest (per-monitor DPI awareness for a crisp tray icon,
//! long path support). Nothing to do on other platforms.

use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/pley.ico");
    println!("cargo:rerun-if-changed=assets/pley.exe.manifest");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let assets = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets");
    let rc_path = |name: &str| assets.join(name).display().to_string().replace('\\', "\\\\");
    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let numeric: Vec<&str> = version.split(['.', '-', '+']).take(3).collect();
    let [major, minor, patch] = [0, 1, 2].map(|i| numeric.get(i).and_then(|n| n.parse::<u16>().ok()).unwrap_or(0));
    let rc = format!(
        r#"1 ICON "{icon}"
1 24 "{manifest}"
1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "FileDescription", "pley media server"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "pley"
      VALUE "OriginalFilename", "pley.exe"
      VALUE "ProductName", "pley"
      VALUE "ProductVersion", "{version}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        icon = rc_path("pley.ico"),
        manifest = rc_path("pley.exe.manifest"),
    );
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("pley.rc");
    std::fs::write(&out, rc).expect("writing pley.rc");
    // A missing resource compiler only costs the icon, not the build.
    if let Err(err) = embed_resource::compile(&out, embed_resource::NONE).manifest_optional() {
        println!("cargo:warning=could not embed the Windows resources: {err}");
    }
}
