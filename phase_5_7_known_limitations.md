# Phase 5.7: Known Limitations & Technical Constraints

**Document ID:** `ENGRAM-PHASE-5.7-KNOWN-LIMITATIONS`  
**Date:** 2026-11-20  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Target Release:** `1.5.4` (Public Alpha Candidate)

---

## 1. Operational & Governance Limitations

### 1.1 Pending Authorization Checkpoints
The current release candidate is strictly confined to local offline evaluation. The following capabilities remain formally blocked until explicit human operator authorization:
1. **Checkpoint A (Online Security Audit):** Live advisory database scanning (`cargo audit` online) is `NOT_AUTHORIZED`.
2. **Checkpoint B (Live Model Benchmarking):** Live API calls to LLM providers (OpenAI, Anthropic, Gemini) are `NOT_AUTHORIZED`.
3. **Checkpoint C (Release Distribution):** Git commit, push, tagging (`v1.5.4`), and binary publishing are `NOT_AUTHORIZED`.

### 1.2 Working Tree Baseline
- The release candidate binary was compiled from a working tree containing **13 tracked modified files** representing the cumulative fixes of Phases 5.4.31 to 5.7.
- A final clean release baseline commit has not yet been minted.

---

## 2. Technical & Architectural Limitations

### 2.1 Language Grammar Coverage
- **Deep AST Extraction Supported:** Rust, Python, JavaScript, TypeScript, Go.
- **Fallback Languages:** C/C++, Java, Ruby, PHP, C#, Swift, Kotlin, and other languages currently fall back to line-based text indexing and declaration heuristic extraction. Full tree-sitter grammars for these languages are scheduled for Engram v1.6.0.

### 2.2 Memory Storage & Concurrency
- Architectural memory (`.engram/memory.yaml`) and the index cache (`.engram/cache.db`) are designed for single-developer workstations.
- While process-level advisory locks prevent corruption across concurrent agent turns, high-concurrency multi-agent swarms editing `memory.yaml` simultaneously may experience write-lock contention (500 ms backoff).
- Distributed or remote database backends (PostgreSQL, Redis) are not supported in the 1.5.x series.

### 2.3 Compilation LRU Cache Capacity
- The in-memory context compilation cache (`COMPILED_CONTEXT_CACHE`) is bounded to **32 entries**.
- In workflows with dozens of diverse, rapid-fire queries against different subpaths, cache churn will occur, causing re-compilation of AST excerpts (though SQLite index queries remain fast at <15 ms).
- The cache resides in process memory and is invalidated upon process restart or any mutation to `.engram/memory.yaml`.

### 2.4 File Size & Binary Limits
- Source files exceeding **2 MB** are automatically bypassed from deep AST symbol extraction to avoid excessive memory allocation in the Tree-sitter C runtime.
- Minified JavaScript files and large bundled JSON fixtures should be excluded via `.gitignore` to maintain optimal indexing performance.
