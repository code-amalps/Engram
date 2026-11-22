# Phase 5.4.29: Release Workflow and Artifact Provenance Audit

**Phase:** 5.4.29  
**Component:** `.github/workflows/release.yml`, `install.ps1`, `install.sh`  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Authorization Status:** `NOT_AUTHORIZED` (Audit only, zero workflow executions)  

---

## 1. Executive Summary

Phase 5.4.29 conducted a comprehensive security and integrity audit of Engram's release packaging workflow ([`.github/workflows/release.yml`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/.github/workflows/release.yml)) and distribution installers.

The audit verified that release artifacts are compiled directly from source and packaged cleanly without candidate binary contamination. However, the audit identified two workflow-level hardening opportunities for future release phases: adding `--locked` to CI compilation commands and binding CI toolchain setup to `rust-toolchain.toml`.

---

## 2. Eight-Point Release Workflow Forensic Audit

| Audit Dimension | Requirement | Actual Workflow Implementation | Status / Finding |
|---|---|---|---|
| **1. Source Compilation** | Must compile fresh binaries from source code | Line 69: `cargo build --release --target ${{ matrix.target }}` | **VERIFIED** |
| **2. Commit Checkout** | Must check out exact release commit/tag | Line 55: `uses: actions/checkout@v4` on tagged Git ref | **VERIFIED** |
| **3. Toolchain Pinning in CI** | Toolchain must be pinned deterministically | Line 58: `uses: dtolnay/rust-toolchain@stable` | **MOVING CHANNEL** (Uses `@stable` rather than exact version) |
| **4. Dependency Locking in CI** | Dependencies must be locked to `Cargo.lock` | Line 69 omits `--locked` | **RISK** (Should use `cargo build --release --locked`) |
| **5. Build Provenance Exposure** | Executable must expose commit SHA and compiler | Binary contains `build.rs` compile-time metadata exported in `engram --version` | **VERIFIED** |
| **6. Candidate Binary Isolation** | Release package must not include candidate fixtures | Lines 133 & 151 copy strictly from `target/.../release/`, ignoring `benchmarks/` | **VERIFIED (Zero Contamination)** |
| **7. Checksum Publication** | Release must generate and publish `SHA256SUMS` | Lines 138-139, 157-161, and lines 187-195 compute and verify `SHA256SUMS` | **VERIFIED** |
| **8. Multi-Platform Coverage** | Platform coverage claims must be verified | 4 platforms configured in matrix (see Section 3 below) | **AUDITED & CLASSIFIED** |

---

## 3. Platform Matrix Coverage Classification

In accordance with Phase 5.4.29 requirements, all four build targets in the CI release matrix are classified based on empirical evidence:

| Target Triple | Operating System | Binary Format | Verification Status | Evidence Base |
|---|---|---|---|---|
| `x86_64-pc-windows-msvc` | Windows 64-bit | Native PE/COFF (`engram.exe`) | **VERIFIED BY LOCAL EVIDENCE** | Tested in Phase 5.4.28 & 5.4.29 (108/108 PASS) |
| `x86_64-unknown-linux-musl` | Linux 64-bit | Statically Linked ELF (`engram`) | **CONFIGURATION PRESENT (RUNTIME UNVERIFIED)** | Defined in `release.yml`; unverified locally on Windows host |
| `x86_64-apple-darwin` | macOS Intel 64-bit | Mach-O 64-bit (`engram`) | **CONFIGURATION PRESENT (RUNTIME UNVERIFIED)** | Defined in `release.yml`; unverified locally on Windows host |
| `aarch64-apple-darwin` | macOS Apple Silicon | Mach-O ARM64 (`engram`) | **CONFIGURATION PRESENT (RUNTIME UNVERIFIED)** | Defined in `release.yml`; unverified locally on Windows host |
| `aarch64-unknown-linux-musl` | Linux ARM64 | — | **UNSUPPORTED** | Explicitly declared unsupported in release manifest |
| `aarch64-pc-windows-msvc` | Windows ARM64 | — | **UNSUPPORTED** | Explicitly declared unsupported in release manifest |

---

## 4. Installer Integrity and Safety Verification

The distribution installers ([`install.ps1`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/install.ps1) and [`install.sh`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/install.sh)) were tested in Phase 5.4.29 using `tests/installer_safety_test.ps1`:
- **Fail-Closed Checksum Verification:** Correctly rejects tampered or mismatched archives.
- **Missing Manifest Protection:** Fails closed if `SHA256SUMS` cannot be fetched.
- **Process In-Use Guard:** Prevents replacing the binary while an existing `engram` process is running.
- **Atomic Staged Replacement:** Extracts to temporary directory before atomic swap.
- **Token Sanitization:** Authentication tokens for private repository downloads are strictly redacted from logs and error messages.

---

## 5. Hardening Recommendations for Future Authorized Phases

When user authorization is granted for workflow modifications in a future phase:
1. **Add `--locked` Flag:** Update line 69 of `.github/workflows/release.yml` from:
   `run: cargo build --release --target ${{ matrix.target }}`
   to:
   `run: cargo build --release --locked --target ${{ matrix.target }}`
2. **Synchronize CI with `rust-toolchain.toml`:** Update step 58 to respect repository toolchain pinning.
