# Engram Phase 5.4.31: Evidence Index & Verification Traces

**Phase:** 5.4.31  
**Scope:** Verification Evidence, Installer Forensic Review, Test Logs, and Discrepancy Reconciliation  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  

---

## 1. Executive Summary

This document indexes all forensic traces and verifications conducted in Phase 5.4.31, including dependency count mathematics, AGPL-3.0 and MIT/BSD notice obligations, public claim revisions, working-tree classification, and installer safety architecture.

---

## 2. Dependency Count Reconciliation Evidence

- **Script:** Python 3.11 `tomllib` parser on `Cargo.lock`.
- **Total `[[package]]` entries:** 115.
- **Root workspace package:** `engram 1.5.4` (1 package).
- **External dependency packages:** 114 package entries from `crates.io`.
- **Unique external crate names:** 112 unique names.
- **Dual-version packages:**
  - `hashbrown`: versions `0.14.5` and `0.17.1`.
  - `syn`: versions `2.0.119` and `3.0.6`.
- **Formula:** $112 \text{ unique} + 2 \text{ duplicates} = 114 \text{ external} + 1 \text{ root workspace} = 115 \text{ total packages}$.

---

## 3. Installer Safety Forensic Evidence

### 3.1 InnoSetup Windows Installer (`installer/engram.iss`)
- **Privilege Level:** `PrivilegesRequired=lowest` (runs in standard user context; zero elevation required).
- **Target Path:** `{localappdata}\Programs\Engram\bin`.
- **Registry Integration:** Writes strictly to `HKCU\Environment\Path` using custom Pascal routine `NeedsAddPath`.
- **Uninstallation Safety:** Custom procedure `CurUninstallStepChanged` surgically strips `{app}\bin` and `{app}\bin\` from `HKCU\Environment\Path`, leaving other user path entries untouched.

### 3.2 PowerShell Installer (`install.ps1`)
- **Process Lock Prevention:** Pre-flight probe checks `Get-Process -Name "engram*"`. If active, aborts with clear error message, preserving existing installation.
- **Cryptographic Validation:** Downloads `SHA256SUMS`, verifies exact regex match, verifies `Get-FileHash -Algorithm SHA256` against `SHA256SUMS`. Fails closed on any mismatch.
- **Staged Smoke Test:** Extracts to `$TEMP`, invokes staged binary with `& $ExtractedBin --version`, checks exit code 0 before moving to `$InstallDir`.
- **Atomic Replacement & Rollback:** Backs up current `engram.exe` to `engram.exe.bak`. If replacement fails, restores from backup.

---

## 4. Working Tree State & Public Claims Diff Trace

- **Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`.
- **Staged diff:** 0 bytes.
- **Tracked modified files:** Exactly 13 files.
- **`README.md` diff trace:**
  ```diff
  - Droid queries only the exact functions and types it needs, cutting token usage by over 80%.
  + Droid queries targeted function and type signatures rather than dumping whole repositories. *(Note: While designed to substantially reduce token consumption, quantitative token reduction varies by repository and model; formal empirical validation via live matched-pair benchmark suites is currently under evaluation).*
  ```
