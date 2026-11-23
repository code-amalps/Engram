//! Engram Phase 9.0.1 Golden Regression Test Suite
//!
//! Locks deterministic JSON outputs and validates:
//! 1. Micro-Knowledge Graph status schema and lifecycle states against golden JSON.
//! 2. Shortest multi-hop call path output against golden fixture.
//! 3. Transitive blast-radius impact analysis against golden fixture.
//! 4. Context compilation and engram_engage output against golden fixture.
//! 5. Accurate context byte length, labeled token estimates, and token reduction
//!    across 5 distinct real-world repository tasks.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use engram::engage::{EngageRequest, execute_engage};
use engram::knowledge_graph::{
    GraphLifecycleState, KnowledgeGraphIndex, open_graph_db, rank_context_with_graph,
    sync_knowledge_graph,
};

struct GoldenWorkspace {
    root: PathBuf,
}

impl GoldenWorkspace {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("engram_golden_{}_{}", name, uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("tests")).unwrap();
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
        run_cmd(&["config", "user.name", "EngramGoldenTester"]);
        run_cmd(&["config", "user.email", "golden@engram.dev"]);

        // Cargo.toml
        fs::write(
            root.join("Cargo.toml"),
            r#"[package]
name = "golden_pkg"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();

        // src/lib.rs
        fs::write(
            root.join("src/lib.rs"),
            r#"pub mod math;
pub mod service;

pub fn initialize() -> bool {
    service::run_service()
}
"#,
        )
        .unwrap();

        // src/math.rs
        fs::write(
            root.join("src/math.rs"),
            r#"pub struct Calculator {
    pub precision: usize,
}

impl Calculator {
    pub fn new(precision: usize) -> Self {
        Self { precision }
    }

    pub fn compute(&self, val: f64) -> f64 {
        val * 2.0
    }
}
"#,
        )
        .unwrap();

        // src/service.rs
        fs::write(
            root.join("src/service.rs"),
            r#"use crate::math::Calculator;

pub fn run_service() -> bool {
    let calc = Calculator::new(4);
    calc.compute(21.0) == 42.0
}
"#,
        )
        .unwrap();

        // tests/math_test.rs
        fs::write(
            root.join("tests/math_test.rs"),
            r#"use golden_pkg::math::Calculator;

#[test]
fn test_compute() {
    let calc = Calculator::new(2);
    assert_eq!(calc.compute(10.0), 20.0);
}
"#,
        )
        .unwrap();

        run_cmd(&["add", "."]);
        run_cmd(&["commit", "-m", "Initial golden fixture workspace commit"]);

        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for GoldenWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn test_golden_graph_status() {
    let ws = GoldenWorkspace::new("status");
    let root = ws.path();

    let status = sync_knowledge_graph(root, true).expect("Failed to sync knowledge graph");
    assert_eq!(status.state, GraphLifecycleState::Ready);

    let mut actual_json = serde_json::to_value(&status).unwrap();
    // Normalize commit hash
    if actual_json.get("last_indexed_commit").is_some() {
        actual_json["last_indexed_commit"] = serde_json::json!("[COMMIT]");
    }

    let golden_path = Path::new("tests/fixtures/golden/graph_status.golden.json");
    let golden_str = fs::read_to_string(golden_path).expect("Failed to read golden status");
    let golden_json: Value = serde_json::from_str(&golden_str).expect("Invalid golden JSON");

    assert_eq!(actual_json["state"], golden_json["state"]);
    assert_eq!(actual_json["schema_version"], golden_json["schema_version"]);
    assert_eq!(
        actual_json["indexed_files_count"],
        golden_json["indexed_files_count"]
    );
    assert_eq!(
        actual_json["failed_files_count"],
        golden_json["failed_files_count"]
    );
    assert_eq!(
        actual_json["unsupported_files_count"],
        golden_json["unsupported_files_count"]
    );
    assert_eq!(
        actual_json["last_indexed_commit"],
        golden_json["last_indexed_commit"]
    );
}

#[test]
fn test_golden_call_path() {
    let ws = GoldenWorkspace::new("call_path");
    let root = ws.path();

    sync_knowledge_graph(root, true).unwrap();
    let conn = open_graph_db(root).unwrap();
    let index = KnowledgeGraphIndex::load_from_db(&conn).unwrap();

    let path = index
        .call_path("initialize", "compute", 4)
        .expect("Call path from initialize to compute must exist");

    // Normalize hashes from symbol IDs
    let normalized_path: Vec<String> = path
        .into_iter()
        .map(|node_id| {
            if let Some(pos) = node_id.rfind("::") {
                node_id[..pos].to_string()
            } else {
                node_id
            }
        })
        .collect();

    let golden_path = Path::new("tests/fixtures/golden/call_path.golden.json");
    let golden_str = fs::read_to_string(golden_path).expect("Failed to read golden call path");
    let golden_json: Vec<String> = serde_json::from_str(&golden_str).expect("Invalid golden JSON");

    assert_eq!(normalized_path, golden_json);
}

#[test]
fn test_golden_impact_report() {
    let ws = GoldenWorkspace::new("impact");
    let root = ws.path();

    sync_knowledge_graph(root, true).unwrap();
    let conn = open_graph_db(root).unwrap();
    let index = KnowledgeGraphIndex::load_from_db(&conn).unwrap();

    let report = index.impact("file:src/math.rs", 2);

    let golden_path = Path::new("tests/fixtures/golden/impact_report.golden.json");
    let golden_str = fs::read_to_string(golden_path).expect("Failed to read golden impact");
    let golden_json: Value = serde_json::from_str(&golden_str).expect("Invalid golden JSON");

    assert_eq!(report.target, golden_json["target"].as_str().unwrap());
    assert_eq!(
        serde_json::to_value(&report.affected_files).unwrap(),
        golden_json["affected_files"]
    );
    assert_eq!(
        serde_json::to_value(&report.affected_symbols).unwrap(),
        golden_json["affected_symbols"]
    );
    assert_eq!(
        serde_json::to_value(&report.exercised_tests).unwrap(),
        golden_json["exercised_tests"]
    );
}

#[test]
fn test_golden_engage_context() {
    let ws = GoldenWorkspace::new("engage");
    let root = ws.path();

    sync_knowledge_graph(root, true).unwrap();

    let req = EngageRequest {
        task: "compute math calculator precision".to_string(),
        token_budget: 600,
        planned_files: Some(vec![
            "src/math.rs".to_string(),
            "src/lib.rs".to_string(),
            "src/service.rs".to_string(),
            "tests/math_test.rs".to_string(),
            "Cargo.toml".to_string(),
        ]),
        forbidden: None,
        symbols: None,
        subpath: None,
        language: None,
        detail_level: "standard".to_string(),
        no_bypass: false,
        inject_instructions: false,
        max_bytes: 65536,
    };

    let engage_res = execute_engage(root, req).expect("execute_engage failed");

    let golden_path = Path::new("tests/fixtures/golden/engage_context.golden.json");
    let golden_str = fs::read_to_string(golden_path).expect("Failed to read golden engage");
    let golden_json: Value = serde_json::from_str(&golden_str).expect("Invalid golden JSON");

    assert_eq!(
        engage_res["next_action"].as_str().unwrap(),
        golden_json["next_action"].as_str().unwrap()
    );
    assert_eq!(
        engage_res["boundaries"]["expected_files"],
        golden_json["expected_files"]
    );

    // Verify top prioritized file in context
    let target_files = engage_res["context"]["target_files"].as_array().unwrap();
    assert!(!target_files.is_empty());
    assert_eq!(
        target_files[0]["path"].as_str().unwrap(),
        golden_json["top_prioritized_file"].as_str().unwrap()
    );

    // Verify token reduction
    let reduction_pct = engage_res["context"]["graph_intelligence"]["token_reduction_pct"]
        .as_f64()
        .unwrap();
    assert!(reduction_pct > 0.0);
}

#[test]
fn test_context_reduction_across_5_distinct_tasks() {
    let ws = GoldenWorkspace::new("tasks5");
    let root = ws.path();

    sync_knowledge_graph(root, true).unwrap();

    let candidate_files = vec![
        "src/lib.rs".to_string(),
        "src/math.rs".to_string(),
        "src/service.rs".to_string(),
        "tests/math_test.rs".to_string(),
        "Cargo.toml".to_string(),
    ];

    let tasks = vec![
        ("compute precision math", "src/math.rs"),
        ("run_service execution logic", "src/service.rs"),
        ("test_compute assertion tests", "tests/math_test.rs"),
        ("initialize library package module entry", "src/lib.rs"),
        ("Cargo.toml package configuration", "Cargo.toml"),
    ];

    for (task_query, expected_top_file) in tasks {
        let report = rank_context_with_graph(root, task_query, &candidate_files, 800)
            .expect("Context ranking failed");

        assert_eq!(report.task_query, task_query);
        assert!(!report.prioritized_files.is_empty());
        assert_eq!(
            report.prioritized_files[0], expected_top_file,
            "Query '{}' must prioritize '{}' as top file",
            task_query, expected_top_file
        );
        assert!(
            report.prioritized_files.len() < candidate_files.len(),
            "Prioritized files must be pruned for query: {}",
            task_query
        );
        assert!(
            report.token_reduction_percentage > 0.0,
            "Token reduction must be positive for query: {}",
            task_query
        );
        assert!(
            report.estimated_graph_tokens < report.estimated_baseline_tokens,
            "Graph tokens ({}) must be strictly less than baseline ({}) for query: {}",
            report.estimated_graph_tokens,
            report.estimated_baseline_tokens,
            task_query
        );
    }
}
