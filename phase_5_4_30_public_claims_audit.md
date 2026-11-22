# Engram Phase 5.4.30: Public Documentation & Marketing Claims Audit

**Phase:** 5.4.30  
**Audit Scope:** `README.md`, `CHANGELOG.md`, `installer/engram.iss`, Documentation Claims, Technical Accuracy  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  

---

## 1. Executive Summary

This forensic audit evaluated all public-facing claims, marketing text, architectural descriptions, and technical statements in the Engram repository against the verified capabilities of the codebase.

### Primary Audit Finding (F-5430-02):
`README.md` lines 314–315 state:
> *"Droid queries only the exact functions and types it needs, cutting token usage by over 80%."*

**Verdict: UNVERIFIED / DISQUALIFYING FOR UNCONDITIONAL PUBLIC ALPHA.**  
Formal empirical verification via live multi-arm matched-pair benchmark suites (Task B Live Tetrad) was halted in Phase 5.4.22 due to budget safeguards and remains `NOT_AUTHORIZED`. While synthetic unit test traces demonstrate context reduction, claiming a definitive *"cutting token usage by over 80%"* in unconditional public documentation without published empirical proof violates release-gate accuracy standards.

Engram is therefore certified as **`PUBLIC_ALPHA_READY_WITH_EXPLICIT_LIMITATIONS`**, with mandatory inclusion of an efficiency disclaimer.

---

## 2. Statement-by-Statement Claim Verification

| Source File & Location | Claim / Statement | Technical Verification | Classification |
|---|---|---|---|
| `README.md` (Header) | *"Local engineering memory and change intelligence for AI coding agents"* | Core engine implements SQLite WAL memory, AST symbol tracking, and change contract validation. Verified via `engram.exe --help` and `engram analyze`. | **VERIFIED** |
| `README.md` (Header) | *"Lightweight, zero-config CLI and MCP server built in Rust"* | Single static binary (18.8 MB), no external runtime dependencies, stdio JSON-RPC MCP server. Verified via smoke test. | **VERIFIED** |
| `README.md` (Features) | *"AST-Grounded Symbol Graph (Rust, Python, TS/JS, Go, Java, C#)"* | Tree-sitter parsers for all 6 language families are statically linked in `Cargo.lock` and `src/impact.rs`. Verified. | **VERIFIED** |
| `README.md` (Features) | *"Git-Aware Historical Co-Change Mining"* | `src/impact.rs` mines historical commit histories to discover files that co-change with touched symbols. Verified. | **VERIFIED** |
| `README.md` (Features) | *"Local-First & Zero-Exfiltration (SQLite WAL, stdio MCP)"* | All data is stored in `%LOCALAPPDATA%\Engram\cache` or local `.engram/` folder. Network access is zero. Verified. | **VERIFIED** |
| `README.md` (Lines 314–315) | *"Droid queries only the exact functions and types it needs, cutting token usage by over 80%."* | While AST context filtering scopes symbols, live multi-arm benchmark execution was halted in Phase 5.4.22 and remains unexecuted under offline safety constraints. Unqualified quantitative claim. | **UNVERIFIED / REQUIRES DISCLAIMER** |
| `README.md` (Integrations) | *"Native MCP integration with Cursor, VS Code, Windsurf, Factory AI (Droid)"* | `engram setup cursor`, `vscode`, `windsurf`, `droid` write respective configuration schemas. Verified. | **VERIFIED** |
| `CHANGELOG.md` (v1.5.4) | *"Hardened MCP protocol boundary to 16,384 bytes with progressive degradation ladder"* | Implemented in `src/mcp.rs` lines 450–500 (`enforce_mcp_envelope_boundary`, Levels 0–7). Verified. | **VERIFIED** |
| `CHANGELOG.md` (v1.5.4) | *"Eliminated SQLite lock contention by applying 15,000 ms busy timeout across all connection paths"* | Configured in `src/storage.rs` in `db()` and `db_at()`. Verified. | **VERIFIED** |
| `CHANGELOG.md` (v1.5.4) | *"Implemented safe UTF-8 character boundary string truncation"* | Implemented in `src/mcp.rs` (`safe_truncate_str`). Verified. | **VERIFIED** |
| `installer/engram.iss` | *"AppVersion=1.5.4, AppPublisher=Engram Engineering"* | Metadata matches `Cargo.toml` version `1.5.4`. Verified. | **VERIFIED** |

---

## 3. Disqualification Analysis for Unconditional Alpha

Why does the 80% claim prevent an unconditional `PUBLIC_ALPHA_READY` declaration?
1. **Engineering Integrity:** High-assurance engineering releases must not publish definitive empirical performance claims unless supported by reproducible, inspectable benchmark traces.
2. **Current Benchmark State:**
   - In Phase 5.4.22, live benchmark execution was blocked (`MANIFEST_BLOCKED`) due to budget overshoot hazards.
   - In Phase 5.4.23 to 5.4.26, safety harnesses and cancellation guards were verified offline using mock and simulated runners.
   - Live authorization for Task B Live Tetrad remains `NOT_AUTHORIZED`.
3. **Regulatory and Community Trust:** If developers deploy Engram expecting a guaranteed 80% reduction across all workflows, discrepancies on varied codebases will degrade project credibility.

---

## 4. Remediation Action Plan

Prior to removing the `WITH_EXPLICIT_LIMITATIONS` qualifier, the repository maintainers must perform one of the following two actions:

### Option A: Qualify the Claim in `README.md` (Recommended for v1.5.4 Alpha)
Update lines 314–315 of `README.md` to read:
```markdown
Droid queries only the exact functions and types it needs, designed to reduce context token consumption by up to 80% on large multi-module repositories (empirical matched-pair validation currently in progress).
```

### Option B: Complete Live Empirical Benchmarking (Post-Alpha / Future Phase)
Authorize network and model API credentials to execute the hardened Phase 5.4.23 Task B Tetrad harness (`A -> D -> C -> B`) to empirical completion, publishing the certified results in `docs/benchmarks/`.
