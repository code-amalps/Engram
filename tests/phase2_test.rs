use std::fs;
use std::path::PathBuf;

use engram::analyzer::analyze;
use engram::compiler::context;
use engram::constitution::evaluate_rules;
use engram::graph::GraphTraversal;
use engram::manifest::detect_ecosystem;
use engram::model::Direction;
use engram::setup::{list_targets, setup_client};
use engram::storage::db;

fn temp_test_dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!("engram_p2_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn test_setup_targets_list() {
    let list = list_targets();
    let supported = list["supported"].as_array().unwrap();
    assert!(supported.iter().any(|s| s.as_str().unwrap().contains("cursor")));
    assert!(supported.iter().any(|s| s.as_str().unwrap().contains("claude")));
    assert!(supported.iter().any(|s| s.as_str().unwrap().contains("windsurf")));
}

#[test]
fn test_setup_safe_merge_preserves_unrelated_servers() {
    let dir = temp_test_dir();
    let cursor_dir = dir.join(".cursor");
    fs::create_dir_all(&cursor_dir).unwrap();
    let mcp_file = cursor_dir.join("mcp.json");

    // Existing third-party config
    fs::write(
        &mcp_file,
        r#"{
            "mcpServers": {
                "github": {
                    "command": "gh",
                    "args": ["mcp"]
                }
            }
        }"#,
    )
    .unwrap();

    // Run setup for cursor
    let res = setup_client(&dir, "cursor", false, false, false).unwrap();
    assert_eq!(res["configured"], true);

    // Verify both github and engram exist
    let content = fs::read_to_string(&mcp_file).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content).unwrap();
    let servers = v["mcpServers"].as_object().unwrap();

    assert!(servers.contains_key("github"), "Must preserve unrelated servers");
    assert!(servers.contains_key("engram"), "Must add engram server block");

    // Verify backup file was created
    assert!(cursor_dir.join("mcp.json.bak").exists());

    // Test removal
    let rem_res = setup_client(&dir, "cursor", false, true, false).unwrap();
    assert_eq!(rem_res["removed"], true);

    let content_after = fs::read_to_string(&mcp_file).unwrap();
    let v_after: serde_json::Value = serde_json::from_str(&content_after).unwrap();
    let servers_after = v_after["mcpServers"].as_object().unwrap();
    assert!(servers_after.contains_key("github"), "Must still preserve github");
    assert!(!servers_after.contains_key("engram"), "Engram must be removed");
}

#[test]
fn test_setup_dry_run_does_not_modify_disk() {
    let dir = temp_test_dir();
    let mcp_file = dir.join(".cursor").join("mcp.json");

    let res = setup_client(&dir, "cursor", true, false, false).unwrap();
    assert_eq!(res["dry_run"], true);
    assert!(!mcp_file.exists(), "Dry run must not create files");
}

#[test]
fn test_package_json_and_lockfile_intelligence() {
    let dir = temp_test_dir();
    fs::write(
        dir.join("package.json"),
        r#"{
            "name": "test-project",
            "scripts": {
                "test": "jest",
                "build": "tsc"
            },
            "dependencies": {
                "@angular/core": "^17.0.0",
                "@ngrx/store": "17.2.0"
            },
            "devDependencies": {
                "typescript": "~5.2.0",
                "jest": "^29.0.0"
            }
        }"#,
    )
    .unwrap();

    fs::write(
        dir.join("package-lock.json"),
        r#"{
            "packages": {
                "node_modules/@angular/core": { "version": "17.3.5" },
                "node_modules/@ngrx/store": { "version": "17.2.0" },
                "node_modules/typescript": { "version": "5.2.2" }
            }
        }"#,
    )
    .unwrap();

    let eco = detect_ecosystem(&dir).unwrap();
    assert_eq!(eco.frameworks.len(), 1);
    assert_eq!(eco.frameworks[0].name, "@angular/core");
    assert_eq!(eco.frameworks[0].declared_version, "^17.0.0");
    assert_eq!(eco.frameworks[0].resolved_version, Some("17.3.5".into()));

    assert_eq!(eco.libraries.len(), 1);
    assert_eq!(eco.libraries[0].name, "@ngrx/store");
    assert_eq!(eco.libraries[0].resolved_version, Some("17.2.0".into()));

    assert!(eco.dev_tools.iter().any(|d| d.name == "typescript"));
    assert_eq!(eco.scripts.get("test"), Some(&"jest".to_string()));
}

#[test]
fn test_constitution_rules_and_overrides() {
    let dir = temp_test_dir();
    let engram_dir = dir.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();

    fs::write(
        engram_dir.join("constitution.yaml"),
        r#"
schema_version: 1
rules:
  - id: RULE-001
    name: "No direct fetch in components"
    type: PATTERN
    severity: WARNING
    target_files: "**/*.component.ts"
    pattern: "fetch\\("
    description: "Components must not call window.fetch directly."
"#,
    )
    .unwrap();

    // Create a violating component
    let comp_file = dir.join("user.component.ts");
    fs::write(&comp_file, "export class UserComponent { load() { fetch('/api/user'); } }").unwrap();

    let conn = db(&dir).unwrap();
    let violations = evaluate_rules(&dir, &["user.component.ts".into()], &conn).unwrap();
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].rule_id, "RULE-001");
    assert!(!violations[0].overridden);

    // Record an advisory override decision
    conn.execute(
        "INSERT INTO decisions(task_id, decision, reason, rule_id, violation_id, created_at)
         VALUES('task_123', 'OVERRIDE', 'Legacy migration exception', 'RULE-001', 'RULE-001-user_component_ts-L1', '2026-10-06T00:00:00Z')",
        [],
    )
    .unwrap();

    // Re-evaluate: violation is now recognized as overridden with reason
    let violations_after = evaluate_rules(&dir, &["user.component.ts".into()], &conn).unwrap();
    assert_eq!(violations_after.len(), 1);
    assert!(violations_after[0].overridden);
    assert_eq!(violations_after[0].override_reason, Some("Legacy migration exception".into()));
}

#[test]
fn test_graph_traversal_and_reverse_dependencies() {
    let dir = temp_test_dir();

    // File A imports File B
    // File C tests File B
    fs::write(
        dir.join("service.ts"),
        "export class OrderService { process() { return 42; } }",
    )
    .unwrap();

    fs::write(
        dir.join("controller.ts"),
        "import { OrderService } from './service';\nexport class OrderController {}",
    )
    .unwrap();

    fs::write(
        dir.join("service.spec.ts"),
        "import { OrderService } from './service';\ndescribe('OrderService', () => {});",
    )
    .unwrap();

    let summary = analyze(&dir).unwrap();
    assert_eq!(summary.indexed_files, 3);

    let conn = db(&dir).unwrap();
    let traversal = GraphTraversal::new(&conn);

    // Outgoing dependencies of controller.ts -> should find service.ts
    let out = traversal.neighbors("controller.ts", &["IMPORTS"], Direction::Outgoing, 1).unwrap();
    assert!(out.iter().any(|e| e.target == "service.ts"));

    // Reverse dependencies of service.ts -> should find controller.ts (imported by) and service.spec.ts (tested by)
    let rev = traversal.reverse_dependencies("service.ts").unwrap();
    assert!(rev.iter().any(|e| e.source == "controller.ts" && e.kind == "IMPORTS"));
    assert!(rev.iter().any(|e| e.source == "service.spec.ts" && e.kind == "TESTS"));

    // Context compilation pulls service.ts when querying controller, with provenance attached
    let ctx = context(&dir, "OrderController workflow", 1000).unwrap();
    assert!(!ctx.files.is_empty());
    assert!(ctx.files.iter().any(|f| !f.provenance.is_empty()));
}
