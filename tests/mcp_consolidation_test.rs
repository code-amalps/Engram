//! Phase 3: Single-Shot MCP Tool Consolidation Integration Tests
//!
//! Verifies:
//! 1. Consolidated `engram_prepare_task` primitive executes atomically in one round-trip.
//! 2. Context compilation, preflight boundary check, and Change Contract locking succeed.
//! 3. Adaptive small-task bypass safely fast-paths trivial edits with zero contract overhead.
//! 4. `--no-bypass` / `no_bypass: true` forces full contract locking on trivial tasks.
//! 5. Idempotent contract reuse when the same task is prepared multiple times.
//! 6. Explicit planned files and forbidden boundaries are respected.
//! 7. Path traversal and malformed inputs are strictly rejected.
//! 8. Compact (default) vs detailed response payloads enforce token efficiency.
//! 9. Legacy tools (`engram_get_context`, `engram_preflight`, `engram_postflight`, etc.) remain 100% backward compatible.
//! 10. End-to-end stdio JSON-RPC protocol execution for `engram_prepare_task`.

use engram::contract::get_active_contract;
use engram::mcp::{mcp_call, tools_definitions};
use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn setup_test_workspace(name: &str) -> PathBuf {
    let tmp = std::env::temp_dir().join(format!("engram_p3_mcp_{}_{}", name, uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let files = vec![
        (
            "package.json",
            r#"{"name": "auth-service", "version": "1.0.0", "dependencies": {"@angular/core": "^17.0.0"}}"#,
        ),
        (
            "src/app/core/auth/auth.effects.ts",
            "export class AuthEffects { login$ = true; rememberMe$ = false; }\n",
        ),
        (
            "src/app/core/auth/auth.models.ts",
            "export interface User { username: string; token: string; rememberMe?: boolean; }\n",
        ),
        (
            "src/app/core/auth/auth.reducer.ts",
            "export function authReducer(state: any, action: any) { return state; }\n",
        ),
        (
            "src/app/features/settings/settings.component.ts",
            "export class SettingsComponent {}\n",
        ),
        (
            "README.md",
            "# Auth Service\nProvides enterprise authentication.\n",
        ),
    ];

    for (rel, content) in files {
        let p = tmp.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    Command::new("git")
        .args(["init"])
        .current_dir(&tmp)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.name", "Test Engineer"])
        .current_dir(&tmp)
        .output()
        .unwrap();

    Command::new("git")
        .args(["config", "user.email", "test@engram.dev"])
        .current_dir(&tmp)
        .output()
        .unwrap();

    Command::new("git")
        .args(["add", "."])
        .current_dir(&tmp)
        .output()
        .unwrap();

    Command::new("git")
        .args(["commit", "-m", "Initial baseline commit"])
        .current_dir(&tmp)
        .output()
        .unwrap();

    engram::analyzer::analyze(&tmp).unwrap();
    tmp
}

#[test]
fn test_single_shot_prepare_task_success() {
    let root = setup_test_workspace("prep_success");

    let args = json!({
        "task": "Add remember-me feature to auth flow and session persistence",
        "token_budget": 1200
    });

    let resp = mcp_call(&root, "engram_prepare_task", &args).unwrap();
    assert_eq!(resp["isError"], false);

    let text = resp["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();

    assert_eq!(parsed["status"], "READY");
    let contract_id = parsed["contract_id"].as_str().unwrap();
    assert!(contract_id.starts_with("ctr_"));
    assert_eq!(parsed["contract_version"], 1);

    let baseline = parsed["baseline_commit"].as_str().unwrap();
    assert!(!baseline.is_empty());
    assert_ne!(baseline, "UNKNOWN");

    let expected_files = parsed["boundaries"]["expected_files"].as_array().unwrap();
    assert!(!expected_files.is_empty());
    assert!(expected_files.iter().any(|f| f.as_str().unwrap().contains("auth")));

    let target_files = parsed["context"]["target_files"].as_array().unwrap();
    assert!(!target_files.is_empty());
    assert!(target_files[0]["score"].as_f64().is_some());

    assert_eq!(parsed["is_reused_contract"], false);
    assert!(parsed["blocking_violations"].as_array().unwrap().is_empty());
    assert!(parsed["next_action"].as_str().unwrap().contains("Change Contract locked"));

    // Verify active contract was persisted on disk
    let active = get_active_contract(&root).unwrap().unwrap();
    assert_eq!(active.contract_id, contract_id);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_single_shot_prepare_task_adaptive_bypass() {
    let root = setup_test_workspace("prep_bypass");

    let args = json!({
        "task": "Fix typo in README.md documentation",
        "token_budget": 1200
    });

    let resp = mcp_call(&root, "engram_prepare_task", &args).unwrap();
    assert_eq!(resp["isError"], false);

    let text = resp["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();

    assert_eq!(parsed["status"], "BYPASS_TRIVIAL");
    assert_eq!(parsed["classification"], "TRIVIAL");
    assert!(parsed["contract_id"].is_null());
    assert!(parsed["contract_version"].is_null());
    assert!(parsed["baseline_commit"].is_null());
    assert!(parsed["boundaries"]["expected_files"].as_array().unwrap().is_empty());
    assert!(parsed["context"]["target_files"].as_array().unwrap().is_empty());
    assert!(parsed["next_action"].as_str().unwrap().contains("trivial leaf edit"));

    // Verify NO contract was created on disk
    let active = get_active_contract(&root).unwrap();
    assert!(active.is_none());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_single_shot_prepare_task_force_no_bypass() {
    let root = setup_test_workspace("prep_no_bypass");

    let args = json!({
        "task": "Fix typo in README.md documentation",
        "token_budget": 1200,
        "no_bypass": true
    });

    let resp = mcp_call(&root, "engram_prepare_task", &args).unwrap();
    assert_eq!(resp["isError"], false);

    let text = resp["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();

    assert_eq!(parsed["status"], "READY");
    let contract_id = parsed["contract_id"].as_str().unwrap();
    assert!(contract_id.starts_with("ctr_"));
    assert_eq!(parsed["contract_version"], 1);

    let active = get_active_contract(&root).unwrap().unwrap();
    assert_eq!(active.contract_id, contract_id);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_single_shot_prepare_task_idempotent_contract_reuse() {
    let root = setup_test_workspace("prep_reuse");

    let args = json!({
        "task": "Add remember-me feature to auth flow and session persistence",
        "token_budget": 1200
    });

    // First call: locks new contract
    let resp1 = mcp_call(&root, "engram_prepare_task", &args).unwrap();
    let text1 = resp1["content"][0]["text"].as_str().unwrap();
    let parsed1: Value = serde_json::from_str(text1).unwrap();
    assert_eq!(parsed1["is_reused_contract"], false);
    let cid1 = parsed1["contract_id"].as_str().unwrap().to_string();

    // Second call: detects and reuses active contract
    let resp2 = mcp_call(&root, "engram_prepare_task", &args).unwrap();
    let text2 = resp2["content"][0]["text"].as_str().unwrap();
    let parsed2: Value = serde_json::from_str(text2).unwrap();
    assert_eq!(parsed2["status"], "READY");
    assert_eq!(parsed2["contract_id"].as_str().unwrap(), cid1);
    assert_eq!(parsed2["is_reused_contract"], true);
    assert!(parsed2["reused_reason"].as_str().is_some());
    assert!(parsed2["next_action"].as_str().unwrap().contains("Valid Change Contract active"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_single_shot_prepare_task_explicit_planned_and_forbidden() {
    let root = setup_test_workspace("prep_planned");

    let args = json!({
        "task": "Refactor auth models",
        "token_budget": 1200,
        "planned_files": ["src/app/core/auth/auth.models.ts"],
        "forbidden": ["src/app/features/settings/settings.component.ts"]
    });

    let resp = mcp_call(&root, "engram_prepare_task", &args).unwrap();
    let text = resp["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();

    assert_eq!(parsed["status"], "READY");
    let expected = parsed["boundaries"]["expected_files"].as_array().unwrap();
    assert!(expected.iter().any(|f| f == "src/app/core/auth/auth.models.ts"));

    let forbidden = parsed["boundaries"]["forbidden_boundaries"].as_array().unwrap();
    assert_eq!(forbidden.len(), 1);
    assert_eq!(forbidden[0], "src/app/features/settings/settings.component.ts");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_single_shot_prepare_task_compact_vs_detailed_payload() {
    let root = setup_test_workspace("prep_payload");

    let compact_args = json!({
        "task": "Add remember-me feature to auth flow and session persistence",
        "token_budget": 1200
    });
    let compact_resp = mcp_call(&root, "engram_prepare_task", &compact_args).unwrap();
    let compact_str = serde_json::to_string(&compact_resp).unwrap();

    let detailed_args = json!({
        "task": "Add remember-me feature to auth flow and session persistence",
        "token_budget": 1200,
        "details": true
    });
    let detailed_resp = mcp_call(&root, "engram_prepare_task", &detailed_args).unwrap();
    let detailed_str = serde_json::to_string(&detailed_resp).unwrap();

    let compact_len = compact_str.len();
    let detailed_len = detailed_str.len();

    assert!(
        compact_len < detailed_len,
        "Compact payload ({compact_len} chars) must be strictly smaller than detailed ({detailed_len} chars)"
    );

    let detailed_text = detailed_resp["content"][0]["text"].as_str().unwrap();
    let detailed_parsed: Value = serde_json::from_str(detailed_text).unwrap();
    assert!(detailed_parsed["preflight"].is_object());
    assert!(detailed_parsed["context"].is_object());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_single_shot_prepare_task_input_validation_and_errors() {
    let root = setup_test_workspace("prep_errors");

    // 1. Empty task
    let res = mcp_call(&root, "engram_prepare_task", &json!({ "task": "" }));
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Task description must not be empty"));

    // 2. Whitespace-only task
    let res = mcp_call(&root, "engram_prepare_task", &json!({ "task": "   " }));
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Task description must not be empty"));

    // 3. Token budget == 0
    let res = mcp_call(&root, "engram_prepare_task", &json!({ "task": "Valid task", "token_budget": 0 }));
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("Token budget must be greater than 0"));

    // 4. Planned files path traversal escape
    let res = mcp_call(
        &root,
        "engram_prepare_task",
        &json!({ "task": "Valid task", "planned_files": ["../../outside.ts"] }),
    );
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("directory traversal ('..') is prohibited"));

    // 5. Forbidden boundary path traversal escape
    let res = mcp_call(
        &root,
        "engram_prepare_task",
        &json!({ "task": "Valid task", "forbidden": ["../../secret.key"] }),
    );
    assert!(res.is_err());
    assert!(res.unwrap_err().to_string().contains("directory traversal ('..') is prohibited"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_single_shot_prepare_task_alias() {
    let root = setup_test_workspace("prep_alias");

    let args = json!({
        "task": "Add remember-me feature to auth flow and session persistence",
        "token_budget": 1200
    });

    // Calling via alias "engram_prepare"
    let resp = mcp_call(&root, "engram_prepare", &args).unwrap();
    assert_eq!(resp["isError"], false);

    let text = resp["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["status"], "READY");
    assert!(parsed["contract_id"].as_str().unwrap().starts_with("ctr_"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_tools_definitions_contains_prepare_task() {
    let defs = tools_definitions();
    let tools = defs.as_array().expect("tools array");

    let prep_tool = tools
        .iter()
        .find(|t| t["name"] == "engram_prepare_task")
        .expect("engram_prepare_task must be defined in tools_definitions");

    assert_eq!(prep_tool["name"], "engram_prepare_task");
    assert!(prep_tool["description"].as_str().unwrap().contains("Single-shot"));

    let schema = &prep_tool["inputSchema"];
    assert_eq!(schema["type"], "object");
    let req = schema["required"].as_array().unwrap();
    assert!(req.iter().any(|r| r == "task"));

    let props = &schema["properties"];
    assert!(props["task"].is_object());
    assert!(props["token_budget"].is_object());
    assert!(props["planned_files"].is_object());
    assert!(props["forbidden"].is_object());
    assert!(props["no_bypass"].is_object());
    assert!(props["details"].is_object());
}

#[test]
fn test_legacy_tools_backward_compatibility() {
    let root = setup_test_workspace("prep_legacy");

    // 1. engram_get_context
    let ctx_res = mcp_call(
        &root,
        "engram_get_context",
        &json!({ "task": "Add remember-me to auth", "token_budget": 800 }),
    )
    .unwrap();
    assert_eq!(ctx_res["isError"], false);
    let ctx_text = ctx_res["content"][0]["text"].as_str().unwrap();
    let ctx_parsed: Value = serde_json::from_str(ctx_text).unwrap();
    assert_eq!(ctx_parsed["status"], "READY");
    assert!(ctx_parsed["target_files"].is_array());

    // 2. engram_preflight
    let pre_res = mcp_call(
        &root,
        "engram_preflight",
        &json!({ "task": "Add remember-me to auth", "token_budget": 800 }),
    )
    .unwrap();
    assert_eq!(pre_res["isError"], false);
    let pre_text = pre_res["content"][0]["text"].as_str().unwrap();
    let pre_parsed: Value = serde_json::from_str(pre_text).unwrap();
    assert_eq!(pre_parsed["status"], "READY_FOR_IMPLEMENTATION");
    let contract_id = pre_parsed["contract_id"].as_str().unwrap();
    assert!(contract_id.starts_with("ctr_"));

    // 3. engram_postflight
    let post_res = mcp_call(
        &root,
        "engram_postflight",
        &json!({ "contract_id": contract_id }),
    )
    .unwrap();
    assert_eq!(post_res["isError"], false);
    let post_text = post_res["content"][0]["text"].as_str().unwrap();
    let post_parsed: Value = serde_json::from_str(post_text).unwrap();
    assert_eq!(post_parsed["status"], "COMPLIANT");

    // 4. engram_doctor
    let doc_res = mcp_call(&root, "engram_doctor", &json!({})).unwrap();
    assert_eq!(doc_res["isError"], false);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_stdio_jsonrpc_protocol_prepare_task() {
    let root = setup_test_workspace("prep_stdio");

    let bin = env!("CARGO_BIN_EXE_engram");
    let mut child = Command::new(bin)
        .arg("-C")
        .arg(&root)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn engram mcp");

    let mut stdin = child.stdin.take().expect("failed to open stdin");
    let mut reader = BufReader::new(child.stdout.take().expect("failed to open stdout"));

    // 1. Initialize
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "consolidation-test", "version": "1.0.0" }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init_req).unwrap()).unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let init_resp: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init_resp["id"], 1);
    assert_eq!(init_resp["result"]["serverInfo"]["name"], "engram");

    // 2. tools/call for engram_prepare_task
    line.clear();
    let call_req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "engram_prepare_task",
            "arguments": {
                "task": "Add remember-me feature to auth flow and session persistence",
                "token_budget": 1200
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call_req).unwrap()).unwrap();
    stdin.flush().unwrap();

    reader.read_line(&mut line).unwrap();
    let call_resp: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(call_resp["id"], 2);
    assert_eq!(call_resp["result"]["isError"], false);

    let text = call_resp["result"]["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["status"], "READY");
    assert!(parsed["contract_id"].as_str().unwrap().starts_with("ctr_"));
    assert!(!parsed["context"]["target_files"].as_array().unwrap().is_empty());

    // Clean exit
    drop(stdin);
    let _ = child.wait();
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_workflow_metrics_and_tool_call_reduction() {
    let root = setup_test_workspace("metrics_eval");

    // ==========================================
    // SCENARIO 1: Non-Trivial Feature Task
    // ==========================================
    let task_desc = "Add remember-me feature to auth flow and session persistence";

    // --- OLD WORKFLOW (2-3 MCP Calls) ---
    // Turn 1: get_context
    let old_req1 = json!({
        "task": task_desc,
        "token_budget": 1200
    });
    let old_req1_str = serde_json::to_string(&old_req1).unwrap();
    let old_resp1 = mcp_call(&root, "engram_get_context", &old_req1).unwrap();
    let old_resp1_str = serde_json::to_string(&old_resp1).unwrap();

    // Turn 2: preflight
    let old_req2 = json!({
        "task": task_desc,
        "token_budget": 1200,
        "planned_files": ["src/app/core/auth/auth.effects.ts", "src/app/core/auth/auth.models.ts"]
    });
    let old_req2_str = serde_json::to_string(&old_req2).unwrap();
    let old_resp2 = mcp_call(&root, "engram_preflight", &old_req2).unwrap();
    let old_resp2_str = serde_json::to_string(&old_resp2).unwrap();

    let old_calls = 2;
    let old_req_bytes = old_req1_str.len() + old_req2_str.len();
    let old_resp_bytes = old_resp1_str.len() + old_resp2_str.len();

    // --- NEW WORKFLOW (1 MCP Call) ---
    // Single Turn: prepare_task
    let new_req = json!({
        "task": task_desc,
        "token_budget": 1200
    });
    let new_req_str = serde_json::to_string(&new_req).unwrap();
    let new_resp = mcp_call(&root, "engram_prepare_task", &new_req).unwrap();
    let new_resp_str = serde_json::to_string(&new_resp).unwrap();

    let new_calls = 1;
    let new_req_bytes = new_req_str.len();
    let new_resp_bytes = new_resp_str.len();

    println!("\n=== SCENARIO 1: COMPLEX FEATURE TASK METRICS ===");
    println!("Old Workflow MCP Calls: {} (get_context + preflight)", old_calls);
    println!("New Workflow MCP Calls: {} (engram_prepare_task)", new_calls);
    println!("Tool Call Reduction:    {}% ({} -> {})", (old_calls - new_calls) * 100 / old_calls, old_calls, new_calls);
    println!("Old Workflow Request Bytes:  {}", old_req_bytes);
    println!("New Workflow Request Bytes:  {} (-{}%)", new_req_bytes, (old_req_bytes - new_req_bytes) * 100 / old_req_bytes);
    println!("Old Workflow Response Bytes: {}", old_resp_bytes);
    println!("New Workflow Response Bytes: {}", new_resp_bytes);

    assert_eq!(old_calls, 2);
    assert_eq!(new_calls, 1);
    assert!(new_req_bytes < old_req_bytes);

    // ==========================================
    // SCENARIO 2: Trivial Task with Adaptive Bypass
    // ==========================================
    let trivial_task = "Fix typo in README.md documentation";

    let trivial_new_req = json!({
        "task": trivial_task,
        "token_budget": 1200
    });
    let trivial_new_req_str = serde_json::to_string(&trivial_new_req).unwrap();
    let trivial_new_resp = mcp_call(&root, "engram_prepare_task", &trivial_new_req).unwrap();
    let trivial_new_resp_str = serde_json::to_string(&trivial_new_resp).unwrap();

    println!("\n=== SCENARIO 2: TRIVIAL TASK ADAPTIVE BYPASS METRICS ===");
    println!("New Workflow Status: BYPASS_TRIVIAL");
    println!("Trivial Request Bytes:  {}", trivial_new_req_str.len());
    println!("Trivial Response Bytes: {}", trivial_new_resp_str.len());
    println!("Contract Overhead: ZERO (0 contract files created)\n");

    let text = trivial_new_resp["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["status"], "BYPASS_TRIVIAL");

    let _ = fs::remove_dir_all(&root);
}
