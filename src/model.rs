//! Canonical, serializable Engineering Memory model used by the Rust runtime.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EntityKind {
    Project,
    Module,
    Package,
    File,
    Symbol,
    ApiEndpoint,
    TestEntity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceTier {
    Explicit,
    Verified,
    Extracted,
    Historical,
    Inferred,
    Provisional,
}

impl EvidenceTier {
    /// Higher values take precedence when evidence conflicts.
    pub fn precedence(&self) -> u8 {
        match self {
            Self::Explicit => 6,
            Self::Verified => 5,
            Self::Extracted => 4,
            Self::Historical => 3,
            Self::Inferred => 2,
            Self::Provisional => 1,
        }
    }

    pub fn default_half_life_commits(&self) -> Option<u64> {
        matches!(self, Self::Historical).then_some(200)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub entity_id: String,
    pub kind: EntityKind,
    pub subkind: Option<String>,
    pub name: String,
    pub parent_id: Option<String>,
    pub ast_fingerprint: Option<String>,
    pub signature: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityVersion {
    pub entity_id: String,
    pub revision_id: String,
    pub path: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub name: String,
    pub signature: Option<String>,
    pub ast_fingerprint: Option<String>,
    pub invalidated_at_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub relationship_id: String,
    pub source_id: String,
    pub target_id_or_name: String,
    pub relationship_type: String,
    pub revision_id: String,
    pub invalidated_at_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub evidence_id: String,
    pub target_id: String,
    pub target_type: String,
    pub tier: EvidenceTier,
    pub confidence: f64,
    pub source_type: String,
    pub source_path: Option<String>,
    pub source_line: Option<u32>,
    pub recorded_at: DateTime<Utc>,
    pub decay_half_life_commits: Option<u64>,
    pub verification_status: String,
    pub details: BTreeMap<String, serde_json::Value>,
}

impl Evidence {
    pub fn effective_confidence(&self, commits_since_evidence: u64) -> f64 {
        match self.decay_half_life_commits {
            Some(half_life) if half_life > 0 => {
                self.confidence * 2_f64.powf(-(commits_since_evidence as f64) / half_life as f64)
            }
            _ => self.confidence,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    pub observation_id: String,
    pub target_id: String,
    pub revision_id: String,
    pub property_name: String,
    pub value: serde_json::Value,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inference {
    pub inference_id: String,
    pub target_id: String,
    pub conclusion: serde_json::Value,
    pub evidence_ids: Vec<String>,
    pub confidence: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prescription {
    pub prescription_id: String,
    pub intent: String,
    pub severity: String,
    pub selector: serde_json::Value,
    pub constraint: serde_json::Value,
    pub exceptions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    pub revision_id: String,
    pub commit_hash: Option<String>,
    pub branch_name: Option<String>,
    pub parent_revision_id: Option<String>,
    pub dirty: bool,
    pub analysis_key: String,
    pub recorded_at: DateTime<Utc>,
}

/// Invalidates analysis when content, analyzer, parser, language, or config changes.
pub fn analysis_key(
    content_hash: &str,
    analyzer_version: &str,
    parser_version: &str,
    language_version: &str,
    config_hash: &str,
) -> String {
    let input = format!(
        "{content_hash}\0{analyzer_version}\0{parser_version}\0{language_version}\0{config_hash}"
    );
    format!("sha256:{:x}", Sha256::digest(input.as_bytes()))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ChangeOrigin {
    Agent,
    Human,
    AuthoritativeHumanIntervention,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub change_id: String,
    pub task_id: Option<String>,
    pub revision_id: String,
    pub path: String,
    pub entity_id: Option<String>,
    pub change_kind: String,
    pub origin: ChangeOrigin,
    pub evidence_tier: EvidenceTier,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Gap {
    pub gap_id: String,
    pub task_id: String,
    pub code: String,
    pub description: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub task_id: String,
    pub goal: String,
    pub token_budget: usize,
    pub intent_grounding: serde_json::Value,
    pub state: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeContract {
    pub task_id: String,
    pub version: u32,
    pub revision_id: String,
    pub expected_files: Vec<String>,
    pub expected_entity_ids: Vec<String>,
    pub expected_symbols: Vec<String>,
    pub expected_modules: Vec<String>,
    pub expected_dependencies: Vec<String>,
    pub expected_tests: Vec<String>,
    pub constraints: Vec<String>,
    pub unresolved_gap_ids: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub decision_id: String,
    pub task_id: String,
    pub decision: String,
    pub reason: String,
    pub actor: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub memory_id: String,
    pub task_id: Option<String>,
    pub content: String,
    pub tier: EvidenceTier,
    pub accepted_revision_id: Option<String>,
    pub created_at: DateTime<Utc>,
}
