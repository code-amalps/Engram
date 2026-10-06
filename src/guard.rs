//! Change Guard: task tracking, Change Contracts, blast radius validation, and constitution rule checks.

use anyhow::{Result, anyhow};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    path::Path,
};
use uuid::Uuid;

use crate::analyzer::ensure_fresh;
use crate::compiler::{ContextResult, context};
use crate::constitution::evaluate_rules;
use crate::graph::GraphTraversal;
use crate::storage::{db, git_changed, hash, now, revision, root_path};

pub fn analyze_task(root: &Path, task: &str, budget: usize) -> Result<(String, ContextResult)> {
    let root = root_path(root)?;
    let mut ctx = context(&root, task, budget)?;
    let id = format!("task_{}", Uuid::new_v4().simple());
    let baseline = serde_json::to_string(&git_changed(&root))?;

    // Evidence-aware Gap Detection
    let task_lower = task.to_ascii_lowercase();
    let words: Vec<&str> = task.split_whitespace().collect();

    if words.len() <= 2 && (task_lower.contains("refactor") || task_lower.contains("auth") || task_lower.contains("database")) {
        ctx.gaps.push(
            "GAP-AMBIGUOUS-SCOPE: Scope is broad or unspecified. Clarify which boundary or module should be changed."
                .to_string(),
        );
    }

    let conn = db(&root)?;
    conn.execute(
        "INSERT INTO tasks(task_id, task, created_at, state, baseline_json) VALUES(?1, ?2, ?3, 'ANALYZED', ?4)",
        params![id, task, now(), baseline],
    )?;

    for gap in &ctx.gaps {
        let (code, description) = gap.split_once(':').unwrap_or(("GAP-UNKNOWN", gap));
        conn.execute(
            "INSERT INTO task_gaps(gap_id, task_id, code, description, status, created_at) VALUES(?1, ?2, ?3, ?4, 'OPEN', ?5)",
            params![Uuid::new_v4().to_string(), id, code.trim(), description.trim(), now()],
        )?;
    }

    Ok((id, ctx))
}

pub fn make_contract(root: &Path, id: &str) -> Result<Value> {
    let root = root_path(root)?;
    let conn = db(&root)?;
    let (task, baseline): (String, String) = conn
        .query_row(
            "SELECT task, baseline_json FROM tasks WHERE task_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("Unknown task id {id}"))?;
    drop(conn);

    let ctx = context(&root, &task, 2400)?;
    let files: Vec<String> = ctx.files.iter().map(|x| x.path.clone()).collect();
    let mut expected_symbols = Vec::new();
    {
        let conn = db(&root)?;
        let mut stmt = conn.prepare(
            "SELECT name, kind, path 
             FROM entity_versions 
             WHERE path=?1 AND invalidated_at_revision IS NULL 
             ORDER BY start_line"
        )?;
        for file in &files {
            for row in stmt.query_map([file], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })? {
                let (name, kind, path) = row?;
                expected_symbols.push(format!("{path}::{kind} {name}"));
            }
        }
    }

    let conn = db(&root)?;
    let version: usize = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) + 1 FROM contracts WHERE task_id=?1",
        [id],
        |r| r.get(0),
    )?;
    let revision_id = revision(&root);
    let created_at = now();
    let contract = json!({
        "task_id": id,
        "task": task,
        "created_at": created_at,
        "expected_files": files,
        "expected_symbols": expected_symbols,
        "constraints": ctx.constraints,
        "confidence": ctx.confidence,
        "gaps": ctx.gaps,
        "baseline": serde_json::from_str::<Value>(&baseline)?,
        "advisory": true,
        "version": version
    });

    conn.execute(
        "UPDATE tasks SET state='CONTRACTED', contract_json=?1 WHERE task_id=?2",
        params![contract.to_string(), id],
    )?;
    conn.execute(
        "INSERT INTO contracts(task_id, version, revision_id, body_json, created_at) VALUES(?1, ?2, ?3, ?4, ?5)",
        params![id, version as i64, revision_id, contract.to_string(), created_at],
    )?;

    Ok(contract)
}

pub fn impact(root: &Path, id_or_symbol: &str) -> Result<Value> {
    let root = root_path(root)?;
    ensure_fresh(&root)?;
    let conn = db(&root)?;

    // Check if target is a task_id or a symbol/file for reverse dependency query
    let is_task = id_or_symbol.starts_with("task_");
    if !is_task {
        // Reverse dependency query: "Who will break if I change this?"
        let traversal = GraphTraversal::new(&conn);
        let callers = traversal.reverse_dependencies(id_or_symbol)?;
        return Ok(json!({
            "target": id_or_symbol,
            "reverse_dependencies": callers.iter().map(|e| json!({
                "source": e.source,
                "relationship": e.kind,
                "evidence": format!("{:?}", e.evidence_tier),
                "confidence": e.confidence,
                "line": e.line
            })).collect::<Vec<_>>()
        }));
    }

    let id = id_or_symbol;
    let (contract, baseline): (Option<String>, String) = conn
        .query_row(
            "SELECT contract_json, baseline_json FROM tasks WHERE task_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("Unknown task id {id}"))?;

    let expected: Value = contract
        .as_ref()
        .map(|s| serde_json::from_str(s))
        .transpose()?
        .unwrap_or(json!({"expected_files": []}));

    let base: Vec<String> = serde_json::from_str(&baseline)?;
    let actual = git_changed(&root);
    let added: Vec<String> = actual.into_iter().filter(|f| !base.contains(f)).collect();

    let expected_files = expected
        .get("expected_files")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let unexpected: Vec<String> = added
        .iter()
        .filter(|f| !expected_files.iter().any(|e| e.as_str() == Some(f)))
        .cloned()
        .collect();

    let modules: HashSet<String> = added
        .iter()
        .map(|f| f.split('/').next().unwrap_or(f).into())
        .collect();

    let mut symbols = Vec::new();
    let mut tests = Vec::new();
    for file in &added {
        let mut stmt = conn.prepare(
            "SELECT name, kind, start_line 
             FROM entity_versions 
             WHERE path=?1 AND invalidated_at_revision IS NULL 
             ORDER BY start_line"
        )?;
        for row in stmt.query_map([file], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, usize>(2)?,
            ))
        })? {
            let (name, kind, line) = row?;
            symbols.push(json!({"path": file, "name": name, "kind": kind, "line": line}));
        }
        if file.to_ascii_lowercase().contains("test") || file.to_ascii_lowercase().contains("spec") {
            tests.push(file.clone());
        }
    }

    let expected_symbols = expected
        .get("expected_symbols")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let unexpected_symbols: Vec<Value> = symbols
        .iter()
        .filter(|symbol| {
            let actual_id = format!(
                "{}::{} {}",
                symbol["path"].as_str().unwrap_or_default(),
                symbol["kind"].as_str().unwrap_or_default(),
                symbol["name"].as_str().unwrap_or_default()
            );
            !expected_symbols
                .iter()
                .any(|item| item.as_str() == Some(&actual_id))
        })
        .cloned()
        .collect();

    let revision_id = revision(&root);
    for file in &added {
        let symbols_for_file: Vec<Option<String>> = symbols
            .iter()
            .filter(|symbol| symbol["path"].as_str() == Some(file))
            .map(|symbol| symbol["name"].as_str().map(str::to_string))
            .collect();
        let names = if symbols_for_file.is_empty() {
            vec![None]
        } else {
            symbols_for_file
        };
        for symbol in names {
            let change_id = format!(
                "chg_{}",
                &hash(format!("{id}\0{revision_id}\0{file}\0{symbol:?}").as_bytes())[..24]
            );
            conn.execute(
                "INSERT OR IGNORE INTO change_events(change_id, task_id, revision_id, path, symbol_name, change_kind, origin, evidence_tier, observed_at) 
                 VALUES(?1, ?2, ?3, ?4, ?5, 'WORKTREE_DELTA', 'UNKNOWN', 'PROVISIONAL', ?6)",
                params![change_id, id, revision_id, file, symbol, now()],
            )?;
        }
    }

    Ok(json!({
        "task_id": id,
        "expected": {
            "files": expected_files,
            "symbols": expected_symbols
        },
        "actual": {
            "files": added,
            "symbols": symbols,
            "tests": tests,
            "modules": modules
        },
        "unexpected_files": unexpected,
        "unexpected_symbols": unexpected_symbols,
        "warning": if contract.is_some() && (!unexpected.is_empty() || !unexpected_symbols.is_empty()) {
            Some("Working-tree changes exceed the expected boundary; review whether the expansion is justified.")
        } else {
            None
        }
    }))
}

pub fn validate(root: &Path, id: &str) -> Result<Value> {
    let impact = impact(root, id)?;
    let root = root_path(root)?;
    let mut warnings = Vec::new();

    if impact["unexpected_files"]
        .as_array()
        .is_some_and(|a| !a.is_empty())
    {
        warnings.push(json!({
            "code": "SCOPE_EXPANSION",
            "files": impact["unexpected_files"],
            "message": "Changed paths are outside the contract boundary."
        }));
    }

    let changed_files: Vec<String> = impact["actual"]["files"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();

    let conn = db(&root)?;

    // Evaluate Project Constitution rules
    let rule_violations = evaluate_rules(&root, &changed_files, &conn)?;
    let mut active_violations = Vec::new();
    let mut overridden_violations = Vec::new();

    for v in rule_violations {
        if v.overridden {
            overridden_violations.push(json!({
                "rule_id": v.rule_id,
                "rule_name": v.rule_name,
                "file": v.file_path,
                "override_reason": v.override_reason
            }));
        } else {
            active_violations.push(json!({
                "violation_id": v.violation_id,
                "rule_id": v.rule_id,
                "rule_name": v.rule_name,
                "severity": format!("{:?}", v.severity).to_ascii_uppercase(),
                "file": v.file_path,
                "line": v.line,
                "message": v.message
            }));
        }
    }

    Ok(json!({
        "task_id": id,
        "valid": true,
        "advisory": true,
        "errors": [],
        "warnings": warnings,
        "rule_violations": active_violations,
        "overridden_violations": overridden_violations,
        "impact": impact
    }))
}

pub fn record_decision(
    root: &Path,
    id: &str,
    value: &str,
    reason: &str,
    rule_id: Option<&str>,
    violation_id: Option<&str>,
) -> Result<Value> {
    let conn = db(&root_path(root)?)?;
    let task_id = if id.trim().is_empty() {
        conn.query_row(
            "SELECT task_id FROM tasks ORDER BY created_at DESC LIMIT 1",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .ok_or_else(|| anyhow!("No task exists yet; analyze a task before recording a decision"))?
    } else {
        id.to_string()
    };

    conn.execute(
        "INSERT INTO decisions(task_id, decision, reason, rule_id, violation_id, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![task_id, value, reason, rule_id, violation_id, now()],
    )?;

    Ok(json!({
        "task_id": task_id,
        "decision": value,
        "reason": reason,
        "rule_id": rule_id,
        "violation_id": violation_id,
        "recorded": true
    }))
}

pub fn decision(root: &Path, id: &str, value: &str, reason: &str) -> Result<Value> {
    record_decision(root, id, value, reason, None, None)
}

pub fn update_memory(root: &Path, id: &str) -> Result<Value> {
    let conn = db(&root_path(root)?)?;
    let (task, decision, reason): (String, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT t.task, d.decision, d.reason 
             FROM tasks t 
             LEFT JOIN decisions d ON d.id = (SELECT MAX(id) FROM decisions WHERE task_id = t.task_id) 
             WHERE t.task_id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .ok_or_else(|| anyhow!("Unknown task id {id}"))?;

    let memory_text = if let (Some(dec), Some(res)) = (decision, reason) {
        format!("Task '{task}' concluded with decision [{dec}]: {res}")
    } else {
        format!("Task '{task}' was completed and verified against working tree.")
    };

    conn.execute(
        "INSERT INTO memories(task_id, content, tier, created_at) VALUES(?1, ?2, 'HISTORICAL', ?3)",
        params![id, memory_text, now()],
    )?;

    Ok(json!({
        "promoted": true,
        "task_id": id,
        "tier": "HISTORICAL",
        "message": "Accepted change promoted to local memory."
    }))
}
