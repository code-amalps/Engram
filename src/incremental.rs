//! Incremental Analysis Engine:
//! Two-tier invalidation pipeline:
//! - Tier 1 (Syntactic): Content/blob hash is authoritative (mtime/size as fast-path hint)
//!   to identify modified, added, deleted, or renamed files. Re-parse only those files.
//! - Tier 2 (Relational): Query SQLite for entities that import, call, or implement symbols
//!   defined in Tier 1 files, and re-link their graph edges.
//! - Fallback: Safe full-analysis fallback when incremental correctness cannot be established.

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
    time::Instant,
};
use uuid::Uuid;

use crate::analyzer::{excluded, ignore_patterns, language, parse_file, reconcile_symbol_ids};
use crate::model;
use crate::storage::{config_hash, db, hash, now, revision, root_path};

pub const PROTOCOL_VERSION: &str = "1.1";

fn default_protocol_version() -> String {
    PROTOCOL_VERSION.to_string()
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
pub struct IncrementalOptions {
    pub force_content_hashing: bool,
    pub fallback_on_error: bool,
}

impl Default for IncrementalOptions {
    fn default() -> Self {
        Self {
            force_content_hashing: true,
            fallback_on_error: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalAnalysisResult {
    #[serde(default = "default_protocol_version")]
    pub protocol_version: String,
    pub revision: String,
    pub total_files: usize,
    pub tier1_reparsed_files: Vec<String>,
    pub tier1_deleted_files: Vec<String>,
    #[serde(default)]
    pub tier1_renamed_files: Vec<(String, String)>,
    pub tier2_relinked_files: Vec<String>,
    pub tier2_relinked_entities: usize,
    pub unchanged_files_count: usize,
    pub fast_path_hits: usize,
    #[serde(default)]
    pub fallback_full_analysis: bool,
    #[serde(default)]
    pub diagnostics: Vec<String>,
    pub duration_ms: f64,
    pub status: String,
}

/// Initialize the file metadata cache table in SQLite.
pub fn init_meta_cache(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS file_meta_cache (
            path TEXT PRIMARY KEY,
            mtime_ns INTEGER NOT NULL,
            size_bytes INTEGER NOT NULL,
            content_hash TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_file_meta_cache_path ON file_meta_cache(path);",
    )?;
    Ok(())
}

/// Fast-path file hash hint lookup:
/// Returns `(content_hash, bytes, is_fast_path)`.
/// - If `mtime` and `size_bytes` match cached metadata and `untrusted_mtime` is false,
///   the cached content hash is returned as an optimization hint (`is_fast_path = true`, empty bytes).
/// - Otherwise, file bytes are read directly, SHA-256 computed authoritatively, and metadata cache is refreshed (`is_fast_path = false`).
///
/// NOTE: Modification time and file size are optimization hints, never proof that content is unchanged.
/// A cached hash must not be described as authoritative if the underlying bytes were not verified.
/// For authoritative freshness guarantees, actual bytes are read and hashed via SHA-256.
pub fn get_authoritative_content_hash(
    abs_path: &Path,
    rel_path: &str,
    conn: &Connection,
) -> Result<(String, Vec<u8>, bool)> {
    get_authoritative_content_hash_internal(abs_path, rel_path, conn, false)
}

pub fn get_authoritative_content_hash_internal(
    abs_path: &Path,
    rel_path: &str,
    conn: &Connection,
    untrusted_mtime: bool,
) -> Result<(String, Vec<u8>, bool)> {
    let metadata = fs::metadata(abs_path)
        .with_context(|| format!("Failed to read metadata for {}", abs_path.display()))?;

    let size_bytes = metadata.len();
    let mtime_ns = match metadata.modified() {
        Ok(t) => {
            let dur = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
            dur.as_nanos() as i64
        }
        Err(_) => 0,
    };

    // Fast-path hint check (only if mtime is trusted)
    if !untrusted_mtime {
        let cached: Option<String> = conn
            .query_row(
                "SELECT content_hash FROM file_meta_cache WHERE path = ?1 AND mtime_ns = ?2 AND size_bytes = ?3",
                params![rel_path, mtime_ns, size_bytes as i64],
                |r| r.get(0),
            )
            .optional()?;

        if let Some(h) = cached {
            return Ok((h, Vec::new(), true));
        }
    }

    // Fast path miss or untrusted: read bytes and compute authoritative hash
    let bytes = fs::read(abs_path)
        .with_context(|| format!("Failed to read content of {}", abs_path.display()))?;
    let content_hash = hash(&bytes);

    conn.execute(
        "INSERT INTO file_meta_cache (path, mtime_ns, size_bytes, content_hash, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(path) DO UPDATE SET
            mtime_ns = excluded.mtime_ns,
            size_bytes = excluded.size_bytes,
            content_hash = excluded.content_hash,
            updated_at = excluded.updated_at",
        params![rel_path, mtime_ns, size_bytes as i64, content_hash, now()],
    )?;

    Ok((content_hash, bytes, false))
}

/// Query SQLite for entities and files that import, call, or implement symbols defined in Tier 1 files.
pub fn find_tier2_relational_dependents(
    conn: &Connection,
    tier1_files: &[String],
    tier1_symbol_names: &[String],
) -> Result<Vec<String>> {
    if tier1_files.is_empty() && tier1_symbol_names.is_empty() {
        return Ok(Vec::new());
    }

    let mut dependent_files = HashSet::new();

    // 1. Files importing Tier 1 files directly
    for t1_file in tier1_files {
        let pattern_full = format!("%{t1_file}%");
        let stem = Path::new(t1_file)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(t1_file);
        let path_no_ext = t1_file.strip_suffix(&format!(".{stem}")).unwrap_or(t1_file);
        let pattern_stem = format!("%{path_no_ext}%");

        let mut stmt = conn.prepare(
            "SELECT DISTINCT path FROM relationships 
             WHERE (target = ?1 OR target LIKE ?2 OR target LIKE ?3) 
               AND invalidated_at_revision IS NULL",
        )?;
        let rows = stmt.query_map(params![t1_file, pattern_full, pattern_stem], |r| {
            r.get::<_, String>(0)
        })?;
        for r in rows {
            let p = r?;
            if !tier1_files.contains(&p) {
                dependent_files.insert(p);
            }
        }
    }

    // 2. Files calling or implementing symbols defined in Tier 1
    for sym in tier1_symbol_names {
        let pattern_sym_scope = format!("%::{sym}");
        let pattern_sym_any = format!("%{sym}%");
        let mut stmt = conn.prepare(
            "SELECT DISTINCT path FROM relationships 
             WHERE (target = ?1 OR target LIKE ?2 OR target LIKE ?3) 
               AND invalidated_at_revision IS NULL",
        )?;
        let rows = stmt.query_map(params![sym, pattern_sym_scope, pattern_sym_any], |r| {
            r.get::<_, String>(0)
        })?;
        for r in rows {
            let p = r?;
            if !tier1_files.contains(&p) {
                dependent_files.insert(p);
            }
        }
    }

    Ok(dependent_files.into_iter().collect())
}

/// Execute two-tier incremental analysis over a repository using default options.
pub fn incremental_analyze(root: &Path) -> Result<IncrementalAnalysisResult> {
    incremental_analyze_with_options(root, &IncrementalOptions::default())
}

/// Execute two-tier incremental analysis over a repository with explicit options.
pub fn incremental_analyze_with_options(
    root: &Path,
    options: &IncrementalOptions,
) -> Result<IncrementalAnalysisResult> {
    match incremental_analyze_internal(root, options) {
        Ok(res) => Ok(res),
        Err(e) => {
            if options.fallback_on_error {
                let mut diagnostics = vec![format!("FALLBACK_FULL_ANALYSIS: {}", e)];
                // If database itself is corrupted, quarantine it so a fresh clean analysis can rebuild it
                if let Ok(dir) = crate::storage::cache_dir(root) {
                    let db_path = dir.join("derived_symbols.db");
                    if db_path.exists() {
                        let corrupt_path = dir.join(format!(
                            "derived_symbols.db.corrupt.{}",
                            Uuid::new_v4().simple()
                        ));
                        let _ = fs::rename(&db_path, &corrupt_path);
                    }
                }
                let full_summary = crate::analyzer::analyze(root)?;
                diagnostics.push(format!(
                    "Full analysis indexed {} files, {} symbols in revision {}",
                    full_summary.indexed_files, full_summary.symbols, full_summary.revision
                ));
                if let Ok(conn) = db(root) {
                    let _ = init_meta_cache(&conn);
                }
                Ok(IncrementalAnalysisResult {
                    protocol_version: PROTOCOL_VERSION.to_string(),
                    revision: full_summary.revision,
                    total_files: full_summary.indexed_files,
                    tier1_reparsed_files: Vec::new(),
                    tier1_deleted_files: Vec::new(),
                    tier1_renamed_files: Vec::new(),
                    tier2_relinked_files: Vec::new(),
                    tier2_relinked_entities: 0,
                    unchanged_files_count: full_summary.indexed_files,
                    fast_path_hits: 0,
                    fallback_full_analysis: true,
                    diagnostics,
                    duration_ms: 0.0,
                    status: "FALLBACK_FULL_ANALYSIS_COMPLETE".to_string(),
                })
            } else {
                Err(e)
            }
        }
    }
}

fn incremental_analyze_internal(
    root: &Path,
    options: &IncrementalOptions,
) -> Result<IncrementalAnalysisResult> {
    let start_time = Instant::now();
    let root = root_path(root)?;
    let rev = revision(&root);
    let project_config_hash = config_hash(&root);
    let mut conn = db(&root)?;

    init_meta_cache(&conn)?;

    let patterns = ignore_patterns(&root);
    let mut candidate_files = Vec::new();

    for entry in ignore::WalkBuilder::new(&root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = entry.path();
        if p.is_dir() {
            continue;
        }
        if excluded(&root, p, &patterns) {
            continue;
        }
        if language(p).is_none() {
            continue;
        }
        let rel = p.strip_prefix(&root)?.to_string_lossy().replace('\\', "/");
        candidate_files.push((p.to_path_buf(), rel));
    }

    let total_files = candidate_files.len();
    let current_paths: HashSet<String> = candidate_files.iter().map(|(_, p)| p.clone()).collect();

    // Query Git dirty files (files modified in working tree or staged in index)
    let git_dirty: HashSet<String> = crate::storage::git_changed(&root).into_iter().collect();

    // Check which files previously existed in database
    let mut previously_analyzed: HashMap<String, (String, String)> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT path, content_hash, analysis_key FROM files")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?;
        for r in rows {
            let (p, data) = r?;
            previously_analyzed.insert(p, data);
        }
    }

    let mut tier1_reparsed_files = Vec::new();
    let mut tier1_symbol_names = Vec::new();
    let mut tier1_parsed_data = Vec::new();
    let mut computed_hashes: HashMap<String, String> = HashMap::new();
    let mut fast_path_hits = 0;
    let mut unchanged_count = 0;
    let mut diagnostics = Vec::new();

    // TIER 1: Syntactic invalidation
    for (abs, rel) in &candidate_files {
        let file_lang = language(abs).expect("language checked");
        let is_untrusted = options.force_content_hashing || git_dirty.contains(rel);

        let (content_hash, bytes, is_fast_path) =
            get_authoritative_content_hash_internal(abs, rel, &conn, is_untrusted)?;

        computed_hashes.insert(rel.clone(), content_hash.clone());

        if is_fast_path {
            fast_path_hits += 1;
        }

        let file_analysis_key = model::analysis_key(
            &content_hash,
            VERSION,
            "tree-sitter-0.25.10",
            file_lang,
            &project_config_hash,
        );

        // Check if content hash and analysis key match database record
        if let Some((old_hash, old_key)) = previously_analyzed.get(rel) {
            if old_hash == &content_hash && old_key == &file_analysis_key {
                unchanged_count += 1;
                continue;
            }
        }

        // File modified or new: parse in Tier 1
        let bytes = if bytes.is_empty() {
            fs::read(abs).unwrap_or_default()
        } else {
            bytes
        };
        let (rec, symbols, rels) = parse_file(&root, rel, &bytes, &rev)?;
        for s in &symbols {
            tier1_symbol_names.push(s.name.clone());
        }

        tier1_reparsed_files.push(rel.clone());
        tier1_parsed_data.push((rec, symbols, rels, file_analysis_key));
    }

    // Detect deleted and renamed files
    let mut tier1_deleted_files = Vec::new();
    let mut tier1_renamed_files = Vec::new();

    // Candidate additions: newly computed files not in previously_analyzed
    let mut candidate_additions: HashMap<String, String> = HashMap::new();
    for (rel, content_hash) in &computed_hashes {
        if !previously_analyzed.contains_key(rel) {
            candidate_additions.insert(content_hash.clone(), rel.clone());
        }
    }

    for (prev_path, (prev_hash, _)) in &previously_analyzed {
        if !current_paths.contains(prev_path) {
            // Check if this deleted file matches the content hash of a newly added file (rename!)
            if let Some(new_path) = candidate_additions.remove(prev_hash) {
                tier1_renamed_files.push((prev_path.clone(), new_path));
            } else {
                tier1_deleted_files.push(prev_path.clone());
            }
        }
    }

    // TIER 2: Relational graph invalidation and re-linking
    let tier2_relinked_files =
        find_tier2_relational_dependents(&conn, &tier1_reparsed_files, &tier1_symbol_names)?;

    let mut tier2_relinked_entities = 0;

    // Apply updates in SQLite transaction
    let tx = conn.transaction()?;

    // Update revisions table
    let head = crate::storage::git(&root, &["rev-parse", "HEAD"]);
    let commit_hash = if head.is_empty() { "WORKTREE" } else { &head };
    let branch = crate::storage::git(&root, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let parent = crate::storage::git(&root, &["rev-parse", "HEAD^"]);
    let dirty = i64::from(!crate::storage::git(&root, &["status", "--porcelain=v1"]).is_empty());
    let project_analysis_key = model::analysis_key(
        &rev,
        VERSION,
        "tree-sitter-0.25.10",
        "typescript,javascript,python,csharp,rust,go,java",
        &project_config_hash,
    );

    tx.execute(
        "INSERT INTO revisions(revision_id, commit_hash, branch_name, dirty, parent_hash, analysis_key, recorded_at)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(revision_id) DO UPDATE SET analysis_key = excluded.analysis_key",
        params![rev, commit_hash, branch, dirty, if parent.is_empty(){None}else{Some(parent)}, project_analysis_key, now()],
    )?;

    // Handle renames: update entities and relationships to new path
    for (old_path, new_path) in &tier1_renamed_files {
        tx.execute("DELETE FROM files WHERE path = ?1", params![old_path])?;
        tx.execute(
            "DELETE FROM file_meta_cache WHERE path = ?1",
            params![old_path],
        )?;
        tx.execute(
            "UPDATE entities SET path = ?1 WHERE path = ?2 AND invalidated_at_revision IS NULL",
            params![new_path, old_path],
        )?;
        tx.execute(
            "UPDATE entity_versions SET path = ?1 WHERE path = ?2 AND invalidated_at_revision IS NULL",
            params![new_path, old_path],
        )?;
        tx.execute(
            "UPDATE relationships SET path = ?1 WHERE path = ?2 AND invalidated_at_revision IS NULL",
            params![new_path, old_path],
        )?;
        tx.execute(
            "UPDATE relationships SET target = ?1 WHERE target = ?2 AND invalidated_at_revision IS NULL",
            params![new_path, old_path],
        )?;
        diagnostics.push(format!("RENAME_DETECTED: '{}' -> '{}'", old_path, new_path));
    }

    // Invalidate deleted files
    for del in &tier1_deleted_files {
        tx.execute("DELETE FROM files WHERE path = ?1", params![del])?;
        tx.execute("DELETE FROM file_meta_cache WHERE path = ?1", params![del])?;
        tx.execute("UPDATE entities SET invalidated_at_revision = ?1 WHERE path = ?2 AND invalidated_at_revision IS NULL", params![rev, del])?;
        tx.execute("UPDATE entity_versions SET invalidated_at_revision = ?1 WHERE path = ?2 AND invalidated_at_revision IS NULL", params![rev, del])?;
        tx.execute("UPDATE relationships SET invalidated_at_revision = ?1 WHERE path = ?2 AND invalidated_at_revision IS NULL", params![rev, del])?;
        diagnostics.push(format!("DELETED_FILE: '{}'", del));
    }

    let mut assigned_ids = HashSet::new();

    // Persist Tier 1 parsed updates
    for (rec, mut symbols, rels, file_analysis_key) in tier1_parsed_data {
        reconcile_symbol_ids(&tx, &mut symbols, &current_paths, &mut assigned_ids)?;

        tx.execute(
            "INSERT INTO files(path, content_hash, analysis_key, language, excerpt, revision, analyzed_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(path) DO UPDATE SET
                content_hash = excluded.content_hash,
                analysis_key = excluded.analysis_key,
                language = excluded.language,
                excerpt = excluded.excerpt,
                revision = excluded.revision,
                analyzed_at = excluded.analyzed_at",
            params![rec.path, rec.hash, file_analysis_key, rec.language, rec.excerpt, rev, now()],
        )?;

        tx.execute("UPDATE entities SET invalidated_at_revision = ?1 WHERE path = ?2 AND invalidated_at_revision IS NULL", params![rev, rec.path])?;
        tx.execute("UPDATE entity_versions SET invalidated_at_revision = ?1 WHERE path = ?2 AND invalidated_at_revision IS NULL", params![rev, rec.path])?;
        tx.execute("UPDATE relationships SET invalidated_at_revision = ?1 WHERE path = ?2 AND invalidated_at_revision IS NULL", params![rev, rec.path])?;

        for s in symbols {
            tx.execute("INSERT OR IGNORE INTO entity_identity(entity_id, kind, created_at_revision) VALUES(?1, ?2, ?3)", params![s.id, s.kind, rev])?;
            tx.execute(
                "INSERT OR REPLACE INTO entity_versions(version_id, entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, invalidated_at_revision, status)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, 'OBSERVED')",
                params![Uuid::new_v4().to_string(), s.id, s.path, s.name, s.kind, s.line as i64, s.end_line as i64, s.signature, s.fingerprint, rev],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO entities(entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, invalidated_at_revision, status)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, NULL, 'OBSERVED')",
                params![s.id, s.path, s.name, s.kind, s.line as i64, s.end_line as i64, s.signature, s.fingerprint, rev],
            )?;
        }

        for r in rels {
            let rel_id = format!(
                "rel_{}",
                &hash(format!("{}\0{}\0{}\0{}", rec.path, r.kind, r.target, r.line).as_bytes())
                    [..24]
            );
            tx.execute(
                "INSERT INTO relationships(source_id, target, kind, evidence, confidence, path, line, created_at_revision)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![r.source, r.target, r.kind, r.evidence, r.confidence, rec.path, r.line as i64, rev],
            )?;
            tx.execute(
                "INSERT INTO evidence(evidence_id, target_id, target_type, tier, confidence, source_type, source_path, source_line, recorded_at, details_json)
                 VALUES(?1, ?2, 'RELATIONSHIP', 'EXTRACTED', ?3, 'TREE_SITTER', ?4, ?5, ?6, ?7)",
                params![rel_id, r.confidence, rec.path, r.line as i64, now(), json!({"edge_type": &r.kind}).to_string()],
            ).ok();
        }
    }

    // Re-link Tier 2 dependent entities: re-extract relationships for affected dependants
    for t2_file in &tier2_relinked_files {
        let abs_t2 = root.join(t2_file);
        if let Ok(bytes) = fs::read(&abs_t2) {
            if let Ok((rec, _symbols, rels)) = parse_file(&root, t2_file, &bytes, &rev) {
                // Count affected entities in dependent file
                let ent_count: usize = tx.query_row(
                    "SELECT COUNT(*) FROM entities WHERE path = ?1 AND invalidated_at_revision IS NULL",
                    params![t2_file],
                    |r| r.get(0),
                ).unwrap_or(0);
                tier2_relinked_entities += ent_count;

                // Invalidate old relationships for this dependent file
                tx.execute(
                    "UPDATE relationships SET invalidated_at_revision = ?1 WHERE path = ?2 AND invalidated_at_revision IS NULL",
                    params![rev, t2_file],
                )?;

                // Re-insert fresh relationships
                for r in rels {
                    let rel_id = format!(
                        "rel_{}",
                        &hash(
                            format!("{}\0{}\0{}\0{}", rec.path, r.kind, r.target, r.line)
                                .as_bytes()
                        )[..24]
                    );
                    tx.execute(
                        "INSERT INTO relationships(source_id, target, kind, evidence, confidence, path, line, created_at_revision)
                         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![r.source, r.target, r.kind, r.evidence, r.confidence, rec.path, r.line as i64, rev],
                    )?;
                    tx.execute(
                        "INSERT INTO evidence(evidence_id, target_id, target_type, tier, confidence, source_type, source_path, source_line, recorded_at, details_json)
                         VALUES(?1, ?2, 'RELATIONSHIP', 'EXTRACTED', ?3, 'TREE_SITTER', ?4, ?5, ?6, ?7)",
                        params![rel_id, r.confidence, rec.path, r.line as i64, now(), json!({"edge_type": &r.kind}).to_string()],
                    ).ok();
                }
            }
        }
    }

    // Cross-file Go structural interface matching if Go files affected
    let has_go = tier1_reparsed_files
        .iter()
        .chain(tier2_relinked_files.iter())
        .any(|p| p.ends_with(".go"));
    if has_go {
        let mut go_file_asts = Vec::new();
        for (abs, rel) in &candidate_files {
            if rel.ends_with(".go") {
                if let Ok(content) = fs::read_to_string(abs) {
                    if let Ok(ast) = crate::go_parser::extract_go_ast(&content) {
                        go_file_asts.push((rel.clone(), ast));
                    }
                }
            }
        }
        if !go_file_asts.is_empty() {
            let go_refs: Vec<(&str, &crate::go_parser::GoFileAst)> =
                go_file_asts.iter().map(|(p, a)| (p.as_str(), a)).collect();
            let cross_iface_rels = crate::go_parser::infer_cross_file_go_interfaces(&go_refs);
            for r in cross_iface_rels {
                let path = r.source.split("::").next().unwrap_or(&r.source);
                let rel_id = format!(
                    "rel_{}",
                    &hash(format!("{}\0{}\0{}\0{}", path, r.kind, r.target, r.line).as_bytes())
                        [..24]
                );
                tx.execute(
                    "INSERT INTO relationships(source_id, target, kind, evidence, confidence, path, line, created_at_revision) 
                     VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![r.source, r.target, r.kind, r.evidence, r.confidence, path, r.line as i64, rev],
                )?;
                tx.execute(
                    "INSERT INTO evidence(evidence_id, target_id, target_type, tier, confidence, source_type, source_path, source_line, recorded_at, details_json) 
                     VALUES(?1, ?2, 'RELATIONSHIP', 'EXTRACTED', ?3, 'TREE_SITTER', ?4, ?5, ?6, ?7)",
                    params![rel_id, r.confidence, path, r.line as i64, now(), json!({"edge_type": &r.kind}).to_string()],
                ).ok();
            }
        }
    }

    tx.commit()?;

    let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    Ok(IncrementalAnalysisResult {
        protocol_version: PROTOCOL_VERSION.to_string(),
        revision: rev,
        total_files,
        tier1_reparsed_files,
        tier1_deleted_files,
        tier1_renamed_files,
        tier2_relinked_files,
        tier2_relinked_entities,
        unchanged_files_count: unchanged_count,
        fast_path_hits,
        fallback_full_analysis: false,
        diagnostics,
        duration_ms,
        status: "INCREMENTAL_ANALYSIS_COMPLETE".to_string(),
    })
}
