# Phase 5.4.29: Rust Toolchain Determinism and Offline Resolution Audit

**Phase:** 5.4.29  
**Component:** `rust-toolchain.toml` & Toolchain Resolution  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED`  

---

## 1. Context and Problem Statement

Phase 5.4.28 added [`rust-toolchain.toml`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/rust-toolchain.toml) to pin the compiler channel:
```toml
[toolchain]
channel = "stable"
profile = "minimal"
targets = ["x86_64-pc-windows-msvc"]
```

However, a critical finding from Phase 5.4.27 and Phase 5.4.29 is that `channel = "stable"` is a **moving channel**, not an exact, immutable compiler version pin. If a new stable Rust version is released in the future, a build environment with network access would pull the newer compiler, potentially introducing behavioral shifts.

This audit evaluates whether exact semantic version pinning (e.g. `channel = "1.99.0"`) can be achieved offline on the current host.

---

## 2. Seven-Point Toolchain Investigation

### 2.1 Recorded Environment Metrics

1. **`rustup toolchain list`:**
   ```
   stable-x86_64-pc-windows-msvc (active, default)
   ```
2. **`rustc --version --verbose`:**
   ```
   rustc 1.99.0 (b940084d7 2026-09-28)
   binary: rustc
   commit-hash: b940084d7eb6a299eb4bfeb8e34901bc051e7ac4
   commit-date: 2026-09-28
   host: x86_64-pc-windows-msvc
   release: 1.99.0
   LLVM version: 23.1.1
   ```
3. **`cargo --version`:**
   ```
   cargo 1.99.0 (5f94df478 2026-08-27)
   ```
4. **Installed Target Triples (`rustup target list --installed`):**
   ```
   x86_64-pc-windows-msvc
   ```
5. **Exact Locally Available Toolchain Identifier:**
   The physical toolchain directory on disk is:
   `C:\Users\Amal P S\.rustup\toolchains\stable-x86_64-pc-windows-msvc`

---

### 2.2 Empirical Test: Exact Version Pinning Offline (Task-59848)

To determine whether `rust-toolchain.toml` could specify `channel = "1.99.0"` without network access, a test manifest was authored in an isolated temporary directory:
```toml
[toolchain]
channel = "1.99.0"
```
When `rustup show` was invoked in this directory, rustup produced:
```
Default host: x86_64-pc-windows-msvc
rustup home:  C:\Users\Amal P S\.rustup

info: syncing channel updates for 1.99.0-x86_64-pc-windows-msvc
info: latest update on 2026-10-01 for version 1.99.0 (b940084d7 2026-09-28)
info: downloading 6 components
```

**Forensic Finding:** Rustup does not alias explicit version strings (`"1.99.0"`) to installed channel names (`"stable-x86_64-pc-windows-msvc"`), even when the underlying compiler release is identical. Instead, it attempts to download a second toolchain named `1.99.0-x86_64-pc-windows-msvc` from `static.rust-lang.org`.

In compliance with the mandatory constraint **"No external network access / No rustup installation"**, this command was immediately terminated.

---

### 2.3 Offline Resolution Verification

In contrast, with `channel = "stable"`:
- Rustup resolves immediately to `stable-x86_64-pc-windows-msvc`.
- `cargo check --offline` completes in 0.28s.
- Zero network requests are initiated.
- Opening the repository does not trigger rustup updates or component downloads.

---

## 3. Architectural Distinction: Channel vs Exact Pin vs Target

| Configuration Level | Current Manifest Setting | Behavior on Host Machine | Behavior on Clean CI / Remote Host | Reproducibility Scope |
|---|---|---|---|---|
| **Moving Channel** | `channel = "stable"` | Resolves to local `rustc 1.99.0` with zero downloads | Fetches latest stable compiler available at CI build time | Unpinned across long time horizons |
| **Exact Version** | `channel = "1.99.0"` | Fails in offline environments (requires 6-component download) | Fetches exact `rustc 1.99.0` deterministically | Fully pinned across all machines with network |
| **Target Selection** | `targets = ["x86_64-pc-windows-msvc"]` | Restricts compilation to native Windows 64-bit target | Enforces target consistency | Consistent architecture |

---

## 4. Conclusion & Disposition

In accordance with Phase 5.4.29 instructions:
> *"If exact pinning cannot be completed offline, retain the existing configuration and classify deterministic toolchain pinning as unresolved."*

**Disposition:**
- `rust-toolchain.toml` is retained with `channel = "stable"` to ensure strictly offline compilation on the current host.
- **Classification:** **`DETERMINISTIC_TOOLCHAIN_PINNING_UNRESOLVED`** for cross-machine / long-term builds; **`OFFLINE_LOCAL_STABLE_VERIFIED`** for the current host environment.
