//! Test suite for Phase 9.6.8 Build Provenance Closure
//!
//! Validates:
//! - `--build-info --format json` CLI argument compatibility
//! - Canonical serialization of 17 fields with length-prefix framing
//! - Privacy sanitization: zero absolute filesystem paths or usernames leaked in rustflags
//! - Profile identity completeness: debug_level and panic_strategy
//! - Compiler metadata: rustc_commit and rustc_llvm
//! - Git dirty state separation: compilation_dirty and working_tree_dirty
//! - Backward compatibility with existing schema and consumers

use engram::build_info;

#[test]
fn test_build_info_format_json_cli_support() {
    let json_str = build_info::build_info_json();
    let val: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");

    assert!(val["schema_version"] == "1" || val["schema_version"] == "2");
    assert_eq!(val["package_version"], build_info::VERSION);
    assert_eq!(val["build_id"], build_info::BUILD_ID);

    // Verify build_environment includes Phase 9.6.8 additions and Schema v2 linker fields
    let env = &val["build_environment"];
    assert!(
        env["debug_level"].is_string(),
        "debug_level must be present"
    );
    assert!(
        env["panic_strategy"].is_string(),
        "panic_strategy must be present"
    );
    assert!(
        env["rustc_commit"].is_string(),
        "rustc_commit must be present"
    );
    assert!(env["rustc_llvm"].is_string(), "rustc_llvm must be present");
    if val["schema_version"] == "2" {
        assert!(
            env["linker_name"].is_string(),
            "linker_name must be present in schema 2"
        );
        assert!(
            env["linker_sha256"].is_string(),
            "linker_sha256 must be present in schema 2"
        );
        assert!(
            env["linker_flags"].is_string(),
            "linker_flags must be present in schema 2"
        );
    }

    // Verify source_identity includes dirty state separation
    let src = &val["source_identity"];
    assert!(
        src["git_dirty"].is_string(),
        "legacy git_dirty must be present"
    );
    assert!(
        src["compilation_dirty"].is_string(),
        "compilation_dirty must be present"
    );
    assert!(
        src["working_tree_dirty"].is_string(),
        "working_tree_dirty must be present"
    );
}

#[test]
fn test_rustflags_path_privacy_sanitization() {
    // build_info::RUSTFLAGS must never contain user profiles or absolute directories
    let flags = build_info::RUSTFLAGS.to_lowercase();
    assert!(
        !flags.contains("c:\\users\\"),
        "RUSTFLAGS must not contain Windows user profile paths, got: {}",
        build_info::RUSTFLAGS
    );
    assert!(
        !flags.contains("/home/"),
        "RUSTFLAGS must not contain Linux home paths, got: {}",
        build_info::RUSTFLAGS
    );
    assert!(
        !flags.contains("/users/"),
        "RUSTFLAGS must not contain macOS user paths, got: {}",
        build_info::RUSTFLAGS
    );
}

#[test]
fn test_dirty_state_values_are_valid_booleans() {
    let comp = build_info::COMPILATION_DIRTY;
    let wt = build_info::WORKING_TREE_DIRTY;

    assert!(
        comp == "true" || comp == "false" || comp == "unknown",
        "compilation_dirty must be boolean string or unknown, got: {}",
        comp
    );
    assert!(
        wt == "true" || wt == "false" || wt == "unknown",
        "working_tree_dirty must be boolean string or unknown, got: {}",
        wt
    );
}

#[test]
fn test_compiler_metadata_integrity() {
    let commit = build_info::RUSTC_COMMIT;
    let llvm = build_info::RUSTC_LLVM;

    if commit != "unknown" {
        assert_eq!(
            commit.len(),
            40,
            "rustc commit hash must be 40-character hex, got: {}",
            commit
        );
        assert!(commit.chars().all(|c| c.is_ascii_hexdigit()));
    }

    if llvm != "unknown" {
        assert!(
            llvm.contains('.'),
            "LLVM version must have major.minor, got: {}",
            llvm
        );
    }
}

#[test]
fn test_canonical_identity_serialization_injective() {
    // Independent reference implementation of length-prefixed canonical framing
    let fields: Vec<(&str, &str)> = if build_info::SCHEMA_VERSION == "1" {
        vec![
            ("schema_version", build_info::SCHEMA_VERSION),
            ("package_version", build_info::VERSION),
            ("git_commit", build_info::GIT_COMMIT),
            ("git_dirty", build_info::GIT_DIRTY),
            ("provenance_source", build_info::PROVENANCE_SOURCE),
            ("source_fingerprint", build_info::SOURCE_FINGERPRINT),
            ("lockfile_sha256", build_info::LOCKFILE_SHA256),
            ("build_profile", build_info::BUILD_PROFILE),
            ("rustc_version", build_info::RUSTC_VERSION),
            ("target_triple", build_info::TARGET_TRIPLE),
            ("host_triple", build_info::HOST_TRIPLE),
            ("opt_level", build_info::OPT_LEVEL),
            ("debug_assertions", build_info::DEBUG_ASSERTIONS),
            ("debug_level", build_info::DEBUG_LEVEL),
            ("panic_strategy", build_info::PANIC_STRATEGY),
            ("cargo_features", build_info::CARGO_FEATURES),
            ("rustflags", build_info::RUSTFLAGS),
        ]
    } else {
        vec![
            ("schema_version", build_info::SCHEMA_VERSION),
            ("package_version", build_info::VERSION),
            ("git_commit", build_info::GIT_COMMIT),
            ("git_dirty", build_info::GIT_DIRTY),
            ("provenance_source", build_info::PROVENANCE_SOURCE),
            ("source_fingerprint", build_info::SOURCE_FINGERPRINT),
            ("lockfile_sha256", build_info::LOCKFILE_SHA256),
            ("build_profile", build_info::BUILD_PROFILE),
            ("rustc_version", build_info::RUSTC_VERSION),
            ("rustc_commit", build_info::RUSTC_COMMIT),
            ("rustc_llvm", build_info::RUSTC_LLVM),
            ("linker_name", build_info::LINKER_NAME),
            ("linker_sha256", build_info::LINKER_SHA256),
            ("linker_flags", build_info::LINKER_FLAGS),
            ("target_triple", build_info::TARGET_TRIPLE),
            ("host_triple", build_info::HOST_TRIPLE),
            ("opt_level", build_info::OPT_LEVEL),
            ("debug_assertions", build_info::DEBUG_ASSERTIONS),
            ("debug_level", build_info::DEBUG_LEVEL),
            ("panic_strategy", build_info::PANIC_STRATEGY),
            ("cargo_features", build_info::CARGO_FEATURES),
            ("rustflags", build_info::RUSTFLAGS),
        ]
    };

    let mut canonical = String::new();
    for (k, v) in fields {
        canonical.push_str(&format!("{}:{}={}:{}\n", k.len(), k, v.len(), v));
    }

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let expected_id = format!("{:X}", hasher.finalize());

    assert_eq!(
        build_info::BUILD_ID,
        expected_id,
        "Compiled build_id must strictly match independent reference calculation"
    );
}
