# Engram Amendment 05 — Milestone 4: Implementation Plan
## Subpath- and Language-Scoped Query Routing

**Document Version:** 1.0.0  
**Date:** 2026-10-30  
**Phase:** Phase 1 — Self-Hosted Implementation Preparation  
**Status:** Approved for Implementation

---

## 1. Objective & Architecture Overview

The objective of Milestone 4 is to implement optional subpath- and language-scoped query filtering in Engram's context compiler (`src/compiler.rs`), prompt generator (`src/prompt.rs`), and CLI entry points (`src/main.rs`).

This feature resolves the key retrieval precision bottlenecks identified during Milestone 3:
1. **Cross-Language Lexical Competition:** Prevents Rust source files and Python harness files from competing for the same BM25 lexical score.
2. **Test-Mock Crowding:** Prevents large integration test suites and test mocks from displacing core implementation files during token-budget allocation.
3. **Small-Budget Truncation:** Ensures that under compact context budgets (e.g. 1,200 tokens), 100% of the token allocation is reserved for files within the targeted component subtree.

---

## 2. Interface Specifications & CLI Design

### 2.1 CLI Flags
Two optional flags will be added to `engram context` and `engram prompt` in `src/main.rs`:

```text
--subpath <PATH>     Restrict candidate retrieval to files within the specified repository-relative subtree (e.g. 'src/', 'tests/', 'benchmarks/matched-pair-harness/').
--language <LANG>    Restrict candidate retrieval to files matching the specified canonical language or alias (e.g. 'rust', 'python', 'rs', 'py').
```

### 2.2 Shared Retrieval Engine Interface (`src/compiler.rs`)
To guarantee 100% backward compatibility across existing internal callers (`bench.rs`, `contract.rs`, `flywheel.rs`, `guard.rs`, `mcp.rs`), the existing signature:
```rust
pub fn context(root: &Path, task: &str, budget: usize) -> Result<ContextResult>
```
will be preserved as a wrapper calling:
```rust
pub fn context_scoped(
    root: &Path,
    task: &str,
    budget: usize,
    subpath: Option<&str>,
    language: Option<&str>,
) -> Result<ContextResult>
```

### 2.3 Prompt Generation Interface (`src/prompt.rs`)
`generate_prompt` and `prepare_task_context` will accept optional `subpath` and `language` parameters and forward them directly to `compiler::context_scoped`.

---

## 3. Detailed Technical Design

### 3.1 Subpath Normalization, Validation, and Safety
- **Path Separators:** Normalize Windows backslashes (`\`) to forward slashes (`/`).
- **Trailing Slashes:** Strip trailing slashes during prefix matching so `--subpath src` and `--subpath src/` behave identically.
- **Path Traversal Protection:** 
  - Reject absolute paths (e.g. `C:\...`, `/etc/...`).
  - Reject path traversal attempts containing `..` (e.g. `../outside`).
  - Safe error message: `Subpath filter cannot be absolute or contain parent directory references ('..')`.
- **Filtering Stage:** Filtering is applied **immediately after database retrieval** into `docs` before BM25 term frequency calculation:
  ```rust
  if let Some(sub) = subpath {
      docs.retain(|(path, _, _)| {
          path == sub || path.starts_with(&format!("{sub}/"))
      });
  }
  ```
  This ensures that excluded files never influence inverse document frequency (IDF) or consume graph traversal hops.

### 3.2 Language Canonicalization & Alias Mapping
Use Engram's canonical language identifiers while supporting standard industry aliases:
- `rust`, `rs` $\rightarrow$ `"rust"`
- `python`, `py` $\rightarrow$ `"python"`
- `typescript`, `ts` $\rightarrow$ `"typescript"`
- `javascript`, `js` $\rightarrow$ `"javascript"`
- `csharp`, `cs` $\rightarrow$ `"csharp"`
- `go` $\rightarrow$ `"go"`
- `java` $\rightarrow$ `"java"`

If an unsupported language is provided (e.g. `--language ruby`), the compiler immediately returns a clean error:
`Unsupported language filter 'ruby'. Supported languages: rust, python, typescript, javascript, csharp, go, java (and standard extensions rs, py, ts, js, cs).`

### 3.3 Source-versus-Test Weighting (Phase 5)
In [compiler.rs](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/compiler.rs#L125-L133), the current impact category weights assign `Test => 9.0` and `Implementation => 8.0`.
This will be corrected so that:
- `ImpactCategory::Direct => 12.0`
- `ImpactCategory::Implementation => 10.0`
- `ImpactCategory::Test => 7.0`
- `ImpactCategory::Caller => 6.0`
- `ImpactCategory::ImportDependent => 4.0`

This ensures that implementation modules take precedence over test fixtures while preserving test ranking when tests are explicitly targeted.

### 3.4 Empty Matching Behavior
If a valid subpath or language contains no files in the repository:
- `ContextResult` is returned with `files: []`, `estimated_tokens: 0`, and `confidence: "LOW"`.
- A descriptive gap is recorded:
  `GAP-002: No files matched the specified filter (subpath: '...', language: '...').`

---

## 4. Test Strategy

A dedicated offline test suite will be authored:
`tests/scoped_query_routing_test.rs`

Test Matrix:
1. **Subpath Tests:**
   - Filter `src/`: Verifies 100% of returned files begin with `src/`.
   - Filter `benchmarks/matched-pair-harness/`: Verifies Python harness files are retrieved.
   - Nested subpath: `src/empirical_live/`.
   - Traversal rejection: `../`, `..\\`.
   - Absolute path rejection: `C:\Windows`, `/usr/bin`.
2. **Language Tests:**
   - Canonical `rust` and alias `rs`.
   - Canonical `python` and alias `py`.
   - Unsupported language rejection (e.g. `haskell`).
3. **Combined Tests:**
   - `subpath: "src/", language: "rust"`: Exact intersection.
   - Conflicting filter: `subpath: "src/", language: "python"` $\rightarrow$ Empty result, 0 files, safe gap.
4. **Token Budget & Ranking:**
   - Verifies that with `--subpath src/`, `src/main.rs` and `src/compiler.rs` fit comfortably into a 1,200 token budget without being crowded out by test mocks.
5. **Backward Compatibility:**
   - Verifies that calling `context(&root, &task, budget)` without filters behaves identically to the pre-change baseline.

---

## 5. Scope & Preservation Boundaries

- **Specifically Authorized Source Files:**
  - `src/main.rs`
  - `src/compiler.rs`
  - `src/prompt.rs`
  - `tests/scoped_query_routing_test.rs`
- **Explicitly Excluded / Protected Files:**
  - `benchmarks/matched-pair-harness/bin/*` (Candidate Phase 1, frozen v1.5.1, v1.5.4).
  - Golden Angular repository (`C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter`).
  - Historical evidence files (`benchmarks/AMENDMENT*`, `benchmarks/STAGE1*`).
  - Completed Milestones 1, 1A, 2, 2A, 2B, 2C, 2D, and 3 deliverables.
