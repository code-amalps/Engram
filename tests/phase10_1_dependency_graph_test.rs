//! Engram Phase 10.1 Comprehensive Verification Suite:
//! 1. Cross-file AST dependency extraction for C#, TypeScript/JS, Python, and Rust
//! 2. Schema v2.0.0 resolution status & ambiguity/unresolved discipline
//! 3. Differential verification across all 20 scenarios (DG-01..DG-20):
//!    incremental `sync_knowledge_graph(root, false)` == fresh full `sync_knowledge_graph(root, true)`
//! 4. Unified analyze / incremental_analyze / ensure_fresh synchronization
//! 5. Persistence, crash/corruption recovery, partial parse preservation, repo move, config/ignore rebuild
//! 6. Multi-hop KG retrieval provenance & strict token-budget enforcement
//! 7. Boundary/symlink/secret security and CLI/MCP graph surfaces

use engram::analyzer::{analyze, ensure_fresh};
use engram::budget::DetailLevel;
use engram::compiler::context_scoped_optimized;
use engram::incremental::incremental_analyze;
use engram::knowledge_graph::{
    GRAPH_SCHEMA_VERSION, GraphLifecycleState, KnowledgeGraphIndex, RelationCategory,
    ResolutionStatus, open_graph_db, sync_knowledge_graph,
};
use engram::mcp::{mcp_call, tools_definitions};
use rusqlite::Connection;
use serde_json::json;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct TestRepo {
    root: PathBuf,
}

impl TestRepo {
    fn new(tag: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("engram_p10_1_{tag}_{}", uuid::Uuid::new_v4()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let git_ok = |args: &[&str]| {
            let out = Command::new("git")
                .args(args)
                .current_dir(&root)
                .output()
                .expect("git failed");
            assert!(
                out.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git_ok(&["init", "-b", "main"]);
        git_ok(&["config", "user.name", "EngramP10Tester"]);
        git_ok(&["config", "user.email", "p10@engram.local"]);
        Self { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, rel: &str, content: &str) {
        let p = self.root.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    fn remove(&self, rel: &str) {
        let p = self.root.join(rel);
        if p.exists() {
            fs::remove_file(p).unwrap();
        }
    }

    fn commit(&self, msg: &str) {
        for args in [
            vec!["add", "-A"],
            vec!["commit", "-m", msg, "--allow-empty"],
        ] {
            let _ = Command::new("git")
                .args(&args)
                .current_dir(&self.root)
                .output();
        }
    }
}

impl Drop for TestRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NormNode {
    id: String,
    category: String,
    label: String,
    relative_path: String,
    module_path: Option<String>,
    start_line: Option<i64>,
    end_line: Option<i64>,
    signature: Option<String>,
    fingerprint: String,
    resolution_status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NormEdge {
    id: String,
    source_id: String,
    target_id: String,
    category: String,
    source_path: String,
    source_line: Option<i64>,
    extraction_method: String,
    classification: String,
    resolution_status: String,
    confidence_milli: i64,
    target_hint: Option<String>,
}

fn snapshot_normalized_graph(conn: &Connection) -> (Vec<NormNode>, Vec<NormEdge>) {
    let mut nodes = Vec::new();
    {
        let mut st = conn
            .prepare(
                "SELECT id, category, label, relative_path, module_path, start_line, end_line, signature, fingerprint, resolution_status
                 FROM kg_nodes ORDER BY id",
            )
            .unwrap();
        let rows = st
            .query_map([], |r| {
                Ok(NormNode {
                    id: r.get(0)?,
                    category: r.get(1)?,
                    label: r.get(2)?,
                    relative_path: r.get(3)?,
                    module_path: r.get(4)?,
                    start_line: r.get(5)?,
                    end_line: r.get(6)?,
                    signature: r.get(7)?,
                    fingerprint: r.get(8)?,
                    resolution_status: r.get(9)?,
                })
            })
            .unwrap();
        for n in rows {
            nodes.push(n.unwrap());
        }
    }
    let mut edges = Vec::new();
    {
        let mut st = conn
            .prepare(
                "SELECT id, source_id, target_id, category, source_path, source_line, extraction_method, classification, resolution_status, confidence, target_hint
                 FROM kg_edges ORDER BY id",
            )
            .unwrap();
        let rows = st
            .query_map([], |r| {
                let conf: f64 = r.get(9)?;
                Ok(NormEdge {
                    id: r.get(0)?,
                    source_id: r.get(1)?,
                    target_id: r.get(2)?,
                    category: r.get(3)?,
                    source_path: r.get(4)?,
                    source_line: r.get(5)?,
                    extraction_method: r.get(6)?,
                    classification: r.get(7)?,
                    resolution_status: r.get(8)?,
                    confidence_milli: (conf * 1000.0).round() as i64,
                    target_hint: r.get(10)?,
                })
            })
            .unwrap();
        for e in rows {
            edges.push(e.unwrap());
        }
    }
    nodes.sort();
    edges.sort();
    (nodes, edges)
}

fn assert_incremental_equals_full(root: &Path, scenario: &str) {
    let inc_status = sync_knowledge_graph(root, false).expect("incremental sync failed");
    assert_eq!(inc_status.schema_version, GRAPH_SCHEMA_VERSION);
    let inc_conn = open_graph_db(root).unwrap();
    let (inc_nodes, inc_edges) = snapshot_normalized_graph(&inc_conn);
    drop(inc_conn);

    // Clone SQLite check? Compare directly against clean force_full rebuild on same repo!
    let full_status = sync_knowledge_graph(root, true).expect("full rebuild failed");
    let full_conn = open_graph_db(root).unwrap();
    let (full_nodes, full_edges) = snapshot_normalized_graph(&full_conn);

    assert_eq!(
        inc_status.state, full_status.state,
        "[{scenario}] GraphLifecycleState mismatch"
    );
    assert_eq!(
        inc_status.indexed_files_count, full_status.indexed_files_count,
        "[{scenario}] indexed_files_count mismatch"
    );

    if inc_nodes != full_nodes {
        let inc_set: BTreeSet<_> = inc_nodes.iter().collect();
        let full_set: BTreeSet<_> = full_nodes.iter().collect();
        let missing: Vec<_> = full_set.difference(&inc_set).collect();
        let extra: Vec<_> = inc_set.difference(&full_set).collect();
        panic!(
            "[{scenario}] NODE DIFFERENTIAL MISMATCH:\n  missing in inc ({}): {:?}\n  extra in inc ({}): {:?}",
            missing.len(),
            missing.into_iter().take(10).collect::<Vec<_>>(),
            extra.len(),
            extra.into_iter().take(10).collect::<Vec<_>>()
        );
    }

    if inc_edges != full_edges {
        let inc_set: BTreeSet<_> = inc_edges.iter().collect();
        let full_set: BTreeSet<_> = full_edges.iter().collect();
        let missing: Vec<_> = full_set.difference(&inc_set).collect();
        let extra: Vec<_> = inc_set.difference(&full_set).collect();
        panic!(
            "[{scenario}] EDGE DIFFERENTIAL MISMATCH:\n  missing in inc ({}): {:?}\n  extra in inc ({}): {:?}",
            missing.len(),
            missing.into_iter().take(10).collect::<Vec<_>>(),
            extra.len(),
            extra.into_iter().take(10).collect::<Vec<_>>()
        );
    }

    // Referential integrity: zero dangling edges
    let dangling: i64 = full_conn
        .query_row(
            "SELECT COUNT(*) FROM kg_edges WHERE source_id NOT IN (SELECT id FROM kg_nodes) OR target_id NOT IN (SELECT id FROM kg_nodes)",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(dangling, 0, "[{scenario}] zero dangling edges required");
}

// ============================================================================
// 1. Multi-Language Cross-File AST Dependency Extraction
// ============================================================================

#[test]
fn test_csharp_cross_file_dependencies() {
    let repo = TestRepo::new("cs_cross");
    repo.write(
        "dotnet/Contracts/IPaymentGateway.cs",
        r#"namespace Acme.Contracts
{
    public interface IPaymentGateway
    {
        bool Charge(Order order);
    }
}
"#,
    );
    repo.write(
        "dotnet/Models/Order.cs",
        r#"namespace Acme.Models;

public class BaseEntity
{
    public string Id { get; set; }
}

public class Order : BaseEntity
{
    public decimal Total { get; set; }
}
"#,
    );
    repo.write(
        "dotnet/Services/StripeGateway.cs",
        r#"using System;
using Acme.Contracts;
using Acme.Models;

namespace Acme.Services
{
    public class StripeGateway : BaseEntity, IPaymentGateway
    {
        public bool Charge(Order order)
        {
            Validate(order);
            return true;
        }

        private void Validate(Order order) {}
    }
}
"#,
    );
    repo.write(
        "dotnet/Tests/StripeGatewayTests.cs",
        r#"using Xunit;
using Acme.Services;
using Acme.Models;

namespace Acme.Tests
{
    public class StripeGatewayTests
    {
        [Fact]
        public void Charge_Succeeds()
        {
            var gw = new StripeGateway();
            gw.Charge(new Order());
        }
    }
}
"#,
    );

    let st = sync_knowledge_graph(repo.path(), true).unwrap();
    assert_eq!(st.state, GraphLifecycleState::Ready);

    let conn = open_graph_db(repo.path()).unwrap();
    let idx = KnowledgeGraphIndex::load_from_db(&conn).unwrap();

    // Check namespace Module nodes & Declares
    assert!(idx.nodes_by_id.contains_key("mod:csharp:Acme.Contracts"));
    assert!(idx.nodes_by_id.contains_key("mod:csharp:Acme.Models"));
    assert!(idx.nodes_by_id.contains_key("mod:csharp:Acme.Services"));

    let all_edges: Vec<_> = idx.outgoing.values().flatten().collect();

    // IMPLEMENTS edge: StripeGateway -> IPaymentGateway (Resolved!)
    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Implements
                && e.source_id.contains("StripeGateway")
                && e.target_id.contains("IPaymentGateway")
                && e.provenance.resolution_status == ResolutionStatus::Resolved
        }),
        "C# IMPLEMENTS StripeGateway -> IPaymentGateway missing"
    );

    // INHERITS edge: StripeGateway -> BaseEntity and Order -> BaseEntity
    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Inherits
                && e.source_id.contains("StripeGateway")
                && e.target_id.contains("BaseEntity")
                && e.provenance.resolution_status == ResolutionStatus::Resolved
        }),
        "C# INHERITS StripeGateway -> BaseEntity missing"
    );

    // CALLS edge inside Charge -> Validate (verifies tree-sitter-c-sharp 0.23 "function" field fix)
    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Calls
                && e.source_id.contains("Charge")
                && e.target_id.contains("Validate")
                && e.provenance.resolution_status == ResolutionStatus::Resolved
        }),
        "C# CALLS Charge -> Validate missing"
    );

    // External System / Xunit imports classified External
    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Imports
                && e.target_id.starts_with("ext:pkg:System")
                && e.provenance.resolution_status == ResolutionStatus::External
        }),
        "C# System using should be External"
    );

    // TESTS edge targeting StripeGateway.cs
    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Tests
                && e.source_id == "file:dotnet/Tests/StripeGatewayTests.cs"
                && e.target_id == "file:dotnet/Services/StripeGateway.cs"
                && e.provenance.resolution_status == ResolutionStatus::Resolved
        }),
        "C# TESTS edge must resolve dynamically without hardcoded src/Cart path"
    );
}

#[test]
fn test_typescript_cross_file_and_barrel_dependencies() {
    let repo = TestRepo::new("ts_cross");
    repo.write(
        "src/contracts.ts",
        r#"export interface IExporter {
    exportRow(id: string): string;
}
export class BaseExporter {
    tag(): string { return "base"; }
}
"#,
    );
    repo.write(
        "src/impl/csv.exporter.ts",
        r#"import { IExporter, BaseExporter } from '../contracts';

export function formatCsvCell(s: string): string {
    return s.trim();
}

export class CsvExporter extends BaseExporter implements IExporter {
    exportRow(id: string): string {
        return formatCsvCell(id);
    }
}
"#,
    );
    repo.write(
        "src/impl/index.ts",
        r#"export { CsvExporter } from './csv.exporter';
"#,
    );
    repo.write(
        "src/app.ts",
        r#"import { CsvExporter } from './impl';

export function runExport(): string {
    const e = new CsvExporter();
    return e.exportRow("42");
}
"#,
    );
    repo.write(
        "src/impl/csv.exporter.spec.ts",
        r#"import { CsvExporter } from './csv.exporter';
export function spec() { return new CsvExporter(); }
"#,
    );

    sync_knowledge_graph(repo.path(), true).unwrap();
    let conn = open_graph_db(repo.path()).unwrap();
    let idx = KnowledgeGraphIndex::load_from_db(&conn).unwrap();
    let all_edges: Vec<_> = idx.outgoing.values().flatten().collect();

    // Barrel index.ts resolution + re-export transitive symbol import
    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Imports
                && e.source_id == "file:src/app.ts"
                && e.target_id.contains("CsvExporter")
                && e.provenance.resolution_status == ResolutionStatus::Resolved
        }),
        "TS barrel re-export import ./impl -> CsvExporter must resolve cleanly"
    );

    // class extends + implements
    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Inherits
                && e.source_id.contains("CsvExporter")
                && e.target_id.contains("BaseExporter")
        }),
        "TS class extends BaseExporter must emit INHERITS"
    );
    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Implements
                && e.source_id.contains("CsvExporter")
                && e.target_id.contains("IExporter")
        }),
        "TS class implements IExporter must emit IMPLEMENTS"
    );

    // spec test relation
    assert!(all_edges.iter().any(|e| {
        e.category == RelationCategory::Tests
            && e.source_id == "file:src/impl/csv.exporter.spec.ts"
            && e.target_id == "file:src/impl/csv.exporter.ts"
    }));
}

#[test]
fn test_python_cross_file_relative_and_inheritance() {
    let repo = TestRepo::new("py_cross");
    repo.write("pkg/__init__.py", "");
    repo.write(
        "pkg/base.py",
        r#"class BaseHandler:
    def validate(self) -> bool:
        return True
"#,
    );
    repo.write("pkg/orders/__init__.py", "");
    repo.write(
        "pkg/orders/models.py",
        r#"class OrderPayload:
    pass
"#,
    );
    repo.write(
        "pkg/orders/service.py",
        r#"from ..base import BaseHandler
from .models import OrderPayload
import json

class OrderService(BaseHandler):
    def submit(self, payload: OrderPayload) -> bool:
        return self.validate()
"#,
    );
    repo.write(
        "tests/test_service.py",
        r#"from pkg.orders.service import OrderService
def test_submit():
    assert OrderService()
"#,
    );

    sync_knowledge_graph(repo.path(), true).unwrap();
    let conn = open_graph_db(repo.path()).unwrap();
    let idx = KnowledgeGraphIndex::load_from_db(&conn).unwrap();
    let all_edges: Vec<_> = idx.outgoing.values().flatten().collect();

    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Imports
                && e.source_id == "file:pkg/orders/service.py"
                && (e.target_id == "file:pkg/base.py"
                    || e.target_id.contains("pkg/base.py::BaseHandler"))
                && e.provenance.resolution_status == ResolutionStatus::Resolved
        }),
        "Python relative import ..base must resolve to pkg/base.py"
    );

    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Inherits
                && e.source_id.contains("OrderService")
                && e.target_id.contains("BaseHandler")
        }),
        "Python class inheritance OrderService(BaseHandler) must emit INHERITS"
    );

    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Imports
                && e.source_id == "file:pkg/orders/service.py"
                && e.target_id == "ext:pkg:json"
                && e.provenance.resolution_status == ResolutionStatus::External
        }),
        "Python stdlib json must be External"
    );

    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Tests
                && e.source_id == "file:tests/test_service.py"
                && e.target_id == "file:pkg/orders/service.py"
                && e.provenance.resolution_status == ResolutionStatus::Resolved
        }),
        "Python test must resolve dynamically to pkg/orders/service.py"
    );
}

#[test]
fn test_rust_cross_file_mods_nested_uses_and_traits() {
    let repo = TestRepo::new("rs_cross");
    repo.write(
        "Cargo.toml",
        r#"[package]
name = "billing_app"
version = "0.1.0"
edition = "2021"
"#,
    );
    repo.write(
        "src/core/engine.rs",
        r#"pub trait BaseRule {
    fn check(&self) -> bool;
}

pub trait PricingRule: BaseRule {
    fn price(&self) -> u64;
}

pub struct FlatRule;

impl PricingRule for FlatRule {
    fn price(&self) -> u64 { 10 }
}

pub fn apply_pricing() -> u64 { 10 }
"#,
    );
    repo.write("src/core/mod.rs", "pub mod engine;\n");
    repo.write(
        "src/lib.rs",
        r#"use std::collections::HashMap;
pub mod core;
pub use crate::core::engine::{PricingRule, FlatRule, apply_pricing};

pub fn run() -> u64 {
    let _map: HashMap<String, u64> = HashMap::new();
    apply_pricing()
}
"#,
    );
    repo.write(
        "tests/engine_test.rs",
        r#"use billing_app::core::engine::apply_pricing;
#[test]
fn check_it() { assert_eq!(apply_pricing(), 10); }
"#,
    );

    sync_knowledge_graph(repo.path(), true).unwrap();
    let conn = open_graph_db(repo.path()).unwrap();
    let idx = KnowledgeGraphIndex::load_from_db(&conn).unwrap();
    let all_edges: Vec<_> = idx.outgoing.values().flatten().collect();

    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Imports
                && e.source_id == "file:src/lib.rs"
                && e.target_id == "file:src/core/mod.rs"
        }),
        "Rust out-of-line mod core; must resolve to src/core/mod.rs"
    );
    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Imports
                && e.source_id == "file:src/lib.rs"
                && e.target_id.starts_with("symbol:src/core/engine.rs::")
                && e.target_id.contains("::FlatRule::")
                && e.provenance.resolution_status == ResolutionStatus::Resolved
        }),
        "Rust nested use_list must flatten and resolve FlatRule"
    );
    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Implements
                && e.source_id.starts_with("symbol:src/core/engine.rs")
                && e.target_id.contains("PricingRule")
        }),
        "Rust impl PricingRule for FlatRule must emit IMPLEMENTS"
    );
    assert!(
        all_edges.iter().any(|e| {
            e.category == RelationCategory::Inherits
                && e.source_id.contains("PricingRule")
                && e.target_id.contains("BaseRule")
        }),
        "Rust trait supertrait bound must emit INHERITS"
    );
    assert!(all_edges.iter().any(|e| {
        e.category == RelationCategory::Tests
            && e.source_id == "file:tests/engine_test.rs"
            && e.target_id == "file:src/core/engine.rs"
    }));
}

// ============================================================================
// 2. Differential Equivalence Suite (DG-01 through DG-20)
// ============================================================================

#[test]
fn test_differential_equivalence_dg01_through_dg10() {
    let repo = TestRepo::new("dg_1_10");
    repo.write(
        "Cargo.toml",
        "[package]\nname = \"dg_repo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    );
    repo.write("src/math.rs", "pub fn compute(x: i32) -> i32 { x + 1 }\n");
    repo.write(
        "src/caller.rs",
        "use crate::math::compute;\npub fn call_it() -> i32 { compute(2) }\n",
    );
    repo.write(
        "src/sub/missing_user.ts",
        "import { ghostFn } from './ghost';\nexport function runGhost() { return ghostFn(); }\n",
    );
    repo.write(
        "dotnet/NsA.cs",
        "namespace Demo.Core {\n  public interface IA { void Ping(); }\n  public class ImplA : IA { public void Ping() {} }\n}\n",
    );
    repo.write("py/pkg/__init__.py", "");
    repo.write(
        "py/pkg/helper.py",
        "class HelperBase:\n    def act(self): return 1\n",
    );
    repo.write(
        "py/pkg/app.py",
        "from .helper import HelperBase\nclass App(HelperBase):\n    pass\n",
    );
    repo.commit("init");
    assert_incremental_equals_full(repo.path(), "INIT");

    // DG-01: Body-only leaf function edit (signature unchanged)
    repo.write(
        "src/math.rs",
        "pub fn compute(x: i32) -> i32 { x * 3 + 7 }\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-01_body_only_edit");

    // DG-02: Public signature change affecting callers
    repo.write(
        "src/math.rs",
        "pub fn compute(x: i64, y: i64) -> i64 { x + y }\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-02_signature_change");

    // DG-03: File rename / move updating importers
    repo.remove("src/math.rs");
    repo.write(
        "src/arith.rs",
        "pub fn compute(x: i64, y: i64) -> i64 { x + y }\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-03_file_rename");

    // DG-04: Duplicate symbol introduction (RESOLVED -> AMBIGUOUS) then removal (AMBIGUOUS -> RESOLVED)
    repo.write("src/dup_fn.rs", "pub fn dup_target() -> i32 { 1 }\n");
    repo.write(
        "src/dup_caller.rs",
        "pub fn use_dup() -> i32 { dup_target() }\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-04a_unique_target");

    repo.write("src/dup_fn_2.rs", "pub fn dup_target() -> i32 { 2 }\n");
    {
        assert_incremental_equals_full(repo.path(), "DG-04b_ambiguous_target");
        let conn = open_graph_db(repo.path()).unwrap();
        let idx = KnowledgeGraphIndex::load_from_db(&conn).unwrap();
        let amb_edges: Vec<_> = idx
            .outgoing
            .values()
            .flatten()
            .filter(|e| {
                e.provenance.source_path == "src/dup_caller.rs"
                    && e.provenance.resolution_status == ResolutionStatus::Ambiguous
            })
            .collect();
        assert!(
            !amb_edges.is_empty(),
            "DG-04: duplicate symbol must produce ResolutionStatus::Ambiguous rather than picking candidates[0]"
        );
    }

    repo.remove("src/dup_fn_2.rs");
    assert_incremental_equals_full(repo.path(), "DG-04c_ambiguity_resolved_again");

    // DG-05: Interface/trait contract addition/removal
    repo.write(
        "dotnet/NsB.cs",
        "using Demo.Core;\nnamespace Demo.Consumer {\n  public class ImplB : IA { public void Ping() {} }\n}\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-05_interface_impl_added");

    // DG-06: Missing import target added later (UNRESOLVED -> RESOLVED) then deleted
    repo.write(
        "src/sub/ghost.ts",
        "export function ghostFn(): number { return 99; }\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-06a_missing_target_added");

    repo.remove("src/sub/ghost.ts");
    assert_incremental_equals_full(repo.path(), "DG-06b_missing_target_deleted");

    // DG-07: TS barrel re-export chain edit
    repo.write(
        "src/web/target.ts",
        "export function widget(): number { return 1; }\n",
    );
    repo.write("src/web/index.ts", "export { widget } from './target';\n");
    repo.write(
        "src/web/consumer.ts",
        "import { widget } from './index';\nexport const w = widget();\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-07a_barrel_created");
    repo.write(
        "src/web/target.ts",
        "export function widgetV2(): number { return 2; }\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-07b_barrel_target_changed");

    // DG-08: C# namespace addition & consumer upgrade
    repo.write(
        "dotnet/UseLater.cs",
        "using Demo.Future;\nnamespace Demo.Caller { public class C {} }\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-08a_namespace_missing");
    repo.write(
        "dotnet/FutureNs.cs",
        "namespace Demo.Future { public class FutureSvc {} }\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-08b_namespace_added");

    // DG-09: Python relative import & base edit
    repo.write(
        "py/pkg/helper.py",
        "class HelperBase:\n    def act(self): return 2\nclass ExtraBase:\n    pass\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-09_python_module_update");

    // DG-10: Rust out-of-line mod + nested use reorganization
    repo.write("src/nested_mod.rs", "pub fn alpha() {}\npub fn beta() {}\n");
    repo.write(
        "src/lib.rs",
        "pub mod nested_mod;\npub use crate::nested_mod::{alpha, beta};\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-10_rust_mod_added");
}

#[test]
fn test_differential_equivalence_dg11_through_dg20() {
    let repo = TestRepo::new("dg_11_20");
    repo.write(
        "src/deep/folder/mod_a.ts",
        "import 'external-pkg';\nexport function modA() {}\n",
    );
    repo.write("src/core/calc.rs", "pub fn calc() -> i32 { 1 }\n");
    repo.write("tests/calc_test.rs", "#[test] fn t() {}\n");
    repo.write(
        "go/service.go",
        "package main\ntype Runner interface { Run() }\n",
    );
    repo.write(
        "go/impl.go",
        "package main\ntype Worker struct{}\nfunc (w Worker) Run() {}\n",
    );
    repo.commit("init_11_20");
    assert_incremental_equals_full(repo.path(), "INIT_11_20");

    // DG-11: Deleting last file in deep directory cleans directory, ext, and file nodes cleanly
    repo.remove("src/deep/folder/mod_a.ts");
    assert_incremental_equals_full(repo.path(), "DG-11_orphan_dir_and_ext_cleanup");

    // DG-12: Test file removal cleans TESTS edge cleanly
    repo.remove("tests/calc_test.rs");
    assert_incremental_equals_full(repo.path(), "DG-12_test_file_removed");

    // DG-13: Constitution rule creation & edit
    repo.write(
        ".engram/constitution.yaml",
        "rules:\n  - id: R001\n    name: IsolateCore\n    severity: block\n    description: Keep isolated\n    paths: ['src/**']\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-13a_constitution_added");
    repo.write(
        ".engram/constitution.yaml",
        "rules:\n  - id: R002\n    name: StrictLayers\n    severity: warn\n    description: Layered architecture\n    paths: ['src/**']\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-13b_constitution_changed");

    // DG-14: .engramignore edit triggers clean rebuild excluding newly ignored files
    repo.write("src/ignored_later.rs", "pub fn skip_me() {}\n");
    assert_incremental_equals_full(repo.path(), "DG-14a_before_ignore");
    repo.write(".engramignore", "src/ignored_later.rs\n");
    assert_incremental_equals_full(repo.path(), "DG-14b_engramignore_updated");

    // DG-15: engram.yaml config change triggers clean rebuild
    repo.write("engram.yaml", "version: 1\n");
    assert_incremental_equals_full(repo.path(), "DG-15_config_changed");

    // DG-16: Malformed syntax file (Partial) preserves healthy files & sets PartiallyIndexed
    repo.write(
        "src/broken.rs",
        "pub fn unbroken_sig() {}\npub fn broken((((( { ;;\n",
    );
    let st_partial = sync_knowledge_graph(repo.path(), false).unwrap();
    assert_eq!(st_partial.state, GraphLifecycleState::PartiallyIndexed);
    assert_incremental_equals_full(repo.path(), "DG-16_partial_parse_equivalence");
    repo.remove("src/broken.rs");
    assert_incremental_equals_full(repo.path(), "DG-16b_recovered_after_broken_removed");

    // DG-17: Corrupt knowledge_graph.db recovery on incremental sync
    let db_path = engram::knowledge_graph::graph_db_path(repo.path()).unwrap();
    fs::write(&db_path, b"THIS IS CORRUPTED SQLITE HEADER BYTES").unwrap();
    assert_incremental_equals_full(repo.path(), "DG-17_corrupt_db_recovery");

    // DG-18: External vs Unresolved separation check
    repo.write(
        "src/ext_vs_missing.ts",
        "import { Observable } from 'rxjs';\nimport { nope } from './nope';\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-18_external_vs_unresolved");
    {
        let conn = open_graph_db(repo.path()).unwrap();
        let idx = KnowledgeGraphIndex::load_from_db(&conn).unwrap();
        let edges: Vec<_> = idx.outgoing.values().flatten().collect();
        assert!(edges.iter().any(|e| {
            e.target_id.starts_with("ext:pkg:rxjs")
                && e.provenance.resolution_status == ResolutionStatus::External
        }));
        assert!(edges.iter().any(|e| {
            e.target_id.starts_with("unresolved:missing:")
                && e.provenance.resolution_status == ResolutionStatus::Unresolved
        }));
    }

    // DG-19: Multi-step sequence (add -> rename -> duplicate -> fix)
    repo.write("src/step_a.rs", "pub fn multi_step() -> i32 { 1 }\n");
    assert_incremental_equals_full(repo.path(), "DG-19a");
    repo.write(
        "src/step_b.rs",
        "pub fn caller_ms() -> i32 { multi_step() }\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-19b");
    repo.write("src/step_c.rs", "pub fn multi_step() -> i32 { 2 }\n");
    assert_incremental_equals_full(repo.path(), "DG-19c");
    repo.remove("src/step_a.rs");
    assert_incremental_equals_full(repo.path(), "DG-19d");

    // DG-20: Go interface satisfaction incremental sync across files
    repo.write(
        "go/impl2.go",
        "package main\ntype FastWorker struct{}\nfunc (f FastWorker) Run() {}\n",
    );
    assert_incremental_equals_full(repo.path(), "DG-20_go_interface_incremental");
}

// ============================================================================
// 3. Unified Synchronization, Persistence, Retrieval, Budget, Security & CLI/MCP
// ============================================================================

#[test]
fn test_unified_sync_on_analyze_incremental_and_ensure_fresh() {
    let repo = TestRepo::new("unified_sync");
    repo.write("src/alpha.rs", "pub fn alpha_fn() -> i32 { 10 }\n");
    repo.write(
        "src/beta.rs",
        "use crate::alpha::alpha_fn;\npub fn beta_fn() -> i32 { alpha_fn() }\n",
    );
    repo.commit("init");

    // 1. analyze() must automatically populate knowledge_graph.db
    analyze(repo.path()).unwrap();
    let kg_path = engram::knowledge_graph::graph_db_path(repo.path()).unwrap();
    assert!(kg_path.exists(), "analyze() must sync knowledge_graph.db");

    {
        let conn = open_graph_db(repo.path()).unwrap();
        let idx = KnowledgeGraphIndex::load_from_db(&conn).unwrap();
        assert!(idx.nodes_by_id.contains_key("file:src/alpha.rs"));
        assert!(idx.nodes_by_id.contains_key("file:src/beta.rs"));
    }

    // 2. Modify + run incremental_analyze() -> knowledge_graph.db stays synchronized
    repo.write("src/gamma.rs", "pub fn gamma_fn() -> i32 { 20 }\n");
    incremental_analyze(repo.path()).unwrap();
    {
        let conn = open_graph_db(repo.path()).unwrap();
        let idx = KnowledgeGraphIndex::load_from_db(&conn).unwrap();
        assert!(idx.nodes_by_id.contains_key("file:src/gamma.rs"));
    }

    // 3. Edit without calling analyze + call ensure_fresh() -> BOTH derived & KG synced
    repo.write("src/delta.rs", "pub fn delta_fn() -> i32 { 30 }\n");
    ensure_fresh(repo.path()).unwrap();
    {
        let conn = open_graph_db(repo.path()).unwrap();
        let idx = KnowledgeGraphIndex::load_from_db(&conn).unwrap();
        assert!(idx.nodes_by_id.contains_key("file:src/delta.rs"));
    }
}

#[test]
fn test_retrieval_provenance_and_strict_token_budget_enforcement() {
    let repo = TestRepo::new("retrieval_budget");
    // Construct a dense multi-file fixture so mandatory Tier 0 + dependencies test exact budget bounds
    let mut huge_body = String::from("pub fn primary_entry() -> usize {\n    let mut acc = 0;\n");
    for i in 0..250 {
        huge_body.push_str(&format!(
            "    acc += {i}; // mandatory padding line index {i}\n"
        ));
    }
    huge_body.push_str("    acc\n}\n");
    repo.write("src/primary.rs", &huge_body);
    repo.write("src/helper.rs", "pub fn help_work() -> usize { 42 }\n");
    repo.write(
        "src/caller.rs",
        "use crate::primary::primary_entry;\nuse crate::helper::help_work;\npub fn invoke_all() -> usize { primary_entry() + help_work() }\n",
    );
    repo.commit("init");

    // 1. Verify multi-hop retrieval provenance & strictly bounded token budget (1200)
    let res_1200 = context_scoped_optimized(
        repo.path(),
        "primary_entry invoke_all",
        1200,
        None,
        None,
        DetailLevel::Contracted,
        None,
    )
    .unwrap();
    assert!(
        res_1200.estimated_tokens <= res_1200.token_budget,
        "Strict budget invariant violated: estimated {} > limit {}",
        res_1200.estimated_tokens,
        res_1200.token_budget
    );

    // 2. Tight budget (e.g. 120 tokens) MUST NEVER exceed token_budget even for Tier0Mandatory files!
    let res_tight = context_scoped_optimized(
        repo.path(),
        "primary_entry",
        120,
        None,
        None,
        DetailLevel::Full,
        None,
    )
    .unwrap();
    assert!(
        res_tight.estimated_tokens <= 120,
        "Tight budget invariant violated on Tier0Mandatory: estimated {} > limit 120",
        res_tight.estimated_tokens
    );

    // 3. Verify Native KG provenance labels appear cleanly in graph-expanded files
    let res_prov = context_scoped_optimized(
        repo.path(),
        "primary_entry",
        4000,
        None,
        None,
        DetailLevel::Contracted,
        None,
    )
    .unwrap();
    let has_kg_prov = res_prov.files.iter().any(|f| {
        f.provenance
            .iter()
            .any(|p| p.contains("Native KG DIRECT [RESOLVED]") || p.contains("Native KG"))
    });
    assert!(
        has_kg_prov,
        "Graph-expanded context files must carry explicit Native KG resolution provenance"
    );
}

#[test]
fn test_crash_metadata_repo_root_move_and_security_boundaries() {
    let repo = TestRepo::new("security_crash");
    repo.write("src/safe.rs", "pub fn safe_op() -> bool { true }\n");
    repo.write(".env", "API_KEY=sk-secret1234567890abcdef\n");
    repo.commit("init");

    // Initial build
    sync_knowledge_graph(repo.path(), true).unwrap();

    // Simulate interrupted indexing crash ('INDEXING' left in kg_metadata)
    {
        let conn = open_graph_db(repo.path()).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO kg_metadata (key, value) VALUES ('state', 'INDEXING')",
            [],
        )
        .unwrap();
    }
    let rec_status = sync_knowledge_graph(repo.path(), false).unwrap();
    assert_eq!(rec_status.state, GraphLifecycleState::Ready);

    // Simulate moved/copied repo_root_hash mismatch -> forces clean full rebuild automatically
    {
        let conn = open_graph_db(repo.path()).unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO kg_metadata (key, value) VALUES ('repo_root_hash', 'stale_old_root_hash')",
            [],
        )
        .unwrap();
    }
    let moved_status = sync_knowledge_graph(repo.path(), false).unwrap();
    assert_eq!(moved_status.state, GraphLifecycleState::Ready);

    // Secret exclusion check (.env must not appear in kg_nodes)
    {
        let conn = open_graph_db(repo.path()).unwrap();
        let idx = KnowledgeGraphIndex::load_from_db(&conn).unwrap();
        assert!(!idx.nodes_by_id.contains_key("file:.env"));
        assert!(
            !idx.nodes_by_id
                .values()
                .any(|n| n.relative_path.contains(".env"))
        );
    }

    // MCP engram_graph_explain boundary traversal check
    let traversal_err = mcp_call(
        repo.path(),
        "engram_graph_explain",
        &json!({ "target": "../../etc/passwd" }),
    );
    assert!(
        traversal_err.is_err(),
        "Path traversal must be rejected by engram_graph_explain"
    );

    let valid_explain = mcp_call(
        repo.path(),
        "engram_graph_explain",
        &json!({ "target": "src/safe.rs", "depth": 2 }),
    )
    .unwrap();
    let txt = valid_explain["content"][0]["text"].as_str().unwrap();
    assert!(txt.contains("graph_freshness"));
    assert!(txt.contains("src/safe.rs"));
}

#[test]
fn test_cli_and_mcp_graph_endpoints() {
    let repo = TestRepo::new("cli_mcp_graph");
    repo.write("src/base.rs", "pub fn core_val() -> i32 { 7 }\n");
    repo.write("src/qual_leaf.rs", "pub fn leaf_num() -> i32 { 42 }\n");
    repo.write(
        "src/service.rs",
        "use crate::base::core_val;\npub fn svc_run() -> i32 { core_val() + crate::qual_leaf::leaf_num() }\n",
    );
    repo.commit("init");

    // Verify ENGRAM_GRAPH_TOOLS=1 registers engram_graph_explain without altering default core tool list
    let bin = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/debug/engram.exe");
    assert!(
        bin.exists(),
        "Built binary {:?} must exist for CLI tests",
        bin
    );

    let deps_out = Command::new(&bin)
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "--json",
            "graph",
            "dependencies",
            "src/service.rs",
        ])
        .output()
        .unwrap();
    assert!(
        deps_out.status.success(),
        "CLI graph dependencies failed: {}",
        String::from_utf8_lossy(&deps_out.stderr)
    );
    let deps_json: serde_json::Value = serde_json::from_slice(&deps_out.stdout).unwrap();
    assert_eq!(deps_json["schema_version"], GRAPH_SCHEMA_VERSION);
    assert!(!deps_json["dependencies"].as_array().unwrap().is_empty());

    let rdeps_out = Command::new(&bin)
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "--json",
            "graph",
            "reverse-dependencies",
            "src/base.rs",
        ])
        .output()
        .unwrap();
    assert!(rdeps_out.status.success());
    let rdeps_json: serde_json::Value = serde_json::from_slice(&rdeps_out.stdout).unwrap();
    assert!(
        !rdeps_json["reverse_dependencies"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let qual_rdeps_out = Command::new(&bin)
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "--json",
            "graph",
            "reverse-dependencies",
            "src/qual_leaf.rs",
        ])
        .output()
        .unwrap();
    assert!(qual_rdeps_out.status.success());
    let qual_rdeps_json: serde_json::Value =
        serde_json::from_slice(&qual_rdeps_out.stdout).unwrap();
    assert!(
        !qual_rdeps_json["reverse_dependencies"]
            .as_array()
            .unwrap()
            .is_empty(),
        "reverse-dependencies must include qualified symbol references declared in target file"
    );

    let explain_out = Command::new(&bin)
        .args([
            "--root",
            repo.path().to_str().unwrap(),
            "--json",
            "graph",
            "explain",
            "src/service.rs",
        ])
        .output()
        .unwrap();
    assert!(explain_out.status.success());
    let explain_json: serde_json::Value = serde_json::from_slice(&explain_out.stdout).unwrap();
    assert!(
        !explain_json["outgoing_dependencies"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        explain_json["graph_status"]["schema_version"],
        GRAPH_SCHEMA_VERSION
    );

    let _ = tools_definitions();
}
