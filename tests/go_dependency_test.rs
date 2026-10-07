//! Integration Test Suite for Engram v0.6.0: Go Dependency Intelligence & Interface Inference
//! Verifies full go.mod directives, resolution taxonomy (LOCAL, VENDORED, REPLACED_LOCAL,
//! REPLACED_EXTERNAL, EXTERNAL, UNRESOLVED), strict invariant enforcement,
//! two-stage structural interface satisfaction (candidate lookup + full signature comparison),
//! and end-to-end graph context grounding.

use std::fs;
use std::path::{Path, PathBuf};

use engram::analyzer::analyze;
use engram::compiler::context;
use engram::go_parser::{
    check_struct_implements_interface, extract_go_ast, parse_full_go_mod,
    resolve_go_dependency_origin, GoMethod, GoModDocument, GoReplace, GoRequire, PackageOrigin,
};
use engram::graph::GraphTraversal;
use engram::model::Direction;
use engram::storage::db;

fn temp_repo_dir(prefix: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("engram_go_dep_{}_{}", prefix, uuid::Uuid::new_v4()));
    fs::create_dir_all(&p).unwrap();
    let _ = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&p)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "EngramTester"])
        .current_dir(&p)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "tester@engram.dev"])
        .current_dir(&p)
        .output();
    p
}

/// 1. Full go.mod directive parsing and resolution
#[test]
fn test_full_go_mod_parsing_and_resolution() {
    let go_mod_content = r#"
module github.com/acme/shop

go 1.22
toolchain go1.22.5

require (
	github.com/google/uuid v1.6.0
	golang.org/x/sync v0.6.0 // indirect
)

replace (
	github.com/acme/common => ./local/common
	golang.org/x/crypto => github.com/golang/crypto v0.1.0
)

exclude (
	github.com/bad/pkg v1.0.0
)
"#;

    let doc = parse_full_go_mod(go_mod_content);
    assert_eq!(doc.module, Some("github.com/acme/shop".to_string()));
    assert_eq!(doc.go_version, Some("1.22".to_string()));
    assert_eq!(doc.toolchain, Some("go1.22.5".to_string()));

    // Requires (direct and indirect)
    assert_eq!(doc.requires.len(), 2);
    assert_eq!(doc.requires[0].path, "github.com/google/uuid");
    assert_eq!(doc.requires[0].version, "v1.6.0");
    assert!(!doc.requires[0].indirect);
    assert_eq!(doc.requires[1].path, "golang.org/x/sync");
    assert!(doc.requires[1].indirect);

    // Replaces (local directory vs external module)
    assert_eq!(doc.replaces.len(), 2);
    assert_eq!(doc.replaces[0].old_path, "github.com/acme/common");
    assert_eq!(doc.replaces[0].new_path, "./local/common");
    assert!(doc.replaces[0].is_local_dir);

    assert_eq!(doc.replaces[1].old_path, "golang.org/x/crypto");
    assert_eq!(doc.replaces[1].new_path, "github.com/golang/crypto");
    assert_eq!(doc.replaces[1].new_version, Some("v0.1.0".to_string()));
    assert!(!doc.replaces[1].is_local_dir);

    // Excludes
    assert_eq!(doc.excludes.len(), 1);
    assert!(doc.excludes[0].contains("github.com/bad/pkg"));
}

/// 2. Vendored dependency resolution precedence
#[test]
fn test_vendored_dependency_precedence() {
    let root = temp_repo_dir("vendored");
    let vendor_pkg_dir = root.join("vendor/github.com/secure/vault");
    fs::create_dir_all(&vendor_pkg_dir).unwrap();
    fs::write(vendor_pkg_dir.join("vault.go"), "package vault\ntype Client struct {}\n").unwrap();

    let doc = GoModDocument {
        module: Some("github.com/acme/shop".to_string()),
        requires: vec![GoRequire {
            path: "github.com/secure/vault".to_string(),
            version: "v2.0.0".to_string(),
            indirect: false,
        }],
        ..Default::default()
    };

    let (origin, targets) = resolve_go_dependency_origin(
        &root,
        "internal/service/service.go",
        "github.com/secure/vault",
        Some(&doc),
    );

    assert_eq!(origin, PackageOrigin::Vendored);
    assert_eq!(origin.as_str(), "VENDORED");
    assert!(targets.iter().any(|t| t.contains("vendor/github.com/secure/vault")));

    fs::remove_dir_all(&root).unwrap();
}

/// 3. Replaced local and replaced external resolution
#[test]
fn test_replaced_local_and_external_resolution() {
    let root = temp_repo_dir("replaced");
    let local_common_dir = root.join("local/common");
    fs::create_dir_all(&local_common_dir).unwrap();
    fs::write(local_common_dir.join("logger.go"), "package common\ntype Logger struct {}\n").unwrap();

    let doc = GoModDocument {
        module: Some("github.com/acme/shop".to_string()),
        replaces: vec![
            GoReplace {
                old_path: "github.com/acme/common".to_string(),
                old_version: None,
                new_path: "./local/common".to_string(),
                new_version: None,
                is_local_dir: true,
            },
            GoReplace {
                old_path: "golang.org/x/crypto".to_string(),
                old_version: None,
                new_path: "github.com/golang/crypto".to_string(),
                new_version: Some("v0.1.0".to_string()),
                is_local_dir: false,
            },
        ],
        ..Default::default()
    };

    // Replaced Local
    let (origin_local, targets_local) = resolve_go_dependency_origin(
        &root,
        "internal/service.go",
        "github.com/acme/common",
        Some(&doc),
    );
    assert_eq!(origin_local, PackageOrigin::ReplacedLocal);
    assert_eq!(origin_local.as_str(), "REPLACED_LOCAL");
    assert!(targets_local.iter().any(|t| t.contains("local/common")));

    // Replaced External
    let (origin_ext, targets_ext) = resolve_go_dependency_origin(
        &root,
        "internal/service.go",
        "golang.org/x/crypto",
        Some(&doc),
    );
    assert_eq!(origin_ext, PackageOrigin::ReplacedExternal);
    assert_eq!(origin_ext.as_str(), "REPLACED_EXTERNAL");
    assert_eq!(targets_ext, vec!["github.com/golang/crypto".to_string()]);

    fs::remove_dir_all(&root).unwrap();
}

/// 4. Invariant: An unresolved dependency must remain UNRESOLVED and must never silently degrade into an inferred relationship.
#[test]
fn test_unresolved_dependency_evidence_integrity() {
    let root = temp_repo_dir("unresolved");

    let doc = GoModDocument {
        module: Some("github.com/acme/shop".to_string()),
        requires: vec![GoRequire {
            path: "github.com/google/uuid".to_string(),
            version: "v1.6.0".to_string(),
            indirect: false,
        }],
        ..Default::default()
    };

    // Missing / undeclared dependency
    let (origin, targets) = resolve_go_dependency_origin(
        &root,
        "internal/orders.go",
        "github.com/acme/ghost_package",
        Some(&doc),
    );

    assert_eq!(origin, PackageOrigin::Unresolved);
    assert_eq!(origin.as_str(), "UNRESOLVED");
    assert_eq!(targets, vec!["github.com/acme/ghost_package".to_string()]);

    // Parse file importing unresolved dependency
    let source = "package orders\nimport \"github.com/acme/ghost_package\"\n";
    let ast = extract_go_ast(source).unwrap();
    let (_, rels) = engram::go_parser::convert_go_ast_to_engram_with_root(
        &root,
        "internal/orders.go",
        &ast,
        Some(&doc),
    );

    let imp_rel = rels.iter().find(|r| r.target == "github.com/acme/ghost_package").unwrap();
    assert_eq!(imp_rel.evidence, "UNRESOLVED");
    assert_eq!(imp_rel.confidence, 0.0);

    fs::remove_dir_all(&root).unwrap();
}

/// 5. Invariant: AMstraLift/Engram must never report an external dependency as local merely because a matching-looking directory exists.
#[test]
fn test_external_not_confused_with_local_subpath() {
    let root = temp_repo_dir("external_vs_local");
    // Accidentally matching directory in workspace:
    fs::create_dir_all(root.join("github.com/google/uuid")).unwrap();
    fs::write(root.join("github.com/google/uuid/uuid.go"), "package uuid\n").unwrap();

    let doc = GoModDocument {
        module: Some("github.com/acme/shop".to_string()),
        requires: vec![GoRequire {
            path: "github.com/google/uuid".to_string(),
            version: "v1.6.0".to_string(),
            indirect: false,
        }],
        ..Default::default()
    };

    let (origin, _) = resolve_go_dependency_origin(
        &root,
        "internal/service.go",
        "github.com/google/uuid",
        Some(&doc),
    );

    // Must be classified as EXTERNAL, NEVER LOCAL!
    assert_eq!(origin, PackageOrigin::External);
    assert_eq!(origin.as_str(), "EXTERNAL");

    fs::remove_dir_all(&root).unwrap();
}

/// 6. Two-stage interface satisfaction: Stage 1 (candidate name lookup) + Stage 2 (exact signature match)
#[test]
fn test_two_stage_interface_satisfaction_match() {
    let source = r#"
package orders

type Exporter interface {
    ExportCSV(path string) error
}

type OrderService struct {}

func (s *OrderService) ExportCSV(path string) error {
    return nil
}
"#;

    let ast = extract_go_ast(source).unwrap();
    assert_eq!(ast.interfaces.len(), 1);
    let iface = &ast.interfaces[0];
    assert_eq!(iface.name, "Exporter");
    assert_eq!(iface.methods.len(), 1);
    assert_eq!(iface.methods[0].name, "ExportCSV");
    assert_eq!(iface.methods[0].params, vec!["string"]);
    assert_eq!(iface.methods[0].results, vec!["error"]);

    let struct_methods: Vec<&GoMethod> = ast.methods.iter().collect();
    assert!(check_struct_implements_interface(&struct_methods, iface));

    // Convert to relationships
    let dummy_root = Path::new(".");
    let (_, rels) = engram::go_parser::convert_go_ast_to_engram_with_root(
        dummy_root,
        "orders/service.go",
        &ast,
        None,
    );

    let impl_rel = rels.iter().find(|r| r.kind == "IMPLEMENTS").unwrap();
    assert_eq!(impl_rel.source, "orders/service.go::OrderService");
    assert_eq!(impl_rel.target, "orders/service.go::Exporter");
    assert_eq!(impl_rel.evidence, "EXTRACTED");
    assert_eq!(impl_rel.confidence, 0.95);
}

/// 7. Two-stage interface satisfaction: name matches but signature fails -> MUST NOT implement!
#[test]
fn test_two_stage_interface_satisfaction_signature_mismatch() {
    let source = r#"
package orders

type Exporter interface {
    ExportCSV(path string) error
}

type OrderService struct {}

func (s *OrderService) ExportCSV(count int) error {
    return nil
}
"#;

    let ast = extract_go_ast(source).unwrap();
    let iface = &ast.interfaces[0];
    let struct_methods: Vec<&GoMethod> = ast.methods.iter().collect();

    // Must NOT match even though method name "ExportCSV" is identical!
    assert!(!check_struct_implements_interface(&struct_methods, iface));

    let dummy_root = Path::new(".");
    let (_, rels) = engram::go_parser::convert_go_ast_to_engram_with_root(
        dummy_root,
        "orders/service.go",
        &ast,
        None,
    );

    assert!(rels.iter().all(|r| r.kind != "IMPLEMENTS"));
}

/// 8. Cross-file interface implementation & graph context grounding
#[test]
fn test_interface_implementation_graph_grounding() {
    let root = temp_repo_dir("go_iface_grounding");

    fs::write(
        root.join("go.mod"),
        "module github.com/acme/shop\n\ngo 1.22\n",
    )
    .unwrap();

    let export_dir = root.join("pkg/export");
    fs::create_dir_all(&export_dir).unwrap();
    fs::write(
        export_dir.join("exporter.go"),
        r#"package export

type Exporter interface {
    Export() error
}
"#,
    )
    .unwrap();

    let orders_dir = root.join("internal/orders");
    fs::create_dir_all(&orders_dir).unwrap();
    fs::write(
        orders_dir.join("service.go"),
        r#"package orders

type OrderService struct {}

func (s *OrderService) Export() error {
    return nil
}
"#,
    )
    .unwrap();

    // Analyze workspace
    let summary = analyze(&root).unwrap();
    assert_eq!(summary.indexed_files, 2);

    // Verify IMPLEMENTS relationship stored in database
    let conn = db(&root).unwrap();
    let traversal = GraphTraversal::new(&conn);

    let iface_edges = traversal
        .neighbors("internal/orders/service.go", &["IMPLEMENTS"], Direction::Both, 2)
        .unwrap();

    assert!(
        iface_edges.iter().any(|e| e.kind == "IMPLEMENTS" && e.target.contains("Exporter")),
        "Expected IMPLEMENTS relationship connecting OrderService to Exporter!"
    );

    // Context compilation with graph expansion pulls implementing interface
    let ctx = context(&root, "OrderService streaming implementation", 1500).unwrap();
    assert!(ctx.files.iter().any(|f| f.path == "internal/orders/service.go"));
    assert!(ctx.files.iter().any(|f| f.path == "pkg/export/exporter.go"));

    let iface_file = ctx.files.iter().find(|f| f.path == "pkg/export/exporter.go").unwrap();
    assert!(
        iface_file.provenance.iter().any(|p| p.contains("IMPLEMENTS")),
        "Expected IMPLEMENTS graph edge provenance on exporter.go!"
    );

    fs::remove_dir_all(&root).unwrap();
}
