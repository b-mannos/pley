//! The commit the build is made from (`PLEY_COMMIT`, shown beside the
//! version), and on Windows: embed the application icon (Explorer, taskbar,
//! tray), version information and a manifest (per-monitor DPI awareness for a
//! crisp tray icon, long path support).

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let commit = commit();
    println!("cargo:rustc-env=PLEY_COMMIT={commit}");
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
      VALUE "CompanyName", "the pley authors"
      VALUE "FileDescription", "pley media server"
      VALUE "FileVersion", "{version}"
      VALUE "InternalName", "pley"
      VALUE "LegalCopyright", "Copyright (c) 2026 the pley authors"
      VALUE "OriginalFilename", "pley.exe"
      VALUE "ProductName", "pley"
      VALUE "ProductVersion", "{version} ({commit})"
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

/// `PLEY_COMMIT` when set (builds without the repository, such as Docker's),
/// else git's short hash, else "unknown". Rebuilt when HEAD moves.
fn commit() -> String {
    println!("cargo:rerun-if-env-changed=PLEY_COMMIT");
    if let Some(given) = env::var("PLEY_COMMIT").ok().map(|c| c.trim().to_owned()).filter(|c| !c.is_empty()) {
        return given.chars().take(9).collect();
    }
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| String::from_utf8(out.stdout).ok())
            .map(|s| s.trim().to_owned())
    };
    for watched in [git(&["rev-parse", "--git-path", "HEAD"]), git(&["rev-parse", "--git-path", "packed-refs"])]
        .into_iter()
        .chain([git(&["symbolic-ref", "-q", "HEAD"]).and_then(|r| git(&["rev-parse", "--git-path", &r]))])
        .flatten()
    {
        println!("cargo:rerun-if-changed={watched}");
    }
    git(&["rev-parse", "--short=9", "HEAD"]).unwrap_or_else(|| "unknown".into())
}
