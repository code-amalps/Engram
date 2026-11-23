//! Engram Cross-Layer Feature Tracing Test Suite (`engram_trace_feature`)
//!
//! Validates:
//! 1. Framework entry point discovery (FastAPI routes and Typer CLI commands).
//! 2. Deterministic multi-hop path traversal ($H_0 \to H_1 \to H_2 \to \dots \to H_k$).
//! 3. Honest unresolved link detection and external boundary classification.
//! 4. Cycle prevention in recursive or circular caller-callee graphs.
//! 5. Depth and path limits enforcement (`max_depth`, `max_paths`).
//! 6. Security boundary enforcement (scope traversal rejection, empty query rejection).
//! 7. Output payload bounding (`max_bytes`).
//! 8. Multi-service vertical slice validation against isolated test workspace.

use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use engram::feature_trace::{
    TraceConfidence, TraceRequest, handle_engram_trace_feature, trace_feature,
};
use engram::knowledge_graph::sync_knowledge_graph;

struct TestWorkspace {
    root: PathBuf,
}

impl TestWorkspace {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "engram_p902_test_{}_{}",
            name,
            uuid::Uuid::new_v4()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("api")).unwrap();
        fs::create_dir_all(root.join("services")).unwrap();
        fs::create_dir_all(root.join("db")).unwrap();
        fs::create_dir_all(root.join(".engram")).unwrap();

        let run_cmd = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(&root)
                .output()
                .expect("git execution failed");
            assert!(out.status.success());
        };

        run_cmd(&["init", "-b", "main"]);
        run_cmd(&["config", "user.name", "EngramTester"]);
        run_cmd(&["config", "user.email", "tester@engram.dev"]);

        Self { root }
    }

    fn commit(&self, msg: &str) {
        let run_cmd = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(&self.root)
                .output()
                .expect("git execution failed");
            assert!(out.status.success());
        };

        run_cmd(&["add", "."]);
        run_cmd(&["commit", "-m", msg, "--allow-empty"]);
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn test_feature_trace_fastapi_and_cli_entry_points() {
    let ws = TestWorkspace::new("endpoints");

    fs::write(
        ws.root.join("api/routes.py"),
        r#"
@app.post("/api/v1/order")
def create_order():
    process_order()

@app.get("/api/v1/order/{id}")
def get_order():
    fetch_order_data()
"#,
    )
    .unwrap();

    fs::write(
        ws.root.join("services/order_service.py"),
        r#"
def process_order():
    save_order_record()

def fetch_order_data():
    load_order_record()
"#,
    )
    .unwrap();

    fs::write(
        ws.root.join("db/repository.py"),
        r#"
def save_order_record():
    pass

def load_order_record():
    pass
"#,
    )
    .unwrap();

    fs::write(
        ws.root.join("cli.py"),
        r#"
@app.command("sync")
def sync_cmd():
    run_sync_job()

def run_sync_job():
    save_order_record()
"#,
    )
    .unwrap();

    ws.commit("initial commit with endpoints");
    sync_knowledge_graph(&ws.root, false).unwrap();

    // Trace 1: HTTP route for order
    let req = TraceRequest {
        query: "create_order".to_string(),
        entry_point_kind: Some("HTTP_ROUTE".to_string()),
        scope: None,
        max_depth: Some(5),
        max_paths: Some(3),
        confidence_threshold: Some("EXACT".to_string()),
        include_unresolved: Some(true),
        max_bytes: None,
    };

    let trace = trace_feature(&ws.root, &req).unwrap();
    assert_eq!(trace.query, "create_order");
    assert!(!trace.entry_points.is_empty());
    assert_eq!(trace.entry_points[0].kind, "HTTP_ROUTE");
    assert!(
        trace.entry_points[0]
            .display_name
            .contains("POST /api/v1/order")
    );

    assert!(!trace.paths.is_empty());
    let path = &trace.paths[0];
    assert!(
        path.is_complete,
        "Path should reach db/repository.py data access layer"
    );
    assert!(path.hops.len() >= 3);
    assert_eq!(path.hops[0].node_kind, "HTTP_ROUTE");
    assert_eq!(path.hops[1].node_kind, "HANDLER");
    assert_eq!(path.hops[path.hops.len() - 1].node_kind, "DATA_ACCESS");

    // Trace 2: CLI command sync
    let req_cli = TraceRequest {
        query: "sync".to_string(),
        entry_point_kind: Some("CLI_COMMAND".to_string()),
        scope: None,
        max_depth: Some(5),
        max_paths: Some(3),
        confidence_threshold: None,
        include_unresolved: Some(true),
        max_bytes: None,
    };

    let trace_cli = trace_feature(&ws.root, &req_cli).unwrap();
    assert!(!trace_cli.entry_points.is_empty());
    assert_eq!(trace_cli.entry_points[0].kind, "CLI_COMMAND");
    assert!(trace_cli.entry_points[0].display_name.contains("sync"));
    assert!(!trace_cli.paths.is_empty());
    assert!(trace_cli.paths[0].is_complete);
}

#[test]
fn test_feature_trace_external_boundary_and_unresolved() {
    let ws = TestWorkspace::new("boundary");

    fs::write(
        ws.root.join("api/webhook.py"),
        r#"
@app.post("/webhook")
def handle_webhook():
    print("Received webhook")
    x = len([1, 2, 3])
    forward_to_external_api()

def forward_to_external_api():
    httpx.post("https://external.example.com/api")
    unknown_dynamic_sink()
"#,
    )
    .unwrap();

    ws.commit("webhook commit");
    sync_knowledge_graph(&ws.root, false).unwrap();

    let req = TraceRequest {
        query: "webhook".to_string(),
        entry_point_kind: None,
        scope: None,
        max_depth: Some(5),
        max_paths: Some(3),
        confidence_threshold: None,
        include_unresolved: Some(true),
        max_bytes: None,
    };

    let trace = trace_feature(&ws.root, &req).unwrap();
    assert!(!trace.unresolved_links.is_empty());

    // Verify trivial builtins like print, len are filtered out
    for link in &trace.unresolved_links {
        assert_ne!(
            link.target_hint, "print",
            "Trivial builtin 'print' must be filtered"
        );
        assert_ne!(
            link.target_hint, "len",
            "Trivial builtin 'len' must be filtered"
        );
    }

    // Verify external boundary is recognized
    let has_external = trace.unresolved_links.iter().any(|l| {
        l.link_type == "ExternalBoundary"
            || l.target_hint.contains("post")
            || l.target_hint.contains("httpx")
    });
    assert!(
        has_external,
        "Should identify external client or dynamic boundary"
    );
}

#[test]
fn test_feature_trace_cycle_prevention() {
    let ws = TestWorkspace::new("cycles");

    fs::write(
        ws.root.join("api/router.py"),
        r#"
@app.get("/ping")
def ping_entry():
    step_one()

def step_one():
    step_two()

def step_two():
    step_one()
"#,
    )
    .unwrap();

    ws.commit("cycle commit");
    sync_knowledge_graph(&ws.root, false).unwrap();

    let req = TraceRequest {
        query: "ping".to_string(),
        entry_point_kind: None,
        scope: None,
        max_depth: Some(6),
        max_paths: Some(3),
        confidence_threshold: None,
        include_unresolved: Some(true),
        max_bytes: None,
    };

    // Traversal must terminate and not loop indefinitely
    let trace = trace_feature(&ws.root, &req).unwrap();
    assert!(!trace.paths.is_empty());

    for p in &trace.paths {
        // Assert no node ID is visited multiple times in the same path
        let mut seen = std::collections::HashSet::new();
        for h in &p.hops {
            assert!(
                seen.insert(&h.node_id),
                "Cycle detected: node {} visited twice in path",
                h.node_id
            );
        }
    }
}

#[test]
fn test_feature_trace_depth_and_path_limits() {
    let ws = TestWorkspace::new("limits");

    fs::write(
        ws.root.join("api/chain.py"),
        r#"
@app.get("/chain")
def chain_start():
    level_1()

def level_1():
    level_2()

def level_2():
    level_3()

def level_3():
    level_4()

def level_4():
    pass
"#,
    )
    .unwrap();

    ws.commit("chain commit");
    sync_knowledge_graph(&ws.root, false).unwrap();

    let req = TraceRequest {
        query: "chain".to_string(),
        entry_point_kind: None,
        scope: None,
        max_depth: Some(3), // Limit depth to 3 hops
        max_paths: Some(1), // Limit to 1 path
        confidence_threshold: None,
        include_unresolved: Some(true),
        max_bytes: None,
    };

    let trace = trace_feature(&ws.root, &req).unwrap();
    assert_eq!(trace.paths.len(), 1);
    assert!(
        trace.paths[0].hops.len() <= 3,
        "Hops count must be clamped by max_depth"
    );
}

#[test]
fn test_feature_trace_security_scope_rejection() {
    let ws = TestWorkspace::new("security");
    ws.commit("empty");

    // Empty query rejection
    let empty_query = json!({
        "query": "   "
    });
    let err = handle_engram_trace_feature(&ws.root, &empty_query).unwrap_err();
    assert!(err.to_string().contains("required and must not be empty"));

    // Path traversal escape in scope
    let escape_scope = json!({
        "query": "test",
        "scope": "../parent_escape"
    });
    let err = handle_engram_trace_feature(&ws.root, &escape_scope).unwrap_err();
    assert!(err.to_string().contains("safe workspace-relative path"));

    let root_scope = json!({
        "query": "test",
        "scope": "/absolute/path"
    });
    let err = handle_engram_trace_feature(&ws.root, &root_scope).unwrap_err();
    assert!(err.to_string().contains("safe workspace-relative path"));
}

#[test]
fn test_feature_trace_max_bytes_bounding() {
    let ws = TestWorkspace::new("bytes");

    fs::write(
        ws.root.join("api/app.py"),
        r#"
@app.get("/big")
def big_entry():
    call_a()

def call_a():
    call_b()

def call_b():
    call_c()

def call_c():
    pass
"#,
    )
    .unwrap();

    ws.commit("big commit");
    sync_knowledge_graph(&ws.root, false).unwrap();

    let req = TraceRequest {
        query: "big".to_string(),
        entry_point_kind: None,
        scope: None,
        max_depth: Some(5),
        max_paths: Some(5),
        confidence_threshold: None,
        include_unresolved: Some(true),
        max_bytes: Some(1024), // Strict byte ceiling
    };

    let trace = trace_feature(&ws.root, &req).unwrap();
    let serialized = serde_json::to_string(&trace).unwrap();
    assert!(
        serialized.len() <= 1024 || !trace.diagnostics.is_empty(),
        "Payload must be bounded or diagnostic emitted"
    );
}

#[test]
fn test_multi_service_ground_truth_traces() {
    let ws = TestWorkspace::new("multi_service");

    fs::write(
        ws.root.join("api/app.py"),
        r#"
@app.post("/api/v1/audit")
def run_security_audit():
    run_remediation()

@app.get("/api/v1/policy")
def get_policy():
    load_policy()

@app.get("/api/v1/doctor")
def get_system_diagnostics():
    to_dict()
    urlopen(Request("https://api.example.com/health"))

def to_dict():
    pass
"#,
    )
    .unwrap();

    fs::write(
        ws.root.join("services/security.py"),
        r#"
def run_remediation():
    query_repository()

def load_policy():
    save_storage()
"#,
    )
    .unwrap();

    fs::write(
        ws.root.join("db/storage.py"),
        r#"
def query_repository():
    pass

def save_storage():
    pass

def list_campaigns():
    pass
"#,
    )
    .unwrap();

    fs::write(
        ws.root.join("cli.py"),
        r#"
@app.command("checkpoints")
def checkpoints_cmd():
    query_repository()

@app.command("campaign")
def campaign_cmd():
    list_campaigns()
"#,
    )
    .unwrap();

    ws.commit("multi service endpoints");
    sync_knowledge_graph(&ws.root, false).expect("Knowledge graph sync failed");

    // 1. Audit slice: POST /api/v1/audit -> run_security_audit() -> run_remediation() -> query_repository()
    let trace_audit = trace_feature(
        &ws.root,
        &TraceRequest {
            query: "audit".to_string(),
            entry_point_kind: Some("HTTP_ROUTE".to_string()),
            scope: None,
            max_depth: Some(5),
            max_paths: Some(3),
            confidence_threshold: Some("EXACT".to_string()),
            include_unresolved: Some(true),
            max_bytes: None,
        },
    )
    .expect("Failed to trace audit feature");

    assert!(!trace_audit.entry_points.is_empty());
    assert!(
        trace_audit.entry_points[0]
            .display_name
            .contains("/api/v1/audit")
    );
    assert!(!trace_audit.paths.is_empty());
    assert!(trace_audit.paths[0].is_complete);
    assert_eq!(
        trace_audit.paths[0].hops[0].confidence,
        TraceConfidence::Exact
    );

    // 2. Policy slice: GET /api/v1/policy -> get_policy() -> load_policy() -> save_storage()
    let trace_policy = trace_feature(
        &ws.root,
        &TraceRequest {
            query: "policy".to_string(),
            entry_point_kind: Some("HTTP_ROUTE".to_string()),
            scope: None,
            max_depth: Some(5),
            max_paths: Some(3),
            confidence_threshold: Some("EXACT".to_string()),
            include_unresolved: Some(true),
            max_bytes: None,
        },
    )
    .expect("Failed to trace policy feature");

    assert!(!trace_policy.entry_points.is_empty());
    assert!(
        trace_policy.entry_points[0]
            .display_name
            .contains("/api/v1/policy")
    );
    assert!(!trace_policy.paths.is_empty());
    assert!(trace_policy.paths[0].is_complete);
    let policy_path = &trace_policy.paths[0];
    assert!(
        policy_path
            .hops
            .iter()
            .any(|h| h.display_name.contains("load_policy"))
    );

    // 3. Checkpoints slice: CLI command: checkpoints -> checkpoints_cmd() -> query_repository()
    let trace_checkpoints = trace_feature(
        &ws.root,
        &TraceRequest {
            query: "checkpoints".to_string(),
            entry_point_kind: Some("CLI_COMMAND".to_string()),
            scope: None,
            max_depth: Some(5),
            max_paths: Some(3),
            confidence_threshold: None,
            include_unresolved: Some(true),
            max_bytes: None,
        },
    )
    .expect("Failed to trace checkpoints command");

    assert!(!trace_checkpoints.entry_points.is_empty());
    assert!(
        trace_checkpoints
            .entry_points
            .iter()
            .any(|ep| ep.route_or_command.as_deref() == Some("checkpoints"))
    );

    // 4. Doctor slice: GET /api/v1/doctor -> get_system_diagnostics() -> Request/urlopen ExternalBoundary
    let trace_doctor = trace_feature(
        &ws.root,
        &TraceRequest {
            query: "doctor".to_string(),
            entry_point_kind: Some("HTTP_ROUTE".to_string()),
            scope: None,
            max_depth: Some(5),
            max_paths: Some(3),
            confidence_threshold: None,
            include_unresolved: Some(true),
            max_bytes: None,
        },
    )
    .expect("Failed to trace doctor feature");

    assert!(!trace_doctor.entry_points.is_empty());
    assert!(
        trace_doctor.entry_points[0]
            .display_name
            .contains("/api/v1/doctor")
    );
    let has_doctor_boundary = trace_doctor.unresolved_links.iter().any(|l| {
        l.link_type == "ExternalBoundary"
            && (l.target_hint.contains("urlopen") || l.target_hint.contains("Request"))
    });
    assert!(
        has_doctor_boundary,
        "Doctor trace should capture external network boundary (urlopen/Request)"
    );

    // 5. Campaign slice: CLI command: campaign -> campaign_cmd() -> list_campaigns()
    let trace_campaign = trace_feature(
        &ws.root,
        &TraceRequest {
            query: "campaign".to_string(),
            entry_point_kind: Some("CLI_COMMAND".to_string()),
            scope: None,
            max_depth: Some(5),
            max_paths: Some(3),
            confidence_threshold: None,
            include_unresolved: Some(true),
            max_bytes: None,
        },
    )
    .expect("Failed to trace campaign command");

    assert!(!trace_campaign.entry_points.is_empty());
    assert!(!trace_campaign.paths.is_empty());
    assert!(trace_campaign.paths.iter().any(|p| p.is_complete));
}
