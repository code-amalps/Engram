//! Regression test suite for commit SHA validation and provenance semantics (DEFECT-964-02)
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn make_temp_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{}_{}", prefix, uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn find_build_script_binary() -> PathBuf {
    let build_dir = repo_root().join("target").join("debug").join("build");
    let mut candidates = Vec::new();
    if let Ok(entries) = fs::read_dir(&build_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && path
                    .file_name()
                    .map(|n| n.to_string_lossy().starts_with("engram-"))
                    .unwrap_or(false)
            {
                let bin = path.join("build-script-build.exe");
                if bin.exists() {
                    if let Ok(meta) = bin.metadata() {
                        if let Ok(mtime) = meta.modified() {
                            candidates.push((mtime, bin));
                        }
                    }
                }
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates
        .first()
        .expect("must find compiled build-script-build.exe in target/debug/build")
        .1
        .clone()
}

fn run_build_rs(
    build_dir: &Path,
    envs: &[(&str, &str)],
    remove_envs: &[&str],
) -> (bool, Vec<(String, String)>) {
    let build_bin = find_build_script_binary();

    let mut cmd = Command::new(&build_bin);
    cmd.current_dir(build_dir);
    cmd.env("CARGO_MANIFEST_DIR", build_dir);
    cmd.env("CARGO_PKG_VERSION", "1.5.5");
    cmd.env("PROFILE", "release");
    cmd.env("TARGET", "x86_64-pc-windows-msvc");

    for key in remove_envs {
        cmd.env_remove(key);
    }
    for (k, v) in envs {
        cmd.env(k, v);
    }

    let output = cmd.output().expect("execute build_script_binary");

    if !output.status.success() {
        return (false, Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut vars = Vec::new();
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("cargo:rustc-env=") {
            if let Some((k, v)) = rest.split_once('=') {
                vars.push((k.to_string(), v.to_string()));
            }
        }
    }

    (true, vars)
}

fn init_test_git_repo(path: &Path) -> String {
    Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@engram.dev"])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Engram Tester"])
        .current_dir(path)
        .output()
        .unwrap();

    fs::write(
        path.join("Cargo.toml"),
        "[package]\nname = \"pkg\"\nversion = \"1.5.5\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(path.join("Cargo.lock"), "lock").unwrap();
    fs::write(path.join("build.rs"), "fn main() {}").unwrap();
    fs::create_dir_all(path.join("src")).unwrap();
    fs::write(path.join("src").join("main.rs"), "fn main() {}\n").unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(path)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(path)
        .output()
        .unwrap();

    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(path)
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn get_var<'a>(vars: &'a [(String, String)], key: &str) -> Option<&'a str> {
    vars.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

#[test]
fn test_valid_short_abbreviation_resolved_to_full_commit() {
    let tmp = make_temp_dir("commit_abbrev");
    let full_commit = init_test_git_repo(&tmp);
    let short_abbrev = &full_commit[..7];

    let (ok, vars) = run_build_rs(
        &tmp,
        &[("ENGRAM_GIT_COMMIT", short_abbrev)],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );
    assert!(ok);
    let resolved = get_var(&vars, "ENGRAM_BUILD_GIT_COMMIT").unwrap();
    assert_eq!(
        resolved, full_commit,
        "Abbreviated commit must resolve to full 40-hex object SHA"
    );
    assert_eq!(resolved.len(), 40);
    assert_eq!(
        get_var(&vars, "ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("LocalGitRepository"),
        "Locally resolved abbreviation must report LocalGitRepository provenance"
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_non_commit_object_in_git_marked_invalid() {
    let tmp = make_temp_dir("commit_tree_obj");
    init_test_git_repo(&tmp);
    let out = Command::new("git")
        .args(["rev-parse", "HEAD^{tree}"])
        .current_dir(&tmp)
        .output()
        .unwrap();
    let tree_sha = String::from_utf8(out.stdout).unwrap().trim().to_string();

    let (ok, vars) = run_build_rs(
        &tmp,
        &[("ENGRAM_GIT_COMMIT", &tree_sha)],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );
    assert!(ok);
    assert_eq!(
        get_var(&vars, "ENGRAM_BUILD_GIT_COMMIT"),
        Some("invalid"),
        "Tree object SHA must be rejected as invalid commit"
    );
    assert_eq!(
        get_var(&vars, "ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("Unknown"),
        "Invalid commit input must fall back to Unknown provenance"
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_explicit_snapshot_invalid_fingerprint_falls_back_to_unknown() {
    let tmp = make_temp_dir("commit_inv_fp");
    fs::write(tmp.join("Cargo.lock"), "lock").unwrap();

    let (ok, vars) = run_build_rs(
        &tmp,
        &[
            (
                "ENGRAM_GIT_COMMIT",
                "e6220aa9705436fe6d9a31bb014ad13a286f84ee",
            ),
            ("ENGRAM_SOURCE_FINGERPRINT", "not-a-valid-fingerprint"),
            ("ENGRAM_PROVENANCE_SOURCE", "ExplicitSnapshot"),
        ],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );
    assert!(ok);
    assert_eq!(
        get_var(&vars, "ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("Unknown"),
        "ExplicitSnapshot with invalid fingerprint must fall back to Unknown provenance"
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_ambiguous_or_nonexistent_abbreviation_marked_invalid() {
    let tmp = make_temp_dir("commit_nonexistent");
    init_test_git_repo(&tmp);

    let (ok, vars) = run_build_rs(
        &tmp,
        &[("ENGRAM_GIT_COMMIT", "1234567")],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );
    assert!(ok);
    assert_eq!(
        get_var(&vars, "ENGRAM_BUILD_GIT_COMMIT"),
        Some("invalid"),
        "Non-existent abbreviation must be marked invalid"
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_non_hex_commit_marked_invalid() {
    let tmp = make_temp_dir("commit_nonhex");
    init_test_git_repo(&tmp);

    let (ok, vars) = run_build_rs(
        &tmp,
        &[("ENGRAM_GIT_COMMIT", "not-a-valid-hex-commit!")],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );
    assert!(ok);
    assert_eq!(get_var(&vars, "ENGRAM_BUILD_GIT_COMMIT"), Some("invalid"));

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_too_short_or_overlong_commit_marked_invalid() {
    let tmp = make_temp_dir("commit_len");
    init_test_git_repo(&tmp);

    // 6 chars (too short, min is 7)
    let (_, vars_short) = run_build_rs(
        &tmp,
        &[("ENGRAM_GIT_COMMIT", "abcdef")],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );
    assert_eq!(
        get_var(&vars_short, "ENGRAM_BUILD_GIT_COMMIT"),
        Some("invalid")
    );

    // 65 chars (overlong)
    let overlong = "a".repeat(65);
    let (_, vars_long) = run_build_rs(
        &tmp,
        &[("ENGRAM_GIT_COMMIT", &overlong)],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );
    assert_eq!(
        get_var(&vars_long, "ENGRAM_BUILD_GIT_COMMIT"),
        Some("invalid")
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_explicit_snapshot_full_sha1_valid_without_git() {
    let tmp = make_temp_dir("commit_outside_git");
    fs::write(tmp.join("Cargo.lock"), "lock").unwrap();

    let full_sha1 = "e6220aa9705436fe6d9a31bb014ad13a286f84ee";
    let (ok, vars) = run_build_rs(
        &tmp,
        &[
            ("ENGRAM_GIT_COMMIT", full_sha1),
            (
                "ENGRAM_SOURCE_FINGERPRINT",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            ),
            ("ENGRAM_PROVENANCE_SOURCE", "ExplicitSnapshot"),
        ],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );
    assert!(ok);
    assert_eq!(get_var(&vars, "ENGRAM_BUILD_GIT_COMMIT"), Some(full_sha1));
    assert_eq!(
        get_var(&vars, "ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("ExplicitSnapshot")
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_explicit_snapshot_abbreviation_outside_git_rejected() {
    let tmp = make_temp_dir("commit_abbrev_outside_git");
    fs::write(tmp.join("Cargo.lock"), "lock").unwrap();

    // 7-char abbreviation without git database cannot be resolved
    let (ok, vars) = run_build_rs(
        &tmp,
        &[
            ("ENGRAM_GIT_COMMIT", "e6220aa"),
            (
                "ENGRAM_SOURCE_FINGERPRINT",
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            ),
        ],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );
    assert!(ok);
    assert_eq!(
        get_var(&vars, "ENGRAM_BUILD_GIT_COMMIT"),
        Some("invalid"),
        "Abbreviated commit without git database must be rejected as invalid"
    );

    let _ = fs::remove_dir_all(&tmp);
}
