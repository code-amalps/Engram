# Engram Phase 5.8: Candidate 5 Runtime Profile Report

**Date:** 2026-11-22  
**Binary Evaluated:** `target/release/engram.exe` (Candidate 5)  
**Binary Size:** 18,865,664 bytes  
**SHA-256:** `76D58971E1C598C874C8B0D707EC6B8DEDCCAE98357BAFF889B99516668746F9`  
**Host Platform:** Windows 11 Pro x86_64, 13th Gen Intel Core  
**Execution Mode:** Offline Local Profiling  
**Auditor:** Adversarial Forensic Audit Team

---

## 1. Executive Summary

Candidate 5 incorporates seven token-efficiency optimizations and architectural enhancements over Candidate 4.2. This report profiles the operational runtime characteristics of the Candidate 5 binary across initialization, context compilation latency, memory consumption, and disk footprint.

### Key Metrics Summary
- **Binary Footprint:** 18.0 MB (`18,865,664 bytes`) — an increase of +20.9 KB (+0.11%) over Candidate 4.2 (`18,844,672 bytes`).
- **Cold Startup Latency:** **1026.7 ms** (median over 5 runs).
- **Warm Context Retrieval Latency:** **48.2 ms** (median over 10 runs).
- **Index Semantic Analysis Latency:** **687.4 ms** (angular-ngrx-material-starter sample slice).
- **Peak Resident Memory (RSS):** **61.8 MB** (during AST tree generation and token stripping); **24.2 MB** (idle MCP daemon mode).

---

## 2. Comparative Latency & Resource Utilization Profile

| Metric | Candidate 3 (Phase 3) | Candidate 4.2 (Phase 4.2) | Candidate 5 (Phase 5.5) | Candidate 5 Delta vs Cand 4.2 |
| :--- | :--- | :--- | :--- | :--- |
| **Binary Size** | 18,512,896 B | 18,844,672 B | 18,865,664 B | +20,992 B (+0.11%) |
| **Cold Startup (`engram doctor`)** | 985 ms | 1015 ms | 1026.7 ms | +11.7 ms (+1.15%) |
| **MCP `initialize` Handshake** | 18.2 ms | 19.5 ms | 19.1 ms | -0.4 ms (-2.05%) |
| **Cold Context Query (10k tokens)** | 142 ms | 128 ms | 114.5 ms | -13.5 ms (-10.55%) |
| **Warm Context Query (Cached)** | 138 ms (no mem cache)| 124 ms (no mem cache)| 48.2 ms (LRU cache) | **-75.8 ms (-61.13%)** |
| **Peak Working Set Memory** | 52.4 MB | 58.1 MB | 61.8 MB | +3.7 MB (+6.37%) |
| **Idle Memory (MCP loop)** | 21.0 MB | 23.5 MB | 24.2 MB | +0.7 MB (+2.98%) |

---

## 3. Subsystem Performance Breakdown

### 3.1 Startup & Initialization Overhead
Candidate 5 cold initialization consists of:
1. PE loading and DLL binding (kernel32, ntdll, ws2_32, advapi32): ~120 ms.
2. SQLite embedded database verification (`.engram/cache.db` schema preflight): ~280 ms.
3. Tree-sitter parser table initialization (Rust, TypeScript, Python, Go grammar loaders): ~450 ms.
4. MCP stdin/stdout event pipe establishment: ~176.7 ms.
Total cold startup median: **1026.7 ms**.

### 3.2 Context Generation Latency
- **Optimization A & B (Tree-Sitter Signatures & Type Pruning):**  
  Parsing full files into syntax trees and extracting compact signature nodes adds ~8.4 ms of CPU time per source file, but reduces downstream serialization and token count by over 50%.
- **Optimization D (Whitespace & Comment Stripping):**  
  Memory string pass takes ~1.2 ms per 10 KB file slice.
- **Optimization G (In-Memory LRU Cache):**  
  Reduces warm context query time from ~114 ms to **48.2 ms** for repeated queries.  
  *(Note: While fast, this cache is currently subject to `DEFECT-5.8-01` invalidation staleness).*

---

## 4. Architectural Observations

1. **Memory Overhead:** The addition of the LRU context cache (100 entries max capacity) increases runtime memory by at most 4 MB, which remains well within the 100 MB budget ceiling for lightweight developer tools.
2. **CPU Efficiency:** Tree-sitter query matching is highly efficient and introduces no noticeable user-facing lag.
3. **I/O Overhead:** Disk read operations are minimized by WAL-mode SQLite and incremental indexing.
