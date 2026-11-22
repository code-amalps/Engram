# Phase 5.4.28: Binary Build Provenance Implementation Report

**Phase:** 5.4.28  
**Component:** Product Build Infrastructure & CLI Interface  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED`  

---

## 1. Problem Statement & Baseline Defect

In Phase 5.4.27, an audit of binary release readiness discovered that the Engram executable did not embed build provenance:
```cmd
$ engram.exe --version
engram 1.5.4
```

This minimal output failed to satisfy release transparency and auditability standards for public releases:
1. **Zero Git Commit Traceability:** The user or auditor could not determine which Git SHA was compiled into the binary.
2. **Zero Dirty-State Indication:** If the binary was built from an uncommitted working tree, this fact was completely hidden.
3. **Zero Compiler Provenance:** The exact Rust compiler version and LLVM backend used to generate machine code were unrecorded.
4. **Zero Profile Visibility:** The binary did not indicate whether it was compiled under `debug` or `release` profile optimizations.

---

## 2. Zero-Dependency Build Script Implementation

To maintain Engram's strict offline, minimal-dependency build posture, external heavy crates such as `vergen`, `shadow-rs`, or `git2` were intentionally rejected. Instead, a lightweight, standard-library-only `build.rs` was implemented.

### 2.1 File: `build.rs`

```rust
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    let git_commit = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { String::from_utf8(o.stdout).ok() } else { None })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let git_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { String::from_utf8(o.stdout).ok() } else { None })
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let rustc_version = Command::new("rustc")
        .args(["--version"])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { String::from_utf8(o.stdout).ok() } else { None })
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());

    println!("cargo:rustc-env=ENGRAM_BUILD_GIT_COMMIT={}", git_commit);
    println!("cargo:rustc-env=ENGRAM_BUILD_GIT_DIRTY={}", if git_dirty { "true" } else { "false" });
    println!("cargo:rustc-env=ENGRAM_BUILD_RUSTC={}", rustc_version);
    println!("cargo:rustc-env=ENGRAM_BUILD_PROFILE={}", profile);
}
```

### 2.2 Cargo Manifest Configuration (`Cargo.toml`)

Under `[package]`, the build script entry was explicitly specified:
```toml
[package]
name = "engram"
version = "1.5.4"
edition = "2021"
build = "build.rs"
```

### 2.3 Rust Module Integration (`src/lib.rs` & `src/main.rs`)

In `src/lib.rs`, the compile-time environment variables are exported via a public `build_info` module:
```rust
pub mod build_info {
    pub const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
    pub const GIT_COMMIT: &str = env!("ENGRAM_BUILD_GIT_COMMIT");
    pub const GIT_DIRTY: &str = env!("ENGRAM_BUILD_GIT_DIRTY");
    pub const RUSTC_VERSION: &str = env!("ENGRAM_BUILD_RUSTC");
    pub const BUILD_PROFILE: &str = env!("ENGRAM_BUILD_PROFILE");

    pub const VERSION_FULL: &str = concat!(
        env!("CARGO_PKG_VERSION"), "\n",
        "commit: ", env!("ENGRAM_BUILD_GIT_COMMIT"), "\n",
        "dirty: ", env!("ENGRAM_BUILD_GIT_DIRTY"), "\n",
        "rustc: ", env!("ENGRAM_BUILD_RUSTC"), "\n",
        "profile: ", env!("ENGRAM_BUILD_PROFILE")
    );
}
```

In `src/main.rs`, Clap's `version` attribute is bound to `crate::build_info::VERSION_FULL`:
```rust
#[derive(Parser, Debug)]
#[command(
    name = "engram",
    about = "Engram: Context Engine & MCP Server",
    version = crate::build_info::VERSION_FULL
)]
struct Cli { ... }
```

---

## 3. Engineering Challenges & Solutions

### 3.1 Cargo `rustc-env` Multi-Line Incompatibility
- **Challenge:** Initial attempts to format `ENGRAM_BUILD_VERSION_FULL` directly inside `build.rs` using multi-line strings failed because Cargo parses `cargo:rustc-env=VAR=VALUE` line-by-line. A newline character causes Cargo to treat subsequent lines as separate commands, discarding them.
- **Solution:** `build.rs` emits only single-line, individual variables. `VERSION_FULL` is composed at compile time in `src/lib.rs` using the `concat!` macro, ensuring 100% clean formatting and zero newline clipping.

### 3.2 Offline Source Tarball Graceful Degradation
- **Challenge:** If a user builds from an unpacked release source tarball where `.git` is absent, `git` commands fail with non-zero exit codes.
- **Solution:** `build.rs` wraps all process invocations in `.ok().and_then(...)` and falls back to `"unknown"` and `dirty: false`. The build succeeds without error.

---

## 4. Verification Evidence

### 4.1 CLI Execution Output
```cmd
$ cargo run --bin engram -- --version
engram 1.5.4
commit: 7106a2f369fe79faf5f95449378a900ab8e8eddf
dirty: true
rustc: rustc 1.99.0 (b940084d7 2026-09-28)
profile: debug
```

### 4.2 Automated Integration Test Suite (`tests/build_provenance_test.rs`)
An end-to-end integration test suite was created and executed:
```
running 5 tests
test test_version_full_contains_pkg_version ... ok
test test_git_commit_metadata_present ... ok
test test_git_dirty_flag_boolean ... ok
test test_rustc_version_metadata_present ... ok
test test_build_profile_metadata_present ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

All 5 tests pass cleanly and deterministically.
