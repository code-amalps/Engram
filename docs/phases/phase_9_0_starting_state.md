# Engram Phase 9.0: Starting State Baseline & Discrepancy Verification

**Phase:** 9.0  
**Phase Title:** Original Roadmap Reconciliation, Remaining Engineering & Coding Closure  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Repository:** `code-amalps/Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Expected Starting HEAD:** `d01f84d4ac358d8f6c9ec396730b5eec0d877929`  
**Actual Current HEAD:** `367286f94d10b819986f50780226d014c5d7f9fe`  
**Remote Branch HEAD:** `367286f94d10b819986f50780226d014c5d7f9fe` (`origin/feature/phase2-stage5-remediation`)  
**Parent of HEAD (Expected Baseline):** `d01f84d4ac358d8f6c9ec396730b5eec0d877929` (Phase 8.2.1)  
**Governing Baseline Candidate:** `v1.5.5-cand1` (source commit: `ded9ca521024d18248235a33e2e292964b79f9b0`)  
**Previous Certified Release:** `v1.5.4-cand5` at commit `2df1cac683435314ffd7ad52369e6d20198fae0d`  
**Date of Verification:** 2026-11-23  
**Working Tree Status:** Clean  
**Discrepancy Status:** `STARTING_HEAD_MISMATCH_DETECTED`

---

## 1. Baseline Verification & Discrepancy Audit

In accordance with Section 3 ("Establish and verify the baseline"):
1. **Repository Path:** `C:\Users\Amal P S\OneDrive\Projects\Engram` (Confirmed).
2. **Current Branch:** `feature/phase2-stage5-remediation` (Confirmed).
3. **Remote Branch:** `origin/feature/phase2-stage5-remediation` fetched and verified without history modification.
4. **Ancestry & Commit Identification:**
   - The user's directive specifies **Expected Starting HEAD:** `d01f84d4ac358d8f6c9ec396730b5eec0d877929` (Phase 8.2.1 closure).
   - The active local and remote HEAD is `367286f94d10b819986f50780226d014c5d7f9fe`.
   - Inspection of `367286f` reveals:
     - Title: `feat(integration): implement AMstraLift modernization and Graphify macro-graph adapters`
     - Author: `Amal PS <amaljunks@gmail.com>`
     - Date: `Mon Nov 23 19:38:10 2026 +0530`
     - Parent: `d01f84d4ac358d8f6c9ec396730b5eec0d877929`
     - Contents: 11 files changed (+2,599 lines), adding AMstraLift and Graphify modules, tests, CLI subcommands, and Phase 9.0 integration docs.
5. **Direct Discrepancy with User Directive:**
   - The user's directive states:
     > *"AMstraLift integration or dependency on the separate AMstraLift project is NOT part of this phase."*  
     > *"Graphify integration, bridge, graph ingestion, or runtime dependency is NOT part of this phase."*  
     > *"AMstraLift and Graphify must remain outside Engram's implementation scope unless an authoritative original Engram requirement explicitly proves otherwise."*  
     > *"Do not reset, clean, force-push, rebase, or rewrite history. If the starting HEAD differs, stop implementation and report the discrepancy."*
   - Therefore, commit `367286f` contains changes that are explicitly ruled **OUT OF SCOPE** by the directive, and HEAD does not equal the expected starting commit `d01f84d4ac358d8f6c9ec396730b5eec0d877929`.

---

## 2. Existing Tags and Release References

Inspection of repository tags via `git tag -l` reveals:
- Historical releases: `v0.3.0` through `v1.5.4`
- Latest certified release tag: `v1.5.4-cand5` at commit `2df1cac683435314ffd7ad52369e6d20198fae0d`
- Candidate baseline `v1.5.5-cand1` has NO Git tag, preserving its immutable candidate state at commit `ded9ca521024d18248235a33e2e292964b79f9b0`.

---

## 3. Independent Audit of Phase 8.2.1 Security Invariants (at `d01f84d`)

A forensic verification was performed against commit `d01f84d4ac358d8f6c9ec396730b5eec0d877929` (`fix(security): harden pre-commit bypass authorization and contract state`):

1. **Pre-Commit Contract-State Handling:**
   - In `src/hook.rs` (`run_hook`): When staged changes are compliant with an active contract, the hook reads the true persisted status from storage (`contract.status`) and emits:
     ```text
     ✓ Engram Pre-Commit Guard: Staged changes authorized; local postflight checks passed (0 model tokens consumed).
       Active Contract: <contract_id> (Status: <actual_status>; Verification: Passed)
     ```
   - It does not fabricate or print an unearned state.
   - When recovery session resolution fails (`crate::recovery::resolve_session`), errors fail closed with exit code 2.
2. **Trivial Bypass Security Lifecycle:**
   - Schema version: `BYPASS_SCHEMA_VERSION = 1`.
   - TTL: 15-minute expiration (`BYPASS_DEFAULT_TTL_SECS = 900`).
   - Atomic consumption: `validate_and_consume_bypass` uses atomic `fs::rename` from `bypass_trivial.json` to `bypass_trivial.consuming.<uuid>` before unlinking, preventing concurrent use or race conditions.
   - Cryptographic and context binding: verifies `record.repo_root_hash == compute_repo_root_hash(&root)` and `record.baseline_commit == get_head_commit(&root)`.
   - Replay protection: stale or malformed records fail closed.
   - Negative guards: `is_pure_documentation_change` immediately rejects multi-file, security (`is_security_file_path`), manifest, test, or sovereign policy (`is_policy_file`) changes.
3. **Constitution & Policy Enforcement:**
   - In `src/constitution.rs`: Malformed YAML in `load_constitution` returns explicit `Err(...)`.
   - In `src/ci_guard.rs`: Constitution parsing errors trigger `GUARD-CONST-000` (blocking).
   - In `src/ci_guard.rs`: Sovereign policy mutations in PR mode trigger `GUARD-CONST-001` (blocking).
4. **Test Integrity:**
   - `tests/hook_precommit_test.rs` contains 18 dedicated tests exercising real failure paths (expired bypass, wrong target, wrong commit, corrupt recovery session, etc.).

---

## 4. Current State & Handoff Discrepancy Options

Because Rule 8 explicitly forbids history rewriting (`reset`, `clean`, `force-push`, `rebase`), the mismatch between current HEAD `367286f` and expected HEAD `d01f84d` must be reported to the user.

To restore the repository state to the exact tree of `d01f84d4ac358d8f6c9ec396730b5eec0d877929` without rewriting history or force-pushing, the standard Git approach is:
- **`git revert --no-edit 367286f94d10b819986f50780226d014c5d7f9fe`**
- This creates a clean forward revert commit that completely removes the out-of-scope AMstraLift and Graphify files and restores the working tree byte-for-byte to `d01f84d4ac358d8f6c9ec396730b5eec0d877929`.
- Alternatively, if the user chooses to rewrite local and remote history back to `d01f84d4ac358d8f6c9ec396730b5eec0d877929`, that requires explicit user instruction.
