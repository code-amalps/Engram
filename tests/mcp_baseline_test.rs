use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// RAII helper for managing temporary test workspace and engram mcp process.
struct McpFixture {
    temp_dir: PathBuf,
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<ChildStdout>,
}

impl McpFixture {
    fn new(fixture_name: &str) -> Self {
        let temp_dir = std::env::temp_dir().join(format!(
            "engram_mcp_bl_{}_{}",
            fixture_name,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(temp_dir.join("src")).expect("failed to create temp workspace");
        std::fs::write(
            temp_dir.join("src/service.ts"),
            "export class OrderService {\n  processOrder(id: string): boolean { return true; }\n}\n",
        )
        .expect("failed to write test file");

        // Initialize engram workspace
        let init_status = Command::new(env!("CARGO_BIN_EXE_engram"))
            .arg("-C")
            .arg(&temp_dir)
            .arg("init")
            .status()
            .expect("engram init failed to spawn");
        assert!(init_status.success(), "engram init must succeed");

        // Analyze engram workspace
        let analyze_status = Command::new(env!("CARGO_BIN_EXE_engram"))
            .arg("-C")
            .arg(&temp_dir)
            .arg("analyze")
            .status()
            .expect("engram analyze failed to spawn");
        assert!(analyze_status.success(), "engram analyze must succeed");

        // Spawn engram mcp server over stdio
        let mut child = Command::new(env!("CARGO_BIN_EXE_engram"))
            .arg("-C")
            .arg(&temp_dir)
            .arg("mcp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn engram mcp");

        let stdin = child.stdin.take().expect("failed to open stdin");
        let reader = BufReader::new(child.stdout.take().expect("failed to open stdout"));

        Self {
            temp_dir,
            child,
            stdin,
            reader,
        }
    }

    fn send_raw(&mut self, payload: &str) {
        self.stdin
            .write_all(payload.as_bytes())
            .expect("failed to write to mcp stdin");
        self.stdin.flush().expect("failed to flush mcp stdin");
    }

    fn send_json(&mut self, value: &serde_json::Value) {
        let serialized = serde_json::to_string(value).expect("failed to serialize JSON");
        self.send_raw(&format!("{}\n", serialized));
    }

    fn read_json_response(&mut self) -> serde_json::Value {
        let mut line = String::new();
        let bytes_read = self
            .reader
            .read_line(&mut line)
            .expect("failed to read line from mcp stdout");
        assert!(
            bytes_read > 0,
            "unexpected EOF when reading response from server"
        );
        serde_json::from_str(&line).unwrap_or_else(|e| {
            panic!(
                "failed to parse server line as JSON: '{}'. Error: {}",
                line.trim(),
                e
            )
        })
    }
}

impl Drop for McpFixture {
    fn drop(&mut self) {
        // Terminate child if still running
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.temp_dir);
    }
}

#[test]
fn test_bl_01_standard_handshake() {
    let mut fixture = McpFixture::new("bl01");

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-harness", "version": "1.0.0" }
        }
    });

    fixture.send_json(&req);
    let resp = fixture.read_json_response();

    println!(
        "[BL-01 Actual Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["protocolVersion"], "2024-11-05");
    assert_eq!(resp["result"]["serverInfo"]["name"], "engram");
    assert!(resp["result"]["capabilities"]["tools"].is_object());
    assert!(resp["result"]["capabilities"]["resources"].is_object());
    assert!(resp["result"]["capabilities"]["prompts"].is_object());
}

#[test]
fn test_bl_02_initialized_notification() {
    let mut fixture = McpFixture::new("bl02");

    // Initialize first
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "protocolVersion": "2024-11-05", "capabilities": {} }
    }));
    let _ = fixture.read_json_response();

    // Send initialized notification (no id)
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));

    // Follow immediately with a ping (with id) to verify notification produced NO spurious response line
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "ping"
    }));

    let resp = fixture.read_json_response();
    println!(
        "[BL-02 Ping After Notification]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    // Must be the response to ping (id: 2), proving notification was silently handled
    assert_eq!(resp["id"], 2);
}

#[test]
fn test_bl_03_ping() {
    let mut fixture = McpFixture::new("bl03");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 42,
        "method": "ping"
    }));

    let resp = fixture.read_json_response();
    println!(
        "[BL-03 Actual Ping Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["id"], 42);
    assert!(resp["result"].is_object());
}

#[test]
fn test_bl_04_tools_list() {
    let mut fixture = McpFixture::new("bl04");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 100,
        "method": "tools/list",
        "params": {}
    }));

    let resp = fixture.read_json_response();
    let tools = resp["result"]["tools"]
        .as_array()
        .expect("tools must be array");
    println!("[BL-04 Tool Count]: {}", tools.len());

    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    println!("[BL-04 Tool Names]: {:?}", tool_names);

    // Verify exact 15 approved tools are present
    assert_eq!(tools.len(), 15);
    assert!(tool_names.contains(&"engram_get_context"));
    assert!(tool_names.contains(&"engram_inspect_symbol"));
    assert!(tool_names.contains(&"engram_preflight"));
    assert!(tool_names.contains(&"engram_postflight"));
    assert!(tool_names.contains(&"engram_doctor"));
    assert!(tool_names.contains(&"engram_record_decision"));
    assert!(!tool_names.contains(&"engram_gc"));
}

#[test]
fn test_bl_05_tool_execution_success() {
    let mut fixture = McpFixture::new("bl05");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 201,
        "method": "tools/call",
        "params": {
            "name": "engram_get_context",
            "arguments": {
                "task": "process order",
                "token_budget": 800
            }
        }
    }));

    let resp = fixture.read_json_response();
    println!(
        "[BL-05 Success Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 201);
    assert!(resp["error"].is_null(), "must not be an error");
    assert!(resp["result"]["content"].is_array());
    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap();
    let parsed_context: serde_json::Value =
        serde_json::from_str(content_text).expect("context text must be valid JSON");
    assert_eq!(parsed_context["task"], "process order");
    assert_eq!(resp["result"]["isError"], false);
}

#[test]
fn test_bl_06_tool_execution_failure() {
    let mut fixture = McpFixture::new("bl06");

    // Invoke engram_recover with a non-existent contract_id when no active contract exists
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 202,
        "method": "tools/call",
        "params": {
            "name": "engram_recover",
            "arguments": {
                "contract_id": "non_existent_contract_id_9999"
            }
        }
    }));

    let resp = fixture.read_json_response();
    println!(
        "[BL-06 Tool Failure Actual Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 202);
    // MCP 2024-11-05: tool failures must return isError: true inside result
    assert!(
        resp["error"].is_null(),
        "must not return JSON-RPC protocol error"
    );
    assert_eq!(
        resp["result"]["isError"], true,
        "must return isError: true on tool failure"
    );
    assert!(
        resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("not found")
    );
}

#[test]
fn test_bl_07_unknown_tool_call() {
    let mut fixture = McpFixture::new("bl07");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 203,
        "method": "tools/call",
        "params": {
            "name": "engram_unknown_tool_test",
            "arguments": {}
        }
    }));

    let resp = fixture.read_json_response();
    println!(
        "[BL-07 Unknown Tool Actual Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 203);
    // MCP 2024-11-05: unknown tool must return isError: true inside result
    assert!(
        resp["error"].is_null(),
        "must not return JSON-RPC protocol error"
    );
    assert_eq!(
        resp["result"]["isError"], true,
        "must return isError: true on unknown tool"
    );
    assert!(
        resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Unknown tool")
    );
}

#[test]
fn test_bl_08_unknown_method() {
    let mut fixture = McpFixture::new("bl08");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 204,
        "method": "custom/nonExistentMethod",
        "params": {}
    }));

    let resp = fixture.read_json_response();
    println!(
        "[BL-08 Unknown Method Actual Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 204);
    assert_eq!(
        resp["error"]["code"], -32601,
        "unknown method must return standard JSON-RPC -32601"
    );
    assert!(
        resp["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Method not found")
    );
}

#[test]
fn test_bl_09_malformed_json_syntax() {
    let mut fixture = McpFixture::new("bl09");

    // Send broken JSON followed by newline
    fixture.send_raw("{\"jsonrpc\": \"2.0\", \"id\": 300, \"method\": \n");

    let resp = fixture.read_json_response();
    println!(
        "[BL-09 Malformed JSON Actual Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["jsonrpc"], "2.0");
    assert_eq!(resp["error"]["code"], -32700);
}

#[test]
fn test_bl_10_invalid_json_rpc_structure() {
    let mut fixture = McpFixture::new("bl10");

    // Case A: Missing method but has id: {"id": 310, "hello": "world"}
    fixture.send_raw("{\"id\": 310, \"hello\": \"world\"}\n");
    let resp = fixture.read_json_response();
    println!(
        "[BL-10 With ID Actual Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );
    assert_eq!(resp["id"], 310);
    // JSON-RPC 2.0: invalid request structure returns -32600 Invalid Request
    assert_eq!(resp["error"]["code"], -32600);
    assert!(
        resp["error"]["message"]
            .as_str()
            .unwrap()
            .contains("Invalid Request")
    );

    // Case B: Completely missing id and method: {"hello": "world"}
    // Treated as notification (no response sent).
    // Send a ping immediately after to confirm server didn't crash and pipeline remains active.
    fixture.send_raw("{\"hello\": \"world\"}\n");
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 311,
        "method": "ping"
    }));
    let ping_resp = fixture.read_json_response();
    assert_eq!(ping_resp["id"], 311);
    println!(
        "[BL-10 Ping after notification-like invalid structure succeeded]: {}",
        ping_resp["id"]
    );
}

#[test]
fn test_bl_11_crlf_vs_lf() {
    let mut fixture = McpFixture::new("bl11");

    // Send ping terminated strictly with Windows CRLF (\r\n)
    fixture.send_raw("{\"jsonrpc\": \"2.0\", \"id\": 401, \"method\": \"ping\"}\r\n");
    let resp = fixture.read_json_response();

    assert_eq!(resp["id"], 401);
    println!(
        "[BL-11 CRLF Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );
}

#[test]
fn test_bl_12_pipelined_requests() {
    let mut fixture = McpFixture::new("bl12");

    // Send three requests back-to-back in single write
    let reqs = format!(
        "{}\n{}\n{}\n",
        serde_json::json!({"jsonrpc": "2.0", "id": 501, "method": "ping"}),
        serde_json::json!({"jsonrpc": "2.0", "id": 502, "method": "ping"}),
        serde_json::json!({"jsonrpc": "2.0", "id": 503, "method": "ping"})
    );
    fixture.send_raw(&reqs);

    let resp1 = fixture.read_json_response();
    let resp2 = fixture.read_json_response();
    let resp3 = fixture.read_json_response();

    assert_eq!(resp1["id"], 501);
    assert_eq!(resp2["id"], 502);
    assert_eq!(resp3["id"], 503);
    println!(
        "[BL-12 Pipelined IDs matched]: {}, {}, {}",
        resp1["id"], resp2["id"], resp3["id"]
    );
}

#[test]
fn test_bl_13_eof_client_disconnect() {
    let temp_dir = std::env::temp_dir().join(format!("engram_mcp_bl_eof_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(temp_dir.join("src")).unwrap();
    std::fs::write(temp_dir.join("src/lib.rs"), "pub fn test() {}\n").unwrap();

    let _ = Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("-C")
        .arg(&temp_dir)
        .arg("init")
        .status();

    let mut child = Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("-C")
        .arg(&temp_dir)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn mcp");

    let stdin = child.stdin.take().expect("stdin take");
    // Immediately drop stdin to simulate client disconnect / EOF
    drop(stdin);

    let start = Instant::now();
    let status = child.wait().expect("child wait");
    let elapsed = start.elapsed();

    println!("[BL-13 EOF Exit Status]: {:?} in {:?}", status, elapsed);
    assert!(
        status.success(),
        "Server should exit cleanly with code 0 upon stdin EOF"
    );
    assert!(
        elapsed < Duration::from_secs(3),
        "Server must terminate promptly upon EOF"
    );

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_p1_01_resources_list() {
    let mut fixture = McpFixture::new("p1_01");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 601,
        "method": "resources/list",
        "params": {}
    }));

    let resp = fixture.read_json_response();
    println!(
        "[P1-01 Resources List]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 601);
    let resources = resp["result"]["resources"]
        .as_array()
        .expect("resources array");
    assert_eq!(
        resources.len(),
        4,
        "must expose exactly 4 canonical resources"
    );

    let uris: Vec<&str> = resources
        .iter()
        .map(|r| r["uri"].as_str().unwrap())
        .collect();
    assert!(uris.contains(&"engram://memories"));
    assert!(uris.contains(&"engram://constitution"));
    assert!(uris.contains(&"engram://contract/active"));
    assert!(uris.contains(&"engram://ecosystem"));

    for r in resources {
        let uri = r["uri"].as_str().unwrap();
        if uri == "engram://memories" {
            assert_eq!(r["mimeType"], "text/markdown");
        } else {
            assert_eq!(r["mimeType"], "application/json");
        }
    }
}

#[test]
fn test_p1_02_resources_read_all() {
    let mut fixture = McpFixture::new("p1_02");

    // 1. Read engram://memories (zero-state)
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 602,
        "method": "resources/read",
        "params": { "uri": "engram://memories" }
    }));
    let resp = fixture.read_json_response();
    assert_eq!(resp["id"], 602);
    let mem_text = resp["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(mem_text.contains("# Durable Engineering Memories"));
    assert!(mem_text.contains("No durable memories recorded."));

    // 2. Read engram://constitution
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 603,
        "method": "resources/read",
        "params": { "uri": "engram://constitution" }
    }));
    let resp = fixture.read_json_response();
    assert_eq!(resp["id"], 603);
    assert_eq!(
        resp["result"]["contents"][0]["mimeType"],
        "application/json"
    );

    // 3. Read engram://contract/active (zero-state)
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 604,
        "method": "resources/read",
        "params": { "uri": "engram://contract/active" }
    }));
    let resp = fixture.read_json_response();
    assert_eq!(resp["id"], 604);
    let contract_text = resp["result"]["contents"][0]["text"].as_str().unwrap();
    assert!(contract_text.contains("NO_ACTIVE_CONTRACT"));

    // 4. Read engram://ecosystem
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 605,
        "method": "resources/read",
        "params": { "uri": "engram://ecosystem" }
    }));
    let resp = fixture.read_json_response();
    assert_eq!(resp["id"], 605);
    assert_eq!(
        resp["result"]["contents"][0]["mimeType"],
        "application/json"
    );
}

#[test]
fn test_p1_03_resources_read_unknown_uri() {
    let mut fixture = McpFixture::new("p1_03");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 606,
        "method": "resources/read",
        "params": { "uri": "engram://nonexistent" }
    }));

    let resp = fixture.read_json_response();
    assert_eq!(resp["id"], 606);
    assert_eq!(
        resp["error"]["code"], -32602,
        "unknown resource URI must return -32602 Invalid params"
    );
}

#[test]
fn test_p1_04_prompts_list() {
    let mut fixture = McpFixture::new("p1_04");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 701,
        "method": "prompts/list",
        "params": {}
    }));

    let resp = fixture.read_json_response();
    println!(
        "[P1-04 Prompts List]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 701);
    let prompts = resp["result"]["prompts"].as_array().expect("prompts array");
    assert_eq!(prompts.len(), 2, "must expose exactly 2 approved prompts");

    let prompt_names: Vec<&str> = prompts
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert!(prompt_names.contains(&"engram_preflight_handshake"));
    assert!(prompt_names.contains(&"engram_postflight_audit"));
}

#[test]
fn test_p1_05_prompts_get() {
    let mut fixture = McpFixture::new("p1_05");

    // 1. Get engram_preflight_handshake
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 702,
        "method": "prompts/get",
        "params": {
            "name": "engram_preflight_handshake",
            "arguments": { "task": "Add multi-currency billing" }
        }
    }));
    let resp = fixture.read_json_response();
    assert_eq!(resp["id"], 702);
    let msg = resp["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap();
    assert!(msg.contains("Add multi-currency billing"));
    assert!(msg.contains("engram_preflight"));

    // 2. Get engram_postflight_audit
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 703,
        "method": "prompts/get",
        "params": {
            "name": "engram_postflight_audit",
            "arguments": { "contract_id": "contract_abc_999" }
        }
    }));
    let resp = fixture.read_json_response();
    assert_eq!(resp["id"], 703);
    let msg = resp["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap();
    assert!(msg.contains("contract_abc_999"));
    assert!(msg.contains("engram_postflight"));
}

#[test]
fn test_p1_06_prompts_get_unknown() {
    let mut fixture = McpFixture::new("p1_06");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 704,
        "method": "prompts/get",
        "params": { "name": "unknown_prompt_test" }
    }));

    let resp = fixture.read_json_response();
    assert_eq!(resp["id"], 704);
    assert_eq!(
        resp["error"]["code"], -32602,
        "unknown prompt name must return -32602 Invalid params"
    );
}

#[test]
fn test_p1_07_path_traversal_rejected() {
    let mut fixture = McpFixture::new("p1_07");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 801,
        "method": "tools/call",
        "params": {
            "name": "engram_get_cochanges",
            "arguments": {
                "file_path": "../../secret_config.json"
            }
        }
    }));

    let resp = fixture.read_json_response();
    println!(
        "[P1-07 Path Traversal Actual]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 801);
    assert!(
        resp["error"].is_null(),
        "must return tool error, not protocol error"
    );
    assert_eq!(resp["result"]["isError"], true);
    let err_msg = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(err_msg.contains("prohibited") || err_msg.contains("escapes"));
}

#[test]
fn test_p1_08_absolute_external_path_rejected() {
    let mut fixture = McpFixture::new("p1_08");

    let external_path = if cfg!(windows) {
        "C:\\Windows\\System32\\drivers\\etc\\hosts"
    } else {
        "/etc/passwd"
    };

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 802,
        "method": "tools/call",
        "params": {
            "name": "engram_get_memory",
            "arguments": {
                "files": [external_path]
            }
        }
    }));

    let resp = fixture.read_json_response();
    println!(
        "[P1-08 External Path Actual]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 802);
    assert_eq!(resp["result"]["isError"], true);
    let err_msg = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(err_msg.contains("escapes") || err_msg.contains("outside"));
}

#[test]
fn test_p1_09_null_bytes_rejected() {
    let mut fixture = McpFixture::new("p1_09");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 803,
        "method": "tools/call",
        "params": {
            "name": "engram_get_cochanges",
            "arguments": {
                "file_path": "src/service\0.ts"
            }
        }
    }));

    let resp = fixture.read_json_response();
    assert_eq!(resp["id"], 803);
    assert_eq!(resp["result"]["isError"], true);
    let err_msg = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(err_msg.contains("null byte"));
}

#[test]
fn test_p1_10_governed_provisional_decision() {
    let mut fixture = McpFixture::new("p1_10");

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 804,
        "method": "tools/call",
        "params": {
            "name": "engram_record_decision",
            "arguments": {
                "title": "Use HMAC for Webhooks",
                "statement": "All incoming webhooks must be verified using HMAC-SHA256 signatures.",
                "reason": "Prevent replay attacks and impersonation.",
                "modules": ["webhooks"],
                "accepted_by": "ai_agent"
            }
        }
    }));

    let resp = fixture.read_json_response();
    println!(
        "[P1-10 Decision Actual]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 804);
    assert_eq!(resp["result"]["isError"], false);
    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap();
    let mem: serde_json::Value = serde_json::from_str(content_text).unwrap();

    // Verify PROVISIONAL state is strictly assigned
    assert_eq!(mem["lifecycle_state"], "PROVISIONAL");
    assert!(
        mem["ratified_by"].is_null(),
        "ratified_by must be null until human CLI ratification"
    );
    assert!(
        mem["ratified_at"].is_null(),
        "ratified_at must be null until human CLI ratification"
    );

    // Also inspect disk to ensure storage file reflects PROVISIONAL
    let yaml_path = fixture.temp_dir.join(".engram/memory.yaml");
    let content = std::fs::read_to_string(&yaml_path).expect("memory.yaml must exist");
    assert!(content.contains("PROVISIONAL"));
}

#[test]
fn test_p1_11_sibling_prefix_path_rejected() {
    let mut fixture = McpFixture::new("p1_11");

    // Construct a sibling path that shares the exact same string prefix as the workspace root:
    // e.g. root = "C:\Temp\engram_mcp_bl_p1_11_uuid"
    // sibling = "C:\Temp\engram_mcp_bl_p1_11_uuid_evil\secret.txt"
    let sibling_path = format!(
        "{}_evil/secret.txt",
        fixture.temp_dir.to_string_lossy().replace('\\', "/")
    );

    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 811,
        "method": "tools/call",
        "params": {
            "name": "engram_get_cochanges",
            "arguments": {
                "file_path": sibling_path
            }
        }
    }));

    let resp = fixture.read_json_response();
    println!(
        "[P1-11 Sibling Prefix Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 811);
    assert!(
        resp["error"].is_null(),
        "must be tool result error, not JSON-RPC protocol error"
    );
    assert_eq!(
        resp["result"]["isError"], true,
        "sibling prefix path must be rejected"
    );
    let err_msg = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        err_msg.contains("outside workspace root") || err_msg.contains("escapes workspace root"),
        "error must explicitly indicate containment escape, got: {}",
        err_msg
    );
}

#[test]
fn test_p1_12_nonexistent_target_in_workspace_accepted() {
    let mut fixture = McpFixture::new("p1_12");

    // "src/future_feature.ts" does not exist, but its parent directory "src" exists in the workspace.
    // It should be accepted by containment validation without triggering an escape error,
    // allowing provisional decisions to reference upcoming/planned in-workspace files.
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 812,
        "method": "tools/call",
        "params": {
            "name": "engram_record_decision",
            "arguments": {
                "title": "Add Future Feature",
                "statement": "Create future feature module",
                "reason": "Feature expansion roadmap",
                "files": ["src/future_feature.ts"]
            }
        }
    }));

    let resp = fixture.read_json_response();
    println!(
        "[P1-12 Nonexistent Target Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 812);
    // Non-existent target file in a valid workspace path passes path containment validation and records decision
    assert_eq!(resp["result"]["isError"], false);
    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(content_text.contains("future_feature.ts"));
}

#[test]
fn test_p1_13_preflight_forbidden_validation() {
    let mut fixture = McpFixture::new("p1_13");

    // Case A: Traversal in forbidden boundary -> rejected with isError: true
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8131,
        "method": "tools/call",
        "params": {
            "name": "engram_preflight",
            "arguments": {
                "task": "Add auth",
                "forbidden": ["../../etc/shadow"]
            }
        }
    }));
    let resp1 = fixture.read_json_response();
    assert_eq!(resp1["id"], 8131);
    assert_eq!(resp1["result"]["isError"], true);
    let msg1 = resp1["result"]["content"][0]["text"].as_str().unwrap();
    assert!(msg1.contains("prohibited") || msg1.contains("traversal"));

    // Case B: External absolute path in forbidden boundary -> rejected with isError: true
    let external_abs = if cfg!(windows) {
        "C:\\Windows\\System32\\cmd.exe"
    } else {
        "/etc/passwd"
    };
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8132,
        "method": "tools/call",
        "params": {
            "name": "engram_preflight",
            "arguments": {
                "task": "Add auth",
                "forbidden": [external_abs]
            }
        }
    }));
    let resp2 = fixture.read_json_response();
    assert_eq!(resp2["id"], 8132);
    assert_eq!(resp2["result"]["isError"], true);
    let msg2 = resp2["result"]["content"][0]["text"].as_str().unwrap();
    assert!(msg2.contains("outside workspace root"));

    // Case C: Legitimate in-workspace relative forbidden boundary -> accepted with isError: false
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8133,
        "method": "tools/call",
        "params": {
            "name": "engram_preflight",
            "arguments": {
                "task": "Add auth",
                "forbidden": ["internal/secrets", "config/*.env"]
            }
        }
    }));
    let resp3 = fixture.read_json_response();
    assert_eq!(resp3["id"], 8133);
    assert_eq!(resp3["result"]["isError"], false);
    let content_text = resp3["result"]["content"][0]["text"].as_str().unwrap();
    assert!(content_text.contains("contract_id"));
}

#[test]
fn test_p1_14_symlink_escape_rejected() {
    let mut fixture = McpFixture::new("p1_14");

    // Create an external secret file outside workspace
    let external_dir =
        std::env::temp_dir().join(format!("engram_ext_sec_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&external_dir).unwrap();
    let external_secret = external_dir.join("secret_data.txt");
    std::fs::write(&external_secret, "CONFIDENTIAL_KEY=12345\n").unwrap();

    let link_path = fixture.temp_dir.join("src/external_symlink.txt");

    // Attempt creating symlink or directory junction
    #[cfg(windows)]
    let (escape_created, link_rel) = {
        if std::os::windows::fs::symlink_file(&external_secret, &link_path).is_ok() {
            (true, "src/external_symlink.txt".to_string())
        } else {
            // Unprivileged Windows environments cannot create file symlinks without Developer Mode / elevation,
            // but can create directory junctions via mklink /J without elevation.
            let junc_link = fixture.temp_dir.join("src").join("external_junction");
            let junc_str = junc_link.to_string_lossy();
            let clean_junc_raw = if junc_str.starts_with(r"\\?\") {
                &junc_str[4..]
            } else {
                &junc_str
            };
            let clean_junc = clean_junc_raw.replace('/', "\\");
            let ext_str = external_dir.to_string_lossy();
            let clean_ext_raw = if ext_str.starts_with(r"\\?\") {
                &ext_str[4..]
            } else {
                &ext_str
            };
            let clean_ext = clean_ext_raw.replace('/', "\\");
            let status = std::process::Command::new("cmd")
                .args(["/c", "mklink", "/J", &clean_junc, &clean_ext])
                .status();
            if status.map(|s| s.success()).unwrap_or(false) {
                (true, "src/external_junction/secret_data.txt".to_string())
            } else {
                (false, String::new())
            }
        }
    };
    #[cfg(unix)]
    let (escape_created, link_rel) = {
        let res = std::os::unix::fs::symlink(&external_secret, &link_path);
        assert!(
            res.is_ok(),
            "Symlink creation failed on Unix runner ({:?}). Security test must fail closed.",
            res.err()
        );
        (true, "src/external_symlink.txt".to_string())
    };

    #[cfg(windows)]
    if !escape_created {
        println!(
            "[P1-14 NOTICE]: Neither symlink nor junction creation permitted by OS on this Windows runner. Gating test gracefully."
        );
        let _ = std::fs::remove_dir_all(&external_dir);
        return;
    }

    // Call engram_get_cochanges on the symlink/junction escape
    fixture.send_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": 814,
        "method": "tools/call",
        "params": {
            "name": "engram_get_cochanges",
            "arguments": {
                "file_path": link_rel
            }
        }
    }));

    let resp = fixture.read_json_response();
    println!(
        "[P1-14 Symlink Escape Response]: {}",
        serde_json::to_string_pretty(&resp).unwrap()
    );

    assert_eq!(resp["id"], 814);
    assert_eq!(
        resp["result"]["isError"], true,
        "symlink/junction pointing outside workspace root must be rejected"
    );
    let err_msg = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        err_msg.contains("escapes workspace root"),
        "Error must report workspace root escape, got: {}",
        err_msg
    );

    #[cfg(windows)]
    {
        let junc_link = fixture.temp_dir.join("src").join("external_junction");
        let junc_str = junc_link.to_string_lossy();
        let clean_junc_raw = if junc_str.starts_with(r"\\?\") {
            &junc_str[4..]
        } else {
            &junc_str
        };
        let clean_junc = clean_junc_raw.replace('/', "\\");
        if junc_link.exists() {
            let _ = std::process::Command::new("cmd")
                .args(["/c", "rmdir", &clean_junc])
                .status();
        }
    }
    let _ = std::fs::remove_dir_all(&external_dir);
}
