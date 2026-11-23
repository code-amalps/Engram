//! Phase 8.1: Consolidated Engagement Endpoint (`engram_engage`) Integration Tests
//!
//! Verifies:
//! 1. Endpoint tool definition registration in core and all tool schemas.
//! 2. Contract validation (empty task, oversized task >4096 chars).
//! 3. Security boundaries (path traversal rejection in planned_files and forbidden).
//! 4. Standard task engagement (status READY, contract locked, context compiled, AST symbols).
//! 5. Adaptive trivial bypass (status BYPASS_TRIVIAL, zero tokens, contract null).
//! 6. Forced execution (no_bypass: true forces contract lock on trivial task).
//! 7. Constitution enforcement (status BLOCKED when editing forbidden boundaries).
//! 8. Contract reuse idempotency across repeated invocations.
//! 9. Turn-0 instruction injection integration (`inject_instructions: true`).
//! 10. Backward compatibility with all existing MCP tools.
//! 11. Maximum response byte budget (`max_bytes`) boundary enforcement.

use engram::contract::get_active_contract;
use engram::mcp::{all_tools_definitions, core_tools_definitions, mcp_call, mcp_call_raw};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn setup_git_workspace(name: &str) -> PathBuf {
    let tmp = std::env::temp_dir().join(format!(
        "engram_p81_engage_{}_{}",
        name,
        uuid::Uuid::new_v4()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    // Initialize git repository
    let run_git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&tmp)
            .output()
            .expect("failed to execute git command");
        assert!(
            output.status.success(),
            "Git command {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    };

    run_git(&["init", "-b", "main"]);
    run_git(&["config", "user.name", "Engram Test"]);
    run_git(&["config", "user.email", "test@engram.dev"]);

    // Create realistic codebase structure
    let files = vec![
        (
            "Cargo.toml",
            r#"[package]
name = "service"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
        ),
        (
            "src/lib.rs",
            r#"pub mod auth;
pub mod user;
pub fn init() -> bool { true }
"#,
        ),
        (
            "src/auth.rs",
            r#"pub struct AuthService { pub enabled: bool }
impl AuthService {
    pub fn authenticate(token: &str) -> bool {
        !token.is_empty()
    }
}
"#,
        ),
        (
            "src/user.rs",
            r#"pub struct User { pub id: u64, pub name: String }
impl User {
    pub fn new(id: u64, name: &str) -> Self {
        Self { id, name: name.to_string() }
    }
}
"#,
        ),
        (
            "README.md",
            "# Service\nEnterprise microservice documentation.\n",
        ),
    ];

    for (rel, content) in files {
        let p = tmp.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    run_git(&["add", "."]);
    run_git(&["commit", "-m", "Initial commit"]);

    tmp
}

#[test]
fn test_engage_tool_definition_registered() {
    let all = all_tools_definitions();
    let all_tools = all.as_array().expect("all_tools_definitions must be array");
    let engage_tool = all_tools
        .iter()
        .find(|t| t["name"] == "engram_engage")
        .expect("engram_engage must be registered in all_tools_definitions");

    assert!(
        engage_tool["description"]
            .as_str()
            .unwrap()
            .contains("Consolidated")
    );
    let schema = &engage_tool["inputSchema"];
    assert_eq!(schema["type"], "object");
    let props = &schema["properties"];
    assert!(props.get("task").is_some());
    assert!(props.get("token_budget").is_some());
    assert!(props.get("planned_files").is_some());
    assert!(props.get("forbidden").is_some());
    assert!(props.get("symbols").is_some());
    assert!(props.get("subpath").is_some());
    assert!(props.get("language").is_some());
    assert!(props.get("detail_level").is_some());
    assert!(props.get("no_bypass").is_some());
    assert!(props.get("inject_instructions").is_some());
    assert!(props.get("max_bytes").is_some());
    assert_eq!(schema["required"], json!(["task"]));

    // Lean core tools baseline remains exactly 4 tools
    let core = core_tools_definitions();
    let core_tools = core
        .as_array()
        .expect("core_tools_definitions must be array");
    assert_eq!(
        core_tools.len(),
        4,
        "Core lean tools must be exactly 4 tools"
    );
}

#[test]
fn test_engage_empty_task_rejected() {
    let ws = setup_git_workspace("empty_task");
    let args = json!({ "task": "" });
    let res = mcp_call_raw(&ws, "engram_engage", &args);
    assert!(res.is_err(), "Empty task must be rejected");
    assert!(res.unwrap_err().to_string().contains("must not be empty"));

    let whitespace_args = json!({ "task": "   \n\t  " });
    let res_ws = mcp_call_raw(&ws, "engram_engage", &whitespace_args);
    assert!(res_ws.is_err(), "Whitespace task must be rejected");
    assert!(
        res_ws
            .unwrap_err()
            .to_string()
            .contains("must not be empty")
    );
    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_engage_oversized_task_rejected() {
    let ws = setup_git_workspace("oversized_task");
    let huge_task = "x".repeat(4097);
    let args = json!({ "task": huge_task });
    let res = mcp_call_raw(&ws, "engram_engage", &args);
    assert!(res.is_err(), "Task exceeding 4096 chars must be rejected");
    assert!(
        res.unwrap_err()
            .to_string()
            .contains("exceeds maximum length")
    );
    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_engage_path_traversal_rejected() {
    let ws = setup_git_workspace("path_traversal");

    // Traversal in planned_files
    let args_planned = json!({
        "task": "Refactor auth module",
        "planned_files": ["../../etc/shadow"]
    });
    let res_planned = mcp_call_raw(&ws, "engram_engage", &args_planned);
    assert!(
        res_planned.is_err(),
        "Path traversal in planned_files must fail"
    );
    assert!(res_planned.unwrap_err().to_string().contains("traversal"));

    // Traversal in forbidden
    let args_forbidden = json!({
        "task": "Refactor auth module",
        "forbidden": ["../outside/secret.key"]
    });
    let res_forbidden = mcp_call_raw(&ws, "engram_engage", &args_forbidden);
    assert!(
        res_forbidden.is_err(),
        "Path traversal in forbidden must fail"
    );
    assert!(res_forbidden.unwrap_err().to_string().contains("traversal"));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_engage_standard_task_ready() {
    let ws = setup_git_workspace("standard_ready");
    let args = json!({
        "task": "Refactor authentication token verification in src/auth.rs and update User in src/user.rs",
        "planned_files": ["src/auth.rs", "src/user.rs"]
    });

    let res = mcp_call_raw(&ws, "engram_engage", &args).expect("engram_engage should succeed");

    assert_eq!(res["status"], "READY");
    assert_eq!(res["task"]["classification"], "STANDARD");
    assert_eq!(res["task"]["bypass_active"], false);

    // Repository metadata
    assert!(res["repository"]["baseline_commit"].is_string());
    assert!(
        !res["repository"]["baseline_commit"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    assert!(res["repository"]["is_clean"].as_bool().unwrap());

    // Contract locking
    let contract_id = res["contract"]["contract_id"]
        .as_str()
        .expect("contract_id must be present");
    assert!(!contract_id.is_empty());
    assert_eq!(res["contract"]["is_reused"], false);

    // Boundaries
    let expected = res["boundaries"]["expected_files"]
        .as_array()
        .expect("expected_files must be array");
    let expected_str: Vec<_> = expected.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(expected_str.contains(&"src/auth.rs"));
    assert!(expected_str.contains(&"src/user.rs"));

    // Context
    let targets = res["context"]["target_files"]
        .as_array()
        .expect("target_files must be array");
    assert!(
        !targets.is_empty(),
        "target_files should have extracted context"
    );

    // Guidance
    assert!(
        res["next_action"]
            .as_str()
            .unwrap()
            .contains("Proceed directly to implement")
    );
    assert!(res["blocking_violations"].as_array().unwrap().is_empty());

    // Verify active contract exists on disk
    let active = get_active_contract(&ws)
        .expect("active contract lookup failed")
        .expect("contract must exist");
    assert_eq!(active.contract_id, contract_id);

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_engage_trivial_task_bypass() {
    let ws = setup_git_workspace("trivial_bypass");
    let args = json!({
        "task": "Fix typo in README.md"
    });

    let res = mcp_call_raw(&ws, "engram_engage", &args).expect("engram_engage should succeed");

    assert_eq!(res["status"], "BYPASS_TRIVIAL");
    assert_eq!(res["task"]["classification"], "TRIVIAL");
    assert_eq!(res["task"]["bypass_active"], true);
    assert!(res["contract"]["contract_id"].is_null());
    assert_eq!(res["contract"]["is_reused"], false);
    assert_eq!(res["context"]["estimated_tokens"], 0);
    assert!(
        res["next_action"]
            .as_str()
            .unwrap()
            .contains("trivial leaf edit")
    );

    // Verify NO contract was created on disk
    let active = get_active_contract(&ws).expect("active contract check failed");
    assert!(
        active.is_none(),
        "No contract should be created for bypassed trivial task"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_engage_trivial_task_no_bypass_forced() {
    let ws = setup_git_workspace("trivial_no_bypass");
    let args = json!({
        "task": "Fix typo in README.md",
        "no_bypass": true
    });

    let res = mcp_call_raw(&ws, "engram_engage", &args).expect("engram_engage should succeed");

    assert_eq!(res["status"], "READY");
    assert_eq!(res["task"]["bypass_active"], false);
    assert!(res["contract"]["contract_id"].is_string());

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_engage_constitution_violation_blocked() {
    let ws = setup_git_workspace("constitution_blocked");

    // Create an explicit Project Constitution with a forbidden boundary rule
    let engram_dir = ws.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    let constitution = r#"
rules:
  - id: "SEC-001"
    name: "Protect credentials and keys"
    type: "PATTERN"
    severity: "ERROR"
    forbidden: "config/secrets.json"
"#;
    fs::write(engram_dir.join("constitution.yaml"), constitution).unwrap();

    let args = json!({
        "task": "Update credentials in config/secrets.json",
        "planned_files": ["config/secrets.json"],
        "forbidden": ["config/secrets.json"]
    });

    let res =
        mcp_call_raw(&ws, "engram_engage", &args).expect("engram_engage should return response");

    assert_eq!(res["status"], "BLOCKED");
    let blocking = res["blocking_violations"]
        .as_array()
        .expect("blocking_violations must be array");
    assert!(!blocking.is_empty(), "Blocking violations must be reported");
    assert!(
        res["next_action"]
            .as_str()
            .unwrap()
            .contains("Reconcile blocking violations")
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_engage_contract_reuse_idempotency() {
    let ws = setup_git_workspace("contract_reuse");
    let args = json!({
        "task": "Implement token validation in src/auth.rs",
        "planned_files": ["src/auth.rs"]
    });

    let first = mcp_call_raw(&ws, "engram_engage", &args).expect("first engage failed");
    assert_eq!(first["status"], "READY");
    assert_eq!(first["contract"]["is_reused"], false);
    let cid1 = first["contract"]["contract_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Second call with same task and scope
    let second = mcp_call_raw(&ws, "engram_engage", &args).expect("second engage failed");
    assert_eq!(second["status"], "READY");
    assert_eq!(second["contract"]["is_reused"], true);
    let cid2 = second["contract"]["contract_id"].as_str().unwrap();
    assert_eq!(cid1, cid2, "Contract ID must be reused idempotently");

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_engage_instruction_injection() {
    let ws = setup_git_workspace("instruction_injection");
    let args = json!({
        "task": "Refactor auth and user modules",
        "planned_files": ["src/auth.rs", "src/user.rs"],
        "inject_instructions": true
    });

    let res = mcp_call_raw(&ws, "engram_engage", &args)
        .expect("engage with instruction injection failed");
    assert_eq!(res["status"], "READY");
    let injected = res["instructions"]["injected_files"]
        .as_array()
        .expect("injected_files must be array");
    assert!(
        !injected.is_empty(),
        "Injected instruction files should be reported"
    );

    // Verify injected file exists on disk
    let has_injected_file = injected.iter().any(|p| {
        let path_str = p.as_str().unwrap();
        std::path::Path::new(path_str).exists() || ws.join(path_str).exists()
    });
    assert!(
        has_injected_file,
        "At least one instruction file must be created on disk"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_backward_compatibility_existing_tools() {
    let ws = setup_git_workspace("backward_compat");

    // 1. engram_get_context
    let ctx_res = mcp_call_raw(
        &ws,
        "engram_get_context",
        &json!({ "task": "Explain AuthService", "token_budget": 1000 }),
    )
    .expect("engram_get_context must succeed");
    assert_eq!(ctx_res["status"], "READY");
    assert!(ctx_res.get("target_files").is_some());

    // 2. engram_inspect_symbol
    let inspect_res = mcp_call_raw(
        &ws,
        "engram_inspect_symbol",
        &json!({ "target": "AuthService" }),
    )
    .expect("engram_inspect_symbol must succeed");
    assert_eq!(inspect_res["target"], "AuthService");
    assert!(inspect_res.get("impact_cone").is_some());

    // 3. engram_preflight
    let preflight_res = mcp_call_raw(
        &ws,
        "engram_preflight",
        &json!({ "task": "Add user method", "planned_files": ["src/user.rs"] }),
    )
    .expect("engram_preflight must succeed");
    assert_eq!(preflight_res["status"], "READY_FOR_IMPLEMENTATION");
    let cid = preflight_res["contract_id"].as_str().unwrap();

    // 4. engram_postflight
    let postflight_res = mcp_call_raw(
        &ws,
        "engram_postflight",
        &json!({ "contract_id": cid, "test_evidence": true }),
    )
    .expect("engram_postflight must succeed");
    assert_eq!(postflight_res["verdict"], "PASS");

    // 5. engram_recover
    let recover_res = mcp_call_raw(&ws, "engram_recover", &json!({ "contract_id": cid }))
        .expect("engram_recover must succeed");
    assert!(
        recover_res.get("recovery_recipe").is_some() || recover_res.get("contract_id").is_some()
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_envelope_max_bytes_budget_enforcement() {
    let ws = setup_git_workspace("max_bytes_budget");
    let args = json!({
        "task": "Refactor authentication token verification in src/auth.rs and update User in src/user.rs",
        "planned_files": ["src/auth.rs", "src/user.rs"],
        "max_bytes": 2048
    });

    let res = mcp_call(&ws, "engram_engage", &args).expect("mcp_call engram_engage should succeed");
    assert_eq!(res["isError"], false);
    let content = &res["content"][0]["text"]
        .as_str()
        .expect("text must be string");
    let raw_bytes = content.len();
    assert!(
        raw_bytes <= 2048,
        "Envelope content byte length ({}) must not exceed requested max_bytes (2048)",
        raw_bytes
    );

    let parsed: Value = serde_json::from_str(content).expect("content must be valid JSON");
    assert!(parsed.get("status").is_some());

    let _ = fs::remove_dir_all(&ws);
}
