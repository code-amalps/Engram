//! Change Impact Intelligence Engine: Multi-Hop Impact Cone, Deterministic

//! Relationship Categories, Provenance Explanations, and Authorization Separation.

//!

//! Invariants:

//! 1. Impact predicts relevance; Change Contract authorizes change.

//! 2. Relationship distance does not equal confidence; preserve evidence tiers.

//! 3. IMPORT_DEPENDENT is an import relationship, distinct from guaranteed disruption.

//! 4. Provenance Explanation Accuracy: Produce verifiable, causal reasons for blast radius.

use anyhow::{Result, anyhow};

use rusqlite::{Connection, params};

use serde::{Deserialize, Serialize};

use std::collections::{HashMap, HashSet, VecDeque};

use std::path::Path;

use crate::model::EvidenceTier;

use crate::storage::root_path;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]

pub enum ImpactCategory {
    Direct, // The target symbol or enclosing definition

    Test, // Test suites asserting against the target (TESTS)

    Implementation, // Struct ↔ Interface structural implementation (IMPLEMENTS)

    Caller, // Functions/methods referencing or invoking the target

    ImportDependent, // Workspace packages importing the target package (IMPORTS)

    Configuration, // Manifests, environment, or ecosystem configurations

    Historical, // Statistically mined Git co-change coupling
}

impl ImpactCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Direct => "DIRECT",

            Self::Implementation => "IMPLEMENTATION",

            Self::Caller => "CALLER",

            Self::Test => "TEST",

            Self::ImportDependent => "IMPORT_DEPENDENT",

            Self::Configuration => "CONFIGURATION",

            Self::Historical => "HISTORICAL",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DependencyClassification {
    Workspace,
    External,
    Unresolved,
}

impl Default for DependencyClassification {
    fn default() -> Self {
        Self::Workspace
    }
}

impl DependencyClassification {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Workspace => "WORKSPACE",
            Self::External => "EXTERNAL",
            Self::Unresolved => "UNRESOLVED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactRelationship {
    pub category: ImpactCategory,
    pub source: String,
    pub target: String,
    pub hop_distance: usize,
    pub evidence_tier: EvidenceTier,
    pub provenance: String,
    pub confidence: f64,
    #[serde(default)]
    pub is_external: bool,
    #[serde(skip)]
    pub classification: DependencyClassification,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactEntity {
    pub entity_id: String,
    pub path: String,
    pub symbol_name: Option<String>,
    pub category: ImpactCategory,
    pub hop_distance: usize,
    pub relationships: Vec<ImpactRelationship>,
    pub reason: String,
    #[serde(default)]
    pub is_external: bool,
    #[serde(skip)]
    pub classification: DependencyClassification,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ImpactMetrics {
    pub total_nodes: usize,
    pub first_party_nodes: usize,
    pub external_terminal_nodes: usize,
    #[serde(default)]
    pub unresolved_nodes: usize,
    pub traversed_edges: usize,
    pub pruned_edges: usize,
    pub max_traversal_depth: usize,
    pub payload_bytes: usize,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct ImpactCone {
    pub target_symbol: String,

    pub target_file: Option<String>,

    pub total_hops: usize,

    pub direct_entities: Vec<ImpactEntity>,

    pub implementation_entities: Vec<ImpactEntity>,

    pub caller_entities: Vec<ImpactEntity>,

    pub test_entities: Vec<ImpactEntity>,

    pub import_dependent_entities: Vec<ImpactEntity>,

    pub historical_entities: Vec<ImpactEntity>,

    pub configuration_entities: Vec<ImpactEntity>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metrics: Option<ImpactMetrics>,
}

impl ImpactCone {
    /// Total count of all entities across categories.

    pub fn total_entities(&self) -> usize {
        self.direct_entities.len()
            + self.implementation_entities.len()
            + self.caller_entities.len()
            + self.test_entities.len()
            + self.import_dependent_entities.len()
            + self.historical_entities.len()
            + self.configuration_entities.len()
    }

    /// Retrieve all unique file paths in the impact cone.

    pub fn all_affected_paths(&self) -> Vec<String> {
        let mut paths = HashSet::new();

        for e in self.all_entities() {
            paths.insert(e.path.clone());
        }

        let mut sorted: Vec<String> = paths.into_iter().collect();

        sorted.sort();

        sorted
    }

    /// Retrieve all entities in deterministic priority order.

    pub fn all_entities(&self) -> Vec<&ImpactEntity> {
        let mut out = Vec::new();

        out.extend(&self.direct_entities);

        out.extend(&self.test_entities);

        out.extend(&self.implementation_entities);

        out.extend(&self.caller_entities);

        out.extend(&self.import_dependent_entities);

        out.extend(&self.historical_entities);

        out.extend(&self.configuration_entities);

        out
    }

    /// Retrieve all first-party entities in the impact cone.

    pub fn first_party_entities(&self) -> Vec<&ImpactEntity> {
        self.all_entities()
            .into_iter()
            .filter(|e| e.classification == DependencyClassification::Workspace)
            .collect()
    }

    /// Retrieve all external terminal entities in the impact cone.
    pub fn external_entities(&self) -> Vec<&ImpactEntity> {
        self.all_entities()
            .into_iter()
            .filter(|e| e.classification == DependencyClassification::External)
            .collect()
    }

    /// Retrieve all unresolved entities in the impact cone.
    pub fn unresolved_entities(&self) -> Vec<&ImpactEntity> {
        self.all_entities()
            .into_iter()
            .filter(|e| e.classification == DependencyClassification::Unresolved)
            .collect()
    }

    /// Retrieve all unique first-party file paths in the impact cone.
    pub fn first_party_paths(&self) -> Vec<String> {
        let mut paths = HashSet::new();
        for e in self.first_party_entities() {
            paths.insert(e.path.clone());
        }
        let mut sorted: Vec<String> = paths.into_iter().collect();
        sorted.sort();
        sorted
    }

    /// Retrieve all unique external dependency paths in the impact cone.
    pub fn external_paths(&self) -> Vec<String> {
        let mut paths = HashSet::new();
        for e in self.external_entities() {
            paths.insert(e.path.clone());
        }
        let mut sorted: Vec<String> = paths.into_iter().collect();
        sorted.sort();
        sorted
    }

    /// Retrieve all unique unresolved dependency paths in the impact cone.
    pub fn unresolved_paths(&self) -> Vec<String> {
        let mut paths = HashSet::new();
        for e in self.unresolved_entities() {
            paths.insert(e.path.clone());
        }
        let mut sorted: Vec<String> = paths.into_iter().collect();
        sorted.sort();
        sorted
    }

    /// Access traversal metrics if computed.

    pub fn metrics(&self) -> Option<&ImpactMetrics> {
        self.metrics.as_ref()
    }
}

/// Compute the multi-hop Impact Cone for a given target symbol or file.

/// Classify a dependency target into Workspace, External, or Unresolved.
pub fn classify_dependency(
    node: &str,
    rel_is_ext: bool,
    kind: &str,
    workspace_files: &HashSet<String>,
) -> DependencyClassification {
    let clean = node.trim().replace('\\', "/");
    if clean.is_empty() {
        return DependencyClassification::Unresolved;
    }

    let file_part = clean.split("::").next().unwrap_or(&clean);

    // 1. Direct third-party or standard library specifiers on node or file_part
    if clean.starts_with('@')
        || clean.starts_with("std::")
        || clean.starts_with("core::")
        || clean.starts_with("alloc::")
        || clean.starts_with("java.")
        || clean.starts_with("javax.")
        || clean.contains("node_modules")
        || clean.contains("vendor/")
        || clean.contains("vendor\\")
        || clean.starts_with("rxjs")
        || clean.starts_with("lodash")
        || clean.starts_with("express")
        || file_part.starts_with('@')
        || file_part.starts_with("rxjs")
        || file_part.starts_with("lodash")
    {
        return DependencyClassification::External;
    }

    // 2. Exact workspace file match
    if workspace_files.contains(&clean) || workspace_files.contains(file_part) {
        return DependencyClassification::Workspace;
    }

    // Basename or relative suffix match in workspace
    if file_part.contains('.')
        && workspace_files
            .iter()
            .any(|f| f.ends_with(&format!("/{}", file_part)) || f == file_part)
    {
        return DependencyClassification::Workspace;
    }

    // Non-import relationships (IMPLEMENTS, TESTS, CALLS) within workspace default to first-party
    if !kind.starts_with("IMPORT") {
        if workspace_files.contains(&clean) || workspace_files.contains(file_part) {
            return DependencyClassification::Workspace;
        }
        if rel_is_ext
            || clean.starts_with('@')
            || clean.starts_with("std::")
            || clean.starts_with("core::")
            || clean.starts_with("java.")
            || file_part.starts_with('@')
            || crate::analyzer::is_external_target(file_part)
        {
            return DependencyClassification::External;
        }
        return DependencyClassification::Workspace;
    }

    // 3. Unresolved workspace paths: specifies a workspace relative or file path, but absent from workspace_files
    if file_part.starts_with('.')
        || file_part.starts_with('/')
        || file_part.starts_with("src/")
        || file_part.starts_with("projects/")
        || file_part.starts_with("app/")
        || file_part.starts_with("tests/")
        || file_part.ends_with(".ts")
        || file_part.ends_with(".js")
        || file_part.ends_with(".tsx")
        || file_part.ends_with(".jsx")
        || file_part.ends_with(".rs")
        || file_part.ends_with(".go")
        || file_part.ends_with(".java")
    {
        return DependencyClassification::Unresolved;
    }

    // 4. Confirmed external packages (bare package specifiers e.g. "chalk", "uuid")
    if rel_is_ext || crate::analyzer::is_external_target(file_part) {
        if !file_part.contains('/') && !file_part.contains('\\') && !file_part.contains('.') {
            return DependencyClassification::External;
        }
        if file_part.starts_with('@')
            || file_part.starts_with("std::")
            || file_part.starts_with("rxjs")
        {
            return DependencyClassification::External;
        }
    }

    DependencyClassification::Unresolved
}

pub fn is_external_node(
    node: &str,
    rel_is_ext: bool,
    kind: &str,
    workspace_files: &HashSet<String>,
) -> bool {
    classify_dependency(node, rel_is_ext, kind, workspace_files)
        == DependencyClassification::External
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ImpactBudgetOptions {
    pub max_depth: usize,
    pub max_entities_per_category: usize,
    pub max_relationships_per_entity: usize,
    pub prioritize_workspace: bool,
}

impl Default for ImpactBudgetOptions {
    fn default() -> Self {
        Self {
            max_depth: 2,
            max_entities_per_category: 10,
            max_relationships_per_entity: 5,
            prioritize_workspace: true,
        }
    }
}

/// Compute the multi-hop Impact Cone for a given target symbol or file.
pub fn compute_impact_cone(
    conn: &Connection,
    root: &Path,
    target: &str,
    max_depth: usize,
) -> Result<ImpactCone> {
    compute_impact_cone_with_budget(
        conn,
        root,
        target,
        ImpactBudgetOptions {
            max_depth,
            max_entities_per_category: usize::MAX,
            max_relationships_per_entity: usize::MAX,
            prioritize_workspace: false,
        },
    )
    .map(|(cone, _)| cone)
}

/// Compute the multi-hop Impact Cone with deterministic budgeting and omission tracking.
pub fn compute_impact_cone_with_budget(
    conn: &Connection,
    root: &Path,
    target: &str,
    opts: ImpactBudgetOptions,
) -> Result<(ImpactCone, Vec<crate::budget::OmissionEntry>)> {
    let _root = root_path(root)?;
    let max_depth = if opts.max_depth == 0 {
        0
    } else {
        opts.max_depth.min(3)
    };
    let normalized_path_target = target.trim().replace('\\', "/");
    let trimmed_target = normalized_path_target.as_str();
    let norm_target = crate::go_parser::normalize_go_symbol_name(trimmed_target);
    let norm_java = crate::java_parser::normalize_java_symbol_name(trimmed_target);

    // Build set of all indexed first-party workspace files and symbols
    let mut workspace_files: HashSet<String> = HashSet::new();
    {
        let mut stmt = conn.prepare("SELECT path FROM files")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for r in rows.flatten() {
            workspace_files.insert(r.replace('\\', "/"));
        }
    }
    let mut workspace_symbol_paths: HashMap<String, Vec<String>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT name, path FROM entity_versions WHERE invalidated_at_revision IS NULL ORDER BY path ASC",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for r in rows.flatten() {
            workspace_symbol_paths
                .entry(r.0)
                .or_default()
                .push(r.1.replace('\\', "/"));
        }
    }

    // 1. Locate Direct Target Entities in SQLite
    let mut direct_entities = Vec::new();
    let mut target_file: Option<String> = None;

    // Check if target refers to a file path or qualified symbol
    let file_candidate = if let Some((f, _)) = trimmed_target.split_once("::") {
        Some(f.trim_start_matches('/'))
    } else if trimmed_target.contains('/') || trimmed_target.contains('.') {
        Some(trimmed_target.trim_start_matches('/'))
    } else {
        None
    };

    let resolved_canonical_file = if let Some(cand) = file_candidate {
        let mut exact_stmt = conn.prepare("SELECT path FROM files WHERE path = ?1")?;
        let exact_opt: Option<String> = exact_stmt.query_row([cand], |r| r.get(0)).ok();
        if let Some(exact) = exact_opt {
            Some(exact)
        } else {
            let mut suffix_stmt = conn
                .prepare("SELECT path FROM files WHERE path LIKE '%/' || ?1 ORDER BY path ASC")?;
            let matches: Vec<String> = suffix_stmt
                .query_map([cand], |r| r.get(0))?
                .filter_map(|r| r.ok())
                .collect();
            if matches.len() > 1 {
                return Err(anyhow!(
                    "Target file '{}' is ambiguous: matches multiple files: {:?}. Specify the exact canonical path.",
                    trimmed_target,
                    matches
                ));
            }
            matches.into_iter().next()
        }
    } else {
        None
    };

    {
        if let Some(ref can_file) = resolved_canonical_file {
            if let Some((_, sym_part)) = trimmed_target.split_once("::") {
                let mut stmt = conn.prepare(
                    "SELECT entity_id, path, name, kind, start_line, signature FROM entity_versions WHERE path = ?1 AND name = ?2 AND invalidated_at_revision IS NULL",
                )?;
                let rows = stmt.query_map(params![can_file, sym_part], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, usize>(4)?,
                        r.get::<_, String>(5)?,
                    ))
                })?;
                for row in rows.flatten() {
                    if target_file.is_none() {
                        target_file = Some(row.1.clone());
                    }
                    direct_entities.push(ImpactEntity {
                        entity_id: row.0,
                        path: row.1.clone(),
                        symbol_name: Some(row.2.clone()),
                        category: ImpactCategory::Direct,
                        hop_distance: 0,
                        relationships: Vec::new(),
                        reason: format!("Target entity {} ({}) at line {}", row.2, row.3, row.4),
                        is_external: false,
                        classification: DependencyClassification::Workspace,
                    });
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT entity_id, path, name, kind, start_line, signature FROM entity_versions WHERE path = ?1 AND invalidated_at_revision IS NULL",
                )?;
                let rows = stmt.query_map(params![can_file], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, usize>(4)?,
                        r.get::<_, String>(5)?,
                    ))
                })?;
                for row in rows.flatten() {
                    if target_file.is_none() {
                        target_file = Some(row.1.clone());
                    }
                    direct_entities.push(ImpactEntity {
                        entity_id: row.0,
                        path: row.1.clone(),
                        symbol_name: Some(row.2.clone()),
                        category: ImpactCategory::Direct,
                        hop_distance: 0,
                        relationships: Vec::new(),
                        reason: format!("Target entity {} ({}) at line {}", row.2, row.3, row.4),
                        is_external: false,
                        classification: DependencyClassification::Workspace,
                    });
                }
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT entity_id, path, name, kind, start_line, signature FROM entity_versions WHERE (name = ?1 OR name = ?2 OR name = ?3 OR path = ?1 OR path || '::' || name = ?1) AND invalidated_at_revision IS NULL",
            )?;

            let rows = stmt.query_map(params![trimmed_target, norm_target, norm_java], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, usize>(4)?,
                    r.get::<_, String>(5)?,
                ))
            })?;

            for row in rows.flatten() {
                if target_file.is_none() {
                    target_file = Some(row.1.clone());
                }
                direct_entities.push(ImpactEntity {
                    entity_id: row.0,
                    path: row.1.clone(),
                    symbol_name: Some(row.2.clone()),
                    category: ImpactCategory::Direct,
                    hop_distance: 0,
                    relationships: Vec::new(),
                    reason: format!("Target entity {} ({}) at line {}", row.2, row.3, row.4),
                    is_external: false,
                    classification: DependencyClassification::Workspace,
                });
            }
        }
    }

    // If no direct symbol was matched, check if target is an indexed file path
    if direct_entities.is_empty() {
        if let Some(matched) = resolved_canonical_file {
            target_file = Some(matched.clone());
            direct_entities.push(ImpactEntity {
                entity_id: format!("file_{}", matched),
                path: matched.clone(),
                symbol_name: None,
                category: ImpactCategory::Direct,
                hop_distance: 0,
                relationships: Vec::new(),
                reason: format!("Direct target file {}", matched),
                is_external: false,
                classification: DependencyClassification::Workspace,
            });
        }
    }

    if direct_entities.is_empty() {
        return Err(anyhow!(
            "Target entity or file '{}' not found in repository index",
            target
        ));
    }

    let resolved_target_file = target_file
        .clone()
        .unwrap_or_else(|| trimmed_target.to_string());

    // 2. Multi-Hop Breadth-First Expansion
    let mut visited_nodes: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize, String)> = VecDeque::new(); // (node_id_or_path, depth, current_file)
    let mut enqueued: HashSet<String> = HashSet::new();

    for d in &direct_entities {
        visited_nodes.insert(d.path.clone());
        if let Some(ref sym) = d.symbol_name {
            let sym_key = format!("{}::{}", d.path, sym);
            visited_nodes.insert(sym_key.clone());
            if enqueued.insert(sym_key.clone()) {
                queue.push_back((sym_key, 0, d.path.clone()));
            }
        }
        if enqueued.insert(d.path.clone()) {
            queue.push_back((d.path.clone(), 0, d.path.clone()));
        }
    }

    let mut collected_entities: HashMap<String, ImpactEntity> = HashMap::new();
    let mut traversed_edges = 0usize;
    let mut pruned_edges = 0usize;
    let mut max_observed_depth = 0usize;

    while let Some((curr_node, curr_depth, curr_file)) = queue.pop_front() {
        if curr_depth >= max_depth {
            continue;
        }

        let next_depth = curr_depth + 1;
        if next_depth > max_observed_depth {
            max_observed_depth = next_depth;
        }

        // A. Outgoing relationships: curr_node -> target
        {
            let prefix_start = format!("{}::", curr_node);
            let prefix_end = format!("{}::\u{ffff}", curr_node);
            let mut stmt = conn.prepare(
                "SELECT source_id, target, kind, evidence, confidence, path, line, is_external
                 FROM relationships
                 WHERE (source_id = ?1 OR (source_id >= ?2 AND source_id < ?3) OR path = ?1)
                   AND invalidated_at_revision IS NULL",
            )?;

            let rows = stmt.query_map(params![&curr_node, &prefix_start, &prefix_end], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, f64>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, usize>(6)?,
                    r.get::<_, i64>(7).unwrap_or(0),
                ))
            })?;

            for row in rows.flatten() {
                let (
                    source_id,
                    target,
                    kind,
                    evidence,
                    confidence,
                    _rel_path,
                    line,
                    rel_is_ext_int,
                ) = row;
                traversed_edges += 1;
                let target_clean = target.trim();
                let resolved_sym_file = if kind != "CALLS"
                    && !target_clean.contains('/')
                    && !target_clean.contains('\\')
                    && rel_is_ext_int == 0
                {
                    workspace_symbol_paths.get(target_clean).and_then(|paths| {
                        if paths.len() == 1 {
                            Some(paths[0].clone())
                        } else {
                            let curr_dir = Path::new(&curr_file)
                                .parent()
                                .and_then(|p| p.to_str())
                                .unwrap_or("");
                            paths.iter().find(|p| p.starts_with(curr_dir)).cloned()
                        }
                    })
                } else {
                    None
                };

                // Skip type/symbol references back to self or the seed target file to avoid self-loop inflation
                if kind == "REFERENCES" {
                    if let Some(ref sym_f) = resolved_sym_file {
                        if sym_f == &curr_file || sym_f == &resolved_target_file {
                            continue;
                        }
                    } else if target_clean == resolved_target_file {
                        continue;
                    }
                }

                let target_file_part = resolved_sym_file
                    .as_deref()
                    .unwrap_or_else(|| target_clean.split("::").next().unwrap_or(target_clean));

                let classification = if resolved_sym_file.is_some() {
                    DependencyClassification::Workspace
                } else {
                    classify_dependency(target_clean, rel_is_ext_int != 0, &kind, &workspace_files)
                };
                let is_ext = classification == DependencyClassification::External;
                let is_unresolved = classification == DependencyClassification::Unresolved;

                let (cat, prov) = match kind.as_str() {
                    "IMPLEMENTS" => (
                        ImpactCategory::Implementation,
                        match classification {
                            DependencyClassification::External => {
                                format!(
                                    "IMPLEMENTS external interface {} (line {})",
                                    target_clean, line
                                )
                            }
                            DependencyClassification::Unresolved => {
                                format!(
                                    "IMPLEMENTS unresolved interface {} (line {})",
                                    target_clean, line
                                )
                            }
                            DependencyClassification::Workspace => {
                                format!("IMPLEMENTS interface {} (line {})", target_clean, line)
                            }
                        },
                    ),
                    "TESTS" => (
                        ImpactCategory::Test,
                        match classification {
                            DependencyClassification::External => {
                                format!("TESTS external target {} (line {})", target_clean, line)
                            }
                            DependencyClassification::Unresolved => {
                                format!("TESTS unresolved target {} (line {})", target_clean, line)
                            }
                            DependencyClassification::Workspace => {
                                format!("TESTS assertion target {} (line {})", target_clean, line)
                            }
                        },
                    ),
                    "CALLS" => (
                        ImpactCategory::Caller,
                        match classification {
                            DependencyClassification::External => {
                                format!(
                                    "CALLS external invocation target {} (line {})",
                                    target_clean, line
                                )
                            }
                            DependencyClassification::Unresolved => {
                                format!(
                                    "CALLS unresolved invocation target {} (line {})",
                                    target_clean, line
                                )
                            }
                            DependencyClassification::Workspace => {
                                format!("CALLS invocation target {} (line {})", target_clean, line)
                            }
                        },
                    ),
                    "IMPORTS" => (
                        ImpactCategory::ImportDependent,
                        match classification {
                            DependencyClassification::External => {
                                format!(
                                    "IMPORTS external dependency {} (line {})",
                                    target_clean, line
                                )
                            }
                            DependencyClassification::Unresolved => {
                                format!(
                                    "IMPORTS unresolved dependency target {} (line {})",
                                    target_clean, line
                                )
                            }
                            DependencyClassification::Workspace => {
                                format!(
                                    "IMPORTS dependency target {} (line {})",
                                    target_clean, line
                                )
                            }
                        },
                    ),
                    _ => (
                        ImpactCategory::ImportDependent,
                        match classification {
                            DependencyClassification::External => {
                                format!(
                                    "{} external relationship to {} (line {})",
                                    kind, target_clean, line
                                )
                            }
                            DependencyClassification::Unresolved => {
                                format!(
                                    "{} unresolved relationship to {} (line {})",
                                    kind, target_clean, line
                                )
                            }
                            DependencyClassification::Workspace => {
                                format!("{} relationship to {} (line {})", kind, target_clean, line)
                            }
                        },
                    ),
                };

                let rel = ImpactRelationship {
                    category: cat,
                    source: source_id,
                    target: target_clean.to_string(),
                    hop_distance: next_depth,
                    evidence_tier: parse_evidence_tier(&evidence),
                    provenance: prov.clone(),
                    confidence,
                    is_external: is_ext,
                    classification,
                };

                record_or_merge_entity(
                    &mut collected_entities,
                    target_file_part,
                    target_clean,
                    cat,
                    next_depth,
                    rel,
                    prov,
                    is_ext,
                    classification,
                    opts.max_relationships_per_entity,
                );

                if is_ext {
                    pruned_edges += 1;
                    visited_nodes.insert(target_clean.to_string());
                    visited_nodes.insert(target_file_part.to_string());
                    // Terminal leaf: NEVER push external nodes to the queue!
                    continue;
                }

                if is_unresolved {
                    visited_nodes.insert(target_clean.to_string());
                    visited_nodes.insert(target_file_part.to_string());
                    // Terminal leaf: NEVER push unresolved nodes to the queue!
                    continue;
                }

                if !visited_nodes.contains(target_clean) {
                    visited_nodes.insert(target_clean.to_string());
                    let is_new_file = visited_nodes.insert(target_file_part.to_string());
                    if next_depth < max_depth && (resolved_sym_file.is_none() || is_new_file) {
                        let q_node = resolved_sym_file
                            .clone()
                            .unwrap_or_else(|| target_clean.to_string());
                        queue.push_back((q_node, next_depth, target_file_part.to_string()));
                    }
                }
            }
        }

        // B. Incoming relationships: source -> curr_node (reverse dependency query)
        {
            let curr_trimmed = curr_node.trim();
            let curr_without_ext = curr_trimmed
                .strip_suffix(".ts")
                .or_else(|| curr_trimmed.strip_suffix(".js"))
                .or_else(|| curr_trimmed.strip_suffix(".tsx"))
                .or_else(|| curr_trimmed.strip_suffix(".jsx"))
                .unwrap_or(curr_trimmed);

            let file_trimmed = curr_file.trim();
            let file_without_ext = file_trimmed
                .strip_suffix(".ts")
                .or_else(|| file_trimmed.strip_suffix(".js"))
                .or_else(|| file_trimmed.strip_suffix(".tsx"))
                .or_else(|| file_trimmed.strip_suffix(".jsx"))
                .unwrap_or(file_trimmed);

            let file_name = Path::new(file_trimmed)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(file_trimmed);
            let file_name_without_ext = Path::new(file_without_ext)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(file_without_ext);

            let prefix_start = format!("{}::", curr_trimmed);
            let prefix_end = format!("{}::\u{ffff}", curr_trimmed);
            let wo_prefix_start = format!("{}::", curr_without_ext);
            let wo_prefix_end = format!("{}::\u{ffff}", curr_without_ext);

            let mut stmt = conn.prepare(
                "SELECT source_id, target, kind, evidence, confidence, path, line, is_external
                 FROM relationships
                 WHERE (target = ?1
                        OR (target >= ?2 AND target < ?3)
                        OR target = ?4
                        OR target = ?5
                        OR (target >= ?6 AND target < ?7)
                        OR target = ?8
                        OR target = ?9
                        OR target = ?10)
                   AND invalidated_at_revision IS NULL",
            )?;

            let rows = stmt.query_map(
                params![
                    curr_trimmed,
                    &prefix_start,
                    &prefix_end,
                    file_trimmed,
                    curr_without_ext,
                    &wo_prefix_start,
                    &wo_prefix_end,
                    file_without_ext,
                    file_name,
                    file_name_without_ext,
                ],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, f64>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, usize>(6)?,
                        r.get::<_, i64>(7).unwrap_or(0),
                    ))
                },
            )?;

            for row in rows.flatten() {
                let (
                    source_id,
                    target,
                    kind,
                    evidence,
                    confidence,
                    _rel_path,
                    line,
                    rel_is_ext_int,
                ) = row;
                traversed_edges += 1;
                let source_clean = source_id.trim();
                let source_file_part = source_clean.split("::").next().unwrap_or(source_clean);

                let classification =
                    classify_dependency(source_clean, rel_is_ext_int != 0, &kind, &workspace_files);
                let is_ext = classification == DependencyClassification::External;
                let is_unresolved = classification == DependencyClassification::Unresolved;

                let (cat, prov) = match kind.as_str() {
                    "IMPLEMENTS" => (
                        ImpactCategory::Implementation,
                        match classification {
                            DependencyClassification::External => {
                                format!(
                                    "Implemented by external struct {} (line {})",
                                    source_clean, line
                                )
                            }
                            DependencyClassification::Unresolved => {
                                format!(
                                    "Implemented by unresolved struct {} (line {})",
                                    source_clean, line
                                )
                            }
                            DependencyClassification::Workspace => {
                                format!("Implemented by struct {} (line {})", source_clean, line)
                            }
                        },
                    ),
                    "TESTS" => (
                        ImpactCategory::Test,
                        match classification {
                            DependencyClassification::External => {
                                format!(
                                    "Asserted by external test file {} (line {})",
                                    source_clean, line
                                )
                            }
                            DependencyClassification::Unresolved => {
                                format!(
                                    "Asserted by unresolved test file {} (line {})",
                                    source_clean, line
                                )
                            }
                            DependencyClassification::Workspace => {
                                format!("Asserted by test file {} (line {})", source_clean, line)
                            }
                        },
                    ),
                    "CALLS" => (
                        ImpactCategory::Caller,
                        match classification {
                            DependencyClassification::External => {
                                format!(
                                    "Invoked by external caller {} (line {})",
                                    source_clean, line
                                )
                            }
                            DependencyClassification::Unresolved => {
                                format!(
                                    "Invoked by unresolved caller {} (line {})",
                                    source_clean, line
                                )
                            }
                            DependencyClassification::Workspace => {
                                format!("Invoked by caller {} (line {})", source_clean, line)
                            }
                        },
                    ),
                    "IMPORTS" => (
                        ImpactCategory::ImportDependent,
                        match classification {
                            DependencyClassification::External => {
                                format!(
                                    "Imported by external dependent package {} (line {})",
                                    source_clean, line
                                )
                            }
                            DependencyClassification::Unresolved => {
                                format!(
                                    "Imported by unresolved dependent package {} (line {})",
                                    source_clean, line
                                )
                            }
                            DependencyClassification::Workspace => {
                                format!(
                                    "Imported by dependent package {} (line {})",
                                    source_clean, line
                                )
                            }
                        },
                    ),
                    _ => (
                        ImpactCategory::ImportDependent,
                        match classification {
                            DependencyClassification::External => {
                                format!("Referenced by external {} (line {})", source_clean, line)
                            }
                            DependencyClassification::Unresolved => {
                                format!("Referenced by unresolved {} (line {})", source_clean, line)
                            }
                            DependencyClassification::Workspace => {
                                format!("Referenced by {} (line {})", source_clean, line)
                            }
                        },
                    ),
                };

                let rel = ImpactRelationship {
                    category: cat,
                    source: source_clean.to_string(),
                    target: target.clone(),
                    hop_distance: next_depth,
                    evidence_tier: parse_evidence_tier(&evidence),
                    provenance: prov.clone(),
                    confidence,
                    is_external: is_ext,
                    classification,
                };

                record_or_merge_entity(
                    &mut collected_entities,
                    source_file_part,
                    source_clean,
                    cat,
                    next_depth,
                    rel,
                    prov,
                    is_ext,
                    classification,
                    opts.max_relationships_per_entity,
                );

                if is_ext {
                    pruned_edges += 1;
                    visited_nodes.insert(source_clean.to_string());
                    visited_nodes.insert(source_file_part.to_string());
                    // Terminal leaf: NEVER push external nodes to the queue!
                    continue;
                }

                if is_unresolved {
                    visited_nodes.insert(source_clean.to_string());
                    visited_nodes.insert(source_file_part.to_string());
                    // Terminal leaf: NEVER push unresolved nodes to the queue!
                    continue;
                }

                if !visited_nodes.contains(source_clean) {
                    visited_nodes.insert(source_clean.to_string());
                    visited_nodes.insert(source_file_part.to_string());
                    if next_depth < max_depth {
                        queue.push_back((
                            source_clean.to_string(),
                            next_depth,
                            source_file_part.to_string(),
                        ));
                    }
                }
            }
        }

        // C. Statistical Git Co-Change Signals (Historical Evidence)
        if let Ok(cochanges) =
            crate::git_history::get_cochanges_for_file_conn(conn, &curr_file, 0.25)
        {
            for cochange in cochanges {
                let partner = cochange.target_file.replace('\\', "/");
                if visited_nodes.contains(&partner) {
                    continue;
                }
                // Verify partner is in workspace_files
                if !workspace_files.is_empty() && !workspace_files.contains(&partner) {
                    continue;
                }

                traversed_edges += 1;

                let prov = format!(
                    "Historically co-changes in {} commits (P={:.2}, Lift={:.2})",
                    cochange.support, cochange.conditional_probability, cochange.lift
                );

                let rel = ImpactRelationship {
                    category: ImpactCategory::Historical,
                    source: curr_file.clone(),
                    target: partner.clone(),
                    hop_distance: next_depth,
                    evidence_tier: EvidenceTier::Historical,
                    provenance: prov.clone(),
                    confidence: cochange.conditional_probability,
                    is_external: false,
                    classification: DependencyClassification::Workspace,
                };

                record_or_merge_entity(
                    &mut collected_entities,
                    &partner,
                    &partner,
                    ImpactCategory::Historical,
                    next_depth,
                    rel,
                    prov,
                    false,
                    DependencyClassification::Workspace,
                    opts.max_relationships_per_entity,
                );

                visited_nodes.insert(partner.clone());
            }
        }
    }

    // 3. Bucket and group into ImpactCone
    let mut implementation_entities = Vec::new();
    let mut caller_entities = Vec::new();
    let mut test_entities = Vec::new();
    let mut import_dependent_entities = Vec::new();
    let mut historical_entities = Vec::new();
    let mut configuration_entities = Vec::new();

    for (_, entity) in collected_entities {
        // Exclude direct target entities from secondary buckets
        if direct_entities.iter().any(|d| d.path == entity.path) {
            continue;
        }

        match entity.category {
            ImpactCategory::Implementation => implementation_entities.push(entity),
            ImpactCategory::Caller => caller_entities.push(entity),
            ImpactCategory::Test => test_entities.push(entity),
            ImpactCategory::ImportDependent => import_dependent_entities.push(entity),
            ImpactCategory::Historical => historical_entities.push(entity),
            ImpactCategory::Configuration => configuration_entities.push(entity),
            ImpactCategory::Direct => {}
        }
    }

    // Sort entities within each category by hop distance, then path, then symbol name for deterministic outputs
    let sort_fn = |a: &ImpactEntity, b: &ImpactEntity| {
        let ext_cmp = if opts.prioritize_workspace {
            a.is_external.cmp(&b.is_external)
        } else {
            std::cmp::Ordering::Equal
        };
        ext_cmp
            .then_with(|| a.hop_distance.cmp(&b.hop_distance))
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.symbol_name.cmp(&b.symbol_name))
    };

    direct_entities.sort_by(sort_fn);
    implementation_entities.sort_by(sort_fn);
    caller_entities.sort_by(sort_fn);
    test_entities.sort_by(sort_fn);
    import_dependent_entities.sort_by(sort_fn);
    historical_entities.sort_by(sort_fn);
    configuration_entities.sort_by(sort_fn);

    // Prune categories to budget and record omissions
    let mut omissions = Vec::new();
    if opts.max_entities_per_category < usize::MAX {
        let mut prune_category =
            |entities: &mut Vec<ImpactEntity>, cat_name: &str, priority: usize| {
                if entities.len() > opts.max_entities_per_category {
                    let dropped = entities.split_off(opts.max_entities_per_category);
                    for item in dropped {
                        omissions.push(crate::budget::OmissionEntry {
                            context_item_id: item.entity_id.clone(),
                            symbol: item.symbol_name.clone(),
                            file: item.path.clone(),
                            context_type: cat_name.to_string(),
                            priority_tier: priority,
                            original_token_count: 50,
                            included_token_count: 0,
                            omitted_token_count: 50,
                            omission_reason: format!(
                                "Exceeded max_entities_per_category limit ({})",
                                opts.max_entities_per_category
                            ),
                            omission_status: "CAPACITY_PRUNED".to_string(),
                            retrievable_via_targeted_request: true,
                        });
                    }
                }
            };

        prune_category(&mut import_dependent_entities, "IMPORT_DEPENDENT", 3);
        prune_category(&mut caller_entities, "CALLER", 3);
        prune_category(&mut implementation_entities, "IMPLEMENTATION", 2);
        prune_category(&mut test_entities, "TEST", 5);
        prune_category(&mut historical_entities, "HISTORICAL", 5);
        prune_category(&mut configuration_entities, "CONFIGURATION", 3);
    }

    // Also sort relationships within each entity deterministically
    let sort_rel = |a: &ImpactRelationship, b: &ImpactRelationship| {
        a.hop_distance
            .cmp(&b.hop_distance)
            .then_with(|| a.category.cmp(&b.category))
            .then_with(|| a.source.cmp(&b.source))
            .then_with(|| a.target.cmp(&b.target))
    };
    for e in &mut direct_entities {
        e.relationships.sort_by(sort_rel);
    }
    for e in &mut implementation_entities {
        e.relationships.sort_by(sort_rel);
    }
    for e in &mut caller_entities {
        e.relationships.sort_by(sort_rel);
    }
    for e in &mut test_entities {
        e.relationships.sort_by(sort_rel);
    }
    for e in &mut import_dependent_entities {
        e.relationships.sort_by(sort_rel);
    }
    for e in &mut historical_entities {
        e.relationships.sort_by(sort_rel);
    }
    for e in &mut configuration_entities {
        e.relationships.sort_by(sort_rel);
    }

    // 4. Compute traversal metrics
    let mut total_nodes = 0usize;
    let mut first_party_nodes = 0usize;
    let mut external_terminal_nodes = 0usize;
    let mut unresolved_nodes = 0usize;

    for e in direct_entities
        .iter()
        .chain(implementation_entities.iter())
        .chain(caller_entities.iter())
        .chain(test_entities.iter())
        .chain(import_dependent_entities.iter())
        .chain(historical_entities.iter())
        .chain(configuration_entities.iter())
    {
        total_nodes += 1;
        match e.classification {
            DependencyClassification::External => external_terminal_nodes += 1,
            DependencyClassification::Workspace => first_party_nodes += 1,
            DependencyClassification::Unresolved => unresolved_nodes += 1,
        }
    }

    let mut cone = ImpactCone {
        target_symbol: trimmed_target.to_string(),
        target_file: Some(resolved_target_file),
        total_hops: max_depth,
        direct_entities,
        implementation_entities,
        caller_entities,
        test_entities,
        import_dependent_entities,
        historical_entities,
        configuration_entities,
        metrics: None,
    };

    let payload_bytes = serde_json::to_string(&cone).map(|s| s.len()).unwrap_or(0);
    let estimated_tokens = (payload_bytes + 3) / 4;

    let metrics = ImpactMetrics {
        total_nodes,
        first_party_nodes,
        external_terminal_nodes,
        unresolved_nodes,
        traversed_edges,
        pruned_edges,
        max_traversal_depth: max_observed_depth,
        payload_bytes,
        estimated_tokens,
    };

    cone.metrics = Some(metrics);

    Ok((cone, omissions))
}

fn record_or_merge_entity(
    entities: &mut HashMap<String, ImpactEntity>,
    file_path: &str,
    symbol_or_path: &str,
    category: ImpactCategory,
    hop_distance: usize,
    rel: ImpactRelationship,
    reason: String,
    is_external: bool,
    classification: DependencyClassification,
    max_relationships_per_entity: usize,
) {
    let key = file_path.to_string();
    let entry = entities.entry(key).or_insert_with(|| ImpactEntity {
        entity_id: format!("imp_{}", file_path.replace(['/', '.', '\\', ':'], "_")),
        path: file_path.to_string(),
        symbol_name: if symbol_or_path.contains("::") {
            symbol_or_path.split("::").nth(1).map(String::from)
        } else {
            None
        },
        category,
        hop_distance,
        relationships: Vec::new(),
        reason,
        is_external,
        classification,
    });

    if category < entry.category {
        entry.category = category;
        entry.reason = rel.provenance.clone();
    }
    entry.hop_distance = entry.hop_distance.min(hop_distance);
    // Classification precedence: Workspace > External > Unresolved
    if entry.classification == DependencyClassification::Unresolved
        && classification != DependencyClassification::Unresolved
    {
        entry.classification = classification;
    } else if entry.classification == DependencyClassification::External
        && classification == DependencyClassification::Workspace
    {
        entry.classification = classification;
    }
    entry.is_external = entry.classification == DependencyClassification::External;

    // Deduplicate and cap relationships
    if !entry.relationships.iter().any(|r| {
        r.source == rel.source
            && r.target == rel.target
            && r.category == rel.category
            && r.provenance == rel.provenance
    }) {
        if entry.relationships.len() < max_relationships_per_entity {
            entry.relationships.push(rel);
        }
    }
}

fn parse_evidence_tier(tier: &str) -> EvidenceTier {
    match tier {
        "EXPLICIT" => EvidenceTier::Explicit,
        "VERIFIED" => EvidenceTier::Verified,
        "HISTORICAL" => EvidenceTier::Historical,
        "INFERRED" => EvidenceTier::Inferred,
        "PROVISIONAL" => EvidenceTier::Provisional,
        _ => EvidenceTier::Extracted,
    }
}

/// Format the impact cone into human-readable inspect output.
pub fn format_impact_explanation(cone: &ImpactCone) -> String {
    let mut out = String::new();

    out.push_str("🎯 TARGET\n");
    out.push_str(&format!("  {}\n", cone.target_symbol));
    if let Some(ref f) = cone.target_file {
        out.push_str(&format!("  Path: {}\n", f));
    }
    out.push('\n');

    out.push_str("💥 IMPACT BLAST RADIUS\n");

    if !cone.direct_entities.is_empty() {
        out.push_str("  DIRECT:\n");
        for e in &cone.direct_entities {
            out.push_str(&format!("    • {} (hop {})\n", e.path, e.hop_distance));
        }
    }

    if !cone.implementation_entities.is_empty() {
        out.push_str("  IMPLEMENTATION:\n");
        for e in &cone.implementation_entities {
            out.push_str(&format!("    • {} (hop {})\n", e.path, e.hop_distance));
        }
    }

    if !cone.caller_entities.is_empty() {
        out.push_str("  CALLER:\n");
        for e in &cone.caller_entities {
            out.push_str(&format!("    • {} (hop {})\n", e.path, e.hop_distance));
        }
    }

    if !cone.test_entities.is_empty() {
        out.push_str("  TEST:\n");
        for e in &cone.test_entities {
            out.push_str(&format!("    • {} (hop {})\n", e.path, e.hop_distance));
        }
    }

    if !cone.import_dependent_entities.is_empty() {
        out.push_str("  IMPORT_DEPENDENT:\n");
        for e in &cone.import_dependent_entities {
            let ext_tag = match e.classification {
                DependencyClassification::External => " [EXTERNAL TERMINAL]",
                DependencyClassification::Unresolved => " [UNRESOLVED]",
                DependencyClassification::Workspace => "",
            };
            out.push_str(&format!(
                "    • {}{} (hop {})\n",
                e.path, ext_tag, e.hop_distance
            ));
        }
    }

    if !cone.historical_entities.is_empty() {
        out.push_str("  HISTORICAL:\n");
        for e in &cone.historical_entities {
            out.push_str(&format!("    • {} (hop {})\n", e.path, e.hop_distance));
        }
    }

    if !cone.configuration_entities.is_empty() {
        out.push_str("  CONFIGURATION:\n");
        for e in &cone.configuration_entities {
            out.push_str(&format!("    • {} (hop {})\n", e.path, e.hop_distance));
        }
    }

    let has_dependents = !cone.implementation_entities.is_empty()
        || !cone.caller_entities.is_empty()
        || !cone.test_entities.is_empty()
        || !cone.import_dependent_entities.is_empty()
        || !cone.historical_entities.is_empty()
        || !cone.configuration_entities.is_empty();

    if !has_dependents {
        out.push_str("  (No downstream dependents, tests, or callers detected in index)\n");
    }

    if let Some(ref m) = cone.metrics {
        out.push_str("\n📊 TRAVERSAL METRICS\n");
        out.push_str(&format!("  Total Nodes: {}\n", m.total_nodes));
        out.push_str(&format!("  First-Party Nodes: {}\n", m.first_party_nodes));
        out.push_str(&format!(
            "  External Terminal Nodes: {}\n",
            m.external_terminal_nodes
        ));
        out.push_str(&format!("  Unresolved Nodes: {}\n", m.unresolved_nodes));
        out.push_str(&format!("  Traversed Edges: {}\n", m.traversed_edges));
        out.push_str(&format!("  Pruned Edges: {}\n", m.pruned_edges));
        out.push_str(&format!(
            "  Max Traversal Depth: {}\n",
            m.max_traversal_depth
        ));
        out.push_str(&format!("  Payload Size: {} bytes\n", m.payload_bytes));
        out.push_str(&format!("  Estimated Tokens: ~{}\n", m.estimated_tokens));
    }

    out.push_str("\n🔍 PROVENANCE (WHY)\n");
    for e in cone.all_entities() {
        out.push_str(&format!("  {}\n", e.path));
        out.push_str(&format!("    └─ Category: {}\n", e.category.as_str()));
        for r in &e.relationships {
            out.push_str(&format!(
                "       └─ {} ({:?}, confidence: {:.2})\n",
                r.provenance, r.evidence_tier, r.confidence
            ));
        }
    }

    out
}

#[cfg(test)]

mod tests {

    use super::*;

    #[test]

    fn test_impact_category_ordering_precedence() {
        assert!(ImpactCategory::Direct < ImpactCategory::Test);

        assert!(ImpactCategory::Test < ImpactCategory::Implementation);

        assert!(ImpactCategory::Implementation < ImpactCategory::Caller);

        assert!(ImpactCategory::Caller < ImpactCategory::ImportDependent);

        assert!(ImpactCategory::ImportDependent < ImpactCategory::Historical);
    }
}
