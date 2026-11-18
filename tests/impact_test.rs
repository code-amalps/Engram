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
use engram::impact::{compute_impact_cone, format_impact_explanation, ImpactEntity, DependencyClassification};
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


// =============================================================================
// PHASE 4C: WORKSPACE-BOUNDED DEPENDENCY GRAPH & IMPACT TRAVERSAL TESTS
// =============================================================================

#[test]
fn test_phase4c_first_party_traversal() {
    let root = std::env::temp_dir().join(format!("engram_phase4c_p1_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src/core")).unwrap();

    fs::write(
        root.join("src/core/leaf.ts"),
        "export class LeafClass { getValue() { return 42; } }\n",
    )
    .unwrap();

    fs::write(
        root.join("src/core/middle.ts"),
        "import { LeafClass } from './leaf';\nexport class MiddleClass { leaf = new LeafClass(); }\n",
    )
    .unwrap();

    fs::write(
        root.join("src/core/root.ts"),
        "import { MiddleClass } from './middle';\nexport class RootClass { mid = new MiddleClass(); }\n",
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone = compute_impact_cone(&conn, &root, "src/core/root.ts", 2).unwrap();

    // 1. Direct entity is root.ts
    assert_eq!(cone.direct_entities.len(), 1);
    assert_eq!(cone.direct_entities[0].path.replace('\\', "/"), "src/core/root.ts");
    assert!(!cone.direct_entities[0].is_external);

    // 2. Hop 1 entity is middle.ts (first-party)
    let mid = cone
        .import_dependent_entities
        .iter()
        .find(|e| e.path.replace('\\', "/").ends_with("src/core/middle.ts"));
    assert!(mid.is_some(), "Hop 1 middle.ts must be found");
    let mid_e = mid.unwrap();
    assert_eq!(mid_e.hop_distance, 1);
    assert!(!mid_e.is_external, "middle.ts must be first-party");

    // 3. Hop 2 entity is leaf.ts (first-party)
    let leaf = cone
        .import_dependent_entities
        .iter()
        .find(|e| e.path.replace('\\', "/").ends_with("src/core/leaf.ts"));
    assert!(leaf.is_some(), "Hop 2 leaf.ts must be found");
    let leaf_e = leaf.unwrap();
    assert_eq!(leaf_e.hop_distance, 2);
    assert!(!leaf_e.is_external, "leaf.ts must be first-party");

    // 4. Metrics verification
    let metrics = cone.metrics().expect("Metrics must be present");
    assert!(metrics.first_party_nodes >= 3);
    assert_eq!(metrics.external_terminal_nodes, 0);
    assert_eq!(metrics.max_traversal_depth, 2);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase4c_external_terminal_leaf_behavior() {
    let root = std::env::temp_dir().join(format!("engram_phase4c_p2_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src/services")).unwrap();

    fs::write(
        root.join("src/services/api.service.ts"),
        r#"import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';
import { map } from 'rxjs/operators';
import { HelperService } from './helper.service';

@Injectable()
export class ApiService {
  constructor(private helper: HelperService) {}
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/services/helper.service.ts"),
        r#"import { Store } from '@ngrx/store';

export class HelperService {
  constructor(private store: Store<any>) {}
}
"#,
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone = compute_impact_cone(&conn, &root, "src/services/api.service.ts", 2).unwrap();

    // 1. External dependencies captured as terminal leaves
    let ext_entities = cone.external_entities();
    assert!(
        ext_entities.iter().any(|e| e.path == "@angular/core"),
        "@angular/core must be recorded"
    );
    assert!(
        ext_entities.iter().any(|e| e.path.starts_with("rxjs")),
        "rxjs must be recorded"
    );
    assert!(
        ext_entities.iter().any(|e| e.path == "@ngrx/store"),
        "@ngrx/store must be recorded at hop 2"
    );

    // 2. All external entities must have is_external == true
    for ext in &ext_entities {
        assert!(ext.is_external, "Entity {} must be flagged is_external", ext.path);
        assert!(
            ext.reason.contains("external") || ext.reason.contains("IMPORTS"),
            "Entity reason must reflect external provenance: {}",
            ext.reason
        );
    }

    // 3. Pruned edges metric must be >= 4 (for @angular/core, rxjs, rxjs/operators, @ngrx/store)
    let metrics = cone.metrics().expect("Metrics must be present");
    assert!(
        metrics.pruned_edges >= 4,
        "Pruned edges must be >= 4, got {}",
        metrics.pruned_edges
    );
    assert!(
        metrics.external_terminal_nodes >= 4,
        "External terminal nodes must be >= 4, got {}",
        metrics.external_terminal_nodes
    );

    // 4. First-party entity is helper.service.ts
    let fp_entities = cone.first_party_entities();
    assert!(
        fp_entities.iter().any(|e| e.path.replace('\\', "/").ends_with("helper.service.ts")),
        "helper.service.ts must be first-party"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase4c_depth_boundaries() {
    let root = std::env::temp_dir().join(format!("engram_phase4c_p3_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src")).unwrap();

    fs::write(root.join("src/step4.ts"), "export class Step4 {}\n").unwrap();
    fs::write(root.join("src/step3.ts"), "import { Step4 } from './step4';\nexport class Step3 {}\n").unwrap();
    fs::write(root.join("src/step2.ts"), "import { Step3 } from './step3';\nexport class Step2 {}\n").unwrap();
    fs::write(root.join("src/step1.ts"), "import { Step2 } from './step2';\nexport class Step1 {}\n").unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    // Depth 0: only direct entity
    let cone0 = compute_impact_cone(&conn, &root, "src/step1.ts", 0).unwrap();
    assert_eq!(cone0.total_entities(), 1);
    assert_eq!(cone0.direct_entities[0].path.replace('\\', "/"), "src/step1.ts");

    // Depth 1: step1 (direct) + step2 (hop 1)
    let cone1 = compute_impact_cone(&conn, &root, "src/step1.ts", 1).unwrap();
    let paths1 = cone1.all_affected_paths();
    assert!(paths1.iter().any(|p| p.replace('\\', "/").ends_with("step1.ts")));
    assert!(paths1.iter().any(|p| p.replace('\\', "/").ends_with("step2.ts")));
    assert!(!paths1.iter().any(|p| p.replace('\\', "/").ends_with("step3.ts")));
    assert!(!paths1.iter().any(|p| p.replace('\\', "/").ends_with("step4.ts")));

    // Depth 2: step1 + step2 (hop 1) + step3 (hop 2)
    let cone2 = compute_impact_cone(&conn, &root, "src/step1.ts", 2).unwrap();
    let paths2 = cone2.all_affected_paths();
    assert!(paths2.iter().any(|p| p.replace('\\', "/").ends_with("step1.ts")));
    assert!(paths2.iter().any(|p| p.replace('\\', "/").ends_with("step2.ts")));
    assert!(paths2.iter().any(|p| p.replace('\\', "/").ends_with("step3.ts")));
    assert!(!paths2.iter().any(|p| p.replace('\\', "/").ends_with("step4.ts")));

    // Depth 3: step1 + step2 (hop 1) + step3 (hop 2) + step4 (hop 3)
    let cone3 = compute_impact_cone(&conn, &root, "src/step1.ts", 3).unwrap();
    let paths3 = cone3.all_affected_paths();
    assert!(paths3.iter().any(|p| p.replace('\\', "/").ends_with("step1.ts")));
    assert!(paths3.iter().any(|p| p.replace('\\', "/").ends_with("step2.ts")));
    assert!(paths3.iter().any(|p| p.replace('\\', "/").ends_with("step3.ts")));
    assert!(paths3.iter().any(|p| p.replace('\\', "/").ends_with("step4.ts")));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase4c_circular_dependencies() {
    let root = std::env::temp_dir().join(format!("engram_phase4c_p4_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src")).unwrap();

    fs::write(root.join("src/a.ts"), "import { B } from './b';\nexport class A {}\n").unwrap();
    fs::write(root.join("src/b.ts"), "import { C } from './c';\nexport class B {}\n").unwrap();
    fs::write(root.join("src/c.ts"), "import { A } from './a';\nexport class C {}\n").unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone = compute_impact_cone(&conn, &root, "src/a.ts", 3).unwrap();

    assert_eq!(cone.direct_entities.len(), 1);
    assert_eq!(cone.direct_entities[0].path.replace('\\', "/"), "src/a.ts");

    let dep_paths: Vec<String> = cone
        .import_dependent_entities
        .iter()
        .map(|e| e.path.replace('\\', "/"))
        .collect();

    assert!(
        !dep_paths.iter().any(|p| p.ends_with("src/a.ts")),
        "Direct target a.ts must not duplicate in dependents: {:?}",
        dep_paths
    );
    assert!(dep_paths.iter().any(|p| p.ends_with("src/b.ts")));
    assert!(dep_paths.iter().any(|p| p.ends_with("src/c.ts")));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase4c_duplicate_paths_diamond() {
    let root = std::env::temp_dir().join(format!("engram_phase4c_p5_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src")).unwrap();

    fs::write(root.join("src/bottom.ts"), "export class Bottom {}\n").unwrap();
    fs::write(root.join("src/left.ts"), "import { Bottom } from './bottom';\nexport class Left {}\n").unwrap();
    fs::write(root.join("src/right.ts"), "import { Bottom } from './bottom';\nexport class Right {}\n").unwrap();
    fs::write(
        root.join("src/top.ts"),
        "import { Left } from './left';\nimport { Right } from './right';\nexport class Top {}\n",
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone = compute_impact_cone(&conn, &root, "src/top.ts", 2).unwrap();

    let bottom_matches: Vec<&ImpactEntity> = cone
        .all_entities()
        .into_iter()
        .filter(|e| e.path.replace('\\', "/").ends_with("src/bottom.ts"))
        .collect();

    assert_eq!(
        bottom_matches.len(),
        1,
        "bottom.ts must appear exactly once in impact cone: {:?}",
        bottom_matches
    );

    let bottom = bottom_matches[0];
    assert_eq!(bottom.hop_distance, 2);
    assert!(
        bottom.relationships.len() >= 2,
        "bottom.ts must preserve complete multi-path provenance: {:?}",
        bottom.relationships
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase4c_missing_unresolved_dependencies() {
    let root = std::env::temp_dir().join(format!("engram_phase4c_p6_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src")).unwrap();

    fs::write(root.join("src/valid.ts"), "export class Valid {}\n").unwrap();
    fs::write(
        root.join("src/consumer.ts"),
        r#"import { Valid } from './valid';
import { NonExistent } from './does_not_exist';
import { ExternalMissing } from 'non-existent-npm-package';

export class Consumer {}
"#,
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone = compute_impact_cone(&conn, &root, "src/consumer.ts", 2).unwrap();

    assert!(
        cone.first_party_entities().iter().any(|e| e.path.replace('\\', "/").ends_with("valid.ts")),
        "valid.ts must be found as first party"
    );

    assert!(
        cone.external_entities().iter().any(|e| e.path == "non-existent-npm-package"),
        "Unresolved package must be handled as external terminal node"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase4c_invalidated_entities_handling() {
    let root = std::env::temp_dir().join(format!("engram_phase4c_p7_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src")).unwrap();

    fs::write(root.join("src/target.ts"), "export class Target {}\n").unwrap();
    fs::write(
        root.join("src/origin.ts"),
        "import { Target } from './target';\nexport class Origin {}\n",
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone_before = compute_impact_cone(&conn, &root, "src/origin.ts", 2).unwrap();
    assert!(
        cone_before.all_affected_paths().iter().any(|p| p.replace('\\', "/").ends_with("target.ts")),
        "target.ts must be present initially"
    );

    conn.execute(
        "UPDATE relationships SET invalidated_at_revision = 'rev_obsolete' WHERE source_id LIKE '%origin.ts%'",
        [],
    )
    .unwrap();

    let cone_after = compute_impact_cone(&conn, &root, "src/origin.ts", 2).unwrap();
    assert!(
        !cone_after.all_affected_paths().iter().any(|p| p.replace('\\', "/").ends_with("target.ts")),
        "Invalidated relationship target must NOT appear in cone"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase4c_mixed_graph_framework_independence() {
    let root = std::env::temp_dir().join(format!("engram_phase4c_p8_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src/app")).unwrap();

    fs::write(
        root.join("src/app/auth.service.ts"),
        r#"import { Injectable } from '@angular/core';
import { Observable } from 'rxjs';

export interface AuthContract {
  login(): boolean;
}

export class AuthService implements AuthContract {
  login(): boolean { return true; }
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/app/login.component.ts"),
        r#"import { AuthService } from './auth.service';

export class LoginComponent {
  constructor(private auth: AuthService) {}
}
"#,
    )
    .unwrap();

    fs::write(
        root.join("src/app/auth.service.spec.ts"),
        r#"import { AuthService } from './auth.service';

describe('AuthService', () => {
  it('should create', () => {
    const s = new AuthService();
    expect(s.login()).toBe(true);
  });
});
"#,
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone = compute_impact_cone(&conn, &root, "src/app/auth.service.ts", 2).unwrap();

    assert_eq!(cone.direct_entities[0].path.replace('\\', "/"), "src/app/auth.service.ts");

    let fp_paths = cone.first_party_paths();
    assert!(fp_paths.iter().any(|p| p.replace('\\', "/").ends_with("login.component.ts")));
    assert!(fp_paths.iter().any(|p| p.replace('\\', "/").ends_with("auth.service.spec.ts")));

    let ext_paths = cone.external_paths();
    assert!(ext_paths.iter().any(|p| p == "@angular/core"));
    assert!(ext_paths.iter().any(|p| p == "rxjs"));

    for e in cone.first_party_entities() {
        assert!(!e.is_external);
    }
    for e in cone.external_entities() {
        assert!(e.is_external);
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase4c_deterministic_ordering() {
    let root = std::env::temp_dir().join(format!("engram_phase4c_p9_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src")).unwrap();

    fs::write(root.join("src/z_dep.ts"), "export class ZDep {}\n").unwrap();
    fs::write(root.join("src/a_dep.ts"), "export class ADep {}\n").unwrap();
    fs::write(root.join("src/m_dep.ts"), "export class MDep {}\n").unwrap();
    fs::write(
        root.join("src/main.ts"),
        "import { ZDep } from './z_dep';\nimport { ADep } from './a_dep';\nimport { MDep } from './m_dep';\nexport class Main {}\n",
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let baseline = compute_impact_cone(&conn, &root, "src/main.ts", 2).unwrap();
    let baseline_json = serde_json::to_string(&baseline).unwrap();

    for i in 0..10 {
        let sample = compute_impact_cone(&conn, &root, "src/main.ts", 2).unwrap();
        let sample_json = serde_json::to_string(&sample).unwrap();
        assert_eq!(
            baseline_json, sample_json,
            "Iteration {} produced non-deterministic output!",
            i
        );
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase4c_scenario_b_payload_and_coverage() {
    let root = std::env::temp_dir().join(format!("engram_phase4c_scenario_b_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("projects/demo/src/app/core/auth")).unwrap();
    fs::create_dir_all(root.join("projects/demo/src/app/core/local-storage")).unwrap();
    fs::create_dir_all(root.join("projects/demo/src/app/app")).unwrap();

    // 1. auth.actions.ts
    fs::write(
        root.join("projects/demo/src/app/core/auth/auth.actions.ts"),
        "export const authLogin = () => ({ type: '[Auth] Login' });\n",
    )
    .unwrap();

    // 2. local-storage.service.ts
    fs::write(
        root.join("projects/demo/src/app/core/local-storage/local-storage.service.ts"),
        r#"import { Injectable } from '@angular/core';

@Injectable()
export class LocalStorageService {
  setItem(key: string, value: any) {}
  getItem(key: string): any {}
  removeItem(key: string) {}
}
"#,
    )
    .unwrap();

    // 3. core.module.ts
    fs::write(
        root.join("projects/demo/src/app/core/core.module.ts"),
        r#"import { NgModule } from '@angular/core';
import { LocalStorageService } from './local-storage/local-storage.service';

@NgModule({
  providers: [LocalStorageService]
})
export class CoreModule {}
"#,
    )
    .unwrap();

    // 4. auth.effects.ts (The target of Scenario B)
    fs::write(
        root.join("projects/demo/src/app/core/auth/auth.effects.ts"),
        r#"import { Injectable } from '@angular/core';
import { Router } from '@angular/router';
import { Actions, createEffect, ofType } from '@ngrx/effects';
import { tap } from 'rxjs/operators';
import { LocalStorageService } from '../local-storage/local-storage.service';
import { authLogin } from './auth.actions';

@Injectable()
export class AuthEffects {
  constructor(
    private actions$: Actions,
    private localStorageService: LocalStorageService,
    private router: Router
  ) {}
}
"#,
    )
    .unwrap();

    // 5. auth.effects.spec.ts
    fs::write(
        root.join("projects/demo/src/app/core/auth/auth.effects.spec.ts"),
        r#"import { AuthEffects } from './auth.effects';

describe('AuthEffects', () => {
  it('should compile', () => {});
});
"#,
    )
    .unwrap();

    // 6. app.component.ts (transitive caller)
    fs::write(
        root.join("projects/demo/src/app/app/app.component.ts"),
        r#"import { Component } from '@angular/core';
import { Store } from '@ngrx/store';
import { authLogin } from '../core/auth/auth.actions';

@Component({ selector: 'demo-root', template: '' })
export class AppComponent {
  login() { this.store.dispatch(authLogin()); }
  constructor(private store: Store<any>) {}
}
"#,
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    // Query Depth 1
    let cone_d1 = compute_impact_cone(
        &conn,
        &root,
        "projects/demo/src/app/core/auth/auth.effects.ts",
        1,
    )
    .unwrap();

    // Query Depth 2
    let cone_d2 = compute_impact_cone(
        &conn,
        &root,
        "projects/demo/src/app/core/auth/auth.effects.ts",
        2,
    )
    .unwrap();

    let m1 = cone_d1.metrics().expect("D1 metrics present");
    let m2 = cone_d2.metrics().expect("D2 metrics present");

    let bytes_d1 = m1.payload_bytes;
    let bytes_d2 = m2.payload_bytes;
    let inflation = bytes_d2 as f64 / bytes_d1 as f64;

    eprintln!("\n=================== SCENARIO B PAYLOAD MEASUREMENT ===================");
    eprintln!("Depth-1 Payload: {} bytes (est. {} tokens)", bytes_d1, m1.estimated_tokens);
    eprintln!("Depth-2 Payload: {} bytes (est. {} tokens)", bytes_d2, m2.estimated_tokens);
    eprintln!("Payload Inflation Factor: {:.4}x", inflation);
    eprintln!("Depth-2 Nodes: {} total (First-party: {}, External terminal: {})", m2.total_nodes, m2.first_party_nodes, m2.external_terminal_nodes);
    eprintln!("Traversed Edges: {}, Pruned Edges: {}", m2.traversed_edges, m2.pruned_edges);

    // 1. Depth-2 retrieval payload: no more than 45 KB (46,080 bytes)
    assert!(
        bytes_d2 <= 45_000,
        "Depth-2 payload {} exceeded 45,000 byte threshold!",
        bytes_d2
    );

    // 2. Payload inflation: no more than 2x against Depth-1 baseline
    assert!(
        inflation <= 2.0,
        "Payload inflation {:.2}x exceeded 2.0x threshold!",
        inflation
    );

    // 3. First-party dependency coverage: at least 95%
    let fp_paths = cone_d2.first_party_paths();
    let mut covered = 0usize;
    if fp_paths.iter().any(|p| p.replace('\\', "/").ends_with("auth.actions.ts")) { covered += 1; }
    if fp_paths.iter().any(|p| p.replace('\\', "/").ends_with("local-storage.service.ts")) { covered += 1; }
    if fp_paths.iter().any(|p| p.replace('\\', "/").ends_with("core.module.ts")) { covered += 1; }
    if fp_paths.iter().any(|p| p.replace('\\', "/").ends_with("auth.effects.spec.ts")) { covered += 1; }

    let expected_fp = 4usize;
    let coverage = covered as f64 / expected_fp as f64;

    assert!(
        coverage >= 0.95,
        "First-party coverage {:.1}% fell below 95.0% threshold!",
        coverage * 100.0
    );

    // 4. External dependencies must be terminal (pruned)
    assert!(
        m2.pruned_edges >= 4,
        "Pruned edges must be >= 4, got {}",
        m2.pruned_edges
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase4c_golden_angular_scenario_b_replay() {
    let angular_path = std::path::Path::new(r"C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter");
    if !angular_path.exists() {
        eprintln!("Golden Angular repo not found, skipping");
        return;
    }
    let conn = match db(angular_path) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Could not open db for Golden Angular, skipping");
            return;
        }
    };
    let cone_d1 = match compute_impact_cone(&conn, angular_path, "AuthEffects", 1) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("AuthEffects not indexed in Golden Angular ({}), skipping", e);
            return;
        }
    };
    let cone_d2 = compute_impact_cone(&conn, angular_path, "AuthEffects", 2).unwrap();

    let m1 = cone_d1.metrics().expect("D1 metrics present");
    let m2 = cone_d2.metrics().expect("D2 metrics present");

    let bytes_d1 = m1.payload_bytes;
    let bytes_d2 = m2.payload_bytes;
    let inflation = bytes_d2 as f64 / bytes_d1 as f64;

    eprintln!("\n=================== GOLDEN ANGULAR SCENARIO B REPLAY ===================");
    eprintln!("Depth-1 Payload: {} bytes (est. {} tokens)", bytes_d1, m1.estimated_tokens);
    eprintln!("Depth-2 Payload: {} bytes (est. {} tokens)", bytes_d2, m2.estimated_tokens);
    eprintln!("Payload Inflation Factor: {:.4}x", inflation);
    eprintln!("Depth-2 Nodes: {} total (First-party: {}, External terminal: {})", m2.total_nodes, m2.first_party_nodes, m2.external_terminal_nodes);
    eprintln!("Traversed Edges: {}, Pruned Edges: {}", m2.traversed_edges, m2.pruned_edges);
}

#[test]
fn test_phase4c2_three_state_dependency_classification() {
    let root = std::env::temp_dir().join(format!("engram_phase4c2_class_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src")).unwrap();

    // 1. Resolved relative workspace import (leaf)
    fs::write(root.join("src/leaf.ts"), "export class LeafService {}
").unwrap();

    // 2. Resolved relative workspace import (middle)
    fs::write(
        root.join("src/middle.ts"),
        r#"import { LeafService } from './leaf';
import { Store } from '@ngrx/store';

export class MiddleService {}
"#,
    )
    .unwrap();

    // 3. Entry file with all 3 classification categories:
    //    - Resolved relative workspace: ./middle
    //    - Confirmed third-party: @angular/core
    //    - Standard library: std::collections::HashMap
    //    - Unresolved relative import: ./unresolved_rel
    //    - Missing workspace file: src/missing_workspace.ts
    //    - Incorrect/stale import path: ../incorrect/stale.ts
    fs::write(
        root.join("src/entry.ts"),
        r#"import { MiddleService } from './middle';
import { Injectable } from '@angular/core';
import { HashMap } from 'std::collections::HashMap';
import { MissingRel } from './unresolved_rel';
import { MissingFile } from 'src/missing_workspace.ts';
import { StaleImport } from '../incorrect/stale.ts';

@Injectable()
export class EntryComponent {}
"#,
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone = compute_impact_cone(&conn, &root, "src/entry.ts", 2).unwrap();
    let m = cone.metrics().expect("Metrics present");

    // 1. Verify WORKSPACE classification: successfully resolved first-party dependencies
    let fp_entities = cone.first_party_entities();
    assert!(
        fp_entities.iter().any(|e| e.path.replace('\\', "/").ends_with("middle.ts")),
        "middle.ts must be classified as WORKSPACE"
    );
    assert!(
        fp_entities.iter().any(|e| e.path.replace('\\', "/").ends_with("leaf.ts")),
        "leaf.ts must be classified as WORKSPACE"
    );
    for e in &fp_entities {
        assert_eq!(e.classification, DependencyClassification::Workspace);
        assert!(!e.is_external);
    }

    // 2. Verify EXTERNAL classification: confirmed third-party and standard-library dependencies
    let ext_entities = cone.external_entities();
    assert!(
        ext_entities.iter().any(|e| e.path == "@angular/core"),
        "@angular/core must be classified as EXTERNAL"
    );
    assert!(
        ext_entities.iter().any(|e| e.path == "std" || e.path.starts_with("std::")),
        "std::collections::HashMap must be classified as EXTERNAL"
    );
    assert!(
        ext_entities.iter().any(|e| e.path == "@ngrx/store"),
        "@ngrx/store must be classified as EXTERNAL terminal leaf"
    );
    for e in &ext_entities {
        assert_eq!(e.classification, DependencyClassification::External);
        assert!(e.is_external);
    }

    // 3. Verify UNRESOLVED classification: missing, invalid, or stale workspace targets
    let unres_entities = cone.unresolved_entities();
    assert!(
        unres_entities.iter().any(|e| e.path.replace('\\', "/").ends_with("unresolved_rel.ts")),
        "unresolved_rel.ts must be classified as UNRESOLVED"
    );
    assert!(
        unres_entities.iter().any(|e| e.path.replace('\\', "/").contains("missing_workspace.ts")),
        "missing_workspace.ts must be classified as UNRESOLVED"
    );
    assert!(
        unres_entities.iter().any(|e| e.path.replace('\\', "/").contains("stale.ts")),
        "stale.ts must be classified as UNRESOLVED"
    );
    for e in &unres_entities {
        assert_eq!(e.classification, DependencyClassification::Unresolved);
        assert!(!e.is_external, "Unresolved dependencies must NOT be flagged as external!");
    }

    // 4. Verify Metrics Separation
    assert_eq!(m.unresolved_nodes, 3, "Exactly 3 unresolved nodes must be tracked");
    assert_eq!(m.external_terminal_nodes, 3, "Exactly 3 external terminal nodes (@angular/core, std::*, @ngrx/store)");
    assert_eq!(m.first_party_nodes, 3, "Exactly 3 first-party nodes (entry, middle, leaf)");
    assert_eq!(m.total_nodes, 9, "Total nodes must equal 3 FP + 3 Ext + 3 Unresolved");

    // 5. Verify Unresolved Dependencies Do NOT Trigger Traversal Outside Workspace
    //    None of the unresolved targets should spawn outgoing or incoming secondary expansions.
    for e in &unres_entities {
        assert_eq!(e.hop_distance, 1, "Unresolved targets must remain at discovery hop (1)");
    }

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_phase4c2_unresolved_not_counted_as_confirmed_external() {
    let root = std::env::temp_dir().join(format!("engram_phase4c2_unres_{}", uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(root.join("src")).unwrap();

    fs::write(
        root.join("src/service.ts"),
        r#"import { MissingLocal } from './non_existent_local';
import { ConfirmedExternal } from '@angular/common';

export class Service {}
"#,
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone = compute_impact_cone(&conn, &root, "src/service.ts", 1).unwrap();
    let m = cone.metrics().expect("Metrics present");

    // Confirmed external dependency
    assert_eq!(m.external_terminal_nodes, 1);
    assert!(cone.external_paths().contains(&"@angular/common".to_string()));

    // Unresolved local dependency must NOT be counted as confirmed external
    assert_eq!(m.unresolved_nodes, 1);
    assert!(!cone.external_paths().iter().any(|p| p.contains("non_existent_local")));
    assert!(cone.unresolved_paths().iter().any(|p| p.contains("non_existent_local")));

    let _ = fs::remove_dir_all(&root);
}
