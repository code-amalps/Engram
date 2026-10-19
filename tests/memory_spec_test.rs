//! Step 0: Engineering Memory Specification Test Suite (Engram v0.8.0).
//! Verifies:
//! 1. Memory record immutability during supersession (ADR-014 -> ADR-029 -> ADR-041).
//! 2. Scoped hierarchy and shadowing (Symbol > Package > Module > Global).
//! 3. Direct contradiction detection requiring human resolution.
//! 4. Human-in-the-loop memory gate: agent output NEVER becomes durable memory automatically.
//! 5. Orthogonal team dimension separation from structural scope.
//! 6. Active query exclusion of superseded records.

use std::fs;
use std::path::PathBuf;

use engram::memory::{
    ConflictType, MemoryLifecycleState, MemoryScope, ScopeLevel, detect_memory_conflicts,
    load_durable_memories, promote_candidate, query_scoped_memories, record_scoped_decision,
    record_task_candidate, supersede_memory,
};

fn setup_temp_repo(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("engram_mem_spec_{}_{}", name, uuid::Uuid::new_v4()));
    if root.exists() {
        let _ = fs::remove_dir_all(&root);
    }
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn test_immutable_record_supersession_chain() {
    let root = setup_temp_repo("supersession");

    // 1. Record ADR-014 (In-memory CSV export)
    let scope_orders = MemoryScope::module("orders");
    let adr14 = record_scoped_decision(
        &root,
        Some("ADR-014"),
        "In-Memory CSV Export",
        "OrderService.ExportCSV buffers CSV in memory before disk write",
        "Initial prototype simplicity",
        scope_orders.clone(),
        Some("ADR-014.md".to_string()),
        "lead_architect",
    )
    .unwrap();
    assert_eq!(adr14.id, "ADR-014");
    assert_eq!(adr14.lifecycle_state, MemoryLifecycleState::Durable);

    // 2. Team records ADR-029 superseding ADR-014 (Streaming CSV Export)
    let adr29 = supersede_memory(
        &root,
        "ADR-014",
        "Streaming CSV Export",
        "OrderService.ExportCSV must stream via io.Writer to avoid memory buffering",
        "Memory consumption on high-volume order exports",
        scope_orders.clone(),
        "lead_architect",
    )
    .unwrap();
    assert_eq!(adr29.supersedes.as_deref(), Some("ADR-014"));
    assert_eq!(adr29.lifecycle_state, MemoryLifecycleState::Durable);

    // Verify ADR-014 was NOT mutated, but marked superseded with pointer
    let doc = load_durable_memories(&root).unwrap();
    let old = doc.memories.iter().find(|m| m.id == "ADR-014").unwrap();
    assert_eq!(
        old.statement,
        "OrderService.ExportCSV buffers CSV in memory before disk write"
    ); // Immutable statement!
    assert_eq!(old.lifecycle_state, MemoryLifecycleState::Superseded);
    assert_eq!(old.superseded_by.as_deref(), Some(adr29.id.as_str()));

    // 3. Active query excludes superseded ADR-014 and returns only active ADR-029
    let active = query_scoped_memories(
        &root,
        "Export CSV for orders",
        &["internal/orders/service.go".into()],
        &[],
        false,
    )
    .unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, adr29.id);
    assert!(active[0].statement.contains("must stream via io.Writer"));

    // 4. Audit query (include_superseded: true) returns both with resolution graph intact
    let audit = query_scoped_memories(
        &root,
        "Export CSV for orders",
        &["internal/orders/service.go".into()],
        &[],
        true,
    )
    .unwrap();
    assert_eq!(audit.len(), 2);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_scope_hierarchy_and_shadowing() {
    let root = setup_temp_repo("shadowing");

    // Global rule: All services must use repository pattern
    let _global = record_scoped_decision(
        &root,
        Some("ADR-001"),
        "Repository Pattern Architecture",
        "All services must use repository pattern for database access",
        "Enforces decoupled persistence",
        MemoryScope::global(),
        None,
        "architect",
    )
    .unwrap();

    // Module-scoped rule: Payment service may bypass repository for high throughput
    let mut payment_scope = MemoryScope::module("payment");
    payment_scope.target = Some("payment".to_string());
    let payment_rule = record_scoped_decision(
        &root,
        Some("ADR-019"),
        "Payment Direct DB Access",
        "Payment service must bypass repository pattern and use direct connection pool",
        "Sub-millisecond settlement latency requirements",
        payment_scope,
        None,
        "payment_lead",
    )
    .unwrap();

    // Conflict engine detects SCOPE_SHADOWING (not a direct contradiction failure!)
    let doc = load_durable_memories(&root).unwrap();
    let conflicts = detect_memory_conflicts(&doc.memories);
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].conflict_type, ConflictType::ScopeShadowing);
    assert!(!conflicts[0].requires_human_resolution); // Handled automatically by scope hierarchy!

    // Query for payment module: specialized rule shadows global rule
    let payment_res = query_scoped_memories(
        &root,
        "Update payment transaction processor",
        &["internal/payment/processor.go".into()],
        &[],
        false,
    )
    .unwrap();
    assert_eq!(payment_res.len(), 1);
    assert_eq!(payment_res[0].id, payment_rule.id);
    assert!(payment_res[0].statement.contains("direct connection pool"));

    // Query for orders module: global rule still active!
    let orders_res = query_scoped_memories(
        &root,
        "Update orders checkout",
        &["internal/orders/service.go".into()],
        &[],
        false,
    )
    .unwrap();
    assert_eq!(orders_res.len(), 1);
    assert_eq!(orders_res[0].id, "ADR-001");
    assert!(orders_res[0].statement.contains("repository pattern"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_direct_contradiction_requires_human_resolution() {
    let root = setup_temp_repo("contradiction");

    let scope = MemoryScope::symbol("PaymentService.Save");

    // Memory A: must use direct connection pool
    let _a = record_scoped_decision(
        &root,
        Some("DEC-01"),
        "Save DB Connection",
        "PaymentService.Save must use direct connection pool",
        "Performance",
        scope.clone(),
        None,
        "dev1",
    )
    .unwrap();

    // Memory B: at same scope level without supersedes link: must not use direct connection pool
    let _b = record_scoped_decision(
        &root,
        Some("DEC-02"),
        "Save Connection Override",
        "PaymentService.Save must not use direct connection pool",
        "Safety",
        scope,
        None,
        "dev2",
    )
    .unwrap();

    let doc = load_durable_memories(&root).unwrap();
    let conflicts = detect_memory_conflicts(&doc.memories);

    assert_eq!(conflicts.len(), 1);
    assert_eq!(
        conflicts[0].conflict_type,
        ConflictType::DirectContradiction
    );
    assert!(
        conflicts[0].requires_human_resolution,
        "Same-scope contradiction MUST require human resolution!"
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_agent_output_never_becomes_durable_automatically() {
    let root = setup_temp_repo("poisoning_gate");

    // 1. Agent executes task and generates candidate memory
    let candidate = record_task_candidate(
        &root,
        "task_agent_123",
        "Unverified Optimization",
        "Disable security token validation in debug builds",
        "Agent noticed speed improvement",
        vec!["auth".to_string()],
        vec!["internal/auth/token.go".to_string()],
        "ai_coding_agent",
    )
    .unwrap();

    assert_eq!(candidate.lifecycle_state, MemoryLifecycleState::Candidate);

    // 2. Crucial Invariant: Candidate does NOT appear in DurableMemory
    let doc = load_durable_memories(&root).unwrap();
    assert!(
        doc.memories.is_empty(),
        "Agent output must NEVER enter durable memory automatically!"
    );

    // 3. Active queries return nothing (agent cannot poison next turn)
    let queried = query_scoped_memories(
        &root,
        "auth token validation",
        &["internal/auth/token.go".into()],
        &[],
        false,
    )
    .unwrap();
    assert!(
        queried.is_empty(),
        "Unpromoted agent candidate must not be retrieved"
    );

    // 4. Only explicit human promotion converts candidate to DurableMemory
    let promoted = promote_candidate(
        &root,
        &candidate.candidate_id,
        "security_team",
        Some("Approved debug flag exception with compile guards"),
    )
    .unwrap();
    assert_eq!(promoted.lifecycle_state, MemoryLifecycleState::Durable);

    let doc_after = load_durable_memories(&root).unwrap();
    assert_eq!(doc_after.memories.len(), 1);
    assert_eq!(doc_after.memories[0].accepted_by, "security_team");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_team_orthogonal_dimension() {
    let mut scope1 = MemoryScope::module("orders");
    scope1.team = Some("orders-team".to_string());

    let mut scope2 = MemoryScope::module("billing");
    scope2.team = Some("billing-team".to_string());

    // Team does not alter structural ScopeLevel hierarchy
    assert_eq!(scope1.level, ScopeLevel::Module);
    assert_eq!(scope2.level, ScopeLevel::Module);
    assert_eq!(scope1.level, scope2.level);

    assert_eq!(scope1.team.as_deref(), Some("orders-team"));
    assert_eq!(scope2.team.as_deref(), Some("billing-team"));
}
