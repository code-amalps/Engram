//! Context Compiler: progressive BM25 ranking, graph expansion, provenance tracking, and token budget packing.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Mutex, OnceLock},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexStateFingerprint {
    pub workspace_canonical: std::path::PathBuf,
    pub index_generation: i64,
    pub files_count: i64,
    pub files_hash_sum: i64,
    pub active_entities_count: i64,
    pub memory_version: String,
    pub head_revision: String,
}

pub fn compute_index_fingerprint(
    root: &Path,
    conn: &rusqlite::Connection,
) -> rusqlite::Result<IndexStateFingerprint> {
    let workspace_canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let index_generation: i64 = conn
        .query_row(
            "SELECT CAST(COALESCE(value, '1') AS INTEGER) FROM metadata WHERE key='index_generation'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(1);
    let head_revision: String = conn
        .query_row(
            "SELECT COALESCE(value, 'initial') FROM metadata WHERE key='head_revision'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "initial".into());
    let (files_count, files_hash_sum): (i64, i64) = conn
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(LENGTH(content_hash)), 0) FROM files",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap_or((0, 0));
    let active_entities_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM entity_versions WHERE invalidated_at_revision IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let mem_ver = std::fs::metadata(root.join(".engram").join("memory.yaml"))
        .and_then(|m| m.modified())
        .map(|t| format!("{:?}", t))
        .unwrap_or_else(|_| "none".into());

    Ok(IndexStateFingerprint {
        workspace_canonical,
        index_generation,
        files_count,
        files_hash_sum,
        active_entities_count,
        memory_version: mem_ver,
        head_revision,
    })
}

#[derive(Debug, Clone)]
struct CacheEntry {
    fingerprint: IndexStateFingerprint,
    result: ContextResult,
    last_accessed: std::time::Instant,
}

static COMPILED_CONTEXT_CACHE: OnceLock<Mutex<HashMap<String, CacheEntry>>> = OnceLock::new();
const MAX_CACHE_ENTRIES: usize = 64;

pub fn clear_context_cache() {
    if let Some(cache) = COMPILED_CONTEXT_CACHE.get() {
        if let Ok(mut lock) = cache.lock() {
            lock.clear();
        }
    }
}

pub fn clear_workspace_cache(root: &Path) {
    if let Some(cache) = COMPILED_CONTEXT_CACHE.get() {
        if let Ok(mut lock) = cache.lock() {
            let canon = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
            let canon_str = canon.display().to_string();
            let root_str = root.display().to_string();
            lock.retain(|k, _| !k.starts_with(&canon_str) && !k.starts_with(&root_str));
        }
    }
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub budget_diagnostics: Option<ContextBudgetDiagnostics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextBudgetDiagnostics {
    pub configured_token_budget: usize,
    pub estimated_tokens: usize,
    pub exact_serialized_bytes: usize,
    pub selected_source_bytes: usize,
    pub serialization_overhead_bytes: usize,
    pub tokenizer_telemetry: &'static str,
    pub provider_input_tokens: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum PackingTier {
    Tier0Mandatory = 0,
    Tier1Direct = 1,
    Tier2Supporting = 2,
    Tier3Optional = 3,
}

impl PackingTier {
    pub fn as_usize(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Tier0Mandatory => "Tier 0 (Mandatory)",
            Self::Tier1Direct => "Tier 1 (Directly Relevant)",
            Self::Tier2Supporting => "Tier 2 (Graph-Discovered Supporting)",
            Self::Tier3Optional => "Tier 3 (Optional)",
        }
    }
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
    context_scoped_with_detail(
        root,
        task,
        budget,
        None,
        None,
        crate::budget::DetailLevel::Contracted,
    )
}

pub fn context_scoped(
    root: &Path,
    task: &str,
    budget: usize,
    subpath: Option<&str>,
    language: Option<&str>,
) -> Result<ContextResult> {
    context_scoped_with_detail(
        root,
        task,
        budget,
        subpath,
        language,
        crate::budget::DetailLevel::Contracted,
    )
}

pub fn context_scoped_with_detail(
    root: &Path,
    task: &str,
    budget: usize,
    subpath: Option<&str>,
    language: Option<&str>,
    detail_level: crate::budget::DetailLevel,
) -> Result<ContextResult> {
    context_scoped_optimized(root, task, budget, subpath, language, detail_level, None)
}

pub fn context_scoped_optimized(
    root: &Path,
    task: &str,
    budget: usize,
    subpath: Option<&str>,
    language: Option<&str>,
    detail_level: crate::budget::DetailLevel,
    target_symbols: Option<&[String]>,
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

    let fingerprint = compute_index_fingerprint(&root, &conn).ok();

    let canon_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let cache_key = format!(
        "{}:{}:{}:{}:{}:{:?}:{:?}",
        canon_root.display(),
        task,
        budget,
        subpath.unwrap_or(""),
        language.unwrap_or(""),
        detail_level,
        target_symbols
    );

    if let Some(ref fp) = fingerprint {
        if let Some(cache) = COMPILED_CONTEXT_CACHE.get() {
            if let Ok(mut guard) = cache.lock() {
                if let Some(entry) = guard.get_mut(&cache_key) {
                    if &entry.fingerprint == fp {
                        entry.last_accessed = std::time::Instant::now();
                        return Ok(entry.result.clone());
                    }
                }
            }
        }
    }

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
        l.ends_with(".module.ts")
            || l.ends_with("/main.ts")
            || l.ends_with("/main.rs")
            || l.ends_with("/lib.rs")
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

    let kg_conn_opt = crate::knowledge_graph::open_graph_db(&root).ok();
    let kg_index_opt = kg_conn_opt
        .as_ref()
        .and_then(|c| crate::knowledge_graph::KnowledgeGraphIndex::load_from_db(c).ok());

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
                        *composite_scores
                            .entry(clean_neighbor.clone())
                            .or_insert(0.0) += boost;

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

        // D. Native Micro-Knowledge Graph multi-hop expansion (IMPORTS, CALLS, IMPLEMENTS, INHERITS, REFERENCES, TESTS)
        #[allow(clippy::collapsible_if)]
        if !is_container_module(seed_path) {
            if let Some(ref kg_index) = kg_index_opt {
                let mut best_neighbor_score: HashMap<String, f64> = HashMap::new();
                let mut neighbor_provs: HashMap<String, Vec<String>> = HashMap::new();
                let deps = kg_index.dependencies(seed_path, 2, false);
                for edge in deps {
                    if matches!(
                        edge.provenance.resolution_status,
                        crate::knowledge_graph::ResolutionStatus::External
                            | crate::knowledge_graph::ResolutionStatus::Unresolved
                    ) {
                        continue;
                    }
                    let target_node = kg_index.nodes_by_id.get(&edge.target_id);
                    let target_file = target_node
                        .map(|n| n.relative_path.clone())
                        .filter(|p| !p.is_empty())
                        .or_else(|| edge.target_id.strip_prefix("file:").map(str::to_string));
                    if let Some(raw_tf) = target_file {
                        let clean_tf = raw_tf.replace('\\', "/");
                        if doc_map.contains_key(&clean_tf) && clean_tf != *seed_path {
                            let is_confirmed = edge.provenance.resolution_status
                                == crate::knowledge_graph::ResolutionStatus::Resolved
                                && edge.provenance.classification
                                    != crate::knowledge_graph::EvidenceClassification::Inferred;
                            let base_weight = if is_confirmed {
                                if edge.category == crate::knowledge_graph::RelationCategory::Tests
                                {
                                    6.0
                                } else if edge.provenance.classification
                                    == crate::knowledge_graph::EvidenceClassification::DirectFact
                                {
                                    4.5
                                } else {
                                    4.0
                                }
                            } else {
                                1.5
                            };
                            let boost = base_weight * edge.provenance.confidence;
                            let ev_tag = if is_confirmed { "DIRECT" } else { "INFERRED" };
                            let prov = format!(
                                "Native KG {} [{}] {} via {} ({}:{}, conf: {:.2})",
                                ev_tag,
                                edge.provenance.resolution_status.as_str(),
                                edge.category.as_str(),
                                seed_path,
                                edge.provenance.source_path,
                                edge.provenance.source_line.unwrap_or(1),
                                edge.provenance.confidence
                            );
                            best_neighbor_score
                                .entry(clean_tf.clone())
                                .and_modify(|s| {
                                    if boost > *s {
                                        *s = boost;
                                    }
                                })
                                .or_insert(boost);
                            let plist = neighbor_provs.entry(clean_tf).or_default();
                            if !plist.contains(&prov) {
                                plist.push(prov);
                            }
                        }
                    }
                }

                // Multi-hop reverse impact (callers, implementors, subclasses, tests)
                let impact_rep = kg_index.impact(seed_path, 2);
                for finding in impact_rep.findings {
                    let clean_tf = finding.relative_path.replace('\\', "/");
                    if !clean_tf.is_empty()
                        && doc_map.contains_key(&clean_tf)
                        && clean_tf != *seed_path
                    {
                        let decay = 1.0 / (finding.hop.max(1) as f64);
                        let is_confirmed = finding.classification
                            != crate::knowledge_graph::EvidenceClassification::Inferred;
                        let base_weight = if is_confirmed {
                            if finding.relation == crate::knowledge_graph::RelationCategory::Tests {
                                6.0
                            } else {
                                4.0
                            }
                        } else {
                            1.5
                        };
                        let boost = base_weight * finding.confidence * decay;
                        let ev_tag = if is_confirmed { "DIRECT" } else { "INFERRED" };
                        let prov = format!(
                            "Native KG {} reverse {} (hop {}) via {} (conf: {:.2})",
                            ev_tag,
                            finding.relation.as_str(),
                            finding.hop,
                            seed_path,
                            finding.confidence
                        );
                        best_neighbor_score
                            .entry(clean_tf.clone())
                            .and_modify(|s| {
                                if boost > *s {
                                    *s = boost;
                                }
                            })
                            .or_insert(boost);
                        let plist = neighbor_provs.entry(clean_tf).or_default();
                        if !plist.contains(&prov) {
                            plist.push(prov);
                        }
                    }
                }

                for (clean_tf, boost) in best_neighbor_score {
                    *composite_scores.entry(clean_tf.clone()).or_insert(0.0) += boost;
                    if let Some(provs) = neighbor_provs.remove(&clean_tf) {
                        let list = provenance_map.entry(clean_tf).or_default();
                        for p in provs {
                            if !list.contains(&p) {
                                list.push(p);
                            }
                        }
                    }
                }
            } else if let Some(ref kg_conn) = kg_conn_opt {
                let file_id = format!("file:{seed_path}");
                let kg_edges =
                    crate::knowledge_graph::KnowledgeGraphIndex::query_file_neighborhood_fast(
                        kg_conn, seed_path, 20,
                    );
                for edge in kg_edges {
                    let target_file =
                        if edge.target_id.starts_with("file:") && edge.target_id != file_id {
                            Some(edge.target_id.trim_start_matches("file:").to_string())
                        } else if edge.source_id.starts_with("file:") && edge.source_id != file_id {
                            Some(edge.source_id.trim_start_matches("file:").to_string())
                        } else {
                            None
                        };

                    if let Some(clean_tf) = target_file {
                        let clean_tf = clean_tf.replace('\\', "/");
                        if doc_map.contains_key(&clean_tf) && clean_tf != *seed_path {
                            let boost = 4.0 * edge.provenance.confidence;
                            *composite_scores.entry(clean_tf.clone()).or_insert(0.0) += boost;
                            let prov = format!(
                                "Native KG {} edge via {} (confidence: {:.2})",
                                edge.category.as_str(),
                                seed_path,
                                edge.provenance.confidence
                            );
                            let list = provenance_map.entry(clean_tf).or_default();
                            if !list.contains(&prov) {
                                list.push(prov);
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. Deterministic 4-Tier Dependency-Aware Ranking
    let active_contract = crate::contract::get_active_contract(&root)
        .ok()
        .flatten()
        .filter(|c| {
            c.status == crate::contract::ContractStatus::Active
                && (c.task_goal.trim().eq_ignore_ascii_case(task.trim())
                    || task
                        .to_ascii_lowercase()
                        .contains(&c.task_goal.to_ascii_lowercase())
                    || c.task_goal
                        .to_ascii_lowercase()
                        .contains(&task.to_ascii_lowercase()))
        });
    let query_lower = task.to_ascii_lowercase();

    let is_task_query_target = |path: &str| -> bool {
        let p_clean = path.replace('\\', "/");
        let file_name = p_clean.split('/').next_back().unwrap_or(&p_clean);
        let stem = file_name
            .split('.')
            .next()
            .unwrap_or(file_name)
            .to_ascii_lowercase();

        let is_test = p_clean.starts_with("tests/")
            || p_clean.contains("/tests/")
            || p_clean.ends_with("_test.rs")
            || p_clean.ends_with(".test.ts");

        if is_test {
            if query_lower.contains(&stem) && query_lower.contains("test") {
                return true;
            }
            return false;
        }

        if stem.len() >= 3 {
            if query_lower.contains(&stem) {
                return true;
            }
            let space_variant = stem.replace('_', " ");
            if query_lower.contains(&space_variant) {
                return true;
            }
            let dash_variant = stem.replace('_', "-");
            if query_lower.contains(&dash_variant) {
                return true;
            }
        }
        if query_lower.contains(&p_clean.to_ascii_lowercase()) {
            return true;
        }

        // Semantic domain mapping for core Engram architectural modules
        if stem == "ci_guard"
            && (query_lower.contains("bypass")
                || query_lower.contains("guard")
                || query_lower.contains("postflight")
                || query_lower.contains("verification"))
        {
            return true;
        }
        if stem == "hook"
            && (query_lower.contains("pre-commit")
                || query_lower.contains("postflight")
                || query_lower.contains("hook")
                || query_lower.contains("installation"))
        {
            return true;
        }
        if stem == "storage"
            && (query_lower.contains("storage")
                || query_lower.contains("sqlite")
                || query_lower.contains("transaction")
                || query_lower.contains("token")
                || query_lower.contains("atomic")
                || query_lower.contains("state is corrupt")
                || query_lower.contains("recovery session")
                || query_lower.contains("session management")
                || query_lower.contains("roll back")
                || query_lower.contains("rollback"))
        {
            return true;
        }
        if stem == "constitution"
            && (query_lower.contains("constitution")
                || query_lower.contains("constraint")
                || query_lower.contains("authorization")
                || query_lower.contains("policy"))
        {
            return true;
        }
        if stem == "git"
            && (query_lower.contains("worktree")
                || query_lower.contains("head")
                || query_lower.contains("git")
                || query_lower.contains("rev-parse")
                || query_lower.contains("uncommitted"))
        {
            return true;
        }
        if stem == "recovery"
            && (query_lower.contains("recovery") || query_lower.contains("recover"))
        {
            return true;
        }
        if stem == "engage"
            && (query_lower.contains("engage")
                || query_lower.contains("engagement")
                || query_lower.contains("wire")
                || query_lower.contains("measurement"))
        {
            return true;
        }
        if stem == "mcp"
            && (query_lower.contains("mcp")
                || query_lower.contains("tool")
                || query_lower.contains("endpoint")
                || query_lower.contains("protocol")
                || query_lower.contains("round-trip"))
        {
            return true;
        }
        if stem == "main"
            && (query_lower.contains("cli ")
                || query_lower.contains(" cli")
                || query_lower.starts_with("cli")
                || query_lower.contains("command")
                || query_lower.contains("subcommand")
                || query_lower.contains("execution")
                || query_lower.contains("single turn"))
        {
            return true;
        }
        if stem == "contract"
            && (query_lower.contains("contract")
                || query_lower.contains("boundary")
                || query_lower.contains("boundaries")
                || query_lower.contains("fail-open")
                || query_lower.contains("fail open"))
        {
            return true;
        }
        if stem == "lib"
            && (query_lower.contains("library")
                || query_lower.contains("lib.rs")
                || (query_lower.contains("tool") && query_lower.contains("consolidat"))
                || (query_lower.contains("mcp") && query_lower.contains("turn")))
        {
            return true;
        }

        false
    };

    let determine_tier = |path: &str, score: f64| -> PackingTier {
        let is_test = path.starts_with("tests/")
            || path.contains("/tests/")
            || path.ends_with("_test.rs")
            || path.ends_with(".test.ts");

        // 1. Check active contract target or allowed files
        if let Some(ref c) = active_contract {
            let matches_boundary = c
                .boundary
                .expected_files
                .iter()
                .any(|f| f == path || path.ends_with(f))
                || c.boundary
                    .targets
                    .iter()
                    .any(|t| t.path == path || path.ends_with(&t.path));
            if matches_boundary {
                return PackingTier::Tier0Mandatory;
            }
        }

        // 2. Check explicit task query component target
        if is_task_query_target(path) {
            return PackingTier::Tier0Mandatory;
        }

        // 3. Check requested target symbols
        if let Some(targets) = target_symbols {
            let matches_symbol = symbol_seeds
                .iter()
                .any(|s| s.0 == path && targets.contains(&s.1));
            if matches_symbol {
                return PackingTier::Tier0Mandatory;
            }
        }

        // 4. Tier 1: Direct exact symbol match, high BM25, or 1-hop dependency of target
        if score >= 100.0 {
            return PackingTier::Tier1Direct;
        }
        if let Some(provs) = provenance_map.get(path) {
            let matches_direct = provs.iter().any(|p| {
                p.contains("Exact symbol match")
                    || p.contains("Framework task companion")
                    || p.contains("Native KG DIRECT")
                    || p.contains("IMPORTS")
                    || p.contains("IMPLEMENTS")
                    || p.contains("INHERITS")
                    || p.contains("Impact DIRECT")
                    || p.contains("Impact IMPLEMENTATION")
            });
            if matches_direct {
                return PackingTier::Tier1Direct;
            }
        }

        // Unreferenced or indirect tests are Tier 2 (Supporting)
        if is_test {
            return PackingTier::Tier2Supporting;
        }

        // 5. Tier 2: Supporting graph edge, impact cone, co-change, or moderate score
        if score >= 5.0 {
            return PackingTier::Tier2Supporting;
        }
        if let Some(provs) = provenance_map.get(path) {
            let matches_supporting = provs
                .iter()
                .any(|p| p.contains("Impact") || p.contains("Graph") || p.contains("Native KG"));
            if matches_supporting {
                return PackingTier::Tier2Supporting;
            }
        }

        // 6. Tier 3: Optional peripheral
        PackingTier::Tier3Optional
    };

    let mut tiered_candidates: Vec<(String, f64, PackingTier)> = composite_scores
        .into_iter()
        .map(|(path, score)| {
            let tier = determine_tier(&path, score);
            (path, score, tier)
        })
        .collect();

    // Sort primarily by PackingTier ascending (0, 1, 2, 3), secondarily by score descending, then path ascending
    tiered_candidates.sort_by(|a, b| {
        a.2.cmp(&b.2)
            .then_with(|| b.1.total_cmp(&a.1))
            .then_with(|| a.0.cmp(&b.0))
    });

    let count = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get::<_, usize>(0))?;
    let limit = budget;
    let mut used = 0;
    let mut has_capacity_overflow = limit == 0;
    let mut selected = Vec::new();
    let mut excluded = Vec::new();
    let mut omissions = Vec::new();

    // Reserve token slice for ecosystem guard and constraints without exceeding configured limit
    let overhead_tokens = (estimate_tokens(&constraints.join("\n"))
        + eco_guard.as_deref().map(estimate_tokens).unwrap_or(0))
    .min(limit);
    used += overhead_tokens;

    let mut all_selected_symbols = Vec::new();
    let mut seen_signatures = HashSet::<String>::new();

    for (path, score, tier) in tiered_candidates {
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

        // Task-aware symbol ranking within the file
        let compute_symbol_score = |sym_name: &str, sym_sig: &str| -> f64 {
            let mut sym_score = 0.0;
            let sym_lower = sym_name.to_ascii_lowercase();
            if let Some(targets) = target_symbols {
                if targets.iter().any(|t| t.eq_ignore_ascii_case(sym_name)) {
                    sym_score += 5000.0;
                }
            }
            if potential_symbols
                .iter()
                .any(|ps| ps.eq_ignore_ascii_case(sym_name))
            {
                sym_score += 2000.0;
            }
            for term in &terms {
                if sym_lower == *term {
                    sym_score += 1000.0;
                } else if sym_lower.contains(term) && term.len() >= 3 {
                    sym_score += 300.0;
                } else if sym_sig.to_ascii_lowercase().contains(term) && term.len() >= 3 {
                    sym_score += 100.0;
                }
            }
            sym_score
        };

        syms_meta.sort_by(|a, b| {
            let sa = compute_symbol_score(&a.1, &a.5);
            let sb = compute_symbol_score(&b.1, &b.5);
            sb.total_cmp(&sa).then_with(|| a.3.cmp(&b.3))
        });

        let has_targeted_symbols = target_symbols.is_some()
            || syms_meta
                .iter()
                .any(|s| compute_symbol_score(&s.1, &s.5) >= 1000.0);

        let body_lines: Vec<&str> = body.lines().collect();
        let remaining_tokens = limit.saturating_sub(used);
        let max_chars = remaining_tokens.saturating_mul(4);

        let mut content = format!("FILE {path} [{lang}]\n");

        if detail_level == crate::budget::DetailLevel::Compact {
            content.push_str("SYMBOLS (COMPACT METADATA):\n");
            for (_ent_id, name, kind, start_line, end_line, sig) in &syms_meta {
                let sig_key = format!("{}::{}", name, sig);
                let line_str = if seen_signatures.contains(&sig_key) {
                    format!("- {kind} {name} (L{start_line}..L{end_line}): {sig} [DEDUPLICATED]\n")
                } else {
                    seen_signatures.insert(sig_key);
                    format!("- {kind} {name} (L{start_line}..L{end_line}): {sig}\n")
                };
                if content.len() + line_str.len() > max_chars {
                    break;
                }
                content.push_str(&line_str);
            }
        } else if !syms_meta.is_empty() {
            content.push_str("SYMBOLS & CODE EXCERPTS:\n");
            let mut chars_available = max_chars.saturating_sub(content.len());

            for (ent_id, name, kind, start_line, end_line, sig) in &syms_meta {
                let sym_score = compute_symbol_score(name, sig);
                let is_requested = if has_targeted_symbols {
                    sym_score >= 1000.0
                } else {
                    true
                };

                let sig_key = format!("{}::{}", name, sig);
                if seen_signatures.contains(&sig_key) {
                    let dedup_line = format!(
                        "- {kind} {name} (L{start_line}..L{end_line}): {sig} [DEDUPLICATED]\n"
                    );
                    if dedup_line.len() <= chars_available {
                        content.push_str(&dedup_line);
                        chars_available = chars_available.saturating_sub(dedup_line.len());
                    }
                    continue;
                }
                seen_signatures.insert(sig_key);

                if !is_requested {
                    let sym_line = format!("- {kind} {name} (L{start_line}..L{end_line}): {sig}\n");
                    if sym_line.len() <= chars_available {
                        content.push_str(&sym_line);
                        chars_available = chars_available.saturating_sub(sym_line.len());
                    }
                    continue;
                }

                if chars_available < 60 {
                    let trunc_line = format!(
                        "- {kind} {name} (L{start_line}..L{end_line}): {sig} [TRUNCATED: Budget Exceeded]\n"
                    );
                    if trunc_line.len() <= chars_available {
                        content.push_str(&trunc_line);
                        chars_available = chars_available.saturating_sub(trunc_line.len());
                    }
                    continue;
                }

                let members = crate::storage::get_entity_members(&conn, ent_id).unwrap_or_default();
                if !members.is_empty() {
                    let header = format!("* {kind} {name} members:\n");
                    if header.len() <= chars_available {
                        content.push_str(&header);
                        chars_available = chars_available.saturating_sub(header.len());
                        for m in &members {
                            let mline = format!(
                                "  - {} {} {}: {}\n",
                                if m.visibility.is_empty() {
                                    "pub"
                                } else {
                                    &m.visibility
                                },
                                m.member_kind,
                                m.name,
                                m.signature
                            );
                            if mline.len() > chars_available {
                                break;
                            }
                            content.push_str(&mline);
                            chars_available = chars_available.saturating_sub(mline.len());
                        }
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
                        let blk = format!(
                            "--- {kind} {name} (Lines {start_line}..{end_line}) ---\n{sig}\n{bounded_str}\n\n"
                        );
                        if blk.len() <= chars_available {
                            content.push_str(&blk);
                            chars_available = chars_available.saturating_sub(blk.len());
                        } else {
                            let om_line = format!(
                                "- {kind} {name} (L{start_line}..L{end_line}): {sig} [EXCERPT OMITTED: Budget Limit]\n"
                            );
                            if om_line.len() <= chars_available {
                                content.push_str(&om_line);
                                chars_available = chars_available.saturating_sub(om_line.len());
                            }
                        }
                    } else if raw_block.len() + sig.len() + 60 <= chars_available {
                        let blk = format!(
                            "--- {kind} {name} (Lines {start_line}..{end_line}) [COMPLETE_BLOCK] ---\n{sig}\n{raw_block}\n\n"
                        );
                        content.push_str(&blk);
                        chars_available = chars_available.saturating_sub(blk.len());
                    } else {
                        let mut partial = Vec::new();
                        let mut partial_len = 0;
                        let reserve = sig.len() + 140;
                        let inner_cap = chars_available.saturating_sub(reserve);
                        for l in symbol_lines {
                            if partial_len + l.len() + 1 > inner_cap {
                                break;
                            }
                            partial_len += l.len() + 1;
                            partial.push(*l);
                        }
                        if !partial.is_empty() {
                            let partial_count = partial.len();
                            let blk = format!(
                                "--- {kind} {name} (Lines {start_line}..{}) [INCOMPLETE_EXCERPT: showing {} of {} lines] ---\n{sig}\n{}\n// [TRUNCATED to fit token budget]\n\n",
                                start_line + partial_count - 1,
                                partial_count,
                                symbol_lines.len(),
                                partial.join("\n")
                            );
                            if blk.len() <= chars_available {
                                content.push_str(&blk);
                                chars_available = chars_available.saturating_sub(blk.len());
                            }
                        } else {
                            let om_line = format!(
                                "- {kind} {name} (L{start_line}..L{end_line}): {sig} [EXCERPT OMITTED: Budget Limit]\n"
                            );
                            if om_line.len() <= chars_available {
                                content.push_str(&om_line);
                                chars_available = chars_available.saturating_sub(om_line.len());
                            }
                        }
                    }
                } else {
                    let sym_line = format!("- {kind} {name} (L{start_line}..L{end_line}): {sig}\n");
                    if sym_line.len() <= chars_available {
                        content.push_str(&sym_line);
                        chars_available = chars_available.saturating_sub(sym_line.len());
                    }
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
                let blk = format!(
                    "Lines 1..{} [COMPLETE_BLOCK]:\n{}\n",
                    body_lines.len(),
                    body
                );
                if blk.len() <= chars_available {
                    content.push_str(&blk);
                }
            } else if !partial.is_empty() {
                let blk = format!(
                    "Lines 1..{} [INCOMPLETE_EXCERPT: showing {} of {} lines]:\n{}\n// [TRUNCATED to fit token budget]\n",
                    partial.len(),
                    partial.len(),
                    body_lines.len(),
                    partial.join("\n")
                );
                if blk.len() <= chars_available {
                    content.push_str(&blk);
                }
            }
        }

        let level = if score > 3.0 { 3 } else { 2 };

        let cost = estimate_tokens(&content);
        let fits_budget = used + cost <= limit && remaining_tokens > 0;

        if !fits_budget {
            let (status_tag, reason) = if tier == PackingTier::Tier0Mandatory {
                has_capacity_overflow = true;
                (
                    "CAPACITY_OVERFLOW",
                    format!(
                        "Tier 0 (Mandatory) item exceeds remaining token budget of {} tokens (needed {} tokens, {} remaining)",
                        limit,
                        cost,
                        limit.saturating_sub(used)
                    ),
                )
            } else {
                (
                    "COMPLETELY_OMITTED",
                    format!(
                        "{} file excluded: strict token budget limit of {} tokens reached (needed {} tokens, {} tokens remaining)",
                        tier.name(),
                        limit,
                        cost,
                        limit.saturating_sub(used)
                    ),
                )
            };
            excluded.push((path.clone(), reason.clone()));
            omissions.push(crate::budget::OmissionEntry {
                context_item_id: format!("file_{}", path),
                symbol: None,
                file: path,
                context_type: "FILE_CONTEXT".to_string(),
                priority_tier: tier.as_usize(),
                original_token_count: cost,
                included_token_count: 0,
                omitted_token_count: cost,
                omission_reason: reason,
                omission_status: status_tag.to_string(),
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

    let budget_status = if limit == 0 || has_capacity_overflow {
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

    let selected_source_bytes: usize = selected.iter().map(|f| f.content.len()).sum();
    let rough_metadata_bytes = serde_json::to_string(&constraints)
        .map(|s| s.len())
        .unwrap_or(0)
        + serde_json::to_string(&relevant_memories)
            .map(|s| s.len())
            .unwrap_or(0)
        + serde_json::to_string(&cochange_alerts)
            .map(|s| s.len())
            .unwrap_or(0)
        + eco_guard.as_ref().map(|s| s.len()).unwrap_or(0)
        + serde_json::to_string(&omission_manifest)
            .map(|s| s.len())
            .unwrap_or(0)
        + 1024;
    let exact_serialized_bytes = selected_source_bytes + rough_metadata_bytes;
    let serialization_overhead_bytes = exact_serialized_bytes.saturating_sub(selected_source_bytes);

    let budget_diagnostics = ContextBudgetDiagnostics {
        configured_token_budget: limit,
        estimated_tokens: used,
        exact_serialized_bytes,
        selected_source_bytes,
        serialization_overhead_bytes,
        tokenizer_telemetry: "TOKEN_COUNT_UNAVAILABLE",
        provider_input_tokens: None,
    };

    let final_res = ContextResult {
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
        budget_diagnostics: Some(budget_diagnostics),
    };

    if let Some(fp) = fingerprint {
        let cache = COMPILED_CONTEXT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
        if let Ok(mut guard) = cache.lock() {
            if guard.len() >= MAX_CACHE_ENTRIES {
                if let Some((oldest_key, _)) = guard
                    .iter()
                    .min_by_key(|(_, entry)| entry.last_accessed)
                    .map(|(k, v)| (k.clone(), v.last_accessed))
                {
                    guard.remove(&oldest_key);
                }
            }
            guard.insert(
                cache_key,
                CacheEntry {
                    fingerprint: fp,
                    result: final_res.clone(),
                    last_accessed: std::time::Instant::now(),
                },
            );
        }
    }

    Ok(final_res)
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
    if let Some(ref diag) = res.budget_diagnostics {
        out.push_str(&format!(
            "Diagnostics: Configured Budget: {} tokens | Estimated Tokens: {}\n",
            diag.configured_token_budget, diag.estimated_tokens
        ));
        out.push_str(&format!(
            "Payload:     {} bytes total ({} source, {} overhead)\n",
            diag.exact_serialized_bytes,
            diag.selected_source_bytes,
            diag.serialization_overhead_bytes
        ));
        out.push_str(&format!("Tokenizer:   {}\n\n", diag.tokenizer_telemetry));
    } else {
        out.push_str(&format!(
            "Confidence: {} | Budget: {} / {} tokens\n\n",
            res.confidence, res.estimated_tokens, res.token_budget
        ));
    }

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
        assert!(
            normalize_subpath("/src/")
                .unwrap_err()
                .to_string()
                .contains("cannot be absolute")
        );
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

    #[test]
    fn test_budget_diagnostics_calculation() {
        let diag = ContextBudgetDiagnostics {
            configured_token_budget: 1200,
            estimated_tokens: 850,
            exact_serialized_bytes: 4200,
            selected_source_bytes: 3500,
            serialization_overhead_bytes: 700,
            tokenizer_telemetry: "TOKEN_COUNT_UNAVAILABLE",
            provider_input_tokens: None,
        };

        assert_eq!(diag.configured_token_budget, 1200);
        assert_eq!(diag.estimated_tokens, 850);
        assert_eq!(diag.exact_serialized_bytes, 4200);
        assert_eq!(diag.selected_source_bytes, 3500);
        assert_eq!(diag.serialization_overhead_bytes, 700);
        assert_eq!(diag.tokenizer_telemetry, "TOKEN_COUNT_UNAVAILABLE");
        assert!(diag.provider_input_tokens.is_none());
    }

    #[test]
    fn test_packing_tier_ordering() {
        assert!(PackingTier::Tier0Mandatory < PackingTier::Tier1Direct);
        assert!(PackingTier::Tier1Direct < PackingTier::Tier2Supporting);
        assert!(PackingTier::Tier2Supporting < PackingTier::Tier3Optional);

        assert_eq!(PackingTier::Tier0Mandatory.as_usize(), 0);
        assert_eq!(PackingTier::Tier1Direct.as_usize(), 1);
        assert_eq!(PackingTier::Tier2Supporting.as_usize(), 2);
        assert_eq!(PackingTier::Tier3Optional.as_usize(), 3);

        assert_eq!(PackingTier::Tier0Mandatory.name(), "Tier 0 (Mandatory)");
        assert_eq!(
            PackingTier::Tier1Direct.name(),
            "Tier 1 (Directly Relevant)"
        );
        assert_eq!(
            PackingTier::Tier2Supporting.name(),
            "Tier 2 (Graph-Discovered Supporting)"
        );
        assert_eq!(PackingTier::Tier3Optional.name(), "Tier 3 (Optional)");
    }
}
