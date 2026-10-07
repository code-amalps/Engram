//! Change Impact Intelligence Engine: Multi-Hop Impact Cone, Deterministic
//! Relationship Categories, Provenance Explanations, and Authorization Separation.
//!
//! Invariants:
//! 1. Impact predicts relevance; Change Contract authorizes change.
//! 2. Relationship distance does not equal confidence; preserve evidence tiers.
//! 3. IMPORT_DEPENDENT is an import relationship, distinct from guaranteed disruption.
//! 4. Provenance Explanation Accuracy: Produce verifiable, causal reasons for blast radius.

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;

use crate::model::EvidenceTier;
use crate::storage::root_path;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ImpactCategory {
    Direct,           // The target symbol or enclosing definition
    Test,             // Test suites asserting against the target (TESTS)
    Implementation,   // Struct ↔ Interface structural implementation (IMPLEMENTS)
    Caller,           // Functions/methods referencing or invoking the target
    ImportDependent,  // Workspace packages importing the target package (IMPORTS)
    Configuration,    // Manifests, environment, or ecosystem configurations
    Historical,       // Statistically mined Git co-change coupling
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactRelationship {
    pub category: ImpactCategory,
    pub source: String,
    pub target: String,
    pub hop_distance: usize,
    pub evidence_tier: EvidenceTier,
    pub provenance: String,
    pub confidence: f64,
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
}

/// Compute the multi-hop Impact Cone for a given target symbol or file.
pub fn compute_impact_cone(
    conn: &Connection,
    root: &Path,
    target: &str,
    max_depth: usize,
) -> Result<ImpactCone> {
    let root = root_path(root)?;
    let trimmed_target = target.trim();
    let norm_target = crate::go_parser::normalize_go_symbol_name(trimmed_target);

    // 1. Locate Direct Target Entities in SQLite
    let mut direct_entities = Vec::new();
    let mut target_file: Option<String> = None;

    {
        let mut stmt = conn.prepare(
            "SELECT entity_id, path, name, kind, start_line, signature 
             FROM entity_versions 
             WHERE (name = ?1 OR name = ?2 OR path = ?1 OR path || '::' || name = ?1)
               AND invalidated_at_revision IS NULL"
        )?;

        let rows = stmt.query_map(params![trimmed_target, norm_target], |r| {
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
            });
        }
    }

    // If no direct symbol was matched, check if target is an indexed file path
    if direct_entities.is_empty() {
        let mut stmt = conn.prepare("SELECT path FROM files WHERE path = ?1")?;
        let exists: bool = stmt.query_row([trimmed_target], |_| Ok(true)).unwrap_or(false);
        if exists {
            target_file = Some(trimmed_target.to_string());
            direct_entities.push(ImpactEntity {
                entity_id: format!("file_{}", trimmed_target),
                path: trimmed_target.to_string(),
                symbol_name: None,
                category: ImpactCategory::Direct,
                hop_distance: 0,
                relationships: Vec::new(),
                reason: format!("Direct target file {}", trimmed_target),
            });
        }
    }

    if direct_entities.is_empty() {
        return Err(anyhow!("Target entity or file '{}' not found in repository index", target));
    }

    let resolved_target_file = target_file.clone().unwrap_or_else(|| trimmed_target.to_string());

    // 2. Multi-Hop Breadth-First Expansion
    let mut visited_nodes: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize, String)> = VecDeque::new(); // (node_id_or_path, depth, current_file)

    for d in &direct_entities {
        visited_nodes.insert(d.path.clone());
        if let Some(ref sym) = d.symbol_name {
            visited_nodes.insert(format!("{}::{}", d.path, sym));
            queue.push_back((format!("{}::{}", d.path, sym), 0, d.path.clone()));
        }
        queue.push_back((d.path.clone(), 0, d.path.clone()));
    }

    let mut collected_entities: HashMap<String, ImpactEntity> = HashMap::new();

    while let Some((curr_node, curr_depth, curr_file)) = queue.pop_front() {
        if curr_depth >= max_depth {
            continue;
        }

        let prefix = format!("{}::%", curr_node);
        let next_depth = curr_depth + 1;

        // A. Outgoing relationships: curr_node -> target
        {
            let mut stmt = conn.prepare(
                "SELECT source_id, target, kind, evidence, confidence, path, line
                 FROM relationships
                 WHERE (source_id = ?1 OR source_id LIKE ?2 OR path = ?1)
                   AND invalidated_at_revision IS NULL"
            )?;

            let rows = stmt.query_map(params![&curr_node, &prefix], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, f64>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, usize>(6)?,
                ))
            })?;

            for row in rows.flatten() {
                let (source_id, target, kind, evidence, confidence, _rel_path, line) = row;
                let target_clean = target.trim();
                let target_file_part = target_clean.split("::").next().unwrap_or(target_clean);

                let (cat, prov) = match kind.as_str() {
                    "IMPLEMENTS" => (
                        ImpactCategory::Implementation,
                        format!("IMPLEMENTS interface {} (line {})", target_clean, line),
                    ),
                    "TESTS" => (
                        ImpactCategory::Test,
                        format!("TESTS assertion target {} (line {})", target_clean, line),
                    ),
                    "CALLS" => (
                        ImpactCategory::Caller,
                        format!("CALLS invocation target {} (line {})", target_clean, line),
                    ),
                    "IMPORTS" => (
                        ImpactCategory::ImportDependent,
                        format!("IMPORTS dependency target {} (line {})", target_clean, line),
                    ),
                    _ => (
                        ImpactCategory::ImportDependent,
                        format!("{} relationship to {} (line {})", kind, target_clean, line),
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
                };

                record_or_merge_entity(
                    &mut collected_entities,
                    target_file_part,
                    target_clean,
                    cat,
                    next_depth,
                    rel,
                    prov,
                );

                if !visited_nodes.contains(target_clean) {
                    visited_nodes.insert(target_clean.to_string());
                    visited_nodes.insert(target_file_part.to_string());
                    queue.push_back((target_clean.to_string(), next_depth, target_file_part.to_string()));
                }
            }
        }

        // B. Incoming relationships: source -> curr_node (reverse dependency query)
        {
            let mut stmt = conn.prepare(
                "SELECT source_id, target, kind, evidence, confidence, path, line
                 FROM relationships
                 WHERE (target = ?1 OR target LIKE ?2 OR target = ?3)
                   AND invalidated_at_revision IS NULL"
            )?;

            let rows = stmt.query_map(params![&curr_node, &prefix, &curr_file], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, f64>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, usize>(6)?,
                ))
            })?;

            for row in rows.flatten() {
                let (source_id, target, kind, evidence, confidence, _rel_path, line) = row;
                let source_clean = source_id.trim();
                let source_file_part = source_clean.split("::").next().unwrap_or(source_clean);

                let (cat, prov) = match kind.as_str() {
                    "IMPLEMENTS" => (
                        ImpactCategory::Implementation,
                        format!("Implemented by struct {} (line {})", source_clean, line),
                    ),
                    "TESTS" => (
                        ImpactCategory::Test,
                        format!("Asserted by test file {} (line {})", source_clean, line),
                    ),
                    "CALLS" => (
                        ImpactCategory::Caller,
                        format!("Invoked by caller {} (line {})", source_clean, line),
                    ),
                    "IMPORTS" => (
                        ImpactCategory::ImportDependent,
                        format!("Imported by dependent package {} (line {})", source_clean, line),
                    ),
                    _ => (
                        ImpactCategory::ImportDependent,
                        format!("Referenced by {} (line {})", source_clean, line),
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
                };

                record_or_merge_entity(
                    &mut collected_entities,
                    source_file_part,
                    source_clean,
                    cat,
                    next_depth,
                    rel,
                    prov,
                );

                if !visited_nodes.contains(source_clean) {
                    visited_nodes.insert(source_clean.to_string());
                    visited_nodes.insert(source_file_part.to_string());
                    queue.push_back((source_clean.to_string(), next_depth, source_file_part.to_string()));
                }
            }
        }

        // C. Statistical Git Co-Change Signals (Historical Evidence)
        if let Ok(cochanges) = crate::git_history::get_cochanges_for_file(&root, &curr_file, 0.25) {
            for cochange in cochanges {
                let partner = cochange.target_file.replace('\\', "/");
                if visited_nodes.contains(&partner) {
                    continue;
                }

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
                };

                record_or_merge_entity(
                    &mut collected_entities,
                    &partner,
                    &partner,
                    ImpactCategory::Historical,
                    next_depth,
                    rel,
                    prov,
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

    // Sort entities within each category by hop distance then path for deterministic outputs
    let sort_fn = |a: &ImpactEntity, b: &ImpactEntity| {
        a.hop_distance.cmp(&b.hop_distance).then_with(|| a.path.cmp(&b.path))
    };

    direct_entities.sort_by(sort_fn);
    implementation_entities.sort_by(sort_fn);
    caller_entities.sort_by(sort_fn);
    test_entities.sort_by(sort_fn);
    import_dependent_entities.sort_by(sort_fn);
    historical_entities.sort_by(sort_fn);
    configuration_entities.sort_by(sort_fn);

    Ok(ImpactCone {
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
    })
}

fn record_or_merge_entity(
    entities: &mut HashMap<String, ImpactEntity>,
    file_path: &str,
    symbol_or_path: &str,
    category: ImpactCategory,
    hop_distance: usize,
    rel: ImpactRelationship,
    reason: String,
) {
    let key = file_path.to_string();
    let entry = entities.entry(key).or_insert_with(|| ImpactEntity {
        entity_id: format!("imp_{}", file_path.replace(['/', '.', '\\', ':'], "_")),
        path: file_path.to_string(),
        symbol_name: if symbol_or_path.contains("::") {
            symbol_or_path.split("::").nth(1).map(ToString::to_string)
        } else {
            None
        },
        category,
        hop_distance,
        relationships: Vec::new(),
        reason,
    });

    // Category precedence: Direct > Test > Implementation > Caller > Configuration > Historical > ImportDependent
    if category < entry.category {
        entry.category = category;
        entry.reason = rel.provenance.clone();
    }
    entry.hop_distance = entry.hop_distance.min(hop_distance);
    entry.relationships.push(rel);
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
            out.push_str(&format!("    • {} (hop {})\n", e.path, e.hop_distance));
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
