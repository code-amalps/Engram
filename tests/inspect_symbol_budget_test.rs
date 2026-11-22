use std::fs;
use std::path::PathBuf;
use serde_json::Value;

use engram::analyzer::analyze;
use engram::storage::db;
use engram::impact::{
    compute_impact_cone_with_budget, format_impact_explanation,
    ImpactBudgetOptions,
};
use engram::mcp::mcp_call;

fn setup_test_workspace(name: &str) -> PathBuf {
    let ws = std::env::temp_dir().join(format!("engram_inspect_budget_{}_{}", name, uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();
    ws
}

#[test]
fn test_inspect_symbol_empty_results_and_invalid_target() {
    let ws = setup_test_workspace("empty_invalid");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("main.rs"), "fn main() {}\n").unwrap();
    analyze(&ws).unwrap();

    // 1. Empty target rejection
    let empty_args = serde_json::json!({
        "target": ""
    });
    let res = mcp_call(&ws, "engram_inspect_symbol", &empty_args);
    assert!(res.is_err(), "Empty target must return error");

    // 2. Non-existent symbol rejection
    let non_existent_args = serde_json::json!({
        "target": "NonExistentSymbolxyz123"
    });
    let res2 = mcp_call(&ws, "engram_inspect_symbol", &non_existent_args);
    assert!(res2.is_err(), "Non-existent target must return error");

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_excessive_dependency_fanout_pruning() {
    let ws = setup_test_workspace("fanout_pruning");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // Create a hub service
    fs::write(
        src.join("hub.ts"),
        r#"export class HubService {
  execute() { return 42; }
}
"#,
    ).unwrap();

    // Create 15 dependent services that import HubService
    for i in 1..=15 {
        fs::write(
            src.join(format!("dep_{:02}.ts", i)),
            format!(
                r#"import {{ HubService }} from './hub';
export class Dep{}Service {{
  constructor(private hub: HubService) {{}}
}}
"#,
                i
            ),
        ).unwrap();
    }

    analyze(&ws).unwrap();
    let conn = db(&ws).unwrap();

    // 1. Direct engine test with ImpactBudgetOptions max_entities = 5
    let opts = ImpactBudgetOptions {
        max_depth: 2,
        max_entities_per_category: 5,
        max_relationships_per_entity: 3,
        prioritize_workspace: true,
    };
    let (cone, omissions) = compute_impact_cone_with_budget(&conn, &ws, "src/hub.ts", opts).unwrap();

    assert_eq!(
        cone.import_dependent_entities.len(),
        5,
        "import_dependent_entities must be pruned to max_entities_per_category = 5"
    );
    assert_eq!(
        omissions.len(),
        10,
        "10 excess entities must be recorded in omission manifest"
    );
    for om in &omissions {
        assert_eq!(om.context_type, "IMPORT_DEPENDENT");
        assert_eq!(om.omission_status, "CAPACITY_PRUNED");
        assert!(om.retrievable_via_targeted_request);
    }

    // 2. MCP tool invocation test
    let mcp_args = serde_json::json!({
        "target": "src/hub.ts",
        "depth": 2,
        "max_entities": 4
    });
    let res = mcp_call(&ws, "engram_inspect_symbol", &mcp_args).unwrap();
    let content_text = res["content"][0]["text"].as_str().unwrap();
    let parsed: Value = serde_json::from_str(content_text).unwrap();

    // Verify bounded entities in MCP result
    let import_deps = parsed["impact_cone"]["import_dependent_entities"].as_array().unwrap();
    assert_eq!(import_deps.len(), 4, "MCP output must respect max_entities: 4");

    // Verify omission manifest in MCP result
    let manifest = &parsed["omission_manifest"];
    assert_eq!(manifest["budget_status"].as_str().unwrap(), "OMISSIONS_RECORDED");
    assert_eq!(manifest["omitted_items_count"].as_i64().unwrap(), 11);

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_external_dependencies_prioritization() {
    let ws = setup_test_workspace("external_prioritization");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("service.ts"),
        r#"import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { InternalHelper } from './helper';

@Injectable()
export class MainService {
  constructor(private helper: InternalHelper) {}
}
"#,
    ).unwrap();

    fs::write(
        src.join("helper.ts"),
        r#"export class InternalHelper {
  run() {}
}
"#,
    ).unwrap();

    analyze(&ws).unwrap();
    let conn = db(&ws).unwrap();

    // Query with prioritization enabled
    let opts = ImpactBudgetOptions {
        max_depth: 2,
        max_entities_per_category: 10,
        max_relationships_per_entity: 5,
        prioritize_workspace: true,
    };
    let (cone, _) = compute_impact_cone_with_budget(&conn, &ws, "src/service.ts", opts).unwrap();

    // Direct entities must be non-empty and workspace first
    assert!(!cone.direct_entities.is_empty());
    assert!(!cone.direct_entities[0].is_external);

    // Verify format explanation is produced and valid
    let exp = format_impact_explanation(&cone);
    assert!(exp.contains("TARGET"));
    assert!(exp.contains("MainService") || exp.contains("service.ts"));

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_duplicate_entities_and_relationships_deduplication() {
    let ws = setup_test_workspace("dedup_rels");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // Diamond dependency pattern
    fs::write(src.join("base.ts"), "export class BaseClass {}\n").unwrap();
    fs::write(src.join("left.ts"), "import { BaseClass } from './base'; export class Left extends BaseClass {}\n").unwrap();
    fs::write(src.join("right.ts"), "import { BaseClass } from './base'; export class Right extends BaseClass {}\n").unwrap();
    fs::write(
        src.join("top.ts"),
        "import { Left } from './left'; import { Right } from './right'; export class Top { l = new Left(); r = new Right(); }\n",
    ).unwrap();

    analyze(&ws).unwrap();
    let conn = db(&ws).unwrap();

    let opts = ImpactBudgetOptions {
        max_depth: 2,
        max_entities_per_category: 10,
        max_relationships_per_entity: 5,
        prioritize_workspace: true,
    };
    let (cone, _) = compute_impact_cone_with_budget(&conn, &ws, "src/base.ts", opts).unwrap();

    // Check that each entity has deduplicated relationships
    for ent in cone.all_entities() {
        let mut seen = std::collections::HashSet::new();
        for r in &ent.relationships {
            let key = format!("{}:{}:{}:{}", r.source, r.target, r.category.as_str(), r.provenance);
            assert!(seen.insert(key), "Duplicate relationship found in entity {}: {:?}", ent.entity_id, r);
        }
    }

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_oversized_response_bounded_and_valid_json() {
    let ws = setup_test_workspace("oversized_bounded");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // Generate a hub component with large docstrings and multiple dependencies
    let mut large_comment = String::new();
    for i in 0..500 {
        large_comment.push_str(&format!("/// Architectural note line {}\n", i));
    }

    fs::write(
        src.join("root.ts"),
        format!(
            r#"{large_comment}
export class RootComponent {{
  render() {{ return 'hello'; }}
}}
"#
        ),
    ).unwrap();

    for i in 1..=20 {
        fs::write(
            src.join(format!("consumer_{}.ts", i)),
            format!("import {{ RootComponent }} from './root';\nexport const c{} = new RootComponent();\n", i),
        ).unwrap();
    }

    analyze(&ws).unwrap();

    // Invoke engram_inspect_symbol with a strict 4096 byte ceiling
    let mcp_args = serde_json::json!({
        "target": "src/root.ts",
        "depth": 2,
        "max_entities": 15,
        "max_bytes": 4096
    });

    let res = mcp_call(&ws, "engram_inspect_symbol", &mcp_args).unwrap();
    let inner_str = res["content"][0]["text"].as_str().expect("Tool response content must be string");
    
    // 2. Inner payload must deserialize cleanly (valid structured JSON under truncation)
    let inner_val: Value = serde_json::from_str(inner_str).expect("Inner tool JSON must be strictly valid JSON");

    // 3. Serialized byte size must be within configured byte ceiling
    let actual_bytes = inner_str.len();
    assert!(
        actual_bytes <= 4096,
        "Serialized inner payload ({} bytes) must strictly obey byte ceiling 4096",
        actual_bytes
    );

    // 4. Required keys must be preserved
    assert_eq!(inner_val["target"].as_str().unwrap(), "src/root.ts");
    assert!(inner_val.get("impact_cone").is_some());
    assert!(inner_val.get("explanation").is_some());
    assert!(inner_val.get("omission_manifest").is_some());

    // 5. Omission manifest must record omissions due to budget shedding
    let manifest = &inner_val["omission_manifest"];
    assert_eq!(manifest["budget_status"].as_str().unwrap(), "OMISSIONS_RECORDED");
    assert!(manifest["omitted_items_count"].as_i64().unwrap() > 0);

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_small_response_within_16k_limit() {
    let ws = setup_test_workspace("small_response");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("simple.ts"),
        "export function add(a: number, b: number): number { return a + b; }\n",
    ).unwrap();

    analyze(&ws).unwrap();

    let mcp_args = serde_json::json!({
        "target": "src/simple.ts",
        "depth": 1,
        "max_entities": 5,
        "max_bytes": 16384
    });

    let res = mcp_call(&ws, "engram_inspect_symbol", &mcp_args).unwrap();
    let outer_bytes = serde_json::to_vec(&res).unwrap();
    assert!(
        outer_bytes.len() <= 16384,
        "Final outer serialized MCP response size ({} bytes) must be <= 16384",
        outer_bytes.len()
    );

    let inner_str = res["content"][0]["text"].as_str().expect("Tool response content must be string");
    assert!(
        inner_str.len() <= 16384,
        "Inner serialized JSON ({} bytes) must be <= 16384",
        inner_str.len()
    );

    let inner_val: Value = serde_json::from_str(inner_str).expect("Inner tool JSON must be strictly valid JSON");
    assert_eq!(inner_val["target"].as_str().unwrap(), "src/simple.ts");
    assert_eq!(
        inner_val["omission_manifest"]["budget_status"].as_str().unwrap(),
        "PACKED_WITHIN_BUDGET"
    );
    assert_eq!(
        inner_val["omission_manifest"]["omitted_items_count"].as_i64().unwrap(),
        0
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_immediately_below_limit() {
    let ws = setup_test_workspace("immediately_below");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // Sized payload that stays below 4096 bytes without needing pruning
    fs::write(
        src.join("service.ts"),
        r#"export class ModerateService {
  execute() { return 'ok'; }
  helper() { return 1; }
}
"#,
    ).unwrap();

    for i in 1..=2 {
        fs::write(
            src.join(format!("dep_{}.ts", i)),
            format!("import {{ ModerateService }} from './service';\nexport const s{} = new ModerateService();\n", i),
        ).unwrap();
    }

    analyze(&ws).unwrap();

    let mcp_args = serde_json::json!({
        "target": "src/service.ts",
        "depth": 1,
        "max_entities": 5,
        "max_bytes": 4096
    });

    let res = mcp_call(&ws, "engram_inspect_symbol", &mcp_args).unwrap();
    let outer_bytes = serde_json::to_vec(&res).unwrap();
    assert!(
        outer_bytes.len() <= 4096,
        "Final outer serialized MCP response size ({} bytes) must be <= 4096",
        outer_bytes.len()
    );

    let inner_str = res["content"][0]["text"].as_str().unwrap();
    assert!(inner_str.len() <= 4096);

    let inner_val: Value = serde_json::from_str(inner_str).expect("Must parse cleanly");
    assert_eq!(
        inner_val["omission_manifest"]["budget_status"].as_str().unwrap(),
        "PACKED_WITHIN_BUDGET"
    );
    assert_eq!(
        inner_val["omission_manifest"]["omitted_items_count"].as_i64().unwrap(),
        0
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_immediately_above_limit_sheds_cleanly() {
    let ws = setup_test_workspace("immediately_above");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // Create a service with several dependencies that naturally exceed 3000 bytes
    fs::write(
        src.join("service.ts"),
        r#"export class HubService {
  run() { return 'hub'; }
}
"#,
    ).unwrap();

    for i in 1..=8 {
        fs::write(
            src.join(format!("dep_{:02}.ts", i)),
            format!("import {{ HubService }} from './service';\nexport class Client{} {{ constructor(h: HubService) {{}} }}\n", i),
        ).unwrap();
    }

    analyze(&ws).unwrap();

    // Set a ceiling of 2500 bytes - immediately below the natural unpruned response
    let mcp_args = serde_json::json!({
        "target": "src/service.ts",
        "depth": 2,
        "max_entities": 10,
        "max_bytes": 2500
    });

    let res = mcp_call(&ws, "engram_inspect_symbol", &mcp_args).unwrap();
    let outer_bytes = serde_json::to_vec(&res).unwrap();
    assert!(
        outer_bytes.len() <= 2500,
        "Final outer serialized MCP response size ({} bytes) must be <= 2500",
        outer_bytes.len()
    );

    let inner_str = res["content"][0]["text"].as_str().unwrap();
    assert!(inner_str.len() <= 2500, "Inner string ({} bytes) must be <= 2500", inner_str.len());

    let inner_val: Value = serde_json::from_str(inner_str).expect("Truncated JSON must remain syntactically valid");
    assert_eq!(
        inner_val["omission_manifest"]["budget_status"].as_str().unwrap(),
        "OMISSIONS_RECORDED"
    );
    assert!(inner_val["omission_manifest"]["omitted_items_count"].as_i64().unwrap() > 0);

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_extremely_oversized_response() {
    let ws = setup_test_workspace("extremely_oversized");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // Create a huge component with extensive comments and 40 consumer files
    let mut large_doc = String::new();
    for i in 0..300 {
        large_doc.push_str(&format!("/// Enterprise architecture documentation paragraph {}\n", i));
    }

    fs::write(
        src.join("giant.ts"),
        format!(
            r#"{large_doc}
export class GiantEnterpriseService {{
  execute() {{ return 1000; }}
}}
"#
        ),
    ).unwrap();

    for i in 1..=40 {
        fs::write(
            src.join(format!("consumer_{:03}.ts", i)),
            format!("import {{ GiantEnterpriseService }} from './giant';\nexport const x{} = new GiantEnterpriseService();\n", i),
        ).unwrap();
    }

    analyze(&ws).unwrap();

    // Default required limit: 16,384 bytes
    let mcp_args = serde_json::json!({
        "target": "src/giant.ts",
        "depth": 2,
        "max_entities": 30,
        "max_bytes": 16384
    });

    let res = mcp_call(&ws, "engram_inspect_symbol", &mcp_args).unwrap();
    let outer_bytes = serde_json::to_vec(&res).unwrap();
    assert!(
        outer_bytes.len() <= 16384,
        "Final outer serialized MCP response ({} bytes) must be <= 16384",
        outer_bytes.len()
    );

    let inner_str = res["content"][0]["text"].as_str().unwrap();
    assert!(inner_str.len() <= 16384);

    let inner_val: Value = serde_json::from_str(inner_str).expect("Must remain strictly valid JSON under extreme shedding");
    assert_eq!(inner_val["target"].as_str().unwrap(), "src/giant.ts");
    assert_eq!(
        inner_val["omission_manifest"]["budget_status"].as_str().unwrap(),
        "OMISSIONS_RECORDED"
    );
    assert!(inner_val["omission_manifest"]["omitted_items_count"].as_i64().unwrap() > 0);

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_large_code_excerpts() {
    let ws = setup_test_workspace("large_code_excerpts");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    let mut body = String::new();
    for i in 0..400 {
        body.push_str(&format!("    let var_{} = {};\n", i, i));
    }

    fs::write(
        src.join("large_fn.ts"),
        format!(
            r#"export function computeLargeAlgorithm(): number {{
{body}
    return 42;
}}
"#
        ),
    ).unwrap();

    analyze(&ws).unwrap();

    let mcp_args = serde_json::json!({
        "target": "src/large_fn.ts",
        "depth": 1,
        "max_entities": 5,
        "max_bytes": 1500
    });

    let res = mcp_call(&ws, "engram_inspect_symbol", &mcp_args).unwrap();
    let outer_bytes = serde_json::to_vec(&res).unwrap();
    assert!(
        outer_bytes.len() <= 1500,
        "Final outer serialized MCP response ({} bytes) must be <= 1500",
        outer_bytes.len()
    );

    let inner_str = res["content"][0]["text"].as_str().unwrap();
    assert!(inner_str.len() <= 1500);

    let inner_val: Value = serde_json::from_str(inner_str).expect("Must parse cleanly");
    // Code excerpt should be contracted to fit within budget
    if let Some(exc) = inner_val["code_excerpt"].as_str() {
        assert!(exc.len() <= 500, "Excerpt must be contracted to fit within budget (was {})", exc.len());
    }

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_large_omission_manifests_capped() {
    let ws = setup_test_workspace("large_omissions_capped");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(src.join("hub.ts"), "export class CoreHub {}\n").unwrap();

    // 50 dependent files to cause a huge omission manifest
    for i in 1..=50 {
        fs::write(
            src.join(format!("dep_{:03}.ts", i)),
            format!("import {{ CoreHub }} from './hub';\nexport class Client{} extends CoreHub {{}}\n", i),
        ).unwrap();
    }

    analyze(&ws).unwrap();

    let mcp_args = serde_json::json!({
        "target": "src/hub.ts",
        "depth": 2,
        "max_entities": 5,
        "max_bytes": 8192
    });

    let res = mcp_call(&ws, "engram_inspect_symbol", &mcp_args).unwrap();
    let outer_bytes = serde_json::to_vec(&res).unwrap();
    assert!(outer_bytes.len() <= 8192);

    let inner_str = res["content"][0]["text"].as_str().unwrap();
    let inner_val: Value = serde_json::from_str(inner_str).expect("Valid JSON");

    let manifest = &inner_val["omission_manifest"];
    let omitted_count = manifest["omitted_items_count"].as_i64().unwrap();
    assert!(omitted_count >= 40, "Total omitted count must accurately reflect all pruned items ({})", omitted_count);

    // But sample omission list must be capped so the manifest itself doesn't exceed budget
    let sample_list = manifest["omission_manifest"].as_array().unwrap();
    assert!(
        sample_list.len() <= 5,
        "Sample omission entries must be capped to at most 5 (got {})",
        sample_list.len()
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_empty_response() {
    let ws = setup_test_workspace("empty_response");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // File with comments only (no symbols)
    fs::write(src.join("empty.ts"), "// Empty file with comments only\n").unwrap();

    analyze(&ws).unwrap();

    let mcp_args = serde_json::json!({
        "target": "src/empty.ts",
        "depth": 1,
        "max_entities": 5,
        "max_bytes": 16384
    });

    let res = mcp_call(&ws, "engram_inspect_symbol", &mcp_args).unwrap();
    let outer_bytes = serde_json::to_vec(&res).unwrap();
    assert!(outer_bytes.len() <= 16384);

    let inner_str = res["content"][0]["text"].as_str().unwrap();
    let inner_val: Value = serde_json::from_str(inner_str).expect("Valid JSON");

    assert_eq!(inner_val["target"].as_str().unwrap(), "src/empty.ts");
    assert_eq!(
        inner_val["omission_manifest"]["budget_status"].as_str().unwrap(),
        "PACKED_WITHIN_BUDGET"
    );
    assert_eq!(
        inner_val["omission_manifest"]["omitted_items_count"].as_i64().unwrap(),
        0
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_inspect_symbol_serialization_failure_and_error_handling() {
    let ws = setup_test_workspace("serialization_error");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("valid.ts"), "export const x = 1;\n").unwrap();
    analyze(&ws).unwrap();

    // 1. Path traversal injection
    let path_traversal = serde_json::json!({
        "target": "../../etc/passwd"
    });
    let res1 = mcp_call(&ws, "engram_inspect_symbol", &path_traversal);
    assert!(res1.is_err(), "Path traversal must fail safely");

    // 2. Control characters / null byte in target
    let null_byte = serde_json::json!({
        "target": "src/valid\0.ts"
    });
    let res2 = mcp_call(&ws, "engram_inspect_symbol", &null_byte);
    assert!(res2.is_err(), "Null byte target must fail safely");

    let _ = fs::remove_dir_all(&ws);
}

