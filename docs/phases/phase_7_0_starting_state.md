# Phase 7.0: Initial Repository Forensic Baseline Report

**Execution Date:** 2026-11-22T23:15:35+05:30  
**Repository Path:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**GitHub Repository:** `code-amalps/Engram`  
**Remote Name:** `origin` (`https://github.com/code-amalps/Engram`)  

---

## 1. Git Repository State

| Parameter | Actual Value | Verification Method | Status |
| :--- | :--- | :--- | :---: |
| **Repository Root** | `C:/Users/Amal P S/OneDrive/Projects/Engram` | `git rev-parse --show-toplevel` | MATCH |
| **Current Branch** | `feature/phase2-stage5-remediation` | `git branch --show-current` | MATCH |
| **Current HEAD Commit** | `2df1cac683435314ffd7ad52369e6d20198fae0d` | `git rev-parse HEAD` | MATCH |
| **Remote Tracking Branch** | `origin/feature/phase2-stage5-remediation` | `git rev-parse --symbolic-full-name @{u}` | MATCH |
| **Remote Branch HEAD** | `2df1cac683435314ffd7ad52369e6d20198fae0d` | `git rev-parse origin/feature/phase2-stage5-remediation` | SYNCHRONIZED |
| **Staged Changes** | None (0 files, empty diff) | `git diff --cached` | CLEAN |
| **Unstaged Changes** | None (0 files, empty diff) | `git diff` | CLEAN |
| **Untracked Files** | Untracked reports, manifests, test scratch | `git status --porcelain` | AUDITED |

---

## 2. Tag Inventory & Resolution

| Tag Name | Target Object | Peeled Target Commit | Notes |
| :--- | :--- | :--- | :--- |
| **`v1.5.4-cand5` (Local)** | `6b4beabdf52beaeb062a5a70c736c168da020bb3` | `2df1cac683435314ffd7ad52369e6d20198fae0d` | Points to release HEAD `2df1cac` |
| **`v1.5.4-cand5` (Origin)** | `6b4beabdf52beaeb062a5a70c736c168da020bb3` | `2df1cac683435314ffd7ad52369e6d20198fae0d` | Verified on GitHub remote |
| **`v1.5.4` (Local)** | `6608e96d63a436c6cc297a804f4d90b515231d6b` | `a75a2c6e2c02acf7af578fd4c5f0b2751efd5988` | Historical tag preserved |
| **`v1.5.4` (Origin)** | `6608e96d63a436c6cc297a804f4d90b515231d6b` | `a75a2c6e2c02acf7af578fd4c5f0b2751efd5988` | Preserved untouched |

---

## 3. Release Artifact Preservation Baseline

| Asset Name | Relative Location | Size (Bytes) | SHA-256 Checksum | Preservation State |
| :--- | :--- | :--- | :--- | :---: |
| **Published Clean ZIP** | `releases/engram-preview-v1.5.4-cand5-clean-windows-x64.zip` | `4,303,797` | `CDD34117E217C44DE0C0F2BD860C04A64350EAEF500A4FC1B39FCABE9B85FF2E` | **IMMUTABLE / PRESERVED** |
| **Published Executable** | `releases/engram-preview-v1.5.4-cand5-clean-windows-x64/bin/engram.exe` | `18,883,584` | `1C5DD68B499298ACE1F6BFFBB4028951DD30C63500604CB355CC0A307E09AD38` | **IMMUTABLE / PRESERVED** |
| **Historical Option A ZIP** | `releases/engram-preview-v1.5.4-cand5-windows-x64.zip` | `4,502,442` | `7C355109EEB8BE7B08301080FD660DE4A4A23DD12AFE89F008546C32FB3C1741` | **IMMUTABLE / PRESERVED** |
| **Historical Option A EXE** | `releases/engram-preview-v1.5.4-cand5-windows-x64/bin/engram.exe` | `19,535,360` | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | **IMMUTABLE / PRESERVED** |

---

## 4. Build Environment & Architecture State

- **Rust Compiler:** `rustc 1.99.0 (b940084d7 2026-09-28)`
- **Cargo Toolchain:** `cargo 1.99.0 (5f94df478 2026-08-27)`
- **Cargo Config:** `.cargo/config.toml` currently does NOT exist (default MSVC linker flags active).
- **Existing Workflows:** `ci.yml`, `engram-guard.yml`, `release.yml`.
- **Instruction Generation Component:** `src/instructions.rs` exists (contains baseline Copilot instruction file generator).

---

## 5. Forensic Verdict

The repository is synchronized with origin at certified release commit `2df1cac683435314ffd7ad52369e6d20198fae0d`. All published and historical release assets are intact, matched to verified SHA-256 hashes, and preserved. Gate A execution is cleared to proceed.
