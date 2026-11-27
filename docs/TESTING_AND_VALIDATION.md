# Testing, Verification & Historical Checksums

Engram adheres to rigorous verification and testing standards to ensure stability, determinism, and security across all releases.

---

## 1. Test Suite Architecture

The Engram codebase maintains an extensive automated test suite comprising **800 discrete `#[test]` functions** across **84 files**:

### Unit Test Modules (`src/`)
11 internal modules declare **41 unit test functions**:
- `src/main.rs`: 11 tests (CLI argument parsing, secret redaction, path resolution).
- `src/budget.rs`: 5 tests (token budget slicing, priority truncation).
- `src/compiler.rs`: 5 tests (context formatting, boundary armor tags).
- `src/java_parser.rs`: 5 tests (Java class, interface, and method AST parsing).
- `src/go_parser.rs`: 3 tests (Go structural interface matching, package parsing).
- `src/frameworks/mod.rs`: 3 tests (framework detection).
- `src/hook.rs`: 3 tests (pre-commit hook validation).
- `src/instructions.rs`: 2 tests (instruction file injection).
- `src/storage.rs`: 2 tests (SQLite WAL schema migrations, transaction integrity).
- `src/bench.rs`: 1 test (benchmark runner harness).
- `src/impact.rs`: 1 test (blast radius impact scoring).

### Integration Test Suites (`tests/`)
**73 integration test files** declare **759 integration tests**:
- `tests/empirical_live_test.rs`: 99 tests (live-agent context and token measurement).
- `tests/hook_precommit_test.rs`: 25 tests (Git hook boundary enforcement).
- `tests/impact_test.rs`: 24 tests (multi-hop blast radius calculations).
- `tests/context_budget_test.rs`: 20 tests (budget prioritization and slicing).
- `tests/change_guard_test.rs`: 18 tests (Git diff boundary evaluation).
- `tests/recovery_test.rs`: 18 tests (max-2 retry recovery engine).
- `tests/mcp_baseline_test.rs`: 27 tests (JSON-RPC stdio protocol compliance).
- `tests/ast_extraction_test.rs`: 13 tests (multi-language AST symbol extraction).
- `tests/cache_invalidation_test.rs`: 13 tests (two-tier mtime + hash invalidation).
- `tests/knowledge_graph_test.rs`: 13 tests (Schema 2.0.0 dependency graph traversal).
- `tests/storage_migration_test.rs`: 11 tests (SQLite WAL migrations).
- *... plus 62 additional specialized integration test suites.*

### Historical Full Quality Gate
In Phase 10.1 (commit `b5c63d6...`), `cargo test --locked --no-fail-fast` executed across the entire repository:
```text
test result: ok. 73 suites, 816 passed, 0 failed, 0 ignored, 0 measured, 0 filtered out
```
*(The difference between 800 function declarations and 816 executed test instances reflects doc-tests and parameterized test instances.)*

---

## 2. Fresh-Environment Acceptance Protocol (Phase 10.2.2.6)

Before authorizing release publication, Engram undergoes full smoke testing in an isolated, temporary environment without pre-existing state:

| Protocol Step | Test Action | Verification Target | Result |
|:---:|---|---|:---:|
| 1 | Archive Extraction | Clean extraction without permission or path errors | **PASS** |
| 2 | Checksum Verification | Binary matches authoritative release SHA-256 (`BB0AAA...`) | **PASS** |
| 3 | Version Command | `engram --version` outputs correct version string | **PASS** |
| 4 | Help Command | `engram --help` lists all canonical subcommands | **PASS** |
| 5 | Workspace Init | `engram init` generates `.engram/`, schema, and config | **PASS** |
| 6 | Repository Indexing | `engram analyze` indexes test repo without errors | **PASS** |
| 7 | Context Query | `engram context` emits structured, budgeted AST context | **PASS** |
| 8 | Workspace Inspection | `engram inspect <TARGET>` confirms symbol blast radius & traversal | **PASS** |
| 9 | Health Doctor | `engram doctor` passes all environmental checks | **PASS** |
| 10 | MCP stdio Handshake | Launches MCP server, queries `tools/list` (17 tools discovered) | **PASS** |

---

## 3. Historical Release Checksum Registry

The following table records the authoritative SHA-256 checksums and verification status of all Engram release archives:

| Release Tag / Archive | Size (Bytes) | SHA-256 Checksum | Release Status |
|---|:---:|---|:---:|
| `engram-preview-v1.5.3-cand5-windows-x64.zip` | 4,503,445 | `3A04FD53AB1D2ED6716B186A2DF28A301C61CAB538CC422547ED4A2AA1C6A9EF` | Archived Historical |
| `engram-preview-v1.5.4-cand5-clean-windows-x64.zip` | 4,303,797 | `CDD34117E217C44DE0C0F2BD860C04A64350EAEF500A4FC1B39FCABE9B85FF2E` | Archived Historical |
| `engram-preview-v1.5.4-cand5-windows-x64.zip` | 4,502,442 | `7C355109EEB8BE7B08301080FD660DE4A4A23DD12AFE89F008546C32FB3C1741` | Archived Historical |
| `engram-preview-v1.5.5-cand1-windows-x64.zip` | 4,422,441 | `DD67AB9FF391182373E3AD17B2C40929206E13F33E13D4A2E77BEC0596A730BF` | Archived Historical |
| `engram-preview-v1.5.5-cand2-windows-x64.zip` | 4,811,989 | `7D52CB0C7627CBEF58BC76FE8ECDBEE7B9B7035869A22B3F1795252FFB4C5EB0` | Archived Historical |
| `engram-preview-v1.5.5-cand3-windows-x64.zip` | 4,814,726 | `ED05F3B2F26D16D9838033620023A0385281E0536446D6B81814B2CBBC04B9D3` | Archived Historical |
| `engram-preview-v1.5.5-cand4-windows-x64.zip` | 4,815,568 | `FCA714D90B0A4F63F31089C031AD099A9763BE1546A9E377D094B2FEBDDAE97E` | Archived Historical |
| `engram-preview-v1.5.5-cand5-windows-x64.zip` | 4,816,022 | `D7016554143B4A447D4B02E836EB76032DB89B160C86B605427243003A24AC54` | Archived Historical |
| `engram-preview-v1.5.5-cand6-windows-x64.zip` | 4,833,126 | `566F3A467A9FCB4F9CEB3E7D6B46A4D8A3FA849FCB10BD697B312EC43EBBFF8B` | Archived Historical |
| `engram-preview-v1.5.5-rc1-windows-x64.zip` | 5,064,220 | `E5152F68803A7759343FC0A60C2A499539E27173EDD7C53C631AAAC2C160D2C0` | Superseded Candidate |
| **`engram-preview-v1.5.5-rc2-windows-x64.zip`** | **5,064,678** | **`92B4FE6F7358920F3650782EB43BE70D4A64D7927D25A2C26E412741CAC3751D`** | **Authoritative Published Release** |

---

## 4. Packaged Executable Verification

The packaged Windows executable bundled within `engram-preview-v1.5.5-rc2-windows-x64.zip` has been verified with 100% byte-for-byte identity:

- **Path:** `bin/engram.exe`
- **File Size:** `20,685,824` bytes
- **SHA-256:** `BB0AAA66A2F3ADEE3C8F2D5B91C3DA63FF041FEB98CFE08B1DD27B45F858D087`
- **Build Commit:** `b5c63d606e8c1bf89e9b89278f36d7eb1b02ea61`
- **Source Tree:** `2ad4e331ec4e8507d784fd8992e78358bee69277`
