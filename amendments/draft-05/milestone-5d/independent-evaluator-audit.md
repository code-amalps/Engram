# Independent Evaluator Audit: `task_commit_cache_invalidation`

**Milestone:** Engram Amendment 05 — Milestone 5D  
**Document ID:** `amendments/draft-05/milestone-5d/independent-evaluator-audit.md`  
**Evaluation Date:** 2026-10-30  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Target Module:** `benchmarks/matched-pair-harness/src/evaluator.py`  
**Status:** INDEPENDENTLY AUDITED & FORMALLY DISCLOSED  

---

## 1. Executive Summary

This independent audit evaluates the implementation of the remediated evaluator in `benchmarks/matched-pair-harness/src/evaluator.py` for task `task_commit_cache_invalidation`. Each of the four registered task requirements was examined against its contract, execution mechanism, positive/negative fixture behavior, false-positive vulnerabilities, and fail-closed properties.

The audit confirms that all four requirements must pass simultaneously to achieve $C = 1.00$. Crucially, this audit establishes that **REQ-2 is evaluated via a composite of Cargo L2 unit tests and static AST inspection**, rather than an independent runtime simulation of repository commit drift by the evaluator itself.

---

## 2. Requirement-by-Requirement Contract and Verification Audit

### 2.1 REQ-1: Commit-Bound Metadata Persistence
1. **Task Contract:** Record active Git commit HEAD in SQLite `.engram/index.db` metadata table (`criteria: index.db contains meta table recording git_commit_head`). Weight: 0.25.
2. **Responsible Method:** `Evaluator._check_commit_cache_req1_meta_persistence(sandbox_path)` invoked via `_check_static_requirement(sandbox_path, "REQ-1")`.
3. **Check Nature:** **Dual (Physical Executable + Static AST)**.
   - *Physical DB Check:* If an index database exists (`.engram/index.db` or `.engram/derived_symbols.db`), the evaluator connects using Python's `sqlite3` driver, queries `sqlite_master` for `meta` or `metadata` table, and verifies `git_commit_head` column or key value.
   - *Static AST Check:* If the physical DB has not been materialized, inspects `src/analyzer.rs`, `src/compiler.rs`, `src/storage.rs`, and `src/main.rs` for `CREATE TABLE IF NOT EXISTS meta` and `git_commit_head` insertion logic.
4. **Positive Fixture:** SQLite `.engram/index.db` containing `meta` table with row `key='git_commit_head'`, `value='c70845b4...'`; or source code implementing table definition and commit insertion.
5. **Negative Fixture:** Missing meta table; or database containing unrelated keys only; or source files omitting commit persistence.
6. **False Positive Risk:** Low to Moderate. In the static fallback layer, an agent could define the SQL string without invoking it during indexing; however, subsequent compilation and unit tests constrain purely dead code. When the database exists, verification is direct and physical.
7. **Fail-Closed Behavior:** Verified. Any SQLite operational error or file read exception is caught and returns `False`.

---

### 2.2 REQ-2: Commit Drift Detection and Cache Invalidation
1. **Task Contract:** In `ensure_fresh()`, check whether the current repository HEAD matches the cached commit hash; trigger invalidation of stale file records if commit drift is detected. Weight: 0.25.
2. **Responsible Method:** `Evaluator.evaluate_requirements()` composite check:
   ```python
   elif eval_type == "UNIT_TEST":
       l2 = gate_results.get("L2_unit_tests", "SKIPPED")
       if self.task_id == "task_commit_cache_invalidation" and req_id == "REQ-2":
           drift_ok = self._check_commit_cache_req2_drift_detection(sandbox_path)
           req_results[req_id] = "PASS" if (l2 == "PASS" and drift_ok) else "FAIL"
   ```
3. **Check Nature:** **Composite (Test-Backed Cargo Gate + Static AST Pattern)**.
4. **Special Audit Finding on REQ-2 Runtime Independence:**
   > [!IMPORTANT]
   > The evaluator **does NOT independently simulate commit drift at runtime**. The evaluator does not alter Git HEAD in a live test repo to observe whether `ensure_fresh()` triggers cache invalidation. Instead:
   > - Dynamic runtime execution is delegated entirely to the Cargo L2 unit test gate (`cargo test --offline --lib compiler::tests` or `tests/commit_cache_invalidation_test.rs`).
   > - `_check_commit_cache_req2_drift_detection` verifies that the source code in `src/analyzer.rs` actually incorporates commit comparison logic (`!= cached_commit` / `drift`) or dedicated test coverage.
   > - Both conditions must hold for REQ-2 to pass.
5. **Positive Fixture:** `ensure_fresh()` containing commit comparison against repository HEAD and invalidating stale state (`stale = true`), combined with passing L2 unit tests.
6. **Negative Fixture:** `ensure_fresh()` that only inspects file modification timestamps/hashes without commit comparison; or failing L2 unit tests.
7. **False Positive Risk:** Low. While the AST check alone would be susceptible to keyword spoofing, coupling it with L2 unit test pass requirements prevents non-functional code from passing.
8. **Fail-Closed Behavior:** Verified. If L2 tests fail or time out, or if commit comparison patterns are absent, REQ-2 strictly evaluates to `FAIL`.

---

### 2.3 REQ-3: Cache Purge Subcommand (`engram clean --cache`)
1. **Task Contract:** Implement `engram clean --cache` subcommand to safely purge the AST SQLite database while preserving `.engram/memories.json`. Weight: 0.25.
2. **Responsible Method:** `Evaluator._check_commit_cache_req3_clean_subcommand(sandbox_path)` invoked via `_check_static_requirement(sandbox_path, "REQ-3")`.
3. **Check Nature:** **Dual (Deterministic Executable Subprocess + Static AST)**.
   - *Executable Check:* If compiled binary `target/debug/engram.exe` exists, creates an isolated temporary directory containing a dummy `.engram/index.db` and sentinel `.engram/memories.json`, invokes `engram.exe clean --cache`, and asserts that `index.db` is deleted while `memories.json` remains byte-for-byte identical.
   - *Static AST Check:* Inspects `src/main.rs` for `Clean { cache: bool }` in `enum CommandLine`, verifies unlinking of index/cache database, and asserts a negative check that `memories.json` is not unlinked.
4. **Positive Fixture:** Compiled binary purges dummy SQLite index while preserving sentinel `memories.json`; or source code implementing `Clean { cache: bool }` without memory deletion.
5. **Negative Fixture:** `Clean` subcommand omitting `--cache` option; command failing to delete cache database; or code that deletes `.engram/memories.json`.
6. **False Positive Risk:** Minimal. When L3 build compiles the binary, execution is empirically tested against real files.
7. **Fail-Closed Behavior:** Verified. Any non-zero exit code, lingering cache file, or missing memory file returns `False`.

---

### 2.4 REQ-4: Blast Radius Containment and Regression Prevention
1. **Task Contract:** Changes strictly confined to authorized files (`src/compiler.rs`, `src/analyzer.rs`, `src/main.rs`, `tests/commit_cache_invalidation_test.rs`); zero changes to `Cargo.toml`, `Cargo.lock`, or unauthorized modules; existing compiler tests pass. Weight: 0.25.
2. **Responsible Method:** `Evaluator.evaluate_requirements()` under `eval_type == "AUTOMATED_DIFF"` + `analyze_git_impact()` + `compute_cargo_manifest()`.
3. **Check Nature:** **Automated Diff Analysis & Cryptographic Manifest Verification**.
4. **Positive Fixture:** Working-tree modifications touching only `src/compiler.rs` and `src/analyzer.rs` with zero modifications to package manifests.
5. **Negative Fixture:** Modification to `Cargo.toml`, `Cargo.lock`, or unauthorized files such as `src/mcp.rs`.
6. **False Positive Risk:** Zero. Enforced via `git status --porcelain`, `git diff HEAD --numstat`, and pre/post SHA-256 manifests.
7. **Fail-Closed Behavior:** Verified. If `dependency_mutated == True`, score is forced to $C = 0.00$ and all requirements evaluate to `FAIL`.

---

## 3. Strict Correctness Threshold Invariant

In `Evaluator.calculate_score()`:
$$C = \sum_{i=1}^4 w_i \cdot \mathbb{I}(\text{req}_i = \text{PASS})$$
$$\text{passed\_all\_requirements} \iff (C = 1.00) \land \bigwedge_{i=1}^4 (\text{req}_i = \text{PASS})$$

- If any single requirement fails, maximum attainable score is $C \le 0.75$.
- A score of $C < 1.00$ strictly prevents `passed_all_requirements` from being `True`.
- Under the orchestrator's mutual acceptance rule, any run with $C < 1.00$ invalidates all comparative token savings claims.
