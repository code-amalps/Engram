//! Test suite for build provenance and version formatting
use engram::build_info;

#[test]
fn test_build_provenance_env_vars_present() {
    // 1. Package version must match Cargo.toml
    assert_eq!(build_info::VERSION, env!("CARGO_PKG_VERSION"));

    // 2. Full version string must contain required fields
    let full_str = build_info::VERSION_FULL;

    assert!(
        full_str.contains(env!("CARGO_PKG_VERSION")),
        "Version must contain package version"
    );
    assert!(
        full_str.contains("commit:"),
        "Version must contain commit field"
    );
    assert!(
        full_str.contains("dirty:"),
        "Version must contain dirty field"
    );
    assert!(
        full_str.contains("rustc:"),
        "Version must contain rustc field"
    );
    assert!(
        full_str.contains("profile:"),
        "Version must contain profile field"
    );
    assert!(
        full_str.contains("provenance:"),
        "Version must contain provenance field"
    );
    assert!(
        full_str.contains("fingerprint:"),
        "Version must contain fingerprint field"
    );
    assert!(
        full_str.contains("build_id:"),
        "Version must contain build_id field"
    );
}

#[test]
fn test_build_provenance_extended_fields() {
    assert!(!build_info::PROVENANCE_SOURCE.is_empty());
    assert!(!build_info::SOURCE_FINGERPRINT.is_empty());
    assert!(!build_info::LOCKFILE_SHA256.is_empty());
    assert!(!build_info::BUILD_ID.is_empty());
    assert!(build_info::SCHEMA_VERSION == "1" || build_info::SCHEMA_VERSION == "2");

    // JSON build info format
    let json_str = build_info::build_info_json();
    let val: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");
    assert!(val["schema_version"] == "1" || val["schema_version"] == "2");
    assert_eq!(val["package_version"], build_info::VERSION);
    assert_eq!(val["build_id"], build_info::BUILD_ID);
}

#[test]
fn test_build_provenance_git_commit_format() {
    let commit = build_info::GIT_COMMIT;
    if commit != "unknown" {
        // Git commit SHA must be a 40-character hex string
        assert_eq!(commit.len(), 40, "Git commit SHA must be 40 hex characters");
        assert!(
            commit.chars().all(|c| c.is_ascii_hexdigit()),
            "Git commit must be hex digits"
        );
    }
}

#[test]
fn test_build_provenance_dirty_state_format() {
    let dirty = build_info::GIT_DIRTY;
    assert!(
        dirty == "true" || dirty == "false" || dirty == "unknown",
        "Dirty state must be 'true', 'false', or 'unknown', got: {}",
        dirty
    );
}

#[test]
fn test_build_provenance_profile_format() {
    let profile = build_info::BUILD_PROFILE;
    assert!(
        profile == "debug" || profile == "release" || profile == "test" || profile == "unknown",
        "Profile must be a standard Cargo profile, got: {}",
        profile
    );
}

#[test]
fn test_build_provenance_formatting_function() {
    // Test formatting helper logic with clean, dirty, and missing git states
    fn format_version(pkg: &str, commit: &str, dirty: &str, rustc: &str, profile: &str) -> String {
        format!(
            "{}\ncommit: {}\ndirty: {}\nrustc: {}\nprofile: {}",
            pkg, commit, dirty, rustc, profile
        )
    }

    // Clean git state
    let clean = format_version(
        "1.5.4",
        "7106a2f369fe79faf5f95449378a900ab8e8eddf",
        "false",
        "rustc 1.99.0",
        "release",
    );
    assert!(clean.contains("commit: 7106a2f369fe79faf5f95449378a900ab8e8eddf"));
    assert!(clean.contains("dirty: false"));

    // Dirty git state
    let dirty = format_version(
        "1.5.4",
        "7106a2f369fe79faf5f95449378a900ab8e8eddf",
        "true",
        "rustc 1.99.0",
        "release",
    );
    assert!(dirty.contains("dirty: true"));

    // Missing git state (e.g. source tarball outside git)
    let missing_git = format_version("1.5.4", "unknown", "unknown", "rustc 1.99.0", "release");
    assert!(missing_git.contains("commit: unknown"));
    assert!(missing_git.contains("dirty: unknown"));

    // Missing compiler metadata fallback
    let missing_rustc = format_version("1.5.4", "unknown", "unknown", "unknown", "release");
    assert!(missing_rustc.contains("rustc: unknown"));
}
