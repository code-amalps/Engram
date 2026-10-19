use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn setup_test_git_repo(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "engram_hook_test_{}_{}",
        name,
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&p).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&p).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Hook Tester"])
        .current_dir(&p)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "hook@engram.local"])
        .current_dir(&p)
        .output();

    // Initial commit
    fs::write(p.join("README.md"), "# Initial\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&p)
        .output();
    let _ = Command::new("git")
        .args(["commit", "-m", "Initial"])
        .current_dir(&p)
        .output();

    p
}

#[test]
fn test_hook_clean_authorized_commit() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("clean_commit");

    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .unwrap();

    // Create contract expecting src/app.ts
    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let app_file = src_dir.join("app.ts");
    fs::write(&app_file, "export const app = 1;\n").unwrap();

    let contract_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "contract",
            "create",
            "Add app feature",
            "--files",
            "src/app.ts",
        ])
        .output()
        .unwrap();
    assert!(contract_out.status.success());

    // Stage authorized file
    let _ = Command::new("git")
        .args(["add", "src/app.ts"])
        .current_dir(&ws)
        .output();

    // Execute engram hook run
    let hook_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "run"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&hook_out.stdout);
    assert!(
        hook_out.status.success(),
        "Clean commit must exit 0: {stdout}"
    );
    assert!(stdout.contains("Staged changes authorized"));
    assert!(stdout.contains("Active Contract"));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_blocks_unauthorized_boundary_expansion() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("boundary_violation");

    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .unwrap();

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("app.ts"), "export const app = 1;\n").unwrap();
    fs::write(src_dir.join("unauthorized.ts"), "export const rogue = 2;\n").unwrap();

    // Contract authorizes only src/app.ts
    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["contract", "create", "Scoped task", "--files", "src/app.ts"])
        .output()
        .unwrap();

    // Stage both authorized file and unauthorized file
    let _ = Command::new("git")
        .args(["add", "src/app.ts", "src/unauthorized.ts"])
        .current_dir(&ws)
        .output();

    // Run hook in strict mode
    let hook_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "run", "--strict"])
        .output()
        .unwrap();

    assert!(
        !hook_out.status.success(),
        "Unauthorized expansion must fail"
    );
    assert_eq!(hook_out.status.code(), Some(1));

    let stderr = String::from_utf8_lossy(&hook_out.stderr);
    assert!(stderr.contains("COMMIT BLOCKED"));
    assert!(stderr.contains("src/unauthorized.ts"));
    assert!(stderr.contains("Actionable Remediation"));
    assert!(stderr.contains("git restore --staged"));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_blocks_forbidden_path_secrets() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("forbidden_secrets");

    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .unwrap();

    // Add constitution forbidding secrets/**
    let engram_dir = ws.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    fs::write(
        engram_dir.join("constitution.yaml"),
        "schema_version: 1\nrules:\n  - id: \"RULE-NO-SECRETS\"\n    name: \"No secrets rule\"\n    type: PATTERN\n    forbidden: \"secrets/**\"\n    description: \"Secrets must never be committed\"\n",
    )
    .unwrap();

    // Stage a secret file
    let sec_dir = ws.join("secrets");
    fs::create_dir_all(&sec_dir).unwrap();
    fs::write(sec_dir.join("token.key"), "SUPER_SECRET_KEY\n").unwrap();

    let _ = Command::new("git")
        .args(["add", "secrets/token.key"])
        .current_dir(&ws)
        .output();

    let hook_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "run"])
        .output()
        .unwrap();

    assert!(
        !hook_out.status.success(),
        "Forbidden secrets must block commit"
    );
    let stderr = String::from_utf8_lossy(&hook_out.stderr);
    assert!(stderr.contains("COMMIT BLOCKED"));
    assert!(stderr.contains("secrets/token.key"));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_partial_staging_ignores_unstaged_violations() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("partial_staging");

    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .unwrap();

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("allowed.ts"), "export const allowed = 1;\n").unwrap();
    fs::write(
        src_dir.join("forbidden.ts"),
        "export const forbidden = 2;\n",
    )
    .unwrap();

    // Contract expects only allowed.ts
    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "contract",
            "create",
            "Scoped task",
            "--files",
            "src/allowed.ts",
        ])
        .output()
        .unwrap();

    // STAGE ONLY allowed.ts; forbidden.ts remains unstaged in working tree
    let _ = Command::new("git")
        .args(["add", "src/allowed.ts"])
        .current_dir(&ws)
        .output();

    // Hook run evaluates staged index ONLY
    let hook_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "run", "--strict"])
        .output()
        .unwrap();

    assert!(
        hook_out.status.success(),
        "Partial staging: unstaged forbidden file must NOT block staged authorized commit"
    );
    let stdout = String::from_utf8_lossy(&hook_out.stdout);
    assert!(stdout.contains("Staged changes authorized"));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_filenames_with_spaces() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("spaces_filename");

    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .unwrap();

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let space_file = src_dir.join("my cool component.ts");
    fs::write(&space_file, "export const Cool = true;\n").unwrap();

    // Contract authorizes file with spaces
    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "contract",
            "create",
            "Component with spaces",
            "--files",
            "src/my cool component.ts",
        ])
        .output()
        .unwrap();

    let _ = Command::new("git")
        .args(["add", "src/my cool component.ts"])
        .current_dir(&ws)
        .output();

    let hook_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "run"])
        .output()
        .unwrap();

    assert!(
        hook_out.status.success(),
        "File with spaces must be handled cleanly"
    );
    let stdout = String::from_utf8_lossy(&hook_out.stdout);
    assert!(stdout.contains("Staged changes authorized"));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_handles_deleted_files() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("deleted_files");

    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .unwrap();

    // Commit file first
    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let del_file = src_dir.join("old_code.ts");
    fs::write(&del_file, "export const old = true;\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "src/old_code.ts"])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["commit", "-m", "Add old code"])
        .current_dir(&ws)
        .output();

    // Create contract expecting old_code.ts modification/deletion
    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "contract",
            "create",
            "Deprecate old code",
            "--files",
            "src/old_code.ts",
        ])
        .output()
        .unwrap();

    // Delete file and stage deletion
    let _ = Command::new("git")
        .args(["rm", "src/old_code.ts"])
        .current_dir(&ws)
        .output();

    let hook_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "run"])
        .output()
        .unwrap();

    assert!(
        hook_out.status.success(),
        "Staged deletion must pass when in contract"
    );
    let stdout = String::from_utf8_lossy(&hook_out.stdout);
    assert!(stdout.contains("Staged changes authorized"));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_install_uninstall_and_dry_run_cli() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("cli_install");

    let hook_path = ws.join(".git").join("hooks").join("pre-commit");

    // 1. Dry run install
    let dry_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "install", "--dry-run"])
        .output()
        .unwrap();
    assert!(dry_out.status.success());
    assert!(!hook_path.exists());

    // 2. Real install
    let inst_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "install"])
        .output()
        .unwrap();
    assert!(inst_out.status.success());
    assert!(hook_path.exists());

    let content = fs::read_to_string(&hook_path).unwrap();
    assert!(content.contains("BEGIN ENGRAM CHANGE GUARD"));
    assert!(content.contains("hook run"));

    // 3. Uninstall
    let uninst_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "uninstall"])
        .output()
        .unwrap();
    assert!(uninst_out.status.success());
    assert!(!hook_path.exists());

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_script_explicit_engram_path_valid() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("engram_path_valid");

    let inst_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "install"])
        .output()
        .unwrap();
    assert!(inst_out.status.success());

    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .unwrap();
    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("app.ts"), "export const a = 1;").unwrap();
    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["contract", "create", "Scoped", "--files", "src/app.ts"])
        .output()
        .unwrap();

    let _ = Command::new("git")
        .args(["add", "src/app.ts"])
        .current_dir(&ws)
        .output();

    let commit_out = Command::new("git")
        .args(["commit", "-m", "Valid ENGRAM_PATH commit"])
        .env("ENGRAM_PATH", bin)
        .current_dir(&ws)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&commit_out.stdout);
    let stderr = String::from_utf8_lossy(&commit_out.stderr);
    assert!(
        commit_out.status.success(),
        "Commit with valid ENGRAM_PATH must succeed: stdout={stdout}, stderr={stderr}"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_script_explicit_engram_path_invalid_fails_closed() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("engram_path_invalid");

    let inst_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "install"])
        .output()
        .unwrap();
    assert!(inst_out.status.success());

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("app.ts"), "export const a = 1;").unwrap();
    let _ = Command::new("git")
        .args(["add", "src/app.ts"])
        .current_dir(&ws)
        .output();

    let commit_out = Command::new("git")
        .args(["commit", "-m", "Invalid ENGRAM_PATH commit"])
        .env("ENGRAM_PATH", "/nonexistent/path/to/engram_invalid_exe")
        .current_dir(&ws)
        .output()
        .unwrap();

    assert!(!commit_out.status.success(), "Commit must fail with invalid ENGRAM_PATH");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&commit_out.stdout),
        String::from_utf8_lossy(&commit_out.stderr)
    );
    assert!(
        combined.contains("Configured ENGRAM_PATH"),
        "Must output diagnostic mentioning ENGRAM_PATH: {combined}"
    );
    assert!(
        combined.contains("--no-verify"),
        "Must mention --no-verify bypass: {combined}"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_script_explicit_engram_path_incompatible_fails_closed() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("engram_path_incompatible");

    let inst_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "install"])
        .output()
        .unwrap();
    assert!(inst_out.status.success());

    let mock_bin = ws.join("mock_old_engram.sh");
    fs::write(
        &mock_bin,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"engram 1.5.0\"; exit 0; fi\necho \"error: unrecognized subcommand '$1'\" >&2\nexit 1\n",
    )
    .unwrap();

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("app.ts"), "export const a = 1;").unwrap();
    let _ = Command::new("git")
        .args(["add", "src/app.ts"])
        .current_dir(&ws)
        .output();

    let mock_bin_str = mock_bin.to_string_lossy().replace('\\', "/");
    let commit_out = Command::new("git")
        .args(["commit", "-m", "Incompatible ENGRAM_PATH commit"])
        .env("ENGRAM_PATH", &mock_bin_str)
        .current_dir(&ws)
        .output()
        .unwrap();

    assert!(!commit_out.status.success(), "Commit must fail with incompatible ENGRAM_PATH");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&commit_out.stdout),
        String::from_utf8_lossy(&commit_out.stderr)
    );
    assert!(
        combined.contains("does not support the required 'hook' command"),
        "Must output diagnostic explaining hook subcommand is missing: {combined}"
    );
    assert!(
        combined.contains("--no-verify"),
        "Must mention --no-verify: {combined}"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_script_prefers_recorded_installer_binary() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("recorded_installer");

    let inst_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "install"])
        .output()
        .unwrap();
    assert!(inst_out.status.success());

    let hook_path = ws.join(".git").join("hooks").join("pre-commit");
    let hook_content = fs::read_to_string(&hook_path).unwrap();
    assert!(
        hook_content.contains("INSTALLER_BIN="),
        "Hook must declare INSTALLER_BIN: {hook_content}"
    );

    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .unwrap();
    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("app.ts"), "export const ok = true;").unwrap();
    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["contract", "create", "Add ok", "--files", "src/app.ts"])
        .output()
        .unwrap();

    let _ = Command::new("git")
        .args(["add", "src/app.ts"])
        .current_dir(&ws)
        .output();

    let commit_out = Command::new("git")
        .args(["commit", "-m", "Commit with installed hook"])
        .current_dir(&ws)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&commit_out.stdout);
    let stderr = String::from_utf8_lossy(&commit_out.stderr);
    assert!(
        commit_out.status.success(),
        "Git commit must succeed using recorded installer: stdout={stdout}, stderr={stderr}"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_executes_from_repository_subdirectory() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("subfolder_commit");

    let inst_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["hook", "install"])
        .output()
        .unwrap();
    assert!(inst_out.status.success());

    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .unwrap();

    let sub_dir = ws.join("nested").join("subpkg");
    fs::create_dir_all(&sub_dir).unwrap();
    let nested_file = sub_dir.join("sub.ts");
    fs::write(&nested_file, "export const sub = true;\n").unwrap();

    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["contract", "create", "Nested task", "--files", "nested/subpkg/sub.ts"])
        .output()
        .unwrap();

    let _ = Command::new("git")
        .args(["add", "nested/subpkg/sub.ts"])
        .current_dir(&ws)
        .output();

    let commit_out = Command::new("git")
        .args(["commit", "-m", "Commit from subdirectory"])
        .current_dir(&sub_dir)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&commit_out.stdout);
    let stderr = String::from_utf8_lossy(&commit_out.stderr);
    assert!(
        commit_out.status.success(),
        "Subdirectory commit must resolve REPO_ROOT and succeed: stdout={stdout}, stderr={stderr}"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_prefers_repository_target_over_outdated_path() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_git_repo("repo_target_over_path");

    let hooks_dir = ws.join(".git").join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    let hook_path = hooks_dir.join("pre-commit");
    let script = format!("#!/bin/sh\n{}\n", engram::hook::HOOK_SCRIPT_BODY);
    fs::write(&hook_path, &script).unwrap();

    let target_debug = ws.join("target").join("debug");
    fs::create_dir_all(&target_debug).unwrap();
    let target_bin = target_debug.join(if cfg!(windows) { "engram.exe" } else { "engram" });
    fs::copy(bin, &target_bin).unwrap();

    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .unwrap();

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("app.ts"), "export const ok = 1;").unwrap();
    let _ = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["contract", "create", "Scoped", "--files", "src/app.ts"])
        .output()
        .unwrap();

    let _ = Command::new("git")
        .args(["add", "src/app.ts"])
        .current_dir(&ws)
        .output();

    let commit_out = Command::new("git")
        .args(["commit", "-m", "Commit using repo target"])
        .env_remove("ENGRAM_PATH")
        .current_dir(&ws)
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&commit_out.stdout);
    let stderr = String::from_utf8_lossy(&commit_out.stderr);
    assert!(
        commit_out.status.success(),
        "Must find repo target and succeed: stdout={stdout}, stderr={stderr}"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_hook_outdated_binary_without_fallback_diagnostics() {
    let ws = setup_test_git_repo("outdated_diag");

    let hooks_dir = ws.join(".git").join("hooks");
    fs::create_dir_all(&hooks_dir).unwrap();
    let hook_path = hooks_dir.join("pre-commit");
    let script = format!("#!/bin/sh\n{}\n", engram::hook::HOOK_SCRIPT_BODY);
    fs::write(&hook_path, &script).unwrap();

    let mock_bin_dir = ws.join("mock_bin");
    fs::create_dir_all(&mock_bin_dir).unwrap();
    let mock_engram = mock_bin_dir.join("engram");
    fs::write(
        &mock_engram,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"engram 1.4.0\"; exit 0; fi\necho \"error: unrecognized subcommand '$1'\" >&2\nexit 1\n",
    )
    .unwrap();

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("test.ts"), "export const t = 1;").unwrap();
    let _ = Command::new("git")
        .args(["add", "src/test.ts"])
        .current_dir(&ws)
        .output();

    // Filter out any target or scratch directories from PATH so only outdated binary is on PATH
    let path_var = std::env::var("PATH").unwrap_or_default();
    let filtered: Vec<_> = std::env::split_paths(&path_var)
        .filter(|p| {
            let s = p.to_string_lossy().to_lowercase();
            !s.contains("target\\debug")
                && !s.contains("target/debug")
                && !s.contains("target\\release")
                && !s.contains("target/release")
                && !s.contains("test_clean_install")
                && !s.contains("smoke_test")
        })
        .collect();
    let mut new_paths = vec![mock_bin_dir.clone()];
    new_paths.extend(filtered);
    let new_path = std::env::join_paths(new_paths).unwrap();

    let commit_out = Command::new("git")
        .args(["commit", "-m", "Outdated commit"])
        .env("PATH", new_path)
        .env_remove("ENGRAM_PATH")
        .current_dir(&ws)
        .output()
        .unwrap();

    assert!(!commit_out.status.success(), "Commit must fail when only outdated binary is found");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&commit_out.stdout),
        String::from_utf8_lossy(&commit_out.stderr)
    );
    assert!(
        combined.contains("does not support the required 'hook' command")
            || combined.contains("No compatible 'engram' executable found"),
        "Must output actionable diagnostic: {combined}"
    );
    assert!(
        combined.contains("--no-verify"),
        "Must output --no-verify escape hatch: {combined}"
    );

    let _ = fs::remove_dir_all(&ws);
}
