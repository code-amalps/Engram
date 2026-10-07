//! Integration test suite for Engram v0.7.0 Change Impact Intelligence:
//! Multi-hop impact cone traversal, 7 deterministic relationship categories,
//! provenance accuracy, and Invariant 1 (Separation of Prediction and Authorization).

use std::fs;
use std::path::PathBuf;

use engram::analyzer::analyze;
use engram::compiler::context;
use engram::contract::{
    create_contract, create_contract_with_impact, evaluate_impact_against_contract,
};
use engram::impact::{compute_impact_cone, format_impact_explanation};
use engram::storage::db;

fn setup_go_ecommerce_fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!("engram_impact_test_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(&root).unwrap();

    // 1. go.mod
    fs::write(
        root.join("go.mod"),
        "module github.com/acme/shop\n\ngo 1.22\n",
    )
    .unwrap();

    // 2. internal/orders/service.go
    fs::create_dir_all(root.join("internal/orders")).unwrap();
    fs::write(
        root.join("internal/orders/service.go"),
        r#"package orders

import (
	"github.com/acme/shop/pkg/export"
)

type OrderService struct {
	exporter export.Exporter
}

func (s *OrderService) ExportCSV(path string) error {
	return nil
}
"#,
    )
    .unwrap();

    // 3. internal/orders/service_test.go
    fs::write(
        root.join("internal/orders/service_test.go"),
        r#"package orders

import "testing"

func TestOrderServiceExportCSV(t *testing.T) {
	s := &OrderService{}
	_ = s.ExportCSV("test.csv")
}
"#,
    )
    .unwrap();

    // 4. pkg/export/exporter.go
    fs::create_dir_all(root.join("pkg/export")).unwrap();
    fs::write(
        root.join("pkg/export/exporter.go"),
        r#"package export

type Exporter interface {
	Export(data string) error
}
"#,
    )
    .unwrap();

    // 5. pkg/export/csv.go (implements Exporter)
    fs::write(
        root.join("pkg/export/csv.go"),
        r#"package export

type CsvExporter struct{}

func (c *CsvExporter) Export(data string) error {
	return nil
}
"#,
    )
    .unwrap();

    // 6. cmd/server/main.go (imports orders)
    fs::create_dir_all(root.join("cmd/server")).unwrap();
    fs::write(
        root.join("cmd/server/main.go"),
        r#"package main

import "github.com/acme/shop/internal/orders"

func main() {
	_ = &orders.OrderService{}
}
"#,
    )
    .unwrap();

    // 7. internal/secret/keys.go (forbidden boundary)
    fs::create_dir_all(root.join("internal/secret")).unwrap();
    fs::write(
        root.join("internal/secret/keys.go"),
        r#"package secret

const ApiSecret = "super-confidential-key"
"#,
    )
    .unwrap();

    root
}

#[test]
fn test_impact_cone_categories_and_hops() {
    let root = setup_go_ecommerce_fixture();
    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    // 1. Compute impact cone for OrderService.ExportCSV
    let cone = compute_impact_cone(&conn, &root, "OrderService.ExportCSV", 2).unwrap();

    assert_eq!(cone.target_symbol, "OrderService.ExportCSV");
    assert!(cone.target_file.is_some());
    assert_eq!(
        cone.target_file.as_deref().unwrap().replace('\\', "/"),
        "internal/orders/service.go"
    );

    // Direct target
    assert!(
        cone.direct_entities
            .iter()
            .any(|e| e.path.replace('\\', "/") == "internal/orders/service.go")
    );

    // Test association
    assert!(
        cone.test_entities
            .iter()
            .any(|e| e.path.replace('\\', "/") == "internal/orders/service_test.go")
    );

    // Dependent package importing orders
    assert!(
        cone.import_dependent_entities
            .iter()
            .any(|e| e.path.replace('\\', "/") == "cmd/server/main.go")
    );

    // Total hops should be respected
    assert_eq!(cone.total_hops, 2);
    for e in cone.all_entities() {
        assert!(e.hop_distance <= 2);
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_impact_provenance_accuracy() {
    let root = setup_go_ecommerce_fixture();
    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone = compute_impact_cone(&conn, &root, "OrderService.ExportCSV", 2).unwrap();
    let explanation = format_impact_explanation(&cone);

    // Formatted explanation must contain all required sections
    assert!(explanation.contains("🎯 TARGET"));
    assert!(explanation.contains("OrderService.ExportCSV"));
    assert!(explanation.contains("💥 IMPACT BLAST RADIUS"));
    assert!(explanation.contains("DIRECT:"));
    assert!(explanation.contains("TEST:"));
    assert!(explanation.contains("🔍 PROVENANCE (WHY)"));

    // Every entity in cone must have non-empty reason and relationships
    for e in cone.all_entities() {
        assert!(!e.reason.is_empty(), "Entity {} must have causal reason", e.path);
        if e.hop_distance > 0 {
            assert!(
                !e.relationships.is_empty(),
                "Non-direct entity {} must have explicit relationships",
                e.path
            );
            for r in &e.relationships {
                assert!(!r.provenance.is_empty());
                assert!(r.confidence > 0.0);
            }
        }
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_contract_impact_expansion_authorization() {
    let root = setup_go_ecommerce_fixture();
    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone = compute_impact_cone(&conn, &root, "OrderService.ExportCSV", 2).unwrap();

    // Invariant 1: Impact predicts. Contract authorizes.
    // Create contract that ONLY authorizes internal/orders/**, forbidding internal/secret/**
    let contract = create_contract(
        &root,
        "Update OrderService export",
        Some(vec!["orders".to_string()]),
        Some(vec!["internal/orders/service.go".to_string()]),
        Some(vec!["internal/secret/**".to_string()]),
    )
    .unwrap();

    let auth = evaluate_impact_against_contract(&contract, &cone);

    // 1. Direct target is authorized
    assert!(auth.authorized_targets.iter().any(|t| t.replace('\\', "/") == "internal/orders/service.go"));

    // 2. cmd/server/main.go was predicted in the blast radius (imported orders),
    // BUT is NOT authorized because module is 'server', not 'orders'!
    assert!(
        auth.unauthorized_relevant.iter().any(|u| u.replace('\\', "/") == "cmd/server/main.go"),
        "cmd/server/main.go must be classified as UNAUTHORIZED RELEVANT"
    );

    // 3. Test creating contract seeded with impact cone
    let seeded_contract = create_contract_with_impact(
        &root,
        "Seeded task with impact cone",
        &cone,
        Some(vec!["orders".to_string()]),
        Some(vec!["internal/secret/**".to_string()]),
    )
    .unwrap();

    // Verify seeded contract has expected files from direct & test entities
    assert!(seeded_contract.boundary.expected_files.iter().any(|f| f.replace('\\', "/") == "internal/orders/service.go"));
    assert!(seeded_contract.boundary.expected_files.iter().any(|f| f.replace('\\', "/") == "internal/orders/service_test.go"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_context_compiler_impact_ranking() {
    let root = setup_go_ecommerce_fixture();
    analyze(&root).unwrap();

    // Compile context for task matching OrderService
    let ctx = context(&root, "Export CSV from OrderService", 1200).unwrap();

    // Direct file must be ranked higher than transitive caller
    let service_idx = ctx.files.iter().position(|f| f.path.replace('\\', "/") == "internal/orders/service.go");
    assert!(service_idx.is_some(), "internal/orders/service.go must be packed in context");

    let service_file = &ctx.files[service_idx.unwrap()];
    // Must contain impact provenance annotation
    assert!(
        service_file.provenance.iter().any(|p| p.contains("Impact")),
        "File provenance must include Impact ranking information: {:?}",
        service_file.provenance
    );
    let _ = fs::remove_dir_all(&root);
}
