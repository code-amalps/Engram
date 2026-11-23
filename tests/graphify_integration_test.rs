use std::fs;
use std::path::PathBuf;
use std::process::Command;
use uuid::Uuid;

use engram::graphify::{
    GraphStaleness, GraphifyIndex, GraphifyLink, GraphifyMacroGraph, GraphifyNode,
    check_graph_staleness, inspect_graphify_workspace, validate_macro_graph_boundaries,
};

fn setup_test_repo(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "engram_graphify_test_{}_{}",
        prefix,
        Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).unwrap();

    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };

    run(&["init", "-b", "main"]);
    run(&["config", "user.name", "Graphify Tester"]);
    run(&["config", "user.email", "graphify-tester@engram.dev"]);

    let init_file = dir.join("README.md");
    fs::write(&init_file, "# Graphify Test Repo\n").unwrap();
    run(&["add", "README.md"]);
    run(&["commit", "-m", "Initial commit"]);

    dir
}

#[test]
fn test_parse_valid_macro_graph() {
    let json = serde_json::json!({
        "directed": false,
        "multigraph": false,
        "built_at_commit": "abcdef1234567890abcdef1234567890abcdef12",
        "nodes": [
            {
                "id": "node_auth_service",
                "label": "AuthService",
                "_callable": true,
                "_origin": "ast",
                "community": 1,
                "community_name": "Authentication & Session",
                "file_type": "code",
                "source_file": "src/auth/service.py",
                "source_location": "L25"
            },
            {
                "id": "node_db_pool",
                "label": "DatabasePool",
                "_callable": false,
                "_origin": "ast",
                "community": 2,
                "community_name": "Persistence",
                "file_type": "code",
                "source_file": "src/db/connection.py",
                "source_location": "L10"
            }
        ],
        "links": [
            {
                "source": "node_auth_service",
                "target": "node_db_pool",
                "relation": "queries",
                "_origin": "ast",
                "confidence": "EXTRACTED",
                "confidence_score": 1.0,
                "source_file": "src/auth/service.py"
            }
        ],
        "hyperedges": [
            {
                "id": "hyper_security_layer",
                "label": "Security Infrastructure",
                "nodes": ["node_auth_service", "node_db_pool"],
                "relation": "governs",
                "confidence": "EXTRACTED",
                "confidence_score": 1.0
            }
        ]
    })
    .to_string();

    let graph: GraphifyMacroGraph = serde_json::from_str(&json).unwrap();
    assert_eq!(graph.nodes.len(), 2);
    assert_eq!(graph.links.len(), 1);
    assert_eq!(graph.hyperedges.as_ref().unwrap().len(), 1);
    assert_eq!(
        graph.built_at_commit.as_deref(),
        Some("abcdef1234567890abcdef1234567890abcdef12")
    );
}

#[test]
fn test_validate_macro_graph_boundaries_rejects_path_traversal() {
    let repo = setup_test_repo("boundary_check");

    // Node with path traversal
    let bad_node_graph = GraphifyMacroGraph {
        directed: false,
        multigraph: false,
        graph: None,
        nodes: vec![GraphifyNode {
            id: "bad_node".to_string(),
            label: "Bad".to_string(),
            _callable: None,
            _callable_class: None,
            _origin: None,
            community: None,
            community_name: None,
            file_type: None,
            source_file: Some("../../etc/shadow".to_string()),
            source_location: None,
        }],
        links: vec![],
        hyperedges: None,
        built_at_commit: None,
    };

    assert!(
        validate_macro_graph_boundaries(&bad_node_graph, &repo).is_err(),
        "Node with path traversal MUST be rejected"
    );

    // Link with path traversal
    let bad_link_graph = GraphifyMacroGraph {
        directed: false,
        multigraph: false,
        graph: None,
        nodes: vec![],
        links: vec![GraphifyLink {
            source: "a".to_string(),
            target: "b".to_string(),
            relation: "calls".to_string(),
            _origin: None,
            confidence: None,
            confidence_score: None,
            context: None,
            source_file: Some("../external.py".to_string()),
            source_location: None,
            weight: None,
        }],
        hyperedges: None,
        built_at_commit: None,
    };

    assert!(
        validate_macro_graph_boundaries(&bad_link_graph, &repo).is_err(),
        "Link with path traversal MUST be rejected"
    );

    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn test_staleness_detection_fresh_vs_stale() {
    let repo = setup_test_repo("staleness_test");

    // Get current HEAD commit SHA
    let head_sha = engram::git::resolve_baseline(&repo, "HEAD")
        .unwrap()
        .unwrap();

    // 1. Fresh graph matching HEAD
    let fresh_graph = GraphifyMacroGraph {
        directed: false,
        multigraph: false,
        graph: None,
        nodes: vec![],
        links: vec![],
        hyperedges: None,
        built_at_commit: Some(head_sha.clone()),
    };
    let staleness = check_graph_staleness(&fresh_graph, &repo).unwrap();
    assert_eq!(staleness, GraphStaleness::Fresh { commit: head_sha });

    // 2. Stale graph with old commit
    let stale_graph = GraphifyMacroGraph {
        directed: false,
        multigraph: false,
        graph: None,
        nodes: vec![],
        links: vec![],
        hyperedges: None,
        built_at_commit: Some("0000000000000000000000000000000000000000".to_string()),
    };
    let staleness2 = check_graph_staleness(&stale_graph, &repo).unwrap();
    match staleness2 {
        GraphStaleness::Stale {
            built_at,
            current_head,
        } => {
            assert_eq!(built_at, "0000000000000000000000000000000000000000");
            assert_ne!(current_head, built_at);
        }
        other => panic!("Expected Stale status, got {:?}", other),
    }

    let _ = fs::remove_dir_all(&repo);
}

#[test]
fn test_graphify_index_and_impact_cone_expansion() {
    let graph = GraphifyMacroGraph {
        directed: false,
        multigraph: false,
        graph: None,
        nodes: vec![
            GraphifyNode {
                id: "node_service".to_string(),
                label: "OrderService".to_string(),
                _callable: Some(true),
                _callable_class: None,
                _origin: Some("ast".to_string()),
                community: Some(1),
                community_name: Some("Orders".to_string()),
                file_type: Some("code".to_string()),
                source_file: Some("src/order/service.py".to_string()),
                source_location: Some("L10".to_string()),
            },
            GraphifyNode {
                id: "node_repo".to_string(),
                label: "OrderRepository".to_string(),
                _callable: Some(true),
                _callable_class: None,
                _origin: Some("ast".to_string()),
                community: Some(1),
                community_name: Some("Orders".to_string()),
                file_type: Some("code".to_string()),
                source_file: Some("src/order/repo.py".to_string()),
                source_location: Some("L5".to_string()),
            },
            GraphifyNode {
                id: "node_billing".to_string(),
                label: "BillingClient".to_string(),
                _callable: Some(true),
                _callable_class: None,
                _origin: Some("ast".to_string()),
                community: Some(2),
                community_name: Some("Payment".to_string()),
                file_type: Some("code".to_string()),
                source_file: Some("src/billing/client.py".to_string()),
                source_location: Some("L20".to_string()),
            },
        ],
        links: vec![
            GraphifyLink {
                source: "node_service".to_string(),
                target: "node_repo".to_string(),
                relation: "persists_with".to_string(),
                _origin: Some("ast".to_string()),
                confidence: Some("EXTRACTED".to_string()),
                confidence_score: Some(1.0),
                context: Some("call".to_string()),
                source_file: Some("src/order/service.py".to_string()),
                source_location: Some("L15".to_string()),
                weight: Some(1.0),
            },
            GraphifyLink {
                source: "node_service".to_string(),
                target: "node_billing".to_string(),
                relation: "notifies".to_string(),
                _origin: Some("ast".to_string()),
                confidence: Some("EXTRACTED".to_string()),
                confidence_score: Some(0.9),
                context: Some("call".to_string()),
                source_file: Some("src/order/service.py".to_string()),
                source_location: Some("L30".to_string()),
                weight: Some(1.0),
            },
        ],
        hyperedges: None,
        built_at_commit: None,
    };

    let index = GraphifyIndex::build(graph);

    // Expand impact from src/order/service.py
    let impact = index.expand_impact("src/order/service.py", 2);

    assert_eq!(impact.len(), 2);
    assert!(impact.iter().any(|c| c.file_path == "src/order/repo.py"));
    assert!(
        impact
            .iter()
            .any(|c| c.file_path == "src/billing/client.py")
    );

    // Verify all candidates have is_graph_derived = true
    for c in &impact {
        assert!(c.is_graph_derived);
    }
}

#[test]
fn test_inspect_graphify_workspace_reports_correctly() {
    let repo = setup_test_repo("inspect_report");

    // 1. Missing graphify-out
    let report_missing = inspect_graphify_workspace(&repo).unwrap();
    assert!(!report_missing.present);
    assert_eq!(report_missing.node_count, 0);

    // 2. Add realistic graphify-out/graph.json
    let out_dir = repo.join("graphify-out");
    fs::create_dir_all(&out_dir).unwrap();

    let json = serde_json::json!({
        "directed": false,
        "multigraph": false,
        "built_at_commit": "HEAD",
        "nodes": [
            {
                "id": "node_a",
                "label": "ModuleA",
                "community": 1,
                "community_name": "Core",
                "source_file": "src/a.py"
            },
            {
                "id": "node_b",
                "label": "ModuleB",
                "community": 1,
                "community_name": "Core",
                "source_file": "src/b.py"
            }
        ],
        "links": [
            {
                "source": "node_a",
                "target": "node_b",
                "relation": "calls",
                "confidence_score": 1.0,
                "source_file": "src/a.py"
            }
        ]
    })
    .to_string();

    fs::write(out_dir.join("graph.json"), json).unwrap();

    let report_present = inspect_graphify_workspace(&repo).unwrap();
    assert!(report_present.present);
    assert_eq!(report_present.node_count, 2);
    assert_eq!(report_present.link_count, 1);
    assert_eq!(report_present.community_count, 1);
    assert_eq!(report_present.communities[0].name, "Core");
    assert_eq!(report_present.top_god_nodes.len(), 2);

    let _ = fs::remove_dir_all(&repo);
}
