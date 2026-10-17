# Engram v1.5.0 Release Packaging Checklist & Gate Procedures

**Document Identifier:** `ENGRAM-V150-PACKAGING-CHECKLIST-20261017`
**Target Path:** `docs/v1_5_0_release_packaging_checklist.md`
**Target Engine Release:** `v1.5.0`
**Target Protocol Specification:** Protocol `1.1` (`PROTOCOL_VERSION = "1.1"`)
**Execution Posture:** PENDING RELEASE COMMIT & TAG AUTHORIZATION

---

## 1. Release Invariants & Safety Rules

1. **Independent Gates:** No release step may proceed until its predecessor gate is fully verified and documented.
2. **No Unilateral Waivers:** Failing tests, stale assertions, or unverified platform claims must not be bypassed or waived without explicit human authorization.
3. **Strict B6 Exclusion:** Untracked empirical research artifacts (`src/empirical*`, `tests/empirical*`, `tests/fixtures/empirical/`) must remain permanently excluded from all release manifests and commits.
4. **Explicit Staging Only:** Never execute `git add .` or `git add -A`. Every staged file must be explicitly enumerated.
5. **No Version Downgrade:** Engine package version is locked at `1.5.0` (delivering Protocol `1.1`). Historical tags (`v1.1.0` through `v1.4.0`) must remain immutable.

---

## 2. Nine Sequential Release Gates

### Gate 1: Test Assertion Maintenance (Completed & Verified)
*Status: PASS (Verified)*
- [x] Update stale MCP tool count assertions (`14 -> 15`) in:
  - `tests/mcp_test.rs:79`
  - `tests/mcp_baseline_test.rs:219`
- [x] Execute test verification:
  ```powershell
  cargo test --test mcp_test
  cargo test --test mcp_baseline_test
  ```
- [x] Both tests exit with code `0` and 100% pass rate (1/1 passed in `mcp_test`, 27/27 passed in `mcp_baseline_test`).

---

### Gate 2: Full Product Regression Matrix Re-Verification (Completed & Verified)
*Status: PASS (Verified)*
- [x] Execute all 18 targeted product suites:
  ```powershell
  cargo test java_parser::tests
  cargo test --test north_star_1_1_test
  cargo test --test phase1_fixtures_bench_test
  cargo test --test recovery_test
  cargo test --test telemetry_test
  cargo test --test incremental_test
  cargo test --test change_guard_test
  cargo test --test contract_guard_test
  cargo test --test drift_intelligence_test
  cargo test --test drift_test
  cargo test --test git_hardening_test
  cargo test --test flywheel_e2e_test
  cargo test --test go_dependency_test
  cargo test --test go_extraction_test
  cargo test --test memory_inspect_test
  cargo test --test memory_spec_test
  cargo test --test phase2_hardening_test
  cargo test --test setup_integration_test
  ```
- [x] Confirmed 100% pass across all 18 suites (178/178 tests passed, 0 failed).

---

### Gate 3: Version and Manifest Consistency (Verified)
*Status: PASS (Verified)*
- [x] Confirm `Cargo.toml` engine package version is `1.5.0`.
- [x] Confirm `Cargo.lock` dependency package version is `1.5.0`.
- [x] Confirm `extensions/vscode/package.json` extension version is `1.5.0`.
- [x] Confirm CLI binary `--version` outputs `engram 1.5.0`.
- [x] Confirm wire communication protocol is `pub const PROTOCOL_VERSION: &str = "1.1"` across all 8 modules.

---

### Gate 4: Working-Tree Audit & B6 Exclusion Verification (Verified)
*Status: PASS (Verified)*
- [x] Verify that untracked B6 empirical files are strictly untracked:
  ```powershell
  git status --short
  ```
- [x] Confirm that `src/empirical*`, `tests/empirical*`, and `tests/fixtures/empirical/` are NOT in the staging area.

---

### Gate 5: Release Binary Compilation (Local Build Verification)
*Status: PENDING RELEASE COMMIT*
- [ ] Build optimized release binary locally:
  ```powershell
  cargo build --release --bin engram
  ```
- [ ] Verify exit code is `0` and executable is generated at `target/release/engram.exe`.

---

### Gate 6: Binary Smoke Test & Cryptographic Checksums
*Status: PENDING GATE 5*
- [ ] Execute binary smoke test:
  ```powershell
  .\target\release\engram.exe --version
  ```
- [ ] Generate SHA-256 checksums from the final built binary:
  ```powershell
  Get-FileHash .\target\release\engram.exe -Algorithm SHA256 > engram-v1.5.0-windows-x64.sha256
  ```

---

### Gate 7: Explicit File Staging & Release Commit Authorization
*Status: STAGING COMPLETE (Verified Preview — Awaiting Separate Commit Authorization)*
- [x] Stage only explicitly authorized release files:
  ```powershell
  git add tests/mcp_test.rs
  git apply --cached mcp_baseline_assert.patch  # isolates tool count assertion in tests/mcp_baseline_test.rs
  git add docs/v1_5_0_release_notes.md
  git add docs/v1_5_0_walkthrough.md
  git add docs/v1_5_0_release_packaging_checklist.md
  git add docs/v1_1_0_milestone_6_verification_report.md
  git add docs/v1_1_0_milestone_7_release_gate_report.md
  ```
- [x] Verify staged diff via `git diff --cached --stat` (7 files, 862 insertions, 3 deletions) and `git diff --cached --check` (Exit Code 0).
- [ ] Request human approval for release commit message:
  `chore(release): prepare Engram v1.5.0 (Protocol 1.1) release`
- [ ] Execute release commit only upon explicit human approval.

---

### Gate 8: Annotated Release Tag Authorization
*Status: PENDING GATE 7 & SEPARATE AUTHORIZATION*
- [ ] Request human approval to create annotated Git tag:
  ```powershell
  git tag -a v1.5.0 -m "Engram v1.5.0 — Protocol 1.1 & Complete Engineering Intelligence"
  ```
- [ ] Verify tag points to the exact release commit SHA.

---

### Gate 9: Push & Remote Publication Authorization
*Status: PENDING GATE 8 & SEPARATE AUTHORIZATION*
- [ ] Request human approval to push release commit and tag to remote repository:
  ```powershell
  git push origin Development
  git push origin v1.5.0
  ```
- [ ] Attach binary zip, SHA-256 checksums, and reviewed release notes to GitHub release.
