//! MCP Server implementation: stdio JSON-RPC 2.0 protocol exposing engineering intelligence.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    path::Path,
};

use crate::compiler::context;
use crate::constitution::evaluate_rules;
use crate::contract::create_contract;
use crate::git_history::get_cochanges_for_file;
use crate::guard::{analyze_task, impact, validate};
use crate::memory::{load_durable_memories, query_relevant_memories, record_decision};
use crate::storage::{db, root_path};

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
    let root = root_path(root)?;
    Ok(match name {
        "engram_get_context" => {
            let task = args["task"].as_str().unwrap_or("");
            let budget = args["token_budget"].as_u64().unwrap_or(1200) as usize;
            json_result(serde_json::to_value(context(&root, task, budget)?)?)
        }
        "engram_analyze_task" => {
            let task = args["task"].as_str().unwrap_or("");
            let budget = args["token_budget"].as_u64().unwrap_or(1200) as usize;
            let (id, ctx) = analyze_task(&root, task, budget)?;
            json_result(json!({"task_id": id, "context": ctx}))
        }
        "engram_create_change_contract" => {
            let task = args["task"]
                .as_str()
                .or_else(|| args["task_id"].as_str())
                .unwrap_or("");

            let modules = args["expected_modules"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect());
            let files = args["expected_files"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect());
            let forbidden = args["forbidden"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect());

            let contract = create_contract(&root, task, modules, files, forbidden)?;
            json_result(serde_json::to_value(contract)?)
        }
        "engram_validate_change" => {
            let id = args["contract_id"]
                .as_str()
                .or_else(|| args["task_id"].as_str())
                .unwrap_or("");
            json_result(validate(&root, id)?)
        }
        "engram_get_impact" => {
            let target = args["target"]
                .as_str()
                .or_else(|| args["task_id"].as_str())
                .or_else(|| args["symbol"].as_str())
                .unwrap_or("");
            json_result(impact(&root, target)?)
        }
        "engram_inspect_symbol" => {
            let target = args["target"]
                .as_str()
                .or_else(|| args["symbol"].as_str())
                .unwrap_or("");
            let depth = args["depth"].as_u64().unwrap_or(2) as usize;

            crate::analyzer::ensure_fresh(&root)?;
            let conn = db(&root)?;
            let cone = crate::impact::compute_impact_cone(&conn, &root, target, depth)?;
            let active_contract = crate::contract::get_active_contract(&root).ok().flatten();
            let authorization = active_contract.as_ref().map(|c| {
                crate::contract::evaluate_impact_against_contract(c, &cone)
            });
            let explanation = crate::impact::format_impact_explanation(&cone);
            let affected_paths = cone.all_affected_paths();
            let audit_memories = crate::memory::query_scoped_memories(&root, target, &affected_paths, &[target.to_string()], true).unwrap_or_default();
            let memory_conflicts = crate::memory::detect_memory_conflicts(&audit_memories);

            json_result(json!({
                "target": cone.target_symbol,
                "target_file": cone.target_file,
                "impact_cone": cone,
                "active_contract": active_contract,
                "authorization": authorization,
                "explanation": explanation,
                "memories": audit_memories,
                "memory_conflicts": memory_conflicts,
            }))
        }
        "engram_get_cochanges" => {
            let file_path = args["file_path"].as_str().unwrap_or("");
            let min_prob = args["min_probability"].as_f64().unwrap_or(0.25);
            let signals = get_cochanges_for_file(&root, file_path, min_prob)?;
            json_result(serde_json::to_value(signals)?)
        }
        "engram_get_memory" => {
            let query = args["query"].as_str().unwrap_or("");
            let files: Vec<String> = args["files"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();

            if query.is_empty() && files.is_empty() {
                let doc = load_durable_memories(&root)?;
                json_result(serde_json::to_value(doc.memories)?)
            } else {
                let memories = query_relevant_memories(&root, query, &files)?;
                json_result(serde_json::to_value(memories)?)
            }
        }
        "engram_record_decision" => {
            let title = args["title"]
                .as_str()
                .or_else(|| args["decision"].as_str())
                .unwrap_or("Decision");
            let statement = args["statement"].as_str().unwrap_or(title);
            let reason = args["reason"].as_str().unwrap_or("");
            let actor = args["accepted_by"].as_str().unwrap_or("developer");

            let modules: Vec<String> = args["modules"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let files: Vec<String> = args["files"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();

            let memory = record_decision(&root, title, statement, reason, modules, files, None, actor)?;
            json_result(serde_json::to_value(memory)?)
        }
        "engram_check_constitution" => {
            let conn = db(&root)?;
            let changed_files: Vec<String> = args["files"]
                .as_array()
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_else(|| crate::storage::git_changed(&root));
            let violations = evaluate_rules(&root, &changed_files, &conn)?;
            json_result(serde_json::to_value(violations)?)
        }
        "engram_get_ecosystem" => {
            let report = crate::manifest::detect_ecosystem(&root)?;
            json_result(serde_json::to_value(report)?)
        }
        "engram_why" => {
            let target = args["target"]
                .as_str()
                .or_else(|| args["symbol"].as_str())
                .unwrap_or("");
            crate::analyzer::ensure_fresh(&root)?;
            let expl = crate::archaeology::explain_why(&root, target)?;
            json_result(serde_json::to_value(expl)?)
        }
        "engram_doctor" => {
            let report = crate::doctor::diagnose_memory_health(&root)?;
            json_result(serde_json::to_value(report)?)
        }
        "engram_gc" => {
            let dry_run = args["dry_run"].as_bool().unwrap_or(false);
            let candidates = args["candidates"].as_bool().unwrap_or(false);
            let archive = args["archive"].as_bool().unwrap_or(false);
            let vacuum = args["vacuum"].as_bool().unwrap_or(false);
            let opts = crate::gc::GcOptions {
                dry_run,
                prune_candidates: candidates || (!archive && !vacuum),
                archive_superseded: archive || (!candidates && !vacuum),
                vacuum: vacuum || (!candidates && !archive),
            };
            let result = crate::gc::execute_gc(&root, &opts)?;
            json_result(serde_json::to_value(result)?)
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
                        "description": "Compile focused, evidence-backed context with durable memories and co-change alerts.",
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
                        "description": "Ground a task and analyze ambiguity gaps.",
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
                        "description": "Create an elastic Change Contract before editing. Auto-grounds scope if fields are omitted.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task": { "type": "string" },
                                "expected_modules": { "type": "array", "items": { "type": "string" } },
                                "expected_files": { "type": "array", "items": { "type": "string" } },
                                "forbidden": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["task"]
                        }
                    },
                    {
                        "name": "engram_validate_change",
                        "description": "Validate changes against active Change Contract boundaries and constitution rules.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "contract_id": { "type": "string" }
                            }
                        }
                    },
                    {
                        "name": "engram_get_impact",
                        "description": "Query reverse dependencies of a symbol or evaluate overall change impact.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "target": { "type": "string" }
                            },
                            "required": ["target"]
                        }
                    },
                    {
                        "name": "engram_get_cochanges",
                        "description": "Retrieve historical Git co-change signals and statistics for a given file.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "file_path": { "type": "string" },
                                "min_probability": { "type": "number" }
                            },
                            "required": ["file_path"]
                        }
                    },
                    {
                        "name": "engram_get_memory",
                        "description": "Retrieve durable architectural decisions and team engineering memories.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": { "type": "string" },
                                "files": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    {
                        "name": "engram_record_decision",
                        "description": "Record an architectural decision and rationale into durable engineering memory.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "statement": { "type": "string" },
                                "reason": { "type": "string" },
                                "modules": { "type": "array", "items": { "type": "string" } },
                                "files": { "type": "array", "items": { "type": "string" } }
                            },
                            "required": ["title", "reason"]
                        }
                    },
                    {
                        "name": "engram_check_constitution",
                        "description": "Evaluate constitution rules against changed files.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "files": { "type": "array", "items": { "type": "string" } }
                            }
                        }
                    },
                    {
                        "name": "engram_get_ecosystem",
                        "description": "Discover declared and resolved packages, frameworks, and scripts in the project.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "engram_inspect_symbol",
                        "description": "Deeply inspect symbol change impact, multi-hop blast radius, causal provenance, and Change Contract authorization.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "target": { "type": "string", "description": "Symbol name or file path to inspect (e.g. OrderService.ExportCSV)" },
                                "depth": { "type": "integer", "description": "Maximum traversal hops (default 2)" }
                            },
                            "required": ["target"]
                        }
                    },
                    {
                        "name": "engram_why",
                        "description": "Query decision archaeology for a symbol, answering why it was designed this way with active policy, historical evolution, and provenance.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "target": { "type": "string", "description": "Symbol name or file path (e.g. OrderService.ExportCSV)" }
                            },
                            "required": ["target"]
                        }
                    },
                    {
                        "name": "engram_doctor",
                        "description": "Diagnose memory health, liveness, and scope resolution without modifying state (read-only).",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    },
                    {
                        "name": "engram_gc",
                        "description": "Execute controlled garbage collection on memory (supports dry_run, candidates, archive, vacuum).",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "dry_run": { "type": "boolean", "description": "Generate pre-flight plan without modifying disk" },
                                "candidates": { "type": "boolean", "description": "Prune candidate memory cache (TTL > 14 days or rejected)" },
                                "archive_superseded": { "type": "boolean", "description": "Move deep superseded memories (depth >= 3) to cold archive" },
                                "vacuum": { "type": "boolean", "description": "Vacuum SQLite database and compact caches" }
                            }
                        }
                    }
                ]
            })),
            "tools/call" => mcp_call(
                &root,
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
