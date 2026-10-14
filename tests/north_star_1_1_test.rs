//! ENGRAM-NORTHSTAR-1.1 Closed-Loop Engineering Flywheel Test Suite:
//! Verifies the 12-stage closed-loop lifecycle across all 7 supported languages
//! (TypeScript, JavaScript, Python, C#, Rust, Go, Java/JVM):
//! 1. Task Initiation
//! 2. Preflight Handshake
//! 3. Violating Mutation
//! 4. Postflight Guard Block
//! 5. Recovery Plan Generation
//! 6. Agent Repair
//! 7. Postflight Guard Pass
//! 8. Human Promotion (Zero Poisoning Invariant)
//! 9. Repository Evolution
//! 10. Drift Detection (Scoped Staleness)
//! 11. Decision Ratification
//! 12. Next Preflight Grounding

#[path = "benchmarks/north_star/mod.rs"]
pub mod north_star;

use engram::drift::{analyze_drift, DriftOptions};
use engram::flywheel::{
    postflight_check, preflight_task, PostflightOptions, PostflightVerdict, PreflightOptions,
    PreflightStatus,
};
use engram::memory::{
    load_durable_memories, promote_candidate, re_ratify_decision, MemoryLifecycleState,
};
use engram::recovery::{get_recovery_session, recover_contract, RecoveryStatus};
use north_star::*;
use std::fs;
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

struct LanguageTestScenario {
    language: &'static str,
    source_rel: &'static str,
    initial_content: &'static str,
    authorized_mutation: &'static str,
    forbidden_rel: &'static str,
    violating_content: &'static str,
    clean_repaired_content: &'static str,
    task_goal: &'static str,
    second_task: &'static str,
}

fn get_all_scenarios() -> Vec<LanguageTestScenario> {
    vec![
        LanguageTestScenario {
            language: "typescript",
            source_rel: "src/billing.ts",
            initial_content: "export class BillingService { process() { return true; } }\n",
            authorized_mutation: "export class BillingService { process() { return true; } refund() { return true; } }\n",
            forbidden_rel: "secrets/keys.ts",
            violating_content: "export const API_KEY = 'sk-1234567890abcdef1234';\n",
            clean_repaired_content: "// pristine keys\n",
            task_goal: "Add refund capability to BillingService in TypeScript",
            second_task: "Update refund validation in BillingService",
        },
        LanguageTestScenario {
            language: "javascript",
            source_rel: "src/billing.js",
            initial_content: "class BillingService { process() { return true; } } module.exports = BillingService;\n",
            authorized_mutation: "class BillingService { process() { return true; } refund() { return true; } } module.exports = BillingService;\n",
            forbidden_rel: "secrets/keys.js",
            violating_content: "const API_KEY = 'sk-1234567890abcdef1234';\n",
            clean_repaired_content: "// pristine keys\n",
            task_goal: "Add refund capability to BillingService in JavaScript",
            second_task: "Update refund logic in BillingService",
        },
        LanguageTestScenario {
            language: "python",
            source_rel: "billing/service.py",
            initial_content: "class BillingService:\n    def process(self):\n        return True\n",
            authorized_mutation: "class BillingService:\n    def process(self):\n        return True\n    def refund(self):\n        return True\n",
            forbidden_rel: "secrets/config.py",
            violating_content: "SECRET_KEY = 'sk-1234567890abcdef1234'\n",
            clean_repaired_content: "# pristine config\n",
            task_goal: "Add refund capability to BillingService in Python",
            second_task: "Update refund policy in BillingService",
        },
        LanguageTestScenario {
            language: "csharp",
            source_rel: "Billing/Service.cs",
            initial_content: "namespace Billing { public class BillingService { public bool Process() => true; } }\n",
            authorized_mutation: "namespace Billing { public class BillingService { public bool Process() => true; public bool Refund() => true; } }\n",
            forbidden_rel: "secrets/vault.cs",
            violating_content: "namespace Secrets { public class Vault { private string key = \"sk-1234\"; } }\n",
            clean_repaired_content: "// pristine vault\n",
            task_goal: "Add refund capability to BillingService in C#",
            second_task: "Update refund process in BillingService",
        },
        LanguageTestScenario {
            language: "rust",
            source_rel: "src/billing.rs",
            initial_content: "pub struct BillingService;\nimpl BillingService { pub fn process(&self) -> bool { true } }\n",
            authorized_mutation: "pub struct BillingService;\nimpl BillingService { pub fn process(&self) -> bool { true } pub fn refund(&self) -> bool { true } }\n",
            forbidden_rel: "secrets/vault.rs",
            violating_content: "pub struct Vault { key: String }\n",
            clean_repaired_content: "// pristine vault\n",
            task_goal: "Add refund capability to BillingService in Rust",
            second_task: "Update refund rules in BillingService",
        },
        LanguageTestScenario {
            language: "go",
            source_rel: "billing/service.go",
            initial_content: "package billing\n\ntype BillingService struct{}\nfunc (s *BillingService) Process() bool { return true }\n",
            authorized_mutation: "package billing\n\ntype BillingService struct{}\nfunc (s *BillingService) Process() bool { return true }\nfunc (s *BillingService) Refund() bool { return true }\n",
            forbidden_rel: "secrets/vault.go",
            violating_content: "package secrets\n\ntype Vault struct{}\n",
            clean_repaired_content: "package secrets\n",
            task_goal: "Add refund capability to BillingService in Go",
            second_task: "Update refund streaming in BillingService",
        },
        LanguageTestScenario {
            language: "java",
            source_rel: "src/main/java/com/engram/billing/BillingService.java",
            initial_content: "package com.engram.billing;\n\npublic class BillingService {\n    public boolean process() { return true; }\n}\n",
            authorized_mutation: "package com.engram.billing;\n\npublic class BillingService {\n    public boolean process() { return true; }\n    public boolean refund() { return true; }\n}\n",
            forbidden_rel: "secrets/Vault.java",
            violating_content: "package secrets;\n\npublic class Vault {\n    private String key = \"sk-1234\";\n}\n",
            clean_repaired_content: "// pristine vault\n",
            task_goal: "Add refund capability to BillingService in Java",
            second_task: "Update refund execution in BillingService",
        },
    ]
}

fn init_git_repo(root: &Path) {
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };
    run(&["init"]);
    run(&["config", "user.name", "NorthStar Benchmark"]);
    run(&["config", "user.email", "northstar@engram.dev"]);
}

fn commit_all(root: &Path, message: &str) -> String {
    let run = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(root)
            .output()
            .unwrap();
        assert!(out.status.success(), "Git command failed: {:?}", args);
    };
    run(&["add", "."]);
    run(&["commit", "-m", message]);

    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn run_single_scenario(scenario: &LanguageTestScenario) -> NorthStarLanguageResult {
    let start = std::time::Instant::now();
    let root = std::env::temp_dir().join(format!("engram_ns11_{}_{}", scenario.language, Uuid::new_v4()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    // Stage 1: TaskInitiation - Create initial project files and Git baseline
    let src_path = root.join(scenario.source_rel);
    if let Some(p) = src_path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    fs::write(&src_path, scenario.initial_content).unwrap();

    let forb_path = root.join(scenario.forbidden_rel);
    if let Some(p) = forb_path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    fs::write(&forb_path, scenario.clean_repaired_content).unwrap();

    init_git_repo(&root);
    let _init_commit = commit_all(&root, "Initial baseline commit");

    // Stage 2: PreflightHandshake
    let pre_opts = PreflightOptions {
        task: scenario.task_goal.to_string(),
        token_budget: 1500,
        forbidden_boundaries: Some(vec!["secrets/**".to_string()]),
    };
    let pre = preflight_task(&root, &pre_opts).expect("Preflight handshake must succeed");
    assert_eq!(pre.status, PreflightStatus::ReadyForImplementation);
    assert!(!pre.contract_id.is_empty());
    assert!(pre.forbidden_boundaries.contains(&"secrets/**".to_string()));

    // Stage 3: ViolatingMutation
    fs::write(&src_path, scenario.authorized_mutation).unwrap();
    fs::write(&forb_path, scenario.violating_content).unwrap();

    // Stage 4: PostflightGuardBlock
    let post_opts = PostflightOptions {
        contract_id: Some(pre.contract_id.clone()),
        strict: false,
        staged: false,
        base_commit: None,
        ci_mode: false,
    };
    let post_block = postflight_check(&root, &post_opts).expect("Postflight check must evaluate");
    assert_eq!(
        post_block.verdict,
        PostflightVerdict::BlockedBoundaryViolation,
        "Postflight must block violating mutation for {}",
        scenario.language
    );
    assert_eq!(post_block.exit_code, 1);
    assert!(post_block.boundary_violation_count >= 1);

    // Stage 5: RecoveryPlanGeneration
    let recovery_rep = recover_contract(&root, &pre.contract_id).expect("Recovery must generate structured plan");
    assert_eq!(recovery_rep.attempt_count, 1);
    assert_eq!(recovery_rep.status, RecoveryStatus::InProgress);
    assert!(
        recovery_rep
            .plan
            .actions
            .iter()
            .any(|a| a.target.contains("secrets")),
        "Recovery plan must instruct reverting unauthorized secrets mutation"
    );

    // Stage 6: AgentRepair
    fs::write(&forb_path, scenario.clean_repaired_content).unwrap();

    // Stage 7: PostflightGuardPass
    let post_pass = postflight_check(&root, &post_opts).expect("Postflight check after repair must succeed");
    assert_eq!(
        post_pass.verdict,
        PostflightVerdict::Pass,
        "Repaired state must pass postflight guard for {}",
        scenario.language
    );
    assert_eq!(post_pass.exit_code, 0);
    assert_eq!(post_pass.generated_candidates.len(), 1);
    let cand_id = &post_pass.generated_candidates[0].candidate_id;

    // Verify recovery session is marked resolved
    let session = get_recovery_session(&root, &pre.contract_id)
        .unwrap()
        .expect("Recovery session must exist");
    assert_eq!(session.status, RecoveryStatus::Resolved);

    // Stage 8: HumanPromotion & Zero-Poisoning Gate
    let mem_pre = load_durable_memories(&root).unwrap();
    assert_eq!(
        mem_pre.memories.len(),
        0,
        "Zero-poisoning gate: unpromoted candidate must never become durable memory automatically"
    );

    let promoted = promote_candidate(
        &root,
        cand_id,
        "principal_engineer",
        Some(&format!("Approved clean {} billing refund implementation", scenario.language)),
    )
    .expect("Human promotion must succeed");
    assert_eq!(promoted.lifecycle_state, MemoryLifecycleState::Durable);

    let mem_post = load_durable_memories(&root).unwrap();
    assert_eq!(mem_post.memories.len(), 1, "Durable memory must contain promoted decision");

    // Stage 9: RepositoryEvolution
    let _fix_commit = commit_all(&root, "Implement billing refund cleanly");

    // Perform 6 governed mutation commits to exceed default staleness threshold (5)
    for i in 1..=6 {
        fs::write(
            &src_path,
            format!("{}\n// governed evolution {}\n", scenario.authorized_mutation, i),
        )
        .unwrap();
        commit_all(&root, &format!("evolution commit {}", i));
    }

    // Stage 10: DriftDetection (Scoped Staleness)
    let drift_opts = DriftOptions {
        staleness_threshold: 5,
        ..Default::default()
    };
    let drift_rep = analyze_drift(&root, &drift_opts).expect("Drift analysis must succeed");
    assert!(
        drift_rep.stale_decisions.iter().any(|d| d.decision_id == promoted.id),
        "Decision {} must be flagged STALE_REVIEW after 6 governed commits for {}",
        promoted.id,
        scenario.language
    );

    // Stage 11: DecisionRatification
    let rat_commit = commit_all(&root, "Touch point before ratification");
    let re_rat = re_ratify_decision(
        &root,
        &promoted.id,
        Some(&rat_commit),
        "principal_engineer",
    )
    .expect("Re-ratification must succeed");
    assert_eq!(re_rat.ratified_at_commit.as_deref(), Some(rat_commit.as_str()));

    let drift_rep_post = analyze_drift(&root, &drift_opts).expect("Drift analysis after ratification must succeed");
    assert!(
        !drift_rep_post.stale_decisions.iter().any(|d| d.decision_id == promoted.id),
        "Decision must no longer be stale after explicit re-ratification"
    );

    // Stage 12: NextPreflightGrounding
    let next_pre_opts = PreflightOptions {
        task: scenario.second_task.to_string(),
        token_budget: 1500,
        forbidden_boundaries: None,
    };
    let next_pre = preflight_task(&root, &next_pre_opts).expect("Next preflight must succeed");
    assert_eq!(next_pre.status, PreflightStatus::ReadyForImplementation);
    assert!(
        next_pre.governing_policies.iter().any(|m| m.title.contains("billing refund") || m.id == promoted.id),
        "Next preflight must ground previously promoted decision into context for {}",
        scenario.language
    );

    let _ = fs::remove_dir_all(&root);

    NorthStarLanguageResult {
        language: scenario.language.to_string(),
        stages_completed: 12,
        total_stages: 12,
        recovery_success: true,
        zero_poisoning_verified: true,
        drift_detected: true,
        memory_compounding_verified: true,
        execution_ms: start.elapsed().as_secs_f64() * 1000.0,
    }
}

#[test]
fn test_north_star_1_1_seven_language_closed_loop_matrix() {
    let scenarios = get_all_scenarios();
    assert_eq!(scenarios.len(), 7, "All 7 languages must be represented in North-Star 1.1");

    let mut results = Vec::new();
    for s in &scenarios {
        let res = run_single_scenario(s);
        assert_eq!(res.stages_completed, 12, "All 12 stages must pass for {}", s.language);
        assert!(res.recovery_success);
        assert!(res.zero_poisoning_verified);
        assert!(res.drift_detected);
        assert!(res.memory_compounding_verified);
        results.push(res);
    }

    assert_eq!(results.len(), 7);
}

#[test]
fn test_north_star_1_1_failure_taxonomy_classification() {
    let records = vec![
        TaxonomyRecord {
            category: FailureTaxonomyCategory::Parser,
            description: "C# top-level statements without namespace wrappers".to_string(),
            resolution: TaxonomyResolutionStatus::Fixed,
            affected_languages: vec!["csharp".to_string()],
        },
        TaxonomyRecord {
            category: FailureTaxonomyCategory::Git,
            description: "Shallow clone (--depth 1) missing historical commit ancestry".to_string(),
            resolution: TaxonomyResolutionStatus::AcceptedLimitation,
            affected_languages: vec!["all".to_string()],
        },
        TaxonomyRecord {
            category: FailureTaxonomyCategory::Recovery,
            description: "Exhaustion at 2 authorized retries blocking on 3rd attempt".to_string(),
            resolution: TaxonomyResolutionStatus::Fixed,
            affected_languages: vec!["all".to_string()],
        },
    ];

    assert_eq!(records.len(), 3);
    assert_eq!(records[0].category.as_str(), "PARSER");
    assert_eq!(records[1].resolution.as_str(), "ACCEPTED_LIMITATION");
    assert_eq!(records[2].category.as_str(), "RECOVERY");
}
