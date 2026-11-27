//! Cross-Layer Feature Tracing Engine (`engram_trace_feature`).
//!
//! Traces architectural vertical slices through a repository:
//! `Entry point -> Route or dispatch -> Handler -> Business/service logic -> Repository or data access`
//!
//! Provides deterministic, evidence-based traversal without requiring an LLM call.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
use std::path::Path;

use crate::frameworks::{AdapterConfidence, DiscoveredEntryPoint, detect_all_framework_adapters};
use crate::knowledge_graph::{KnowledgeGraphIndex, NodeCategory, RelationCategory, open_graph_db};

/// Evidence strength class for a feature trace hop or relationship.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TraceConfidence {
    Candidate,
    Pattern,
    Exact,
}

impl TraceConfidence {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Candidate => "CANDIDATE",
            Self::Pattern => "PATTERN",
            Self::Exact => "EXACT",
        }
    }

    pub fn from_adapter(ac: AdapterConfidence) -> Self {
        match ac {
            AdapterConfidence::Candidate => Self::Candidate,
            AdapterConfidence::Pattern => Self::Pattern,
            AdapterConfidence::Exact => Self::Exact,
        }
    }
}

/// Determine if an external or unresolved symbol name is a trivial language built-in or container method.
fn is_trivial_builtin_or_method(name: &str) -> bool {
    matches!(
        name,
        // Standard builtins
        "print"
            | "len"
            | "str"
            | "int"
            | "float"
            | "bool"
            | "list"
            | "dict"
            | "set"
            | "tuple"
            | "sum"
            | "min"
            | "max"
            | "any"
            | "all"
            | "range"
            | "enumerate"
            | "zip"
            | "isinstance"
            | "issubclass"
            | "getattr"
            | "hasattr"
            | "setattr"
            | "type"
            | "super"
            | "open"
            | "id"
            | "repr"
            | "iter"
            | "next"
            | "hash"
            | "filter"
            | "map"
            | "sorted"
            | "reversed"
            // Standard methods
            | "append"
            | "extend"
            | "pop"
            | "clear"
            | "insert"
            | "remove"
            | "count"
            | "index"
            | "sort"
            | "reverse"
            | "copy"
            | "update"
            | "keys"
            | "values"
            | "items"
            | "get"
            | "setdefault"
            | "strip"
            | "lstrip"
            | "rstrip"
            | "split"
            | "rsplit"
            | "join"
            | "replace"
            | "lower"
            | "upper"
            | "startswith"
            | "endswith"
            | "find"
            | "rfind"
            | "encode"
            | "decode"
            | "format"
            | "isoformat"
            | "today"
            | "now"
            | "utcnow"
            | "model_dump"
            | "model_dump_json"
            | "model_copy"
            // Standard exceptions
            | "Exception"
            | "ValueError"
            | "TypeError"
            | "KeyError"
            | "IndexError"
            | "RuntimeError"
            | "AttributeError"
            | "IOError"
            | "OSError"
            | "NotImplementedError"
            // Standard path / io / collections
            | "Path"
            | "exists"
            | "is_file"
            | "is_dir"
            | "read_text"
            | "write_text"
            | "iterdir"
            | "defaultdict"
            // CLI / UI presentation helpers
            | "Table"
            | "Panel"
            | "Option"
            | "Exit"
            | "add_column"
            | "add_row"
            | "fit"
            | "echo"
    )
}

/// A single architectural hop within a feature trace path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceHop {
    pub node_id: String,
    pub node_kind: String,
    pub display_name: String,
    pub source_path: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_from_previous: Option<String>,
    pub confidence: TraceConfidence,
    pub evidence: String,
}

/// An explicitly identified unresolved relationship or boundary transition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnresolvedLink {
    pub source_node_id: String,
    pub source_path: String,
    pub source_line: u32,
    pub target_hint: String,
    pub link_type: String, // "ExternalBoundary", "DynamicDispatch", "UnresolvedInterface", "DependencyInjection"
    pub reason: String,
}

/// An entry point discovered through framework conventions or public declarations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEntryPoint {
    pub node_id: String,
    pub kind: String, // "HTTP_ROUTE", "CLI_COMMAND", "PUBLIC_API", "EVENT_HANDLER"
    pub display_name: String,
    pub source_path: String,
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route_or_command: Option<String>,
    pub confidence: TraceConfidence,
}

/// An ordered sequence of hops representing an architectural path from an entry point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracePath {
    pub path_id: String,
    pub entry_point_id: String,
    pub hops: Vec<TraceHop>,
    pub is_complete: bool,
}

/// Summary metrics for a feature trace query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceSummary {
    pub total_paths: usize,
    pub total_hops: usize,
    pub exact_hops_count: usize,
    pub pattern_hops_count: usize,
    pub candidate_hops_count: usize,
    pub unresolved_links_count: usize,
    pub distinct_files_count: usize,
}

/// The complete structured response from a cross-layer feature trace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureTrace {
    pub trace_id: String,
    pub repository_id: String,
    pub snapshot_id: String,
    pub query: String,
    pub graph_lifecycle_state: String,
    pub entry_points: Vec<TraceEntryPoint>,
    pub paths: Vec<TracePath>,
    pub unresolved_links: Vec<UnresolvedLink>,
    pub summary: TraceSummary,
    pub diagnostics: Vec<String>,
}

/// Request parameters for feature tracing.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TraceRequest {
    pub query: String,
    pub entry_point_kind: Option<String>,
    pub scope: Option<String>,
    pub max_depth: Option<usize>,
    pub max_paths: Option<usize>,
    pub confidence_threshold: Option<String>,
    pub include_unresolved: Option<bool>,
    pub max_bytes: Option<usize>,
}

/// Execute cross-layer feature tracing over workspace knowledge graph.
pub fn trace_feature(root: &Path, req: &TraceRequest) -> Result<FeatureTrace> {
    let conn = open_graph_db(root)?;
    let index = KnowledgeGraphIndex::load_from_db(&conn)?;

    let head_commit = conn
        .query_row(
            "SELECT value FROM kg_metadata WHERE key = 'last_indexed_commit'",
            [],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "HEAD".to_string());

    let state = conn
        .query_row(
            "SELECT value FROM kg_metadata WHERE key = 'state'",
            [],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "READY".to_string());

    let repo_id = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .to_string();

    let trace_id = format!(
        "trace_{}",
        uuid::Uuid::new_v4().to_string().replace('-', "")[..12].to_string()
    );
    let mut diagnostics = Vec::new();

    if state != "READY" {
        diagnostics.push(format!(
            "Knowledge graph lifecycle state is '{}'; results may be partial.",
            state
        ));
    }

    // 1. Gather all workspace relative files
    let mut files = Vec::new();
    {
        let mut stmt = conn.prepare("SELECT relative_path FROM kg_file_fingerprints")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for row in rows {
            if let Ok(f) = row {
                files.push(f);
            }
        }
    }

    // 2. Discover Entry Points via Framework Adapters & Static Declarations
    let mut discovered_eps: Vec<DiscoveredEntryPoint> = Vec::new();
    let adapters = detect_all_framework_adapters(root, &files);
    for (adapter, _conf) in adapters {
        let eps = adapter.discover_entry_points(root, &conn, &files);
        discovered_eps.extend(eps);
    }

    // Also scan index for CLI main or public exports if no framework entry points found
    if discovered_eps.is_empty() {
        for (id, node) in &index.nodes_by_id {
            if node.category == NodeCategory::Function
                && (node.label == "main" || node.label.ends_with("_cmd"))
            {
                discovered_eps.push(DiscoveredEntryPoint {
                    node_id: id.clone(),
                    kind: "CLI_COMMAND".to_string(),
                    display_name: format!("CLI: {}", node.label),
                    source_path: node.relative_path.clone(),
                    line: node.start_line.unwrap_or(1) as u32,
                    route_or_command: Some(node.label.clone()),
                    confidence: AdapterConfidence::Pattern,
                });
            }
        }
    }

    // Filter by entry_point_kind if requested
    if let Some(ref epk) = req.entry_point_kind {
        let epk_upper = epk.to_ascii_uppercase();
        discovered_eps.retain(|ep| ep.kind.to_ascii_uppercase() == epk_upper);
    }

    // Filter by scope if requested
    if let Some(ref sc) = req.scope {
        let sc_norm = sc.replace('\\', "/");
        discovered_eps.retain(|ep| ep.source_path.replace('\\', "/").starts_with(&sc_norm));
    }

    // 3. Match & Rank Entry Points against Query
    let query_lower = req.query.trim().to_ascii_lowercase();
    let query_words: Vec<&str> = query_lower
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '/' && c != '-')
        .filter(|w| !w.is_empty())
        .collect();

    let mut scored_eps: Vec<(f64, DiscoveredEntryPoint)> = Vec::new();
    for ep in discovered_eps {
        let mut score = 0.0;
        let route_cmd = ep
            .route_or_command
            .clone()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let disp_lower = ep.display_name.to_ascii_lowercase();
        let path_lower = ep.source_path.to_ascii_lowercase();

        // Exact match against route or command
        if !route_cmd.is_empty()
            && (route_cmd == query_lower
                || query_lower.contains(&route_cmd)
                || route_cmd.contains(&query_lower))
        {
            score += 15.0;
        }

        // Exact match in display name
        if disp_lower.contains(&query_lower) {
            score += 10.0;
        }

        // Token matches
        for word in &query_words {
            if route_cmd.contains(word) {
                score += 4.0;
            }
            if disp_lower.contains(word) {
                score += 3.0;
            }
            if path_lower.contains(word) {
                score += 2.0;
            }
        }

        if score > 0.0 {
            scored_eps.push((score, ep));
        }
    }

    // Stable sort: highest score first, then deterministic node_id tie-breaker
    scored_eps.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.1.node_id.cmp(&b.1.node_id))
    });

    let max_paths_limit = req.max_paths.unwrap_or(3).clamp(1, 10);
    let max_depth_limit = req.max_depth.unwrap_or(5).clamp(1, 10);
    let min_confidence = req
        .confidence_threshold
        .as_deref()
        .map(|s| match s.to_ascii_uppercase().as_str() {
            "EXACT" => TraceConfidence::Exact,
            "PATTERN" => TraceConfidence::Pattern,
            _ => TraceConfidence::Candidate,
        })
        .unwrap_or(TraceConfidence::Candidate);

    let selected_eps: Vec<DiscoveredEntryPoint> = scored_eps
        .into_iter()
        .take(max_paths_limit)
        .map(|(_, ep)| ep)
        .collect();

    if selected_eps.is_empty() {
        diagnostics.push(format!("No entry points matched query '{}'.", req.query));
        return Ok(FeatureTrace {
            trace_id,
            repository_id: repo_id,
            snapshot_id: head_commit,
            query: req.query.clone(),
            graph_lifecycle_state: state,
            entry_points: Vec::new(),
            paths: Vec::new(),
            unresolved_links: Vec::new(),
            summary: TraceSummary {
                total_paths: 0,
                total_hops: 0,
                exact_hops_count: 0,
                pattern_hops_count: 0,
                candidate_hops_count: 0,
                unresolved_links_count: 0,
                distinct_files_count: 0,
            },
            diagnostics,
        });
    }

    let mut trace_entry_points = Vec::new();
    let mut all_candidate_paths: Vec<TracePath> = Vec::new();
    let mut all_unresolved = Vec::new();
    let mut seen_unresolved = HashSet::new();

    for ep in selected_eps {
        let ep_confidence = TraceConfidence::from_adapter(ep.confidence);
        trace_entry_points.push(TraceEntryPoint {
            node_id: ep.node_id.clone(),
            kind: ep.kind.clone(),
            display_name: ep.display_name.clone(),
            source_path: ep.source_path.clone(),
            line: ep.line,
            route_or_command: ep.route_or_command.clone(),
            confidence: ep_confidence,
        });

        let mut initial_hops = Vec::new();

        // Hop 0: Entry Point declaration
        initial_hops.push(TraceHop {
            node_id: format!("ep:{}", ep.node_id),
            node_kind: ep.kind.clone(),
            display_name: ep.display_name.clone(),
            source_path: ep.source_path.clone(),
            start_line: ep.line,
            end_line: ep.line,
            relationship_from_previous: None,
            confidence: ep_confidence,
            evidence: format!("Entry point declaration: {}", ep.display_name),
        });

        // Hop 1: The handler function node
        let handler_node = index.nodes_by_id.get(&ep.node_id);
        let start_traversal_node_id = if let Some(hn) = handler_node {
            initial_hops.push(TraceHop {
                node_id: hn.id.clone(),
                node_kind: "HANDLER".to_string(),
                display_name: format!("{}()", hn.label),
                source_path: hn.relative_path.clone(),
                start_line: hn.start_line.unwrap_or(1) as u32,
                end_line: hn.end_line.unwrap_or(1) as u32,
                relationship_from_previous: Some("ROUTES_TO".to_string()),
                confidence: TraceConfidence::Exact,
                evidence: format!("Route mapped to handler function {}", hn.label),
            });
            hn.id.clone()
        } else {
            ep.node_id.clone()
        };

        // Linear Path Exploration with Queue of Paths
        struct PathState {
            curr_node_id: String,
            hops: Vec<TraceHop>,
            visited_in_path: Vec<String>,
        }

        let mut queue: VecDeque<PathState> = VecDeque::new();
        queue.push_back(PathState {
            curr_node_id: start_traversal_node_id.clone(),
            hops: initial_hops.clone(),
            visited_in_path: vec![ep.node_id.clone(), start_traversal_node_id.clone()],
        });

        let mut ep_paths: Vec<TracePath> = Vec::new();
        let mut expansions = 0;
        let max_expansions = 100;

        while let Some(state) = queue.pop_front() {
            expansions += 1;
            if expansions > max_expansions || ep_paths.len() >= 10 {
                if !state.hops.is_empty() && ep_paths.is_empty() {
                    ep_paths.push(TracePath {
                        path_id: String::new(),
                        entry_point_id: ep.node_id.clone(),
                        hops: state.hops,
                        is_complete: false,
                    });
                }
                break;
            }

            if state.hops.len() >= max_depth_limit {
                ep_paths.push(TracePath {
                    path_id: String::new(),
                    entry_point_id: ep.node_id.clone(),
                    hops: state.hops,
                    is_complete: false,
                });
                continue;
            }

            let mut valid_callees = Vec::new();

            if let Some(edges) = index.outgoing.get(&state.curr_node_id) {
                let mut sorted_edges = edges.clone();
                sorted_edges.sort_by(|a, b| a.target_id.cmp(&b.target_id));

                for edge in sorted_edges {
                    if !matches!(
                        edge.category,
                        RelationCategory::Calls | RelationCategory::References
                    ) {
                        continue;
                    }

                    let target_id = &edge.target_id;
                    if target_id.starts_with("symbol:ext::")
                        || target_id.starts_with("ext:pkg:")
                        || target_id.starts_with("unresolved:")
                    {
                        let ext_name = target_id
                            .strip_prefix("symbol:ext::")
                            .or_else(|| target_id.strip_prefix("ext:pkg:"))
                            .or_else(|| target_id.strip_prefix("unresolved:missing:"))
                            .or_else(|| target_id.strip_prefix("unresolved:ambiguous:"))
                            .unwrap_or(target_id);
                        if is_trivial_builtin_or_method(ext_name) {
                            continue;
                        }

                        let ext_name_low = ext_name.to_ascii_lowercase();
                        let is_external_boundary = ext_name_low.contains("http")
                            || ext_name_low.contains("request")
                            || ext_name_low.contains("url")
                            || ext_name_low.contains("client")
                            || ext_name_low.contains("osv")
                            || ext_name_low.contains("sqlite")
                            || ext_name_low.contains("db")
                            || ext_name_low.contains("fetch")
                            || ext_name_low.contains("socket")
                            || ext_name_low.contains("api");

                        let (link_type, reason) = if is_external_boundary {
                            (
                                "ExternalBoundary".to_string(),
                                format!(
                                    "Reaches external service or client boundary '{}'",
                                    ext_name
                                ),
                            )
                        } else {
                            (
                                "DynamicDispatch".to_string(),
                                format!(
                                    "Target symbol '{}' cannot be statically resolved",
                                    ext_name
                                ),
                            )
                        };

                        let link_key = (state.curr_node_id.clone(), ext_name.to_string());
                        if seen_unresolved.insert(link_key) && all_unresolved.len() < 25 {
                            all_unresolved.push(UnresolvedLink {
                                source_node_id: state.curr_node_id.clone(),
                                source_path: edge.provenance.source_path.clone(),
                                source_line: edge.provenance.source_line.unwrap_or(1) as u32,
                                target_hint: ext_name.to_string(),
                                link_type,
                                reason,
                            });
                        }
                        continue;
                    }

                    if state.visited_in_path.contains(target_id) {
                        continue;
                    }

                    if let Some(target_node) = index.nodes_by_id.get(target_id) {
                        if target_node.category != NodeCategory::Function
                            && target_node.category != NodeCategory::Method
                        {
                            continue;
                        }

                        let t_path_low = target_node.relative_path.to_ascii_lowercase();
                        let t_label_low = target_node.label.to_ascii_lowercase();

                        let (node_kind, is_data_layer) = if t_path_low.contains("storage")
                            || t_path_low.contains("db")
                            || t_path_low.contains("repository")
                            || t_path_low.contains("client")
                            || t_path_low.contains("store")
                            || t_label_low.contains("query")
                            || t_label_low.contains("save")
                            || t_label_low.contains("load")
                            || t_label_low.contains("fetch")
                        {
                            ("DATA_ACCESS".to_string(), true)
                        } else if t_path_low.contains("service")
                            || t_path_low.contains("orchestrator")
                            || t_path_low.contains("engine")
                            || t_path_low.contains("manager")
                            || t_path_low.contains("planner")
                            || t_path_low.contains("evaluator")
                        {
                            ("SERVICE".to_string(), false)
                        } else {
                            ("BUSINESS_LOGIC".to_string(), false)
                        };

                        valid_callees.push((
                            target_node,
                            node_kind,
                            is_data_layer,
                            edge.provenance.source_line,
                        ));
                    }
                }
            }

            if valid_callees.is_empty() {
                if state.hops.len() > initial_hops.len() {
                    ep_paths.push(TracePath {
                        path_id: String::new(),
                        entry_point_id: ep.node_id.clone(),
                        hops: state.hops,
                        is_complete: false,
                    });
                }
            } else {
                for (callee_node, node_kind, is_data_layer, src_line) in valid_callees {
                    let hop = TraceHop {
                        node_id: callee_node.id.clone(),
                        node_kind,
                        display_name: format!("{}()", callee_node.label),
                        source_path: callee_node.relative_path.clone(),
                        start_line: callee_node.start_line.unwrap_or(1) as u32,
                        end_line: callee_node.end_line.unwrap_or(1) as u32,
                        relationship_from_previous: Some("CALLS".to_string()),
                        confidence: TraceConfidence::Exact,
                        evidence: format!(
                            "AST call invocation from line {}",
                            src_line.unwrap_or(1)
                        ),
                    };

                    let mut next_hops = state.hops.clone();
                    next_hops.push(hop);

                    if is_data_layer {
                        ep_paths.push(TracePath {
                            path_id: String::new(),
                            entry_point_id: ep.node_id.clone(),
                            hops: next_hops,
                            is_complete: true,
                        });
                    } else {
                        let mut next_visited = state.visited_in_path.clone();
                        next_visited.push(callee_node.id.clone());
                        queue.push_back(PathState {
                            curr_node_id: callee_node.id.clone(),
                            hops: next_hops,
                            visited_in_path: next_visited,
                        });
                    }
                }
            }
        }

        if ep_paths.is_empty() {
            ep_paths.push(TracePath {
                path_id: String::new(),
                entry_point_id: ep.node_id.clone(),
                hops: initial_hops,
                is_complete: false,
            });
        }

        all_candidate_paths.extend(ep_paths);
    }

    // Filter hops by confidence threshold
    for p in &mut all_candidate_paths {
        p.hops.retain(|h| h.confidence >= min_confidence);
    }
    all_candidate_paths.retain(|p| !p.hops.is_empty());

    // Deduplicate identical paths (same hop node_id sequence)
    let mut unique_paths = Vec::new();
    let mut seen_sequences: HashSet<Vec<String>> = HashSet::new();
    for p in all_candidate_paths {
        let seq: Vec<String> = p.hops.iter().map(|h| h.node_id.clone()).collect();
        if seen_sequences.insert(seq) {
            unique_paths.push(p);
        }
    }

    // Sort paths:
    // 1. Complete paths first (is_complete = true)
    // 2. Highest percentage of Exact confidence hops
    // 3. Shortest path (fewer hops = tighter trace)
    // 4. Lexicographic by entry_point_id
    unique_paths.sort_by(|a, b| {
        b.is_complete
            .cmp(&a.is_complete)
            .then_with(|| {
                let a_exact = a
                    .hops
                    .iter()
                    .filter(|h| h.confidence == TraceConfidence::Exact)
                    .count();
                let a_ratio = if a.hops.is_empty() {
                    0.0
                } else {
                    a_exact as f64 / a.hops.len() as f64
                };
                let b_exact = b
                    .hops
                    .iter()
                    .filter(|h| h.confidence == TraceConfidence::Exact)
                    .count();
                let b_ratio = if b.hops.is_empty() {
                    0.0
                } else {
                    b_exact as f64 / b.hops.len() as f64
                };
                b_ratio
                    .partial_cmp(&a_ratio)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.hops.len().cmp(&b.hops.len()))
            .then_with(|| a.entry_point_id.cmp(&b.entry_point_id))
    });

    let trace_paths: Vec<TracePath> = unique_paths
        .into_iter()
        .take(max_paths_limit)
        .enumerate()
        .map(|(idx, mut p)| {
            p.path_id = format!("path_{}", idx + 1);
            p
        })
        .collect();

    let mut distinct_files = HashSet::new();
    for ep in &trace_entry_points {
        distinct_files.insert(ep.source_path.clone());
    }
    for p in &trace_paths {
        for h in &p.hops {
            distinct_files.insert(h.source_path.clone());
        }
    }

    // Filter unresolved links if not requested
    if req.include_unresolved == Some(false) {
        all_unresolved.clear();
    }

    // Calculate Summary Metrics
    let mut total_hops = 0;
    let mut exact_hops = 0;
    let mut pattern_hops = 0;
    let mut candidate_hops = 0;

    for p in &trace_paths {
        total_hops += p.hops.len();
        for h in &p.hops {
            match h.confidence {
                TraceConfidence::Exact => exact_hops += 1,
                TraceConfidence::Pattern => pattern_hops += 1,
                TraceConfidence::Candidate => candidate_hops += 1,
            }
        }
    }

    let summary = TraceSummary {
        total_paths: trace_paths.len(),
        total_hops,
        exact_hops_count: exact_hops,
        pattern_hops_count: pattern_hops,
        candidate_hops_count: candidate_hops,
        unresolved_links_count: all_unresolved.len(),
        distinct_files_count: distinct_files.len(),
    };

    let result = FeatureTrace {
        trace_id,
        repository_id: repo_id,
        snapshot_id: head_commit,
        query: req.query.clone(),
        graph_lifecycle_state: state,
        entry_points: trace_entry_points,
        paths: trace_paths,
        unresolved_links: all_unresolved,
        summary,
        diagnostics,
    };

    // Output bounds check (max_bytes)
    let max_bytes = req.max_bytes.unwrap_or(16384).clamp(1024, 65536);
    let mut serialized = serde_json::to_string(&result)?;
    if serialized.len() > max_bytes {
        let mut bounded = result;
        while bounded.paths.len() > 1 && serialized.len() > max_bytes {
            bounded.paths.pop();
            serialized = serde_json::to_string(&bounded)?;
        }
        if serialized.len() > max_bytes {
            if let Some(top_path) = bounded.paths.first_mut() {
                if top_path.hops.len() > 4 {
                    top_path.hops.truncate(4);
                }
            }
        }
        bounded.diagnostics.push(format!(
            "Response exceeded {} bytes; bounded candidate paths for payload limits.",
            max_bytes
        ));
        Ok(bounded)
    } else {
        Ok(result)
    }
}

/// Handle MCP tool invocation for `engram_trace_feature`.
pub fn handle_engram_trace_feature(root: &Path, args: &Value) -> Result<Value> {
    let query = match args.get("query").and_then(|q| q.as_str()) {
        Some(q) if !q.trim().is_empty() => q.trim().to_string(),
        _ => bail!("Argument 'query' is required and must not be empty"),
    };

    let scope = match args.get("scope").and_then(|s| s.as_str()) {
        Some(s) => {
            let norm = s.replace('\\', "/");
            if norm.starts_with('/') || norm.contains("..") || norm.starts_with(':') {
                bail!(
                    "Argument 'scope' must be a safe workspace-relative path without parent traversal escapes"
                );
            }
            Some(norm)
        }
        None => None,
    };

    let entry_point_kind = args
        .get("entry_point_kind")
        .and_then(|k| k.as_str())
        .map(|k| k.to_string());

    let max_depth = args
        .get("max_depth")
        .and_then(|d| d.as_u64())
        .map(|d| (d as usize).clamp(1, 10));

    let max_paths = args
        .get("max_paths")
        .and_then(|p| p.as_u64())
        .map(|p| (p as usize).clamp(1, 10));

    let confidence_threshold = args
        .get("confidence_threshold")
        .and_then(|c| c.as_str())
        .map(|c| c.to_string());

    let include_unresolved = args.get("include_unresolved").and_then(|u| u.as_bool());

    let max_bytes = args
        .get("max_bytes")
        .and_then(|b| b.as_u64())
        .map(|b| (b as usize).clamp(1024, 65536));

    let req = TraceRequest {
        query,
        entry_point_kind,
        scope,
        max_depth,
        max_paths,
        confidence_threshold,
        include_unresolved,
        max_bytes,
    };

    let trace = trace_feature(root, &req)?;
    Ok(serde_json::to_value(trace)?)
}
