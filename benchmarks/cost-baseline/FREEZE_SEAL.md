# Engram Phase 0–2 Measurement Pilot — Baseline Freeze Seal

**Seal Timestamp:** 2026-10-18T06:33:00Z  
**Execution Authority:** User Authorized Integrity Gate Passed  
**Candidate Git HEAD Revision:** `17fdf4a2e98f98e6d192afc28271cb8de47aa497` (Branch `Development`)  
**Status:** **SEALED & LOCKED**

---

## 1. Candidate Binary Seal

| Property | Value |
|---|---|
| **Binary Path** | `target\release\engram.exe` |
| **File Size** | 17,722,880 bytes |
| **SHA-256 Hash** | `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` |
| **Reported Version** | `engram 1.5.1` |
| **Toolchain** | `rustc 1.99.0 (b940084d7 2026-09-28)`, `cargo 1.99.0 (5f94df478 2026-08-27)` |
| **Target Architecture** | `x86_64-pc-windows-msvc` |

---

## 2. Integrity Gate Verification Summary

- **Total Build Inputs Re-Hashed:** 49 files (46 Rust source files in `src/`, `Cargo.toml`, `Cargo.lock`, `target/release/engram.exe`).
- **Hash Mismatches:** **0** (100% exact match).
- **Build Manifest:** [`benchmarks/cost-baseline/BUILD_MANIFEST.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/BUILD_MANIFEST.md)
- **Sealed Preregistration:** [`benchmarks/cost-baseline/PREREGISTRATION.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/cost-baseline/PREREGISTRATION.md)  
  *SHA-256:* `db3ab2956a4da40aad8f1f376aa72a50bebc96f3e1ecc3120f405f0a294ea8bb` (14,756 bytes).

---

## 3. Configuration & Path Alignment

Both active execution endpoints point to the sealed release executable:
1. **Global VS Code User Configuration:**
   - Path: `C:\Users\Amal P S\AppData\Roaming\Code\User\settings.json`
   - Value: `"engram.binaryPath": "C:\\Users\\Amal P S\\OneDrive\\Projects\\Engram\\target\\release\\engram.exe"`
2. **Target Workspace MCP Configuration:**
   - Path: `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter\.vscode\mcp.json`
   - Value: `"command": "C:\\Users\\Amal P S\\OneDrive\\Projects\\Engram\\target\\release\\engram.exe"`

---

## 4. Locked Experimental Parameters

- **Sample Size:** Strictly fixed at 5 valid matched pairs per task (total 20 valid runs). No runs added based on observed variance.
- **Randomization Seed:** `20261018` (locked order: Task A = BABA BABA BA; Task B = BABA BABA BA; 60/40 first-run imbalance documented).
- **Blinded Review:** Evaluated by user on anonymized `git diff --no-prefix` code diffs with metadata stripped.
- **Telemetry Accounting:** Strictly isolated per-run timestamped windows; no imputation of credits from wrapper logs.
- **Acceptance Procedure:** Full TypeScript build gates (`tsc -p tsconfig.app.json --noEmit` and `tsconfig.spec.json` both exit 0); browser runtime reload explicitly reported as **UNVERIFIED AT RUNTIME** due to probe outcome on un-modified golden repository.
- **Interrupted Run Rule:** Replacement cap of 2 per arm for infrastructure aborts only. Interrupted expenditure included in cumulative accounting.

---

## 5. Non-Destructive Safeguard Attestation

- Zero tracked modifications were reverted, deleted, stashed, or reset.
- Zero untracked artifacts were cleaned.
- Zero commits, pushes, tags, or releases were created.
- Zero Copilot sessions or benchmark runs were executed.
- Zero model tokens were consumed.
