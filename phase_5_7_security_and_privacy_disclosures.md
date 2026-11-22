# Phase 5.7: Security, Privacy & Boundary Disclosures

**Document ID:** `ENGRAM-PHASE-5.7-SECURITY-DISCLOSURES`  
**Date:** 2026-11-20  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Target Release:** `1.5.4` (Public Alpha Candidate)

---

## 1. Security Architecture & Threat Model

Engram is architected as an offline, zero-trust local engine for coding agents. Its primary responsibility is deterministic context compilation from local source repositories without external telemetry, network dependencies, or arbitrary code execution.

### 1.1 Local Boundary & Network Isolation
- **Zero Outbound Telemetry:** Engram contains **no network client code** in its runtime paths. It does not phone home, report analytics, or communicate with external servers.
- **Strictly Offline MCP Server:** The Model Context Protocol (`engram mcp`) communicates exclusively across local standard input/output (`stdio`). No TCP or HTTP sockets are opened by default.
- **Offline Dependency Stance:** Offline compilation (`--offline`) is fully supported and verified; the release candidate binary requires no network access to operate.

---

## 2. Data Privacy & Credential Protection

### 2.1 Workspace Data Storage
Engram stores all persistent state within the repository's `.engram/` folder:
- `.engram/cache.db`: SQLite database storing AST symbol indices, file modification times, and pre-computed token hashes.
- `.engram/memory.yaml`: Human-readable YAML document recording architectural decisions.

### 2.2 Credential & Secret Exclusion
- Engram does **not** index or store environment files (`.env*`), private keys (`*.pem`, `*.key`), secret tokens, or git configuration files.
- Default ignore rules inherit `.gitignore` patterns and automatically skip common secret locations (`.git/`, `node_modules/`, `target/`, `venv/`).

---

## 3. Boundary Defenses & Vulnerability Mitigations

### 3.1 Path Traversal Protections
- All file queries and subpath arguments passed to `engram_get_context` or `engram_index_repository` undergo strict path canonicalization.
- Requests attempting to traverse outside the repository boundary (e.g., `../../etc/passwd` or `..\..\Windows\System32`) are rejected with an explicit error before filesystem reads occur.

### 3.2 Parser Memory Safety
- Tree-sitter parsers execute as compiled C grammars in-process.
- Input files exceeding 2 MB are skipped from deep AST parsing to prevent denial-of-service or memory exhaustion on large binary or generated files.

### 3.3 Safe UTF-8 Truncation & Token Budgeting
- The context compiler enforces a strict 16 KB degradation ladder (Levels 0 through 7) in `src/budget.rs`.
- Buffer truncations are executed using character-boundary validation (`char_indices`), preventing UTF-8 multi-byte split panics or malformed JSON payloads.

---

## 4. Software Licensing & Copyleft Disclosure

### 4.1 Primary License: AGPL-3.0-or-Later
Engram is licensed under the **GNU Affero General Public License v3.0 or later (AGPL-3.0-or-later)**:
- Users have the right to inspect, modify, and recompile the complete source code.
- If Engram is run as a network service, the full corresponding source code must be made available to network users.

### 4.2 Third-Party Permissive Dependencies
- All 114 external dependency instances compiled into the release binary are licensed under permissive open-source licenses (MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, Unlicense).
- Zero GPL/AGPL copyleft dependencies are linked; the AGPL license applies solely to the Engram codebase itself.
- A comprehensive `THIRD_PARTY_LICENSES.md` file will accompany all binary distributions upon publication.

---

## 5. Security Checkpoint Governance

- **Checkpoint A (Online Vulnerability Audit):**
  - **Status:** `NOT_AUTHORIZED`
  - *Notice:* The current release candidate has been audited against local offline criteria. Online automated scanning against live advisory databases (e.g., `cargo audit`) is staged in `phase_5_4_32_online_audit_authorization_request.md` and awaits explicit human authorization before public distribution.
