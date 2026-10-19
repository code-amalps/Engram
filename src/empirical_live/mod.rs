//! Live model and autonomous agent evaluation module for Engram Phase 2B.
//!
//! This module implements the infrastructure required to evaluate live LLMs
//! (and offline mock models during Milestone B1-B4) under strict budget caps,
//! verifiable isolation, and paired statistical accounting.
//!
//! Submodules:
//! - `schema`: Provider-neutral message representations, tool definitions, and token telemetry.
//! - `budget`: Thread-safe pre-flight atomic budget reservation system (`BudgetGuard`).
//! - `provider`: Provider-neutral LLM interface, secret redaction, and guarded execution boundary.
//! - `mock`: Deterministic offline mock LLM provider for tests and benchmarking.
//! - `anthropic`: Anthropic Messages API wire-format adapter and header sanitization.
//! - `dispatcher`: Tool execution engine with path containment and command allowlisting.
//! - `orchestrator`: Multi-turn dialogue loop with fail-closed halt propagation.
//! - `stats`: Paired statistical engine, Holm-Bonferroni correction, and sample sizing.
//! - `report`: Report serialization, markdown formatting, and epistemological disclosure.
//! - `security`: OS security hardening, Windows Job Objects, environment sanitization, and probe.

pub mod anthropic;
pub mod budget;
pub mod dispatcher;
pub mod mock;
pub mod orchestrator;
pub mod provider;
pub mod report;
pub mod schema;
pub mod security;
pub mod stats;
