# Engram v1.5.1 Release Notes — MCP Health-Check Protocol 1.1 Alignment

**Release Version:** `v1.5.1`  
**Protocol Version:** `1.1` (`PROTOCOL_VERSION = "1.1"`)  
**Release Type:** Patch Release (Bug Fix & Protocol Alignment)  
**Base Release:** `v1.5.0`  
**Target Git Branch:** `main`  
**License:** AGPL-3.0-or-later  

---

## 1. Overview & Purpose

Engram v1.5.1 is a targeted patch release that resolves a setup health-check regression identified immediately following the v1.5.0 release.

While Engram v1.5.0 successfully implemented and exposed all 15 Protocol 1.1 governed MCP tools (introducing `engram_recover`, `engram_drift`, and `engram_ratify_decision`), the client setup health-check logic in `src/setup.rs` retained an outdated fixed-integer expectation (`raw_count != 14`). This caused editor setup verifications (`engram setup cursor`, `vscode`, `claude`, `windsurf`) and automated hosted test jobs to fail with:
`Expected exactly 14 governed tools, server returned 15`.

Engram v1.5.1 aligns `src/setup.rs` with Protocol 1.1, replaces static integer comparisons with dynamic governed tool validation, updates integration test assertions, and re-qualifies hosted CI across all supported operating systems.

---

## 2. Changes & Bug Fixes in v1.5.1

### 1. Governed MCP Tool Surface Alignment (`src/setup.rs`)
- Added `engram_ratify_decision` to `GOVERNED_TOOL_NAMES`, ensuring all 15 Protocol 1.1 governed MCP tools are enumerated and validated.
- Replaced the hardcoded `raw_count != 14` assertion with `raw_count != GOVERNED_TOOL_NAMES.len()`. Future tool surface additions will no longer cause assertion drift.
- Dynamic error messaging now accurately reports the required count based on the governed tool registry.

### 2. Integration Test Maintenance (`tests/setup_integration_test.rs`)
- Imported `GOVERNED_TOOL_NAMES` into setup integration tests.
- Updated tool-count assertions from 14 to 15 in `test_verify_mcp_health_validation_and_clean_termination` and `test_verify_mcp_health_bounded_timeout`.
- Added explicit regression assertion:
  `assert!(GOVERNED_TOOL_NAMES.contains(&"engram_ratify_decision"))`.

### 3. Engine & Extension Version Metadata
- Bumped engine package version to `1.5.1` in `Cargo.toml` and `Cargo.lock`.
- Bumped VS Code extension package version to `1.5.1` in `extensions/vscode/package.json` and `package-lock.json`.

---

## 3. Verified Multi-Platform CI Status

The fix commit (`a26a01a`) was tested and verified across all hosted runner environments on `main` prior to release preparation:
- **Ubuntu** (`ubuntu-latest`): `completed / success` (CI run 39017318962)
- **macOS** (`macos-latest`): `completed / success` (CI run 39017318962)
- **Windows** (`windows-latest`): `completed / success` (CI run 39017318962)

All 29 integration test suites, unit tests, and release compilation checks passed with a 100% success rate.

---

## 4. Documented Boundaries & Exclusions

1. **Scope Boundary:** This patch release introduces zero architectural changes, zero new command-line flags, and zero protocol modifications beyond the setup health-check fix.
2. **Multi-Platform Distribution:** Multi-platform standalone binaries for Windows, Linux, and macOS (Intel & Apple Silicon) will be produced and attested by GitHub Actions on tag publication.

---

## 5. Upgrade Instructions

Engram v1.5.1 is 100% backward-compatible with v1.5.0 and earlier v1.x deployments:
```powershell
# Windows (PowerShell)
iwr -useb https://raw.githubusercontent.com/code-amalps/Engram/main/install.ps1 | iex
```
```bash
# Linux & macOS (Bash / Sh)
curl -fsSL https://raw.githubusercontent.com/code-amalps/Engram/main/install.sh | sh
```
```bash
# Verify installation
engram --version
# Expected output: engram 1.5.1
```
