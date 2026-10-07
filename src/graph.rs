//! Structural Dependency Graph Traversal abstraction: neighbors, reverse queries, and depth expansion.

use anyhow::Result;
use rusqlite::Connection;
use std::collections::{HashSet, VecDeque};

use crate::model::{Direction, EvidenceTier, GraphEdge};

pub struct GraphTraversal<'a> {
    pub conn: &'a Connection,
}

impl<'a> GraphTraversal<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Traverse graph neighbors from a starting node (file path or entity ID)
    /// up to the specified depth (1-hop, 2-hop) with edge kind filtering.
    pub fn neighbors(
        &self,
        start_node: &str,
        kinds: &[&str],
        direction: Direction,
        depth: usize,
    ) -> Result<Vec<GraphEdge>> {
        let mut visited_nodes: HashSet<String> = HashSet::new();
        visited_nodes.insert(start_node.to_string());

        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        queue.push_back((start_node.to_string(), 0));

        let mut edges: Vec<GraphEdge> = Vec::new();
        let mut seen_edges: HashSet<(String, String, String)> = HashSet::new();

        while let Some((curr, curr_depth)) = queue.pop_front() {
            if curr_depth >= depth {
                continue;
            }

            let mut query_edges = Vec::new();

            // Outgoing edges: curr -> target
            if direction == Direction::Outgoing || direction == Direction::Both {
                let prefix = format!("{}::%", curr);
                let mut stmt = self.conn.prepare(
                    "SELECT source_id, target, kind, evidence, confidence, path, line
                     FROM relationships
                     WHERE (source_id = ?1 OR source_id LIKE ?2) AND invalidated_at_revision IS NULL"
                )?;
                let rows = stmt.query_map([&curr, &prefix], |r| {
                    Ok(GraphEdge {
                        source: r.get(0)?,
                        target: r.get(1)?,
                        kind: r.get(2)?,
                        evidence_tier: match r.get::<_, String>(3)?.as_str() {
                            "EXPLICIT" => EvidenceTier::Explicit,
                            "VERIFIED" => EvidenceTier::Verified,
                            "HISTORICAL" => EvidenceTier::Historical,
                            "INFERRED" => EvidenceTier::Inferred,
                            "PROVISIONAL" => EvidenceTier::Provisional,
                            _ => EvidenceTier::Extracted,
                        },
                        confidence: r.get(4)?,
                        path: r.get(5)?,
                        line: r.get::<_, Option<i64>>(6)?.map(|l| l as usize),
                    })
                })?;
                for e in rows {
                    query_edges.push((e?, true));
                }
            }

            // Incoming edges: source -> curr (reverse query)
            if direction == Direction::Incoming || direction == Direction::Both {
                let prefix = format!("{}::%", curr);
                let mut stmt = self.conn.prepare(
                    "SELECT source_id, target, kind, evidence, confidence, path, line
                     FROM relationships
                     WHERE (target = ?1 OR target LIKE ?2) AND invalidated_at_revision IS NULL"
                )?;
                let rows = stmt.query_map([&curr, &prefix], |r| {
                    Ok(GraphEdge {
                        source: r.get(0)?,
                        target: r.get(1)?,
                        kind: r.get(2)?,
                        evidence_tier: match r.get::<_, String>(3)?.as_str() {
                            "EXPLICIT" => EvidenceTier::Explicit,
                            "VERIFIED" => EvidenceTier::Verified,
                            "HISTORICAL" => EvidenceTier::Historical,
                            "INFERRED" => EvidenceTier::Inferred,
                            "PROVISIONAL" => EvidenceTier::Provisional,
                            _ => EvidenceTier::Extracted,
                        },
                        confidence: r.get(4)?,
                        path: r.get(5)?,
                        line: r.get::<_, Option<i64>>(6)?.map(|l| l as usize),
                    })
                })?;
                for e in rows {
                    query_edges.push((e?, false));
                }
            }

            for (edge, is_outgoing) in query_edges {
                if !kinds.is_empty() && !kinds.contains(&edge.kind.as_str()) {
                    continue;
                }
                let key = (edge.source.clone(), edge.target.clone(), edge.kind.clone());
                if seen_edges.insert(key) {
                    let next_node = if is_outgoing {
                        edge.target.split("::").next().unwrap_or(&edge.target).to_string()
                    } else {
                        edge.source.split("::").next().unwrap_or(&edge.source).to_string()
                    };
                    if !visited_nodes.contains(&next_node) {
                        visited_nodes.insert(next_node.clone());
                        queue.push_back((next_node, curr_depth + 1));
                    }
                    edges.push(edge);
                }
            }
        }

        Ok(edges)
    }

    /// "Who will break if I change target?"
    /// Retrieves all files and symbols that import, call, implement, or test target.
    pub fn reverse_dependencies(&self, target: &str) -> Result<Vec<GraphEdge>> {
        self.neighbors(target, &[], Direction::Incoming, 1)
    }
}
