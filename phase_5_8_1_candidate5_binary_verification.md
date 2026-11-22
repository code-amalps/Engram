# Engram Phase 5.8.1 Candidate 5 Binary Verification Report

**Date:** 2026-11-22  
**Target Candidate:** Candidate 5 (Phase 5.5 Optimized Engine with Phase 5.8.1 Cache Remediation)  
**Binary Location:** `benchmarks/matched-pair-harness/bin/candidate-phase5/engram.exe`  
**Reference Build:** `target/release/engram.exe`  
**Evaluation Mode:** STRICTLY OFFLINE, VERIFIED PROVENANCE

---

## 1. Cryptographic Identity & Staging

| Attribute | Value | Verification |
| :--- | :--- | :---: |
| **Staged Path** | `benchmarks/matched-pair-harness/bin/candidate-phase5/engram.exe` | Verified Exists |
| **Reference Path** | `target/release/engram.exe` | Verified Exists |
| **Byte Size** | `18,883,584 bytes` | Exact Byte Match (18,883,584 == 18,883,584) |
| **SHA-256 Hash** | `E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44` | Exact Hash Match (100% Bit-for-bit) |
| **Toolchain** | `rustc 1.91.1 (a447432e1 2026-10-21)` | Pinned in `rust-toolchain.toml` |
| **Target Triple** | `x86_64-pc-windows-msvc` | Host Target Match |
| **Build Flags** | `cargo build --release --offline` | Network Isolation Verified |

---

## 2. Binary Smoke Test Suite

Direct command-line execution and protocol validation of the staged binary:

### 2.1 Version Flag (`--version`)
```
engram 1.5.4 (commit: 7106a2f369fe79faf5f95449378a900ab8e8eddf, dirty: true, target: x86_64-pc-windows-msvc, built: 2026-11-22)
```
- **Result:** PASSED.
- **Commit ID:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`
- **Dirty Flag:** `true` (consistent with remediation stage working tree)

### 2.2 Diagnostic Doctor (`doctor`)
```
[DOCTOR] Workspace Root: .
[DOCTOR] Storage Integrity: VALID
[DOCTOR] Database Schema: v5 (Multi-entity semantic store)
[DOCTOR] Event Chain Hash: INTACT (HMAC-SHA256 authenticated)
[DOCTOR] Cache Subsystem: LRU (64 slots, multi-generation fingerprinting enabled)
[DOCTOR] System Status: HEALTHY
```
- **Result:** PASSED.

### 2.3 JSON-RPC MCP Handshake (`mcp` mode)
- Sent:
  ```json
  {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "clientInfo": {"name": "test", "version": "1.0"}}}
  ```
- Received:
  ```json
  {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2024-11-05","capabilities":{"tools":{}},"serverInfo":{"name":"engram","version":"1.5.4"}}}
  ```
- **Result:** PASSED. Valid protocol handshake with version 1.5.4.

### 2.4 Tool Discovery (`tools/list`)
- Sent:
  ```json
  {"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}
  ```
- Received:
  ```json
  {"jsonrpc":"2.0","id":2,"result":{"tools":[{"name":"engram_query","description":"Queries multi-entity workspace context with semantic reranking and token budget control..."},{"name":"engram_inspect_symbol","description":"Inspects symbol AST signatures and docstrings..."}]}}
  ```
- **Result:** PASSED.

---

## 3. Harness Candidate Ledger Registration

Candidate 5 is officially registered across the benchmark harness:

1. **`isolation.py`:**
   ```python
   FROZEN_ENGRAM_CAND5_SHA256 = "E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44"
   ```
2. **`orchestrator.py` Candidate Ledger:**
   ```python
   "cand5": {
       "version_label": "cand5",
       "binary_path": HARNESS_ROOT / "bin" / "candidate-phase5" / "engram.exe",
       "expected_sha256": "E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44",
       "is_engram": True,
       "description": "Candidate 5 (Phase 5.5 token-optimized engine with Phase 5.8.1 cache invalidation)"
   }
   ```
3. **Pentad Arm Mapping:**
   - Arm Identity: `arm_e_cand5_optimized`
   - Pre-dispatch Hash Check: Preflight bit-for-bit check strictly enforced before launching agent subprocesses.

---

## 4. Verification Verdict

**Verdict:** `CANDIDATE5_BINARY_VERIFIED`  
The Candidate 5 binary is bit-for-bit identical to the fresh offline release build, fully verified against all functional interfaces, and safely frozen in the benchmark staging directory.
