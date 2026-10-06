use anyhow::Result;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use crate::analyzer::{ensure_fresh, estimate_tokens, tokens};
use crate::storage::{db, revision, root_path};

#[derive(Debug, Clone, Serialize)]
pub struct ContextFile {
    pub path: String,
    pub score: f64,
    pub level: u8,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextResult {
    pub task: String,
    pub project: String,
    pub revision: String,
    pub indexed_files: usize,
    pub token_budget: usize,
    pub estimated_tokens: usize,
    pub confidence: String,
    pub gaps: Vec<String>,
    pub files: Vec<ContextFile>,
}

pub fn context(root: &Path, task: &str, budget: usize) -> Result<ContextResult> {
    let root = root_path(root)?;
    ensure_fresh(&root)?;
    let conn = db(&root)?;
    let terms = tokens(task);
    let mut docs = Vec::<(String, String, String)>::new();
    {
        let mut s = conn.prepare("SELECT path, language, excerpt FROM files")?;
        for r in s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))? {
            docs.push(r?);
        }
    }
    let n = docs.len() as f64;
    let mut df: HashMap<String, usize> = HashMap::new();
    for (_, _, text) in &docs {
        let uniq: HashSet<_> = tokens(text).into_iter().collect();
        for t in uniq {
            *df.entry(t).or_default() += 1;
        }
    }
    let avg = (docs.iter().map(|x| tokens(&x.2).len()).sum::<usize>() as f64 / (n.max(1.0))).max(1.0);
    let mut ranked: Vec<(f64, String, String, String)> = docs
        .into_iter()
        .map(|(path, lang, body)| {
            let dt = tokens(&format!("{path} {body}"));
            let score = terms
                .iter()
                .map(|q| {
                    let tf = dt.iter().filter(|x| *x == q).count() as f64;
                    if tf == 0.0 {
                        return 0.0;
                    }
                    let idf = (1.0
                        + (n - df.get(q).copied().unwrap_or(0) as f64 + 0.5)
                            / (df.get(q).copied().unwrap_or(0) as f64 + 0.5))
                        .ln();
                    idf * tf * 2.2 / (tf + 1.2 * (0.25 + 0.75 * dt.len() as f64 / avg))
                })
                .sum();
            (score, path, lang, body)
        })
        .filter(|x| x.0 > 0.0)
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    let count = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get::<_, usize>(0))?;
    let mut selected = Vec::new();
    let limit = budget;
    let mut used = 0;
    for (score, path, lang, body) in ranked {
        let mut syms = Vec::new();
        {
            let mut s = conn.prepare(
                "SELECT name, kind, start_line, signature 
                 FROM entity_versions 
                 WHERE path=?1 AND invalidated_at_revision IS NULL 
                 ORDER BY start_line"
            )?;
            for row in s.query_map([&path], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, usize>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })? {
                let (name, kind, line, sig) = row?;
                syms.push(format!("{kind} {name} (L{line}): {sig}"));
            }
        }
        let level = if score > 3.0 { 3 } else { 2 };
        let max_chars = limit.saturating_sub(used).saturating_mul(4).max(220);
        let mut content = format!(
            "FILE {path} [{lang}]\nSYMBOLS:\n{}\n",
            if syms.is_empty() {
                "No declarations extracted".into()
            } else {
                syms.join("\n")
            }
        );
        if level == 3 {
            content.push_str("SOURCE EXCERPT:\n");
            content.push_str(
                &body
                    .chars()
                    .take(max_chars.saturating_sub(content.len()))
                    .collect::<String>(),
            );
        }
        let cost = estimate_tokens(&content);
        if used + cost > limit {
            continue;
        }
        used += cost;
        selected.push(ContextFile {
            path,
            score: (score * 100.0).round() / 100.0,
            level,
            content,
        });
        if used >= limit {
            break;
        }
    }
    let confidence = selected
        .first()
        .map(|f| {
            if f.score > 4.0 {
                "HIGH"
            } else if f.score > 1.0 {
                "MEDIUM"
            } else {
                "LOW"
            }
        })
        .unwrap_or("LOW")
        .to_string();
    let gaps = if selected.is_empty() {
        vec!["GAP-001: No matching files or symbols were grounded for this task.".into()]
    } else {
        Vec::new()
    };
    Ok(ContextResult {
        task: task.into(),
        project: root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        revision: revision(&root),
        indexed_files: count,
        token_budget: limit,
        estimated_tokens: used,
        confidence,
        gaps,
        files: selected,
    })
}
