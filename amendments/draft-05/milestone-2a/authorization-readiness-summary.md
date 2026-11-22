# Engram Amendment 05 — Milestone 2A: Authorization Readiness Summary

## 1. Executive Summary

This document summarizes the completion of **Milestone 2A: Smoke-Test Specification Correction** under Engram Amendment 05.

All investigations and corrections were conducted **strictly offline** with zero model inference, zero API requests, zero network calls, zero benchmark executions, zero binary rebuilds, and zero spend (\$0.00).

$$\mathbf{Milestone\ 2A\ Readiness\ Classification:\ READY\ WITH\ DOCUMENTATION\ QUALIFICATIONS}$$

*(Technical and procedural documentation is fully corrected and qualified. Live execution remains strictly held at the authorization gates awaiting explicit owner sign-off.)*

---

## 2. Summary of Corrected Technical Specifications

| Topic Area | Milestone 2 Specification (Prior) | Milestone 2A Corrected Specification | Governance Impact |
|:---|:---|:---|:---|
| **Binary & Arm Mapping** | Invocation specified `--arms v154_arm3` without path flags, resolving by default to frozen v1.5.4 (`bin/v1.5.4/engram.exe`). | Command explicitly specifies `--engram-v154-bin "benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe"` and `--expected-v154-sha256 "F56347EF...23D85BEE"`. | **Harmonized.** Authorized binary, path, SHA-256, and arm mapping all refer to the Candidate Phase 1 binary. |
| **Token Guard Guarantee** | Claimed the 2,000 reserve "prevents overrun beyond the 10,000-token ceiling." | Retracted guarantee. Documented as a defensive heuristic estimate. Whole-request overshoot can breach nominal ceiling if a single turn exceeds headroom. | **Qualified.** Preventive stops (`PRE_DISPATCH_HEADROOM_EXHAUSTED`) are formally distinguished from ceiling breaches (`TOKEN_CEILING_BREACHED`). |
| **Source Workspace** | Generic reference to `scratch/` without identifying symbol source. | Explicitly pinned to commit `1c2856086f3a...` in `angular-ngrx-material-starter` (authLogout in `auth.actions.ts:14`). Created via `git worktree add --detach` with `--dependency-strategy none`. | **Contained.** Zero uncommitted golden files and zero `.engram/` files copied. Zero mutations verified before and after. |
| **Model Compatibility** | Assumed `gpt-5.6-luna` valid without qualification. | Verified present in local `~/.codex/models_cache.json` (supported in API, 272k context). Active server entitlement marked `OWNER VERIFICATION REQUIRED`. | **Gated.** Model substitution prohibited without owner approval. |
| **Token Estimate** | Presented as expected consumption (3,000–6,000 tokens). | Classified as an unverified planning estimate / heuristic calculation. | **Qualified.** Not a measured or guaranteed consumption range. |

---

## 3. Corrected Command Template for Owner Authorization

When authorized by the project owner, the smoke test must be executed using the fully specified command:

```powershell
python benchmarks/matched-pair-harness/src/orchestrator.py `
  --task smoke_test_read_only `
  --arms v154_arm3 `
  --engram-v154-bin "benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe" `
  --expected-v154-sha256 "F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE" `
  --max-tokens-ceiling 10000 `
  --headroom-reserve 2000 `
  --model gpt-5.6-luna `
  --reasoning low `
  --dependency-strategy none `
  --output-dir "benchmarks/matched-pair-harness/reports/smoke_test"
```

---

## 4. Preservation Status of Protected Baseline Artifacts

All baseline artifacts remain 100% verified and intact:

| Component | Path | Baseline Hash / Commit | Verification Status |
|:---|:---|:---:|:---:|
| **v1.5.1 Frozen** | `matched-pair-harness/bin/v1.5.1/engram.exe` | `474A1A7AAFBB5A5B52418AA51A978A58AEAE4A177C62EC17C526E165610F44E0` | **MATCH (UNTOUCHED)** |
| **v1.5.4 Frozen** | `matched-pair-harness/bin/v1.5.4/engram.exe` | `CF807EACDEB36DC972019838EDEEB2FA5E8BE25F6C3926FC662189FA9430A798` | **MATCH (UNTOUCHED)** |
| **Candidate Phase 1** | `matched-pair-harness/bin/candidate-phase1/engram.exe` | `F56347EF2F0BFD4854176A3EC3C86D8D20B4C0E5954F04E4464998F623D85BEE` | **MATCH (UNTOUCHED)** |
| **Golden Repository** | `Projects/Test/angular-ngrx-material-starter` | Commit `1c2856086f3a...` (`test-branch`, 18 modified files, 29 `.engram/` files) | **PRESERVED DIRTY BASELINE** |
| **Historical Evidence** | `benchmarks/STAGE1_BLOCK0*.md`, Amendment 04 | Historical contents | **MATCH (UNTOUCHED)** |

---

## 5. Deliverables Created Under `amendments/draft-05/milestone-2a/`

1. [`amendments/draft-05/milestone-2a/specification-corrections.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-2a/specification-corrections.md)  
   *Comprehensive documentation-only corrections addressing arm resolution, token guard qualifications, workspace isolation, model compatibility, and planning estimates.*
2. [`amendments/draft-05/milestone-2a/authorization-readiness-summary.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-2a/authorization-readiness-summary.md)  
   *Executive summary of corrected specifications, reconciled command templates, baseline preservation, and authorization gates.*

---

## 6. Outstanding Owner Decisions (Execution Gating)

Live execution remains **strictly blocked** until the project owner provides formal sign-off on the three gates in [`amendments/draft-05/milestone-2/milestone-2-authorization-gates.md`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/amendments/draft-05/milestone-2/milestone-2-authorization-gates.md):

1. **Gate 1 (Token Budget Allocation):** Formal approval of Option A (10,000 ceiling / 2,000 reserve) or alternative.
2. **Gate 2 (Subscription & Auto-Purchase Certification):** Inspection of ChatGPT account settings to confirm active plan tier and certify auto-purchasing is disabled.
3. **Gate 3 (Live Execution Sign-Off):** Explicit signature authorizing single-arm headless execution.

**Zero live inference has been launched. No further actions or milestones will be initiated without explicit owner direction.**
