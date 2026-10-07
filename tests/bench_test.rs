//! Verification Suite for Phase 3.5 Benchmark Engine:
//! Tests measured vs estimated discovery separation, multi-entity grounding,
//! synthetic mutation classification accuracy, controlled A/B memory lift,
//! determinism, and zero repository side-effects.

use engram::bench::{get_benchmark_tasks, run_benchmark_suite, run_task_benchmark};

#[test]
fn test_discovery_measured_vs_estimated_separation() {
    let tasks = get_benchmark_tasks();
    let task = &tasks[0]; // TypeScript OrderService task

    let res = run_task_benchmark(task).unwrap();

    // Verify strict separation between empirically measured and simulated baseline
    assert!(res.discovery.measured_context_tokens > 0);
    assert!(res.discovery.estimated_baseline_tokens > 0);
    assert_ne!(
        res.discovery.measured_context_tokens,
        res.discovery.estimated_baseline_tokens,
        "Measured context tokens must not be equal to simulated baseline tokens!"
    );

    assert_eq!(res.discovery.measured_tool_calls, 1);
    assert!(res.discovery.estimated_baseline_tool_calls > 1);
    assert!(res.discovery.token_reduction_pct > 0.0);
    assert!(res.discovery.tool_call_reduction_pct > 0.0);
}

#[test]
fn test_grounding_multi_entity_recall_precision() {
    let tasks = get_benchmark_tasks();
    for task in &tasks {
        let res = run_task_benchmark(task).unwrap();

        // Multi-entity grounding evaluations:
        assert!(res.grounding.file_recall > 0.0, "File recall must be positive for {}", task.name);
        assert!(res.grounding.file_precision > 0.0, "File precision must be positive for {}", task.name);
        assert!(res.grounding.file_f1 > 0.0, "File F1 must be positive for {}", task.name);
        assert!(res.grounding.symbol_recall > 0.0, "Symbol recall must be positive for {}", task.name);
        assert!(res.grounding.test_recall > 0.0, "Test recall must be positive for {}", task.name);
        assert!(res.grounding.overall_f1 > 0.0, "Overall F1 must be positive for {}", task.name);
    }
}

#[test]
fn test_synthetic_mutation_accuracy() {
    let tasks = get_benchmark_tasks();
    let mut total_mutations = 0;
    let mut correct_mutations = 0;

    for task in &tasks {
        let res = run_task_benchmark(task).unwrap();
        total_mutations += res.contract.total_mutations;
        correct_mutations += res.contract.correct_classifications;
        assert_eq!(
            res.contract.classification_accuracy_pct, 100.0,
            "Synthetic mutation accuracy failed for task {}",
            task.name
        );
    }

    let expected_mutations = tasks.len() * 4;
    assert_eq!(total_mutations, expected_mutations); // 8 tasks * 4 mutations each = 32
    assert_eq!(correct_mutations, expected_mutations);
}

#[test]
fn test_memory_compounding_ab_control() {
    let tasks = get_benchmark_tasks();
    for task in &tasks {
        let res = run_task_benchmark(task).unwrap();
        if let Some(ref mem) = res.memory {
            // Control (Without Memory) must have 0% recall of Task 1's decision
            assert_eq!(
                mem.control_decision_recall_pct, 0.0,
                "Control must have 0% recall of unlearned decision for {}",
                task.name
            );

            // Treatment (With Memory) must have 100% recall of Task 1's decision in related Task 2
            assert_eq!(
                mem.treatment_decision_recall_pct, 100.0,
                "Treatment must have 100% recall of learned decision for {}",
                task.name
            );

            // Memory lift must be empirically confirmed
            assert!(
                mem.memory_lift_confirmed,
                "Memory lift must be confirmed for {}",
                task.name
            );
        }
    }
}

#[test]
fn test_benchmark_determinism() {
    // Run two identical benchmark suite executions
    let res1 = run_benchmark_suite("git_rev_det_1").unwrap();
    let res2 = run_benchmark_suite("git_rev_det_1").unwrap();

    // Verify deterministic reproducibility
    assert_eq!(res1.total_tasks, res2.total_tasks);
    assert_eq!(res1.metadata.dataset_hash, res2.metadata.dataset_hash);
    assert_eq!(
        res1.aggregate_discovery.measured_context_tokens,
        res2.aggregate_discovery.measured_context_tokens
    );
    assert_eq!(
        res1.aggregate_discovery.estimated_baseline_tokens,
        res2.aggregate_discovery.estimated_baseline_tokens
    );
    assert_eq!(
        res1.aggregate_contract.correct_classifications,
        res2.aggregate_contract.correct_classifications
    );
    assert_eq!(
        res1.aggregate_grounding.overall_f1,
        res2.aggregate_grounding.overall_f1
    );
}

#[test]
fn test_benchmark_zero_side_effects() {
    let root = std::path::Path::new(".");

    // Record initial status
    let initial_status = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    // Run benchmark suite
    let _ = run_benchmark_suite("git_rev_isolation").unwrap();

    // Verify git status is completely unchanged (no temp files or memory pollution left on disk)
    let post_status = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    assert_eq!(
        initial_status, post_status,
        "engram bench caused side effects on the project workspace!"
    );
}

#[test]
fn test_impact_benchmarks_all_scenarios() {
    let tasks = engram::bench::get_impact_benchmark_tasks();
    assert_eq!(tasks.len(), 6);

    for task in &tasks {
        let res = engram::bench::run_task_benchmark(task).unwrap();
        let impact = res.impact.as_ref().expect("Impact scenario must produce metrics");

        assert_eq!(impact.recall, 1.0, "Impact recall must be 100% for {}", task.id);
        assert!(impact.precision >= 0.5, "Impact precision must be >= 0.5 for {}", task.id);
        assert_eq!(
            impact.provenance_accuracy_pct, 100.0,
            "Provenance accuracy must be 100% for {}",
            task.id
        );
        assert_eq!(
            res.contract.classification_accuracy_pct, 100.0,
            "Contract accuracy must be 100% for {}",
            task.id
        );
    }
}

#[test]
fn test_empirical_precision_by_relationship_category() {
    let tasks = engram::bench::get_impact_benchmark_tasks();
    let res = engram::bench::run_task_benchmark(&tasks[0]).unwrap();
    let impact = res.impact.as_ref().expect("Impact scenario must produce metrics");

    // Dynamic empirical precision breakdown by relationship type
    assert!(!impact.precision_by_category.is_empty(), "precision_by_category must be populated");
    assert!(!impact.recall_by_category.is_empty(), "recall_by_category must be populated");
    assert!(impact.precision_by_category.contains_key("DIRECT"));
}

#[test]
fn test_multi_turn_memory_compounding_suite() {
    let res = engram::bench::run_multi_turn_compounding_benchmark().unwrap();

    // 1. Verify 4 multi-turn tasks executed
    assert_eq!(res.total_turns, 4);
    assert_eq!(res.turns.len(), 4);

    // 2. Turn 1: Initial Discovery & Decision Recording
    assert_eq!(res.turns[0].turn_id, "TASK-COMP-01");
    assert!(res.turns[0].success);
    assert_eq!(res.turns[0].active_memories_count, 1);

    // 3. Turn 2: Follow-up Task Memory Grounding (Compounding)
    assert_eq!(res.turns[1].turn_id, "TASK-COMP-02");
    assert!(res.turns[1].success);
    assert_eq!(res.turns[1].grounding_recall, 1.0);

    // 4. Turn 3: Architectural Policy Supersession (ADR-014 -> ADR-029)
    assert_eq!(res.turns[2].turn_id, "TASK-COMP-03");
    assert!(res.turns[2].success);
    assert_eq!(res.supersession_correctness_pct, 100.0, "Superseded policy must be 100% excluded from active query");

    // 5. Turn 4: Scoped Specialization vs Global Shadowing
    assert_eq!(res.turns[3].turn_id, "TASK-COMP-04");
    assert!(res.turns[3].success);
    assert_eq!(res.conflict_resolution_accuracy_pct, 100.0, "Conflict resolution and shadowing must be 100% accurate");

    // 6. Memory Compounding Lift Formula:
    // Compounding Lift = (Baseline Rediscovery Cost - Memory Rediscovery Cost) / Baseline Rediscovery Cost
    assert!(res.baseline_rediscovery_cost > res.memory_rediscovery_cost);
    assert!(res.compounding_lift_pct > 0.0, "Compounding lift must be positive");
    assert!(res.compounding_lift_pct >= 50.0, "Compounding lift should exceed 50% for grounded turns");
}

#[test]
fn test_decision_intelligence_synthetic_benchmark() {
    let di = engram::bench::run_decision_intelligence_benchmark().unwrap();

    assert_eq!(di.total_synthetic_cases, 5);
    assert_eq!(
        di.orphan_detection_accuracy_pct, 100.0,
        "Orphan detection accuracy must be 100% on defined deterministic synthetic corpus"
    );
    assert_eq!(
        di.dormant_transition_accuracy_pct, 100.0,
        "Dormant state transition accuracy must be 100% on defined deterministic synthetic corpus"
    );
    assert_eq!(
        di.archival_candidate_accuracy_pct, 100.0,
        "Archival candidate identification must be 100% on defined deterministic synthetic corpus"
    );
    assert_eq!(
        di.decision_archaeology_accuracy_pct, 100.0,
        "Decision archaeology accuracy must be 100% on defined deterministic synthetic corpus"
    );
    assert!(di.qualified_claim.contains("100% precision on the defined deterministic synthetic orphan/liveness benchmark corpus"));
}
