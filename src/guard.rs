use anyhow::{Result, anyhow};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    path::Path,
};
use uuid::Uuid;

use crate::analyzer::{ensure_fresh, redact};
use crate::compiler::{ContextResult, context};
use crate::storage::{db, git_changed, hash, now, revision, root_path};

pub fn analyze_task(root: &Path, task: &str, budget: usize) -> Result<(String, ContextResult)> {
    let root = root_path(root)?;
    let ctx = context(&root, task, budget)?;
    let id = format!("task_{}", Uuid::new_v4().simple());
    let baseline = serde_json::to_string(&git_changed(&root))?;
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
        "constraints": [],
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

pub fn impact(root: &Path, id: &str) -> Result<Value> {
    let root = root_path(root)?;
    ensure_fresh(&root)?;
    let conn = db(&root)?;
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
    let constitution = fs::read_to_string(root.join(".engram/constitution.yaml")).unwrap_or_default();
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
    let changed_files = impact["actual"]["files"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let conn = db(&root)?;
    if constitution.contains("domain_must_not_depend_on_infrastructure") {
        for file in &changed_files {
            let Some(path) = file.as_str() else { continue };
            if !path.to_ascii_lowercase().contains("domain") {
                continue;
            }
            let mut stmt = conn.prepare(
                "SELECT target, line FROM relationships WHERE path=?1 AND invalidated_at_revision IS NULL"
            )?;
            for row in stmt.query_map([path], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, usize>(1)?))
            })? {
                let (target, line) = row?;
                if target.to_ascii_lowercase().contains("infrastructure") {
                    warnings.push(json!({
                        "code": "ARCHITECTURE_BOUNDARY_REVIEW",
                        "file": path,
                        "line": line,
                        "evidence": "TREE_SITTER_IMPORT_SYNTAX_AND_PATH_HEURISTIC",
                        "message": format!("Domain file imports an Infrastructure path: {target}")
                    }));
                }
            }
        }
    }
    if constitution.contains("service_changes_require_unit_tests: true")
        && changed_files.iter().any(|f| {
            f.as_str().is_some_and(|p| {
                p.to_ascii_lowercase().contains("service")
                    && !p.to_ascii_lowercase().contains("test")
            })
        })
        && !changed_files.iter().any(|f| {
            f.as_str()
                .is_some_and(|p| p.to_ascii_lowercase().contains("test"))
        })
    {
        warnings.push(json!({
            "code": "EXPECTED_TESTS_NOT_CHANGED",
            "message": "A service file changed but no test-like path changed. This is an advisory path-based check."
        }));
    }
    Ok(json!({
        "task_id": id,
        "valid": true,
        "advisory": true,
        "errors": [],
        "warnings": warnings,
        "impact": impact
    }))
}

pub fn decision(root: &Path, id: &str, value: &str, reason: &str) -> Result<Value> {
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
        "INSERT INTO decisions(task_id, decision, reason, created_at) VALUES(?1, ?2, ?3, ?4)",
        params![task_id, value, reason, now()],
    )?;
    Ok(json!({
        "task_id": task_id,
        "decision": value,
        "reason": reason,
        "recorded": true
    }))
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
    let accepted = decision.as_deref().is_some_and(|d| {
        matches!(
            d.to_ascii_lowercase().as_str(),
            "accept" | "accepted" | "approve" | "approved"
        )
    });
    let promoted = if accepted {
        let text = format!(
            "Task: {task}\nDecision: {}\nRationale: {}",
            decision.as_deref().unwrap_or("accepted"),
            reason.as_deref().unwrap_or("")
        );
        conn.execute(
            "INSERT INTO memories(task_id, content, tier, created_at) VALUES(?1, ?2, 'HISTORICAL', ?3)",
            params![id, redact(&text), now()],
        )?;
        true
    } else {
        false
    };
    conn.execute(
        "UPDATE tasks SET state=?1 WHERE task_id=?2",
        params![
            if promoted {
                "MEMORY_UPDATED"
            } else {
                "MEMORY_NOT_PROMOTED"
            },
            id
        ],
    )?;
    Ok(json!({
        "task_id": id,
        "promoted": promoted,
        "tier": if promoted { Some("HISTORICAL") } else { None },
        "message": if promoted {
            "Accepted change promoted to local memory."
        } else {
            "Record an accepted decision before promoting memory."
        }
    }))
}
