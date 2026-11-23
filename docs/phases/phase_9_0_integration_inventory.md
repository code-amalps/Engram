# Engram Phase 9.0: Integration Inventory & Technical Specifications

**Phase:** 9.0  
**Document Identifier:** `ENGRAM-DOC-PHASE-9-0-INVENTORY-20261123`  
**Phase Type:** Integration Implementation / Coding Completion  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Repository:** `code-amalps/Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Expected Starting HEAD:** `d01f84d4ac358d8f6c9ec396730b5eec0d877929`  
**Date:** 2026-11-23  
**Status:** `ACTIVE_INVENTORY`

---

## 1. Executive Overview

Phase 9.0 establishes real, production-ready integration engineering between Engram and two key external ecosystem capabilities:
1. **AMstraLift Enterprise Modernization & Dependency-Upgrade Engine:** Ingesting modernization plans, dependency upgrade proposals, breaking change warnings, and transformation/refusal evidence into Engram's contract and change-guard boundaries.
2. **Graphify Macro-Knowledge Graph Intelligence:** Ingesting Graphify macro-graph topologies (`graphify-out/graph.json`), community clusters, and cross-document/module relationships into Engram's change-impact and task-context subsystems.

Both integrations strictly observe Engram's core architectural tenets:
- **Zero-Trust for External Recommendations:** External modernization plans, upgrade proposals, and graph nodes are treated as unverified inputs until validated against Engram Change Contracts and Project Constitution rules.
- **Contract Boundary Supremacy:** An external engine cannot mutate files without a ratifying or active Change Contract.
- **Clear Evidence Tier Separation:** Graphify macro-graph relationships are explicitly tagged as `is_graph_derived: true` and mapped to `EvidenceTier::Inferred`, never masquerading as source-code-confirmed AST evidence.
- **Fail-Closed Failure Modes:** Missing files, corrupt JSON, stale graph versions, cross-repo mismatches, and path traversals fail safely with explicit, typed errors.

---

## 2. Inventory of Integration Requirements

### Requirement 9.0-REQ-01: AMstraLift Modernization Plan Ingestion & Contract Mapping
- **Requirement Identifier:** `9.0-REQ-01`
- **Source Specification:** `AMstraLift: amstralift/planning/models.py` (`ModernizationPlan`, `ExecutionPhase`, `PlannedTransformation`, `PhaseType`, `TransformationStatus`), `ENGRAM_V1_6_EXECUTION_ROADMAP_AND_METRICS.md` (Phase 9).
- **Current Implementation Status:** `NOT_IMPLEMENTED` (New Module: `src/amstralift.rs`).
- **Required Components:**
  - `src/amstralift.rs`: Rust structs mirroring AMstraLift declarative YAML/JSON modernization plans.
  - Plan validation engine: verifies plan metadata, phase order, transformation status (`safe`, `risky`, `refused`), and targets.
  - Contract mapper: converts AMstraLift modernization transformations into an Engram `ChangeContract` or preflight boundary validation.
- **Data Contract:**
  ```json
  {
    "plan_id": "plan_abc123",
    "ecosystem": "angular | dotnet | python | react",
    "current_version": "16.0.0",
    "target_version": "18.0.0",
    "target_major": 18,
    "total_transformations": 5,
    "phases": [
      {
        "phase_index": 0,
        "name": "Dependencies",
        "phase_type": "dependencies",
        "transformations": [
          {
            "id": "dep_upgrade_angular_core",
            "name": "Upgrade @angular/core to v18",
            "category": "ast",
            "risk": "low",
            "status": "safe",
            "affected_file_patterns": ["package.json"]
          }
        ]
      }
    ]
  }
  ```
- **Dependencies:** `serde`, `serde_json`, `serde_yaml`. No dynamic Python bindings required (pure Rust deserializer and validator).
- **Security Boundaries:**
  - Target paths in `affected_file_patterns` must be validated against path traversal (`..`, absolute paths, symlink escape).
  - Transformations targeting sensitive files (`is_security_file_path`, `is_policy_file`) must be flagged as `HIGH_RISK` and require human ratification.
  - Forbidden constitutional boundaries (`constitution.yaml`) override AMstraLift recommendations.
- **Failure Behavior:** Malformed plan YAML/JSON, unsupported schema, or unauthorized targets return `Err(AmstraliftError::InvalidPlan(reason))` and fail closed.
- **Implementation Priority:** P1.
- **Acceptance Criteria:** Engram can parse AMstraLift YAML/JSON plans, validate file patterns, detect constitutional violations, and generate corresponding Change Contract boundaries.

---

### Requirement 9.0-REQ-02: AMstraLift Transformation & Refusal Evidence Ingestion
- **Requirement Identifier:** `9.0-REQ-02`
- **Source Specification:** `AMstraLift: amstralift/evidence/models.py` (`TransformationResult`, `TransformationRecord`, `RefusalRecord`, `RefusalCategory`).
- **Current Implementation Status:** `NOT_IMPLEMENTED` (New Module: `src/amstralift.rs`).
- **Required Components:**
  - `src/amstralift.rs`: Rust models for transformation and refusal records.
  - Evidence verification engine: verifies that applied transformations match actual file diffs and refusal records guarantee `code_modified == false`.
  - Provenance logger: records modernization audit trail into Engram telemetry/storage.
- **Data Contract:**
  ```json
  {
    "id": "res_8f3a12",
    "adapter": "angular",
    "rule_id": "control-flow",
    "file_path": "src/app/nav.component.html",
    "decision": "APPLIED | REFUSED",
    "transformation_record": {
      "ast_nodes_changed": 4,
      "diff_snippet": "...",
      "sandbox_verified": true
    },
    "refusal_record": {
      "category": "unsupported_pattern",
      "reason": "Complex nested ngIf-else not deterministically convertible",
      "code_modified": false
    }
  }
  ```
- **Dependencies:** `serde`, `serde_json`, `chrono`.
- **Security Boundaries:**
  - Verify `code_modified == false` for all `RefusalRecord` items; if any refused record modified source code on disk, emit a security breach finding.
  - Staged diff must match recorded transformations before pre-commit authorization.
- **Failure Behavior:** Unverified transformations or discrepancies between evidence and working tree reject commit authorization.
- **Implementation Priority:** P1.
- **Acceptance Criteria:** Engram validates transformation evidence records, rejects unverified or tampered operations, and records modernization provenance.

---

### Requirement 9.0-REQ-03: Graphify Macro-Knowledge Graph Ingestion & Schema Validation
- **Requirement Identifier:** `9.0-REQ-03`
- **Source Specification:** `Graphify Skill: C:\Users\Amal P S\.gemini\config\skills\graphify\SKILL.md`, `AMstraLift/graphify-out/graph.json`, `manifest.json`.
- **Current Implementation Status:** `NOT_IMPLEMENTED` (New Module: `src/graphify.rs`).
- **Required Components:**
  - `src/graphify.rs`: High-performance streaming/DOM parser for Graphify `graph.json`.
  - Schema validator: verifies `nodes`, `links`, `hyperedges`, and `built_at_commit`.
  - Path normalization & workspace boundary validator.
- **Data Contract:**
  ```json
  {
    "directed": false,
    "multigraph": false,
    "built_at_commit": "0f95d4a60e8696c69fb632812806bc618ce60393",
    "nodes": [
      {
        "id": "amstralift_adapters_angular_angularadapter",
        "label": "AngularAdapter",
        "_origin": "ast",
        "community": 1,
        "community_name": "Adapter Pipelines",
        "file_type": "code",
        "source_file": "amstralift/adapters/angular.py",
        "source_location": "L45"
      }
    ],
    "links": [
      {
        "source": "amstralift_adapters_angular_angularadapter",
        "target": "amstralift_adapters_base_baseadapter",
        "relation": "inherits_from",
        "_origin": "ast",
        "confidence": "EXTRACTED",
        "confidence_score": 1.0,
        "source_file": "amstralift/adapters/angular.py"
      }
    ]
  }
  ```
- **Dependencies:** `serde`, `serde_json`.
- **Security Boundaries:**
  - Path traversal protection: all `source_file` fields must resolve to canonical repository paths inside the workspace root.
  - Cross-repo protection: validates repository matching (`.graphify_root` or repository path).
- **Failure Behavior:** Missing file returns `Ok(None)` without crashing; malformed JSON or invalid schema returns typed `Err(GraphifyError::MalformedGraph(e))`.
- **Implementation Priority:** P1.
- **Acceptance Criteria:** Engram reads and deserializes `graphify-out/graph.json` without panic, validating nodes, links, and workspace boundaries.

---

### Requirement 9.0-REQ-04: Macro-Graph Staleness Detection & Git HEAD Synchronization
- **Requirement Identifier:** `9.0-REQ-04`
- **Source Specification:** Graphify caching protocol (`manifest.json`, `built_at_commit`), `src/git.rs`.
- **Current Implementation Status:** `NOT_IMPLEMENTED` (New Module: `src/graphify.rs`).
- **Required Components:**
  - Git revision comparator: resolves current `HEAD` commit via `crate::git::resolve_baseline`.
  - Staleness evaluation: compares `built_at_commit` with `HEAD`. If commits diverge, tags graph evidence with `StalenessStatus::Stale { built: String, head: String }`.
- **Data Contract:**
  ```rust
  pub enum GraphStaleness {
      Fresh { commit: String },
      Stale { built_at: String, current_head: String, commits_behind: Option<usize> },
      Unknown { reason: String },
  }
  ```
- **Security Boundaries:** Stale graph data MUST NEVER be treated as authoritative for security checks or contract boundaries.
- **Failure Behavior:** Git errors or missing `built_at_commit` default to `GraphStaleness::Unknown`.
- **Implementation Priority:** P1.
- **Acceptance Criteria:** Macro-graph correctly identifies whether it is fresh or stale against current Git HEAD.

---

### Requirement 9.0-REQ-05: Macro-Graph Change Impact Expansion & Community Context
- **Requirement Identifier:** `9.0-REQ-05`
- **Source Specification:** `src/impact.rs`, `src/graph.rs`, Graphify community detection.
- **Current Implementation Status:** `NOT_IMPLEMENTED` (`src/graphify.rs` + integration with `src/impact.rs`).
- **Required Components:**
  - `GraphifyIndex`: In-memory bidirectional adjacency graph supporting node lookup by source file, community membership, and multi-hop link traversal.
  - Impact cone enrichment: functions to discover related files across module boundaries based on macro-graph communities and hyperedges.
  - Evidence tier tagging: Macro-graph edges are mapped to `EvidenceTier::Inferred` with `is_graph_derived: true`.
- **Data Contract:**
  ```rust
  pub struct MacroGraphImpactCandidate {
      pub file_path: String,
      pub relation: String,
      pub confidence_score: f64,
      pub community_id: Option<usize>,
      pub community_name: Option<String>,
      pub is_graph_derived: bool,
  }
  ```
- **Security Boundaries:** Macro-graph impact candidates predict relevance for context retrieval, but CANNOT bypass Change Contract boundary enforcement or Project Constitution rules.
- **Failure Behavior:** If macro-graph is absent or invalid, impact calculation falls back cleanly to native SQLite AST graph without degradation.
- **Implementation Priority:** P1.
- **Acceptance Criteria:** When `graphify-out/graph.json` is present, impact analysis can enrich its context cone with community and cross-module links.

---

### Requirement 9.0-REQ-06: CLI & MCP Integration Diagnostics
- **Requirement Identifier:** `9.0-REQ-06`
- **Source Specification:** `src/main.rs`, `src/mcp.rs`.
- **Current Implementation Status:** `NOT_IMPLEMENTED`.
- **Required Components:**
  - CLI subcommands in `src/main.rs`:
    - `engram amstralift validate-plan <path>`: Ingests and checks modernization plan against contracts and constitution.
    - `engram graphify inspect`: Inspects `graphify-out/`, reports node count, edge count, communities, and staleness vs HEAD.
  - MCP context compilation integration: optionally incorporates macro-graph context into `engram_get_context` or Turn-0 directives if present.
- **Security Boundaries:** Exposes zero internal filesystem credentials or sensitive paths in diagnostics.
- **Failure Behavior:** CLI outputs clean error messages with exit codes 0 (pass), 1 (validation block), 2 (error).
- **Implementation Priority:** P2.
- **Acceptance Criteria:** `engram amstralift` and `engram graphify` commands run cleanly from CLI with JSON and text output support.

---

## 3. Deliverables and Traceability Matrix

| Requirement | Implementation Module | Test File | Target Status |
| :--- | :--- | :--- | :---: |
| `9.0-REQ-01` | `src/amstralift.rs` | `tests/amstralift_integration_test.rs` | **P1 (Core)** |
| `9.0-REQ-02` | `src/amstralift.rs` | `tests/amstralift_integration_test.rs` | **P1 (Core)** |
| `9.0-REQ-03` | `src/graphify.rs` | `tests/graphify_integration_test.rs` | **P1 (Core)** |
| `9.0-REQ-04` | `src/graphify.rs` | `tests/graphify_integration_test.rs` | **P1 (Core)** |
| `9.0-REQ-05` | `src/graphify.rs`, `src/impact.rs` | `tests/graphify_integration_test.rs` | **P1 (Core)** |
| `9.0-REQ-06` | `src/main.rs` | CLI verification | **P2 (CLI/MCP)** |
