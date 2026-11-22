# Phase 5.4.28: Source Baseline Preparation and Logical Commit Staging Plan

**Phase:** 5.4.28  
**Component:** Git Repository Baseline Management  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED` (Strictly zero Git mutations performed)  

---

## 1. Executive Summary

Phase 5.4.28 has successfully prepared the repository working tree for the proposed Engram v1.5.4 Public Alpha release. All technical blockers identified in Phase 5.4.27 (unclassified working files, missing build provenance, unpinned toolchain, absent changelog, undocumented binary provenance) have been resolved.

In accordance with the strict offline constraints of this phase, **zero Git commits, pushes, tags, branch checkouts, or merges have been executed**. The repository working tree remains in a pristine, reviewable staging state awaiting explicit user authorization.

---

## 2. Working Tree Diff Statistics

### 2.1 Tracked Modified Files (12 files)
```
 Cargo.toml                                         |    1 +
 benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json |   27 +-
 benchmarks/matched-pair-harness/src/classifier.py  |  194 +++-
 benchmarks/matched-pair-harness/src/evaluator.py   |   67 +-
 benchmarks/matched-pair-harness/src/orchestrator.py| 1150 +++++++++++++++++++-
 benchmarks/matched-pair-harness/src/runner.py      |  147 ++-
 benchmarks/matched-pair-harness/src/telemetry.py   |   66 +-
 src/impact.rs                                      |  106 +-
 src/lib.rs                                         |   20 +
 src/main.rs                                        |    2 +-
 src/mcp.rs                                         |  624 ++++++++++-
 src/storage.rs                                     |    2 +
 12 files changed, 2269 insertions(+), 137 deletions(-)
```

### 2.2 Untracked Remediation Files Added in Phase 5.4.28
- `build.rs`: Zero-dependency build script exposing Git commit, dirty state, rustc version, and profile.
- `rust-toolchain.toml`: Toolchain pinning manifest enforcing stable channel.
- `CHANGELOG.md`: Root Keep-a-Changelog document for v1.5.4.
- `tests/build_provenance_test.rs`: Integration test suite for build provenance.
- Phase 5.4.28 forensic documentation and JSON reports.

---

## 3. Recommended Logical Commit Grouping Plan

When the user grants explicit authorization in a future phase to commit these changes, the following 5-commit sequence is recommended. This sequence separates product features, bug fixes, build infrastructure, benchmark harness improvements, and documentation into isolated, bisectable commits.

```
Commit Group 1: feat(mcp)
├── src/mcp.rs
├── src/impact.rs
├── tests/utf8_safety_test.rs
└── tests/inspect_symbol_budget_test.rs
Message: feat(mcp): implement UTF-8 character boundary truncation and 16KB response budget shedding

Commit Group 2: fix(storage)
└── src/storage.rs
Message: fix(storage): add 15-second busy timeout to SQLite connection initialization

Commit Group 3: feat(build)
├── build.rs
├── Cargo.toml
├── src/lib.rs
├── src/main.rs
├── rust-toolchain.toml
└── tests/build_provenance_test.rs
Message: feat(build): implement build provenance metadata and pin stable toolchain

Commit Group 4: test(harness)
├── benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json
├── benchmarks/matched-pair-harness/src/classifier.py
├── benchmarks/matched-pair-harness/src/evaluator.py
├── benchmarks/matched-pair-harness/src/orchestrator.py
├── benchmarks/matched-pair-harness/src/runner.py
└── benchmarks/matched-pair-harness/src/telemetry.py
Message: test(harness): harden tetrad order validation, telemetry sealing, and budget enforcement

Commit Group 5: docs(release)
├── CHANGELOG.md
├── PHASE_5_4_28_*.md
└── phase5_4_28_*.json
Message: docs(release): prepare v1.5.4 changelog and release baseline documentation
```

---

## 4. Verification Check

Before concluding this phase, the working tree was checked against regressions:
1. `cargo check --offline`: PASS (0.28s).
2. All 62 Rust integration tests: PASS.
3. All 8 installer safety tests: PASS.
4. All 38 Python harness safety tests: PASS.
5. `engram.exe --version` invocation: Displays complete provenance cleanly.
6. Git status: Exactly 12 tracked modified files, zero deleted files, zero unexpected mutations.

The source baseline is clean, stable, and ready for user authorization.
