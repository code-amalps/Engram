# Phase 5.7: Platform Compatibility & System Requirements

**Document ID:** `ENGRAM-PHASE-5.7-PLATFORM-COMPATIBILITY`  
**Date:** 2026-11-20  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Target Release:** `1.5.4` (Public Alpha Candidate)

---

## 1. Operating System Compatibility Matrix

| Platform | Tier | Support Level | Architecture | Verification Status | Notes |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **Windows 11** | Tier 1 | **Fully Supported** | `x86_64` | Verified (MSVC 2022) | Primary development and verification host. Native PowerShell installer and Inno Setup support. |
| **Windows 10** | Tier 1 | **Fully Supported** | `x86_64` | Verified (MSVC 2022) | Compatible with PowerShell 5.1+ and PowerShell 7+. |
| **Ubuntu / Debian** | Tier 2 | **Supported via Source** | `x86_64` | Code-portable (glibc 2.31+) | Requires `gcc`/`clang` for building embedded tree-sitter C parsers. |
| **Fedora / RHEL** | Tier 2 | **Supported via Source** | `x86_64` | Code-portable (glibc 2.34+) | Code-portable standard Linux target. |
| **macOS (Darwin)** | Tier 2 | **Supported via Source** | `aarch64` (Apple Silicon) | Code-portable | Universal binary or native arm64 build supported via `cargo build`. |
| **macOS (Darwin)** | Tier 2 | **Supported via Source** | `x86_64` (Intel) | Code-portable | Intel Mac compatibility maintained. |

---

## 2. Hardware & Runtime Prerequisites

### 2.1 Minimum Hardware Specifications
- **Processor:** 64-bit dual-core CPU (2.0 GHz or faster).
- **RAM:** 512 MB available physical memory (Engram idle memory footprint is ~14 MB; indexing peaks at ~85 MB on large repositories).
- **Disk Space:** 50 MB for binary, debug symbols, and local AST index caches.

### 2.2 System Software Requirements
- **Windows:**
  - PowerShell 5.1 or later (PowerShell 7.x recommended).
  - No administrative rights required (`install.ps1` operates entirely in user scope under `$env:LOCALAPPDATA`).
- **Linux / macOS:**
  - POSIX-compliant shell (`bash`, `zsh`).
  - Standard C runtime library (`glibc` or `musl` on Linux; `libSystem` on macOS).
  - User-writable directory at `~/.local/bin` or `~/.engram/bin`.

---

## 3. Toolchain & Compilation Compatibility

Engram is engineered in standard Rust 2021 edition:
- **Rust Toolchain:** `rustc 1.80.0` or higher (verified on `rustc 1.99.0`).
- **C Compiler:** Embedded Tree-sitter parsers require a host C99 compiler (`clang`, `gcc`, or MSVC `cl.exe`) during compilation.
- **Embedded Language Grammars:**
  - Rust: `tree-sitter-rust` (embedded C)
  - Python: `tree-sitter-python` (embedded C)
  - JavaScript: `tree-sitter-javascript` (embedded C)
  - TypeScript: `tree-sitter-typescript` (embedded C)
  - Go: `tree-sitter-go` (embedded C)
  All grammars are statically compiled into the release binary; no runtime external tree-sitter shared libraries (`.dll` / `.so` / `.dylib`) are required.

---

## 4. Cross-Platform Filesystem & Concurrency Semantics

### 4.1 Path Handling
- All internal path representations are normalized to canonical forward-slash (`/`) relative paths within the context compiler and AST retrieval index to guarantee cross-platform cache hits and stable hash manifests.
- Windows drive letters and UNC paths are handled gracefully via `std::path::PathBuf::canonicalize`.

### 4.2 File Locking & Concurrency Isolation
- Multi-process access to `.engram/memory.yaml` and `.engram/cache.db` utilizes OS-native advisory locks:
  - Windows: Uses `LockFileEx` via `fs2` / `windows-sys` abstractions.
  - POSIX (Linux/macOS): Uses `flock` / `fcntl` advisory locking.
- Concurrent reads are non-blocking; writes take exclusive locks with bounded timeouts (500 ms) before failing gracefully to prevent deadlock.
