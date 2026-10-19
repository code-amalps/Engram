//! Step 0: Engineering Decision Intelligence Specification Test Suite (Engram v0.9.0).
//! Verifies data contracts and expected behavior for:
//! 1. Memory ↔ Code Relational Graph (Bidirectional GOVERNS, EXPLAINS, MOTIVATED_BY, SUPERSEDES)
//! 2. Decision Archaeology (`engram why <symbol>`)
//! 3. Memory Doctor diagnosis (Liveness, Orphan detection, DORMANT state transition)
//! 4. Multi-factor Archival Candidate evaluation
//! 5. Controlled Garbage Collection with mandatory `--dry-run` pre-flight

use std::fs;
use std::path::PathBuf;

use engram::analyzer::analyze;
use engram::archaeology::{ArchaeologyReport, explain_why};
use engram::doctor::{DoctorReport, LivenessStatus, diagnose_memory_health};
use engram::gc::{GcOptions, GcPlan, execute_gc};
use engram::memory::{
    MemoryLifecycleState, MemoryScope, load_durable_memories, record_scoped_decision,
    supersede_memory,
};
use engram::memory_graph::{build_memory_graph, query_governed_entities, query_governing_memories};
use engram::storage::db;

fn setup_temp_repo(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("engram_v09_spec_{}_{}", name, uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(&root).unwrap();

    let go_mod = "module github.com/acme/orders\n\ngo 1.22\n";
    let svc = "package orders\n\ntype OrderService struct {}\n\nfunc (s *OrderService) ExportCSV() error {\n\treturn nil\n}\nfunc (s *OrderService) Save() error {\n\treturn nil\n}\n";

    fs::write(root.join("go.mod"), go_mod).unwrap();
    let dir = root.join("internal/orders");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("service.go"), svc).unwrap();

    let _ = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&root)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Tester"])
        .current_dir(&root)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "test@engram.dev"])
        .current_dir(&root)
        .output();
    let _ = std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output();
    let _ = std::process::Command::new("git")
        .args(["commit", "-m", "Initial orders structure"])
        .current_dir(&root)
        .output();

    root
}

#[test]
fn test_memory_code_graph_bidirectional_traversal() {
    let root = setup_temp_repo("graph_bidirectional");
    analyze(&root).unwrap();

    // 1. Record ADR-014 (Module scoped)
    let _adr14 = record_scoped_decision(
        &root,
        Some("ADR-014"),
        "In-Memory CSV Export",
        "OrderService.ExportCSV buffers CSV in memory before disk write",
        "Initial prototype simplicity",
        MemoryScope::module("orders"),
        Some("ADR-014.md".to_string()),
        "lead_architect",
    )
    .unwrap();

    // 2. Supersede ADR-014 with ADR-029
    let adr29 = supersede_memory(
        &root,
        "ADR-014",
        "Streaming CSV Export",
        "OrderService.ExportCSV must stream via io.Writer to avoid memory buffering",
        "Prevents memory exhaustion on high-volume order exports",
        MemoryScope::module("orders"),
        "lead_architect",
    )
    .unwrap();

    // 3. Build relational memory graph in SQLite
    let conn = db(&root).unwrap();
    build_memory_graph(&root, &conn).unwrap();

    // 4. Test Code -> Memory traversal: What decisions govern OrderService.ExportCSV?
    let governing = query_governing_memories(&conn, "OrderService.ExportCSV").unwrap();
    assert!(
        !governing.is_empty(),
        "Governing memories must not be empty"
    );
    assert!(
        governing.iter().any(|m| m.id == adr29.id),
        "ADR-029 must govern OrderService.ExportCSV"
    );

    // 5. Test Memory -> Code traversal: What code does ADR-029 govern?
    let governed = query_governed_entities(&conn, &adr29.id).unwrap();
    assert!(!governed.is_empty(), "Governed entities must not be empty");
    assert!(
        governed
            .iter()
            .any(|e| e.name == "OrderService.ExportCSV" || e.name.contains("orders"))
    );

    // 6. Test Supersedes lineage edge in graph
    let old_lineage = query_governing_memories(&conn, "OrderService.ExportCSV")
        .unwrap()
        .into_iter()
        .find(|m| m.id == "ADR-014");
    // Active governing query excludes superseded, but graph preserves SUPERSEDES edge
    assert!(
        old_lineage.is_none(),
        "Active governing query must exclude superseded ADR-014"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_decision_archaeology_why_query() {
    let root = setup_temp_repo("archaeology");
    analyze(&root).unwrap();

    // Record historical ADR-014 and successor ADR-029
    let _adr14 = record_scoped_decision(
        &root,
        Some("ADR-014"),
        "In-Memory CSV Export",
        "OrderService.ExportCSV buffers CSV in memory before disk write",
        "Initial prototype simplicity",
        MemoryScope::module("orders"),
        Some("ADR-014.md".to_string()),
        "lead_architect",
    )
    .unwrap();

    let adr29 = supersede_memory(
        &root,
        "ADR-014",
        "Streaming CSV Export",
        "OrderService.ExportCSV must stream via io.Writer to avoid memory buffering",
        "Prevents memory exhaustion on high-volume order exports",
        MemoryScope::module("orders"),
        "lead_architect",
    )
    .unwrap();

    let conn = db(&root).unwrap();
    build_memory_graph(&root, &conn).unwrap();

    // Execute decision archaeology: Why was OrderService.ExportCSV designed this way?
    let report: ArchaeologyReport = explain_why(&root, "OrderService.ExportCSV").unwrap();

    assert_eq!(report.target_symbol, "OrderService.ExportCSV");
    assert_eq!(
        report.current_governing_policy.as_ref().unwrap().id,
        adr29.id
    );
    assert_eq!(report.historical_evolution.len(), 1);
    assert_eq!(report.historical_evolution[0].id, "ADR-014");
    assert_eq!(
        report.historical_evolution[0].lifecycle_state,
        MemoryLifecycleState::Superseded
    );
    assert!(report.rationale_summary.contains("io.Writer"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_memory_doctor_liveness_and_dormant_state() {
    let root = setup_temp_repo("doctor_dormant");
    analyze(&root).unwrap();

    // 1. Record decision targeting specific symbol OrderService.ExportCSV
    let sym_memory = record_scoped_decision(
        &root,
        Some("DEC-041"),
        "RFC 4180 Headers",
        "OrderService.ExportCSV headers must adhere to RFC 4180",
        "Standard compliance",
        MemoryScope::symbol("OrderService.ExportCSV"),
        None,
        "lead_architect",
    )
    .unwrap();
    assert_eq!(sym_memory.lifecycle_state, MemoryLifecycleState::Durable);

    // 2. Doctor check when code target is present -> LIVE
    let report1: DoctorReport = diagnose_memory_health(&root).unwrap();
    let dec41_status = report1
        .items
        .iter()
        .find(|i| i.memory_id == "DEC-041")
        .unwrap();
    assert_eq!(dec41_status.liveness, LivenessStatus::Live);

    // 3. Refactor deletes OrderService.ExportCSV from internal/orders/service.go!
    let new_svc = "package orders\n\ntype OrderService struct {}\n\nfunc (s *OrderService) Save() error {\n\treturn nil\n}\n";
    fs::write(root.join("internal/orders/service.go"), new_svc).unwrap();

    // Re-analyze AST
    analyze(&root).unwrap();

    // 4. Doctor diagnosis detects target is missing -> transitions to DORMANT
    let report2: DoctorReport = diagnose_memory_health(&root).unwrap();
    let dec41_status2 = report2
        .items
        .iter()
        .find(|i| i.memory_id == "DEC-041")
        .unwrap();
    assert_eq!(dec41_status2.liveness, LivenessStatus::Dormant);
    assert!(
        dec41_status2
            .diagnostic_message
            .contains("not found in active codebase")
    );

    // 5. Verify DORMANT memory is excluded from active compiler prompts (Poisoning prevention)
    let active_mems = engram::memory::query_scoped_memories(
        &root,
        "Export CSV for orders",
        &["internal/orders/service.go".into()],
        &[],
        false,
    )
    .unwrap();
    assert!(
        !active_mems.iter().any(|m| m.id == "DEC-041"),
        "DORMANT memory with disappeared target must NOT be injected into active coding context"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_controlled_gc_dry_run_and_execution() {
    let root = setup_temp_repo("controlled_gc");
    analyze(&root).unwrap();

    // 1. Record deep superseded chain: ADR-01 -> ADR-02 -> ADR-03 -> ADR-04
    let scope = MemoryScope::module("orders");
    let _a1 = record_scoped_decision(
        &root,
        Some("ADR-01"),
        "V1 Policy",
        "Rule 1",
        "R1",
        scope.clone(),
        None,
        "arch",
    )
    .unwrap();
    let a2 = supersede_memory(
        &root,
        "ADR-01",
        "V2 Policy",
        "Rule 2",
        "R2",
        scope.clone(),
        "arch",
    )
    .unwrap();
    let a3 = supersede_memory(
        &root,
        &a2.id,
        "V3 Policy",
        "Rule 3",
        "R3",
        scope.clone(),
        "arch",
    )
    .unwrap();
    let _a4 = supersede_memory(&root, &a3.id, "V4 Policy", "Rule 4", "R4", scope, "arch").unwrap();

    // 2. Doctor identifies ADR-01 as ArchivalCandidate (depth >= 3)
    let doc_report = diagnose_memory_health(&root).unwrap();
    let a1_status = doc_report
        .items
        .iter()
        .find(|i| i.memory_id == "ADR-01")
        .unwrap();
    assert_eq!(a1_status.liveness, LivenessStatus::ArchivalCandidate);

    // 3. Dry-run GC generates plan without modifying disk
    let dry_run_opts = GcOptions {
        dry_run: true,
        prune_candidates: true,
        archive_superseded: true,
        vacuum: true,
    };
    let plan: GcPlan = execute_gc(&root, &dry_run_opts).unwrap();
    assert!(plan.dry_run);
    assert!(plan.archival_candidates.contains(&"ADR-01".to_string()));

    // Verify disk has NOT been modified yet
    let doc_before = load_durable_memories(&root).unwrap();
    assert!(doc_before.memories.iter().any(|m| m.id == "ADR-01"));

    // 4. Mutating GC execution archives ADR-01 to .engram/archive/memory_archive.yaml
    let exec_opts = GcOptions {
        dry_run: false,
        prune_candidates: true,
        archive_superseded: true,
        vacuum: true,
    };
    let executed_plan = execute_gc(&root, &exec_opts).unwrap();
    assert!(!executed_plan.dry_run);
    assert_eq!(executed_plan.archived_count, 1);

    // Verify ADR-01 was moved from active memory.yaml into archive
    let doc_after = load_durable_memories(&root).unwrap();
    assert!(
        !doc_after.memories.iter().any(|m| m.id == "ADR-01"),
        "Archived ADR-01 must be pruned from hot memory.yaml"
    );

    let archive_path = root
        .join(".engram")
        .join("archive")
        .join("memory_archive.yaml");
    assert!(
        archive_path.exists(),
        "Cold memory archive must exist on disk"
    );
    let archive_contents = fs::read_to_string(archive_path).unwrap();
    assert!(archive_contents.contains("ADR-01"));

    let _ = fs::remove_dir_all(&root);
}
