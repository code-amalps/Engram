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
