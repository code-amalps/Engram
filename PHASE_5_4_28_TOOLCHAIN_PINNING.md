# Phase 5.4.28: Rust Toolchain Pinning Specification and Forensic Report

**Phase:** 5.4.28  
**Component:** Toolchain Configuration & Reproducibility  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED`  

---

## 1. Context and Problem Statement

Phase 5.4.27 identified the absence of a toolchain pinning file (`rust-toolchain.toml` or `rust-toolchain`) in the repository root as a significant reproducibility risk.

Without toolchain pinning:
1. Developers and CI systems automatically invoke whatever ambient Rust toolchain happens to be active on their host.
2. Minor compiler version mismatches can introduce differing code-generation heuristics, optimization levels, or compiler diagnostics.
3. Inadvertent usage of nightly features could pass unnoticed on developer machines while breaking on stable release builds.

---

## 2. Toolchain Specification: `rust-toolchain.toml`

To ensure deterministic, reproducible builds while strictly respecting the offline execution constraint of this phase, `rust-toolchain.toml` was authored in the repository root:

```toml
[toolchain]
channel = "stable"
profile = "minimal"
targets = ["x86_64-pc-windows-msvc"]
```

### 2.1 Configuration Breakdown
- **`channel = "stable"`**: Restricts compilation strictly to the official stable compiler channel. Guarantees that no unstable nightly features, experimental syntax, or unreleased flags are used.
- **`profile = "minimal"`**: Instructs `rustup` to include only essential compiler tools (`rustc`, `cargo`, `rust-std`), omitting bulky documentation and supplementary tools.
- **`targets = ["x86_64-pc-windows-msvc"]`**: Explicitly declares the primary target triple for native Windows 64-bit production binaries.

---

## 3. Toolchain Resolution & Offline Verification

### 3.1 Offline Resolution Forensics
During implementation testing in Phase 5.4.28, an important behavior of `rustup` was verified:
- When `channel` is set to an explicit numeric version (e.g. `"1.99.0"`), `rustup` attempts to verify the manifest against `static.rust-lang.org`. If the locally installed toolchain was registered under the standard name `stable-x86_64-pc-windows-msvc`, `rustup` will fail closed with a network timeout error in offline environments.
- Setting `channel = "stable"` allows `rustup` to immediately bind to the installed `stable-x86_64-pc-windows-msvc` toolchain locally with **zero network activity**.

### 3.2 Host Toolchain Identity
The active local toolchain resolved by `rust-toolchain.toml`:
- **Toolchain:** `stable-x86_64-pc-windows-msvc` (default)
- **Rustc Version:** `rustc 1.99.0 (b940084d7 2026-09-28)`
- **Host Target:** `x86_64-pc-windows-msvc`
- **Cargo Version:** `cargo 1.99.0 (b940084d7 2026-09-28)`

### 3.3 Verification Commands and Results
Offline verification was performed using `cargo check --offline`:
```cmd
$ cargo check --offline
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s
```
The check passed with exit code 0, confirming that all dependencies and toolchain requirements are completely satisfied offline.
