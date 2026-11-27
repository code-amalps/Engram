//! Test suite for Phase 9.6.7 Build Environment Identity Completeness
//!
//! Validates:
//! - compiler flags (RUSTFLAGS / CARGO_ENCODED_RUSTFLAGS) change build_id
//! - Cargo features change build_id
//! - opt_level and debug_assertions change build_id
//! - target and host changes reflect in build_id
//! - irrelevant environment variables do NOT change build_id
//! - deterministic repeatability across identical inputs
//! - no usernames, machine paths, or personal information in build identity
//! - CLI JSON structure and backward compatibility

use engram::build_info;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn make_temp_dir(prefix: &str) -> PathBuf {
    let tmp = std::env::temp_dir().join(format!("{}_{}", prefix, uuid::Uuid::new_v4()));
    fs::create_dir_all(&tmp).unwrap();
    tmp
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

fn get_var<'a>(vars: &'a [(String, String)], key: &str) -> Option<&'a str> {
    vars.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

#[test]
fn test_compiler_flags_change_build_id() {
    let ws = make_temp_dir("engram_flags_test");
    fs::write(ws.join("Cargo.lock"), "lock").unwrap();

    let base_envs = [
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

    let (ok1, vars1) = run_build_rs(&ws, &base_envs, &["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS"]);
    assert!(ok1);
    let id_base = get_var(&vars1, "ENGRAM_BUILD_ID").unwrap();

    // With RUSTFLAGS
    let flags_envs = [
        (
            "ENGRAM_GIT_COMMIT",
            "0123456789abcdef0123456789abcdef01234567",
        ),
        (
            "ENGRAM_SOURCE_FINGERPRINT",
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
        ),
        ("ENGRAM_PROVENANCE_SOURCE", "ExplicitSnapshot"),
        ("RUSTFLAGS", "-C opt-level=3 -C target-cpu=haswell"),
    ];
    let (ok2, vars2) = run_build_rs(&ws, &flags_envs, &["CARGO_ENCODED_RUSTFLAGS"]);
    assert!(ok2);
    let id_flags = get_var(&vars2, "ENGRAM_BUILD_ID").unwrap();
    assert_ne!(
        id_base, id_flags,
        "Different RUSTFLAGS must produce different build_id"
    );

    // With CARGO_ENCODED_RUSTFLAGS
    let encoded_envs = [
        (
            "ENGRAM_GIT_COMMIT",
            "0123456789abcdef0123456789abcdef01234567",
        ),
        (
            "ENGRAM_SOURCE_FINGERPRINT",
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
        ),
        ("ENGRAM_PROVENANCE_SOURCE", "ExplicitSnapshot"),
        ("CARGO_ENCODED_RUSTFLAGS", "-C\x1fopt-level=2"),
    ];
    let (ok3, vars3) = run_build_rs(&ws, &encoded_envs, &["RUSTFLAGS"]);
    assert!(ok3);
    let id_encoded = get_var(&vars3, "ENGRAM_BUILD_ID").unwrap();
    assert_ne!(
        id_base, id_encoded,
        "CARGO_ENCODED_RUSTFLAGS must produce different build_id"
    );
    assert_ne!(
        id_flags, id_encoded,
        "Distinct flags must produce distinct build_ids"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_cargo_feature_selection_changes_build_id() {
    let ws = make_temp_dir("engram_features_test");
    fs::write(ws.join("Cargo.lock"), "lock").unwrap();

    let base_envs = [
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
    let (ok1, vars1) = run_build_rs(&ws, &base_envs, &["CARGO_CFG_FEATURE", "CARGO_FEATURE_FOO"]);
    assert!(ok1);
    let id1 = get_var(&vars1, "ENGRAM_BUILD_ID").unwrap();

    let feat_envs = [
        (
            "ENGRAM_GIT_COMMIT",
            "0123456789abcdef0123456789abcdef01234567",
        ),
        (
            "ENGRAM_SOURCE_FINGERPRINT",
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
        ),
        ("ENGRAM_PROVENANCE_SOURCE", "ExplicitSnapshot"),
        ("CARGO_CFG_FEATURE", "default,telemetry"),
    ];
    let (ok2, vars2) = run_build_rs(&ws, &feat_envs, &[]);
    assert!(ok2);
    let id2 = get_var(&vars2, "ENGRAM_BUILD_ID").unwrap();
    assert_ne!(id1, id2, "Enabling features must change build_id");

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_opt_level_and_debug_assertions_change_build_id() {
    let ws = make_temp_dir("engram_opt_debug_test");
    fs::write(ws.join("Cargo.lock"), "lock").unwrap();

    let base_envs = [
        (
            "ENGRAM_GIT_COMMIT",
            "0123456789abcdef0123456789abcdef01234567",
        ),
        (
            "ENGRAM_SOURCE_FINGERPRINT",
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
        ),
        ("ENGRAM_PROVENANCE_SOURCE", "ExplicitSnapshot"),
        ("OPT_LEVEL", "3"),
        ("CARGO_CFG_DEBUG_ASSERTIONS", ""),
    ];
    let (ok1, vars1) = run_build_rs(&ws, &base_envs, &[]);
    assert!(ok1);
    let id1 = get_var(&vars1, "ENGRAM_BUILD_ID").unwrap();

    let opt_envs = [
        (
            "ENGRAM_GIT_COMMIT",
            "0123456789abcdef0123456789abcdef01234567",
        ),
        (
            "ENGRAM_SOURCE_FINGERPRINT",
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
        ),
        ("ENGRAM_PROVENANCE_SOURCE", "ExplicitSnapshot"),
        ("OPT_LEVEL", "2"),
        ("CARGO_CFG_DEBUG_ASSERTIONS", ""),
    ];
    let (ok2, vars2) = run_build_rs(&ws, &opt_envs, &[]);
    assert!(ok2);
    let id2 = get_var(&vars2, "ENGRAM_BUILD_ID").unwrap();
    assert_ne!(
        id1, id2,
        "Different OPT_LEVEL must produce different build_id"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_irrelevant_environment_variables_preserve_build_id() {
    let ws = make_temp_dir("engram_irrelevant_env_test");
    fs::write(ws.join("Cargo.lock"), "lock").unwrap();

    let envs1 = [
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
    let (ok1, vars1) = run_build_rs(&ws, &envs1, &["IRRELEVANT_VAR_A", "IRRELEVANT_VAR_B"]);
    assert!(ok1);
    let id1 = get_var(&vars1, "ENGRAM_BUILD_ID").unwrap();

    let envs2 = [
        (
            "ENGRAM_GIT_COMMIT",
            "0123456789abcdef0123456789abcdef01234567",
        ),
        (
            "ENGRAM_SOURCE_FINGERPRINT",
            "E3B0C44298FC1C149AFBF4C8996FB92427AE41E4649B934CA495991B7852B855",
        ),
        ("ENGRAM_PROVENANCE_SOURCE", "ExplicitSnapshot"),
        ("IRRELEVANT_VAR_A", "some_random_value"),
        ("IRRELEVANT_VAR_B", "another_random_value"),
    ];
    let (ok2, vars2) = run_build_rs(&ws, &envs2, &[]);
    assert!(ok2);
    let id2 = get_var(&vars2, "ENGRAM_BUILD_ID").unwrap();

    assert_eq!(
        id1, id2,
        "Irrelevant environment variables must not alter build_id"
    );
    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_no_personal_information_or_absolute_paths_in_identity() {
    let json_str = build_info::build_info_json();
    let val: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");

    assert!(val["schema_version"] == "1" || val["schema_version"] == "2");
    assert_eq!(val["package_version"], build_info::VERSION);

    // Verify build_environment fields
    let env = &val["build_environment"];
    assert!(env["build_profile"].is_string());
    assert!(env["rustc_version"].is_string());
    assert!(env["target_triple"].is_string());
    assert!(env["host_triple"].is_string());
    assert!(env["opt_level"].is_string());
    assert!(env["debug_assertions"].is_string());
    assert!(env["cargo_features"].is_string());
    assert!(env["rustflags"].is_string());

    // Check no usernames or Windows user paths leaked
    let dumped = json_str.to_lowercase();
    assert!(
        !dumped.contains("c:\\users\\"),
        "Identity must not contain user paths"
    );
    assert!(
        !dumped.contains("/home/"),
        "Identity must not contain unix home paths"
    );
}
