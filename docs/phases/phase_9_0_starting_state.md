# Engram Phase 9.0: Starting State Baseline & Verification

**Phase:** 9.0  
**Phase Type:** Integration Implementation / Coding Completion  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Repository:** `code-amalps/Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Expected Starting HEAD:** `d01f84d4ac358d8f6c9ec396730b5eec0d877929`  
**Verified Starting HEAD:** `d01f84d4ac358d8f6c9ec396730b5eec0d877929`  
**Parent Commit (Phase 8.2):** `aaad1d5ff1a479dcb3e5563f1dddffc7ef117a97`  
**Governing Baseline Candidate:** `v1.5.5-cand1` (source commit: `ded9ca521024d18248235a33e2e292964b79f9b0`)  
**Published Release:** `v1.5.4-cand5` at commit `2df1cac683435314ffd7ad52369e6d20198fae0d`  
**Date of Verification:** 2026-11-23  
**Working Tree Status:** Clean (`nothing to commit, working tree clean`)

---

## 1. Commit Ancestry & Verification

Git history verification confirms exact linear progression:
- `d01f84d`: `fix(security): harden pre-commit bypass authorization and contract state` (Phase 8.2.1)
- `aaad1d5`: `feat(phase-8.2): complete remaining engineering, postflight hook, trivial bypass & core tools exposure` (Phase 8.2)
- `8a7f141`: `docs(phase8.1.1.4): record final release integrity gate, audit report, and go/no-go recommendation` (Phase 8.1.1.4)
- `3d37f5b`: `docs(release): update candidate manifest and evidence closure with CI run 45161715786 results`
- `c2552a0`: `docs(release): record v1.5.5-cand1 candidate manifest and verification report`
- `ded9ca5`: `v1.5.5-cand1` candidate source commit

Remote tracking branch `origin/feature/phase2-stage5-remediation` is fully synchronized with local HEAD `d01f84d`.

---

## 2. Phase 8.2.1 Remediation Verification Audit

A forensic verification was performed against the Phase 8.2.1 source implementation to ensure all security properties are intact prior to integration development:

1. **Bypass Record Schema & Lifecycle:**
   - In `src/storage.rs`: `BYPASS_SCHEMA_VERSION = 1`, `BYPASS_DEFAULT_TTL_SECS = 900`.
   - `BypassLifecycleState` enum contains `Created`, `Validated`, `Consumed`.
   - `BypassTrivialRecord` includes `schema_version`, `record_id`, `task`, `task_hash`, `reason`, `target_files`, `baseline_commit`, `repo_root_hash`, `created_at`, `expires_at`, and `state`.
2. **Atomic One-Time Consumption:**
   - In `src/storage.rs`: `validate_and_consume_bypass` uses atomic rename (`fs::rename`) from `bypass_trivial.json` to `bypass_trivial.consuming.<uuid>` before cleaning up, preventing race conditions and ensuring single-use authorization.
3. **Repository & Baseline Binding:**
   - `validate_bypass_record` verifies `record.repo_root_hash == compute_repo_root_hash(&root)` and `record.baseline_commit == get_head_commit(&root)`.
4. **Expiration & Target Guards:**
   - Rejects records where `now > record.expires_at`.
   - Rejects records where changed files count != 1, or target file does not match recorded target, or target is a security, manifest, test, or sovereign policy file.
   - `is_pure_documentation_change` enforces immediate negative checks against security, manifest, test, and policy files.
5. **Contract Status & Recovery Error Propagation:**
   - In `src/hook.rs`: Pre-commit hook displays genuine status from storage (`Active Contract: <cid> (Status: <actual_status>; Verification: Passed)`).
   - In `src/hook.rs`: `resolve_session` errors fail closed with exit code 2.
6. **Constitution Invariants:**
   - In `src/constitution.rs`: Malformed YAML in `load_constitution` and `load_constitution_at_revision` returns explicit `Err(...)`.
   - In `src/ci_guard.rs`: Constitution loading error emits `GUARD-CONST-000` (Block).
   - In `src/ci_guard.rs`: Sovereign policy file mutations in PR mode unconditionally emit `GUARD-CONST-001` (Block).

All Phase 8.2.1 security invariants are verified and operational.

---

## 3. Operational Invariants for Phase 9.0

1. **No CI Triggering or Polling:** Zero GitHub Actions runs will be initiated, watched, or awaited.
2. **No Full Regression Execution:** `cargo test --all-targets` and benchmarks remain reserved for November 26 Full Certification Day.
3. **No Release Operations:** No tags created or moved; no assets published.
4. **Scope Control:** Integration work is strictly bounded to AMstraLift and Graphify interfaces, schemas, adapters, and Engram consumption boundaries.
