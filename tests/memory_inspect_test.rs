//! Integration test for Engram v0.8.0 Memory Tree Inspect & Observability.

use std::fs;
use std::path::PathBuf;

use engram::analyzer::analyze;
use engram::impact::compute_impact_cone;
use engram::memory::{
    MemoryLifecycleState, MemoryScope, ScopeLevel, detect_memory_conflicts, query_scoped_memories,
    record_scoped_decision, supersede_memory,
};
use engram::storage::db;

fn setup_temp_repo(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "engram_inspect_test_{}_{}",
        name,
        uuid::Uuid::new_v4()
    ));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(&root).unwrap();

    let go_mod = "module github.com/acme/orders\n\ngo 1.22\n";
    let svc = "package orders\n\ntype OrderService struct {}\n\nfunc (s *OrderService) ExportCSV() error {\n\treturn nil\n}\n";

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
        .args(["commit", "-m", "Init"])
        .current_dir(&root)
        .output();

    root
}

#[test]
fn test_inspect_memory_supersession_tree() {
    let root = setup_temp_repo("inspect_tree");

    // 1. Analyze repository
    let _ = analyze(&root).unwrap();

    // 2. Record ADR-014 (Module scoped)
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

    // 3. Supersede ADR-014 with ADR-029
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

    // 4. Record DEC-041 (Symbol scoped)
    let _dec41 = record_scoped_decision(
        &root,
        Some("DEC-041"),
        "RFC 4180 Headers",
        "OrderService.ExportCSV headers must adhere to RFC 4180",
        "Standard compatibility",
        MemoryScope::symbol("OrderService.ExportCSV"),
        None,
        "lead_architect",
    )
    .unwrap();

    // 5. Compute Impact Cone for OrderService.ExportCSV
    let conn = db(&root).unwrap();
    let cone = compute_impact_cone(&conn, &root, "OrderService.ExportCSV", 2).unwrap();
    let affected_paths = cone.all_affected_paths();

    // 6. Inspect Memory Query (Audit mode: include_superseded = true)
    let audit_memories = query_scoped_memories(
        &root,
        "OrderService.ExportCSV",
        &affected_paths,
        &["OrderService.ExportCSV".into()],
        true,
    )
    .unwrap();

    assert_eq!(audit_memories.len(), 3);

    // Verify ordering and statuses
    let mem_adr14 = audit_memories.iter().find(|m| m.id == "ADR-014").unwrap();
    let mem_adr29 = audit_memories.iter().find(|m| m.id == adr29.id).unwrap();
    let mem_dec41 = audit_memories.iter().find(|m| m.id == "DEC-041").unwrap();

    assert_eq!(mem_adr14.lifecycle_state, MemoryLifecycleState::Superseded);
    assert_eq!(mem_adr14.superseded_by.as_deref(), Some(adr29.id.as_str()));

    assert_eq!(mem_adr29.lifecycle_state, MemoryLifecycleState::Durable);
    assert_eq!(mem_adr29.supersedes.as_deref(), Some("ADR-014"));

    assert_eq!(mem_dec41.lifecycle_state, MemoryLifecycleState::Durable);
    assert_eq!(mem_dec41.scope.level, ScopeLevel::Symbol);

    // 7. Inspect Memory Query (Active mode: include_superseded = false)
    let active_memories = query_scoped_memories(
        &root,
        "OrderService.ExportCSV",
        &affected_paths,
        &["OrderService.ExportCSV".into()],
        false,
    )
    .unwrap();

    assert_eq!(active_memories.len(), 2);
    assert!(!active_memories.iter().any(|m| m.id == "ADR-014"));
    assert!(active_memories.iter().any(|m| m.id == adr29.id));
    assert!(active_memories.iter().any(|m| m.id == "DEC-041"));

    // 8. Conflict Detection across memories
    let conflicts = detect_memory_conflicts(&audit_memories);
    // Explicit supersession cleanly resolves ADR-014 and ADR-029; DEC-041 is orthogonal RFC formatting
    assert!(
        conflicts.is_empty(),
        "No unresolved conflict should exist for cleanly superseded and orthogonal memories"
    );

    let _ = fs::remove_dir_all(&root);
}
