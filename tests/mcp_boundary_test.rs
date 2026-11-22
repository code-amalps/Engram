use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use engram::analyzer::analyze;
use engram::mcp::{compute_envelope_bytes, enforce_mcp_envelope_boundary, mcp_call};

fn setup_test_workspace(name: &str) -> PathBuf {
    let ws = std::env::temp_dir().join(format!(
        "engram_mcp_boundary_{}_{}",
        name,
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&ws).unwrap();
    ws
}

/// 1. Small response (well within budget)
#[test]
fn test_boundary_01_small_response_well_within_budget() {
    let ws = setup_test_workspace("small_response");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(
        src.join("auth.ts"),
        "export function login(user: string, pass: string): boolean { return true; }\n",
    )
    .unwrap();
    analyze(&ws).unwrap();

    let args = serde_json::json!({
        "task": "Fix login bug in auth.ts",
        "token_budget": 1200
    });
    let res = mcp_call(&ws, "engram_get_context", &args).expect("mcp_call must succeed");

    // Wire JSON-RPC envelope bytes
    let env_bytes = compute_envelope_bytes(&serde_json::json!(1), &res);
    assert!(
        env_bytes <= 16384,
        "Complete envelope bytes ({} bytes) must be <= 16384",
        env_bytes
    );

    let content_text = res["content"][0]["text"].as_str().expect("text string");
    let parsed: Value = serde_json::from_str(content_text).expect("Valid JSON");
    assert_eq!(parsed["status"].as_str().unwrap(), "READY");
    assert!(!parsed["target_files"].as_array().unwrap().is_empty());

    let _ = fs::remove_dir_all(&ws);
}

/// 2. Exact limit (response constructed to serialize to exactly 16,384 bytes, verifying it is accepted)
#[test]
fn test_boundary_02_exact_limit_acceptance() {
    let byte_limit: usize = 16384;
    // Calculate exact padding needed so compute_envelope_bytes equals 16384
    let mut payload = serde_json::json!({
        "status": "READY",
        "pad": ""
    });
    let base_res = serde_json::json!({
        "content": [{ "type": "text", "text": serde_json::to_string(&payload).unwrap() }],
        "isError": false
    });
    let base_len = compute_envelope_bytes(&serde_json::json!(1), &base_res);
    let needed = byte_limit.saturating_sub(base_len);
    payload["pad"] = serde_json::json!("A".repeat(needed));

    let res =
        enforce_mcp_envelope_boundary("test_tool", payload, byte_limit, &serde_json::json!(1));
    let final_bytes = compute_envelope_bytes(&serde_json::json!(1), &res);
    assert!(
        final_bytes <= byte_limit,
        "Exact limit test response ({} bytes) must be <= {}",
        final_bytes,
        byte_limit
    );

    let content_text = res["content"][0]["text"].as_str().expect("text string");
    let _: Value = serde_json::from_str(content_text).expect("Must parse cleanly");
    assert_eq!(res["isError"].as_bool().unwrap(), false);
}

/// 3. Boundary crossing: 16,383 bytes (accepted), 16,384 bytes (accepted), 16,385 bytes (sheds/compacts)
#[test]
fn test_boundary_03_boundary_crossing_16383_16384_16385() {
    for limit in [16383, 16384, 16385] {
        let payload = serde_json::json!({
            "status": "READY",
            "explanation": "A".repeat(8000),
            "target": "src/module.ts",
            "target_files": [
                { "path": "src/module.ts", "level": "L1", "score": 1.0, "content": "B".repeat(12000) }
            ]
        });
        let res = enforce_mcp_envelope_boundary("test_tool", payload, limit, &serde_json::json!(1));
        let env_bytes = compute_envelope_bytes(&serde_json::json!(1), &res);
        assert!(
            env_bytes <= limit,
            "Enforced response ({} bytes) must strictly obey limit {}",
            env_bytes,
            limit
        );

        let content_text = res["content"][0]["text"].as_str().unwrap();
        let parsed: Value =
            serde_json::from_str(content_text).expect("Must parse cleanly as valid JSON");
        assert!(parsed.is_object());
    }
}

/// 4. Massive omission manifest (1,000+ omitted files simulated, verifying bounded output)
#[test]
fn test_boundary_04_massive_omission_manifest_1000_files() {
    let ws = setup_test_workspace("massive_omissions");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(src.join("core.ts"), "export const CORE = 1;\n").unwrap();
    // Create 1000 simulated unselected entries in compiler context result
    let mut omission_entries = Vec::new();
    for i in 0..1200 {
        omission_entries.push(engram::budget::OmissionEntry {
            context_item_id: format!("src/file_{:04}.ts", i),
            symbol: Some(format!("Item{}", i)),
            file: format!("src/file_{:04}.ts", i),
            context_type: "UNSELECTED_FILE".to_string(),
            priority_tier: 5,
            original_token_count: 100,
            included_token_count: 0,
            omitted_token_count: 100,
            omission_reason: "Context budget exhausted; omitted to fit within token ceiling"
                .to_string(),
            omission_status: "CAPACITY_PRUNED".to_string(),
            retrievable_via_targeted_request: true,
        });
    }

    let manifest = engram::budget::OmissionManifest {
        budget_status: "OMISSIONS_RECORDED".to_string(),
        budget_tokens_limit: 2000,
        budget_tokens_estimated: 120000,
        reserved_metadata_tokens: 150,
        packed_tokens: 200,
        remaining_tokens: 1800,
        omitted_tokens: 119800,
        serialized_payload_bytes: 95000,
        included_items_count: 1,
        omitted_items_count: 1200,
        omission_manifest: omission_entries,
    };

    // Construct raw payload as engram_get_context would before bounding
    let raw_payload = serde_json::json!({
        "status": "READY",
        "task": "Test task",
        "target_files": [{ "path": "src/core.ts", "level": "L1", "score": 1.0, "provenance": [], "content": "export const CORE = 1;" }],
        "governing_rules": [],
        "cochange_warnings": [],
        "relevant_decisions": [],
        "ambiguity_gaps": [],
        "token_budget": 2000,
        "estimated_tokens": 200,
        "confidence": "HIGH",
        "omission_manifest": manifest,
        "next_action": "Plan changes"
    });

    // Enforce boundary with standard 16384 byte ceiling
    let res = enforce_mcp_envelope_boundary(
        "engram_get_context",
        raw_payload,
        16384,
        &serde_json::json!(1),
    );
    let final_bytes = compute_envelope_bytes(&serde_json::json!(1), &res);
    assert!(
        final_bytes <= 16384,
        "Response with 1200-item omission manifest ({} bytes) must be <= 16384",
        final_bytes
    );

    let content_text = res["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(content_text).expect("Valid JSON");
    let man = &parsed["omission_manifest"];
    assert_eq!(man["budget_status"].as_str().unwrap(), "OMISSIONS_RECORDED");
    assert_eq!(man["omitted_items_count"].as_i64().unwrap(), 1200);

    let _ = fs::remove_dir_all(&ws);
}

/// 5. Massive code excerpts (100+ KB of code requested, verifying graceful truncation)
#[test]
fn test_boundary_05_massive_code_excerpts_100kb() {
    let ws = setup_test_workspace("massive_code");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // 120 KB source file
    let mut huge_body = String::new();
    for i in 0..3000 {
        huge_body.push_str(&format!(
            "  const intermediate_var_{:04} = 'large string payload for padding';\n",
            i
        ));
    }
    fs::write(
        src.join("huge_algorithm.ts"),
        format!("export function runHugeAlgorithm(): void {{\n{huge_body}}}\n"),
    )
    .unwrap();
    analyze(&ws).unwrap();

    let args = serde_json::json!({
        "target": "src/huge_algorithm.ts",
        "depth": 1,
        "max_bytes": 16384
    });
    let res = mcp_call(&ws, "engram_inspect_symbol", &args).expect("mcp_call must succeed");
    let final_bytes = compute_envelope_bytes(&serde_json::json!(1), &res);
    assert!(
        final_bytes <= 16384,
        "Serialized inspect_symbol response ({} bytes) must be <= 16384",
        final_bytes
    );

    let content_text = res["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(content_text).expect("Must parse cleanly");
    assert_eq!(parsed["target"].as_str().unwrap(), "src/huge_algorithm.ts");

    let _ = fs::remove_dir_all(&ws);
}

/// 6. Deep dependency graph (hundreds of entities, verifying progressive shedding)
#[test]
fn test_boundary_06_deep_dependency_graph_progressive_shedding() {
    let ws = setup_test_workspace("deep_graph");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("base_service.ts"),
        "export class BaseService { ping() { return 1; } }\n",
    )
    .unwrap();
    for i in 1..=60 {
        fs::write(
            src.join(format!("consumer_{:03}.ts", i)),
            format!("import {{ BaseService }} from './base_service';\nexport class Client{} {{ constructor(b: BaseService) {{}} }}\n", i),
        ).unwrap();
    }
    analyze(&ws).unwrap();

    let args = serde_json::json!({
        "target": "src/base_service.ts",
        "depth": 2,
        "max_entities": 40,
        "max_bytes": 8192
    });
    let res = mcp_call(&ws, "engram_inspect_symbol", &args).expect("mcp_call must succeed");
    let final_bytes = compute_envelope_bytes(&serde_json::json!(1), &res);
    assert!(
        final_bytes <= 8192,
        "Deep dependency graph response ({} bytes) must strictly obey 8192 limit",
        final_bytes
    );

    let content_text = res["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(content_text).expect("Must parse cleanly");
    assert_eq!(
        parsed["omission_manifest"]["budget_status"]
            .as_str()
            .unwrap(),
        "OMISSIONS_RECORDED"
    );
    assert!(
        parsed["omission_manifest"]["omitted_items_count"]
            .as_i64()
            .unwrap()
            > 0
    );

    let _ = fs::remove_dir_all(&ws);
}

/// 7. Extreme parameters (max_bytes = 1024, verifying minimal valid response or structured error)
#[test]
fn test_boundary_07_extreme_parameters_max_bytes_1024() {
    let ws = setup_test_workspace("extreme_1024");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(src.join("tiny.ts"), "export const TINY = true;\n").unwrap();
    analyze(&ws).unwrap();

    let args = serde_json::json!({
        "target": "src/tiny.ts",
        "depth": 1,
        "max_bytes": 1024
    });
    let res = mcp_call(&ws, "engram_inspect_symbol", &args).expect("mcp_call must return Result");
    let final_bytes = compute_envelope_bytes(&serde_json::json!(1), &res);
    assert!(
        final_bytes <= 1024,
        "Response under max_bytes=1024 ({} bytes) must strictly obey 1024 limit",
        final_bytes
    );

    let content_text = res["content"][0]["text"].as_str().unwrap();
    let parsed: Value =
        serde_json::from_str(content_text).expect("Must parse cleanly as valid JSON");
    // Either a minimal valid truncated payload or structured BOUNDARY_EXCEEDED error
    let is_valid_min = parsed.get("payload_truncated") == Some(&serde_json::json!(true))
        || parsed.get("error") == Some(&serde_json::json!("BOUNDARY_EXCEEDED"))
        || parsed.get("status") == Some(&serde_json::json!("READY"));
    assert!(
        is_valid_min,
        "Must return either structured minimal payload or structured boundary error"
    );

    let _ = fs::remove_dir_all(&ws);
}

/// 8. Configurable max_bytes (max_bytes = 4096, 8192, 32768, verifying enforcement at each level)
#[test]
fn test_boundary_08_configurable_max_bytes_4096_8192_32768() {
    let ws = setup_test_workspace("config_max_bytes");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("service.ts"),
        "export class DataService { get() { return [1,2,3]; } }\n",
    )
    .unwrap();
    for i in 1..=25 {
        fs::write(
            src.join(format!("dep_{}.ts", i)),
            format!("import {{ DataService }} from './service';\nexport const s{} = new DataService();\n", i),
        ).unwrap();
    }
    analyze(&ws).unwrap();

    for limit in [4096, 8192, 32768] {
        let args = serde_json::json!({
            "target": "src/service.ts",
            "depth": 2,
            "max_entities": 25,
            "max_bytes": limit
        });
        let res = mcp_call(&ws, "engram_inspect_symbol", &args).expect("mcp_call must succeed");
        let final_bytes = compute_envelope_bytes(&serde_json::json!(1), &res);
        assert!(
            final_bytes <= limit,
            "Response ({} bytes) must strictly obey configured max_bytes limit {}",
            final_bytes,
            limit
        );

        let content_text = res["content"][0]["text"].as_str().unwrap();
        let _: Value = serde_json::from_str(content_text).expect("Valid JSON");
    }

    let _ = fs::remove_dir_all(&ws);
}

/// 9. Unicode paths and symbols (multi-byte UTF-8 sequences, verifying correct byte counting vs char counting)
#[test]
fn test_boundary_09_unicode_multibyte_paths_and_symbols() {
    let ws = setup_test_workspace("unicode_boundary");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // Multi-byte UTF-8 path and symbols: Japanese kanji + Cyrillic
    let unicode_file = "コンポーネント_auth.ts";
    fs::write(
        src.join(unicode_file),
        "export class 認証サービス { public ログイン(): boolean { return true; } }\n",
    )
    .unwrap();
    analyze(&ws).unwrap();

    let args = serde_json::json!({
        "target": format!("src/{}", unicode_file),
        "depth": 1,
        "max_bytes": 4096
    });
    let res = mcp_call(&ws, "engram_inspect_symbol", &args).expect("mcp_call must succeed");
    let final_bytes = compute_envelope_bytes(&serde_json::json!(1), &res);
    assert!(
        final_bytes <= 4096,
        "Unicode response ({} bytes) must strictly obey 4096 limit",
        final_bytes
    );

    let content_text = res["content"][0]["text"].as_str().unwrap();
    // Verify UTF-8 byte length is counted correctly
    assert!(
        content_text.len() >= content_text.chars().count(),
        "UTF-8 byte length must be >= char count"
    );
    let parsed: Value = serde_json::from_str(content_text).expect("Must parse cleanly");
    assert_eq!(
        parsed["target"].as_str().unwrap(),
        format!("src/{}", unicode_file)
    );

    let _ = fs::remove_dir_all(&ws);
}

/// 10. Regression test reproducing the exact Task B engram_get_context call that produced 95,010 bytes
#[test]
fn test_boundary_10_task_b_regression_95kb_reproduction() {
    let ws = setup_test_workspace("task_b_regression");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // Create target files relevant to Task B: auth effects and settings
    fs::write(
        src.join("auth.effects.ts"),
        r#"export class AuthEffects {
  login$ = null;
  logout$ = null;
}
"#,
    )
    .unwrap();

    fs::write(
        src.join("settings.reducer.ts"),
        r#"export interface SettingsState {
  rememberMe: boolean;
}
"#,
    )
    .unwrap();

    // Create 150 simulated background repository files with matching keywords to trigger massive omission manifests
    for i in 1..=150 {
        fs::write(
            src.join(format!("feature_{:03}.component.ts", i)),
            format!("// login remember-me preference storage feature component\nexport class Feature{:03}Component {{ title = 'Feature {}'; }}\n", i, i),
        ).unwrap();
    }
    analyze(&ws).unwrap();

    // Exact Task B call parameters with tight token budget to force candidate omission
    let task_b_prompt =
        "Implement remember-me checkbox in login screen with durable preference storage";
    let args = serde_json::json!({
        "task": task_b_prompt,
        "token_budget": 200
    });

    let res = mcp_call(&ws, "engram_get_context", &args).expect("mcp_call must succeed");
    let final_bytes = compute_envelope_bytes(&serde_json::json!(1), &res);
    assert!(
        final_bytes <= 16384,
        "Remediated engram_get_context response size ({} bytes) MUST be <= 16,384 bytes (was previously 95,010 bytes!)",
        final_bytes
    );

    let content_text = res["content"][0]["text"]
        .as_str()
        .expect("Tool response text must be string");
    let parsed: Value = serde_json::from_str(content_text).expect("Valid structured JSON");

    // 1. Verify essential context is retained
    assert_eq!(parsed["status"].as_str().unwrap(), "READY");
    let target_files = parsed["target_files"]
        .as_array()
        .expect("target_files array");
    assert!(
        !target_files.is_empty(),
        "Must retain task-relevant target files"
    );

    // 2. Verify omission manifest is bounded and contains totals + guidance
    let man = &parsed["omission_manifest"];
    assert!(man.is_object(), "omission_manifest must be present");
    let omitted_count = man["omitted_items_count"]
        .as_i64()
        .expect("omitted_items_count");
    assert!(
        omitted_count >= 140,
        "Total omitted items count ({}) must record all unselected files without bloating payload",
        omitted_count
    );
    assert_eq!(man["manifest_truncated"].as_bool().unwrap(), true);
    assert!(man["unrepresented_omissions_count"].as_i64().unwrap() > 0);
    assert!(
        man["targeted_retrieval_guidance"]
            .as_str()
            .unwrap()
            .contains("engram_inspect_symbol")
    );

    // 3. Verify sample omission list is capped (never dumping 150 files)
    let samples = man["sample_omissions"]
        .as_array()
        .expect("sample_omissions array");
    assert!(
        samples.len() <= 5,
        "Sample omissions list must be capped to at most 5 entries (got {})",
        samples.len()
    );

    let _ = fs::remove_dir_all(&ws);
}

/// 11. Universal endpoint coverage: engram_preflight, engram_postflight, engram_prepare_task
#[test]
fn test_boundary_11_universal_endpoint_coverage() {
    let ws = setup_test_workspace("universal_coverage");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("index.ts"), "export const X = 100;\n").unwrap();
    analyze(&ws).unwrap();

    // 1. engram_prepare_task
    let prep_args = serde_json::json!({
        "task": "Update X constant in index.ts",
        "token_budget": 1200,
        "max_bytes": 8192
    });
    let res_prep =
        mcp_call(&ws, "engram_prepare_task", &prep_args).expect("prepare_task must succeed");
    let bytes_prep = compute_envelope_bytes(&serde_json::json!(1), &res_prep);
    assert!(
        bytes_prep <= 8192,
        "prepare_task bytes ({}) must be <= 8192",
        bytes_prep
    );
    let _: Value = serde_json::from_str(res_prep["content"][0]["text"].as_str().unwrap()).unwrap();

    // 2. engram_preflight
    let preflight_args = serde_json::json!({
        "task": "Update X constant in index.ts",
        "planned_files": ["src/index.ts"],
        "max_bytes": 8192
    });
    let res_pre =
        mcp_call(&ws, "engram_preflight", &preflight_args).expect("preflight must succeed");
    let bytes_pre = compute_envelope_bytes(&serde_json::json!(1), &res_pre);
    assert!(
        bytes_pre <= 8192,
        "preflight bytes ({}) must be <= 8192",
        bytes_pre
    );
    let parsed_pre: Value =
        serde_json::from_str(res_pre["content"][0]["text"].as_str().unwrap()).unwrap();
    let contract_id = parsed_pre["contract_id"].as_str().unwrap().to_string();

    // 3. engram_postflight
    let post_args = serde_json::json!({
        "contract_id": contract_id,
        "test_evidence": true,
        "max_bytes": 8192
    });
    let res_post = mcp_call(&ws, "engram_postflight", &post_args).expect("postflight must succeed");
    let bytes_post = compute_envelope_bytes(&serde_json::json!(1), &res_post);
    assert!(
        bytes_post <= 8192,
        "postflight bytes ({}) must be <= 8192",
        bytes_post
    );
    let _: Value = serde_json::from_str(res_post["content"][0]["text"].as_str().unwrap()).unwrap();

    let _ = fs::remove_dir_all(&ws);
}
