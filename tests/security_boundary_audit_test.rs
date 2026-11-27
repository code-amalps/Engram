//! Comprehensive security boundary audit integration test for Engram.
//!
//! Validates the 12 required security scenarios and Universal Secret Exclusions:
//! 1. Normal file inside workspace.
//! 2. Normal nested directory inside workspace.
//! 3. Symlink / Junction to a directory outside workspace.
//! 4. Symlink to a file outside workspace.
//! 5. Symlink / Junction loop (infinite recursion prevention).
//! 6. Symlink / Junction pointing back into workspace.
//! 7. Path containing `..`.
//! 8. Path using Windows extended verbatim prefix (`\\?\`).
//! 9. Path using mixed separators (`/` and `\`).
//! 10. Short path representation (`8.3`).
//! 11. File whose normalized path differs from input.
//! 12. Ignore negation attempting to include a path outside workspace.
//! 13. Universal secret exclusions (.env, *.pem, *.key) against lower-priority negations.

use engram::discovery::DiscoveryEngine;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
fn create_dir_junction(src: &Path, dst: &Path) -> bool {
    let output = Command::new("cmd")
        .args([
            "/c",
            "mklink",
            "/J",
            &dst.to_string_lossy(),
            &src.to_string_lossy(),
        ])
        .output();
    output.map(|o| o.status.success()).unwrap_or(false)
}

#[cfg(unix)]
fn create_dir_junction(src: &Path, dst: &Path) -> bool {
    std::os::unix::fs::symlink(src, dst).is_ok()
}

#[cfg(windows)]
fn create_file_symlink(src: &Path, dst: &Path) -> bool {
    std::os::windows::fs::symlink_file(src, dst).is_ok()
}

#[cfg(unix)]
fn create_file_symlink(src: &Path, dst: &Path) -> bool {
    std::os::unix::fs::symlink(src, dst).is_ok()
}

#[cfg(windows)]
fn get_short_path(p: &Path) -> Option<PathBuf> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = p.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut buf: Vec<u16> = vec![0; 1024];
    unsafe extern "system" {
        fn GetShortPathNameW(
            lpszLongPath: *const u16,
            lpszShortPath: *mut u16,
            cchBuffer: u32,
        ) -> u32;
    }
    unsafe {
        let len = GetShortPathNameW(wide.as_ptr(), buf.as_mut_ptr(), buf.len() as u32);
        if len > 0 && (len as usize) < buf.len() {
            use std::os::windows::ffi::OsStringExt;
            Some(PathBuf::from(std::ffi::OsString::from_wide(
                &buf[..len as usize],
            )))
        } else {
            None
        }
    }
}

#[test]
fn test_audit_c_12_security_scenarios() {
    let ws = std::env::temp_dir().join(format!("engram_sec_audit_ws_{}", uuid::Uuid::new_v4()));
    let ext_dir =
        std::env::temp_dir().join(format!("engram_sec_audit_ext_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();
    fs::create_dir_all(&ext_dir).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Security Tester"])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "sec@engram.local"])
        .current_dir(&ws)
        .output();

    // Prepare files in workspace
    let src_dir = ws.join("src");
    let nested_dir = src_dir.join("models").join("user");
    fs::create_dir_all(&nested_dir).unwrap();

    let main_rs = src_dir.join("main.rs");
    fs::write(&main_rs, "fn main() {}\n").unwrap();

    let profile_rs = nested_dir.join("profile.rs");
    fs::write(&profile_rs, "pub struct Profile;\n").unwrap();

    // Prepare files outside workspace
    let ext_file = ext_dir.join("external_secret.rs");
    fs::write(&ext_file, "pub fn secret() -> i32 { 42 }\n").unwrap();

    // Write engram.yaml with attempts to include external paths
    fs::write(
        ws.join("engram.yaml"),
        r#"
ignore:
  use_gitignore: true
  use_engramignore: true
  exclude:
    - "legacy_build/"
  include:
    - "../**"
    - "!../external_secret.rs"
"#,
    )
    .unwrap();

    // Write .engramignore with attempts to negate external paths
    fs::write(
        ws.join(".engramignore"),
        r#"
!../external_secret.rs
!/external_secret.rs
"#,
    )
    .unwrap();

    let engine = DiscoveryEngine::for_root(&ws);

    // Scenario 1: Normal file inside workspace
    let s1 = engine.explain(&main_rs, false);
    assert!(
        !s1.is_excluded,
        "Scenario 1 failed: normal file inside workspace must not be excluded"
    );

    // Scenario 2: Normal nested directory inside workspace
    let s2 = engine.explain(&nested_dir, true);
    assert!(
        !s2.is_excluded,
        "Scenario 2 failed: normal nested directory inside workspace must not be excluded"
    );

    // Scenario 3: Symlink / Junction to a directory outside workspace
    let ext_junction = ws.join("ext_junction");
    let junction_created = create_dir_junction(&ext_dir, &ext_junction);
    if junction_created {
        let s3 = engine.explain(&ext_junction, true);
        assert!(
            s3.is_excluded,
            "Scenario 3 failed: junction to external directory must be excluded"
        );
        assert_eq!(
            s3.rule_source.as_deref(),
            Some("WorkspaceBoundary"),
            "Scenario 3 rule source must be WorkspaceBoundary"
        );
    }

    // Scenario 4: Symlink to a file outside workspace
    let ext_symlink = ws.join("ext_symlink.rs");
    let symlink_created = create_file_symlink(&ext_file, &ext_symlink);
    if symlink_created {
        let s4 = engine.explain(&ext_symlink, false);
        assert!(
            s4.is_excluded,
            "Scenario 4 failed: symlink to external file must be excluded"
        );
        assert_eq!(
            s4.rule_source.as_deref(),
            Some("WorkspaceBoundary"),
            "Scenario 4 rule source must be WorkspaceBoundary"
        );
    } else {
        // In unprivileged Windows environments where symlink_file returns error 1314,
        // verify that an external file directly presented to explain() is securely rejected.
        let s4 = engine.explain(&ext_file, false);
        assert!(
            s4.is_excluded,
            "Scenario 4 fallback: direct external file must be excluded"
        );
        assert_eq!(s4.rule_source.as_deref(), Some("WorkspaceBoundary"));
    }

    // Scenario 5: Symlink / Junction loop (infinite traversal prevention)
    let loop_dir = ws.join("loop_dir");
    fs::create_dir_all(&loop_dir).unwrap();
    let loop_inner = loop_dir.join("parent_loop");
    let loop_created = create_dir_junction(&loop_dir, &loop_inner);
    if loop_created {
        // WalkBuilder has follow_links(false) so it will not infinitely recurse
        let shared_engine = DiscoveryEngine::new_shared(&ws);
        let entries: Vec<_> = shared_engine.build_walk().flatten().collect();
        assert!(
            entries.len() < 100,
            "Scenario 5 failed: directory walk must not enter infinite recursion on junction loop"
        );
    }

    // Scenario 6: Symlink / Junction pointing back into workspace
    let internal_junction = ws.join("internal_link");
    let internal_created = create_dir_junction(&src_dir, &internal_junction);
    if internal_created {
        let internal_target_file = internal_junction.join("main.rs");
        let s6 = engine.explain(&internal_target_file, false);
        assert!(
            !s6.is_excluded,
            "Scenario 6 failed: junction pointing back into workspace must resolve to valid workspace source"
        );
    }

    // Scenario 7: Path containing `..`
    // 7a: Resolving inside workspace
    let inside_dotdot = ws.join("src").join("..").join("src").join("main.rs");
    let s7a = engine.explain(&inside_dotdot, false);
    assert!(
        !s7a.is_excluded,
        "Scenario 7a failed: path with .. resolving inside workspace must be accepted"
    );

    // 7b: Resolving outside workspace
    let outside_dotdot = ws.join("..").join(ext_file.file_name().unwrap());
    let s7b = engine.explain(&outside_dotdot, false);
    assert!(
        s7b.is_excluded,
        "Scenario 7b failed: path with .. resolving outside workspace must be excluded"
    );
    assert_eq!(s7b.rule_source.as_deref(), Some("WorkspaceBoundary"));

    // Scenario 8: Path using Windows extended verbatim prefix (`\\?\`)
    let verbatim_path = PathBuf::from(format!(r"\\?\{}", main_rs.to_string_lossy()));
    let s8 = engine.explain(&verbatim_path, false);
    assert!(
        !s8.is_excluded,
        "Scenario 8 failed: verbatim prefix path inside workspace must be accepted"
    );

    // Scenario 9: Path using mixed separators (`/` and `\`)
    let mixed_str = format!("{}/src\\models/user\\profile.rs", ws.to_string_lossy());
    let mixed_path = PathBuf::from(&mixed_str);
    let s9 = engine.explain(&mixed_path, false);
    assert!(
        !s9.is_excluded,
        "Scenario 9 failed: mixed separator path inside workspace must be accepted"
    );

    // Scenario 10: Short path representation (`8.3`)
    if let Some(short_ws) = get_short_path(&main_rs) {
        let s10 = engine.explain(&short_ws, false);
        assert!(
            !s10.is_excluded,
            "Scenario 10 failed: 8.3 short path representation must be accepted and resolved"
        );
    }

    // Scenario 11: File whose normalized path differs from input (e.g. redundant `.` and `/`)
    let messy_path = ws.join("src").join(".").join("main.rs");
    let s11 = engine.explain(&messy_path, false);
    assert!(
        !s11.is_excluded,
        "Scenario 11 failed: messy non-normalized path inside workspace must be accepted"
    );

    // Scenario 12: Ignore negation attempting to include a path outside workspace
    let outside_target = ext_file;
    let s12 = engine.explain(&outside_target, false);
    assert!(
        s12.is_excluded,
        "Scenario 12 failed: ignore negation MUST NEVER include paths outside workspace"
    );
    assert_eq!(
        s12.rule_source.as_deref(),
        Some("WorkspaceBoundary"),
        "Scenario 12 rule source must remain WorkspaceBoundary"
    );

    // Cleanup temporary directories
    let _ = fs::remove_dir_all(&ws);
    let _ = fs::remove_dir_all(&ext_dir);
}

#[test]
fn test_audit_c_universal_secret_exclusions_against_negations() {
    let ws = std::env::temp_dir().join(format!("engram_sec_secrets_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Security Tester"])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "sec@engram.local"])
        .current_dir(&ws)
        .output();

    // Attempt to whitelist secrets at all levels
    fs::write(
        ws.join("engram.yaml"),
        r#"
ignore:
  use_gitignore: true
  use_engramignore: true
  include:
    - ".env"
    - ".env.production"
    - "*.pem"
    - "*.key"
    - "secrets/**"
"#,
    )
    .unwrap();

    fs::write(
        ws.join(".engramignore"),
        r#"
!.env
!.env.production
!*.pem
!*.key
"#,
    )
    .unwrap();

    // Create secret files and template files
    let env_file = ws.join(".env");
    fs::write(&env_file, "SECRET_KEY=123456\n").unwrap();

    let env_prod_file = ws.join(".env.production");
    fs::write(&env_prod_file, "PROD_SECRET=abcdef\n").unwrap();

    let pem_file = ws.join("server.pem");
    fs::write(&pem_file, "-----BEGIN CERTIFICATE-----\n").unwrap();

    let key_file = ws.join("private.key");
    fs::write(&key_file, "-----BEGIN PRIVATE KEY-----\n").unwrap();

    // Allowed template files
    let env_example = ws.join(".env.example");
    fs::write(&env_example, "SECRET_KEY=placeholder\n").unwrap();

    let env_sample = ws.join(".env.sample");
    fs::write(&env_sample, "SECRET_KEY=placeholder\n").unwrap();

    let env_template = ws.join(".env.template");
    fs::write(&env_template, "SECRET_KEY=placeholder\n").unwrap();

    let engine = DiscoveryEngine::for_root(&ws);

    // 1. .env must be excluded as SecretFile despite whitelists
    let d_env = engine.explain(&env_file, false);
    assert!(
        d_env.is_excluded,
        ".env must be excluded despite explicit include in engram.yaml and .engramignore"
    );
    assert_eq!(
        d_env.rule_source.as_deref(),
        Some("SecretFile"),
        "rule_source for .env must be SecretFile"
    );

    // 2. .env.production must be excluded as SecretFile
    let d_env_prod = engine.explain(&env_prod_file, false);
    assert!(
        d_env_prod.is_excluded,
        ".env.production must be excluded despite explicit include"
    );
    assert_eq!(d_env_prod.rule_source.as_deref(), Some("SecretFile"));

    // 3. server.pem must be excluded as SecretFile
    let d_pem = engine.explain(&pem_file, false);
    assert!(
        d_pem.is_excluded,
        "server.pem must be excluded despite explicit include"
    );
    assert_eq!(d_pem.rule_source.as_deref(), Some("SecretFile"));

    // 4. private.key must be excluded as SecretFile
    let d_key = engine.explain(&key_file, false);
    assert!(
        d_key.is_excluded,
        "private.key must be excluded despite explicit include"
    );
    assert_eq!(d_key.rule_source.as_deref(), Some("SecretFile"));

    // 5. Allowed templates: .env.example, .env.sample, .env.template must NOT be excluded as SecretFile
    let d_example = engine.explain(&env_example, false);
    assert!(
        !d_example.is_excluded,
        ".env.example must not be excluded as SecretFile"
    );

    let d_sample = engine.explain(&env_sample, false);
    assert!(
        !d_sample.is_excluded,
        ".env.sample must not be excluded as SecretFile"
    );

    let d_template = engine.explain(&env_template, false);
    assert!(
        !d_template.is_excluded,
        ".env.template must not be excluded as SecretFile"
    );

    let _ = fs::remove_dir_all(&ws);
}
