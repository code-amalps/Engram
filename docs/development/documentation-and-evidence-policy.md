# Engram Documentation & Engineering Evidence Policy

**Version:** 1.0.0  
**Effective Date:** 24 November 2026  
**Status:** Active maintained policy  
**Audience:** All engineers, automated coding agents, and contributors working on the Engram repository.

---

## 1. Core Principle & Purpose

The purpose of this policy is to maintain high repository hygiene, clarity, and signal-to-noise ratio in Engram.

**Governing Invariant:**  
> *Git branches must contain only the maintained Engram product, its necessary user and developer documentation, and essential regression fixtures—never the cumulative raw evidence, temporary audit notes, or session transcripts of the engineering processes used to build them.*

The presence of technical information in a document does not make it eligible for Git tracking. Engineering evidence belongs in local session logs and review channels, while durable architectural decisions must be integrated directly into maintained product documentation.

---

## 2. Documentation Classification Framework

Every Markdown document in or related to Engram belongs to one of four discrete categories:

### Category A: Permanent Project Documentation (Eligible for Git)
These documents represent maintained, living technical references required by users, contributors, and developers.

Eligible Category A documents include:
- **Project Gateway:** `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `LICENSE`.
- **System Architecture:** `docs/architecture.md`, `docs/change-contracts.md`, `docs/change-guard.md`.
- **Security & Privacy:** `docs/security-and-privacy.md`.
- **Guides & Walkthroughs:** `docs/getting-started.md`, `docs/installation.md`, `docs/agent-integration.md`, `docs/troubleshooting.md`.
- **Reference & Matrix:** `docs/supported-languages.md`, `docs/benchmarks.md`, `docs/governance/KNOWN_LIMITATIONS_REGISTER.md`.
- **Approved Release Documentation:** Formal approved release notes (e.g. `docs/v1_5_0_release_notes.md`).
- **Maintained Tooling & Extensions:** `benchmarks/matched-pair-harness/README.md`, `extensions/vscode/README.md`.
- **Contribution Templates & Fixtures:** `.github/ISSUE_TEMPLATE/*.md`, maintained regression test fixtures (`tests/fixtures/**/*.md`).

**Quality Criteria for Category A:**
- Must be actively maintained and updated when code behavior changes.
- Must be accurate, reproducible, and free of obsolete transient claims.
- Must use standard GitHub-compatible paths and markdown formatting.

### Category B: Local-Only Engineering Evidence (Default Local)
These documents are created to verify intermediate milestones, record phase results, or provide evidence during development sessions. They **must remain local** and must not be committed to Git branches.

Category B documents include:
- Phase execution reports (`PHASE_*_EXECUTION_REPORT.md`, `docs/phases/phase_*_report.md`).
- Audit and remediation reports (`PHASE_*_AUDIT_REPORT.md`, `phase_*_remediation_report.md`).
- Intermediate certification and readiness memos (`PHASE_*_CERTIFICATION_REPORT.md`, `next_release_readiness.md`).
- Task plans, scratch checklists, and debugging logs.
- Draft amendments and working papers (`amendments/draft-*`).

**Handling Rule for Category B:**
- Deliver findings to the user/reviewer in the session context or local reports.
- If a report uncovers a permanent architectural rule or bug fix, extract that distilled decision into the appropriate Category A document (e.g. `docs/architecture.md`).
- Do not commit the execution report itself.

### Category C: Raw Evidence & Machine Artifacts (Strictly Local)
Machine-generated datasets and raw captures must never be committed to Git:
- Raw benchmark execution outputs and JSON summaries (`*.raw_results.json`).
- Profiler traces, coverage dumps, and heap snapshots.
- Machine-specific path dumps and environment variable listings.
- Scratch scripts and temporary benchmark runs.

### Category D: Needs Explicit Review
Documents that do not clearly fit Category A or B (e.g. strategic multi-quarter roadmaps, candidate proposal RFCs) require explicit human review and approval before staging. Default to keeping them local until approved.

---

## 3. Staging and Commit Hygiene Protocol

Automated agents and human contributors must strictly observe the following protocol before creating any Git commit:

1. **Explicit File Review:** Run `git status` and inspect every modified and untracked file individually.
2. **Strict Prohibition of Broad Adds:** Never execute `git add .`, `git add -A`, or wildcard adds (`git add *.md`).
3. **Category Check:** Verify that every file to be staged belongs exclusively to Category A or has explicit human authorization.
4. **Local Evidence Exclusion:** Ensure that local phase reports, test scratch files, and raw data are excluded via `.git/info/exclude`.
5. **Pre-Commit Verification:** Run `git diff --cached --stat` to verify that no transient `.md` files or raw outputs are staged.

---

## 4. Local-Only Exclusion Practices

To prevent accidental staging of local engineering evidence without modifying shared repository `.gitignore` policies:

- **Use `.git/info/exclude`:** Add local-only patterns and file paths to `.git/info/exclude`. This ensures personal working documents, scratch files, and local inventories remain ignored by Git without dirtying the tracked `.gitignore`.
- **Standard Exclusions in `.git/info/exclude`:**
  ```text
  # Local-only engineering reports and scratch artifacts
  docs_hygiene_local_inventory.md
  scratch/
  PHASE_*.md
  docs/phases/phase_*.md
  ```

---

## 5. Remote Push & Release Invariant

1. **Zero Unauthorized Pushes:** No Git commit may be pushed to a remote repository branch (`git push`) without explicit, unambiguous human review and authorization.
2. **Zero Automated Release Operations:** Tag creation (`git tag`), GitHub release creation, or publication of distribution binaries must only be performed upon formal release approval.
3. **Emergency History Correction:** If an unauthorized commit is pushed, it must be rolled back immediately using a safe, lease-guarded history restoration (`--force-with-lease` specifying the exact remote SHA) while preserving local work in a dedicated recovery branch.

---

## 6. Summary Matrix

| Document Type | Examples | Default Location | Git Tracking Status |
|---|---|:---:|:---:|
| **Maintained User Docs** | `README.md`, `docs/installation.md` | `docs/` | **Tracked (Category A)** |
| **System Architecture** | `docs/architecture.md`, `docs/change-contracts.md` | `docs/` | **Tracked (Category A)** |
| **Security & Privacy** | `docs/security-and-privacy.md` | `docs/` | **Tracked (Category A)** |
| **Issue Templates** | `.github/ISSUE_TEMPLATE/*.md` | `.github/` | **Tracked (Category A)** |
| **Phase Reports** | `docs/phases/phase_9_0_1_report.md` | Local disk | **Local-Only (Category B)** |
| **Audit & Remediation** | `PHASE_8_2_1_REMEDIATION.md` | Local disk | **Local-Only (Category B)** |
| **Raw Telemetry & JSON** | `phase_8_1_1_2_raw_results.json` | Local disk | **Local-Only (Category C)** |
| **Local Inventory** | `docs_hygiene_local_inventory.md` | `.git/info/exclude` | **Local-Only (Category C)** |
