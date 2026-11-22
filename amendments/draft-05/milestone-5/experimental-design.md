# Milestone 5A: Experimental Design Specification
## Matched-Pair Controlled Token Efficiency Experiment

**Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/async-precommit-guard`  
**HEAD Commit:** `c70845b4d5913457f1b176fb0370b80dd792711f`  
**Document ID:** `ENGRAM-A05-M5A-EXPERIMENTAL-DESIGN`  
**Classification:** Experimental Protocol Specification  

---

### 1. Central Hypothesis and Experimental Objective

The central hypothesis of this controlled experiment is:

$$\mathbf{H_1}: \text{Providing Engram-compiled engineering context reduces total AI-agent token consumption per successfully completed task compared to an identical unassisted coding-agent workflow.}$$

$$\mathbf{H_0}: \text{Total AI-agent token consumption per successfully completed task with Engram-compiled context is greater than or equal to that of an unassisted coding-agent workflow.}$$

To support $H_1$, token reductions must be achieved **without compromising**:
1. Functional correctness (all unit, integration, and acceptance tests pass).
2. Code quality and architectural consistency.
3. Regression prevention (zero pre-existing tests broken).
4. Task completion fidelity (complete implementation without partial placeholders).

---

### 2. Matched-Pair Experimental Structure

The experiment evaluates paired executions across two strictly isolated experimental arms.

```
                  +-----------------------------------+
                  |   Task Specification & Baseline   |
                  +-----------------+-----------------+
                                    |
            +-----------------------+-----------------------+
            |                                               |
            v                                               v
    [ Arm A: Baseline ]                           [ Arm B: Engram-Assisted ]
   - Identical Prompt                             - Identical Prompt
   - Standard Shell Tools                         - Standard Shell Tools
   - No Engram Context                            - Engram Context Package
   - Isolated Workspace A                         - Isolated Workspace B
   - Clean Starting Git State                     - Clean Starting Git State
            |                                               |
            +-----------------------+-----------------------+
                                    |
                                    v
                     +-----------------------------+
                     | Independent Acceptance Gate |
                     |  - Blinded Test Runner      |
                     |  - Functional Correctness   |
                     |  - Telemetry Reconciliation |
                     +-----------------------------+
```

---

### 3. Detailed Arm Specifications

#### 3.1 Arm A — Native Unassisted Baseline (Control)
The coding agent receives:
- **Task Prompt:** The raw task description, user requirements, and target acceptance criteria.
- **Repository Baseline:** Clean, pristine checkout of the target repository at the designated baseline commit.
- **Tools:** Standard coding-agent capabilities:
  - File reading / inspection (`Get-Content`, `cat`, regex grep).
  - Directory listing (`Get-ChildItem`, `ls`).
  - File editing / patching (`apply_patch`, file writes).
  - Local test and build commands (`cargo test`, `npm test`, etc.).
- **Restrictions:**
  - **Zero Engram context:** No AST excerpts, dependency maps, or symbol summaries provided in the prompt.
  - **Zero Engram MCP tools:** No background Engram daemon or MCP tools configured.
  - **Zero Turn-0 injection:** No `.github/copilot-instructions.md` directives or Change Contracts injected.
  - **Zero Engram environment variables:** Standard shell environment.

#### 3.2 Arm B — Engram-Assisted (Treatment)
The coding agent receives:
- **Task Prompt:** The identical raw task description and acceptance criteria, appended with a pre-compiled **Engram Context Package** generated via:
  ```powershell
  engram prompt --subpath <path> --language <lang> --token-budget <B> "<task>"
  ```
- **Context Package Contents:**
  - AST-grounded signatures of relevant functions, types, and structs.
  - Co-change and blast-radius dependency awareness.
  - Test locations and targeted verification guidance.
  - Context Completeness metadata (active subpath and language scope).
- **Repository Baseline:** Identical clean checkout at the exact same commit in an isolated workspace.
- **Tools & Permissions:** **100% identical** to Arm A (same shell, same editor, same test tools).
- **Model Configuration:** Identical model ID, temperature, reasoning effort, and max tokens.

---

### 4. Treatment Isolation Rationale (Primary vs. Secondary Mechanisms)

Historical audits (Blocks 01–07) revealed that combining multiple treatment mechanisms introduces severe confounding:
- In Block 07, background MCP tool calls suffered client approval rejections (`approval_policy: never`).
- Turn-0 Change Contracts locked authorized targets too narrowly, causing agent refusal.
- If MCP tools, Change Contracts, and Prompt Context are enabled simultaneously, it is impossible to determine which component drove token changes.

**Methodological Decision:**
The **Primary Treatment** for Milestone 5 is **Generated Context Only (Option 1)**:
1. Engram compiles the repository context package offline into the Turn-0 task prompt.
2. No MCP daemon runs during the agent execution, eliminating client approval deadlocks and daemon overhead.
3. No restrictive Turn-0 Change Contract is enforced, preventing artificial agent refusal.
4. Both arms operate with identical toolsets, ensuring that any observed efficiency delta is attributable solely to **the presence of high-precision architectural context**.

*(Note: MCP interactive assistance and Change Contract verification gates are designated as secondary treatments for subsequent, separate experimental phases).*

---

### 5. Strict Matching and Isolation Invariants

| Dimension | Matching Requirement | Enforcement Mechanism |
| :--- | :--- | :--- |
| **Workspace Isolation** | Fully independent directories | Created in temporary directories (`C:\temp\engram_exp_arm_a_...` and `..._arm_b_...`); deleted post-run. |
| **Git Baseline State** | Bit-for-bit identical HEAD | Both cloned or checked out from the pristine baseline commit; validated by `git rev-parse HEAD`. |
| **Model Identity** | Exact model version | Fixed model parameter (e.g. `gpt-5.6-luna` or specified provider model). |
| **Reasoning Effort** | Identical reasoning configuration | Fixed (e.g., `medium` or `default`). |
| **Agent Toolset** | Identical native capabilities | Exactly the same command execution and patch tools in both arms. |
| **Token Exposure Ceiling** | Identical per-arm cap | Strictly identical nominal stop threshold and hard kill ceiling. |
| **Turn Timeout** | Identical wall-clock limit | Equal timeout per turn (e.g. 300 seconds) and total run (e.g. 1,200 seconds). |
| **Acceptance Criteria** | Identical test suites | Automated test harness executes identical verification scripts on both arms. |
| **Dependency Availability** | Identical pre-cached packages | All dependencies (`cargo` crates, `node_modules`) pre-installed offline before execution begins. |

---

### 6. Execution Order & Counterbalancing Strategy

To eliminate systematic order bias (e.g., local OS disk cache warming, transient provider API latency, or rate-limiting throttling):
1. **Randomized Block Schedule:** For $N$ matched pairs, the execution order within each pair is randomly assigned:
   - Pair 1: Arm A (Baseline) $\rightarrow$ Arm B (Treatment)
   - Pair 2: Arm B (Treatment) $\rightarrow$ Arm A (Baseline)
   - Pair 3: Arm A (Baseline) $\rightarrow$ Arm B (Treatment)
   - *(Generated via a cryptographically secure offline pseudorandom seed)*.
2. **Warm-Up Runs:** Prior to pair execution, an un-measured warm-up task is executed to prime OS filesystem buffers and local network/socket caches.
3. **Inter-Run Cooling Interval:** A mandatory pause of at least 60 seconds is enforced between runs to allow API rate limits and local system memory to settle.

---

### 7. Threat Mitigation Matrix

| Potential Confound / Threat | Mechanism of Action | Experimental Mitigation |
| :--- | :--- | :--- |
| **Prompt Injection Variance** | Engram context formatting alters model instruction adherence. | Standardized markdown template with explicit, neutral separation between task requirements and context excerpts. |
| **Context Over-Pruning** | Subpath filter omits critical dependencies, penalizing Arm B. | Subpath filter scoped to parent subsystem directory; verified by static inspection prior to freeze. |
| **Agent Edit Variance** | Non-deterministic sampling causes random retry loops. | Evaluate multiple matched pairs ($N \ge 3$); analyze turns and reasoning tokens separately. |
| **Stale Cache Bias** | Arm B benefits from previously generated Engram SQLite index. | Index is pre-generated and verified offline for both arms before timing begins; indexing time recorded separately. |
| **Harness Short-Circuiting** | Failing runs skewing acceptance scores. | Blinded test runner evaluates all generated diffs against full test suite regardless of exit status. |
