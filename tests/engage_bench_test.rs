//! Phase 8.1.1.2: Live Stdio JSON-RPC Benchmark Harness & Rigorous Methodology
//!
//! Evaluates `engram_engage` against the two-call baseline (`engram_get_context` + `engram_preflight`)
//! using actual OS-pipe stdio JSON-RPC 2.0 communication against a real spawned `engram mcp` server process.
//!
//! Experimental Protocol:
//! 1. Category 1: Equivalent-Work Comparison (30 paired runs per workload, alternating order, state reset):
//!    - EQ-01: Single-File Refactor ("Refactor token validation in src/auth.rs")
//!    - EQ-02: Multi-File Feature ("Extend User model and update auth in src/auth.rs and src/user.rs")
//!    - EQ-03: Subsystem Impact Query ("Analyze security policies in src/auth.rs")
//!    - EQ-04: Constitutional Boundary Check ("Update credentials in config/secrets.json")
//!
//! 2. Category 2: Trivial-Task Bypass (30 paired runs, measured separately as avoided work):
//!    - BYPASS-01: Trivial Documentation Leaf ("Fix typo in README.md")
//!
//! Generates raw reproducible dataset at `docs/phases/phase_8_1_1_2_raw_results.json`.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
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
    pub paired_wire_savings_percent: f64,
    pub paired_latency_savings_percent: f64,
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
    pub mean_paired_wire_savings_percent: f64,
    pub aggregate_mean_wire_change_percent: f64,
    pub mean_paired_latency_savings_percent: f64,
    pub aggregate_mean_latency_change_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkDataset {
    pub methodology_version: String,
    pub platform: String,
    pub transport: String,
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

/// Live stdio JSON-RPC 2.0 client that communicates with a real spawned `engram mcp` child process over OS pipes.
pub struct LiveMcpWireClient {
    child: Child,
    stdin: std::process::ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl LiveMcpWireClient {
    pub fn spawn(workspace_root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_engram"))
            .arg("-C")
            .arg(workspace_root)
            .arg("mcp")
            .env("ENGRAM_SPECIALIST_TOOLS", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to spawn engram mcp server binary");

        let stdin = child.stdin.take().expect("Child stdin unavailable");
        let reader = BufReader::new(child.stdout.take().expect("Child stdout unavailable"));

        let mut client = Self {
            child,
            stdin,
            reader,
            next_id: 1,
        };

        // Initialize handshake
        client.call_raw(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "live-stdio-bench", "version": "1.6.0" }
            }),
        );

        client
    }

    pub fn call_raw(&mut self, method: &str, params: Value) -> (usize, usize, Value, f64) {
        assert!(
            self.child.try_wait().unwrap().is_none(),
            "MCP server process terminated unexpectedly before call"
        );

        let id = self.next_id;
        self.next_id += 1;

        let req_obj = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        let serialized_req = serde_json::to_string(&req_obj).expect("Failed to serialize request");
        let wire_req = format!("{}\n", serialized_req);
        let req_bytes = wire_req.as_bytes().len();

        let t0 = Instant::now();
        self.stdin
            .write_all(wire_req.as_bytes())
            .expect("Failed to write to child stdin");
        self.stdin.flush().expect("Failed to flush child stdin");

        let mut line = String::new();
        let resp_bytes = self
            .reader
            .read_line(&mut line)
            .expect("Failed to read line from child stdout");
        let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;

        assert!(
            resp_bytes > 0,
            "MCP server closed stdout unexpectedly (EOF)"
        );

        let resp_val: Value =
            serde_json::from_str(&line).expect("Invalid JSON-RPC response from MCP server");

        // Validate JSON-RPC ID
        assert_eq!(
            resp_val.get("id").and_then(|v| v.as_u64()),
            Some(id),
            "Mismatched JSON-RPC response ID (expected {}, got {:?})",
            id,
            resp_val.get("id")
        );

        // Validate that server did not return a JSON-RPC error
        assert!(
            resp_val.get("error").is_none(),
            "MCP server returned JSON-RPC error: {:?}",
            resp_val.get("error")
        );

        (req_bytes, resp_bytes, resp_val, duration_ms)
    }

    pub fn call_tool(&mut self, tool_name: &str, arguments: &Value) -> WireCallRecord {
        let args_bytes = serde_json::to_string(arguments).unwrap().as_bytes().len();

        let (req_bytes, resp_bytes, resp_val, duration_ms) = self.call_raw(
            "tools/call",
            json!({
                "name": tool_name,
                "arguments": arguments
            }),
        );

        let result_obj = resp_val
            .get("result")
            .expect("Missing result in JSON-RPC response");
        let result_bytes = serde_json::to_string(result_obj).unwrap().as_bytes().len();

        let status = if let Some(content_text) = result_obj["content"][0]["text"].as_str() {
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
            request_wire_bytes: req_bytes,
            response_wire_bytes: resp_bytes,
            total_wire_bytes: req_bytes + resp_bytes,
            arguments_bytes: args_bytes,
            result_bytes,
            duration_ms,
            status,
        }
    }
}

impl Drop for LiveMcpWireClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
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
        "engram_p8112_bench_{}_{}",
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
fn test_rigorous_30_run_live_stdio_benchmark() {
    let ws = setup_benchmark_fixture("live_stdio_harness");
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let raw_results_file = manifest_dir.join("docs/phases/phase_8_1_1_2_raw_results.json");

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
    println!("EXECUTING LIVE STDIO OS-PIPE 30-RUN BENCHMARK (150 PAIRED RUNS / 300 EXECUTIONS)");
    println!(
        "=========================================================================================="
    );

    for w in &workloads {
        println!(
            "\nExecuting Workload: {} [{}] - {}",
            w.id, w.category, w.description
        );

        // Spawn a dedicated LiveMcpWireClient connected via stdio OS pipes to `engram mcp`
        let mut client = LiveMcpWireClient::spawn(&ws);

        let mut baseline_wire_bytes_list = Vec::new();
        let mut engage_wire_bytes_list = Vec::new();
        let mut baseline_durations_list = Vec::new();
        let mut engage_durations_list = Vec::new();
        let mut paired_wire_savings_list = Vec::new();
        let mut paired_latency_savings_list = Vec::new();

        for i in 1..=num_iterations {
            let order = if i % 2 == 1 {
                "baseline_then_engage"
            } else {
                "engage_then_baseline"
            };

            let mut b_calls = Vec::new();
            let mut e_calls = Vec::new();

            if order == "baseline_then_engage" {
                // 1. Run Baseline Workflow over live stdio OS pipe
                reset_workspace_contract_state(&ws);
                let c1 = client.call_tool("engram_get_context", &w.baseline_get_context);
                let c2 = client.call_tool("engram_preflight", &w.baseline_preflight);
                b_calls.push(c1);
                b_calls.push(c2);

                // 2. Run Engage Workflow over live stdio OS pipe
                reset_workspace_contract_state(&ws);
                let ce = client.call_tool("engram_engage", &w.engage_args);
                e_calls.push(ce);
            } else {
                // 1. Run Engage Workflow over live stdio OS pipe
                reset_workspace_contract_state(&ws);
                let ce = client.call_tool("engram_engage", &w.engage_args);
                e_calls.push(ce);

                // 2. Run Baseline Workflow over live stdio OS pipe
                reset_workspace_contract_state(&ws);
                let c1 = client.call_tool("engram_get_context", &w.baseline_get_context);
                let c2 = client.call_tool("engram_preflight", &w.baseline_preflight);
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

            let paired_wire_savings = if b_wire > 0 {
                ((b_wire as f64 - e_wire as f64) / b_wire as f64) * 100.0
            } else {
                0.0
            };

            let paired_lat_savings = if b_dur > 0.0 {
                ((b_dur - e_dur) / b_dur) * 100.0
            } else {
                0.0
            };

            baseline_wire_bytes_list.push(b_wire as f64);
            engage_wire_bytes_list.push(e_wire as f64);
            baseline_durations_list.push(b_dur);
            engage_durations_list.push(e_dur);
            paired_wire_savings_list.push(paired_wire_savings);
            paired_latency_savings_list.push(paired_lat_savings);

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
                paired_wire_savings_percent: (paired_wire_savings * 100.0).round() / 100.0,
                paired_latency_savings_percent: (paired_lat_savings * 100.0).round() / 100.0,
            });
        }

        let b_wire_stats = compute_stats(&baseline_wire_bytes_list);
        let e_wire_stats = compute_stats(&engage_wire_bytes_list);
        let b_dur_stats = compute_stats(&baseline_durations_list);
        let e_dur_stats = compute_stats(&engage_durations_list);

        let mean_p_wire =
            (paired_wire_savings_list.iter().sum::<f64>() / num_iterations as f64 * 100.0).round()
                / 100.0;
        let agg_wire_change =
            (((b_wire_stats.mean - e_wire_stats.mean) / b_wire_stats.mean) * 100.0 * 100.0).round()
                / 100.0;

        let mean_p_lat = (paired_latency_savings_list.iter().sum::<f64>() / num_iterations as f64
            * 100.0)
            .round()
            / 100.0;
        let agg_lat_change =
            (((b_dur_stats.mean - e_dur_stats.mean) / b_dur_stats.mean) * 100.0 * 100.0).round()
                / 100.0;

        let summary = WorkloadSummary {
            workload_id: w.id.to_string(),
            category: w.category.to_string(),
            description: w.description.to_string(),
            iterations: num_iterations,
            baseline_calls: 2,
            engage_calls: 1,
            baseline_wire_bytes: b_wire_stats.clone(),
            engage_wire_bytes: e_wire_stats.clone(),
            baseline_duration_ms: b_dur_stats.clone(),
            engage_duration_ms: e_dur_stats.clone(),
            mean_paired_wire_savings_percent: mean_p_wire,
            aggregate_mean_wire_change_percent: agg_wire_change,
            mean_paired_latency_savings_percent: mean_p_lat,
            aggregate_mean_latency_change_percent: agg_lat_change,
        };

        println!(
            "  -> Completed: Baseline Wire: {:.1} B, Engage Wire: {:.1} B (paired: {:.1}%, agg: {:.1}%) | Baseline Dur: {:.1} ms, Engage Dur: {:.1} ms (paired: {:.1}%, agg: {:.1}%)",
            summary.baseline_wire_bytes.mean,
            summary.engage_wire_bytes.mean,
            summary.mean_paired_wire_savings_percent,
            summary.aggregate_mean_wire_change_percent,
            summary.baseline_duration_ms.mean,
            summary.engage_duration_ms.mean,
            summary.mean_paired_latency_savings_percent,
            summary.aggregate_mean_latency_change_percent,
        );

        workload_summaries.push(summary);
    }

    let dataset = BenchmarkDataset {
        methodology_version: "8.1.1.2-live-stdio-paired".to_string(),
        platform: std::env::consts::OS.to_string(),
        transport: "live_os_stdio_pipes".to_string(),
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
    println!("\n### Summary Live Stdio Benchmark Table");
    println!(
        "| ID | Category | Baseline Wire (Mean ± σ) | Engage Wire (Mean ± σ) | Paired Wire Savings | Agg Wire Change | Baseline Lat (Mean ± σ) | Engage Lat (Mean ± σ) | Paired Lat Savings | Agg Lat Change |"
    );
    println!("|---|---|---|---|:---:|:---:|---|---|:---:|:---:|");
    for s in &workload_summaries {
        println!(
            "| **{}** | `{}` | {:.1} ± {:.1} B | {:.1} ± {:.1} B | **{:.1}%** | **{:.1}%** | {:.1} ± {:.1} ms | {:.1} ± {:.1} ms | **{:.1}%** | **{:.1}%** |",
            s.workload_id,
            s.category,
            s.baseline_wire_bytes.mean,
            s.baseline_wire_bytes.std_dev,
            s.engage_wire_bytes.mean,
            s.engage_wire_bytes.std_dev,
            s.mean_paired_wire_savings_percent,
            s.aggregate_mean_wire_change_percent,
            s.baseline_duration_ms.mean,
            s.baseline_duration_ms.std_dev,
            s.engage_duration_ms.mean,
            s.engage_duration_ms.std_dev,
            s.mean_paired_latency_savings_percent,
            s.aggregate_mean_latency_change_percent
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

#[test]
fn test_validate_raw_results_integrity_and_statistics() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join("docs/phases/phase_8_1_1_2_raw_results.json");
    if !path.exists() {
        println!("Note: Raw results file not found yet (will be generated by live benchmark test)");
        return;
    }

    let raw_text = fs::read_to_string(&path).expect("Failed to read raw results");
    let dataset: BenchmarkDataset = serde_json::from_str(&raw_text).expect("Valid JSON dataset");

    // 1. Exactly 150 records overall
    assert_eq!(dataset.total_iterations, 150);
    assert_eq!(dataset.raw_records.len(), 150);

    // 2. Exactly 30 records per workload
    let expected_workloads = ["EQ-01", "EQ-02", "EQ-03", "EQ-04", "BYPASS-01"];
    for wid in &expected_workloads {
        let count = dataset
            .raw_records
            .iter()
            .filter(|r| r.workload_id == *wid)
            .count();
        assert_eq!(count, 30, "Workload {} must have exactly 30 records", wid);
    }

    // 3. No duplicate workload/iteration combinations
    let mut seen = std::collections::HashSet::new();
    for r in &dataset.raw_records {
        assert!(
            seen.insert((r.workload_id.clone(), r.iteration)),
            "Duplicate record found: {} iter {}",
            r.workload_id,
            r.iteration
        );
    }

    // 4. Alternating execution order
    for r in &dataset.raw_records {
        let expected_order = if r.iteration % 2 == 1 {
            "baseline_then_engage"
        } else {
            "engage_then_baseline"
        };
        assert_eq!(r.execution_order, expected_order);
    }

    // 5. Expected tool-call counts
    for r in &dataset.raw_records {
        assert_eq!(r.baseline_total_calls, 2);
        assert_eq!(r.baseline_calls.len(), 2);
        assert_eq!(r.engage_total_calls, 1);
        assert_eq!(r.engage_calls.len(), 1);
    }

    // 6. Wire-byte totals equal request bytes plus response bytes
    for r in &dataset.raw_records {
        assert_eq!(
            r.baseline_wire_bytes,
            r.baseline_req_bytes + r.baseline_resp_bytes
        );
        assert_eq!(
            r.engage_wire_bytes,
            r.engage_req_bytes + r.engage_resp_bytes
        );
        for c in &r.baseline_calls {
            assert_eq!(
                c.total_wire_bytes,
                c.request_wire_bytes + c.response_wire_bytes
            );
        }
        for c in &r.engage_calls {
            assert_eq!(
                c.total_wire_bytes,
                c.request_wire_bytes + c.response_wire_bytes
            );
        }
    }

    // 7. All durations are finite and non-negative
    for r in &dataset.raw_records {
        assert!(
            r.baseline_duration_ms > 0.0 && r.baseline_duration_ms.is_finite(),
            "Baseline duration must be positive finite"
        );
        assert!(
            r.engage_duration_ms > 0.0 && r.engage_duration_ms.is_finite(),
            "Engage duration must be positive finite"
        );
        for c in &r.baseline_calls {
            assert!(
                c.duration_ms > 0.0 && c.duration_ms.is_finite(),
                "Call duration must be positive finite"
            );
        }
        for c in &r.engage_calls {
            assert!(
                c.duration_ms > 0.0 && c.duration_ms.is_finite(),
                "Call duration must be positive finite"
            );
        }
    }

    // 8. All paired percentages can be independently recomputed
    for r in &dataset.raw_records {
        let expected_wire_savings = ((r.baseline_wire_bytes as f64 - r.engage_wire_bytes as f64)
            / r.baseline_wire_bytes as f64)
            * 100.0;
        assert!(
            (r.paired_wire_savings_percent - expected_wire_savings).abs() < 0.1,
            "Paired wire savings mismatch in record {} iter {}",
            r.workload_id,
            r.iteration
        );

        let expected_lat_savings =
            ((r.baseline_duration_ms - r.engage_duration_ms) / r.baseline_duration_ms) * 100.0;
        assert!(
            (r.paired_latency_savings_percent - expected_lat_savings).abs() < 0.1,
            "Paired latency savings mismatch in record {} iter {}",
            r.workload_id,
            r.iteration
        );
    }

    // 9. Summary statistics match raw records within documented rounding tolerance
    for s in &dataset.workload_summaries {
        let records: Vec<&IterationRecord> = dataset
            .raw_records
            .iter()
            .filter(|r| r.workload_id == s.workload_id)
            .collect();
        assert_eq!(records.len(), 30);

        let b_wires: Vec<f64> = records
            .iter()
            .map(|r| r.baseline_wire_bytes as f64)
            .collect();
        let e_wires: Vec<f64> = records.iter().map(|r| r.engage_wire_bytes as f64).collect();
        let b_durs: Vec<f64> = records.iter().map(|r| r.baseline_duration_ms).collect();
        let e_durs: Vec<f64> = records.iter().map(|r| r.engage_duration_ms).collect();
        let p_wires: Vec<f64> = records
            .iter()
            .map(|r| r.paired_wire_savings_percent)
            .collect();
        let p_lats: Vec<f64> = records
            .iter()
            .map(|r| r.paired_latency_savings_percent)
            .collect();

        let expected_b_wire_mean = b_wires.iter().sum::<f64>() / 30.0;
        let expected_e_wire_mean = e_wires.iter().sum::<f64>() / 30.0;
        let expected_b_dur_mean = b_durs.iter().sum::<f64>() / 30.0;
        let expected_e_dur_mean = e_durs.iter().sum::<f64>() / 30.0;
        let expected_p_wire_mean = p_wires.iter().sum::<f64>() / 30.0;
        let expected_p_lat_mean = p_lats.iter().sum::<f64>() / 30.0;

        assert!(
            (s.baseline_wire_bytes.mean - expected_b_wire_mean).abs() < 0.1,
            "Baseline wire mean mismatch"
        );
        assert!(
            (s.engage_wire_bytes.mean - expected_e_wire_mean).abs() < 0.1,
            "Engage wire mean mismatch"
        );
        assert!(
            (s.baseline_duration_ms.mean - expected_b_dur_mean).abs() < 0.1,
            "Baseline duration mean mismatch"
        );
        assert!(
            (s.engage_duration_ms.mean - expected_e_dur_mean).abs() < 0.1,
            "Engage duration mean mismatch"
        );
        assert!(
            (s.mean_paired_wire_savings_percent - expected_p_wire_mean).abs() < 0.1,
            "Mean paired wire savings mismatch"
        );
        assert!(
            (s.mean_paired_latency_savings_percent - expected_p_lat_mean).abs() < 0.1,
            "Mean paired latency savings mismatch"
        );

        let expected_agg_lat =
            ((expected_b_dur_mean - expected_e_dur_mean) / expected_b_dur_mean) * 100.0;
        assert!(
            (s.aggregate_mean_latency_change_percent - expected_agg_lat).abs() < 0.1,
            "Aggregate latency change mismatch"
        );
    }
}
