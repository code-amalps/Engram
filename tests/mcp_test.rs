use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

#[test]
fn test_mcp_protocol_handshake_and_tools() {
    let repo = std::env::temp_dir().join(format!("engram_mcp_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::write(
        repo.join("src/payment.py"),
        "class PaymentService:\n    def charge(self, amount: float):\n        return True\n",
    )
    .unwrap();

    // First init and analyze repo
    let status = Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("-C")
        .arg(&repo)
        .arg("init")
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("-C")
        .arg(&repo)
        .arg("analyze")
        .status()
        .unwrap();
    assert!(status.success());

    // Spawn engram mcp process
    let mut child = Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("-C")
        .arg(&repo)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn engram mcp");

    let mut stdin = child.stdin.take().expect("failed to open stdin");
    let mut reader = BufReader::new(child.stdout.take().expect("failed to open stdout"));

    // 1. Send initialize
    let init_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "test-client", "version": "1.0.0" }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init_req).unwrap()).unwrap();
    stdin.flush().unwrap();

    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["id"], 1);
    assert_eq!(resp["result"]["serverInfo"]["name"], "engram");

    // 2. Send tools/list
    line.clear();
    let tools_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/list",
        "params": {}
    });
    writeln!(stdin, "{}", serde_json::to_string(&tools_req).unwrap()).unwrap();
    stdin.flush().unwrap();

    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["id"], 2);
    let tools = resp["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 14);
    assert!(tools.iter().any(|t| t["name"] == "engram_get_context"));
    assert!(tools.iter().any(|t| t["name"] == "engram_preflight"));
    assert!(tools.iter().any(|t| t["name"] == "engram_postflight"));
    assert!(tools.iter().any(|t| t["name"] == "engram_record_decision"));
    assert!(tools.iter().any(|t| t["name"] == "engram_inspect_symbol"));

    // 3. Send tools/call for engram_get_context
    line.clear();
    let call_req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "engram_get_context",
            "arguments": {
                "task": "charge payment",
                "token_budget": 800
            }
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&call_req).unwrap()).unwrap();
    stdin.flush().unwrap();

    reader.read_line(&mut line).unwrap();
    let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(resp["id"], 3);
    let content_text = resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(content_text.contains("PaymentService"));

    // Gracefully close
    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&repo);
}
