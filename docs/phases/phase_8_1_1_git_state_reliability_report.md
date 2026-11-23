# Engram Phase 8.1.1: Git State Detection Reliability Report

**Execution Mode:** Forensic Remediation & Deterministic Reliability Verification  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Milestone:** Phase 8.1.1  
**Status:** `VERIFIED_NON_FAIL_OPEN`  
**Date:** 23 November 2026  

---

## 1. Executive Summary & Defect Identification

During the Phase 8.1 independent audit, a critical fail-open vulnerability was identified in `src/engage.rs`:

```rust
// HISTORICAL PHASE 8.1 IMPLEMENTATION (src/engage.rs)
fn is_git_clean(root: &Path) -> bool {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(root)
        .output();
    if let Ok(out) = output {
        if out.status.success() {
            return String::from_utf8_lossy(&out.stdout).trim().is_empty();
        }
    }
    true // <-- FAIL-OPEN DEFECT: Collapses all failures into CLEAN
}
```

### 1.1 Impact of the Defect
If Git failed to spawn (missing executable, PATH corruption, sandbox restriction), exited with non-zero status (non-git directory, repository corruption, index lock error), or timed out, the function returned `true`. This caused `engram_engage` to falsely report `"is_clean": true` to agents, misrepresenting an unverified or corrupted workspace as pristine.

---

## 2. Remediation Architecture

The fail-open helper was replaced with a robust, typed subsystem implemented in `src/git.rs` and consumed by `src/engage.rs`.

### 2.1 Typed Error & State Model (`src/git.rs`)
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitStateError {
    SpawnFailed(String),
    CommandFailed(Option<i32>, String),
    InvalidUtf8,
    Timeout(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GitWorkingTreeState {
    Clean,
    Dirty,
    Unknown,
}
```

### 2.2 Bounded Timeout & Non-Fail-Open Logic
`detect_git_state_with_timeout` executes `git status --porcelain=v1 -uall` with a strict execution deadline (default 5 seconds). The process stdout/stderr are gathered concurrently. If the deadline expires, the child process is terminated and an explicit `GitStateError::Timeout` is returned.

Relevant changes are filtered using `is_internal_metadata_path` so internal temporary files (e.g. `.engram/sessions`, `.db-wal`) do not dirty the workspace, while all real tracked modifications and untracked files are detected.

### 2.3 Response Serialization Guarantees
In `execute_engage`, the result is mapped deterministically:
```rust
let git_state_res = crate::git::detect_git_state(&root);
let (is_clean_val, git_state_str, git_state_err_val) = match git_state_res {
    Ok(true) => (json!(true), "CLEAN", Value::Null),
    Ok(false) => (json!(false), "DIRTY", Value::Null),
    Err(ref e) => (Value::Null, "UNKNOWN", json!(e.to_string())),
};
```

In the public JSON response:
- **`CLEAN`**: `is_clean: true`, `git_state: "CLEAN"`, `git_state_error: null`
- **`DIRTY`**: `is_clean: false`, `git_state: "DIRTY"`, `git_state_error: null`
- **`UNKNOWN`**: `is_clean: null`, `git_state: "UNKNOWN"`, `git_state_error: "<detailed error>"`

**Invariant Guaranteed:** Under zero conditions can an inspection error, timeout, or execution failure produce `"is_clean": true` or `"git_state": "CLEAN"`.

---

## 3. Test Suite Verification Matrix (`tests/git_state_reliability_test.rs`)

A dedicated integration test suite verifies all 7 required failure modes plus endpoint integration:

| Test ID | Test Function | Scenario Description | Expected Outcome | Actual Result |
|---|---|---|:---:|:---:|
| **T-01** | `test_git_state_clean_repository` | Fresh git repo with initial commit, zero modifications | `Ok(true)` / `CLEAN` | **PASS** |
| **T-02** | `test_git_state_tracked_modifications_dirty` | Tracked `README.md` modified | `Ok(false)` / `DIRTY` | **PASS** |
| **T-03** | `test_git_state_untracked_files_dirty` | New untracked file added | `Ok(false)` / `DIRTY` | **PASS** |
| **T-04** | `test_git_state_executable_unavailable_fails_closed`| Nonexistent binary path | `Err(SpawnFailed)` | **PASS** |
| **T-05** | `test_git_state_nonzero_exit_status_fails_closed` | Directory without `.git` (exit 128) | `Err(CommandFailed(128))` | **PASS** |
| **T-06** | `test_git_state_invalid_utf8_output_fails_closed` | Command output emitting non-UTF8 bytes | `Err(InvalidUtf8)` | **PASS** |
| **T-07** | `test_git_state_command_timeout_fails_closed` | Sleeping command with 100ms timeout | `Err(Timeout(100))` | **PASS** |
| **T-08** | `test_engage_endpoint_cannot_fail_open_on_broken_git`| `engram_engage` invocation on non-git dir | `git_state: "UNKNOWN"`, `is_clean: null` | **PASS** |

Execution Output:
```
running 8 tests
test test_git_state_command_timeout_fails_closed ... ok
test test_git_state_nonzero_exit_status_fails_closed ... ok
test test_git_state_invalid_utf8_output_fails_closed ... ok
test test_engage_endpoint_cannot_fail_open_on_broken_git ... ok
test test_git_state_executable_unavailable_fails_closed ... ok
test test_git_state_clean_repository ... ok
test test_git_state_untracked_files_dirty ... ok
test test_git_state_tracked_modifications_dirty ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.43s
```

---

## 4. Workstream A Conclusion

Workstream A is **100% complete and certified**. The fail-open vulnerability is eliminated, typed models are in place, and deterministic test coverage guarantees fail-closed behavior.
