use std::fs;
use std::path::PathBuf;

use engram::analyzer::parse_file;
use engram::contract::{
    FileValidationCategory, create_contract, reconcile_contract, validate_contract_diff,
};
use engram::git_history::{CoChangeSignal, is_noise_file};
use engram::memory::{
    MemoryLifecycleState, load_durable_memories, promote_candidate, query_relevant_memories,
    record_decision, record_task_candidate, reject_candidate,
};
use engram::model::EvidenceTier;

fn temp_test_dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!("engram_p3_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn test_change_contract_elastic_boundaries() {
    let dir = temp_test_dir();
    let root = &dir;

    // Create an initial contract with expected modules and forbidden boundaries
    let contract = create_contract(
        root,
        "Add CSV export to OrderService",
        Some(vec!["orders".to_string()]),
        Some(vec![
            "src/orders/order_service.rs".to_string(),
            "src/orders/export/csv_exporter.rs".to_string(),
            "tests/order_export_test.rs".to_string(),
        ]),
        Some(vec![
            "src/payments/**".to_string(),
            "payments/**".to_string(),
        ]),
    )
    .unwrap();

    assert_eq!(contract.version, 1);

    // Simulated agent edits:
    // 1. Expected: src/orders/order_service.rs, src/orders/export/csv_exporter.rs
    // 2. Expected Expansion: src/orders/export/csv_serializer.rs (new helper within "orders" module)
    // 3. Unexpected: src/notifications/mailer.rs (outside orders, but not forbidden)
    // 4. Boundary Violation: src/payments/billing.rs (forbidden!)
    // Note: tests/order_export_test.rs is missing from edits
    let changed_files = vec![
        "src/orders/order_service.rs".to_string(),
        "src/orders/export/csv_exporter.rs".to_string(),
        "src/orders/export/csv_serializer.rs".to_string(),
        "src/notifications/mailer.rs".to_string(),
        "src/payments/billing.rs".to_string(),
    ];

    let result = validate_contract_diff(&contract, &changed_files);

    // Assert overall validity (violation makes it invalid)
    assert!(!result.is_valid);
    assert_eq!(result.summary.violation_count, 1);
    assert_eq!(result.summary.expected_count, 2);
    assert_eq!(result.summary.expansion_count, 1);
    assert_eq!(result.summary.unexpected_count, 1);
    assert_eq!(result.summary.missing_count, 1);

    // Verify individual classifications
    let billing = result
        .classifications
        .iter()
        .find(|c| c.path == "src/payments/billing.rs")
        .unwrap();
    assert_eq!(billing.category, FileValidationCategory::BoundaryViolation);

    let serializer = result
        .classifications
        .iter()
        .find(|c| c.path == "src/orders/export/csv_serializer.rs")
        .unwrap();
    assert_eq!(
        serializer.category,
        FileValidationCategory::ExpectedExpansion
    );

    let mailer = result
        .classifications
        .iter()
        .find(|c| c.path == "src/notifications/mailer.rs")
        .unwrap();
    assert_eq!(mailer.category, FileValidationCategory::Unexpected);

    // Verify missing expected test was detected
    assert!(
        result
            .missing_expected_changes
            .contains(&"tests/order_export_test.rs".to_string())
    );
}

#[test]
fn test_contract_reconciliation_human_intervention() {
    let dir = temp_test_dir();
    let root = &dir;

    let contract = create_contract(
        root,
        "Refactor Order Workflow",
        Some(vec!["orders".to_string()]),
        Some(vec!["src/orders/workflow.rs".to_string()]),
        Some(vec!["payments/**".to_string()]),
    )
    .unwrap();

    assert_eq!(contract.version, 1);

    // Human intervenes and approves modifying payment audit log
    let reconciled = reconcile_contract(
        root,
        &contract.contract_id,
        "Human intervention: payment audit log is required for regulatory compliance",
        "Amal P S",
        vec!["src/payments/audit.rs".to_string()],
        vec!["payments".to_string()],
    )
    .unwrap();

    assert_eq!(reconciled.version, 2);
    assert_eq!(reconciled.reconciliation_history.len(), 1);
    assert_eq!(reconciled.reconciliation_history[0].actor, "Amal P S");
    assert!(
        reconciled
            .boundary
            .expected_files
            .contains(&"src/payments/audit.rs".to_string())
    );

    // Verify re-validation now treats src/payments/audit.rs as EXPECTED
    let changed = vec![
        "src/orders/workflow.rs".to_string(),
        "src/payments/audit.rs".to_string(),
    ];
    let res = validate_contract_diff(&reconciled, &changed);
    assert!(res.is_valid);
    assert_eq!(res.summary.expected_count, 2);
    assert_eq!(res.summary.violation_count, 0);
}

#[test]
fn test_git_cochange_noise_filters_and_signal_formatting() {
    // Test noise filters
    assert!(is_noise_file("Cargo.lock"));
    assert!(is_noise_file("package-lock.json"));
    assert!(is_noise_file("yarn.lock"));
    assert!(is_noise_file("bundle.min.js"));
    assert!(is_noise_file("target/debug/app.exe"));
    assert!(is_noise_file(".git/config"));
    assert!(!is_noise_file("src/orders/order_service.rs"));
    assert!(!is_noise_file("src/events/order_event_consumer.rs"));

    // Test signal formatting: ensure it presents as advisory historical signal, NOT instruction
    let signal = CoChangeSignal {
        source_file: "src/orders/order_service.rs".to_string(),
        target_file: "src/events/order_event_consumer.rs".to_string(),
        support: 17,
        sample_size: 20,
        conditional_probability: 0.85,
        reverse_probability: 0.42,
        lift: 3.2,
        window: 200,
        advisory_message: "Historical signal: 'src/events/order_event_consumer.rs' changed in 85% of historical commits modifying 'src/orders/order_service.rs' (support: 17/20, lift: 3.2).".to_string(),
    };

    assert!(signal.advisory_message.contains("Historical signal:"));
    assert!(!signal.advisory_message.contains("MUST MODIFY"));
    assert!(!signal.advisory_message.contains("REQUIRED"));
    assert_eq!((signal.conditional_probability * 100.0) as u32, 85);
}

#[test]
fn test_memory_poisoning_protection_and_promotion_lifecycle() {
    let dir = temp_test_dir();
    let root = &dir;

    // 1. Agent modifies code and observes a pattern -> records task candidate
    let candidate = record_task_candidate(
        root,
        "task_123",
        "Controllers return Result<T>",
        "Controllers return Result<T> instead of throwing domain exceptions.",
        "Observed in PR refactor",
        vec!["api".to_string()],
        vec!["src/api/**".to_string()],
        "ai_agent",
    )
    .unwrap();

    // 2. CRITICAL POISONING CHECK: Durable memory must NOT automatically contain this candidate!
    let doc_before = load_durable_memories(root).unwrap();
    assert!(
        doc_before.memories.is_empty(),
        "Memory poisoning! Agent edit was auto-promoted to durable memory."
    );

    // 3. Human rejects an unvetted candidate
    let bad_cand = record_task_candidate(
        root,
        "task_999",
        "Use raw SQL queries",
        "Raw SQL is faster than ORM",
        "Temporary hack",
        vec!["db".to_string()],
        vec![],
        "ai_agent",
    )
    .unwrap();

    reject_candidate(
        root,
        &bad_cand.candidate_id,
        "Amal P S",
        "Security violation: raw SQL is forbidden",
    )
    .unwrap();

    // Verify rejection prevented promotion
    let doc_after_reject = load_durable_memories(root).unwrap();
    assert!(doc_after_reject.memories.is_empty());

    // 4. Human reviews and promotes the valid candidate with explicit WHY rationale
    let promoted = promote_candidate(
        root,
        &candidate.candidate_id,
        "Amal P S",
        Some("Centralizes HTTP transport error mapping across microservices"),
    )
    .unwrap();

    assert_eq!(promoted.lifecycle_state, MemoryLifecycleState::Durable);
    assert_eq!(promoted.evidence_tier, EvidenceTier::Verified);
    assert_eq!(promoted.accepted_by, "Amal P S");

    // 5. Verify durable memory is now safely written to .engram/memory.yaml
    let doc_after = load_durable_memories(root).unwrap();
    assert_eq!(doc_after.memories.len(), 1);
    assert_eq!(doc_after.memories[0].title, "Controllers return Result<T>");
    assert_eq!(
        doc_after.memories[0].reason,
        "Centralizes HTTP transport error mapping across microservices"
    );
}

#[test]
fn test_rust_tree_sitter_ast_extraction() {
    let code = r#"
        pub mod orders;
        use std::collections::HashMap;

        pub struct OrderManager {
            id: u64,
        }

        pub enum OrderState {
            Pending,
            Completed,
        }

        pub trait OrderRepository {
            fn find_by_id(&self, id: u64) -> Option<OrderManager>;
        }

        impl OrderManager {
            pub fn cancel(&mut self) -> bool {
                true
            }
        }
    "#;

    let (record, symbols, rels) = parse_file(
        std::path::Path::new("."),
        "src/orders/manager.rs",
        code.as_bytes(),
        "rev_test",
    )
    .unwrap();

    assert_eq!(record.language, "rust");

    // Verify symbol kinds
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "orders" && s.kind == "MODULE")
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "OrderManager" && s.kind == "STRUCT")
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "OrderState" && s.kind == "ENUM")
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "OrderRepository" && s.kind == "TRAIT")
    );
    assert!(
        symbols
            .iter()
            .any(|s| s.name == "cancel" && s.kind == "FUNCTION")
    );

    // Verify import relationships
    assert!(
        rels.iter()
            .any(|r| r.kind == "IMPORTS" && r.target.contains("std::collections::HashMap"))
    );
}

#[test]
fn test_north_star_e2e_flywheel() {
    let dir = temp_test_dir();
    let root = &dir;

    // Step 1: Record durable architecture rule (WHY) in durable memory
    record_decision(
        root,
        "Order Export Location",
        "All export serializers belong under the orders/export module.",
        "Prevents sprawling export logic across unrelated payment modules.",
        vec!["orders".to_string()],
        vec!["src/orders/export/**".to_string()],
        Some("ADR-008".to_string()),
        "Amal P S",
    )
    .unwrap();

    // Step 2: Task grounding and memory retrieval for "Add CSV export to OrderService"
    let memories = query_relevant_memories(
        root,
        "Add CSV export to OrderService",
        &["src/orders/order_service.rs".to_string()],
    )
    .unwrap();
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].title, "Order Export Location");

    // Step 3: Compile Change Contract v1
    let contract = create_contract(
        root,
        "Add CSV export to OrderService",
        Some(vec!["orders".to_string()]),
        Some(vec![
            "src/orders/order_service.rs".to_string(),
            "src/orders/export/csv_exporter.rs".to_string(),
            "tests/order_export_test.rs".to_string(),
        ]),
        Some(vec![
            "payments/**".to_string(),
            "src/payments/**".to_string(),
        ]),
    )
    .unwrap();

    // Step 4: Agent edits:
    // - OrderService (expected)
    // - CsvExporter (expected)
    // - OrderTests (expected)
    // - OrderEventConsumer (co-change helper in orders module -> expected expansion)
    // - PaymentService (forbidden boundary violation!)
    let agent_edits = vec![
        "src/orders/order_service.rs".to_string(),
        "src/orders/export/csv_exporter.rs".to_string(),
        "tests/order_export_test.rs".to_string(),
        "src/orders/events/order_event_consumer.rs".to_string(),
        "src/payments/payment_service.rs".to_string(),
    ];

    let val1 = validate_contract_diff(&contract, &agent_edits);
    assert!(!val1.is_valid);
    assert_eq!(val1.summary.expected_count, 3);
    assert_eq!(val1.summary.expansion_count, 1);
    assert_eq!(val1.summary.violation_count, 1);

    // Step 5: Developer reviews. Developer decides payment audit logging is intentional:
    let contract_v2 = reconcile_contract(
        root,
        &contract.contract_id,
        "Developer approved: Payment audit logging is required for order receipts",
        "Amal P S",
        vec!["src/payments/payment_service.rs".to_string()],
        vec!["payments".to_string()],
    )
    .unwrap();

    assert_eq!(contract_v2.version, 2);

    // Step 6: Re-validate against Contract v2 -> All valid!
    let val2 = validate_contract_diff(&contract_v2, &agent_edits);
    assert!(val2.is_valid);
    assert_eq!(val2.summary.violation_count, 0);

    // Step 7: Record candidate and promote to durable memory
    let cand = record_task_candidate(
        root,
        &contract_v2.task_id,
        "Order Export Receipts Touch Payment Audit",
        "Order export receipts must log audit events in payments/payment_service.",
        "Established in CSV export task",
        vec!["orders".to_string(), "payments".to_string()],
        vec![],
        "Amal P S",
    )
    .unwrap();

    promote_candidate(root, &cand.candidate_id, "Amal P S", None).unwrap();

    // Step 8: Next task begins: "Export orders to XML"
    let next_task_memories = query_relevant_memories(
        root,
        "Export orders to XML",
        &["src/orders/order_service.rs".to_string()],
    )
    .unwrap();
    // Both durable memories are retrieved!
    assert_eq!(next_task_memories.len(), 2);
}

#[test]
fn test_cache_deletion_and_durable_memory_survival() {
    let dir = temp_test_dir();
    let root = &dir;

    // 1. Record decision in durable memory (.engram/memory.yaml)
    record_decision(
        root,
        "Result Error Policy",
        "All endpoints return Result<T, ApiError>.",
        "Standardized error handling",
        vec!["api".to_string()],
        vec![],
        Some("PR-101".to_string()),
        "Amal P S",
    )
    .unwrap();

    // 2. Simulate complete deletion of cache
    let cache = root.join(".engram_cache_simulated");
    if cache.exists() {
        fs::remove_dir_all(&cache).unwrap();
    }

    // 3. Verify durable memory in .engram/memory.yaml survives cleanly
    let doc = load_durable_memories(root).unwrap();
    assert_eq!(doc.memories.len(), 1);
    assert_eq!(doc.memories[0].title, "Result Error Policy");
}
