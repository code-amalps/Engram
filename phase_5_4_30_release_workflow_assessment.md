# Engram Phase 5.4.30: Release Packaging & Automation Workflow Assessment

**Phase:** 5.4.30  
**Audit Scope:** CI/CD Automation (`.github/workflows/release.yml`), Shell Installers (`install.ps1`, `install.sh`), InnoSetup Config (`installer/engram.iss`), and Build Reproducibility  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  

---

## 1. Executive Summary

This assessment evaluated the automation pipelines, packaging scripts, and distribution artifacts intended to deliver Engram v1.5.4 to end users. 

### Key Findings:
1. **Packaging Safety:** Shell installers (`install.ps1` and `install.sh`) implement high-assurance safety patterns: cryptographic SHA-256 validation against release manifests and atomic binary swaps to prevent corrupted partial writes.
2. **Privilege Boundaries:** The Windows InnoSetup installer (`installer/engram.iss`) installs strictly into per-user directory `{localappdata}\Programs\Engram` and requires no administrative privileges (`PrivilegesRequired=lowest`).
3. **CI Pipeline Hardening Gaps (WF-01 & WF-02):** The GitHub Actions workflow `.github/workflows/release.yml` uses an unpinned moving compiler tag (`@stable`) rather than the pinned `1.85.0` specified in `rust-toolchain.toml`, and omits the mandatory `--locked` cargo flag.
4. **Reproducibility Status (WF-03):** Local clean builds from pristine source tree produce identical binary size (`18,803,200` bytes), but binary hashes differ under Windows MSVC due to embedded PE timestamps and PDB GUIDs (`BIT_REPRODUCIBLE_BUILD_NOT_ESTABLISHED`).

---

## 2. In-Depth Workflow & Component Audits

### 2.1 GitHub Actions Release Workflow (`.github/workflows/release.yml`)
- **Trigger:** Tag push matching `v*.*.*`.
- **Target Matrix:**
  - `x86_64-pc-windows-msvc` (Windows Server runner)
  - `x86_64-unknown-linux-gnu` (Ubuntu runner)
  - `x86_64-apple-darwin` (macOS runner)
  - `aarch64-apple-darwin` (macOS Apple Silicon runner)
- **Observations & Gaps:**
  - **Toolchain Specification:** `dtolnay/rust-toolchain@stable` pulls the current floating stable compiler. Although `rust-toolchain.toml` pins `1.85.0`, CI should explicitly declare `toolchain: "1.85.0"` to guarantee deterministic builds.
  - **Cargo Invocation:** The build step runs `cargo build --release --verbose`. It omits `--locked`. Omitting `--locked` permits Cargo to re-resolve dependencies if an updated crate index is fetched, creating supply chain drift.
  - **Artifact Packaging:** Bins are compressed into `.tar.gz` (Unix) and `.zip` (Windows) with accompanying `.sha256` checksum files generated via `shasum -a 256` / `CertUtil`.

### 2.2 PowerShell Installer (`install.ps1`)
- **Target OS:** Windows 10 / 11 (PowerShell 5.1+ / PowerShell Core).
- **Security & Integrity Controls:**
  - Downloads target archive (`engram-x86_64-pc-windows-msvc.zip`) and corresponding checksum file (`.sha256`) from official GitHub releases.
  - Computes SHA-256 via `Get-FileHash -Algorithm SHA256` and verifies match before extraction.
  - Atomic installation: Extracts to a temporary directory, renames existing binary to `.old`, and moves new binary into place.
  - User PATH modification: Appends `%LOCALAPPDATA%\Programs\Engram\bin` to User Environment PATH (does not modify Machine PATH).
- **Verdict:** **PASS (HIGH-ASSURANCE INSTALL SCRIPT)**.

### 2.3 POSIX Shell Installer (`install.sh`)
- **Target OS:** Linux (x86_64) and macOS (x86_64 / Apple Silicon).
- **Security & Integrity Controls:**
  - Detects OS (`uname -s`) and architecture (`uname -m`).
  - Downloads `.tar.gz` and `.sha256`.
  - Verifies checksum using `sha256sum -c` or `shasum -a 256 -c`.
  - Installs to `$HOME/.local/bin` (or `/usr/local/bin` if explicitly run as root).
  - Uses atomic `mv` replacement.
- **Verdict:** **PASS (POSIX COMPLIANT & SAFE)**.

### 2.4 InnoSetup Windows Installer (`installer/engram.iss`)
- **App Name:** Engram (Version 1.5.4).
- **Default Dir:** `{localappdata}\Programs\Engram`.
- **Privileges:** `PrivilegesRequired=lowest` (runs entirely in user context, zero UAC prompt required).
- **Uninstaller:** Fully registered in Windows Add/Remove Programs; cleans up binary, shortcuts, and user PATH entries cleanly.
- **Verdict:** **PASS (CLEAN USER-SPACE INSTALLER)**.

---

## 3. Build Reproducibility Forensic Analysis

In Phase 5.4.29, two independent, clean isolated builds were executed from the working-tree source using Rust 1.85.0:

```
Build 1: target\isolated_build_1\release\engram.exe (18,803,200 bytes)
SHA-256: AC88FEEF5E28641DE3F74E071031D318734EDD7FAB78B0A0A705D00DA0303110

Build 2: target\isolated_build_2\release\engram.exe (18,803,200 bytes)
SHA-256: 039C62C6BFB26A0A747DB77537D82FFFC8CCAA4F8293B5B263A9552DC2BCED85
```

### Analysis of Hash Divergence:
1. **Size Identity:** Both builds produced exact identical size to the byte (`18,803,200` bytes).
2. **Binary Diff Analysis:** Binary inspection revealed that differences are isolated to:
   - Portable Executable (PE) header `TimeDateStamp` field (32-bit Unix epoch of link time).
   - CodeView debug directory `GUID` (PDB correlation identifier).
   - Embedded link timestamps in debug directories.
3. **Rustc Determinism:** Rust machine code, string literals, AST parsers, and SQLite bindings were bit-for-bit identical.
4. **Classification:** In accordance with Section 13 standards, the build is certified as **`DIVERGENCE_BOUNDED`**, but **`BIT_REPRODUCIBLE_BUILD_NOT_ESTABLISHED`** under MSVC without passing `/Brepro` link flags.

---

## 4. Release Automation Findings & Remediation Plan

| Finding ID | Severity | Area | Description | Recommended Remediation |
|---|---|---|---|---|
| **WF-01** | **Medium** | GitHub CI | `cargo build` does not specify `--locked` flag. | Update workflow to: `cargo build --release --locked --verbose`. |
| **WF-02** | **Low** | GitHub CI | Uses moving `@stable` compiler rather than pinned version. | Update workflow to use `toolchain: "1.85.0"`. |
| **WF-03** | **Low** | Linker | PE timestamps cause binary hash variation across builds. | Add `RUSTFLAGS="-C link-arg=/Brepro"` for Windows MSVC release profile. |
| **WF-04** | **Low** | Packaging | No bundled third-party license notices file. | Add `cargo-about generate` step to CI before archive creation. |

---

## 5. Workflow Gate Recommendation

The release workflow is structurally sound, safe, and ready for Public Alpha packaging subject to applying the non-breaking CI flag improvements (`--locked`, toolchain pinning) prior to official tag dispatch.
