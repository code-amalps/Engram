//! Regression test suite for deterministic build identity and Git discovery isolation.
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

#[test]
fn test_live_repository_root_build_metadata() {
    assert_eq!(engram::build_info::VERSION, "1.5.5");
    assert_eq!(
        engram::build_info::PROVENANCE_SOURCE,
        "LocalGitRepository",
        "Repository root build must report LocalGitRepository"
    );
    assert_eq!(
        engram::build_info::GIT_COMMIT.len(),
        40,
        "Commit must be 40 hex characters"
    );
    assert!(
        engram::build_info::GIT_DIRTY == "true" || engram::build_info::GIT_DIRTY == "false",
        "Dirty must be boolean string"
    );
    assert!(engram::build_info::SCHEMA_VERSION == "1" || engram::build_info::SCHEMA_VERSION == "2");
    assert_eq!(engram::build_info::BUILD_ID.len(), 64);
}

#[test]
fn test_explicit_snapshot_metadata_full() {
    let ws = make_temp_dir("engram_explicit_test");
    fs::write(ws.join("Cargo.lock"), "lockfile content").unwrap();

    let commit = "0123456789abcdef0123456789abcdef01234567";
    let fingerprint = "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855";

    let (ok, vars) = run_build_rs(
        &ws,
        &[
            ("ENGRAM_GIT_COMMIT", commit),
            ("ENGRAM_SOURCE_FINGERPRINT", fingerprint),
            ("ENGRAM_PROVENANCE_SOURCE", "ExplicitSnapshot"),
        ],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );

    assert!(ok, "build.rs must succeed with explicit metadata");
    let find_var = |name: &str| {
        vars.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(find_var("ENGRAM_BUILD_GIT_COMMIT"), Some(commit));
    assert_eq!(
        find_var("ENGRAM_BUILD_SOURCE_FINGERPRINT"),
        Some(fingerprint)
    );
    assert_eq!(
        find_var("ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("ExplicitSnapshot")
    );
    assert_eq!(find_var("ENGRAM_BUILD_GIT_DIRTY"), Some("false"));
    assert!(
        find_var("ENGRAM_BUILD_SCHEMA_VERSION") == Some("1")
            || find_var("ENGRAM_BUILD_SCHEMA_VERSION") == Some("2")
    );
    assert!(find_var("ENGRAM_BUILD_ID").unwrap().len() == 64);

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_invalid_fingerprint_rejection() {
    let ws = make_temp_dir("engram_invalid_fp_test");
    fs::write(ws.join("Cargo.lock"), "lock").unwrap();

    let (ok, vars) = run_build_rs(
        &ws,
        &[
            (
                "ENGRAM_GIT_COMMIT",
                "0123456789abcdef0123456789abcdef01234567",
            ),
            ("ENGRAM_SOURCE_FINGERPRINT", "not-a-valid-sha256"),
        ],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );

    assert!(ok, "build.rs must complete gracefully on invalid inputs");
    let find_var = |name: &str| {
        vars.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(
        find_var("ENGRAM_BUILD_SOURCE_FINGERPRINT"),
        Some("invalid"),
        "Invalid fingerprint must be marked invalid"
    );
    assert_eq!(
        find_var("ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("Unknown"),
        "Invalid explicit input must fall back to Unknown provenance"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_partial_explicit_metadata_no_ambient_fallback() {
    let ws = make_temp_dir("engram_partial_meta_test");
    fs::write(ws.join("Cargo.lock"), "lock").unwrap();

    let commit = "0123456789abcdef0123456789abcdef01234567";
    let (ok, vars) = run_build_rs(
        &ws,
        &[("ENGRAM_GIT_COMMIT", commit)],
        &["GIT_DIR", "GIT_WORK_TREE"],
    );

    assert!(ok, "build.rs must succeed with partial explicit metadata");
    let find_var = |name: &str| {
        vars.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(find_var("ENGRAM_BUILD_GIT_COMMIT"), Some(commit));
    assert_eq!(find_var("ENGRAM_BUILD_SOURCE_FINGERPRINT"), Some("unknown"));
    assert_eq!(
        find_var("ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("ExplicitSnapshot")
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_nested_source_snapshot_git_isolation() {
    let nested = repo_root()
        .join("target")
        .join("test_nested_snapshot_isolation");
    let _ = fs::remove_dir_all(&nested);
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("Cargo.lock"), "lock").unwrap();

    let (ok, vars) = run_build_rs(&nested, &[], &["GIT_DIR", "GIT_WORK_TREE"]);

    assert!(ok, "build.rs must succeed in nested directory");
    let find_var = |name: &str| {
        vars.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(
        find_var("ENGRAM_BUILD_GIT_COMMIT"),
        Some("unknown"),
        "Nested snapshot without local .git must NOT inherit parent git commit"
    );
    assert_eq!(
        find_var("ENGRAM_BUILD_GIT_DIRTY"),
        Some("unknown"),
        "Nested snapshot without local .git must NOT inherit parent dirty state"
    );
    assert_eq!(
        find_var("ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("Unknown"),
        "Nested snapshot must report Unknown provenance"
    );

    let _ = fs::remove_dir_all(&nested);
}

#[test]
fn test_inherited_git_dir_and_work_tree_sanitization() {
    let ws = make_temp_dir("engram_git_env_sanitization");
    fs::write(ws.join("Cargo.lock"), "lock").unwrap();

    let fake_git_dir = repo_root().join(".git");
    let (ok, vars) = run_build_rs(
        &ws,
        &[
            ("GIT_DIR", fake_git_dir.to_str().unwrap()),
            ("GIT_WORK_TREE", repo_root().to_str().unwrap()),
        ],
        &[],
    );

    assert!(ok, "build.rs must succeed with inherited git env");
    let find_var = |name: &str| {
        vars.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(
        find_var("ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("Unknown"),
        "Inherited GIT_DIR/GIT_WORK_TREE must be sanitized and ignored"
    );
    assert_eq!(find_var("ENGRAM_BUILD_GIT_COMMIT"), Some("unknown"));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_git_worktree_identification() {
    let tmp = make_temp_dir("engram_worktree_test");
    let repo_dir = tmp.join("main_repo");
    let wt_dir = tmp.join("worktree_copy");
    fs::create_dir_all(&repo_dir).unwrap();

    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    fs::write(repo_dir.join("test.txt"), "hello").unwrap();
    Command::new("git")
        .args(["add", "test.txt"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();

    Command::new("git")
        .args([
            "worktree",
            "add",
            wt_dir.to_str().unwrap(),
            "-b",
            "wt_branch",
        ])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    fs::write(wt_dir.join("Cargo.lock"), "lock").unwrap();
    Command::new("git")
        .args(["add", "Cargo.lock"])
        .current_dir(&wt_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add lock"])
        .current_dir(&wt_dir)
        .output()
        .unwrap();

    let wt_head = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&wt_dir)
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();

    assert!(
        wt_dir.join(".git").is_file(),
        "Worktree .git must be a file"
    );

    let (ok, vars) = run_build_rs(&wt_dir, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    assert!(ok, "build.rs must succeed in git worktree");
    let find_var = |name: &str| {
        vars.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(
        find_var("ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("LocalGitRepository")
    );
    assert_eq!(find_var("ENGRAM_BUILD_GIT_COMMIT"), Some(wt_head.as_str()));
    assert_eq!(find_var("ENGRAM_BUILD_GIT_DIRTY"), Some("false"));

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_deterministic_build_identity_stability() {
    let ws = make_temp_dir("engram_det_stability");
    fs::write(ws.join("Cargo.lock"), "lock").unwrap();

    let envs = [
        (
            "ENGRAM_GIT_COMMIT",
            "0123456789abcdef0123456789abcdef01234567",
        ),
        (
            "ENGRAM_SOURCE_FINGERPRINT",
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
        ),
        ("ENGRAM_PROVENANCE_SOURCE", "ExplicitSnapshot"),
    ];

    let (ok1, vars1) = run_build_rs(&ws, &envs, &["GIT_DIR", "GIT_WORK_TREE"]);
    let (ok2, vars2) = run_build_rs(&ws, &envs, &["GIT_DIR", "GIT_WORK_TREE"]);

    assert!(ok1 && ok2, "Both runs must succeed");
    let id1 = vars1
        .iter()
        .find(|(k, _)| k == "ENGRAM_BUILD_ID")
        .unwrap()
        .1
        .clone();
    let id2 = vars2
        .iter()
        .find(|(k, _)| k == "ENGRAM_BUILD_ID")
        .unwrap()
        .1
        .clone();
    assert_eq!(id1, id2, "Identical inputs must yield identical build_id");

    let diff_fp = [
        (
            "ENGRAM_GIT_COMMIT",
            "0123456789abcdef0123456789abcdef01234567",
        ),
        (
            "ENGRAM_SOURCE_FINGERPRINT",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        ),
        ("ENGRAM_PROVENANCE_SOURCE", "ExplicitSnapshot"),
    ];
    let (ok3, vars3) = run_build_rs(&ws, &diff_fp, &["GIT_DIR", "GIT_WORK_TREE"]);
    assert!(ok3, "Run with diff fingerprint must succeed");
    let id3 = vars3
        .iter()
        .find(|(k, _)| k == "ENGRAM_BUILD_ID")
        .unwrap()
        .1
        .clone();
    assert_ne!(
        id1, id3,
        "Different fingerprint must yield different build_id"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_clean_and_dirty_repository_detection() {
    let tmp = make_temp_dir("engram_clean_dirty_test");
    let repo_dir = tmp.join("repo");
    fs::create_dir_all(&repo_dir).unwrap();

    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    fs::write(repo_dir.join("Cargo.lock"), "lock").unwrap();
    Command::new("git")
        .args(["add", "Cargo.lock"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "init"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();

    // 1. Clean repo
    let (ok1, vars1) = run_build_rs(&repo_dir, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    assert!(ok1, "build.rs must succeed in clean repo");
    let find_var1 = |name: &str| {
        vars1
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(find_var1("ENGRAM_BUILD_GIT_DIRTY"), Some("false"));
    assert_eq!(
        find_var1("ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("LocalGitRepository")
    );

    // 2. Dirty repo (modify tracked file)
    fs::write(repo_dir.join("Cargo.lock"), "modified lock").unwrap();
    let (ok2, vars2) = run_build_rs(&repo_dir, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    assert!(ok2, "build.rs must succeed in dirty repo");
    let find_var2 = |name: &str| {
        vars2
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(find_var2("ENGRAM_BUILD_GIT_DIRTY"), Some("true"));
    assert_eq!(
        find_var2("ENGRAM_BUILD_PROVENANCE_SOURCE"),
        Some("LocalGitRepository")
    );

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_isolated_copied_source_snapshot() {
    let isolated = make_temp_dir("engram_isolated_outside_git");
    fs::write(isolated.join("Cargo.lock"), "lock").unwrap();

    let (ok, vars) = run_build_rs(&isolated, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    assert!(
        ok,
        "build.rs must succeed in isolated directory outside git"
    );
    let find_var = |name: &str| {
        vars.iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(find_var("ENGRAM_BUILD_GIT_COMMIT"), Some("unknown"));
    assert_eq!(find_var("ENGRAM_BUILD_GIT_DIRTY"), Some("unknown"));
    assert_eq!(find_var("ENGRAM_BUILD_SOURCE_FINGERPRINT"), Some("unknown"));
    assert_eq!(find_var("ENGRAM_BUILD_PROVENANCE_SOURCE"), Some("Unknown"));

    let _ = fs::remove_dir_all(&isolated);
}

#[test]
fn test_different_parent_repo_state_does_not_affect_nested_snapshot() {
    let tmp = make_temp_dir("engram_parent_isolation_test");
    let parent_repo = tmp.join("parent");
    let nested_snapshot = parent_repo.join("nested_src");
    fs::create_dir_all(&nested_snapshot).unwrap();

    Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&parent_repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&parent_repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&parent_repo)
        .output()
        .unwrap();
    fs::write(parent_repo.join("file1.txt"), "parent content").unwrap();
    Command::new("git")
        .args(["add", "file1.txt"])
        .current_dir(&parent_repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "p1"])
        .current_dir(&parent_repo)
        .output()
        .unwrap();

    fs::write(nested_snapshot.join("Cargo.lock"), "lock").unwrap();

    // Nested run 1 under parent commit 1
    let (ok1, vars1) = run_build_rs(&nested_snapshot, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    assert!(ok1);
    let id1 = vars1
        .iter()
        .find(|(k, _)| k == "ENGRAM_BUILD_ID")
        .unwrap()
        .1
        .clone();

    // Mutate parent repo: new commit, dirty files in parent
    fs::write(parent_repo.join("file2.txt"), "dirty in parent").unwrap();
    Command::new("git")
        .args(["add", "file2.txt"])
        .current_dir(&parent_repo)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "p2"])
        .current_dir(&parent_repo)
        .output()
        .unwrap();
    fs::write(parent_repo.join("file3.txt"), "uncommitted in parent").unwrap();

    // Nested run 2 under modified parent repo
    let (ok2, vars2) = run_build_rs(&nested_snapshot, &[], &["GIT_DIR", "GIT_WORK_TREE"]);
    assert!(ok2);
    let id2 = vars2
        .iter()
        .find(|(k, _)| k == "ENGRAM_BUILD_ID")
        .unwrap()
        .1
        .clone();

    assert_eq!(
        id1, id2,
        "Parent repo mutations must NOT change the nested copied snapshot's build identity"
    );
    let find_var2 = |name: &str| {
        vars2
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    assert_eq!(find_var2("ENGRAM_BUILD_GIT_COMMIT"), Some("unknown"));
    assert_eq!(find_var2("ENGRAM_BUILD_GIT_DIRTY"), Some("unknown"));
    assert_eq!(find_var2("ENGRAM_BUILD_PROVENANCE_SOURCE"), Some("Unknown"));

    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_different_toolchain_reflects_in_build_id() {
    let ws = make_temp_dir("engram_toolchain_test");
    fs::write(ws.join("Cargo.lock"), "lock").unwrap();

    let envs1 = [
        (
            "ENGRAM_SOURCE_FINGERPRINT",
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
        ),
        ("RUSTC", "rustc"),
    ];
    let (ok1, vars1) = run_build_rs(&ws, &envs1, &["GIT_DIR", "GIT_WORK_TREE"]);
    assert!(ok1);
    let id1 = vars1
        .iter()
        .find(|(k, _)| k == "ENGRAM_BUILD_ID")
        .unwrap()
        .1
        .clone();

    let mut cmd2 = Command::new(find_build_script_binary());
    cmd2.current_dir(&ws);
    cmd2.env("CARGO_MANIFEST_DIR", &ws);
    cmd2.env("CARGO_PKG_VERSION", "1.5.5");
    cmd2.env("PROFILE", "debug"); // different profile!
    cmd2.env("TARGET", "x86_64-pc-windows-msvc");
    cmd2.env(
        "ENGRAM_SOURCE_FINGERPRINT",
        "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
    );

    let out2 = cmd2.output().unwrap();
    assert!(out2.status.success());
    let stdout2 = String::from_utf8_lossy(&out2.stdout);
    let id2 = stdout2
        .lines()
        .find(|l| l.starts_with("cargo:rustc-env=ENGRAM_BUILD_ID="))
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .split_once('=')
        .unwrap()
        .1;

    assert_ne!(
        id1, id2,
        "Different build profile/environment must produce different build_id"
    );

    let _ = fs::remove_dir_all(&ws);
}
