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
    assert_eq!(total_mutations, expected_mutations); // 7 tasks * 4 mutations each = 28
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
