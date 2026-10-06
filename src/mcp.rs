use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    path::Path,
};

use crate::compiler::context;
use crate::guard::{analyze_task, impact, make_contract, record_decision, update_memory, validate};

const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn json_result(v: Value) -> Value {
    json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string())
        }]
    })
}

pub fn mcp_call(root: &Path, name: &str, args: &Value) -> Result<Value> {
    Ok(match name {
        "engram_get_context" => json_result(serde_json::to_value(context(
            root,
            args["task"].as_str().unwrap_or(""),
            args["token_budget"].as_u64().unwrap_or(1200) as usize,
        )?)?),
        "engram_analyze_task" => {
            let (id, ctx) = analyze_task(
                root,
                args["task"].as_str().unwrap_or(""),
                args["token_budget"].as_u64().unwrap_or(1200) as usize,
            )?;
            json_result(json!({"task_id": id, "context": ctx}))
        }
        "engram_create_change_contract" => {
            json_result(make_contract(root, args["task_id"].as_str().unwrap_or(""))?)
        }
        "engram_get_impact" => json_result(impact(root, args["task_id"].as_str().unwrap_or(""))?),
        "engram_validate_change" => {
            json_result(validate(root, args["task_id"].as_str().unwrap_or(""))?)
        }
        "engram_record_decision" => json_result(record_decision(
            root,
            args["task_id"].as_str().unwrap_or(""),
            args["decision"].as_str().unwrap_or(""),
            args["reason"].as_str().unwrap_or(""),
            args["rule_id"].as_str(),
            args["violation_id"].as_str(),
        )?),
        "engram_update_memory" => {
            json_result(update_memory(root, args["task_id"].as_str().unwrap_or(""))?)
        }
        "engram_get_ecosystem" => {
            let report = crate::manifest::detect_ecosystem(root)?;
            json_result(serde_json::to_value(report)?)
        }
        _ => return Err(anyhow!("Unknown tool: {name}")),
    })
}

pub fn mcp(root: &Path) -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                write_message(
                    &mut stdout,
                    &json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": { "code": -32700, "message": e.to_string() }
                    }),
                )?;
                continue;
            }
        };
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req["method"].as_str().unwrap_or("");
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": req["params"]["protocolVersion"].as_str().unwrap_or("2024-11-05"),
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "engram", "version": VERSION }
            })),
            "notifications/initialized" => continue,
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({
                "tools": [
                    {
                        "name": "engram_get_context",
                        "description": "Compile focused, evidence-backed repository context for a task.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task": { "type": "string" },
                                "token_budget": { "type": "integer", "minimum": 0 }
                            },
                            "required": ["task"]
                        }
                    },
                    {
                        "name": "engram_analyze_task",
                        "description": "Ground a task and create a task record for a future contract.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task": { "type": "string" },
                                "token_budget": { "type": "integer" }
                            },
                            "required": ["task"]
                        }
                    },
                    {
                        "name": "engram_create_change_contract",
                        "description": "Create an advisory expected-change boundary for an analyzed task.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": { "type": "string" }
                            },
                            "required": ["task_id"]
                        }
                    },
                    {
                        "name": "engram_get_impact",
                        "description": "Compare repository changes with the task baseline and contract.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": { "type": "string" }
                            },
                            "required": ["task_id"]
                        }
                    },
                    {
                        "name": "engram_validate_change",
                        "description": "Return advisory scope and architecture validation findings.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": { "type": "string" }
                            },
                            "required": ["task_id"]
                        }
                    },
                    {
                        "name": "engram_record_decision",
                        "description": "Record a developer decision and rationale for the current or specified task.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": { "type": "string", "description": "Optional; defaults to the most recently analyzed task." },
                                "decision": { "type": "string" },
                                "reason": { "type": "string" }
                            },
                            "required": ["decision", "reason"]
                        }
                    },
                    {
                        "name": "engram_update_memory",
                        "description": "Promote accepted task knowledge into local engineering memory.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": { "type": "string" }
                            },
                            "required": ["task_id"]
                        }
                    },
                    {
                        "name": "engram_get_ecosystem",
                        "description": "Discover declared and resolved packages, frameworks, and scripts in the project.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    }
                ]
            })),
            "tools/call" => mcp_call(
                root,
                req["params"]["name"].as_str().unwrap_or(""),
                &req["params"]["arguments"],
            ),
            _ => Err(anyhow!("Unknown MCP method: {method}")),
        };
        let response = match result {
            Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": v }),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32000, "message": e.to_string() }
            }),
        };
        if id != Value::Null {
            write_message(&mut stdout, &response)?;
        }
    }
    Ok(())
}

pub fn write_message(out: &mut impl Write, v: &Value) -> Result<()> {
    serde_json::to_writer(&mut *out, v)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}
