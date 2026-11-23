//! Phase 8.1: Consolidated Engagement Endpoint Performance & Efficiency Benchmark
//!
//! Evaluates `engram_engage` against the multi-call baseline (`engram_get_context` + `engram_preflight`)
//! across 5 representative tasks:
//! - TASK-01: Trivial Documentation Leaf ("Fix typo in README.md")
//! - TASK-02: Single-File Refactor ("Refactor token validation in src/auth.rs")
//! - TASK-03: Multi-File Feature ("Extend User model and update authentication in src/auth.rs and src/user.rs")
//! - TASK-04: Bounded Subsystem Query ("Analyze security policies in src/auth.rs")
//! - TASK-05: Constitutional Boundary Check ("Update credentials in config/secrets.json")

use engram::mcp::mcp_call;
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

fn setup_benchmark_workspace(name: &str) -> PathBuf {
    let tmp = std::env::temp_dir().join(format!(
        "engram_p81_bench_{}_{}",
        name,
        uuid::Uuid::new_v4()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let run_git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&tmp)
            .output()
            .expect("git execution failed");
        assert!(output.status.success());
    };

    run_git(&["init", "-b", "main"]);
    run_git(&["config", "user.name", "Engram Benchmark"]);
    run_git(&["config", "user.email", "bench@engram.dev"]);

    let engram_dir = tmp.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    let constitution = r#"
rules:
  - id: "SEC-001"
    name: "Protect credentials and secrets"
    type: "PATTERN"
    severity: "ERROR"
    forbidden: "config/secrets.json"
"#;
    fs::write(engram_dir.join("constitution.yaml"), constitution).unwrap();

    let files = vec![
        (
            "Cargo.toml",
            "[package]\nname = \"bench_service\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        (
            "src/lib.rs",
            "pub mod auth;\npub mod user;\npub mod config;\n",
        ),
        (
            "src/auth.rs",
            "pub struct AuthService { pub enabled: bool }\nimpl AuthService {\n    pub fn verify(token: &str) -> bool { !token.is_empty() }\n}\n",
        ),
        (
            "src/user.rs",
            "pub struct User { pub id: u64, pub name: String }\nimpl User {\n    pub fn new(id: u64, name: &str) -> Self { Self { id, name: name.to_string() } }\n}\n",
        ),
        (
            "src/config.rs",
            "pub struct Config { pub port: u16 }\nimpl Default for Config { fn default() -> Self { Self { port: 8080 } } }\n",
        ),
        (
            "README.md",
            "# Microservice Benchmark\nHigh performance authentication microservice.\n",
        ),
    ];

    for (rel, content) in files {
        let p = tmp.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(p, content).unwrap();
    }

    run_git(&["add", "."]);
    run_git(&["commit", "-m", "Initial commit"]);

    tmp
}

#[allow(dead_code)]
struct TaskBenchResult {
    task_id: &'static str,
    description: &'static str,
    baseline_calls: usize,
    baseline_bytes: usize,
    baseline_ms: f64,
    engage_calls: usize,
    engage_bytes: usize,
    engage_ms: f64,
    calls_reduction_pct: f64,
    bytes_reduction_pct: f64,
    latency_reduction_pct: f64,
}

#[test]
fn test_benchmark_engage_vs_multicall() {
    let ws = setup_benchmark_workspace("eval");

    let tasks: Vec<(&str, &str, Value, Value, Value)> = vec![
        (
            "TASK-01",
            "Trivial Documentation Leaf",
            json!({ "task": "Fix typo in README.md" }),
            json!({ "task": "Fix typo in README.md", "planned_files": ["README.md"] }),
            json!({ "task": "Fix typo in README.md" }),
        ),
        (
            "TASK-02",
            "Single-File Refactor",
            json!({ "task": "Refactor token verification in src/auth.rs", "token_budget": 1200 }),
            json!({ "task": "Refactor token verification in src/auth.rs", "planned_files": ["src/auth.rs"] }),
            json!({ "task": "Refactor token verification in src/auth.rs", "planned_files": ["src/auth.rs"], "token_budget": 1200 }),
        ),
        (
            "TASK-03",
            "Multi-File Feature",
            json!({ "task": "Extend User model and update authentication in src/auth.rs and src/user.rs", "token_budget": 1500 }),
            json!({ "task": "Extend User model and update authentication in src/auth.rs and src/user.rs", "planned_files": ["src/auth.rs", "src/user.rs"] }),
            json!({ "task": "Extend User model and update authentication in src/auth.rs and src/user.rs", "planned_files": ["src/auth.rs", "src/user.rs"], "token_budget": 1500 }),
        ),
        (
            "TASK-04",
            "Bounded Subsystem Query",
            json!({ "task": "Analyze security policies in src/auth.rs", "symbols": ["AuthService"], "token_budget": 1000 }),
            json!({ "task": "Analyze security policies in src/auth.rs", "planned_files": ["src/auth.rs"] }),
            json!({ "task": "Analyze security policies in src/auth.rs", "symbols": ["AuthService"], "planned_files": ["src/auth.rs"], "token_budget": 1000 }),
        ),
        (
            "TASK-05",
            "Constitutional Boundary Check",
            json!({ "task": "Update credentials in config/secrets.json", "token_budget": 1000 }),
            json!({ "task": "Update credentials in config/secrets.json", "planned_files": ["config/secrets.json"], "forbidden": ["config/secrets.json"] }),
            json!({ "task": "Update credentials in config/secrets.json", "planned_files": ["config/secrets.json"], "forbidden": ["config/secrets.json"], "token_budget": 1000 }),
        ),
    ];

    let mut results: Vec<TaskBenchResult> = Vec::new();

    // Warm-up caches and database
    let _ = mcp_call(&ws, "engram_get_context", &json!({ "task": "Warmup" }));

    for (task_id, description, ctx_args, preflight_args, engage_args) in tasks {
        // 1. Multi-call baseline (engram_get_context + engram_preflight)
        let t0 = Instant::now();
        let ctx_res = mcp_call(&ws, "engram_get_context", &ctx_args).expect("ctx call failed");
        let preflight_res =
            mcp_call(&ws, "engram_preflight", &preflight_args).expect("preflight call failed");
        let baseline_dur = t0.elapsed();

        let ctx_str = serde_json::to_string(&ctx_res).unwrap();
        let preflight_str = serde_json::to_string(&preflight_res).unwrap();
        let baseline_bytes = ctx_str.len() + preflight_str.len();
        let baseline_ms = baseline_dur.as_micros() as f64 / 1000.0;

        // 2. Consolidated engram_engage
        let t1 = Instant::now();
        let engage_res = mcp_call(&ws, "engram_engage", &engage_args).expect("engage call failed");
        let engage_dur = t1.elapsed();

        let engage_str = serde_json::to_string(&engage_res).unwrap();
        let engage_bytes = engage_str.len();
        let engage_ms = engage_dur.as_micros() as f64 / 1000.0;

        let calls_reduction = 50.0; // 2 calls -> 1 call = 50% reduction
        let bytes_reduction = if baseline_bytes > 0 {
            ((baseline_bytes as f64 - engage_bytes as f64) / baseline_bytes as f64) * 100.0
        } else {
            0.0
        };
        let latency_reduction = if baseline_ms > 0.0 {
            ((baseline_ms - engage_ms) / baseline_ms) * 100.0
        } else {
            0.0
        };

        results.push(TaskBenchResult {
            task_id,
            description,
            baseline_calls: 2,
            baseline_bytes,
            baseline_ms,
            engage_calls: 1,
            engage_bytes,
            engage_ms,
            calls_reduction_pct: calls_reduction,
            bytes_reduction_pct: bytes_reduction,
            latency_reduction_pct: latency_reduction,
        });
    }

    println!("\n=== PHASE 8.1: CONSOLIDATION BENCHMARK RESULTS ===");
    println!(
        "| Task ID | Description | Multi-Call Calls | Multi-Call Bytes | Multi-Call ms | Engage Calls | Engage Bytes | Engage ms | Bytes Saved (%) | Latency Saved (%) |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|");
    for r in &results {
        println!(
            "| {} | {} | {} | {} B | {:.2} ms | {} | {} B | {:.2} ms | {:.1}% | {:.1}% |",
            r.task_id,
            r.description,
            r.baseline_calls,
            r.baseline_bytes,
            r.baseline_ms,
            r.engage_calls,
            r.engage_bytes,
            r.engage_ms,
            r.bytes_reduction_pct,
            r.latency_reduction_pct
        );
    }
    println!("===================================================\n");

    // Assert that every task achieves:
    // 1. Exactly 1 tool call vs 2 calls (50% call reduction)
    // 2. Positive byte reduction (due to eliminating duplicate task, boundaries, and envelope overhead)
    for r in &results {
        assert_eq!(r.engage_calls, 1);
        assert_eq!(r.baseline_calls, 2);
        assert!(
            r.engage_bytes < r.baseline_bytes,
            "Engage bytes ({}) should be strictly lower than multi-call bytes ({}) for {}",
            r.engage_bytes,
            r.baseline_bytes,
            r.task_id
        );
    }

    let _ = fs::remove_dir_all(&ws);
}
