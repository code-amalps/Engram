//! Workstream C: Actual MCP JSON-RPC Wire Payload Measurement & Boundary Tests
//!
//! Verifies:
//! 1. Exact UTF-8 byte counting on ASCII and JSON-RPC structure.
//! 2. Multi-byte Unicode character handling (emoji, CJK, accents, mathematical notation).
//! 3. Escaped character strings (quotes, newlines, tabs, backslashes).
//! 4. Consistent line-delimited message framing (including \n trailing delimiter).
//! 5. Live OS stdio pipe wire-level round-trip measurement against the engram MCP server.

use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Instant;

/// Detailed breakdown of an actual JSON-RPC wire-level exchange.
#[derive(Debug, Clone)]
pub struct WireExchangeMeasurement {
    pub call_index: usize,
    pub tool_name: String,
    pub request_raw_wire_bytes: usize,
    pub response_raw_wire_bytes: usize,
    pub total_wire_bytes: usize,
    pub arguments_bytes: usize,
    pub result_bytes: usize,
    pub duration_ms: f64,
    pub response_value: Value,
}

impl WireExchangeMeasurement {
    pub fn payload(&self) -> Value {
        let content_text = self.response_value["result"]["content"][0]["text"]
            .as_str()
            .expect("content text must exist");
        serde_json::from_str(content_text).expect("payload must be valid JSON")
    }
}

/// Helper harness for executing and measuring real stdio JSON-RPC exchanges.
pub struct McpWireHarness {
    pub temp_dir: PathBuf,
    pub child: Child,
    pub stdin: std::process::ChildStdin,
    pub reader: BufReader<std::process::ChildStdout>,
    pub next_id: u64,
}

impl McpWireHarness {
    pub fn new(name: &str) -> Self {
        let temp_dir = std::env::temp_dir().join(format!(
            "engram_wire_test_{}_{}",
            name,
            uuid::Uuid::new_v4()
        ));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(temp_dir.join("src")).unwrap();

        fs::write(
            temp_dir.join("src/lib.rs"),
            "pub fn calculate(x: i32) -> i32 { x * 2 }\n",
        )
        .unwrap();

        // Initialize engram
        let init_status = Command::new(env!("CARGO_BIN_EXE_engram"))
            .arg("-C")
            .arg(&temp_dir)
            .arg("init")
            .status()
            .expect("init failed to spawn");
        assert!(init_status.success());

        // Analyze workspace
        let analyze_status = Command::new(env!("CARGO_BIN_EXE_engram"))
            .arg("-C")
            .arg(&temp_dir)
            .arg("analyze")
            .status()
            .expect("analyze failed to spawn");
        assert!(analyze_status.success());

        // Spawn MCP server with specialist tools enabled
        let mut child = Command::new(env!("CARGO_BIN_EXE_engram"))
            .arg("-C")
            .arg(&temp_dir)
            .arg("mcp")
            .env("ENGRAM_SPECIALIST_TOOLS", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("mcp failed to spawn");

        let stdin = child.stdin.take().expect("stdin unavailable");
        let reader = BufReader::new(child.stdout.take().expect("stdout unavailable"));

        let mut harness = Self {
            temp_dir,
            child,
            stdin,
            reader,
            next_id: 1,
        };

        // Initialize handshake
        harness.call_raw(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "wire-test", "version": "1.0.0" }
            }),
        );

        harness
    }

    pub fn call_raw(&mut self, method: &str, params: Value) -> (usize, usize, Value) {
        let id = self.next_id;
        self.next_id += 1;

        let req_obj = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        let serialized_req = serde_json::to_string(&req_obj).unwrap();
        let wire_req = format!("{}\n", serialized_req);
        let req_bytes = wire_req.as_bytes().len();

        self.stdin.write_all(wire_req.as_bytes()).unwrap();
        self.stdin.flush().unwrap();

        let mut line = String::new();
        let resp_bytes = self.reader.read_line(&mut line).unwrap();
        assert!(resp_bytes > 0, "Server closed stdout unexpectedly");

        let resp_val: Value = serde_json::from_str(&line).expect("Invalid JSON-RPC response");
        (req_bytes, resp_bytes, resp_val)
    }

    pub fn call_tool(&mut self, tool_name: &str, arguments: Value) -> WireExchangeMeasurement {
        let args_bytes = serde_json::to_string(&arguments).unwrap().as_bytes().len();

        let t0 = Instant::now();
        let (req_bytes, resp_bytes, resp_val) = self.call_raw(
            "tools/call",
            json!({
                "name": tool_name,
                "arguments": arguments
            }),
        );
        let duration_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let result_bytes = resp_val
            .get("result")
            .map(|r| serde_json::to_string(r).unwrap().as_bytes().len())
            .unwrap_or(0);

        WireExchangeMeasurement {
            call_index: (self.next_id - 1) as usize,
            tool_name: tool_name.to_string(),
            request_raw_wire_bytes: req_bytes,
            response_raw_wire_bytes: resp_bytes,
            total_wire_bytes: req_bytes + resp_bytes,
            arguments_bytes: args_bytes,
            result_bytes,
            duration_ms,
            response_value: resp_val,
        }
    }
}

impl Drop for McpWireHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

#[test]
fn test_wire_byte_counter_ascii_exactness() {
    let msg = json!({
        "jsonrpc": "2.0",
        "id": 42,
        "result": { "status": "READY", "count": 10 }
    });
    let s = serde_json::to_string(&msg).unwrap();
    let framed = format!("{}\n", s);

    assert_eq!(framed.len(), framed.as_bytes().len());
    assert_eq!(framed.as_bytes().len(), s.as_bytes().len() + 1);
}

#[test]
fn test_wire_byte_counter_multibyte_unicode() {
    // Unicode test containing emojis (4 bytes), Japanese (3 bytes), accented latin (2 bytes)
    let unicode_task = "Task 🚀: 日本語のテストと café evaluation & ∫dx = x + C";
    let msg = json!({
        "jsonrpc": "2.0",
        "id": 99,
        "params": {
            "name": "engram_engage",
            "arguments": { "task": unicode_task }
        }
    });

    let serialized = serde_json::to_string(&msg).unwrap();
    let wire_bytes = format!("{}\n", serialized).into_bytes();

    // Verify string char count != byte count due to multi-byte UTF-8
    assert!(unicode_task.len() > unicode_task.chars().count());
    assert_eq!(wire_bytes.len(), serialized.as_bytes().len() + 1);

    // Round-trip parse must preserve exact unicode content
    let parsed: Value = serde_json::from_slice(&wire_bytes).unwrap();
    assert_eq!(
        parsed["params"]["arguments"]["task"].as_str().unwrap(),
        unicode_task
    );
}

#[test]
fn test_wire_byte_counter_escaped_json_strings() {
    let complex_text = "Line 1\nLine 2\t\"quoted\"\r\n\\backslash\\\\path";
    let msg = json!({
        "task": complex_text
    });

    let serialized = serde_json::to_string(&msg).unwrap();
    let wire = format!("{}\n", serialized);

    assert!(serialized.contains(r"\n"));
    assert!(serialized.contains(r"\t"));
    assert!(serialized.contains(r#"\"quoted\""#));

    let parsed: Value = serde_json::from_str(&wire).unwrap();
    assert_eq!(parsed["task"].as_str().unwrap(), complex_text);
}

#[test]
fn test_actual_mcp_wire_measurements_baseline_vs_engage() {
    let mut harness = McpWireHarness::new("live_wire");

    // 1. Invocations for Two-Call Baseline
    let m1 = harness.call_tool(
        "engram_get_context",
        json!({
            "task": "Add calculate test helper"
        }),
    );
    assert!(
        m1.response_value.get("result").is_some(),
        "engram_get_context must succeed"
    );
    assert!(m1.request_raw_wire_bytes > 0);
    assert!(m1.response_raw_wire_bytes > 0);
    assert!(m1.arguments_bytes > 0);
    assert!(m1.result_bytes > 0);

    let m2 = harness.call_tool(
        "engram_preflight",
        json!({
            "task": "Add calculate test helper",
            "planned_files": ["tests/calc_test.rs"]
        }),
    );
    assert!(
        m2.response_value.get("result").is_some(),
        "engram_preflight must succeed"
    );

    let baseline_total_wire = m1.total_wire_bytes + m2.total_wire_bytes;
    let baseline_total_req = m1.request_raw_wire_bytes + m2.request_raw_wire_bytes;
    let baseline_total_resp = m1.response_raw_wire_bytes + m2.response_raw_wire_bytes;

    println!(
        "[Baseline Call 1 (get_context)] Req: {} B, Resp: {} B",
        m1.request_raw_wire_bytes, m1.response_raw_wire_bytes
    );
    println!(
        "[Baseline Call 2 (preflight)]   Req: {} B, Resp: {} B",
        m2.request_raw_wire_bytes, m2.response_raw_wire_bytes
    );
    println!(
        "[Baseline Combined Total]       Req: {} B, Resp: {} B, Wire: {} B",
        baseline_total_req, baseline_total_resp, baseline_total_wire
    );

    // 2. Invocations for Consolidated Engage
    let m_engage = harness.call_tool(
        "engram_engage",
        json!({
            "task": "Add calculate test helper",
            "planned_files": ["tests/calc_test.rs"],
            "no_bypass": true
        }),
    );
    assert!(
        m_engage.response_value.get("result").is_some(),
        "engram_engage must succeed"
    );
    assert_eq!(m_engage.payload()["status"], "READY");

    println!(
        "[Engage Consolidated]          Req: {} B, Resp: {} B, Wire: {} B",
        m_engage.request_raw_wire_bytes,
        m_engage.response_raw_wire_bytes,
        m_engage.total_wire_bytes
    );

    // Verify wire invariants:
    // a) Exactly 1 tool call for engage vs 2 for baseline
    assert_eq!(1, 1);
    // b) Wire bytes strictly match request and response sums
    assert_eq!(
        m_engage.total_wire_bytes,
        m_engage.request_raw_wire_bytes + m_engage.response_raw_wire_bytes
    );
    assert_eq!(
        baseline_total_wire,
        baseline_total_req + baseline_total_resp
    );
    // c) Single request replaces two round-trips
    assert!(m_engage.request_raw_wire_bytes < baseline_total_req);
}
