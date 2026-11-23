//! Graphify Macro-Knowledge Graph Ingestion & Impact Subsystem (Phase 9.0).
//!
//! Integrates Graphify's macro-knowledge graph topologies (`graphify-out/graph.json`):
//! 1. High-fidelity deserialization of macro-graph nodes, links, and hyperedges.
//! 2. Git revision staleness detection (`built_at_commit` vs current Git `HEAD`).
//! 3. Strict workspace boundary and path traversal validation.
//! 4. Fast in-memory indexing with community detection and god-node extraction.
//! 5. Macro-graph change impact cone expansion with clear evidence-tier separation.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

// ---------------------------------------------------------------------------
// Graphify Schema Data Models
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphifyNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub _callable: Option<bool>,
    #[serde(default)]
    pub _callable_class: Option<bool>,
    #[serde(default)]
    pub _origin: Option<String>, // "ast", "inferred", etc.
    #[serde(default)]
    pub community: Option<usize>,
    #[serde(default)]
    pub community_name: Option<String>,
    #[serde(default)]
    pub file_type: Option<String>, // "code", "doc"
    #[serde(default)]
    pub source_file: Option<String>,
    #[serde(default)]
    pub source_location: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphifyLink {
    pub source: String,
    pub target: String,
    pub relation: String,
    #[serde(default)]
    pub _origin: Option<String>,
    #[serde(default)]
    pub confidence: Option<String>, // "EXTRACTED", "INFERRED", "AMBIGUOUS"
    #[serde(default)]
    pub confidence_score: Option<f64>,
    #[serde(default)]
    pub context: Option<String>,
    #[serde(default)]
    pub source_file: Option<String>,
    #[serde(default)]
    pub source_location: Option<String>,
    #[serde(default)]
    pub weight: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphifyHyperedge {
    pub id: String,
    pub label: String,
    pub nodes: Vec<String>,
    pub relation: String,
    #[serde(default)]
    pub confidence: Option<String>,
    #[serde(default)]
    pub confidence_score: Option<f64>,
    #[serde(default)]
    pub source_file: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct GraphifyGraphMetadata {
    #[serde(default)]
    pub hyperedges: Option<Vec<GraphifyHyperedge>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphifyMacroGraph {
    #[serde(default)]
    pub directed: bool,
    #[serde(default)]
    pub multigraph: bool,
    #[serde(default)]
    pub graph: Option<GraphifyGraphMetadata>,
    #[serde(default)]
    pub nodes: Vec<GraphifyNode>,
    #[serde(default)]
    pub links: Vec<GraphifyLink>,
    #[serde(default)]
    pub hyperedges: Option<Vec<GraphifyHyperedge>>,
    #[serde(default)]
    pub built_at_commit: Option<String>,
}

// ---------------------------------------------------------------------------
// Staleness & Health Evaluation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GraphStaleness {
    Fresh {
        commit: String,
    },
    Stale {
        built_at: String,
        current_head: String,
    },
    Unknown {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommunitySummary {
    pub id: usize,
    pub name: String,
    pub member_count: usize,
    pub representative_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphifyInspectionReport {
    pub present: bool,
    pub path: Option<String>,
    pub staleness: GraphStaleness,
    pub node_count: usize,
    pub link_count: usize,
    pub community_count: usize,
    pub hyperedge_count: usize,
    pub top_god_nodes: Vec<(String, usize)>,
    pub communities: Vec<CommunitySummary>,
}

// ---------------------------------------------------------------------------
// In-Memory Macro-Graph Index
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GraphifyIndex {
    pub nodes_by_id: HashMap<String, GraphifyNode>,
    pub file_to_node_ids: HashMap<String, Vec<String>>,
    pub outgoing_links: HashMap<String, Vec<GraphifyLink>>,
    pub incoming_links: HashMap<String, Vec<GraphifyLink>>,
    pub communities: HashMap<usize, (String, HashSet<String>)>,
    pub hyperedges: Vec<GraphifyHyperedge>,
    pub built_at_commit: Option<String>,
}

impl GraphifyIndex {
    pub fn build(graph: GraphifyMacroGraph) -> Self {
        let mut nodes_by_id = HashMap::with_capacity(graph.nodes.len());
        let mut file_to_node_ids: HashMap<String, Vec<String>> = HashMap::new();
        let mut communities: HashMap<usize, (String, HashSet<String>)> = HashMap::new();

        for node in graph.nodes {
            if let Some(ref file) = node.source_file {
                let norm = file.replace('\\', "/");
                file_to_node_ids
                    .entry(norm)
                    .or_default()
                    .push(node.id.clone());
            }

            if let Some(cid) = node.community {
                let cname = node
                    .community_name
                    .clone()
                    .unwrap_or_else(|| format!("Community {}", cid));
                let entry = communities
                    .entry(cid)
                    .or_insert_with(|| (cname, HashSet::new()));
                entry.1.insert(node.id.clone());
            }

            nodes_by_id.insert(node.id.clone(), node);
        }

        let mut outgoing_links: HashMap<String, Vec<GraphifyLink>> = HashMap::new();
        let mut incoming_links: HashMap<String, Vec<GraphifyLink>> = HashMap::new();

        for link in graph.links {
            outgoing_links
                .entry(link.source.clone())
                .or_default()
                .push(link.clone());
            incoming_links
                .entry(link.target.clone())
                .or_default()
                .push(link);
        }

        let mut all_hyperedges = Vec::new();
        if let Some(edges) = graph.hyperedges {
            all_hyperedges.extend(edges);
        }
        if let Some(meta) = graph.graph {
            if let Some(edges) = meta.hyperedges {
                for e in edges {
                    if !all_hyperedges.iter().any(|existing| existing.id == e.id) {
                        all_hyperedges.push(e);
                    }
                }
            }
        }

        Self {
            nodes_by_id,
            file_to_node_ids,
            outgoing_links,
            incoming_links,
            communities,
            hyperedges: all_hyperedges,
            built_at_commit: graph.built_at_commit,
        }
    }

    /// Discover macro-graph candidates connected to a source repository file.
    pub fn expand_impact(
        &self,
        relative_file_path: &str,
        max_hop: usize,
    ) -> Vec<MacroImpactCandidate> {
        let norm_path = relative_file_path.replace('\\', "/");
        let start_nodes = match self.file_to_node_ids.get(&norm_path) {
            Some(nodes) => nodes,
            None => return Vec::new(),
        };

        let mut candidates = Vec::new();
        let mut visited_nodes = HashSet::new();

        for start_id in start_nodes {
            visited_nodes.insert(start_id.clone());
            let mut current_level = vec![start_id.clone()];

            for hop in 1..=max_hop {
                let mut next_level = Vec::new();

                for node_id in current_level {
                    // Outgoing edges
                    if let Some(links) = self.outgoing_links.get(&node_id) {
                        for link in links {
                            if visited_nodes.insert(link.target.clone()) {
                                if let Some(target_node) = self.nodes_by_id.get(&link.target) {
                                    if let Some(ref target_file) = target_node.source_file {
                                        let target_norm = target_file.replace('\\', "/");
                                        if target_norm != norm_path {
                                            candidates.push(MacroImpactCandidate {
                                                file_path: target_norm,
                                                target_node_id: target_node.id.clone(),
                                                relation: link.relation.clone(),
                                                confidence: link
                                                    .confidence
                                                    .clone()
                                                    .unwrap_or_else(|| "INFERRED".to_string()),
                                                confidence_score: link
                                                    .confidence_score
                                                    .unwrap_or(0.8),
                                                hop,
                                                community_id: target_node.community,
                                                community_name: target_node.community_name.clone(),
                                                is_graph_derived: true,
                                            });
                                        }
                                    }
                                }
                                next_level.push(link.target.clone());
                            }
                        }
                    }

                    // Incoming edges (reverse callers / dependents)
                    if let Some(links) = self.incoming_links.get(&node_id) {
                        for link in links {
                            if visited_nodes.insert(link.source.clone()) {
                                if let Some(source_node) = self.nodes_by_id.get(&link.source) {
                                    if let Some(ref source_file) = source_node.source_file {
                                        let source_norm = source_file.replace('\\', "/");
                                        if source_norm != norm_path {
                                            candidates.push(MacroImpactCandidate {
                                                file_path: source_norm,
                                                target_node_id: source_node.id.clone(),
                                                relation: format!("called_by_{}", link.relation),
                                                confidence: link
                                                    .confidence
                                                    .clone()
                                                    .unwrap_or_else(|| "INFERRED".to_string()),
                                                confidence_score: link
                                                    .confidence_score
                                                    .unwrap_or(0.8),
                                                hop,
                                                community_id: source_node.community,
                                                community_name: source_node.community_name.clone(),
                                                is_graph_derived: true,
                                            });
                                        }
                                    }
                                }
                                next_level.push(link.source.clone());
                            }
                        }
                    }
                }

                current_level = next_level;
                if current_level.is_empty() {
                    break;
                }
            }
        }

        // Deduplicate candidates by file_path, keeping highest confidence score
        let mut deduped: HashMap<String, MacroImpactCandidate> = HashMap::new();
        for c in candidates {
            let entry = deduped
                .entry(c.file_path.clone())
                .or_insert_with(|| c.clone());
            if c.confidence_score > entry.confidence_score {
                *entry = c;
            }
        }

        let mut result: Vec<MacroImpactCandidate> = deduped.into_values().collect();
        result.sort_by(|a, b| {
            b.confidence_score
                .partial_cmp(&a.confidence_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MacroImpactCandidate {
    pub file_path: String,
    pub target_node_id: String,
    pub relation: String,
    pub confidence: String,
    pub confidence_score: f64,
    pub hop: usize,
    pub community_id: Option<usize>,
    pub community_name: Option<String>,
    pub is_graph_derived: bool,
}

// ---------------------------------------------------------------------------
// File Loading & Path Traversal Guards
// ---------------------------------------------------------------------------

/// Resolves standard Graphify output location for a workspace (`graphify-out/graph.json`).
pub fn graphify_output_path(repo_root: &Path) -> PathBuf {
    repo_root.join("graphify-out").join("graph.json")
}

/// Loads and validates a Graphify macro-graph if present in `graphify-out/graph.json`.
pub fn load_macro_graph(repo_root: &Path) -> Result<Option<GraphifyMacroGraph>> {
    let path = graphify_output_path(repo_root);
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path).with_context(|| {
        format!(
            "Failed to read Graphify macro-graph at '{}'",
            path.display()
        )
    })?;

    let graph: GraphifyMacroGraph = serde_json::from_str(&content).with_context(|| {
        format!(
            "Failed to parse Graphify macro-graph at '{}': malformed JSON or invalid schema",
            path.display()
        )
    })?;

    // Validate that graph paths do not escape workspace root (path traversal check)
    validate_macro_graph_boundaries(&graph, repo_root)?;

    Ok(Some(graph))
}

/// Validates that all node and link file paths stay within the workspace boundary (no path traversal).
pub fn validate_macro_graph_boundaries(
    graph: &GraphifyMacroGraph,
    _repo_root: &Path,
) -> Result<()> {
    for node in &graph.nodes {
        if let Some(ref path) = node.source_file {
            let norm = path.replace('\\', "/");
            if norm.contains("..") || norm.starts_with('/') || norm.contains(':') {
                return Err(anyhow!(
                    "SECURITY BREACH: Graphify node '{}' contains forbidden path traversal '{}'",
                    node.id,
                    path
                ));
            }
        }
    }

    for link in &graph.links {
        if let Some(ref path) = link.source_file {
            let norm = path.replace('\\', "/");
            if norm.contains("..") || norm.starts_with('/') || norm.contains(':') {
                return Err(anyhow!(
                    "SECURITY BREACH: Graphify link '{} -> {}' contains forbidden path traversal '{}'",
                    link.source,
                    link.target,
                    path
                ));
            }
        }
    }

    Ok(())
}

/// Evaluates graph staleness against current Git HEAD.
pub fn check_graph_staleness(
    graph: &GraphifyMacroGraph,
    repo_root: &Path,
) -> Result<GraphStaleness> {
    let built_commit = match graph.built_at_commit {
        Some(ref c) if !c.trim().is_empty() => c.trim().to_string(),
        _ => {
            return Ok(GraphStaleness::Unknown {
                reason: "Graph does not record built_at_commit".to_string(),
            });
        }
    };

    match crate::git::resolve_baseline(repo_root, "HEAD") {
        Ok(Ok(head_sha)) => {
            if head_sha == built_commit {
                Ok(GraphStaleness::Fresh { commit: head_sha })
            } else {
                Ok(GraphStaleness::Stale {
                    built_at: built_commit,
                    current_head: head_sha,
                })
            }
        }
        Ok(Err(msg)) => Ok(GraphStaleness::Unknown {
            reason: format!("Unable to resolve HEAD baseline: {}", msg),
        }),
        Err(e) => Ok(GraphStaleness::Unknown {
            reason: format!("Git rev-parse HEAD error: {}", e),
        }),
    }
}

/// Inspects the workspace for Graphify artifacts, reporting health, staleness,
/// node counts, god-nodes, and community clusters.
pub fn inspect_graphify_workspace(repo_root: &Path) -> Result<GraphifyInspectionReport> {
    let path = graphify_output_path(repo_root);
    if !path.exists() {
        return Ok(GraphifyInspectionReport {
            present: false,
            path: None,
            staleness: GraphStaleness::Unknown {
                reason: "graphify-out/graph.json not found".to_string(),
            },
            node_count: 0,
            link_count: 0,
            community_count: 0,
            hyperedge_count: 0,
            top_god_nodes: Vec::new(),
            communities: Vec::new(),
        });
    }

    let graph = load_macro_graph(repo_root)?.ok_or_else(|| anyhow!("Graph file vanished"))?;
    let staleness = check_graph_staleness(&graph, repo_root).unwrap_or(GraphStaleness::Unknown {
        reason: "Git staleness check failed".to_string(),
    });

    let node_count = graph.nodes.len();
    let link_count = graph.links.len();

    // Degree computation for god-node detection
    let mut degree_map: HashMap<String, usize> = HashMap::new();
    for link in &graph.links {
        *degree_map.entry(link.source.clone()).or_insert(0) += 1;
        *degree_map.entry(link.target.clone()).or_insert(0) += 1;
    }

    let mut god_nodes: Vec<(String, usize)> = degree_map.into_iter().collect();
    god_nodes.sort_by(|a, b| b.1.cmp(&a.1));
    god_nodes.truncate(10);

    // Communities summary
    let mut comm_map: HashMap<usize, (String, HashSet<String>, HashSet<String>)> = HashMap::new();
    for node in &graph.nodes {
        if let Some(cid) = node.community {
            let cname = node
                .community_name
                .clone()
                .unwrap_or_else(|| format!("Community {}", cid));
            let entry = comm_map
                .entry(cid)
                .or_insert_with(|| (cname, HashSet::new(), HashSet::new()));
            entry.1.insert(node.id.clone());
            if let Some(ref file) = node.source_file {
                entry.2.insert(file.clone());
            }
        }
    }

    let mut communities = Vec::new();
    for (cid, (cname, members, files)) in comm_map {
        let mut rep_files: Vec<String> = files.into_iter().collect();
        rep_files.sort();
        rep_files.truncate(5);
        communities.push(CommunitySummary {
            id: cid,
            name: cname,
            member_count: members.len(),
            representative_files: rep_files,
        });
    }
    communities.sort_by_key(|c| c.id);

    let mut hyperedge_count = graph.hyperedges.as_ref().map(|h| h.len()).unwrap_or(0);
    if let Some(ref meta) = graph.graph {
        if let Some(ref h) = meta.hyperedges {
            hyperedge_count += h.len();
        }
    }

    Ok(GraphifyInspectionReport {
        present: true,
        path: Some(path.display().to_string()),
        staleness,
        node_count,
        link_count,
        community_count: communities.len(),
        hyperedge_count,
        top_god_nodes: god_nodes,
        communities,
    })
}
