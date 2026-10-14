//! Project Constitution and Rules Engine: machine-evaluable prescriptions, multi-type rules, and overrides.

use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

use crate::model::{ConstitutionRule, RuleSeverity, RuleType, RuleViolation};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConstitutionFile {
    pub schema_version: Option<u32>,
    pub architecture: Option<serde_yaml::Value>,
    #[serde(default)]
    pub rules: Vec<ConstitutionRule>,
}

/// Checks whether a given repository relative path is a sovereign policy/governance file.
pub fn is_policy_file(path: &str) -> bool {
    let clean = path.replace('\\', "/");
    let norm = clean.trim_start_matches("./").trim_end_matches('/');
    norm == ".engram/constitution.yaml"
        || norm == ".engram/constitution.yml"
        || norm == ".engram/memory.yaml"
        || norm.starts_with(".engram/policies/")
        || norm == "action.yml"
        || norm.starts_with(".github/workflows/")
}

pub fn load_constitution(root: &Path) -> Result<Vec<ConstitutionRule>> {
    let candidates = [
        root.join(".engram").join("constitution.yaml"),
        root.join(".engram").join("constitution.yml"),
    ];
    for path in &candidates {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let parsed: ConstitutionFile = serde_yaml::from_str(&content).unwrap_or_default();
            return Ok(parsed.rules.into_iter().filter(|r| r.enabled).collect());
        }
    }
    Ok(Vec::new())
}

/// Load sovereign constitution rules at a specific git revision (e.g. PR base commit).
pub fn load_constitution_at_revision(root: &Path, rev: &str) -> Result<Vec<ConstitutionRule>> {
    let trimmed = rev.trim();
    if trimmed == "WORKTREE" || trimmed.is_empty() {
        return load_constitution(root);
    }

    let candidates = [".engram/constitution.yaml", ".engram/constitution.yml"];
    for cand in &candidates {
        let output = std::process::Command::new("git")
            .args(["show", &format!("{}:{}", trimmed, cand)])
            .current_dir(root)
            .output();

        if let Ok(out) = output {
            if out.status.success() {
                let content = String::from_utf8_lossy(&out.stdout);
                if let Ok(parsed) = serde_yaml::from_str::<ConstitutionFile>(&content) {
                    return Ok(parsed.rules.into_iter().filter(|r| r.enabled).collect());
                }
            }
        }
    }

    Ok(Vec::new())
}

pub fn format_constraints(rules: &[ConstitutionRule]) -> Vec<String> {
    rules
        .iter()
        .map(|r| {
            let sev = match r.severity {
                RuleSeverity::Info => "INFO",
                RuleSeverity::Warning => "WARNING",
                RuleSeverity::Error => "ERROR",
            };
            format!(
                "[{}: {}] {} - {}",
                r.id,
                sev,
                r.name,
                if r.description.is_empty() {
                    r.name.as_str()
                } else {
                    r.description.as_str()
                }
            )
        })
        .collect()
}

pub fn load_overrides(conn: &rusqlite::Connection) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let mut stmt = conn.prepare(
        "SELECT rule_id, reason FROM decisions WHERE decision = 'OVERRIDE' AND rule_id IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (rule_id, reason) = row?;
        map.insert(rule_id, reason);
    }
    Ok(map)
}

fn glob_matches(path: &str, pattern: &str) -> bool {
    let p = path.replace('\\', "/");
    let pat = pattern.replace('\\', "/");
    if pat == "**" || pat == "*" {
        return true;
    }
    if let Some(suffix) = pat.strip_prefix("**/*") {
        return p.ends_with(suffix);
    }
    if let Some(prefix) = pat.strip_suffix("/**") {
        return p.starts_with(prefix);
    }
    p.contains(&pat)
}

pub fn evaluate_rules(
    root: &Path,
    modified_files: &[String],
    conn: &rusqlite::Connection,
) -> Result<Vec<RuleViolation>> {
    let rules = load_constitution(root)?;
    if rules.is_empty() {
        return Ok(Vec::new());
    }

    let overrides = load_overrides(conn)?;
    let mut violations = Vec::new();

    for rule in rules {
        let is_overridden = overrides.contains_key(&rule.id);
        let override_reason = overrides.get(&rule.id).cloned();

        match rule.rule_type {
            RuleType::Pattern => {
                let pattern_str = match &rule.pattern {
                    Some(p) => p,
                    None => continue,
                };
                let rx = match Regex::new(pattern_str) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                for file in modified_files {
                    if let Some(target_glob) = &rule.target_files {
                        if !glob_matches(file, target_glob) {
                            continue;
                        }
                    }

                    let file_abs = root.join(file);
                    if let Ok(content) = fs::read_to_string(&file_abs) {
                        for (idx, line) in content.lines().enumerate() {
                            if rx.is_match(line) {
                                violations.push(RuleViolation {
                                    violation_id: format!("{}-{}-L{}", rule.id, file.replace(['/', '\\'], "_"), idx + 1),
                                    rule_id: rule.id.clone(),
                                    rule_name: rule.name.clone(),
                                    severity: rule.severity.clone(),
                                    file_path: file.clone(),
                                    line: Some(idx + 1),
                                    message: format!(
                                        "Matches forbidden pattern '{}': {}",
                                        pattern_str,
                                        line.trim()
                                    ),
                                    overridden: is_overridden,
                                    override_reason: override_reason.clone(),
                                });
                                break;
                            }
                        }
                    }
                }
            }
            RuleType::Dependency => {
                let src_pattern = match &rule.source {
                    Some(s) => s,
                    None => continue,
                };
                let forbidden_pattern = match &rule.forbidden {
                    Some(f) => f,
                    None => continue,
                };

                for file in modified_files {
                    if !glob_matches(file, src_pattern) {
                        continue;
                    }
                    let mut stmt = conn.prepare(
                        "SELECT target, line FROM relationships WHERE source_id = ?1 AND kind = 'IMPORTS'"
                    )?;
                    let rows = stmt.query_map([file], |r| {
                        Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
                    })?;
                    for row in rows {
                        let (target, line) = row?;
                        if glob_matches(&target, forbidden_pattern) {
                            violations.push(RuleViolation {
                                violation_id: format!("{}-{}-dep", rule.id, file.replace(['/', '\\'], "_")),
                                rule_id: rule.id.clone(),
                                rule_name: rule.name.clone(),
                                severity: rule.severity.clone(),
                                file_path: file.clone(),
                                line: line.map(|l| l as usize),
                                message: format!(
                                    "Forbidden architectural dependency: '{}' must not import '{}'",
                                    file, target
                                ),
                                overridden: is_overridden,
                                override_reason: override_reason.clone(),
                            });
                        }
                    }
                }
            }
            RuleType::Architecture => {
                // Architectural relationship checks
                if let (Some(source_kind), Some(forbidden_rel)) = (&rule.source_kind, &rule.forbidden_relationship) {
                    for file in modified_files {
                        let mut stmt = conn.prepare(
                            "SELECT r.target, r.line 
                             FROM relationships r
                             JOIN entity_versions ev ON ev.path = r.source_id
                             WHERE r.source_id = ?1 AND ev.kind = ?2 AND r.kind = ?3"
                        )?;
                        let rows = stmt.query_map(rusqlite::params![file, source_kind, forbidden_rel], |r| {
                            Ok((r.get::<_, String>(0)?, r.get::<_, Option<i64>>(1)?))
                        })?;
                        for row in rows {
                            let (target, line) = row?;
                            violations.push(RuleViolation {
                                violation_id: format!("{}-{}-arch", rule.id, file.replace(['/', '\\'], "_")),
                                rule_id: rule.id.clone(),
                                rule_name: rule.name.clone(),
                                severity: rule.severity.clone(),
                                file_path: file.clone(),
                                line: line.map(|l| l as usize),
                                message: format!(
                                    "Architecture violation: {} '{}' has forbidden relationship '{}' to '{}'",
                                    source_kind, file, forbidden_rel, target
                                ),
                                overridden: is_overridden,
                                override_reason: override_reason.clone(),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(violations)
}
