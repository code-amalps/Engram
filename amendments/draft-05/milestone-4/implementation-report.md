# Milestone 4 — Implementation Report: Subpath- and Language-Scoped Query Routing

## 1. Executive Summary

Milestone 4 implements optional repository subpath (`--subpath`) and programming language (`--language`) query filtering within Engram's context compiler (`src/compiler.rs`), prompt generator (`src/prompt.rs`), and command-line interface (`src/main.rs`), validated by an integration test suite (`tests/scoped_query_routing_test.rs`).

This implementation directly addresses the context retrieval and ranking bottlenecks uncovered during Milestone 3 self-hosting analysis:
1. **Cross-Language BM25 Lexical Competition**: Python benchmark harness files (`benchmarks/matched-pair-harness/`) previously competed with core Rust compiler files for identical engineering keywords (e.g., `tokens`, `budget`, `compiler`, `benchmark`).
2. **Test Mock Crowding**: Large test files and fixtures previously outranked implementation files due to an inverted weight hierarchy (`Test: 9.0` vs `Implementation: 8.0`).
3. **Small-Budget Omission**: Under token budgets of 1,200 tokens, core implementation files were frequently squeezed out by voluminous test fixtures and cross-language artifacts.
4. **Scope Control**: Developers and coding agents lacked a mechanism to focus context retrieval on a specific subsystem or language domain.

---

## 2. Architecture & Implementation Details

### 2.1 Compiler Subpath Normalization & Security Boundary (`src/compiler.rs`)
- **Function**: `pub fn normalize_subpath(raw: &str) -> Result<String>`
- **Normalization Semantics**:
  - Trims leading/trailing whitespace.
  - Normalizes Windows backslashes (`\`) to canonical forward slashes (`/`).
  - Strips leading and trailing slashes (`src/` $\rightarrow$ `src`).
- **Directory Traversal Protection**:
  - Rejects any subpath containing parent directory segments (`..`).
  - Rejects absolute paths (starting with `/` or containing Windows drive letter `:`).
  - Rejects empty strings and root path references (`""` or `"/"`).
- **Matching Rule**:
  - Matches candidate file path `path` if `path == norm || path.starts_with(&format!("{}/", norm))`.
  - Accurately targets both directory subtrees (e.g. `src/`, `benchmarks/matched-pair-harness/`) and discrete files (e.g. `src/compiler.rs`).

### 2.2 Compiler Language Canonicalization (`src/compiler.rs`)
- **Function**: `pub fn canonicalize_language(raw: &str) -> Result<&'static str>`
- **Supported Languages & Aliases**:
  - `rust`, `rs` $\rightarrow$ `"rust"`
  - `python`, `py` $\rightarrow$ `"python"`
  - `typescript`, `ts`, `tsx` $\rightarrow$ `"typescript"`
  - `javascript`, `js`, `jsx`, `mjs`, `cjs` $\rightarrow$ `"javascript"`
  - `csharp`, `cs`, `c#` $\rightarrow$ `"csharp"`
  - `go`, `golang` $\rightarrow$ `"go"`
  - `java` $\rightarrow$ `"java"`
- **Fail-Closed Validation**: Any unrecognized or unsupported language filter yields a descriptive `Err` specifying supported options.

### 2.3 Early-Stage Candidate Pruning
- Filtering occurs immediately upon reading records from SQLite `files` table before BM25 term frequency calculations, average document length calculations, and graph expansions:
  ```rust
  let mut s = conn.prepare("SELECT path, language, excerpt FROM files")?;
  for r in s.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))? {
      let (path, lang, excerpt): (String, String, String) = r?;
      if let Some(ref sub) = normalized_subpath {
          if !(path == *sub || path.starts_with(&format!("{}/", sub))) {
              continue;
          }
      }
      if let Some(target_lang) = canonical_lang {
          if lang != target_lang {
              continue;
          }
      }
      docs.push((path, lang, excerpt));
  }
  ```
- **Token Budget Preservation**: Non-matching candidate files are completely excluded from `doc_map`, ensuring they cannot be pulled in via 1-hop/2-hop AST graph traversal, impact cone expansion, or BM25 ranking. Excluded files never consume token headroom.

### 2.4 Impact Category Weight Inversion
- In `src/compiler.rs`, the impact weighting match block was adjusted:
  - `ImpactCategory::Direct`: 12.0 (unchanged)
  - `ImpactCategory::Implementation`: 10.0 (increased from 8.0)
  - `ImpactCategory::Test`: 7.0 (decreased from 9.0)
  - `ImpactCategory::Caller`: 6.0 (unchanged)
  - `ImpactCategory::ImportDependent`: 3.5 (unchanged)
  - `ImpactCategory::Configuration`: 3.0 (unchanged)
  - `ImpactCategory::Historical`: 2.5 (unchanged)
- **Result**: Core implementation code now deterministically outranks test mocks and fixtures when both match task keywords.

### 2.5 Gap Alerting (`GAP-002`)
- When user filters result in zero candidate files matching the scoped query, Engram outputs a structured gap signal:
  `GAP-002: No files matched the scope filter (subpath: <SUBPATH>, language: <LANG>) for task '<TASK>'.`
- Distinguishes scope mismatch (`GAP-002`) from general repository index absence (`GAP-001`).

### 2.6 ContextResult Structure & Backward Compatibility
- Extended `ContextResult` with non-breaking optional fields:
  ```rust
  #[serde(skip_serializing_if = "Option::is_none")]
  pub subpath_filter: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub language_filter: Option<String>,
  ```
- Existing entry point `pub fn context(root: &Path, task: &str, budget: usize) -> Result<ContextResult>` delegates to `context_scoped(root, task, budget, None, None)`, guaranteeing 100% backward compatibility for all existing callers across `bench.rs`, `contract.rs`, `flywheel.rs`, `guard.rs`, and `mcp.rs`.

### 2.7 Prompt Generation & Context Completeness (`src/prompt.rs`)
- Added `pub fn generate_prompt_scoped` and `pub fn prepare_task_context_scoped` accepting optional `subpath` and `language` parameters.
- Preserved existing wrappers `generate_prompt` and `prepare_task_context`.
- Added scope reflection under `## Context Completeness` (Markdown) and `=== CONTEXT COMPLETENESS ===` (Text):
  - `- **Subpath Scope:** \`<SUBPATH>\``
  - `- **Language Scope:** \`<LANG>\``

### 2.8 CLI Interface Integration (`src/main.rs`)
- Extended `Commands::Context` and `Commands::Prompt` with clap arguments:
  ```rust
  /// Restrict context retrieval to a specific repository subpath (e.g. 'src/' or 'benchmarks/')
  #[arg(long)]
  subpath: Option<String>,
  /// Restrict context retrieval to a specific programming language (e.g. 'rust' or 'python')
  #[arg(long)]
  language: Option<String>,
  ```
- Forwarded arguments to `context_scoped`, `generate_prompt_scoped`, and `prepare_task_context_scoped`.

---

## 3. Files Modified and Authored

| File | Action | Purpose |
| :--- | :---: | :--- |
| `src/compiler.rs` | Modified | Added `normalize_subpath`, `canonicalize_language`, `context_scoped`, inverted ranking weights, `GAP-002` gap detection, and unit tests. |
| `src/prompt.rs` | Modified | Added `generate_prompt_scoped`, `prepare_task_context_scoped`, and completeness scope rendering. |
| `src/main.rs` | Modified | Added `--subpath` and `--language` flags to `Context` and `Prompt` commands. |
| `src/printer.rs` | Modified | Rendered active scope filters in human-readable and explain terminal outputs. |
| `tests/scoped_query_routing_test.rs` | Created | Comprehensive 9-test integration test suite validating offline functionality and security boundaries. |

---

## 4. Verification Summary

1. **Compilation**: `cargo check --offline` passed with 0 errors and 0 warnings.
2. **Formatting**: `rustfmt --check` passed cleanly across all modified and authored files.
3. **Integration Test Suite**: `cargo test --offline --test scoped_query_routing_test` passed 9/9 tests in 14.17s.
4. **Compiler Inline Unit Tests**: `cargo test --offline --lib compiler::tests` passed 3/3 tests in 0.01s.
5. **Full Rust Test Suites**: `cargo test --offline` passed across all suites (including 99/99 in `empirical_live_test.rs` with `--test-threads=1`).
6. **Python Harness Tests**: `python -m unittest discover` passed 233/233 tests in 248.59s.
7. **Empirical Quality Comparison**: Tested 4 matrix combinations on Engram repository, confirming complete elimination of cross-language noise and test mock crowding under 1,200 token budget.
