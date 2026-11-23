//! Phase 8.1.1: Engagement Endpoint Rigorous Benchmark Harness & Methodology
//!
//! Evaluates `engram_engage` against the two-call baseline (`engram_get_context` + `engram_preflight`)
//! using a strictly controlled experimental methodology:
//!
//! 1. Category 1: Equivalent-Work Comparison (30 paired runs per workload, alternating order, state reset):
//!    - EQ-01: Single-File Refactor ("Refactor token validation in src/auth.rs")
//!    - EQ-02: Multi-File Feature ("Extend User model and update auth in src/auth.rs and src/user.rs")
//!    - EQ-03: Subsystem Impact Query ("Analyze security policies in src/auth.rs")
//!    - EQ-04: Constitutional Boundary Check ("Update credentials in config/secrets.json")
//!
//! 2. Category 2: Trivial-Task Bypass (30 paired runs, measured separately as avoided work):
//!    - BYPASS-01: Trivial Documentation Leaf ("Fix typo in README.md")
//!
//! Generates raw reproducible dataset at `docs/phases/phase_8_1_1_benchmark_raw_results.json`.

use engram::mcp::{enforce_mcp_envelope_boundary, mcp_call_raw};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireCallRecord {
    pub tool_name: String,
    pub request_wire_bytes: usize,
    pub response_wire_bytes: usize,
    pub total_wire_bytes: usize,
    pub arguments_bytes: usize,
    pub result_bytes: usize,
    pub duration_ms: f64,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationRecord {
    pub workload_id: String,
    pub workload_category: String,
    pub iteration: usize,
    pub execution_order: String, // "baseline_then_engage" or "engage_then_baseline"
    pub baseline_calls: Vec<WireCallRecord>,
    pub baseline_total_calls: usize,
    pub baseline_req_bytes: usize,
    pub baseline_resp_bytes: usize,
    pub baseline_wire_bytes: usize,
    pub baseline_duration_ms: f64,
    pub engage_calls: Vec<WireCallRecord>,
    pub engage_total_calls: usize,
    pub engage_req_bytes: usize,
    pub engage_resp_bytes: usize,
    pub engage_wire_bytes: usize,
    pub engage_duration_ms: f64,
    pub byte_savings_percent: f64,
    pub latency_savings_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStats {
    pub count: usize,
    pub mean: f64,
    pub median: f64,
    pub std_dev: f64,
    pub min: f64,
    pub max: f64,
    pub p95: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadSummary {
    pub workload_id: String,
    pub category: String,
    pub description: String,
    pub iterations: usize,
    pub baseline_calls: usize,
    pub engage_calls: usize,
    pub baseline_wire_bytes: MetricStats,
    pub engage_wire_bytes: MetricStats,
    pub baseline_duration_ms: MetricStats,
    pub engage_duration_ms: MetricStats,
    pub wire_savings_percent_mean: f64,
    pub latency_savings_percent_mean: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkDataset {
    pub methodology_version: String,
    pub platform: String,
    pub timestamp: String,
    pub total_iterations: usize,
    pub raw_records: Vec<IterationRecord>,
    pub workload_summaries: Vec<WorkloadSummary>,
}

fn compute_stats(values: &[f64]) -> MetricStats {
    if values.is_empty() {
        return MetricStats {
            count: 0,
            mean: 0.0,
            median: 0.0,
            std_dev: 0.0,
            min: 0.0,
            max: 0.0,
            p95: 0.0,
        };
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let count = sorted.len();
    let sum: f64 = sorted.iter().sum();
    let mean = sum / count as f64;

    let median = if count % 2 == 1 {
        sorted[count / 2]
    } else {
        (sorted[count / 2 - 1] + sorted[count / 2]) / 2.0
    };

    let variance: f64 = sorted.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / count as f64;
    let std_dev = variance.sqrt();

    let min = sorted[0];
    let max = sorted[count - 1];

    let p95_idx = ((count as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(count - 1);
    let p95 = sorted[p95_idx];

    MetricStats {
        count,
        mean: (mean * 100.0).round() / 100.0,
        median: (median * 100.0).round() / 100.0,
        std_dev: (std_dev * 100.0).round() / 100.0,
        min: (min * 100.0).round() / 100.0,
        max: (max * 100.0).round() / 100.0,
        p95: (p95 * 100.0).round() / 100.0,
    }
}

/// Executes an MCP call with exact JSON-RPC 2.0 wire framing measurement.
fn execute_measured_mcp_wire_call(
    root: &Path,
    call_id: u64,
    tool_name: &str,
    arguments: &Value,
) -> WireCallRecord {
    let req_obj = json!({
        "jsonrpc": "2.0",
        "id": call_id,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments
        }
    });

    let req_wire_str = format!("{}\n", serde_json::to_string(&req_obj).unwrap());
    let req_wire_bytes = req_wire_str.as_bytes().len();
    let args_bytes = serde_json::to_string(arguments).unwrap().as_bytes().len();

    let t0 = Instant::now();
    let raw_payload = mcp_call_raw(root, tool_name, arguments).unwrap();
    let bounded_payload =
        enforce_mcp_envelope_boundary(tool_name, raw_payload, 16384, &json!(call_id));
    let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;

    let resp_obj = json!({
        "jsonrpc": "2.0",
        "id": call_id,
        "result": bounded_payload
    });

    let resp_wire_str = format!("{}\n", serde_json::to_string(&resp_obj).unwrap());
    let resp_wire_bytes = resp_wire_str.as_bytes().len();
    let result_bytes = serde_json::to_string(&resp_obj["result"])
        .unwrap()
        .as_bytes()
        .len();

    let status = if let Some(content_text) = bounded_payload["content"][0]["text"].as_str() {
        if let Ok(v) = serde_json::from_str::<Value>(content_text) {
            v["status"].as_str().map(str::to_string)
        } else {
            None
        }
    } else {
        None
    };

    WireCallRecord {
        tool_name: tool_name.to_string(),
        request_wire_bytes: req_wire_bytes,
        response_wire_bytes: resp_wire_bytes,
        total_wire_bytes: req_wire_bytes + resp_wire_bytes,
        arguments_bytes: args_bytes,
        result_bytes,
        duration_ms,
        status,
    }
}

/// Reset any active contract or ephemeral session state so runs never contaminate each other.
fn reset_workspace_contract_state(root: &Path) {
    let contracts_dir = root.join(".engram/contracts");
    let _ = fs::remove_dir_all(&contracts_dir);
    let _ = fs::create_dir_all(&contracts_dir);

    let sessions_dir = root.join(".engram/sessions");
    let _ = fs::remove_dir_all(&sessions_dir);
    let _ = fs::create_dir_all(&sessions_dir);
}

fn setup_benchmark_fixture(name: &str) -> PathBuf {
    let tmp = std::env::temp_dir().join(format!(
        "engram_p811_bench_{}_{}",
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
            .expect("git failed");
        assert!(output.status.success());
    };

    run_git(&["init", "-b", "main"]);
    run_git(&["config", "user.name", "Engram Benchmark"]);
    run_git(&["config", "user.email", "bench@engram.dev"]);

    let engram_dir = tmp.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    let constitution = r#"rules:
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
            "[package]\nname = \"bench_svc\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        (
            "src/lib.rs",
            "pub mod auth;\npub mod user;\npub mod config;\n",
        ),
        (
            "src/auth.rs",
            "pub struct AuthService;\nimpl AuthService {\n    pub fn verify(t: &str) -> bool { !t.is_empty() }\n}\n",
        ),
        (
            "src/user.rs",
            "pub struct User { pub id: u64, pub name: String }\nimpl User {\n    pub fn new(id: u64, name: &str) -> Self { Self { id, name: name.into() } }\n}\n",
        ),
        ("src/config.rs", "pub struct Config { pub port: u16 }\n"),
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
        fs::write(&p, content).unwrap();
    }

    run_git(&["add", "."]);
    run_git(&["commit", "-m", "Initial benchmark repository snapshot"]);

    let analyze_status = Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("-C")
        .arg(&tmp)
        .arg("analyze")
        .status()
        .expect("engram analyze failed to spawn");
    assert!(analyze_status.success());

    tmp
}

struct WorkloadDef {
    id: &'static str,
    category: &'static str,
    description: &'static str,
    baseline_get_context: Value,
    baseline_preflight: Value,
    engage_args: Value,
}

#[test]
fn test_rigorous_30_run_benchmark_methodology() {
    let ws = setup_benchmark_fixture("methodology_harness");
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let raw_results_file = manifest_dir.join("docs/phases/phase_8_1_1_benchmark_raw_results.json");

    let workloads = vec![
        // Category 1: Equivalent-Work Comparison
        WorkloadDef {
            id: "EQ-01",
            category: "EQUIVALENT_WORK",
            description: "Single-File Refactor (src/auth.rs)",
            baseline_get_context: json!({
                "task": "Refactor token validation in src/auth.rs",
                "token_budget": 1200
            }),
            baseline_preflight: json!({
                "task": "Refactor token validation in src/auth.rs",
                "planned_files": ["src/auth.rs"]
            }),
            engage_args: json!({
                "task": "Refactor token validation in src/auth.rs",
                "planned_files": ["src/auth.rs"],
                "token_budget": 1200,
                "no_bypass": true
            }),
        },
        WorkloadDef {
            id: "EQ-02",
            category: "EQUIVALENT_WORK",
            description: "Multi-File Feature (src/auth.rs + src/user.rs)",
            baseline_get_context: json!({
                "task": "Extend User model and update authentication in src/auth.rs and src/user.rs",
                "token_budget": 1200
            }),
            baseline_preflight: json!({
                "task": "Extend User model and update authentication in src/auth.rs and src/user.rs",
                "planned_files": ["src/auth.rs", "src/user.rs"]
            }),
            engage_args: json!({
                "task": "Extend User model and update authentication in src/auth.rs and src/user.rs",
                "planned_files": ["src/auth.rs", "src/user.rs"],
                "token_budget": 1200,
                "no_bypass": true
            }),
        },
        WorkloadDef {
            id: "EQ-03",
            category: "EQUIVALENT_WORK",
            description: "Subsystem Query (src/auth.rs)",
            baseline_get_context: json!({
                "task": "Analyze security policies in src/auth.rs",
                "subpath": "src/auth.rs",
                "token_budget": 1200
            }),
            baseline_preflight: json!({
                "task": "Analyze security policies in src/auth.rs"
            }),
            engage_args: json!({
                "task": "Analyze security policies in src/auth.rs",
                "subpath": "src/auth.rs",
                "token_budget": 1200,
                "no_bypass": true
            }),
        },
        WorkloadDef {
            id: "EQ-04",
            category: "EQUIVALENT_WORK",
            description: "Constitutional Boundary Violation (config/secrets.json)",
            baseline_get_context: json!({
                "task": "Update credentials in config/secrets.json",
                "token_budget": 1200
            }),
            baseline_preflight: json!({
                "task": "Update credentials in config/secrets.json",
                "planned_files": ["config/secrets.json"]
            }),
            engage_args: json!({
                "task": "Update credentials in config/secrets.json",
                "planned_files": ["config/secrets.json"],
                "token_budget": 1200,
                "no_bypass": true
            }),
        },
        // Category 2: Trivial Bypass Optimization (Avoided Work)
        WorkloadDef {
            id: "BYPASS-01",
            category: "TRIVIAL_BYPASS",
            description: "Trivial Documentation Leaf (README.md)",
            baseline_get_context: json!({
                "task": "Fix typo in README.md",
                "token_budget": 1200
            }),
            baseline_preflight: json!({
                "task": "Fix typo in README.md",
                "planned_files": ["README.md"]
            }),
            engage_args: json!({
                "task": "Fix typo in README.md",
                "token_budget": 1200,
                "no_bypass": false
            }),
        },
    ];

    let mut all_iteration_records: Vec<IterationRecord> = Vec::new();
    let mut workload_summaries: Vec<WorkloadSummary> = Vec::new();
    let num_iterations = 30;

    println!(
        "\n=========================================================================================="
    );
    println!(
        "EXECUTING RIGOROUS 30-RUN BENCHMARK ACROSS 5 WORKLOADS (150 PAIRED RUNS / 300 EXECUTIONS)"
    );
    println!(
        "=========================================================================================="
    );

    for w in &workloads {
        println!(
            "\nExecuting Workload: {} [{}] - {}",
            w.id, w.category, w.description
        );
        let mut baseline_wire_bytes_list = Vec::new();
        let mut engage_wire_bytes_list = Vec::new();
        let mut baseline_durations_list = Vec::new();
        let mut engage_durations_list = Vec::new();
        let mut wire_savings_list = Vec::new();
        let mut latency_savings_list = Vec::new();

        for i in 1..=num_iterations {
            let order = if i % 2 == 1 {
                "baseline_then_engage"
            } else {
                "engage_then_baseline"
            };

            let mut b_calls = Vec::new();
            let mut e_calls = Vec::new();

            if order == "baseline_then_engage" {
                // 1. Run Baseline
                reset_workspace_contract_state(&ws);
                let c1 = execute_measured_mcp_wire_call(
                    &ws,
                    (i * 10 + 1) as u64,
                    "engram_get_context",
                    &w.baseline_get_context,
                );
                let c2 = execute_measured_mcp_wire_call(
                    &ws,
                    (i * 10 + 2) as u64,
                    "engram_preflight",
                    &w.baseline_preflight,
                );
                b_calls.push(c1);
                b_calls.push(c2);

                // 2. Run Engage
                reset_workspace_contract_state(&ws);
                let ce = execute_measured_mcp_wire_call(
                    &ws,
                    (i * 10 + 3) as u64,
                    "engram_engage",
                    &w.engage_args,
                );
                e_calls.push(ce);
            } else {
                // 1. Run Engage
                reset_workspace_contract_state(&ws);
                let ce = execute_measured_mcp_wire_call(
                    &ws,
                    (i * 10 + 3) as u64,
                    "engram_engage",
                    &w.engage_args,
                );
                e_calls.push(ce);

                // 2. Run Baseline
                reset_workspace_contract_state(&ws);
                let c1 = execute_measured_mcp_wire_call(
                    &ws,
                    (i * 10 + 1) as u64,
                    "engram_get_context",
                    &w.baseline_get_context,
                );
                let c2 = execute_measured_mcp_wire_call(
                    &ws,
                    (i * 10 + 2) as u64,
                    "engram_preflight",
                    &w.baseline_preflight,
                );
                b_calls.push(c1);
                b_calls.push(c2);
            }

            let b_req: usize = b_calls.iter().map(|c| c.request_wire_bytes).sum();
            let b_resp: usize = b_calls.iter().map(|c| c.response_wire_bytes).sum();
            let b_wire: usize = b_req + b_resp;
            let b_dur: f64 = b_calls.iter().map(|c| c.duration_ms).sum();

            let e_req: usize = e_calls.iter().map(|c| c.request_wire_bytes).sum();
            let e_resp: usize = e_calls.iter().map(|c| c.response_wire_bytes).sum();
            let e_wire: usize = e_req + e_resp;
            let e_dur: f64 = e_calls.iter().map(|c| c.duration_ms).sum();

            let wire_savings = if b_wire > 0 {
                (b_wire.saturating_sub(e_wire) as f64 / b_wire as f64) * 100.0
            } else {
                0.0
            };

            let lat_savings = if b_dur > 0.0 {
                ((b_dur - e_dur) / b_dur) * 100.0
            } else {
                0.0
            };

            baseline_wire_bytes_list.push(b_wire as f64);
            engage_wire_bytes_list.push(e_wire as f64);
            baseline_durations_list.push(b_dur);
            engage_durations_list.push(e_dur);
            wire_savings_list.push(wire_savings);
            latency_savings_list.push(lat_savings);

            all_iteration_records.push(IterationRecord {
                workload_id: w.id.to_string(),
                workload_category: w.category.to_string(),
                iteration: i,
                execution_order: order.to_string(),
                baseline_calls: b_calls,
                baseline_total_calls: 2,
                baseline_req_bytes: b_req,
                baseline_resp_bytes: b_resp,
                baseline_wire_bytes: b_wire,
                baseline_duration_ms: b_dur,
                engage_calls: e_calls,
                engage_total_calls: 1,
                engage_req_bytes: e_req,
                engage_resp_bytes: e_resp,
                engage_wire_bytes: e_wire,
                engage_duration_ms: e_dur,
                byte_savings_percent: wire_savings,
                latency_savings_percent: lat_savings,
            });
        }

        let summary = WorkloadSummary {
            workload_id: w.id.to_string(),
            category: w.category.to_string(),
            description: w.description.to_string(),
            iterations: num_iterations,
            baseline_calls: 2,
            engage_calls: 1,
            baseline_wire_bytes: compute_stats(&baseline_wire_bytes_list),
            engage_wire_bytes: compute_stats(&engage_wire_bytes_list),
            baseline_duration_ms: compute_stats(&baseline_durations_list),
            engage_duration_ms: compute_stats(&engage_durations_list),
            wire_savings_percent_mean: (wire_savings_list.iter().sum::<f64>()
                / num_iterations as f64
                * 100.0)
                .round()
                / 100.0,
            latency_savings_percent_mean: (latency_savings_list.iter().sum::<f64>()
                / num_iterations as f64
                * 100.0)
                .round()
                / 100.0,
        };

        println!(
            "  -> Completed: Baseline Wire: {:.1} B, Engage Wire: {:.1} B ({:.1}% savings) | Baseline Dur: {:.1} ms, Engage Dur: {:.1} ms ({:.1}% savings)",
            summary.baseline_wire_bytes.mean,
            summary.engage_wire_bytes.mean,
            summary.wire_savings_percent_mean,
            summary.baseline_duration_ms.mean,
            summary.engage_duration_ms.mean,
            summary.latency_savings_percent_mean
        );

        workload_summaries.push(summary);
    }

    let dataset = BenchmarkDataset {
        methodology_version: "8.1.1-rigorous-paired".to_string(),
        platform: std::env::consts::OS.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        total_iterations: all_iteration_records.len(),
        raw_records: all_iteration_records,
        workload_summaries: workload_summaries.clone(),
    };

    let serialized_dataset = serde_json::to_string_pretty(&dataset).unwrap();
    if let Some(parent) = raw_results_file.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&raw_results_file, &serialized_dataset).expect("failed to write raw results");
    println!(
        "\nWrote {} raw benchmark records to: {}",
        dataset.total_iterations,
        raw_results_file.display()
    );

    // Print Markdown Summary Table
    println!("\n### Summary Benchmark Table");
    println!(
        "| ID | Category | Baseline Calls | Engage Calls | Baseline Wire (Mean ± σ) | Engage Wire (Mean ± σ) | Wire Savings | Baseline Lat (Mean ± σ) | Engage Lat (Mean ± σ) | Latency Savings |"
    );
    println!("|---|---|:---:|:---:|---|---|:---:|---|---|:---:|");
    for s in &workload_summaries {
        println!(
            "| **{}** | `{}` | {} | {} | {:.1} ± {:.1} B | {:.1} ± {:.1} B | **{:.1}%** | {:.1} ± {:.1} ms | {:.1} ± {:.1} ms | **{:.1}%** |",
            s.workload_id,
            s.category,
            s.baseline_calls,
            s.engage_calls,
            s.baseline_wire_bytes.mean,
            s.baseline_wire_bytes.std_dev,
            s.engage_wire_bytes.mean,
            s.engage_wire_bytes.std_dev,
            s.wire_savings_percent_mean,
            s.baseline_duration_ms.mean,
            s.baseline_duration_ms.std_dev,
            s.engage_duration_ms.mean,
            s.engage_duration_ms.std_dev,
            s.latency_savings_percent_mean
        );
    }

    // Verify invariants:
    for s in &workload_summaries {
        assert_eq!(s.iterations, 30);
        assert_eq!(s.baseline_calls, 2);
        assert_eq!(s.engage_calls, 1);
        assert!(
            s.engage_wire_bytes.mean < s.baseline_wire_bytes.mean,
            "Engage wire payload must be strictly smaller"
        );
    }

    let _ = fs::remove_dir_all(&ws);
}
