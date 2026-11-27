//! Comprehensive regression suite for deterministic source fingerprinting (DEFECT-964-01)
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

fn init_test_git_repo(path: &Path) {
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
    fs::write(
        path.join("src").join("main.rs"),
        "fn main() { println!(\"1\"); }\n",
    )
    .unwrap();

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
}

fn get_var<'a>(vars: &'a [(String, String)], key: &str) -> Option<&'a str> {
    vars.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

#[test]
fn test_clean_repository_fingerprint() {
    let tmp = make_temp_dir("fp_clean");
    init_test_git_repo(&tmp);

    let (ok, vars) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    assert!(ok);
    assert_eq!(get_var(&vars, "ENGRAM_BUILD_GIT_DIRTY"), Some("false"));
    let fp = get_var(&vars, "ENGRAM_BUILD_SOURCE_FINGERPRINT").expect("has fp");
    assert_eq!(fp.len(), 64, "Source fingerprint must be 64-hex");
    assert_ne!(fp, "unknown");

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_critical_collision_regression_different_dirty_modifications() {
    let tmp1 = make_temp_dir("fp_col1");
    let tmp2 = make_temp_dir("fp_col2");
    init_test_git_repo(&tmp1);

    // Clone tmp1 to tmp2 so they share the exact same commit
    Command::new("git")
        .args(["clone", tmp1.to_str().unwrap(), tmp2.to_str().unwrap()])
        .output()
        .unwrap();

    // Modify tmp1 with edit A
    fs::write(
        tmp1.join("src").join("main.rs"),
        "fn main() { println!(\"edit A\"); }\n",
    )
    .unwrap();

    // Modify tmp2 with edit B
    fs::write(
        tmp2.join("src").join("main.rs"),
        "fn main() { println!(\"edit B - DIFFERENT\"); }\n",
    )
    .unwrap();

    let (ok1, vars1) = run_build_rs(&tmp1, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    let (ok2, vars2) = run_build_rs(&tmp2, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    assert!(ok1 && ok2);

    let commit1 = get_var(&vars1, "ENGRAM_BUILD_GIT_COMMIT");
    let commit2 = get_var(&vars2, "ENGRAM_BUILD_GIT_COMMIT");
    assert_eq!(commit1, commit2, "Commits must be identical");

    let dirty1 = get_var(&vars1, "ENGRAM_BUILD_GIT_DIRTY");
    let dirty2 = get_var(&vars2, "ENGRAM_BUILD_GIT_DIRTY");
    assert_eq!(dirty1, Some("true"));
    assert_eq!(dirty2, Some("true"));

    let fp1 = get_var(&vars1, "ENGRAM_BUILD_SOURCE_FINGERPRINT").unwrap();
    let fp2 = get_var(&vars2, "ENGRAM_BUILD_SOURCE_FINGERPRINT").unwrap();
    assert_ne!(
        fp1, fp2,
        "Source fingerprints MUST differ between distinct dirty trees"
    );

    let bid1 = get_var(&vars1, "ENGRAM_BUILD_ID").unwrap();
    let bid2 = get_var(&vars2, "ENGRAM_BUILD_ID").unwrap();
    assert_ne!(
        bid1, bid2,
        "Build IDs MUST differ between distinct dirty trees"
    );

    let _ = fs::remove_dir_all(&tmp1);
    let _ = fs::remove_dir_all(&tmp2);
}

#[test]
fn test_added_source_file_changes_fingerprint() {
    let tmp = make_temp_dir("fp_add");
    init_test_git_repo(&tmp);

    let (_, vars_before) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    let fp_before = get_var(&vars_before, "ENGRAM_BUILD_SOURCE_FINGERPRINT")
        .unwrap()
        .to_string();

    fs::write(tmp.join("src").join("new_module.rs"), "pub fn test() {}\n").unwrap();

    let (_, vars_after) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    let fp_after = get_var(&vars_after, "ENGRAM_BUILD_SOURCE_FINGERPRINT")
        .unwrap()
        .to_string();

    assert_ne!(
        fp_before, fp_after,
        "Adding a source file must alter fingerprint"
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_deleted_source_file_changes_fingerprint() {
    let tmp = make_temp_dir("fp_del");
    init_test_git_repo(&tmp);

    fs::write(tmp.join("src").join("extra.rs"), "pub fn extra() {}\n").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&tmp)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add extra"])
        .current_dir(&tmp)
        .output()
        .unwrap();

    let (_, vars_before) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    let fp_before = get_var(&vars_before, "ENGRAM_BUILD_SOURCE_FINGERPRINT")
        .unwrap()
        .to_string();

    fs::remove_file(tmp.join("src").join("extra.rs")).unwrap();

    let (_, vars_after) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    let fp_after = get_var(&vars_after, "ENGRAM_BUILD_SOURCE_FINGERPRINT")
        .unwrap()
        .to_string();

    assert_ne!(
        fp_before, fp_after,
        "Deleting a source file must alter fingerprint"
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_manifest_and_lockfile_change_alters_fingerprint() {
    let tmp = make_temp_dir("fp_manifest");
    init_test_git_repo(&tmp);

    let (_, vars_base) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    let fp_base = get_var(&vars_base, "ENGRAM_BUILD_SOURCE_FINGERPRINT")
        .unwrap()
        .to_string();

    // Change Cargo.toml
    fs::write(
        tmp.join("Cargo.toml"),
        "[package]\nname = \"pkg-mod\"\nversion = \"1.5.5\"\nedition = \"2021\"\n",
    )
    .unwrap();
    let (_, vars_toml) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    let fp_toml = get_var(&vars_toml, "ENGRAM_BUILD_SOURCE_FINGERPRINT")
        .unwrap()
        .to_string();
    assert_ne!(fp_base, fp_toml, "Cargo.toml change must alter fingerprint");

    // Change Cargo.lock
    fs::write(tmp.join("Cargo.lock"), "lock-modified").unwrap();
    let (_, vars_lock) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    let fp_lock = get_var(&vars_lock, "ENGRAM_BUILD_SOURCE_FINGERPRINT")
        .unwrap()
        .to_string();
    assert_ne!(fp_toml, fp_lock, "Cargo.lock change must alter fingerprint");

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_excluded_output_dir_does_not_alter_fingerprint() {
    let tmp = make_temp_dir("fp_excluded");
    init_test_git_repo(&tmp);

    let (_, vars_base) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    let fp_base = get_var(&vars_base, "ENGRAM_BUILD_SOURCE_FINGERPRINT")
        .unwrap()
        .to_string();

    // Create files in target/, docs/, releases/
    fs::create_dir_all(tmp.join("target")).unwrap();
    fs::write(tmp.join("target").join("output.bin"), b"binary junk").unwrap();

    fs::create_dir_all(tmp.join("docs")).unwrap();
    fs::write(tmp.join("docs").join("report.md"), "# Some doc").unwrap();

    let (_, vars_after) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    let fp_after = get_var(&vars_after, "ENGRAM_BUILD_SOURCE_FINGERPRINT")
        .unwrap()
        .to_string();

    assert_eq!(
        fp_base, fp_after,
        "Changes in excluded target/ and docs/ must NOT alter fingerprint"
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_repeated_computation_stability() {
    let tmp = make_temp_dir("fp_stable");
    init_test_git_repo(&tmp);

    let (_, vars1) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    let (_, vars2) = run_build_rs(&tmp, &[], &["GIT_DIR", "GIT_WORK_TREE"]);

    assert_eq!(
        get_var(&vars1, "ENGRAM_BUILD_SOURCE_FINGERPRINT"),
        get_var(&vars2, "ENGRAM_BUILD_SOURCE_FINGERPRINT"),
        "Repeated computation on same snapshot must produce identical fingerprint"
    );

    assert_eq!(
        get_var(&vars1, "ENGRAM_BUILD_ID"),
        get_var(&vars2, "ENGRAM_BUILD_ID"),
        "Repeated computation must produce identical build_id"
    );

    let _ = fs::remove_dir_all(&tmp);
}
