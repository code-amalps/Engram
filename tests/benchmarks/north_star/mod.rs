//! North-Star closed-loop flywheel benchmark definitions (Engram v1.1.0 Phase 5).
//! Implements multi-turn closed-loop execution across the 7-language corpus:
//! Task -> Preflight -> Violating Mutation -> Postflight Guard -> Recovery Plan
//! -> Agent Repair -> Postflight Pass -> Human Promotion -> Repo Evolution
//! -> Drift Detection -> Decision Ratification -> Next Preflight Grounding

use serde::{Deserialize, Serialize};

pub const NORTH_STAR_VERSION: &str = "1.1.0";

/// Stages of the North-Star closed-loop engineering flywheel:
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NorthStarStage {
    TaskInitiation,
    PreflightHandshake,
    ViolatingMutation,
    PostflightGuardBlock,
    RecoveryPlanGeneration,
    AgentRepair,
    PostflightGuardPass,
    HumanPromotion,
    RepositoryEvolution,
    DriftDetection,
    DecisionRatification,
    NextPreflightGrounding,
}

impl NorthStarStage {
    pub fn all_stages() -> &'static [NorthStarStage] {
        &[
            NorthStarStage::TaskInitiation,
            NorthStarStage::PreflightHandshake,
            NorthStarStage::ViolatingMutation,
            NorthStarStage::PostflightGuardBlock,
            NorthStarStage::RecoveryPlanGeneration,
            NorthStarStage::AgentRepair,
            NorthStarStage::PostflightGuardPass,
            NorthStarStage::HumanPromotion,
            NorthStarStage::RepositoryEvolution,
            NorthStarStage::DriftDetection,
            NorthStarStage::DecisionRatification,
            NorthStarStage::NextPreflightGrounding,
        ]
    }
}

/// Formal Failure Taxonomy classification for anomalies, edge cases, and known limitations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailureTaxonomyCategory {
    Parser,
    Grounding,
    Impact,
    Contract,
    Git,
    Memory,
    Recovery,
    Drift,
    Incremental,
    Performance,
    Protocol,
    ProductDesign,
}

impl FailureTaxonomyCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Parser => "PARSER",
            Self::Grounding => "GROUNDING",
            Self::Impact => "IMPACT",
            Self::Contract => "CONTRACT",
            Self::Git => "GIT",
            Self::Memory => "MEMORY",
            Self::Recovery => "RECOVERY",
            Self::Drift => "DRIFT",
            Self::Incremental => "INCREMENTAL",
            Self::Performance => "PERFORMANCE",
            Self::Protocol => "PROTOCOL",
            Self::ProductDesign => "PRODUCT_DESIGN",
        }
    }
}

/// Resolution status for classified failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaxonomyResolutionStatus {
    Fixed,
    AcceptedLimitation,
    Deferred,
    ArchitecturalChangeRequired,
}

impl TaxonomyResolutionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fixed => "FIXED",
            Self::AcceptedLimitation => "ACCEPTED_LIMITATION",
            Self::Deferred => "DEFERRED",
            Self::ArchitecturalChangeRequired => "ARCHITECTURAL_CHANGE_REQUIRED",
        }
    }
}

/// Failure Taxonomy Record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxonomyRecord {
    pub category: FailureTaxonomyCategory,
    pub description: String,
    pub resolution: TaxonomyResolutionStatus,
    pub affected_languages: Vec<String>,
}

/// Per-language North-Star execution metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NorthStarLanguageResult {
    pub language: String,
    pub stages_completed: usize,
    pub total_stages: usize,
    pub recovery_success: bool,
    pub zero_poisoning_verified: bool,
    pub drift_detected: bool,
    pub memory_compounding_verified: bool,
    pub execution_ms: f64,
}

/// Comprehensive North-Star 1.1 Benchmark Suite Report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NorthStarReport {
    pub protocol_version: String,
    pub benchmark_version: String,
    pub total_languages: usize,
    pub languages_passed: usize,
    pub language_results: Vec<NorthStarLanguageResult>,
    pub zero_poisoning_invariants_held: bool,
    pub taxonomy_records: Vec<TaxonomyRecord>,
}
