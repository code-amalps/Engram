//! Context Compiler: progressive BM25 ranking, graph expansion, provenance tracking, and token budget packing.

use anyhow::Result;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use crate::analyzer::{ensure_fresh, estimate_tokens, tokens};
use crate::constitution::{format_constraints, load_constitution};
use crate::git_history::get_cochanges_for_file;
use crate::graph::GraphTraversal;
use crate::manifest::{detect_ecosystem, format_agent_guard};
use crate::model::Direction;
use crate::storage::{db, revision, root_path};

#[derive(Debug, Clone, Serialize)]
pub struct ContextFile {
    pub path: String,
    pub score: f64,
    pub level: u8,
    pub provenance: Vec<String>,
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
    pub constraints: Vec<String>,
    pub relevant_memories: Vec<String>,
    pub cochange_alerts: Vec<String>,
    pub ecosystem_guard: Option<String>,
    pub gaps: Vec<String>,
    pub files: Vec<ContextFile>,
    pub excluded_files: Vec<(String, String)>,
}

pub fn context(root: &Path, task: &str, budget: usize) -> Result<ContextResult> {
    let root = root_path(root)?;
    ensure_fresh(&root)?;
    let conn = db(&root)?;
    let terms = tokens(task);

    // 1. Load active constitution constraints
    let rules = load_constitution(&root).unwrap_or_default();
    let constraints = format_constraints(&rules);

    // 2. Load ecosystem manifest guard
    let eco = detect_ecosystem(&root).ok();
    let eco_guard = eco.as_ref().map(format_agent_guard);

    // 3. BM25 Lexical Ranking
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
    let avg =
        (docs.iter().map(|x| tokens(&x.2).len()).sum::<usize>() as f64 / (n.max(1.0))).max(1.0);

    let mut doc_map: HashMap<String, (String, String)> = HashMap::new();
    let mut bm25_scores: HashMap<String, f64> = HashMap::new();

    for (path, lang, body) in docs {
        let dt = tokens(&format!("{path} {body}"));
        let score: f64 = terms
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
        if score > 0.0 {
            bm25_scores.insert(path.clone(), score);
        }
        doc_map.insert(path, (lang, body));
    }

    // 4. Graph Expansion: 1-hop and 2-hop dependencies and callers
    let traversal = GraphTraversal::new(&conn);
    let mut provenance_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut composite_scores = bm25_scores.clone();

    // Attach lexical provenance
    for (path, score) in &bm25_scores {
        provenance_map
            .entry(path.clone())
            .or_default()
            .push(format!("BM25 lexical match ({:.2})", score));
    }

    // Sort top BM25 seeds for graph expansion
    let mut seeds: Vec<(String, f64)> = bm25_scores.into_iter().collect();
    seeds.sort_by(|a, b| b.1.total_cmp(&a.1));

    for (seed_path, seed_score) in seeds.into_iter().take(5) {
        // Impact Cone Expansion: Categorized ranking (Direct > Test > Implementation > Caller > ImportDependent > Historical)
        if let Ok(cone) = crate::impact::compute_impact_cone(&conn, &root, &seed_path, 2) {
            for entity in cone.all_entities() {
                if doc_map.contains_key(&entity.path) {
                    let cat_weight = match entity.category {
                        crate::impact::ImpactCategory::Direct => 12.0,
                        crate::impact::ImpactCategory::Test => 9.0,
                        crate::impact::ImpactCategory::Implementation => 8.0,
                        crate::impact::ImpactCategory::Caller => 6.0,
                        crate::impact::ImpactCategory::ImportDependent => 3.5,
                        crate::impact::ImpactCategory::Configuration => 3.0,
                        crate::impact::ImpactCategory::Historical => 2.5,
                    };
                    let boost = (seed_score * 0.5).max(cat_weight);
                    let score_entry = composite_scores.entry(entity.path.clone()).or_insert(0.0);
                    *score_entry += boost;

                    let prov = format!(
                        "Impact {} (hop {}) via {} (reason: {})",
                        entity.category.as_str(),
                        entity.hop_distance,
                        seed_path,
                        entity.reason
                    );
                    let list = provenance_map.entry(entity.path.clone()).or_default();
                    if !list.contains(&prov) {
                        list.push(prov);
                    }
                }
            }
        }

        // Expand 1-hop and 2-hop neighbors (IMPORTS, TESTS, IMPLEMENTS)
        if let Ok(neighbors) = traversal.neighbors(
            &seed_path,
            &["IMPORTS", "TESTS", "IMPLEMENTS"],
            Direction::Both,
            2,
        ) {
            for edge in neighbors {
                let neighbor_path = if edge.source == seed_path
                    || edge.source.starts_with(&format!("{seed_path}::"))
                {
                    edge.target.clone()
                } else {
                    edge.source.clone()
                };
                let neighbor_file = neighbor_path.split("::").next().unwrap_or(&neighbor_path);

                // Only consider workspace files
                if doc_map.contains_key(neighbor_file) {
                    let boost = (seed_score * 0.4).max(2.5);
                    let score_entry = composite_scores
                        .entry(neighbor_file.to_string())
                        .or_insert(0.0);
                    *score_entry += boost;

                    let prov = format!(
                        "Graph {} edge via {} (confidence: {:.2})",
                        edge.kind, seed_path, edge.confidence
                    );
                    let list = provenance_map.entry(neighbor_file.to_string()).or_default();
                    if !list.contains(&prov) {
                        list.push(prov);
                    }
                }
            }
        }
    }

    // 5. Rank all candidates by composite score
    let mut candidates: Vec<(String, f64)> = composite_scores.into_iter().collect();
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1));

    let count = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get::<_, usize>(0))?;
    let limit = budget;
    let mut used = 0;
    let mut selected = Vec::new();
    let mut excluded = Vec::new();

    // Reserve a small token slice for ecosystem and constraints
    let overhead_tokens = estimate_tokens(&constraints.join("\n"))
        + eco_guard.as_deref().map(estimate_tokens).unwrap_or(0);
    used += overhead_tokens;

    let mut all_selected_symbols = Vec::new();

    for (path, score) in candidates {
        let (lang, body) = match doc_map.get(&path) {
            Some(d) => d,
            None => continue,
        };

        let mut syms = Vec::new();
        {
            let mut s = conn.prepare(
                "SELECT name, kind, start_line, signature 
                 FROM entity_versions 
                 WHERE path=?1 AND invalidated_at_revision IS NULL 
                 ORDER BY start_line",
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
                all_selected_symbols.push(name.clone());
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
            excluded.push((
                path,
                format!(
                    "Budget limit exceeded (+{} tokens would exceed {})",
                    cost, limit
                ),
            ));
            continue;
        }

        used += cost;
        let prov = provenance_map
            .remove(&path)
            .unwrap_or_else(|| vec!["Ranked candidate".into()]);
        selected.push(ContextFile {
            path,
            score: (score * 100.0).round() / 100.0,
            level,
            provenance: prov,
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

    let touched_paths: Vec<String> = selected.iter().map(|f| f.path.clone()).collect();
    let memories = crate::memory::query_scoped_memories(
        &root,
        task,
        &touched_paths,
        &all_selected_symbols,
        false,
    )
    .unwrap_or_default();
    let relevant_memories: Vec<String> = memories
        .into_iter()
        .map(|m| {
            let scope_tag = if !m.scope.modules.is_empty() {
                m.scope.modules.join(", ")
            } else if let Some(ref t) = m.scope.target {
                t.clone()
            } else {
                format!("{:?}", m.scope.level)
            };
            format!(
                "[{}] {}: {} (Reason: {})",
                scope_tag, m.title, m.statement, m.reason
            )
        })
        .collect();

    let mut cochange_alerts = Vec::new();
    for f in &selected {
        if let Ok(signals) = get_cochanges_for_file(&root, &f.path, 0.4) {
            for sig in signals {
                if !touched_paths.contains(&sig.target_file)
                    && !cochange_alerts.contains(&sig.advisory_message)
                {
                    cochange_alerts.push(sig.advisory_message);
                }
            }
        }
    }

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
        constraints,
        relevant_memories,
        cochange_alerts,
        ecosystem_guard: eco_guard,
        gaps,
        files: selected,
        excluded_files: excluded,
    })
}

pub fn format_explain(res: &ContextResult) -> String {
    let mut out = String::new();
    out.push_str(&format!("🧠 Task: \"{}\"\n", res.task));
    out.push_str(&format!(
        "Confidence: {} | Budget: {} / {} tokens\n\n",
        res.confidence, res.estimated_tokens, res.token_budget
    ));

    if !res.relevant_memories.is_empty() {
        out.push_str("RELEVANT ENGINEERING MEMORIES:\n");
        for m in &res.relevant_memories {
            out.push_str(&format!("  ℹ️ {}\n", m));
        }
        out.push('\n');
    }

    if !res.cochange_alerts.is_empty() {
        out.push_str("HISTORICAL CO-CHANGE SIGNALS:\n");
        for a in &res.cochange_alerts {
            out.push_str(&format!("  ⚡ {}\n", a));
        }
        out.push('\n');
    }

    out.push_str("SELECTED FILES:\n");
    for f in &res.files {
        out.push_str(&format!(
            "  ✓ {} (score: {:.2}, Level {})\n",
            f.path, f.score, f.level
        ));
        for p in &f.provenance {
            out.push_str(&format!("      • {}\n", p));
        }
    }

    if !res.excluded_files.is_empty() {
        out.push_str("\nEXCLUDED CANDIDATES:\n");
        for (path, reason) in res.excluded_files.iter().take(8) {
            out.push_str(&format!("  ✗ {} — {}\n", path, reason));
        }
    }

    if !res.constraints.is_empty() {
        out.push_str("\nACTIVE CONSTRAINTS:\n");
        for c in &res.constraints {
            out.push_str(&format!("  ⚠️ {}\n", c));
        }
    }

    out
}
