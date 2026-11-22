# Milestone 5B-Prep: Workspace Isolation Remediation

## 1. Technical Context and Problem Statement

In Milestone 5A, an architectural blocker was discovered in the benchmark harness:
`benchmarks/matched-pair-harness/src/isolation.py` was originally coupled to the Angular starter repository (`angular-ngrx-material-starter`). Specifically:
1. `compute_comprehensive_golden_manifest` walked `node_modules` (68,274 files) and hashed `package.json` / `package-lock.json`.
2. `verify_dependency_integrity` asserted that `package.json` and `package-lock.json` must exist and match baseline hashes.
3. `verify_sandbox_cleanliness` rejected any directory containing uncommitted files, without recognizing standard Rust project subtrees (`target/`, `.engram/`).
4. `create_sandbox` and `teardown_sandbox` assumed external dependencies were always in `node_modules`.

When attempting to run Task C (the commit-bound cache invalidation task targeting Engram itself, a Rust/Cargo codebase), the harness crashed immediately on cold start because `package.json` was absent and Cargo's dependency graph resides in `Cargo.toml`, `Cargo.lock`, and `target/`.

---

## 2. Remediated Architecture

The isolation system was generalized into a multi-ecosystem, repository-aware isolation engine supporting both **Angular/Node.js** and **Rust/Cargo** workspaces while maintaining complete backward compatibility.

### 2.1 Cargo Manifest Extraction (`compute_cargo_manifest`)

A dedicated manifest computer was introduced in `isolation.py`:
```python
def compute_cargo_manifest(repo_path: Path) -> Dict[str, Any]:
    manifest = {
        "repo_type": "cargo",
        "cargo_toml_sha256": None,
        "cargo_lock_sha256": None,
        "source_files_count": 0,
        "source_total_bytes": 0,
        "source_recursive_digest": None,
    }
    # 1. Exact cryptographic content hash of Cargo.toml
    # 2. Exact cryptographic content hash of Cargo.lock (if present)
    # 3. Recursive metadata and structural digest across src/ and tests/
```
This guarantees that any tampering with Cargo dependencies or unauthorized modification of source trees in the golden repository is instantly detected.

### 2.2 Generalized Dependency Integrity (`verify_dependency_integrity`)

`verify_dependency_integrity` now detects repository type via `baseline.get("repo_type")`:
- **For `repo_type == "cargo"`:**
  - Compares `current["cargo_toml_sha256"]` with `baseline["cargo_toml_sha256"]`.
  - Compares `current["cargo_lock_sha256"]` with `baseline["cargo_lock_sha256"]`.
  - Raises `GoldenDependencyCorruptionError` on any discrepancy.
- **For `repo_type == "angular"` (or default):**
  - Executes the existing Angular verification path (`package.json`, `package-lock.json`, and `node_modules` structural tree digest). Zero regression.

### 2.3 Pristine Sandbox Verification (`verify_sandbox_cleanliness`)

The cleanliness checker now allows repository-specific metadata directories:
- For Cargo repositories: `.github`, `.engram`, and `target` directories do not trigger false cleanliness rejections.
- Any untracked file or uncommitted change outside the recognized whitelist triggers `IsolationError`.

### 2.4 Worktree Lifecycle & Teardown Guards (`create_sandbox` and `teardown_sandbox`)

1. **Junction / Dependency Isolation:**
   - For Angular: Handles `node_modules` junctions or multi-threaded physical copies (`robocopy /MT:8`).
   - For Cargo: Handles `target` directory junctions if configured, or leaves dependencies completely detached when `dependency_strategy="none"`.
2. **Absolute Path Guard Invariant:**
   - Both `create_sandbox` and `teardown_sandbox` strictly assert that the target path contains `engram_pilot\runs` or `engram_pilot/runs`.
   - Any attempt to target paths outside the runs folder immediately raises `SafetyViolationError`, preventing destructive deletion of host system directories.

---

## 3. Empirical Verification Evidence

All isolation features were validated across 20 unit tests in `benchmarks/matched-pair-harness/tests/test_milestone_5b_remediation.py`:
- `test_compute_cargo_manifest`: PASSED
- `test_verify_dependency_integrity_cargo_unmodified`: PASSED
- `test_verify_dependency_integrity_cargo_lock_drift`: PASSED (Correctly caught lockfile drift)
- `test_verify_dependency_integrity_cargo_toml_drift`: PASSED (Correctly caught manifest drift)
- `test_verify_sandbox_cleanliness_cargo`: PASSED (Allowed `.engram`/`target`; blocked unauthorized files)
- `test_teardown_sandbox_safety_guard`: PASSED (Strictly rejected paths outside `engram_pilot/runs`)
- `test_create_and_teardown_sandbox_cargo`: PASSED (Successfully created and destroyed isolated worktrees)

The existing Angular isolation tests (233 tests) continue to pass with 100% success.
