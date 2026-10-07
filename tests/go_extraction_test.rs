//! Integration Test Suite for Engram v0.5.0: Go Language Parser
//! Verifies Tree-sitter AST extraction, receiver pointer normalization,
//! go.mod local import graph resolution, test conventions,
//! Change Contract symbol matching acceptance gate, and Constitution precedence.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use engram::analyzer::{analyze, parse_file};
use engram::compiler::context;
use engram::contract::{
    create_contract, create_contract_with_symbols, matches_contract_symbol,
    validate_contract_diff_with_constitution, validate_symbol_against_contract,
    FileValidationCategory, SymbolMatchResult,
};
use engram::go_parser::{
    extract_go_ast, normalize_go_symbol_name, parse_go_mod, resolve_go_import_targets,
};

fn temp_repo_dir(prefix: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("engram_go_{}_{}", prefix, uuid::Uuid::new_v4()));
    fs::create_dir_all(&p).unwrap();
    // Initialize git repository
    let _ = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&p)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "EngramGoTester"])
        .current_dir(&p)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "tester@engram.dev"])
        .current_dir(&p)
        .output();
    p
}

/// 1. Go struct and interface extraction
#[test]
fn test_go_struct_and_interface_extraction() {
    let source = r#"
package orders

import (
    "io"
)

type OrderService struct {
    ID string
}

type Exporter interface {
    Export(w io.Writer) error
}

func HelperFunction() string {
    return "ok"
}
"#;

    let ast = extract_go_ast(source).unwrap();
    assert_eq!(ast.package.as_ref().map(|p| p.name.as_str()), Some("orders"));

    // Verify struct
    let struct_type = ast.types.iter().find(|t| t.name == "OrderService").unwrap();
    assert_eq!(struct_type.kind, "STRUCT");

    // Verify interface
    let iface = ast.types.iter().find(|t| t.name == "Exporter").unwrap();
    assert_eq!(iface.kind, "INTERFACE");

    // Verify free function
    let func = ast.functions.iter().find(|f| f.name == "HelperFunction").unwrap();
    assert_eq!(func.name, "HelperFunction");
}

/// 2. Receiver pointer semantics & normalization
#[test]
fn test_go_receiver_pointer_value_normalization() {
    let source = r#"
package orders

import "io"

type OrderService struct {}

func (s *OrderService) ExportCSV(w io.Writer) error {
    return nil
}

func (s OrderService) GetStatus() string {
    return "active"
}
"#;

    let ast = extract_go_ast(source).unwrap();
    assert_eq!(ast.methods.len(), 2);

    // Pointer receiver method
    let m_ptr = ast.methods.iter().find(|m| m.method_name == "ExportCSV").unwrap();
    assert_eq!(m_ptr.name, "OrderService.ExportCSV");
    assert_eq!(m_ptr.receiver_type, "OrderService");
    assert!(m_ptr.is_pointer_receiver);

    // Value receiver method
    let m_val = ast.methods.iter().find(|m| m.method_name == "GetStatus").unwrap();
    assert_eq!(m_val.name, "OrderService.GetStatus");
    assert_eq!(m_val.receiver_type, "OrderService");
    assert!(!m_val.is_pointer_receiver);

    // Both resolve to canonical dot-notation without pointer symbol
    assert_eq!(normalize_go_symbol_name("*OrderService.ExportCSV"), "OrderService.ExportCSV");
    assert_eq!(normalize_go_symbol_name("OrderService.ExportCSV"), "OrderService.ExportCSV");
    assert_eq!(normalize_go_symbol_name("&OrderService.GetStatus"), "OrderService.GetStatus");
}

/// 3. Go module resolution via go.mod parsing
#[test]
fn test_go_mod_module_resolution() {
    let go_mod_content = r#"
module github.com/acme/shop

go 1.22
"#;
    assert_eq!(
        parse_go_mod(go_mod_content),
        Some("github.com/acme/shop".to_string())
    );

    let mut workspace = HashSet::new();
    workspace.insert("internal/payment/service.go".to_string());
    workspace.insert("internal/payment/types.go".to_string());

    let module = Some("github.com/acme/shop");
    let targets = resolve_go_import_targets(
        "internal/orders/service.go",
        "github.com/acme/shop/internal/payment",
        module,
        Some(&workspace),
    );

    assert_eq!(targets.len(), 2);
    assert!(targets.contains(&"internal/payment/service.go".to_string()));
    assert!(targets.contains(&"internal/payment/types.go".to_string()));
}

/// 4. Go test conventions (*_test.go -> *.go)
#[test]
fn test_go_test_convention_linking() {
    let root = temp_repo_dir("go_test_convention");
    let orders_dir = root.join("internal").join("orders");
    fs::create_dir_all(&orders_dir).unwrap();

    let service_go = "package orders\ntype OrderService struct {}\n";
    let service_test_go = "package orders\nimport \"testing\"\nfunc TestOrderService(t *testing.T) {}\n";

    fs::write(orders_dir.join("service.go"), service_go).unwrap();
    fs::write(orders_dir.join("service_test.go"), service_test_go).unwrap();

    let (_, _, rels) = parse_file(
        &root,
        "internal/orders/service_test.go",
        service_test_go.as_bytes(),
        "WORKTREE",
    )
    .unwrap();

    let test_rel = rels.iter().find(|r| r.kind == "TESTS").unwrap();
    assert_eq!(test_rel.source, "internal/orders/service_test.go");
    assert_eq!(test_rel.target, "internal/orders/service.go");
    assert_eq!(test_rel.evidence, "NAMING_CONVENTION");

    fs::remove_dir_all(&root).unwrap();
}

/// 5. Change Contract symbol matching acceptance gate:
///    Contract: OrderService.ExportCSV
///    Go source: func (s *OrderService) ExportCSV(...) -> MATCH
///    Go source: func (s *OrderService) Delete(...)    -> NO MATCH
#[test]
fn test_go_contract_symbol_matching_acceptance_gate() {
    let root = temp_repo_dir("go_contract_symbol");

    // 1. Create contract expecting OrderService.ExportCSV
    let contract = create_contract_with_symbols(
        &root,
        "Add CSV export to orders",
        Some(vec!["orders".to_string()]),
        Some(vec!["internal/orders/service.go".to_string()]),
        Some(vec!["OrderService.ExportCSV".to_string()]),
        None,
    )
    .unwrap();

    // 2. Parse Go source containing func (s *OrderService) ExportCSV
    let go_source = r#"
package orders
type OrderService struct {}
func (s *OrderService) ExportCSV() error { return nil }
func (s *OrderService) Delete() error { return nil }
"#;

    let ast = extract_go_ast(go_source).unwrap();
    let export_method = ast.methods.iter().find(|m| m.method_name == "ExportCSV").unwrap();
    let delete_method = ast.methods.iter().find(|m| m.method_name == "Delete").unwrap();

    // Acceptance Gate Assertions:
    // a. OrderService.ExportCSV -> MATCH
    assert_eq!(
        validate_symbol_against_contract(&contract, &export_method.name),
        SymbolMatchResult::Match,
        "Contract symbol 'OrderService.ExportCSV' must MATCH Go receiver method func (s *OrderService) ExportCSV"
    );

    // b. Also matches if contract used pointer prefix '*OrderService.ExportCSV'
    assert!(matches_contract_symbol("*OrderService.ExportCSV", &export_method.name));

    // c. OrderService.Delete -> NO MATCH
    assert_eq!(
        validate_symbol_against_contract(&contract, &delete_method.name),
        SymbolMatchResult::NoMatch,
        "Contract symbol 'OrderService.ExportCSV' must NOT match 'OrderService.Delete'"
    );

    fs::remove_dir_all(&root).unwrap();
}

/// 6. EXTRACTED import edge -> Grounding -> Context Selection
#[test]
fn test_go_extracted_import_grounding_context_selection() {
    let root = temp_repo_dir("go_grounding");

    // 1. Create go.mod
    fs::write(root.join("go.mod"), "module github.com/acme/shop\n\ngo 1.22\n").unwrap();

    // 2. Create internal/orders/service.go importing payment
    let orders_dir = root.join("internal").join("orders");
    fs::create_dir_all(&orders_dir).unwrap();
    fs::write(
        orders_dir.join("service.go"),
        r#"package orders

import "github.com/acme/shop/internal/payment"

type OrderService struct {
    processor *payment.PaymentProcessor
}
"#,
    )
    .unwrap();

    // 3. Create internal/payment/processor.go
    let payment_dir = root.join("internal").join("payment");
    fs::create_dir_all(&payment_dir).unwrap();
    fs::write(
        payment_dir.join("processor.go"),
        r#"package payment

type PaymentProcessor struct {}
"#,
    )
    .unwrap();

    // 4. Run repository indexing (WAL analysis)
    let summary = analyze(&root).unwrap();
    assert!(summary.indexed_files >= 2);

    // 5. Compile context for a task targeting OrderService
    let ctx = context(&root, "Refactor OrderService processor", 4000).unwrap();

    let file_paths: Vec<String> = ctx.files.iter().map(|f| f.path.clone()).collect();

    // Invariant: Orders service is retrieved by BM25
    assert!(
        file_paths.iter().any(|p| p.contains("service.go")),
        "service.go must be selected in context"
    );

    // Invariant: Payment processor is pulled in via EXTRACTED graph IMPORTS edge!
    let payment_file = ctx
        .files
        .iter()
        .find(|f| f.path.contains("processor.go"))
        .expect("payment processor.go must be grounded into context via 1-hop Go IMPORTS graph edge");

    assert!(
        payment_file
            .provenance
            .iter()
            .any(|prov| prov.contains("Graph IMPORTS edge")),
        "Payment file provenance must show Graph IMPORTS edge: {:?}",
        payment_file.provenance
    );

    fs::remove_dir_all(&root).unwrap();
}

/// 7. Change Guard contract boundary enforcement on Go files
#[test]
fn test_go_change_guard_contract_enforcement() {
    let root = temp_repo_dir("go_contract_guard");

    let contract = create_contract(
        &root,
        "Implement Go order exporter",
        Some(vec!["orders".to_string()]),
        Some(vec![
            "internal/orders/service.go".to_string(),
            "internal/orders/service_test.go".to_string(),
        ]),
        Some(vec!["legacy/**".to_string()]),
    )
    .unwrap();

    let changed = vec![
        "internal/orders/service.go".to_string(),      // Expected
        "internal/orders/formatter.go".to_string(),     // ExpectedExpansion (same module)
        "internal/metrics/stats.go".to_string(),       // Unexpected
        "legacy/db.go".to_string(),                    // BoundaryViolation
    ];

    let (classifications, _, summary) =
        validate_contract_diff_with_constitution(&root, &contract, &changed);

    assert_eq!(summary.expected_count, 1);
    assert_eq!(summary.expansion_count, 1);
    assert_eq!(summary.unexpected_count, 1);
    assert_eq!(summary.violation_count, 1);

    assert_eq!(
        classifications.iter().find(|c| c.path == "internal/orders/service.go").unwrap().category,
        FileValidationCategory::Expected
    );
    assert_eq!(
        classifications.iter().find(|c| c.path == "internal/orders/formatter.go").unwrap().category,
        FileValidationCategory::ExpectedExpansion
    );
    assert_eq!(
        classifications.iter().find(|c| c.path == "internal/metrics/stats.go").unwrap().category,
        FileValidationCategory::Unexpected
    );
    assert_eq!(
        classifications.iter().find(|c| c.path == "legacy/db.go").unwrap().category,
        FileValidationCategory::BoundaryViolation
    );

    fs::remove_dir_all(&root).unwrap();
}

/// 8. Constitution Precedence Invariant over Change Contract on Go files
#[test]
fn test_go_constitution_precedence_over_contract() {
    let root = temp_repo_dir("go_constitution_precedence");

    // 1. Define Project Constitution rule forbidding auth modifications
    let engram_dir = root.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    let constitution_yaml = r#"
schema_version: 1
rules:
  - id: RULE-GO-AUTH-IMMUTABLE
    name: "Go Auth Layer Immutable"
    type: DEPENDENCY
    severity: ERROR
    forbidden: "internal/auth/**"
    description: "Core authentication package cannot be modified by autonomous agents."
"#;
    fs::write(engram_dir.join("constitution.yaml"), constitution_yaml).unwrap();

    // 2. Agent creates contract naively marking internal/auth/auth.go as EXPECTED
    let contract = create_contract(
        &root,
        "Update OAuth tokens",
        Some(vec!["auth".to_string()]),
        Some(vec!["internal/auth/auth.go".to_string()]),
        None,
    )
    .unwrap();

    // 3. Agent modifies internal/auth/auth.go
    let changed = vec!["internal/auth/auth.go".to_string()];

    // 4. Validate with Constitution Precedence
    let (classifications, _, summary) =
        validate_contract_diff_with_constitution(&root, &contract, &changed);

    // Invariant: Constitution overrides contract expectation to BOUNDARY_VIOLATION!
    assert_eq!(summary.violation_count, 1);
    assert_eq!(summary.expected_count, 0);

    let auth_change = &classifications[0];
    assert_eq!(auth_change.category, FileValidationCategory::BoundaryViolation);
    assert!(auth_change.reason.contains("Constitution rule 'Go Auth Layer Immutable'"));

    fs::remove_dir_all(&root).unwrap();
}
