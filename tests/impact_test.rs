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
        assert!(
            !e.reason.is_empty(),
            "Entity {} must have causal reason",
            e.path
        );
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
    assert!(
        auth.authorized_targets
            .iter()
            .any(|t| t.replace('\\', "/") == "internal/orders/service.go")
    );

    // 2. cmd/server/main.go was predicted in the blast radius (imported orders),
    // BUT is NOT authorized because module is 'server', not 'orders'!
    assert!(
        auth.unauthorized_relevant
            .iter()
            .any(|u| u.replace('\\', "/") == "cmd/server/main.go"),
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
    assert!(
        seeded_contract
            .boundary
            .expected_files
            .iter()
            .any(|f| f.replace('\\', "/") == "internal/orders/service.go")
    );
    assert!(
        seeded_contract
            .boundary
            .expected_files
            .iter()
            .any(|f| f.replace('\\', "/") == "internal/orders/service_test.go")
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_context_compiler_impact_ranking() {
    let root = setup_go_ecommerce_fixture();
    analyze(&root).unwrap();

    // Compile context for task matching OrderService
    let ctx = context(&root, "Export CSV from OrderService", 1200).unwrap();

    // Direct file must be ranked higher than transitive caller
    let service_idx = ctx
        .files
        .iter()
        .position(|f| f.path.replace('\\', "/") == "internal/orders/service.go");
    assert!(
        service_idx.is_some(),
        "internal/orders/service.go must be packed in context"
    );

    let service_file = &ctx.files[service_idx.unwrap()];
    // Must contain impact provenance annotation
    assert!(
        service_file.provenance.iter().any(|p| p.contains("Impact")),
        "File provenance must include Impact ranking information: {:?}",
        service_file.provenance
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_angular_module_impact_resolution() {
    let root = std::env::temp_dir().join(format!("engram_angular_impact_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src/app/core")).unwrap();

    // 1. core.module.ts
    fs::write(
        root.join("src/app/core/core.module.ts"),
        "export class CoreModule {}\n",
    )
    .unwrap();

    // 2. app.module.ts imports core.module using typical Angular relative specifier without .ts
    fs::write(
        root.join("src/app/app.module.ts"),
        "import { CoreModule } from './core/core.module';\n\nexport class AppModule {}\n",
    )
    .unwrap();

    // Analyze the workspace
    analyze(&root).unwrap();

    let conn = db(&root).unwrap();

    // A. Querying by full path "src/app/core/core.module.ts" must find app.module.ts as dependent
    let cone_full = compute_impact_cone(&conn, &root, "src/app/core/core.module.ts", 2).unwrap();
    assert!(
        cone_full
            .import_dependent_entities
            .iter()
            .any(|e| e.path.replace('\\', "/").ends_with("src/app/app.module.ts")),
        "Full path query must detect app.module.ts as import dependent: {:?}",
        cone_full.import_dependent_entities
    );

    // B. Querying by bare filename "core.module.ts" must also resolve and detect app.module.ts
    let cone_bare = compute_impact_cone(&conn, &root, "core.module.ts", 2).unwrap();
    assert!(
        cone_bare
            .import_dependent_entities
            .iter()
            .any(|e| e.path.replace('\\', "/").ends_with("src/app/app.module.ts")),
        "Bare filename query 'core.module.ts' must detect app.module.ts as import dependent: {:?}",
        cone_bare.import_dependent_entities
    );

    // C. Format explanation should include the dependent
    let explanation = format_impact_explanation(&cone_bare);
    assert!(explanation.contains("app.module.ts"));
    assert!(!explanation.contains("No downstream dependents"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_generic_symbol_and_qualified_symbol_containment() {
    let root = std::env::temp_dir().join(format!("engram_generic_sym_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src/services")).unwrap();
    fs::create_dir_all(root.join("src/components")).unwrap();

    // 1. auth.service.ts with a constructor
    fs::write(
        root.join("src/services/auth.service.ts"),
        "export class AuthService {\n  constructor() {}\n  login() {}\n}\n",
    )
    .unwrap();

    // 2. user.service.ts with a constructor
    fs::write(
        root.join("src/services/user.service.ts"),
        "export class UserService {\n  constructor() {}\n  getUser() {}\n}\n",
    )
    .unwrap();

    // 3. login.component.ts imports AuthService
    fs::write(
        root.join("src/components/login.component.ts"),
        "import { AuthService } from '../services/auth.service';\nexport class LoginComponent {\n  constructor(auth: AuthService) {}\n}\n",
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    // A. Suffix resolution
    let cone_suffix = compute_impact_cone(&conn, &root, "auth.service.ts", 2).unwrap();
    assert!(
        cone_suffix
            .import_dependent_entities
            .iter()
            .any(|e| e.path.replace('\\', "/").ends_with("src/components/login.component.ts")),
        "Suffix resolution for 'auth.service.ts' must find login.component.ts: {:?}",
        cone_suffix.import_dependent_entities
    );

    // B. Canonical path resolution
    let cone_canonical = compute_impact_cone(&conn, &root, "src/services/auth.service.ts", 2).unwrap();
    assert!(
        cone_canonical
            .import_dependent_entities
            .iter()
            .any(|e| e.path.replace('\\', "/").ends_with("src/components/login.component.ts")),
        "Canonical path query must find login.component.ts"
    );

    // C. Qualified symbol resolution
    let cone_qual = compute_impact_cone(&conn, &root, "src/services/auth.service.ts::AuthService", 2).unwrap();
    assert!(
        cone_qual.direct_entities.iter().any(|e| e.symbol_name.as_deref() == Some("AuthService")),
        "Qualified target must resolve AuthService"
    );

    // D. Qualified generic method 'constructor' stays contained to auth.service.ts
    let cone_ctor_qual = compute_impact_cone(&conn, &root, "src/services/auth.service.ts::constructor", 2).unwrap();
    for e in &cone_ctor_qual.direct_entities {
        assert_eq!(
            e.path.replace('\\', "/"),
            "src/services/auth.service.ts",
            "Qualified constructor target must NOT leak to other files: {:?}",
            e.path
        );
    }

    // E. Guard impact reverse dependency query with suffix and canonical paths
    let guard_suffix = engram::guard::impact(&root, "auth.service.ts").unwrap();
    let rev_deps_suffix = guard_suffix["reverse_dependencies"].as_array().unwrap();
    assert!(
        rev_deps_suffix.iter().any(|d| d["source"].as_str().unwrap_or("").replace('\\', "/").ends_with("src/components/login.component.ts")),
        "guard::impact with suffix 'auth.service.ts' must find login.component.ts: {:?}",
        rev_deps_suffix
    );

    let guard_canonical = engram::guard::impact(&root, "src/services/auth.service.ts").unwrap();
    let rev_deps_canonical = guard_canonical["reverse_dependencies"].as_array().unwrap();
    assert!(
        rev_deps_canonical.iter().any(|d| d["source"].as_str().unwrap_or("").replace('\\', "/").ends_with("src/components/login.component.ts")),
        "guard::impact with canonical path must find login.component.ts"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_windows_path_normalization_and_ambiguous_suffix_resolution() {
    let root = std::env::temp_dir().join(format!("engram_win_path_test_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src/services")).unwrap();
    fs::create_dir_all(root.join("src/admin")).unwrap();
    fs::create_dir_all(root.join("src/common")).unwrap();
    fs::create_dir_all(root.join("src/components")).unwrap();

    // 1. Two files with the same basename in separate directories:
    //    a) src/services/auth.service.ts
    fs::write(
        root.join("src/services/auth.service.ts"),
        "export class AuthService {\n  login() {}\n}\n",
    )
    .unwrap();

    //    b) src/admin/auth.service.ts
    fs::write(
        root.join("src/admin/auth.service.ts"),
        "export class AdminAuthService {\n  adminLogin() {}\n}\n",
    )
    .unwrap();

    // 2. Unique file in common:
    //    src/common/logger.service.ts
    fs::write(
        root.join("src/common/logger.service.ts"),
        "export class LoggerService {\n  log(msg: string) {}\n}\n",
    )
    .unwrap();

    // 3. Components importing these files:
    //    a) login.component.ts -> imports src/services/auth.service.ts
    fs::write(
        root.join("src/components/login.component.ts"),
        "import { AuthService } from '../services/auth.service';\nexport class LoginComponent {}\n",
    )
    .unwrap();

    //    b) admin.component.ts -> imports src/admin/auth.service.ts
    fs::write(
        root.join("src/components/admin.component.ts"),
        "import { AdminAuthService } from '../admin/auth.service';\nexport class AdminComponent {}\n",
    )
    .unwrap();

    //    c) dashboard.component.ts -> imports src/common/logger.service.ts
    fs::write(
        root.join("src/components/dashboard.component.ts"),
        "import { LoggerService } from '../common/logger.service';\nexport class DashboardComponent {}\n",
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    // -------------------------------------------------------------
    // Test 1: Windows-style path input (with backslashes)
    // -------------------------------------------------------------
    let win_canonical = engram::guard::impact(&root, r"src\common\logger.service.ts").unwrap();
    assert_eq!(win_canonical.get("ambiguous"), None);
    let rev_deps = win_canonical["reverse_dependencies"].as_array().unwrap();
    assert!(
        rev_deps.iter().any(|d| d["source"].as_str().unwrap_or("").replace('\\', "/").ends_with("src/components/dashboard.component.ts")),
        "Windows canonical path must resolve and find dashboard.component.ts"
    );

    let win_suffix = engram::guard::impact(&root, r"common\logger.service.ts").unwrap();
    assert_eq!(win_suffix.get("ambiguous"), None);
    let rev_deps_win_suf = win_suffix["reverse_dependencies"].as_array().unwrap();
    assert!(
        rev_deps_win_suf.iter().any(|d| d["source"].as_str().unwrap_or("").replace('\\', "/").ends_with("src/components/dashboard.component.ts")),
        "Windows suffix path must resolve and find dashboard.component.ts"
    );

    let cone_win = compute_impact_cone(&conn, &root, r"src\common\logger.service.ts", 2).unwrap();
    assert!(
        cone_win.import_dependent_entities.iter().any(|e| e.path.replace('\\', "/").ends_with("src/components/dashboard.component.ts")),
        "compute_impact_cone must accept Windows backslash paths"
    );

    // -------------------------------------------------------------
    // Test 2: Two files with the same basename in separate directories (Ambiguous Suffix)
    // -------------------------------------------------------------
    let guard_ambiguous = engram::guard::impact(&root, "auth.service.ts").unwrap();
    assert_eq!(
        guard_ambiguous["ambiguous"].as_bool(),
        Some(true),
        "Bare suffix matching two files must report ambiguous: true"
    );
    let matches = guard_ambiguous["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 2, "Must list exactly 2 matching candidate paths");
    let match_strs: Vec<String> = matches.iter().filter_map(|m| m.as_str().map(|s| s.replace('\\', "/"))).collect();
    assert!(match_strs.iter().any(|s| s.ends_with("src/admin/auth.service.ts")));
    assert!(match_strs.iter().any(|s| s.ends_with("src/services/auth.service.ts")));
    assert!(guard_ambiguous["reverse_dependencies"].as_array().unwrap().is_empty());

    let cone_ambiguous_err = compute_impact_cone(&conn, &root, "auth.service.ts", 2);
    assert!(cone_ambiguous_err.is_err(), "compute_impact_cone must error on ambiguous suffix");
    let err_msg = cone_ambiguous_err.unwrap_err().to_string();
    assert!(err_msg.contains("ambiguous"), "Error message must indicate ambiguity: {}", err_msg);

    // -------------------------------------------------------------
    // Test 3: Exact canonical paths still resolving deterministically
    // -------------------------------------------------------------
    let guard_services = engram::guard::impact(&root, "src/services/auth.service.ts").unwrap();
    assert_eq!(guard_services.get("ambiguous"), None);
    let deps_services = guard_services["reverse_dependencies"].as_array().unwrap();
    assert!(
        deps_services.iter().any(|d| d["source"].as_str().unwrap_or("").replace('\\', "/").ends_with("src/components/login.component.ts")),
        "src/services/auth.service.ts must resolve only to login.component.ts"
    );
    assert!(
        !deps_services.iter().any(|d| d["source"].as_str().unwrap_or("").replace('\\', "/").ends_with("src/components/admin.component.ts")),
        "src/services/auth.service.ts must NOT link to admin.component.ts"
    );

    let guard_admin = engram::guard::impact(&root, "src/admin/auth.service.ts").unwrap();
    assert_eq!(guard_admin.get("ambiguous"), None);
    let deps_admin = guard_admin["reverse_dependencies"].as_array().unwrap();
    assert!(
        deps_admin.iter().any(|d| d["source"].as_str().unwrap_or("").replace('\\', "/").ends_with("src/components/admin.component.ts")),
        "src/admin/auth.service.ts must resolve only to admin.component.ts"
    );
    assert!(
        !deps_admin.iter().any(|d| d["source"].as_str().unwrap_or("").replace('\\', "/").ends_with("src/components/login.component.ts")),
        "src/admin/auth.service.ts must NOT link to login.component.ts"
    );

    let guard_suf_admin = engram::guard::impact(&root, "admin/auth.service.ts").unwrap();
    assert_eq!(guard_suf_admin.get("ambiguous"), None);
    assert!(guard_suf_admin["reverse_dependencies"].as_array().unwrap().iter().any(|d| d["source"].as_str().unwrap_or("").replace('\\', "/").ends_with("src/components/admin.component.ts")));

    // -------------------------------------------------------------
    // Test 4: Qualified symbols remaining correctly scoped
    // -------------------------------------------------------------
    let guard_qual = engram::guard::impact(&root, r"src\services\auth.service.ts::AuthService").unwrap();
    assert_eq!(guard_qual.get("ambiguous"), None);
    assert_eq!(guard_qual["resolved_target"], "src/services/auth.service.ts::AuthService");

    let _ = fs::remove_dir_all(&root);
}
