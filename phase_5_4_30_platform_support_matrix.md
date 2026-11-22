# Engram Phase 5.4.30: Platform Support Matrix & Compatibility Report

**Phase:** 5.4.30  
**Audit Scope:** Target Architecture, Operating System Integration, Native Tooling, and Verification Tiers  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  

---

## 1. Executive Summary

Engram v1.5.4 is architectured as a cross-platform Rust binary supporting modern desktop and server operating systems. However, under the strictly offline constraints of this audit, functional execution has been forensically verified only on Windows x86_64. Other platforms are compiled and tested in GitHub Actions CI but remain unverified locally.

---

## 2. Platform Support Classification Tiers

| Platform / Triple | OS & Architecture | Verification Tier | Packaging & Installation | Verification Status & Notes |
|---|---|---|---|---|
| **`x86_64-pc-windows-msvc`** | Windows 10, 11, Server 2019+ (x86_64) | **Tier 1 (Supported & Verified)** | `install.ps1`, InnoSetup Installer (`engram-setup.exe`), `.zip` | **Fully Verified Offline:** `engram.exe` compiled cleanly, CLI smoke tests (`init`, `analyze`, `doctor`) passed with code 0, stdio MCP server verified. |
| **`x86_64-unknown-linux-gnu`** | Linux glibc 2.31+ (Ubuntu 20.04+, Debian 11+, Fedora, Arch) | **Tier 2 (Supported / CI-Verified)** | `install.sh`, `.tar.gz` | **Supported in Source & CI:** POSIX paths, clipboard (`wl-copy`/`xclip`), and signal liveness implemented; not tested locally in current Windows environment. |
| **`x86_64-apple-darwin`** | macOS 12+ (Intel x86_64) | **Tier 2 (Supported / CI-Verified)** | `install.sh`, `.tar.gz` | **Supported in Source & CI:** Darwin path resolution, `pbcopy`, CoreFoundation bindings; not tested locally. |
| **`aarch64-apple-darwin`** | macOS 12+ (Apple Silicon M1/M2/M3/M4) | **Tier 2 (Supported / CI-Verified)** | `install.sh`, `.tar.gz` | **Supported in Source & CI:** Compiled natively on macOS runners; not tested locally. |
| **`aarch64-unknown-linux-gnu`** | Linux ARM64 (Raspberry Pi 4/5, AWS Graviton) | **Tier 3 (Experimental)** | Manual source build (`cargo build --release`) | Standard Rust toolchain targets supported; requires GCC/glibc ARM64. |
| **`aarch64-pc-windows-msvc`** | Windows 11 ARM64 (Qualcomm Snapdragon X) | **Tier 3 (Experimental)** | Emulated via x86_64 binary or manual build | Runs via Windows 11 x86_64 emulation layer or native compilation. |

---

## 3. OS-Specific Subsystem Analysis

### 3.1 Filesystem & Path Normalization
- **Windows:**
  - `src/mcp.rs` and `src/storage.rs` handle Windows extended-length paths (`\\?\` verbatim prefixes) emitted by `std::fs::canonicalize()`.
  - Normalizes case-insensitivity quirks on NTFS/ReFS filesystems.
- **Unix / macOS:**
  - Case-sensitive standard POSIX paths; handles symlink canonicalization and root containment.

### 3.2 System Clipboard Integration
- **Windows:** Invocations of `clip.exe` passing piped UTF-8 payload.
- **macOS:** Invocations of `pbcopy` passing piped UTF-8 payload.
- **Linux:** Probes for `wl-copy` (Wayland) first, falling back to `xclip -selection clipboard` (X11).

### 3.3 Process Liveness Checks
- **Windows:** Parses `tasklist /FI "PID eq <PID>" /FO CSV /NH` with strict integer PID validation.
- **Unix / macOS:** Uses standard POSIX `kill -0 <PID>` signal check.

---

## 4. Platform Certification Conclusion

Engram v1.5.4 is certified as **Tier 1 Verified on Windows x86_64** and **Tier 2 Compatible on Linux x86_64 and macOS (Intel & Apple Silicon)**. The release limitations statement must disclose that Linux and macOS distributions are provided as CI-tested previews pending local runtime certification.
