//! Generic Pass-Through Framework Adapter.
//!
//! Language- and framework-agnostic default provider that discovers
//! companions using convention-free AST relationships and co-located tests.

use rusqlite::Connection;
use std::path::Path;

use super::{ArchitecturalCompanion, CompanionKind, FrameworkRelationshipProvider};

pub struct GenericPassThroughAdapter;

impl GenericPassThroughAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GenericPassThroughAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameworkRelationshipProvider for GenericPassThroughAdapter {
    fn name(&self) -> &'static str {
        "generic"
    }

    fn detect(&self, _workspace_root: &Path, _files: &[String]) -> bool {
        // Fallback provider matches any workspace
        true
    }

    fn expand_companions(
        &self,
        workspace_root: &Path,
        conn: &Connection,
        seed_path: &str,
    ) -> Vec<ArchitecturalCompanion> {
        let mut companions = Vec::new();
        let clean = seed_path.replace('\\', "/");

        // 1. Co-located test detection across common language conventions
        let test_candidates = if clean.ends_with(".rs") {
            let stem = clean.trim_end_matches(".rs");
            vec![
                format!("{}_test.rs", stem),
                format!("tests/test_{}.rs", stem.split('/').last().unwrap_or(stem)),
            ]
        } else if clean.ends_with(".go") {
            let stem = clean.trim_end_matches(".go");
            vec![format!("{}_test.go", stem)]
        } else if clean.ends_with(".java") {
            let stem = clean.trim_end_matches(".java");
            vec![format!("{}Test.java", stem)]
        } else if clean.ends_with(".ts") || clean.ends_with(".js") {
            let (stem, ext) = if clean.ends_with(".ts") {
                (clean.trim_end_matches(".ts"), "ts")
            } else {
                (clean.trim_end_matches(".js"), "js")
            };
            vec![
                format!("{}.spec.{}", stem, ext),
                format!("{}.test.{}", stem, ext),
            ]
        } else {
            Vec::new()
        };

        for candidate in test_candidates {
            if workspace_root.join(&candidate).exists() {
                companions.push(ArchitecturalCompanion {
                    path: candidate.clone(),
                    kind: CompanionKind::RelatedTest,
                    confidence: 0.95,
                    reason: format!("Generic co-located test companion for {}", clean),
                    exists_on_disk: true,
                });
            }
        }

        // 2. Discover direct AST dependencies via relationships table
        if let Ok(mut stmt) = conn.prepare(
            "SELECT DISTINCT target, kind FROM relationships WHERE source_id = ?1 OR source_id LIKE ?2",
        ) {
            let prefix = format!("{}::%", clean);
            if let Ok(rows) = stmt.query_map(rusqlite::params![clean, prefix], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            }) {
                for row in rows.flatten() {
                    let target_path = row.0.replace('\\', "/");
                    let file_part = target_path.split("::").next().unwrap_or(&target_path);
                    if file_part != clean && workspace_root.join(file_part).exists() {
                        let kind = if row.1 == "TESTS" {
                            CompanionKind::RelatedTest
                        } else if row.1 == "IMPLEMENTS" {
                            CompanionKind::InterfaceModel
                        } else {
                            CompanionKind::InjectedService
                        };
                        companions.push(ArchitecturalCompanion {
                            path: file_part.to_string(),
                            kind,
                            confidence: 0.85,
                            reason: format!("AST relationship {} from {}", row.1, clean),
                            exists_on_disk: true,
                        });
                    }
                }
            }
        }

        companions
    }

    fn task_companion_seeds(
        &self,
        _workspace_root: &Path,
        _conn: &Connection,
        _task: &str,
    ) -> Vec<ArchitecturalCompanion> {
        // Generic provider has no framework-specific task seed discovery
        Vec::new()
    }
}
