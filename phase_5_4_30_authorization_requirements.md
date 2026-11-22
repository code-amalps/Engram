# Engram Phase 5.4.30: Operational Authorization & Transition Requirements

**Phase:** 5.4.30  
**Scope:** Specification of Authorization Envelopes, Safety Gates, and Release Transition Protocols  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  

---

## 1. Executive Summary

Throughout Phases 5.4.20 through 5.4.30, all operations were executed under strict air-gapped, offline, read-only, or isolated build constraints. To transition Engram from the current certified state (`PUBLIC_ALPHA_READY_WITH_EXPLICIT_LIMITATIONS`) to an active, distributed public release, specific explicit user authorizations are mandatory.

No autonomous agent or developer script may assume or bypass these authorizations.

---

## 2. Authorization Envelope Matrix

| Authorization Domain | Current Status | Required Action / Unlock Gate | Prerequisite Conditions |
|---|---|---|---|
| **Git Mutation (`COMMIT`, `MERGE`, `RESET`)** | `NOT_AUTHORIZED` | Committing the 12 tracked modified files and phase reports to Git. | Final human sign-off on release-gate certification. |
| **Git Tagging (`TAG`)** | `NOT_AUTHORIZED` | Creating Git tag `v1.5.4` on the certified commit. | Clean working tree after commit; zero uncommitted modifications. |
| **Git Remote Communication (`PUSH`)** | `NOT_AUTHORIZED` | Pushing branch `feature/phase2-stage5-remediation` or `main` and tag `v1.5.4` to GitHub. | Tag creation confirmed; remote origin URL verified. |
| **Live Network Access (`ONLINE_AUDIT`)** | `NOT_AUTHORIZED` | Running `cargo audit` and `cargo deny` against live RustSec advisory databases. | Explicit network boundary waiver granted for crates.io and github.com/rustsec. |
| **Live Model API (`LLM_API`)** | `NOT_AUTHORIZED` | Supplying Anthropic / OpenAI / Google API keys to benchmark harness. | Live benchmark authorization granted; budget caps configured. |
| **Live Benchmark Dispatch (`BENCHMARK`)** | `NOT_AUTHORIZED` | Executing Task B Live Tetrad (`A -> D -> C -> B`) under live model APIs. | Live Model API authorized; fail-closed tetrad validator confirmed active. |
| **Release Publishing (`PUBLISH`)** | `NOT_AUTHORIZED` | Uploading binaries to GitHub Releases and dispatching InnoSetup installers. | GitHub Actions release workflow triggered via authenticated tag push. |
| **Binary Overwrite (`BINARY_WRITE`)** | `NOT_AUTHORIZED` | Replacing `target/release/engram.exe` or overwriting baseline fixtures. | New version bump or clean rebuild explicitly directed by user. |

---

## 3. Step-by-Step Transition Protocol for Public Alpha Release

When the project owner decides to proceed with the official public alpha release, the following sequential steps must be followed:

### Step 1: Human Review & Disclaimer Approval
- Review `phase_5_4_30_final_release_gate_report.md` and the 4 mandatory disclosures.
- Decide whether to update `README.md` lines 314–315 with the recommended qualification or ship with the disclosure note.

### Step 2: Grant Git Mutation Authorization
- Execute git staging and commit of the 12 modified files and release documentation:
  ```powershell
  git add Cargo.toml src/ benchmarks/ phase_5_4_30_*
  git commit -m "chore(release): prepare Engram v1.5.4 public alpha baseline"
  ```

### Step 3: Grant Online Network & Dependency Audit Authorization
- Execute an air-gap check and run the live security audit:
  ```powershell
  cargo audit
  ```
- If zero high/critical advisories are reported, mark finding `F-5430-01` as CLOSED.

### Step 4: Grant Tagging & Push Authorization
- Tag the release commit and push to remote:
  ```powershell
  git tag -a v1.5.4 -m "Engram v1.5.4 Public Alpha"
  git push origin feature/phase2-stage5-remediation
  git push origin v1.5.4
  ```

### Step 5: CI Automation & Distribution Verification
- Monitor GitHub Actions `.github/workflows/release.yml`.
- Verify compilation across Windows, Linux, and macOS runners.
- Verify cryptographic checksums of generated release archives and InnoSetup installers.
- Publish GitHub Release as `pre-release` (Public Alpha) accompanied by the 4 certified limitation disclosures.
