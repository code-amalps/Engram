# Engram v1.5.0 Release Notes — Protocol 1.1 & Complete Engineering Intelligence

**Release Version:** `v1.5.0`
**Protocol Version:** `1.1` (`PROTOCOL_VERSION = "1.1"` in wire payloads & schemas)
**Release Codename:** `Complete Engineering Intelligence`
**Target Git Branch:** `Development` $\to$ `main`
**Status:** Prepared Release Documentation (Pending Release Commit & Tag Authorization)
**License:** AGPL-3.0-or-later

---

## 1. Overview: Engine v1.5.0 & Protocol 1.1

Engram v1.5.0 delivers the full **Protocol 1.1** specification, completing the 7-pillar architecture for software boundary enforcement, autonomous recovery, and longitudinal engineering governance. Building upon the v1.0 Closed-Loop Change Protocol (Preflight Handshake & Postflight Guard), Engram v1.5.0 introduces bounded recovery recipes for contract violations, append-only cryptographic telemetry, two-tier incremental invalidation, scoped architecture drift detection, and native JVM ecosystem integration.

---

## 2. Key Features & Verified Capabilities

### 1. Stateful Automated Recovery (`engram recover`)
When an AI coding agent violates a Change Contract boundary, Engram generates a machine-readable recovery recipe directing the agent on which files to revert and which to preserve:
- **Bounded Autonomous Retries:** Enforces a strict limit of 2 authorized recovery attempts. A 3rd request halts immediately at `BLOCKED_AWAITING_HUMAN` to prevent infinite LLM repair loops.
- **Atomic Compare-and-Update Locking:** Recovery sessions in `.engram/cache/recovery/` maintain `session_version` and atomic file locks, preventing concurrent agent race conditions.
- **Zero Source Modification:** Engram never modifies source code directly; it emits structured action plans for the agent to execute safely.

### 2. Architecture Drift & Decay Intelligence (`engram drift`)
Tracks codebase drift and institutional memory decay across Git commit history:
- **Scoped Staleness Calculation:** Calculates staleness based strictly on commits that modify the governed symbols or modules since the last explicit ratification date, ignoring unrelated commits.
- **Explicit Ratification Lifecycle:** Provides `engram memory re-ratify` and write-ahead audit event recording, ensuring architectural decisions are actively maintained rather than passively forgotten.
- **Trend Vectors:** Reports boundary violation frequency, repeated contract failures, and decision staleness trends per subsystem.

### 3. Two-Tier Incremental Invalidation Engine (`src/incremental.rs`)
Accelerates preflight and postflight checks on local repositories:
- **Authoritative Content Hashing:** Git blob hashes and working-tree content hashes serve as the authoritative invalidation boundary (`mtime` is used strictly as a fast-path optimization).
- **Tier 1 (Syntactic):** Detects and re-parses only modified files.
- **Tier 2 (Relational):** Invalidates and re-links only the callers, implementors, and importers of mutated symbols in the SQLite relational graph.
- **Measured Surface Reduction:** Demonstrates a 90.0% reduction in reparsed files on warm incremental scans (10 files cold $\to$ 1 file warm) on evaluated benchmark fixtures.

### 4. Append-Only Cryptographic Telemetry (`.engram/events/`)
Provides tamper-evident auditability for enterprise compliance and agent observability:
- **Cryptographic Hash Chain:** Every governance event (preflight, postflight, recovery, ratification, candidate promotion) is linked in an append-only SHA-256 hash chain (`previous_event_hash` $\to$ `event_hash`).
- **Doctor Diagnostics:** `engram doctor` evaluates event chain integrity and flags `EVENT_CHAIN_VALID`, `EVENT_CHAIN_BROKEN`, or `EVENT_MISSING`.

### 5. Java / JVM Ecosystem Support (`src/java_parser.rs`)
Native Java AST parsing integrated into the SQLite symbol graph:
- **Tree-sitter Java Grammar:** Extracts classes, interfaces, records, enums, methods, constructors, and annotations with canonical signatures.
- **Relationship Grounding:** Resolves `IMPLEMENTS`, `EXTENDS`, local imports, and JUnit test-to-source naming conventions.
- **Build Manifest Parsing:** Parses dependencies and plugins in standard Maven `pom.xml` and Groovy `build.gradle` manifests.

### 6. CI/CD Change Guard (`engram guard`)
Automates boundary and Constitution enforcement in continuous integration pipelines:
- Inspects pull request diffs against base branch commits (`--base <ref>`).
- Enforces mandatory change contracts and architectural drift policies with deterministic exit codes.
- Pre-built GitHub Actions workflow configuration (`.github/workflows/engram-guard.yml`).

### 7. Polyglot Flywheel Parity (North-Star 1.1)
The 12-stage closed-loop flywheel is verified functional across 7 programming languages in isolated test fixtures:
- **TypeScript**, **JavaScript**, **Python**, **C#**, **Rust**, **Go**, and **Java / JVM**.

---

## 3. Explicit Disclosures & Documented Limitations

1. **Gradle Build Manifests:** Supports standard Groovy DSL (`build.gradle`); Kotlin DSL (`build.gradle.kts`) is unsupported in v1.5.0.
2. **Maven Build Manifests:** Supports standard single-module `pom.xml` dependency trees; property interpolation (`${version}`) and hierarchical multi-module parent POM inheritance are scheduled for subsequent releases.
3. **Java Wildcard Imports:** Wildcard package imports (`import pkg.*;`) generate package-level relational edges; fine-grained class expansion occurs at blast-radius traversal.
4. **Platform Verification Boundary:** Verification in this release was conducted exclusively on Microsoft Windows Enterprise x64; native POSIX execution on Linux and macOS was not independently established during offline assessment.
5. **MCP Tool List Expansion:** The MCP server exports 15 registered tools (up from 14 in v1.0), adding `engram_recover`, `engram_drift`, and `engram_ratify_decision`. Existing MCP clients expecting a fixed count of 14 tools must update their expectations.
6. **SQLite Storage Lifecycle:** SQLite WAL storage acts as an ephemeral symbol index cache that is automatically rebuilt from source if missing or invalid, not an authoritative long-term database requiring schema migration.
7. **B6 Research Track Exclusion:** The exploratory live-provider token savings research track (Milestone B6) is formally closed as NO-GO and permanently excluded from Engram product deliverables.
8. **Hosted CI Pipelines & Remote Publication:** Release verification was conducted in an isolated local environment; hosted CI pipeline execution (GitHub Actions runners) and remote package publication were not run during this offline assessment.
9. **Symlink and Junction Containment:** Path containment and directory traversal guards are verified across standard paths; on Windows runners without Developer Mode symlink creation privileges (`SeCreateSymbolicLinkPrivilege`), symlink tests fallback gracefully to directory junctions.

---

## 4. Backward Compatibility & Upgrading

- Existing `.engram/memory.yaml` files and contracts load seamlessly.
- Telemetry chains initialize automatically upon the first executed preflight or postflight command in `.engram/events/`.
- Repositories running earlier releases can adopt v1.5.0 without modifying existing codebase files.
- The wire communication protocol conforms strictly to Protocol `1.1`.
