//! The commit the build is made from (`PLEY_COMMIT`, shown beside the version).

use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-env=PLEY_COMMIT={}", commit());
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
