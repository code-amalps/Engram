use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    let manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string()));

    // 1. Build profile (debug / release)
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());

    // 2. Active compiler version (single line)
    let rustc_version = std::env::var("RUSTC")
        .ok()
        .and_then(|rustc| {
            Command::new(rustc)
                .arg("--version")
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_string())
        })
        .unwrap_or_else(|| {
            Command::new("rustc")
                .arg("--version")
                .output()
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        });

    // 3. Git commit SHA (single line)
    let git_commit = Command::new("git")
        .current_dir(&manifest_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    // 4. Git dirty state (single line)
    let git_dirty = Command::new("git")
        .current_dir(&manifest_dir)
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let status_str = String::from_utf8_lossy(&out.stdout);
                Some(!status_str.trim().is_empty())
            } else {
                None
            }
        });

    let dirty_str = match git_dirty {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    };

    println!("cargo:rustc-env=ENGRAM_BUILD_GIT_COMMIT={}", git_commit);
    println!("cargo:rustc-env=ENGRAM_BUILD_GIT_DIRTY={}", dirty_str);
    println!("cargo:rustc-env=ENGRAM_BUILD_RUSTC={}", rustc_version);
    println!("cargo:rustc-env=ENGRAM_BUILD_PROFILE={}", profile);
}
