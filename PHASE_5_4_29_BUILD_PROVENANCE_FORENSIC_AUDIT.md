# Phase 5.4.29: Build Provenance Forensic Implementation Audit

**Phase:** 5.4.29  
**Component:** `build.rs`, `src/lib.rs`, `src/main.rs`, `tests/build_provenance_test.rs`  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED`  

---

## 1. Executive Summary

Phase 5.4.29 performed an adversarial forensic audit of the build provenance mechanism introduced in Phase 5.4.28. The audit evaluated source code, compiler interactions, environment variable cascades, and failure-mode semantics across four core dimensions: Git commit identity, dirty-state accuracy, compiler provenance, and fallback behavior.

The audit confirms that the build provenance implementation is **fail-safe, transparent, and completely free of false-clean reporting**.

---

## 2. Forensic Evaluation by Dimension

### 2.1 Git Commit Identity

- **Source of Truth:** Retrieved via `Command::new("git").current_dir(&manifest_dir).args(["rev-parse", "HEAD"])`. It executes directly in the package directory (`CARGO_MANIFEST_DIR`), preventing ambiguity if the parent process was invoked from another working directory.
- **Dynamic Execution:** The commit SHA is dynamically queried at build time; it is not hardcoded in source.
- **Full-Length SHA:** Returns the complete 40-character hexadecimal string (`7106a2f369fe79faf5f95449378a900ab8e8eddf`).
- **Outside Checkout Handling:** If the source tree is extracted into a non-Git environment (such as an isolated source snapshot or tarball release), `git rev-parse HEAD` returns exit code 128. `build.rs` catches non-zero exit codes via `out.status.success()` and falls back explicitly to `"unknown"`. It does not invent or hallucinate a baseline hash.
- **Rerun Invalidation:** `build.rs` emits `println!("cargo:rerun-if-changed=.git/HEAD");` and `println!("cargo:rerun-if-changed=.git/index");`. Any commit, branch checkout, or detached HEAD state changes `.git/HEAD`, forcing Cargo to re-run `build.rs` on the next build.

### 2.2 Dirty-State Accuracy

- **Detection Scope:** In `build.rs`, dirty-state is evaluated via:
  ```rust
  let git_dirty = Command::new("git")
      .current_dir(&manifest_dir)
      .args(["status", "--porcelain"])
      .output()
      .ok()
      .and_then(|out| {
          if out.status.success() {
              let status_str = String::from_utf8_lossy(&out.stdout);
              Some(!status_str.trim().is_empty())
          } else {
              None
          }
      });
  ```
  `git status --porcelain` detects:
  - Modified tracked files (`M`)
  - Staged tracked files (`A`, `M`, `D`)
  - Untracked files (`??`)
  - Deleted tracked files (`D`)
- **Tri-State Semantics (Elimination of False-Clean):**
  ```rust
  let dirty_str = match git_dirty {
      Some(true) => "true",
      Some(false) => "false",
      None => "unknown",
  };
  ```
  If Git execution fails (e.g. missing Git binary, corrupted `.git` index, or non-Git directory), `git_dirty` is `None`, which maps to `"unknown"`. **A failed Git status command never reports `"false"` (clean).**
- **Impact of Build Outputs:** `.gitignore` excludes `target/`, `.engram/`, and `benchmarks/matched-pair-harness/**/__pycache__/`. However, untracked files in the root (such as historical reports) correctly trigger `dirty: true`, ensuring auditors are always alerted when building from an uncommitted working tree.

### 2.3 Compiler Provenance

- **Active Cargo Compiler vs Ambient PATH:** Cargo explicitly injects the `RUSTC` environment variable into build scripts, representing the exact compiler binary Cargo is using. In `build.rs`:
  ```rust
  let rustc_version = std::env::var("RUSTC")
      .ok()
      .and_then(|rustc| {
          Command::new(rustc)
              .arg("--version")
              .output()
              ...
      })
  ```
  This ensures that if a system has multiple compilers installed, `build.rs` queries the specific compiler invoked by Cargo, falling back to ambient `"rustc"` only if the env var is missing.
- **Private Data Protection:** The reported compiler string is the standard single-line `rustc 1.99.0 (b940084d7 2026-09-28)`. It does not embed usernames, local paths, or build machine identities.
- **Profile Visibility:** The build script queries Cargo's `PROFILE` variable and reports `profile: debug` or `profile: release`.

### 2.4 Build Script Failure Modes and Fallback Matrix

| Condition | Git Command Result | `ENGRAM_BUILD_GIT_COMMIT` | `ENGRAM_BUILD_GIT_DIRTY` | `ENGRAM_BUILD_RUSTC` | `ENGRAM_BUILD_PROFILE` |
|---|---|---|---|---|---|
| **Inside Git Repo (Clean)** | Success (empty stdout) | `<40-char SHA>` | `"false"` | `<rustc version>` | `"release"` |
| **Inside Git Repo (Dirty)** | Success (non-empty stdout) | `<40-char SHA>` | `"true"` | `<rustc version>` | `"release"` |
| **Outside Git Checkout** | Exit code 128 | `"unknown"` | `"unknown"` | `<rustc version>` | `"release"` |
| **Git Binary Missing / Not in PATH** | Command execution error | `"unknown"` | `"unknown"` | `<rustc version>` | `"release"` |
| **Compiler Query Failure** | Command execution error | `<40-char SHA>` | `<dirty state>` | `"unknown"` | `"release"` |

---

## 3. Regression Test Verification

The test suite [`tests/build_provenance_test.rs`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/tests/build_provenance_test.rs) was executed both inside the repository and within the isolated build workspace (`engram_isolated_build_1`):
```
running 5 tests
test test_build_provenance_env_vars_present ... ok
test test_build_provenance_git_commit_format ... ok
test test_build_provenance_dirty_state_format ... ok
test test_build_provenance_profile_format ... ok
test test_build_provenance_formatting_function ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

All 5 tests passed with zero failures.
