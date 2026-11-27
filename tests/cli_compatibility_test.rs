//! Test suite for CLI backward compatibility and exact output positions (DEFECT-964-04)
use engram::build_info;

#[test]
fn test_cli_version_full_legacy_positions_preserved() {
    let full = build_info::VERSION_FULL;
    let lines: Vec<&str> = full
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    assert!(
        lines.len() >= 8,
        "VERSION_FULL must contain at least 8 lines, got: {}",
        lines.len()
    );

    // Position 0: package version
    assert_eq!(
        lines[0],
        env!("CARGO_PKG_VERSION"),
        "Line 0 must be package version"
    );

    // Position 1: commit:
    assert!(
        lines[1].starts_with("commit: "),
        "Line 1 must be commit, got: {}",
        lines[1]
    );

    // Position 2: dirty:
    assert!(
        lines[2].starts_with("dirty: "),
        "Line 2 must be dirty, got: {}",
        lines[2]
    );

    // Position 3: rustc: (RESTORED LEGACY POSITION)
    assert!(
        lines[3].starts_with("rustc: "),
        "Line 3 must be rustc, got: {}",
        lines[3]
    );

    // Position 4: profile: (RESTORED LEGACY POSITION)
    assert!(
        lines[4].starts_with("profile: "),
        "Line 4 must be profile, got: {}",
        lines[4]
    );

    // Position 5: provenance: (EXTENDED FIELD APPENDED)
    assert!(
        lines[5].starts_with("provenance: "),
        "Line 5 must be provenance, got: {}",
        lines[5]
    );

    // Position 6: fingerprint: (EXTENDED FIELD APPENDED)
    assert!(
        lines[6].starts_with("fingerprint: "),
        "Line 6 must be fingerprint, got: {}",
        lines[6]
    );

    // Position 7: build_id: (EXTENDED FIELD APPENDED)
    assert!(
        lines[7].starts_with("build_id: "),
        "Line 7 must be build_id, got: {}",
        lines[7]
    );
}

#[test]
fn test_cli_build_info_json_schema_version() {
    let json_str = build_info::build_info_json();
    let val: serde_json::Value = serde_json::from_str(&json_str).expect("Valid JSON");

    assert!(val["schema_version"] == "1" || val["schema_version"] == "2");
    assert_eq!(val["package_version"], env!("CARGO_PKG_VERSION"));
    assert!(val["source_identity"].is_object());
    assert!(val["build_environment"].is_object());
    assert!(val["build_id"].is_string());
    assert_eq!(val["build_id"].as_str().unwrap().len(), 64);
}
