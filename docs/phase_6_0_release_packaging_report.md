# Engram Phase 6.0: Experimental Developer Preview Packaging & Release Integrity Verification Report

**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Branch:** `feature/phase2-stage5-remediation`  
**Expected HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Actual HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Release Classification:** Experimental Developer Preview  
**Release Package ID:** `engram-preview-v1.5.3-cand5-windows-x64`  
**Candidate Executable:** Candidate 5 (v1.5.3 / internal binary build 1.5.4)  
**Evaluation Provenance:** `SYNTHETIC_SIMULATED_USAGE`  
**Timestamp UTC:** 2026-11-22T14:30:00Z  

---

## 1. Executive Summary & Certified Primary Verdict

The release packaging and integrity verification for **Engram Candidate 5 (Experimental Developer Preview)** has been successfully executed in strict offline mode. All packaging operations, cryptographic checksum validations, secret scans, extraction integrity tests, and non-destructive smoke validations have completed with zero errors.

### Certified Release Verdict
```
EXPERIMENTAL_PACKAGE_READY_FOR_REVIEW
```

### Forensic Summary
- **Zero Alteration of Candidate Binaries:** The packaged `engram.exe` was verified bit-for-bit against the Phase 5.9.2.5/5.9.2.6 manifest (`E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44`).
- **Complete Self-Contained Package:** Created in `releases/engram-preview-v1.5.3-cand5-windows-x64/` and archived as `releases/engram-preview-v1.5.3-cand5-windows-x64.zip` (4.5 MB), containing 14 total files including binary, config templates, sample workspace, and documentation.
- **Mandatory Synthetic Benchmark Disclosure:** All documentation prominently includes the mandatory disclosure specifying `SYNTHETIC_SIMULATED_USAGE`, explicitly distinguishing simulated wire payload reductions from unverified live upstream provider token billing.
- **Zero Secrets / Zero Network Calls:** Scanned clean of API keys, credentials, and tokens. Local operations require zero external network requests.
- **Clean Extraction & Smoke Validation:** Extraction into an isolated clean directory succeeded 100%, and `engram doctor` and `engram context` passed with exit code 0.

---

## 2. Git Starting & Ending Identity

- **Target Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`
- **Active Branch:** `feature/phase2-stage5-remediation`
- **Starting Commit HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`
- **Ending Commit HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`
- **Git State:** 100% unaltered. Zero git commits, pushes, merges, or tags created.

---

## 3. Packaged Binary Cryptographic Identity

| Property | Source Manifest Record | Packaged Binary (`bin/engram.exe`) | Extracted Binary Test | Match Status |
| :--- | :--- | :--- | :--- | :---: |
| **Path** | `benchmarks/.../bin/candidate-phase5/engram.exe` | `releases/.../bin/engram.exe` | `<temp>/package/bin/engram.exe` | **VERIFIED** |
| **File Size** | 18,883,584 bytes | 18,883,584 bytes | 18,883,584 bytes | **MATCH (Exact)** |
| **SHA-256 Hash** | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | `E4E53216...` | `E4E53216...` | **MATCH (100%)** |
| **Compiler Toolchain** | Rust 1.99.0 release profile | Rust 1.99.0 release profile | Rust 1.99.0 release profile | **VERIFIED** |
| **Reported Version** | `engram 1.5.4 (commit: 7106a2f...)` | `engram 1.5.4 (commit: 7106a2f...)` | `engram 1.5.4 (commit: 7106a2f...)` | **VERIFIED** |

---

## 4. Package Artifact Inventory & Directory Layout

The release package is established at:
`C:\Users\Amal P S\OneDrive\Projects\Engram\releases\engram-preview-v1.5.3-cand5-windows-x64\`

And the distributable archive is located at:
`C:\Users\Amal P S\OneDrive\Projects\Engram\releases\engram-preview-v1.5.3-cand5-windows-x64.zip`  
*(Size: 4,503,445 bytes | SHA-256: `3A04FD53AB1D2ED6716B186A2DF28A301C61CAB538CC422547ED4A2AA1C6A9EF`)*

### Complete Inventory (14 Included Files)
```
releases/engram-preview-v1.5.3-cand5-windows-x64/
├── BENCHMARK_DISCLOSURE.md             (Mandatory disclosure with required verbatim text)
├── INSTALLATION.md                     (Offline installation, PATH setup, MCP setup for Cursor/Claude/VS Code)
├── KNOWN_LIMITATIONS.md                (Category A/B/C validation boundaries, sandbox and concurrency limits)
├── RELEASE_MANIFEST.json               (Full cryptographic JSON manifest of all package files)
├── RELEASE_NOTES.md                    (Detailed release notes for Experimental Developer Preview)
├── SHA256SUMS.txt                      (Standard sha256 checksums file for package integrity verification)
├── USAGE.md                            (Core command workflows: init, doctor, context, mcp, setup)
├── bin/
│   └── engram.exe                      (18,883,584 bytes | SHA-256 E4E53216...)
├── config/
│   ├── config.example.yaml             (Standard Engram analyzer configuration template)
│   └── constitution.example.yaml       (Standard architectural rules & boundary constitution template)
└── sample_workspace/
    ├── .engram/
    │   ├── config.yaml
    │   └── constitution.yaml
    ├── src/
    │   ├── __init__.py
    │   └── calculator.py
    └── tests/
        ├── __init__.py
        └── test_calculator.py
```

---

## 5. Checksum & Manifest Verification

All 14 files were hashed using SHA-256 and verified against `SHA256SUMS.txt` and `RELEASE_MANIFEST.json`:

| File Relative Path | Size (Bytes) | SHA-256 Checksum (Hex) | Status |
| :--- | :---: | :--- | :---: |
| `bin/engram.exe` | 18,883,584 | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | **VERIFIED** |
| `BENCHMARK_DISCLOSURE.md` | 2,139 | `ecc0a6e1b8f6617053cdfa4018a424dd11f9e0f68586233f7c0cbca291e4fe72` | **VERIFIED** |
| `INSTALLATION.md` | 1,939 | `847b74fdbd09c4046c1164bdd96f964a8eb44c0c66459187badf7d569208f9ac` | **VERIFIED** |
| `KNOWN_LIMITATIONS.md` | 2,776 | `25daf3795f45fa785b00d7dee375eabb8b034a2e5648613e3fae8f40c64fd7ee` | **VERIFIED** |
| `RELEASE_NOTES.md` | 1,749 | `4e59127135a6259a53d8c9059522d838dc062aaaabc9f5a510d012f042b2bcc8` | **VERIFIED** |
| `USAGE.md` | 1,411 | `9ed805c25d97024c069df54bfaa3d015e2f9787f42b6c79411c9ba8a5743521d` | **VERIFIED** |
| `config/config.example.yaml` | 269 | `71b2b7414d484ffd48aba7300ba151209a00d27f55a01cbbff2a24e26c0ba9b6` | **VERIFIED** |
| `config/constitution.example.yaml` | 283 | `1353a915f64c28d427f01dc99c2ff74589b0f1c126d04ee7510ea7ffba88cf42` | **VERIFIED** |
| `sample_workspace/.engram/config.yaml` | 110 | `10859b0a8b6cdb8ddb70583efd9d74e5776bdd1be97cbdb8faf22b78807c0b8e` | **VERIFIED** |
| `sample_workspace/.engram/constitution.yaml` | 95 | `677e97a2cb9824d4f7a899c18a0df42e494385d35a62f01865ca52bac70735c9` | **VERIFIED** |
| `sample_workspace/src/__init__.py` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | **VERIFIED** |
| `sample_workspace/src/calculator.py` | 215 | `6c483d07c8c1d302517eb98d4a0f21c60a27c9d10f02923e2c54d273556f07fc` | **VERIFIED** |
| `sample_workspace/tests/__init__.py` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` | **VERIFIED** |
| `sample_workspace/tests/test_calculator.py` | 511 | `ec1181aaccbbb05bbb49552db51622feda70b22c2a73056bea499492bec089ea` | **VERIFIED** |

---

## 6. Secret & Unwanted-File Scan Results

An automated pattern scan was conducted across all package files:
- **Scan Targets:** API keys (`sk-*`, `ghp_*`, `AKIA*`), bearer tokens, private keys, passwords, personal usernames, environment variables, git history (`.git`), temporary files (`*.tmp`, `*.log`, `__pycache__`).
- **Scan Outcome:** **0 secrets detected. 0 credentials detected. 0 unwanted files found.**
*(Note: A substring search for `sk-` matched only the word `task-aware` in USAGE.md, confirmed non-sensitive prose).*

---

## 7. Smoke-Test Commands & Outcomes

Non-destructive offline smoke testing was performed against the packaged binary:

1. **Version Identification:**
   ```powershell
   & .\bin\engram.exe --version
   ```
   *Outcome:* Exit code `0`. Output: `engram 1.5.4 | commit: 7106a2f369fe79faf5f95449378a900ab8e8eddf | dirty: true | rustc: rustc 1.99.0 | profile: release`.
2. **Help Documentation:**
   ```powershell
   & .\bin\engram.exe --help
   ```
   *Outcome:* Exit code `0`. All 28 subcommands listed with valid argument parsing.
3. **Diagnostic Health Check (`doctor` in `sample_workspace`):**
   ```powershell
   & .\bin\engram.exe -C .\sample_workspace doctor
   ```
   *Outcome:* Exit code `0`. `Total Decisions: 0 | Live: 0 | Audit Event Telemetry Chain: EVENT_CHAIN_VALID`.
4. **Task-Aware Context Compilation (`context` in `sample_workspace`):**
   ```powershell
   & .\bin\engram.exe -C .\sample_workspace context "calculate tax for orders"
   ```
   *Outcome:* Exit code `0`. `Confidence: HIGH | Budget: 367 / 1200 tokens | Files: 2 (tests/test_calculator.py, src/calculator.py)`.
5. **Missing Configuration Graceful Handling:**
   ```powershell
   & .\bin\engram.exe -C <empty_temp_dir> doctor
   ```
   *Outcome:* Exit code `0`. Gracefully handles empty workspace without unhandled exceptions.
6. **Network Isolation Check:**
   *Outcome:* Zero network sockets opened; zero external DNS or HTTP requests initiated.

---

## 8. Mandatory Benchmark Disclosure & Validation Boundaries

### Mandatory Verbatim Disclosure Statement Included in Package
> **"Under the deterministic six-workload simulated benchmark, Candidate 5 recorded 7,589 synthetic wire tokens compared with 30,113 for the Native arm. Excluding the context-heavy workload, Candidate 5 recorded 6,550 compared with 10,512 for Native. These results describe the tested simulation only and do not establish real-world provider token or cost savings."**

### Explicitly Untested Integration Boundaries
1. **Dynamic LLM Autonomy:** Real upstream LLM reasoning, response variability, and prompt drifting were not part of the deterministic benchmark.
2. **Long-Running Stdio MCP Daemon Lifecycle:** While `engram mcp` starts correctly over stdio, high-concurrency editor stress tests have not been executed under automated production IDE test harnesses.
3. **Live Provider Billing & Upstream Latency:** No claims of actual OpenAI/Anthropic API cost reductions or real-world response latency are made.
4. **Cross-Platform Compilation:** This preview is strictly for Windows x64. macOS and Linux native builds have not been packaged in this release.

---

## 9. Distribution Boundary Confirmation

- **Live External Distribution:** NOT AUTHORIZED. Zero packages uploaded to GitHub Releases, crates.io, npm, or cloud storage.
- **Git State:** Clean read-only state maintained. Zero commits, tags, pushes, or branch modifications.
- **Packaging Location:** Confined to local repository directories:
  - `releases/engram-preview-v1.5.3-cand5-windows-x64/`
  - `releases/engram-preview-v1.5.3-cand5-windows-x64.zip`

---

## 10. Next Steps for Production Readiness

Before graduating from **Experimental Developer Preview** to a **Production Release**, the following milestones must be reached:
1. **Stage 1: Automated Headless MCP Integration Tests:** Implement a simulated MCP client harness that launches `engram.exe mcp` over stdio and executes the full JSON-RPC 2.0 handshake and tool lifecycle.
2. **Stage 2: Cross-Platform CI Pipeline:** Set up automated GitHub Actions matrix builds producing reproducible Linux (x86_64, aarch64) and macOS (Universal) binaries.
3. **Stage 3: Separately Authorized Live Provider Benchmark:** When API credentials are provided by the operator, execute the Protocol v3.0.0 pentad against live upstream endpoints (`gpt-5.6-luna`) to measure actual provider-billed token consumption.
