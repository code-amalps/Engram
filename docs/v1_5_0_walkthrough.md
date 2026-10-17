# Engram v1.5.0 — Technical Walkthrough (Protocol 1.1 Closed-Loop Intelligence)

**Document Identifier:** `ENGRAM-V150-WALKTHROUGH-20261017`
**Target Path:** `docs/v1_5_0_walkthrough.md`
**Engine Release Target:** Engram v1.5.0
**Governing Protocol:** Protocol 1.1 (`PROTOCOL_VERSION = "1.1"`)
**Verification Environment:** Microsoft Windows Enterprise x64 (Offline Local Assessment)
**Supported Languages:** TypeScript, JavaScript, Python, C#, Rust, Go, Java/JVM (Flat POM / Groovy Gradle)

---

## 1. Introduction: The Closed-Loop Engineering Flywheel

Modern AI coding agents frequently suffer from two operational failure modes:
1. **Unbounded Mutation & Boundary Erosion:** Agents edit files outside their intended scope, modifying infrastructure configurations, secrets, or protected domain models without authorization.
2. **Context Amnesia & Architecture Drift:** As codebases evolve across multiple commits, agents repeatedly violate architectural decisions made previously because institutional memory is lost.

Engram v1.5.0 (delivering the Protocol 1.1 specification) provides a closed-loop system of preflight handshakes, boundary enforcement, stateful autonomous recovery, append-only cryptographic telemetry, and longitudinal architectural drift detection.

```text
┌────────────────────────────────────────────────────────────────────────┐
│               ENGRAM v1.5.0 CLOSED-LOOP ARCHITECTURE (PROTOCOL 1.1)     │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│   1. Task Description                                                  │
│             │                                                          │
│             ▼                                                          │
│   2. Preflight Handshake ──► Creates Immutable Change Contract        │
│             │                (Defines authorized scopes & boundaries)  │
│             ▼                                                          │
│   3. Agent Implements Code Mutation                                    │
│             │                                                          │
│             ▼                                                          │
│   4. Postflight Guard Check                                            │
│        ├── [PASS] ───────────► 7. Candidate Memory Generated           │
│        └── [VIOLATION]                                                 │
│                │                                                       │
│                ▼                                                       │
│   5. Stateful Recovery Plan ─► Bounded Retry (Max 2 Attempts)          │
│                │                                                       │
│                ▼                                                       │
│   6. Agent Repairs Mutation                                            │
│                │                                                       │
│                ▼                                                       │
│      (Loop back to Postflight)                                         │
│                                                                        │
│   7. Human Promotion Gate ──► Candidate becomes Durable Memory         │
│                               (Zero-Poisoning Invariant)               │
│                                                                        │
│   8. Longitudinal Drift & Staleness Detection                          │
│        └── Evaluates Governed Symbol Commits vs. Ratification Date     │
│                                                                        │
│   9. Human Re-Ratification ──► Clears Staleness & Updates Rationale    │
│                                                                        │
│  10. Subsequent Preflight Grounding ──► Injects Governed Decisions     │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

*Note: In automated verification suites, agent mutations and repairs are evaluated using deterministic synthetic test scenarios without live LLM calls.*

---

## 2. Stage-by-Stage Workflow Guide

### Stage 1 & 2: Task Initiation & Preflight Handshake

Before an engineer or agent modifies code, Engram evaluates the proposed task description, resolves relevant codebase symbols, and establishes an immutable **Change Contract**.

```bash
engram preflight "Add refund capability to BillingService in Java" \
  --forbidden "secrets/**" \
  --token-budget 1500 \
  --json
```

**What Engram Does:**
1. Scans the local Git repository using two-tier incremental invalidation (content hash authoritative).
2. Extracts symbols via Tree-sitter (including Java classes, records, interfaces, and methods).
3. Evaluates active governing architectural decisions in `.engram/memory.yaml`.
4. Emits a strongly typed JSON handshake:

```json
{
  "status": "ReadyForImplementation",
  "contract_id": "ctr_9104bc18a",
  "authorized_scopes": ["src/main/java/com/engram/billing/**"],
  "forbidden_boundaries": ["secrets/**"],
  "governing_policies": [
    {
      "id": "ADR-042",
      "title": "Billing Refund Boundaries",
      "rationale": "Refund operations must not access raw secret stores directly."
    }
  ],
  "token_budget": 1500,
  "protocol_version": "1.1"
}
```

---

### Stage 3 & 4: Code Mutation & Postflight Guard Enforcement

Suppose an agent implements the requested feature in `BillingService.java`, but accidentally mutates `secrets/Vault.java` to access an API key.

When the agent attempts to complete the task, Engram's Postflight Guard intercepts the mutation:

```bash
engram postflight --contract-id ctr_9104bc18a --json
```

**Outcome: Blocked Boundary Violation**
```json
{
  "verdict": "BlockedBoundaryViolation",
  "exit_code": 1,
  "contract_id": "ctr_9104bc18a",
  "boundary_violation_count": 1,
  "violations": [
    {
      "path": "secrets/Vault.java",
      "rule": "FORBIDDEN_BOUNDARY",
      "description": "Modification matches forbidden pattern secrets/**"
    }
  ],
  "protocol_version": "1.1"
}
```

The process exits with code `1`. In CI/CD pipelines, this blocks the pull request via `engram guard`.

---

### Stage 5 & 6: Stateful Automated Recovery & Repair

Instead of requiring manual intervention, Engram generates a machine-readable **Recovery Plan**:

```bash
engram recover --contract-id ctr_9104bc18a --json
```

**Recovery Semantics & Session Tracking:**
- Engram maintains an atomic session in `.engram/cache/recovery/ctr_9104bc18a.json`.
- Uses an atomic CAS lock (`session_version`, `lock_state`) to prevent concurrent agent race conditions.
- Bounded retries: Authorized for a maximum of 2 attempts:
  - `attempt_count: 0` $\to$ Attempt 1 authorized.
  - `attempt_count: 1` $\to$ Attempt 2 authorized (final retry).
  - Subsequent requests $\to$ `RECOVERY_EXHAUSTED` / `BLOCKED_AWAITING_HUMAN`. Engram never executes a 3rd recovery.

**Structured Recovery Plan Output:**
```json
{
  "status": "InProgress",
  "attempt_count": 1,
  "max_attempts": 2,
  "plan": {
    "actions": [
      {
        "type": "REVERT_MUTATION",
        "target": "secrets/Vault.java",
        "instruction": "Revert modifications to forbidden secrets file"
      },
      {
        "type": "PRESERVE_MUTATION",
        "target": "src/main/java/com/engram/billing/BillingService.java",
        "instruction": "Keep authorized refund implementation"
      }
    ]
  },
  "protocol_version": "1.1"
}
```

The agent executes the repair by restoring `secrets/Vault.java`. Re-running postflight yields `verdict: "Pass"` (exit code 0), marks the recovery session as `Resolved`, and generates a **Candidate Memory**.

---

### Stage 7 & 8: Zero-Poisoning Gate & Human Promotion

> [!IMPORTANT]
> **Zero-Poisoning Invariant:** Agent mutations and self-generated candidate memories **never** become durable institutional memory automatically. Explicit human promotion is mandatory.

The agent's successful repair generates a temporary candidate:
```yaml
candidate_id: "cand_3f9104b"
title: "Approved clean Java billing refund implementation"
scope: "src/main/java/com/engram/billing"
status: "CANDIDATE"
```

A human engineer reviews and promotes the decision:
```bash
engram memory promote cand_3f9104b \
  --actor "principal_engineer" \
  --rationale "Verified refund logic conforms to billing security guidelines"
```

The memory is committed to `.engram/memory.yaml` as `Durable`.

---

### Stage 9 & 10: Repository Evolution & Architecture Drift Detection

Software evolves over time. As developers commit to the repository, governing decisions can grow stale or boundaries can erode.

Engram calculates **Scoped Staleness**: it counts only commits that modify the symbols and files governed by a decision, ignoring unrelated commits.

```bash
engram drift --staleness-threshold 5 --json
```

When 6 commits have touched `BillingService.java` without a decision review, Engram detects drift:

```json
{
  "status": "DRIFT_DETECTED",
  "overall_trend": "STALE_REVIEW",
  "stale_decisions": [
    {
      "decision_id": "cand_3f9104b",
      "title": "Approved clean Java billing refund implementation",
      "governed_mutations_since_ratification": 6,
      "staleness_threshold": 5,
      "status": "STALE_REVIEW",
      "action_required": "engram memory re-ratify cand_3f9104b --commit HEAD"
    }
  ],
  "protocol_version": "1.1"
}
```

---

### Stage 11 & 12: Human Re-Ratification & Longitudinal Context Grounding

A lead engineer reviews the changes and re-ratifies the architectural decision:

```bash
engram memory re-ratify cand_3f9104b \
  --commit HEAD \
  --actor "principal_engineer"
```

Engram records an explicit ratification audit event in the cryptographic hash chain. Running `engram drift` now confirms staleness is reset to `0`.

When the agent starts a new task in the same subsystem:
```bash
engram preflight "Update refund execution in BillingService" --json
```
The preflight compiler automatically retrieves the re-ratified decision, grounding the agent with the authoritative rationale and boundaries established across past turns.

---

## 3. Cryptographic Telemetry & Health Verification

All governance events (preflight, postflight, recovery attempts, promotions, ratifications) are recorded in an append-only cryptographic SHA-256 hash chain in `.engram/events/`.

To verify repository telemetry health:
```bash
engram doctor
```

Output:
```text
🩺 Engram System Doctor
──────────────────────────────────────────
  SQLite Graph WAL Storage:     HEALTHY (1,842 symbols indexed)
  Two-Tier Invalidation Cache:  SYNCHRONIZED (Tier 1 Content-Hash Valid)
  Telemetry Event Chain:        EVENT_CHAIN_VALID (64 chained events verified)
  Active Change Contracts:      0 pending (all resolved)
  Governed Decisions:           1 durable (0 stale)
──────────────────────────────────────────
  Status: SYSTEM HEALTHY ✓
```

If any event log has been modified or truncated, `engram doctor` immediately flags `EVENT_CHAIN_BROKEN` or `EVENT_MISSING`.

---

## 4. Supported Polyglot Ecosystem & Known Boundaries

| Language | AST Engine | Dependency Manifests | Status |
|---|---|---|:---:|
| **TypeScript / JS** | Tree-sitter TypeScript / JS | `package.json`, `tsconfig.json` | Fully Supported |
| **Python** | Tree-sitter Python | `pyproject.toml`, `requirements.txt` | Fully Supported |
| **C#** | Tree-sitter C# | `.csproj` | Fully Supported |
| **Rust** | Tree-sitter Rust | `Cargo.toml` | Fully Supported |
| **Go** | Tree-sitter Go | `go.mod` | Fully Supported |
| **Java / JVM** | Tree-sitter Java | Maven `pom.xml`, Gradle `build.gradle` (Groovy) | Supported (Flat POMs & Groovy Gradle) |

*Important Boundaries:*
1. **Multi-Module Maven & Kotlin DSL:** Maven parent POM property interpolation and Gradle Kotlin DSL (`build.gradle.kts`) are scheduled for future minor releases.
2. **Platform Verification Boundary:** The offline verification of these workflows was conducted on Microsoft Windows Enterprise x64. Native Linux and macOS runner verification was not independently established in this assessment.
