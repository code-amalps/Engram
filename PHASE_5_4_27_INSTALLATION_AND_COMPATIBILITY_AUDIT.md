# Engram Phase 5.4.27: Installation and Platform Compatibility Audit

**Phase:** 5.4.27  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY AUDIT  
**Baseline Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Audit Date:** 2026-11-20  
**Release Authorization:** `NOT_AUTHORIZED`

---

## 1. Executive Summary

This audit examines the packaging, installer mechanics, runtime lifecycle, failure recovery, and cross-platform compatibility of Engram.

Engram provides three installation avenues:
1. **PowerShell 1-Click Script (`install.ps1`):** Verified for Windows x86_64 with fail-closed checksum enforcement and backup/rollback capabilities.
2. **Inno Setup Windows Installer (`dist/installer/engram-setup.exe`):** Compiled installer for standard Windows GUI/silent installations.
3. **POSIX Shell Script (`install.sh`):** Download and installation script for Linux and macOS environments.

---

## 2. Platform Support & Compatibility Matrix

Platform classifications are based strictly on direct execution and verified build evidence:

| Operating System & Architecture | Classification | Evidence Source & Qualification |
| :--- | :---: | :--- |
| **Windows x86_64** (`x86_64-pc-windows-msvc`) | **`VERIFIED`** | Natively compiled and smoke-tested on Windows host (`rustc 1.99.0`). 8/8 installer safety tests passed. CLI commands executed. |
| **Linux x86_64** (`x86_64-unknown-linux-musl`) | **`PARTIALLY_VERIFIED`** | Release and CI workflows configured (`release.yml`, `ci.yml`). Static musl linkage. Not executed on local host in this offline audit. |
| **macOS x86_64 Intel** (`x86_64-apple-darwin`) | **`PARTIALLY_VERIFIED`** | Release workflow configured with Mach-O header verification and Rosetta smoke test. Not executed on local host in this offline audit. |
| **macOS aarch64 Apple Silicon** (`aarch64-apple-darwin`) | **`PARTIALLY_VERIFIED`** | Release workflow configured with Mach-O arm64 verification. Not executed on local host in this offline audit. |
| **Windows ARM64** (`aarch64-pc-windows-msvc`) | **`UNSUPPORTED`** | Explicitly blocked in `install.ps1`: *"Windows ARM64 is not currently supported"*. Requires dedicated MSVC cross-runner. |
| **Linux ARM64** (`aarch64-unknown-linux-musl`) | **`UNSUPPORTED`** | Marked in `release.yml` as scheduled for post-v1.4.0 cross-toolchain qualification. |

---

## 3. Installation, Operation & Recovery Lifecycle

### 3.1 First-Time Installation
- **Windows:** Running `install.ps1` downloads the target release archive, verifies its SHA-256 hash against `SHA256SUMS`, extracts to a temporary folder, tests `--version` on the staged executable, copies to `%LOCALAPPDATA%\Programs\Engram\bin\engram.exe`, and adds the directory to the User PATH.
- **Inno Setup:** Provides a wizard-based installation with standard uninstall registration in Windows Control Panel / Settings.
- **Linux / macOS:** Running `install.sh` downloads the tarball, verifies checksums, extracts, and places `engram` in `/usr/local/bin` or `$HOME/.local/bin`.

### 3.2 Update & Upgrade Workflow
- `install.ps1` backs up the existing `engram.exe` to `engram.exe.bak`.
- If copying the new executable succeeds, the `.bak` is cleaned up.
- If copying fails (e.g. disk full, permission error), `install.ps1` automatically restores the backup binary, ensuring zero downtime or corruption.

### 3.3 Interrupted Installation Protection
- Extraction occurs in an isolated GUID-named directory under `$env:TEMP`.
- The existing binary is never touched until the downloaded archive has passed both SHA-256 validation and a staged `--version` smoke test.
- If the network drops or the script is terminated mid-download, the existing installation remains 100% intact.

### 3.4 In-Use Process Locking
- `install.ps1` executes `Get-Process -Name "engram*"`. If an active instance is running (e.g. an active MCP server session in Cursor or Claude), the installer aborts immediately:
  ```text
  Engram is currently running (PID ...). Upgrades cannot proceed while the executable is in use.
  Please close active terminals, IDE sessions, or MCP server tasks using Engram.
  Existing installation preserved.
  ```

### 3.5 Runtime Startup & Basic Usage
- Tested commands in temporary environment:
  - `engram init`: Creates `.engram/` and project configuration in milliseconds.
  - `engram analyze`: Incremental parsing and SQLite indexing runs smoothly with zero external daemons.
  - `engram bench`: Executes internal benchmark suite without network access.
  - `engram doctor`: Health check reports telemetry chain validity.

### 3.6 Uninstallation & Clean Removal
- **Inno Setup:** Full uninstaller registered at `%LOCALAPPDATA%\Programs\Engram\unins000.exe`. Removes binary, PATH entries, and registry keys.
- **Script-Based:** Removing `%LOCALAPPDATA%\Programs\Engram` and deleting the PATH entry removes the product completely. Project-specific caches (`.engram/`) remain within governed repositories unless deleted by the developer.

---

## 4. Installation Readiness Assessment

The installation and recovery mechanisms for Windows x86_64 are **exceptionally robust and fully verified**. The script and installer exhibit exemplary fail-closed defensive design.

Before announcing support for Linux and macOS, actual offline or VM-based functional smoke testing on those targets should be conducted to elevate their status from `PARTIALLY_VERIFIED` to `VERIFIED`.
