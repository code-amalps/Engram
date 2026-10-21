use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use engram::instructions::{END_MARKER, START_MARKER};

fn temp_workspace(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "engram_phase1_test_{}_{}",
        name,
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&p).unwrap();

    // Initialize git repository
    let _ = Command::new("git").args(["init"]).current_dir(&p).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Phase1 Tester"])
        .current_dir(&p)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "tester@engram.local"])
        .current_dir(&p)
        .output();

    let dummy = p.join("README.md");
    fs::write(&dummy, "# Phase 1 Test Workspace\n").unwrap();
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
fn test_prepare_cli_with_task_flag_and_positional() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("task_flag_vs_pos");

    // Initialize engram in workspace
    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let app_file = src_dir.join("app.ts");
    fs::write(&app_file, "export function startApp() { return true; }\n").unwrap();

    // 1. Run using --task flag
    let prep_flag_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "prepare",
            "--task",
            "Refactor startApp function in src/app.ts",
            "--json",
        ])
        .output()
        .expect("engram prepare --task failed");
    assert!(prep_flag_out.status.success(), "prepare --task must succeed");

    let v_flag: Value = serde_json::from_slice(&prep_flag_out.stdout).expect("valid json");
    assert!(v_flag["contract_id"].as_str().unwrap().starts_with("ctr_"));
    assert_eq!(v_flag["status"].as_str().unwrap(), "ReadyForImplementation");
    assert!(v_flag["content"].as_str().unwrap().contains("Refactor startApp"));
    assert!(v_flag["content"].as_str().unwrap().contains("Provenance:"));

    // 2. Run using positional argument (backward compatibility)
    let prep_pos_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "prepare",
            "Refactor startApp function in src/app.ts",
            "--json",
        ])
        .output()
        .expect("engram prepare positional failed");
    assert!(prep_pos_out.status.success(), "prepare <task> positional must succeed");

    let v_pos: Value = serde_json::from_slice(&prep_pos_out.stdout).expect("valid json");
    assert!(v_pos["contract_id"].as_str().unwrap().starts_with("ctr_"));
    assert_eq!(v_pos["contract_id"], v_flag["contract_id"], "Identical task should reuse contract");

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_prepare_cli_missing_task_error() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("missing_task_err");

    let out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("prepare")
        .output()
        .expect("engram prepare execution");

    assert!(!out.status.success(), "prepare without task must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Task description required") || stderr.contains("required") || stderr.contains("Usage:"),
        "stderr must report missing task argument: {}",
        stderr
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_prepare_cli_dry_run() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("dry_run_prepare");

    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    let file = src_dir.join("calc.ts");
    fs::write(&file, "export const add = (a: number, b: number) => a + b;\n").unwrap();

    let prep_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "prepare",
            "--task",
            "Update add function in src/calc.ts",
            "--inject",
            "--write-file",
            "--dry-run",
        ])
        .output()
        .expect("prepare --dry-run failed");
    assert!(prep_out.status.success());

    let stderr = String::from_utf8_lossy(&prep_out.stderr);
    assert!(stderr.contains("DRY-RUN ONLY"));

    // Verify neither active_task.md nor copilot-instructions.md were created
    let active_task = ws.join(".engram").join("active_task.md");
    assert!(!active_task.exists(), "active_task.md must NOT be created under --dry-run");

    let inst_path = ws.join(".github").join("copilot-instructions.md");
    assert!(!inst_path.exists(), "copilot-instructions.md must NOT be created under --dry-run");

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_prepare_cli_unresolved_context() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("unresolved_context");

    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    // Task targeting a non-existent symbol/file that cannot be grounded
    let prep_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "prepare",
            "--task",
            "Modify nonexistent_component_xyz99 in src/nonexistent.ts",
            "--json",
        ])
        .output()
        .expect("engram prepare failed");
    assert!(prep_out.status.success());

    let v: Value = serde_json::from_slice(&prep_out.stdout).expect("valid json");
    let completeness = v["completeness_status"].as_str().unwrap();
    assert!(
        completeness.contains("UNRESOLVED_CONTEXT") || completeness.contains("PARTIAL_GROUNDING"),
        "Completeness status must report unresolved/partial context: {}",
        completeness
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_instruction_idempotency_and_preservation() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("idempotent_inject");

    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    let inst_file = ws.join(".github").join("copilot-instructions.md");
    fs::create_dir_all(inst_file.parent().unwrap()).unwrap();

    let user_header = "# Team Engineering Standards\n- Write unit tests.\n- Follow TypeScript strict mode.\n\n";
    let user_footer = "\n# Performance Guidelines\n- Limit bundle size under 200KB.\n";
    fs::write(&inst_file, format!("{}{}", user_header, user_footer)).unwrap();

    // 1. Run preparation with inject
    let out1 = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "prepare",
            "--task",
            "Add user avatar cache in src/avatar.ts and src/cache.ts",
            "--inject",
        ])
        .output()
        .expect("first prepare inject failed");
    assert!(out1.status.success());

    let content1 = fs::read_to_string(&inst_file).unwrap();
    assert!(content1.starts_with("# Team Engineering Standards"));
    assert!(content1.contains("- Limit bundle size under 200KB."));
    assert_eq!(content1.matches(START_MARKER).count(), 1, "Exactly one start marker after run 1");
    assert_eq!(content1.matches(END_MARKER).count(), 1, "Exactly one end marker after run 1");
    assert!(content1.contains("Generated locally by Engram"));
    assert!(content1.contains("VERIFICATION GUIDANCE:"));

    // 2. Run identical preparation again (Idempotency check)
    let out2 = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "prepare",
            "--task",
            "Add user avatar cache in src/avatar.ts and src/cache.ts",
            "--inject",
        ])
        .output()
        .expect("second prepare inject failed");
    assert!(out2.status.success());

    let content2 = fs::read_to_string(&inst_file).unwrap();
    assert_eq!(content1, content2, "Repeated injection must be byte-for-byte identical (idempotent)");
    assert_eq!(content2.matches(START_MARKER).count(), 1, "Exactly one start marker after run 2");
    assert_eq!(content2.matches(END_MARKER).count(), 1, "Exactly one end marker after run 2");

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_malformed_instruction_file_safeguard() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("malformed_safeguard");

    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    let inst_file = ws.join(".github").join("copilot-instructions.md");
    fs::create_dir_all(inst_file.parent().unwrap()).unwrap();

    // Intentionally corrupt the file with an unmatched start marker
    let corrupted_content = format!(
        "# User Config\n{}\nSome partial block without end marker...\n",
        START_MARKER
    );
    fs::write(&inst_file, &corrupted_content).unwrap();

    let out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "prepare",
            "--task",
            "Implement feature in src/feat.ts",
            "--inject",
        ])
        .output()
        .expect("prepare inject on malformed file");

    assert!(!out.status.success(), "Command must fail on malformed managed block");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Malformed Engram managed block"), "Error must explain malformed block: {}", stderr);

    // Verify file content was NOT overwritten or corrupted
    let content_after = fs::read_to_string(&inst_file).unwrap();
    assert_eq!(corrupted_content, content_after, "File content must be preserved unchanged when block is malformed");

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_adaptive_bypass_comprehensive_matrix() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("bypass_matrix");

    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    let check_bypass = |task: &str, no_bypass: bool| -> (bool, String) {
        let mut cmd = Command::new(bin);
        cmd.arg("-C").arg(&ws).arg("prepare").arg("--task").arg(task).arg("--json");
        if no_bypass {
            cmd.arg("--no-bypass");
        }
        let out = cmd.output().expect("prepare check");
        assert!(out.status.success(), "prepare must succeed for task: {}", task);
        let v: Value = serde_json::from_slice(&out.stdout).expect("valid json");
        let bypass = v["bypass_active"].as_bool().unwrap();
        let reason = if let Some(sig) = v.get("classification_signals") {
            sig["reason"].as_str().unwrap_or("").to_string()
        } else {
            String::new()
        };
        (bypass, reason)
    };

    // 1. Trivial typo task -> Bypass = true
    let (bypass, reason) = check_bypass("Fix typo in src/button.css", false);
    assert!(bypass, "Single file typo must be bypassed: reason={}", reason);

    // 2. Ambiguity signal -> Bypass = false
    let (bypass, _) = check_bypass("Investigate performance or something in src/button.css", false);
    assert!(!bypass, "Ambiguous task must NOT be bypassed");

    // 3. Security keyword -> Bypass = false
    let (bypass, _) = check_bypass("Fix typo in auth token handler in src/auth.ts", false);
    assert!(!bypass, "Security keyword must NOT be bypassed");

    // 4. Security file path -> Bypass = false
    let (bypass, _) = check_bypass("Fix typo in src/secrets.json", false);
    assert!(!bypass, "Security file path must NOT be bypassed");

    // 5. Manifest file path -> Bypass = false
    let (bypass, _) = check_bypass("Fix typo in Cargo.toml", false);
    assert!(!bypass, "Manifest file path must NOT be bypassed");

    // 6. Test file path -> Bypass = false
    let (bypass, _) = check_bypass("Fix typo in tests/test_login.rs", false);
    assert!(!bypass, "Test file path must NOT be bypassed");

    // 7. Architecture keyword -> Bypass = false
    let (bypass, _) = check_bypass("Refactor interface in src/button.css", false);
    assert!(!bypass, "Architecture keyword must NOT be bypassed");

    // 8. Multi-file task -> Bypass = false
    let (bypass, _) = check_bypass("Fix typo in src/button.css and src/modal.css", false);
    assert!(!bypass, "Multi-file task must NOT be bypassed");

    // 9. Trivial task with --no-bypass override -> Bypass = false
    let (bypass, _) = check_bypass("Fix typo in src/button.css", true);
    assert!(!bypass, "Trivial task with --no-bypass must NOT be bypassed");

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_mcp_and_existing_commands_compatibility() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("compat_test");

    let init_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .output()
        .expect("engram init failed");
    assert!(init_out.status.success());

    // Test engram prompt --lock-contract still works
    let prompt_out = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .args([
            "prompt",
            "Add validation in src/validate.ts",
            "--lock-contract",
            "--stdout",
        ])
        .output()
        .expect("engram prompt failed");
    assert!(prompt_out.status.success());
    let stdout = String::from_utf8_lossy(&prompt_out.stdout);
    assert!(stdout.contains("Active Contract"));

    let _ = fs::remove_dir_all(&ws);
}
