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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subpath_filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language_filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub omission_manifest: Option<crate::budget::OmissionManifest>,
}

/// Normalizes and validates a user-provided subpath filter.
/// Enforces forward slashes, trims trailing slashes, and rejects absolute paths and directory traversal (`..`).
pub fn normalize_subpath(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        anyhow::bail!("Subpath cannot be empty");
    }
    let normalized = trimmed.replace('\\', "/");
    if normalized.starts_with('/') || normalized.contains(':') {
        anyhow::bail!(
            "Subpath '{}' cannot be absolute or contain a drive letter; must be relative to repository root",
            raw
        );
    }
    for segment in normalized.split('/') {
        if segment == ".." {
            anyhow::bail!(
                "Subpath '{}' cannot contain parent directory traversal ('..')",
                raw
            );
        }
    }
    let clean = normalized.trim_matches('/').to_string();
    if clean.is_empty() {
        anyhow::bail!("Subpath cannot resolve to empty root");
    }
    Ok(clean)
}

/// Canonicalizes and validates a user-provided language filter.
/// Supports standard aliases (e.g. `rs` -> `rust`, `py` -> `python`, `ts`/`tsx` -> `typescript`).
pub fn canonicalize_language(raw: &str) -> Result<&'static str> {
    let lower = raw.trim().to_ascii_lowercase();
    match lower.as_str() {
        "rust" | "rs" => Ok("rust"),
        "python" | "py" => Ok("python"),
        "typescript" | "ts" | "tsx" => Ok("typescript"),
        "javascript" | "js" | "jsx" | "mjs" | "cjs" => Ok("javascript"),
        "csharp" | "cs" | "c#" => Ok("csharp"),
        "go" | "golang" => Ok("go"),
        "java" => Ok("java"),
        _ => anyhow::bail!(
            "Unsupported language filter '{}'. Supported: rust, python, typescript, javascript, csharp, go, java (aliases: rs, py, ts, js, cs)",
            raw
        ),
    }
}

pub fn context(root: &Path, task: &str, budget: usize) -> Result<ContextResult> {
    context_scoped_with_detail(root, task, budget, None, None, crate::budget::DetailLevel::Contracted)
}

pub fn context_scoped(
    root: &Path,
    task: &str,
    budget: usize,
    subpath: Option<&str>,
    language: Option<&str>,
) -> Result<ContextResult> {
    context_scoped_with_detail(root, task, budget, subpath, language, crate::budget::DetailLevel::Contracted)
}

pub fn context_scoped_with_detail(
    root: &Path,
    task: &str,
    budget: usize,
    subpath: Option<&str>,
    language: Option<&str>,
    detail_level: crate::budget::DetailLevel,
) -> Result<ContextResult> {
    let normalized_subpath = subpath
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(normalize_subpath)
        .transpose()?;
    let canonical_lang = language
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(canonicalize_language)
        .transpose()?;

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
            let (path, lang, excerpt): (String, String, String) = r?;
            if let Some(ref sub) = normalized_subpath {
                if !(path == *sub || path.starts_with(&format!("{}/", sub))) {
                    continue;
                }
            }
            if let Some(target_lang) = canonical_lang {
                if lang != target_lang {
                    continue;
                }
            }
            docs.push((path, lang, excerpt));
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

    for (path, lang, body) in &docs {
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
        doc_map.insert(path.clone(), (lang.clone(), body.clone()));
    }

    // 4. Decoupled Query Routing: Multi-Strategy Seed Discovery & SPI Expansion
    let traversal = GraphTraversal::new(&conn);
    let mut provenance_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut composite_scores = bm25_scores.clone();

    // 4.1 Attach lexical provenance
    for (path, score) in &bm25_scores {
        provenance_map
            .entry(path.clone())
            .or_default()
            .push(format!("BM25 lexical match ({:.2})", score));
    }

    // 4.2 Exact Symbol Discovery from Task Query
    let mut potential_symbols: Vec<String> = Vec::new();
    for word in task.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '.') {
        let w = word.trim_matches('.');
        if w.len() >= 3 && (w.chars().any(|c| c.is_uppercase()) || w.contains('_')) {
            if !potential_symbols.contains(&w.to_string()) {
                potential_symbols.push(w.to_string());
            }
        }
    }

    let mut symbol_seeds: Vec<(String, String, usize, usize)> = Vec::new();
    for sym in &potential_symbols {
        if let Ok(mut s) = conn.prepare(
            "SELECT DISTINCT path, name, start_line, end_line
             FROM entity_versions
             WHERE (name = ?1 OR name LIKE ?2) AND invalidated_at_revision IS NULL",
        ) {
            let pat = format!("%{}", sym);
            if let Ok(rows) = s.query_map(rusqlite::params![sym, pat], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, usize>(2)?,
                    r.get::<_, usize>(3)?,
                ))
            }) {
                for r in rows.flatten() {
                    let clean_p = r.0.replace('\\', "/");
                    if !symbol_seeds.iter().any(|s| s.0 == clean_p && s.1 == r.1) {
                        symbol_seeds.push((clean_p, r.1, r.2, r.3));
                    }
                }
            }
        }
    }

    for (p, sym_name, start_line, end_line) in &symbol_seeds {
        if doc_map.contains_key(p) {
            *composite_scores.entry(p.clone()).or_insert(0.0) += 1000.0;
            let prov = format!(
                "Exact symbol match: {} (Lines {}..{})",
                sym_name, start_line, end_line
            );
            let list = provenance_map.entry(p.clone()).or_default();
            if !list.contains(&prov) {
                list.push(prov);
            }
        }
    }

    // 4.3 Framework Relationship Provider Detection & Task-Specific Companion Seeds
    let all_file_paths: Vec<String> = docs.iter().map(|d| d.0.clone()).collect();
    let framework_provider = crate::frameworks::detect_framework_provider(&root, &all_file_paths);
    let task_companions = framework_provider.task_companion_seeds(&root, &conn, task);

    // Helper to ensure companion files that exist on disk are registered into doc_map
    for comp in &task_companions {
        let clean = comp.path.replace('\\', "/");
        if !doc_map.contains_key(&clean) {
            let disk_path = root.join(&clean);
            if disk_path.exists() {
                let pass_sub = match normalized_subpath {
                    Some(ref sub) => clean == *sub || clean.starts_with(&format!("{}/", sub)),
                    None => true,
                };
                let lang = if clean.ends_with(".html") {
                    "html"
                } else if clean.ends_with(".json") {
                    "json"
                } else if clean.ends_with(".scss") || clean.ends_with(".css") {
                    "css"
                } else if clean.ends_with(".ts") {
                    "typescript"
                } else {
                    "text"
                };
                let pass_lang = match canonical_lang {
                    Some(tl) => lang == tl,
                    None => true,
                };
                if pass_sub && pass_lang {
                    if let Ok(content) = std::fs::read_to_string(&disk_path) {
                        doc_map.insert(clean.clone(), (lang.to_string(), content.clone()));
                        docs.push((clean.clone(), lang.to_string(), content));
                    }
                }
            }
        }

        if doc_map.contains_key(&clean) {
            let boost = match comp.kind {
                crate::frameworks::CompanionKind::StateAction
                | crate::frameworks::CompanionKind::StateEffect => 800.0,
                crate::frameworks::CompanionKind::StateReducer
                | crate::frameworks::CompanionKind::InjectedService => 750.0,
                crate::frameworks::CompanionKind::ComponentTemplate => 700.0,
                crate::frameworks::CompanionKind::TranslationDictionary => 650.0,
                crate::frameworks::CompanionKind::ModuleDeclaration => 600.0,
                crate::frameworks::CompanionKind::RelatedTest => 600.0,
                _ => 500.0,
            } * comp.confidence;
            *composite_scores.entry(clean.clone()).or_insert(0.0) += boost;
            let prov = format!(
                "Framework task companion [{}]: {}",
                comp.kind.as_str(),
                comp.reason
            );
            let list = provenance_map.entry(clean).or_default();
            if !list.contains(&prov) {
                list.push(prov);
            }
        }
    }

    // 4.4 Multi-Seed Framework Companion & Impact Cone Expansion
    let mut bm25_seeds: Vec<(String, f64)> = bm25_scores.into_iter().collect();
    bm25_seeds.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let is_container_module = |p: &str| -> bool {
        let l = p.to_ascii_lowercase();
        l.ends_with(".module.ts") || l.ends_with("/main.ts") || l.ends_with("/main.rs") || l.ends_with("/lib.rs")
    };

    let mut expansion_seeds: Vec<String> = Vec::new();
    for s in &symbol_seeds {
        if !expansion_seeds.contains(&s.0) {
            expansion_seeds.push(s.0.clone());
        }
    }
    for c in &task_companions {
        if !expansion_seeds.contains(&c.path) {
            expansion_seeds.push(c.path.clone());
        }
    }
    for s in bm25_seeds.iter().take(5) {
        if !expansion_seeds.contains(&s.0) {
            expansion_seeds.push(s.0.clone());
        }
    }

    for seed_path in &expansion_seeds {
        // A. Framework SPI Companion Expansion
        let companions = framework_provider.expand_companions(&root, &conn, seed_path);
        for comp in companions {
            let clean = comp.path.replace('\\', "/");
            if !doc_map.contains_key(&clean) {
                let disk_path = root.join(&clean);
                if disk_path.exists() {
                    let pass_sub = match normalized_subpath {
                        Some(ref sub) => clean == *sub || clean.starts_with(&format!("{}/", sub)),
                        None => true,
                    };
                    let lang = if clean.ends_with(".html") {
                        "html"
                    } else if clean.ends_with(".json") {
                        "json"
                    } else if clean.ends_with(".scss") || clean.ends_with(".css") {
                        "css"
                    } else if clean.ends_with(".ts") {
                        "typescript"
                    } else {
                        "text"
                    };
                    let pass_lang = match canonical_lang {
                        Some(tl) => lang == tl,
                        None => true,
                    };
                    if pass_sub && pass_lang {
                        if let Ok(content) = std::fs::read_to_string(&disk_path) {
                            doc_map.insert(clean.clone(), (lang.to_string(), content.clone()));
                            docs.push((clean.clone(), lang.to_string(), content));
                        }
                    }
                }
            }

            if doc_map.contains_key(&clean) {
                let boost = match comp.kind {
                    crate::frameworks::CompanionKind::StateAction
                    | crate::frameworks::CompanionKind::StateEffect => 500.0,
                    crate::frameworks::CompanionKind::StateReducer
                    | crate::frameworks::CompanionKind::InjectedService => 450.0,
                    crate::frameworks::CompanionKind::ComponentTemplate => 400.0,
                    crate::frameworks::CompanionKind::TranslationDictionary => 350.0,
                    crate::frameworks::CompanionKind::ModuleDeclaration => 300.0,
                    crate::frameworks::CompanionKind::RelatedTest => 400.0,
                    _ => 250.0,
                } * comp.confidence;
                *composite_scores.entry(clean.clone()).or_insert(0.0) += boost;
                let prov = format!(
                    "Framework companion [{}] via {}: {}",
                    comp.kind.as_str(),
                    seed_path,
                    comp.reason
                );
                let list = provenance_map.entry(clean).or_default();
                if !list.contains(&prov) {
                    list.push(prov);
                }
            }
        }

        // B. Impact Cone Traversal: Categorized ranking (Direct > Implementation > Test > Caller > ImportDependent)
        // Skip container modules (e.g. core.module.ts) to prevent unbounded multi-hop blast radius explosion
        if !is_container_module(seed_path) {
            if let Ok(cone) = crate::impact::compute_impact_cone(&conn, &root, seed_path, 2) {
                for entity in cone.all_entities() {
                    let clean_ent = entity.path.replace('\\', "/");
                    if doc_map.contains_key(&clean_ent) {
                        let cat_weight = match entity.category {
                            crate::impact::ImpactCategory::Direct => 15.0,
                            crate::impact::ImpactCategory::Implementation => 12.0,
                            crate::impact::ImpactCategory::Test => 8.0,
                            crate::impact::ImpactCategory::Caller => 7.0,
                            crate::impact::ImpactCategory::ImportDependent => 4.0,
                            crate::impact::ImpactCategory::Configuration => 3.5,
                            crate::impact::ImpactCategory::Historical => 2.5,
                        };
                        let decay = 1.0 / (1.0 + entity.hop_distance as f64);
                        let boost = cat_weight * decay;
                        *composite_scores.entry(clean_ent.clone()).or_insert(0.0) += boost;

                        let prov = format!(
                            "Impact {} (hop {}) via {} (reason: {})",
                            entity.category.as_str(),
                            entity.hop_distance,
                            seed_path,
                            entity.reason
                        );
                        let list = provenance_map.entry(clean_ent).or_default();
                        if !list.contains(&prov) {
                            list.push(prov);
                        }
                    }
                }
            }
        }

        // C. Graph neighbor expansion (IMPORTS, TESTS, IMPLEMENTS)
        if !is_container_module(seed_path) {
            if let Ok(neighbors) = traversal.neighbors(
                seed_path,
                &["IMPORTS", "TESTS", "IMPLEMENTS"],
                Direction::Both,
                2,
            ) {
                for edge in neighbors {
                    let neighbor_path = if edge.source == *seed_path
                        || edge.source.starts_with(&format!("{}::", seed_path))
                    {
                        edge.target.clone()
                    } else {
                        edge.source.clone()
                    };
                    let neighbor_file = neighbor_path.split("::").next().unwrap_or(&neighbor_path);
                    let clean_neighbor = neighbor_file.replace('\\', "/");

                    if doc_map.contains_key(&clean_neighbor) {
                        let boost = 3.0 * edge.confidence;
                        *composite_scores.entry(clean_neighbor.clone()).or_insert(0.0) += boost;

                        let prov = format!(
                            "Graph {} edge via {} (confidence: {:.2})",
                            edge.kind, seed_path, edge.confidence
                        );
                        let list = provenance_map.entry(clean_neighbor).or_default();
                        if !list.contains(&prov) {
                            list.push(prov);
                        }
                    }
                }
            }
        }
    }

    // 5. Deterministic Ranking: composite score descending, then path ascending
    let mut candidates: Vec<(String, f64)> = composite_scores.into_iter().collect();
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let count = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get::<_, usize>(0))?;
    let limit = budget;
    let mut used = 0;
    let mut selected = Vec::new();
    let mut excluded = Vec::new();
    let mut omissions = Vec::new();

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

        let mut syms_meta: Vec<(String, String, String, usize, usize, String)> = Vec::new();
        {
            let mut s = conn.prepare(
                "SELECT entity_id, name, kind, start_line, end_line, signature
                 FROM entity_versions
                 WHERE path=?1 AND invalidated_at_revision IS NULL
                 ORDER BY start_line",
            )?;
            for row in s.query_map([&path], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, usize>(3)?,
                    r.get::<_, usize>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })? {
                let (ent_id, name, kind, start_line, end_line, sig) = row?;
                all_selected_symbols.push(name.clone());
                syms_meta.push((ent_id, name, kind, start_line, end_line, sig));
            }
        }

        let body_lines: Vec<&str> = body.lines().collect();
        let remaining_tokens = limit.saturating_sub(used);
        let max_chars = remaining_tokens.saturating_mul(4).max(220);

        let mut content = format!("FILE {path} [{lang}]\n");

        if !syms_meta.is_empty() {
            content.push_str("SYMBOLS & CODE EXCERPTS:\n");
            let mut chars_available = max_chars.saturating_sub(content.len());

            for (ent_id, name, kind, start_line, end_line, sig) in &syms_meta {
                if chars_available < 60 {
                    content.push_str(&format!("- {kind} {name} (L{start_line}..L{end_line}): {sig} [TRUNCATED: Budget Exceeded]\n"));
                    continue;
                }

                let members = crate::storage::get_entity_members(&conn, ent_id).unwrap_or_default();
                if !members.is_empty() {
                    content.push_str(&format!("* {kind} {name} members:\n"));
                    for m in &members {
                        content.push_str(&format!(
                            "  - {} {} {}: {}\n",
                            if m.visibility.is_empty() {
                                "pub"
                            } else {
                                &m.visibility
                            },
                            m.member_kind,
                            m.name,
                            m.signature
                        ));
                    }
                }

                let start_idx = start_line.saturating_sub(1);
                let end_idx = (*end_line).min(body_lines.len());

                if start_idx < end_idx && start_idx < body_lines.len() {
                    let symbol_lines = &body_lines[start_idx..end_idx];
                    let raw_block = symbol_lines.join("\n");

                    if detail_level == crate::budget::DetailLevel::Contracted {
                        let (bounded_str, _status) = crate::analyzer::generate_bounded_excerpt(
                            &body_lines,
                            *start_line,
                            *end_line,
                            &members,
                            lang,
                        );
                        content.push_str(&format!(
                            "--- {kind} {name} (Lines {start_line}..{end_line}) ---\n{sig}\n{bounded_str}\n\n"
                        ));
                        chars_available = chars_available.saturating_sub(bounded_str.len() + 60);
                    } else if raw_block.len() <= chars_available {
                        content.push_str(&format!(
                            "--- {kind} {name} (Lines {start_line}..{end_line}) [COMPLETE_BLOCK] ---\n{sig}\n{raw_block}\n\n"
                        ));
                        chars_available = chars_available.saturating_sub(raw_block.len() + 60);
                    } else {
                        let mut partial = Vec::new();
                        let mut partial_len = 0;
                        for l in symbol_lines {
                            if partial_len + l.len() + 1 > chars_available.saturating_sub(120) {
                                break;
                            }
                            partial_len += l.len() + 1;
                            partial.push(*l);
                        }
                        if !partial.is_empty() {
                            let partial_count = partial.len();
                            content.push_str(&format!(
                                "--- {kind} {name} (Lines {start_line}..{}) [INCOMPLETE_EXCERPT: showing {} of {} lines] ---\n{sig}\n{}\n// [TRUNCATED to fit token budget]\n\n",
                                start_line + partial_count - 1,
                                partial_count,
                                symbol_lines.len(),
                                partial.join("\n")
                            ));
                            chars_available = chars_available.saturating_sub(partial_len + 120);
                        } else {
                            content.push_str(&format!("- {kind} {name} (L{start_line}..L{end_line}): {sig} [EXCERPT OMITTED: Budget Limit]\n"));
                        }
                    }
                } else {
                    content.push_str(&format!("- {kind} {name} (L{start_line}..L{end_line}): {sig}\n"));
                }
            }
        } else {
            content.push_str("SOURCE EXCERPT:\n");
            let chars_available = max_chars.saturating_sub(content.len());
            let mut partial = Vec::new();
            let mut partial_len = 0;
            for l in &body_lines {
                if partial_len + l.len() + 1 > chars_available.saturating_sub(100) {
                    break;
                }
                partial_len += l.len() + 1;
                partial.push(*l);
            }
            if partial.len() == body_lines.len() {
                content.push_str(&format!("Lines 1..{} [COMPLETE_BLOCK]:\n{}\n", body_lines.len(), body));
            } else {
                content.push_str(&format!(
                    "Lines 1..{} [INCOMPLETE_EXCERPT: showing {} of {} lines]:\n{}\n// [TRUNCATED to fit token budget]\n",
                    partial.len(),
                    partial.len(),
                    body_lines.len(),
                    partial.join("\n")
                ));
            }
        }

        let level = if score > 3.0 { 3 } else { 2 };

        let cost = estimate_tokens(&content);
        if used + cost > limit {
            excluded.push((
                path.clone(),
                format!(
                    "Budget limit exceeded (+{} tokens would exceed {})",
                    cost, limit
                ),
            ));
            let tier = if score >= 800.0 {
                1
            } else if score >= 300.0 {
                2
            } else {
                3
            };
            omissions.push(crate::budget::OmissionEntry {
                context_item_id: format!("file_{}", path),
                symbol: None,
                file: path,
                context_type: "FILE_CONTEXT".to_string(),
                priority_tier: tier,
                original_token_count: cost,
                included_token_count: 0,
                omitted_token_count: cost,
                omission_reason: format!(
                    "Budget ceiling reached in Tier {} (needed {} tokens, {} tokens remaining)",
                    tier,
                    cost,
                    limit.saturating_sub(used)
                ),
                omission_status: "COMPLETELY_OMITTED".to_string(),
                retrievable_via_targeted_request: true,
            });
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

    let mut gaps = if selected.is_empty() {
        if normalized_subpath.is_some() || canonical_lang.is_some() {
            vec![format!(
                "GAP-002: No files matched the scope filter (subpath: {:?}, language: {:?}) for task '{}'.",
                normalized_subpath, canonical_lang, task
            )]
        } else {
            vec!["GAP-001: No matching files or symbols were grounded for this task.".into()]
        }
    } else {
        Vec::new()
    };

    if confidence == "LOW" && !selected.is_empty() {
        gaps.push(format!(
            "UNCERTAINTY: File targeting confidence is LOW for task '{}'. Top candidates provided with provenance; verify target components before editing.",
            task
        ));
    }

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

    let budget_status = if limit == 0 {
        "CAPACITY_OVERFLOW".to_string()
    } else if !omissions.is_empty() {
        "TRUNCATED_AT_BUDGET".to_string()
    } else {
        "WITHIN_BUDGET".to_string()
    };

    let omitted_tokens: usize = omissions.iter().map(|o| o.omitted_token_count).sum();
    let total_serialized_bytes: usize = selected.iter().map(|f| f.content.len()).sum();

    let omission_manifest = crate::budget::OmissionManifest {
        budget_status,
        budget_tokens_limit: limit,
        budget_tokens_estimated: used,
        reserved_metadata_tokens: overhead_tokens,
        packed_tokens: used.saturating_sub(overhead_tokens),
        remaining_tokens: limit.saturating_sub(used),
        omitted_tokens,
        serialized_payload_bytes: total_serialized_bytes,
        included_items_count: selected.len(),
        omitted_items_count: omissions.len(),
        omission_manifest: omissions,
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
        constraints,
        relevant_memories,
        cochange_alerts,
        ecosystem_guard: eco_guard,
        gaps,
        files: selected,
        excluded_files: excluded,
        subpath_filter: normalized_subpath,
        language_filter: canonical_lang.map(String::from),
        omission_manifest: Some(omission_manifest),
    })
}

pub fn format_explain(res: &ContextResult) -> String {
    let mut out = String::new();
    out.push_str(&format!("🧠 Task: \"{}\"\n", res.task));
    if let Some(sub) = &res.subpath_filter {
        out.push_str(&format!("Scope Filter:  Subpath=\"{}\"\n", sub));
    }
    if let Some(lang) = &res.language_filter {
        out.push_str(&format!("Scope Filter:  Language=\"{}\"\n", lang));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_subpath_valid() {
        assert_eq!(normalize_subpath("src").unwrap(), "src");
        assert_eq!(normalize_subpath("src/").unwrap(), "src");
        assert!(normalize_subpath("/src/")
            .unwrap_err()
            .to_string()
            .contains("cannot be absolute"));
        assert_eq!(
            normalize_subpath("src/compiler.rs").unwrap(),
            "src/compiler.rs"
        );
        assert_eq!(
            normalize_subpath("benchmarks\\matched-pair-harness\\").unwrap(),
            "benchmarks/matched-pair-harness"
        );
    }

    #[test]
    fn test_normalize_subpath_traversal_rejection() {
        assert!(normalize_subpath("..").is_err());
        assert!(normalize_subpath("../src").is_err());
        assert!(normalize_subpath("src/../../etc").is_err());
        assert!(normalize_subpath("C:\\Windows").is_err());
        assert!(normalize_subpath("   ").is_err());
    }

    #[test]
    fn test_canonicalize_language() {
        assert_eq!(canonicalize_language("rust").unwrap(), "rust");
        assert_eq!(canonicalize_language("rs").unwrap(), "rust");
        assert_eq!(canonicalize_language("python").unwrap(), "python");
        assert_eq!(canonicalize_language("py").unwrap(), "python");
        assert_eq!(canonicalize_language("typescript").unwrap(), "typescript");
        assert_eq!(canonicalize_language("ts").unwrap(), "typescript");
        assert_eq!(canonicalize_language("tsx").unwrap(), "typescript");
        assert_eq!(canonicalize_language("javascript").unwrap(), "javascript");
        assert_eq!(canonicalize_language("js").unwrap(), "javascript");
        assert_eq!(canonicalize_language("c#").unwrap(), "csharp");
        assert_eq!(canonicalize_language("cs").unwrap(), "csharp");
        assert_eq!(canonicalize_language("go").unwrap(), "go");
        assert_eq!(canonicalize_language("java").unwrap(), "java");
        assert!(canonicalize_language("ruby").is_err());
        assert!(canonicalize_language("cpp").is_err());
    }
}
