use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use engram::instructions::{END_MARKER, START_MARKER};

fn temp_workspace(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "engram_turn0_test_{}_{}",
        name,
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&p).unwrap();

    // Initialize git repository so git baseline commit works
    let _ = Command::new("git").args(["init"]).current_dir(&p).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Engram Tester"])
        .current_dir(&p)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "tester@engram.local"])
        .current_dir(&p)
        .output();

    // Create a dummy file and initial commit
    let dummy = p.join("README.md");
    fs::write(&dummy, "# Test Project\n").unwrap();
    let _ = Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&p)
        .output();
    let _ = Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&p)
        .output();

    p
}

#[test]
fn test_turn0_inject_cli_standard_task() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("standard_inject");

    // Initialize engram in workspace
    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    // Create a sample source file
    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let auth_file = src_dir.join("auth.ts");
    fs::write(
        &auth_file,
        "export interface UserSession {\n  id: string;\n  token: string;\n}\n",
    )
    .unwrap();

    // Run engram inject with a standard multi-file/auth task
    let inject_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "inject",
            "Implement remember-me session persistence in src/auth.ts and src/login.ts",
        ])
        .output()
        .expect("engram inject failed");
    assert!(inject_out.status.success());

    let stdout = String::from_utf8_lossy(&inject_out.stdout);
    assert!(stdout.contains("Engram Turn-0 Instruction Injection"));
    assert!(stdout.contains("Locked Change Contract: ctr_"));

    // Verify .github/copilot-instructions.md exists and contains Turn-0 directives
    let inst_path = ws.join(".github").join("copilot-instructions.md");
    assert!(inst_path.exists());
    let inst_content = fs::read_to_string(&inst_path).unwrap();

    assert!(inst_content.contains(START_MARKER));
    assert!(inst_content.contains("Engram Turn-0 Task Intelligence & Directives"));
    assert!(inst_content.contains("Active Change Contract:"));
    assert!(inst_content.contains("ZERO-TURN EXECUTION"));
    assert!(inst_content.contains("DO NOT CALL `engram_get_context` OR `engram_preflight`"));
    assert!(inst_content.contains("Relevant Context (AST Grounded)"));
    assert!(inst_content.contains(END_MARKER));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_turn0_inject_cli_adaptive_bypass() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("bypass_inject");

    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let label_file = src_dir.join("label.ts");
    fs::write(&label_file, "export const HELLO = 'helo world';\n").unwrap();

    // Run engram inject with a trivial single-file typo fix
    let inject_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["inject", "Fix typo in src/label.ts"])
        .output()
        .expect("engram inject failed");
    assert!(inject_out.status.success());

    let stdout = String::from_utf8_lossy(&inject_out.stdout);
    assert!(stdout.contains("Adaptive Bypass Active"));
    assert!(stdout.contains("Trivial task detected"));

    // Verify .github/copilot-instructions.md contains bypass directives
    let inst_path = ws.join(".github").join("copilot-instructions.md");
    let inst_content = fs::read_to_string(&inst_path).unwrap();

    assert!(inst_content.contains(START_MARKER));
    assert!(inst_content.contains("Adaptive Small-Task Bypass Active"));
    assert!(inst_content.contains("TRIVIAL"));
    assert!(inst_content.contains("DO NOT call `engram_get_context` or `engram_preflight`"));
    assert!(inst_content.contains("Proceed directly to editing the target file"));
    assert!(inst_content.contains(END_MARKER));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_turn0_inject_cli_dry_run_and_target_flags() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("dry_run_target");

    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    // 1. Dry run: must not create file
    let dry_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "inject",
            "Update button style in src/button.css",
            "--dry-run",
        ])
        .output()
        .expect("dry run failed");
    assert!(dry_out.status.success());
    let inst_path = ws.join(".github").join("copilot-instructions.md");
    assert!(
        !inst_path.exists(),
        "Dry run must not create instruction file"
    );

    // 2. Client flag: targets .cursorrules
    let cursor_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "inject",
            "Update button style in src/button.css",
            "--client",
            "cursor",
        ])
        .output()
        .expect("cursor inject failed");
    assert!(cursor_out.status.success());
    let cursor_rules = ws.join(".cursorrules");
    assert!(cursor_rules.exists());
    let cursor_content = fs::read_to_string(&cursor_rules).unwrap();
    assert!(cursor_content.contains(START_MARKER));

    // 3. Custom target flag
    let custom_target = ws.join("custom").join("rules.md");
    let target_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "inject",
            "Update button style in src/button.css",
            "--target",
            "custom/rules.md",
        ])
        .output()
        .expect("target inject failed");
    assert!(target_out.status.success());
    assert!(custom_target.exists());
    let custom_content = fs::read_to_string(&custom_target).unwrap();
    assert!(custom_content.contains(START_MARKER));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_turn0_inject_clear_and_strip_lifecycle() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("clear_strip");

    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    let inst_file = ws.join(".github").join("copilot-instructions.md");
    fs::create_dir_all(inst_file.parent().unwrap()).unwrap();
    fs::write(&inst_file, "# Project Standards\n- Keep code clean.\n").unwrap();

    // 1. Inject a task
    let inject_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "inject",
            "Refactor session store in src/session.ts and src/store.ts",
        ])
        .output()
        .expect("inject failed");
    assert!(inject_out.status.success());

    let content_after_inject = fs::read_to_string(&inst_file).unwrap();
    assert!(content_after_inject.contains("# Project Standards"));
    assert!(content_after_inject.contains("Refactor session store"));
    assert!(content_after_inject.contains("Active Change Contract:"));

    // 2. Clear: resets back to DEFAULT_INSTRUCTION_BODY, keeping user section
    let clear_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["inject", "--clear"])
        .output()
        .expect("inject clear failed");
    assert!(clear_out.status.success());

    let content_after_clear = fs::read_to_string(&inst_file).unwrap();
    assert!(content_after_clear.contains("# Project Standards"));
    assert!(!content_after_clear.contains("Refactor session store"));
    assert!(content_after_clear.contains("Engram Architecture Context & Task-Aware Intelligence"));

    // 3. Strip: removes the managed block completely
    let strip_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args(["inject", "--strip"])
        .output()
        .expect("inject strip failed");
    assert!(strip_out.status.success());

    let content_after_strip = fs::read_to_string(&inst_file).unwrap();
    assert!(content_after_strip.contains("# Project Standards"));
    assert!(!content_after_strip.contains(START_MARKER));
    assert!(!content_after_strip.contains(END_MARKER));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_turn0_prepare_cli_inject_integration_and_json() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("prepare_inject");

    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    // Run engram prepare with --inject flag
    let prep_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "prepare",
            "Implement OAuth login handler in src/auth.ts and src/oauth.ts",
            "--inject",
        ])
        .output()
        .expect("engram prepare failed");
    assert!(prep_out.status.success());

    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&prep_out.stdout),
        String::from_utf8_lossy(&prep_out.stderr)
    );
    assert!(combined.contains("Locked Change Contract"));
    assert!(combined.contains("Injected Turn-0 task directives into"));

    let inst_path = ws.join(".github").join("copilot-instructions.md");
    assert!(inst_path.exists());
    let inst_content = fs::read_to_string(&inst_path).unwrap();
    assert!(inst_content.contains("OAuth login handler"));

    // Test JSON output of engram inject
    let json_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "inject",
            "Implement OAuth login handler in src/auth.ts and src/oauth.ts",
            "--json",
        ])
        .output()
        .expect("engram inject --json failed");
    assert!(json_out.status.success());

    let stdout = String::from_utf8_lossy(&json_out.stdout);
    let v: Value = serde_json::from_str(&stdout).expect("must parse as json");
    assert!(v["contract_id"].as_str().unwrap().starts_with("ctr_"));
    assert!(v["reports"].is_array());
    assert!(!v["bypass_active"].as_bool().unwrap());

    let _ = fs::remove_dir_all(&ws);
}
