# Engram Phase 5.4.3: UTF-8 Safety Remediation & Candidate 4.2 Integrity Gate Report

**Execution Date:** 2026-11-19  
**Branch:** `feature/phase2-stage5-remediation`  
**Git HEAD:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Report Document:** `PHASE_5_4_3_UTF8_SAFETY_AND_CANDIDATE_4_2_REPORT.md`  
**Sealed Binary:** Candidate 4.2 (`benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe`)

---

## 1. Executive Summary & Final Verdict

### Final Decision: `READY FOR TASK B PREFLIGHT RECHECK`

All remediation, adversarial boundary testing, Rust and Python regression test suites, binary provenance verification, and smoke-test gates have **PASSED** unconditionally.

Candidate 4.2 successfully remedies the UTF-8 character boundary panic defect discovered in Candidate 4.1 (`src/mcp.rs:860`) without modifying the response boundary, without converting byte limits to character counts, without introducing heap allocations, and without regressing any existing MCP response compaction tiers.

### Key Milestones Achieved:
1. **Root-Cause Defect Eliminated:** Slicing at raw byte index 1500 (`&raw_explanation[..1500]`) has been replaced with zero-allocation, character-boundary-aware slicing via `safe_truncate_str`.
2. **Comprehensive UTF-8 Audit:** Audited all string slicing occurrences across `src/`; identified and patched all 6 unsafe byte-slicing locations in `src/mcp.rs` (Level 2 explanation/summary compaction, Level 6 task compaction, and `engram_inspect_symbol` explanation & excerpt truncation).
3. **Adversarial UTF-8 Test Suite:** Created `tests/utf8_safety_test.rs` with 14 comprehensive tests covering ASCII at boundary, boundary exceeding, multibyte ending at boundary, multibyte crossing boundary, Unicode bullets, Unicode dashes, emoji, mixed text sweep, empty strings, repeated truncation, JSON validity, deterministic panic reproduction, and end-to-end `mcp_call` integration.
4. **100% Regression Pass Rate:**
   - **Rust Regression:** 96 passed, 0 failed across 7 test suites (14 in `utf8_safety_test`, 11 in `mcp_boundary_test`, 24 in `cargo test --lib`, 20 in `context_budget_test`, 13 in `inspect_symbol_budget_test`, 13 in `mcp_consolidation_test`, 1 in `mcp_test`).
   - **Python Harness:** 62 passed, 0 failed across all harness suites (`test_adaptive_activation.py`, `test_tetrad_harness.py`, `test_task_b_readiness_fixes.py`, `test_task_b_evaluator.py`, `test_triad.py`, `test_mcp_runtime_activation.py`).
5. **Sealed Candidate 4.2:** Built from clean source, verified SHA-256 `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`, confirmed previous Candidates 3, 4, and 4.1 hashes remain unmodified.
6. **Live Reproduction Proof:** Deterministically proved that `target: "auth.effects.ts"` crashes Candidate 4.1 with panic `end byte index 1500 is not a char boundary; it is inside '•'` while Candidate 4.2 completes successfully in 11,087 bytes with `isError: False`.

---

## 2. Baseline Cryptographic & Environment Ledger

Prior to source edits, the repository state and candidate binaries were cryptographically recorded:

```
[Repository State]
Branch: feature/phase2-stage5-remediation
Commit: 7106a2f369fe79faf5f95449378a900ab8e8eddf

[Candidate Binary Ledger]
Candidate 3:
  Path:    benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe
  SHA-256: 97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777
  Size:    18,668,032 bytes
  Status:  PRESERVED / UNCHANGED

Candidate 4:
  Path:    benchmarks/matched-pair-harness/bin/candidate-phase4/engram.exe
  SHA-256: FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8
  Size:    18,724,864 bytes
  Status:  PRESERVED / UNCHANGED

Candidate 4.1:
  Path:    benchmarks/matched-pair-harness/bin/candidate-phase4.1/engram.exe
  SHA-256: 300ACE35A31E8E4AF6DD531E8B83B0A5BA9F03E70B27F5E7823E3650CE494015
  Size:    18,801,152 bytes
  Time:    2026-11-19 16:56:22 +05:30
  Status:  PRESERVED / UNCHANGED

Candidate 4.2 (New Sealed Output):
  Path:    benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe
  SHA-256: 78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2
  Size:    18,802,176 bytes
  Time:    2026-11-19 18:28:16 +05:30
  Status:  SEALED / AUTHORIZED
```

---

## 3. Original Defect, Root Cause & Audit of UTF-8 Slicing

### 3.1 Defect Description
In Candidate 4 (`src/mcp.rs:494`) and Candidate 4.1 (`src/mcp.rs:860`), symbol impact inspection formatted the explanation and truncated it using raw byte indexing:

```rust
let raw_explanation = crate::impact::format_impact_explanation(&cone);
let mut explanation = if raw_explanation.len() > 1500 {
    format!(
        "{}\n... [Explanation condensed to fit context budget]",
        &raw_explanation[..1500] // <-- UNSAFE BYTE SLICE
    )
} else {
    raw_explanation
};
```

### 3.2 Root Cause
In Rust, `&str[..N]` indexes byte offsets, not character counts. The formatted markdown impact explanation uses Unicode bullet points (`•`, U+2022, encoded in UTF-8 as 3 bytes `\xE2\x80\xA2`). When byte offset 1500 fell across bytes 1498..1501 of a bullet point, the Rust runtime panicked with:
```
thread 'engram-main' panicked at src\mcp.rs:860:37:
end byte index 1500 is not a char boundary; it is inside '•' (bytes 1498..1501 of string)
```
This aborted the thread, terminating the MCP stdio communication and causing client calls to fail immediately.

### 3.3 Complete Audit of String Slicing Across Codebase
A comprehensive regex audit was performed on all `src/*.rs` files. Beyond fixed-length hex hash slicing (e.g. `hash(...)[..24]` in analyzer and storage, which are guaranteed ASCII), exactly **6 unsafe string slicing locations** were discovered in `src/mcp.rs`:

1. **`src/mcp.rs:258`:** `&exp[..150]` — Level 2 envelope compaction of explanation.
2. **`src/mcp.rs:271`:** `&summary[..100]` — Level 2 envelope compaction of summary.
3. **`src/mcp.rs:444`:** `&task_str[..60]` — Level 6 envelope compaction of task string.
4. **`src/mcp.rs:860`:** `&raw_explanation[..1500]` — `engram_inspect_symbol` initial explanation budget.
5. **`src/mcp.rs:924`:** `&explanation[..300]` — `engram_inspect_symbol` secondary explanation reduction.
6. **`src/mcp.rs:958`:** `&s[..250]` — `engram_inspect_symbol` code excerpt truncation.

All 6 locations were remediated.

---

## 4. Implementation: Smallest Safe Correction

### 4.1 The `safe_truncate_str` Function
A zero-allocation helper was implemented in `src/mcp.rs`:

```rust
/// Safely truncates a string slice to at most `max_bytes` without slicing inside a multi-byte UTF-8 character boundary.
/// If `max_bytes` falls inside a character, the boundary is moved backwards to the start of that character.
/// If `max_bytes == 0` or less than the first character's byte length, returns an empty string slice `""`.
pub fn safe_truncate_str(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &s[..boundary]
}
```

### 4.2 Properties of this Correction
- **Strict Byte-Limit Compliance:** The returned string slice length is strictly $\le \text{max\_bytes}$.
- **Not a Character Count Limit:** Maintains byte-budget semantics rather than character counts (e.g., 1,500 bytes of ASCII = 1,500 chars; 1,500 bytes of 3-byte bullets = 500 bullets).
- **Exact Boundary Behavior:** When the next complete character would exceed `max_bytes`, its partial bytes are excluded, and the boundary decrements to the preceding valid codepoint boundary (at most 3 iterations).
- **Zero Allocation:** Operates entirely by borrowing slices (`&str`), introducing zero heap allocations.
- **Syntactic JSON Validity:** Guarantees that truncated strings in JSON fields are always valid UTF-8, preventing JSON serializer or parser errors.

---

## 5. Adversarial Testing & Reproduction Results

A dedicated test suite `tests/utf8_safety_test.rs` was created covering all required adversarial conditions:

| Test Name | Condition Tested | Result | Duration |
| :--- | :--- | :---: | :---: |
| `test_utf8_01_ascii_exact_boundary` | ASCII text at exactly 1,500 bytes | **PASS** | <0.01s |
| `test_utf8_02_ascii_exceeding_boundary` | ASCII text exceeding 1,500 bytes | **PASS** | <0.01s |
| `test_utf8_03_multibyte_ending_exactly_at_boundary` | 1497 ASCII + 3-byte bullet ending at 1500 | **PASS** | <0.01s |
| `test_utf8_04_multibyte_crossing_boundary` | 1498 ASCII + 3-byte bullet crossing 1500 | **PASS** | <0.01s |
| `test_utf8_05_unicode_bullets` | Repetitive 3-byte bullets (`•`) with boundary split | **PASS** | <0.01s |
| `test_utf8_06_unicode_dashes` | Em-dashes (`—`) and En-dashes (`–`) boundary split | **PASS** | <0.01s |
| `test_utf8_07_emoji` | 4-byte emoji (`🦀`, `🚀`, `💡`) boundary splits | **PASS** | <0.01s |
| `test_utf8_08_mixed_ascii_and_multibyte_sweep` | Exhaustive limit sweep from 0 to length + 10 | **PASS** | <0.01s |
| `test_utf8_09_empty_strings` | Empty string with 0, 1, 1500 limits | **PASS** | <0.01s |
| `test_utf8_10_strings_shorter_than_threshold` | Short strings unaffected | **PASS** | <0.01s |
| `test_utf8_11_repeated_truncation` | Idempotent re-truncation stability | **PASS** | <0.01s |
| `test_utf8_12_valid_serialized_json_after_truncation` | Enforce envelope under tight 1024-byte boundary | **PASS** | <0.01s |
| `test_utf8_deterministic_panic_reproduction` | Asserts naive slice panics & safe slice passes | **PASS** | <0.01s |
| `test_utf8_14_mcp_inspect_symbol_end_to_end_no_panic` | End-to-end `mcp_call` on 25-file fanout with bullets | **PASS** | 1.15s |

### Deterministic Panic Reproduction Output:
```
thread 'test_utf8_deterministic_panic_reproduction' panicked at tests\utf8_safety_test.rs:186:29:
end byte index 1500 is not a char boundary; it is inside '•' (bytes 1498..1501 of string)
test test_utf8_deterministic_panic_reproduction ... ok
```
The panic was caught by `catch_unwind`, proving that the naive expression fails while `safe_truncate_str` succeeds.

---

## 6. End-to-End Before / After Comparison on Task B Repository

Both Candidate 4.1 and Candidate 4.2 were evaluated side-by-side on the golden `angular-ngrx-material-starter` repository (`Test/angular-ngrx-material-starter`):

### Query: `engram_inspect_symbol(target="auth.effects.ts")`

| Metric / Outcome | Candidate 4.1 (Pre-remediation) | Candidate 4.2 (Remediated) |
| :--- | :--- | :--- |
| **Process Exit Status** | **CRASHED (Exit code 101)** | **SUCCESS (Exit code 0)** |
| **Stderr Output** | `thread 'engram-main' panicked at src\mcp.rs:860:37: end byte index 1500 is not a char boundary; it is inside '•'` | Clean, zero errors/panics |
| **Response Emitted** | **0 bytes** (Socket/stdio aborted) | **11,087 bytes** |
| **`isError` Status** | N/A (Process died) | **`false`** |
| **Envelope Compliance** | N/A | **11,087 bytes $\le$ 16,384 bytes** |
| **JSON Validity** | N/A | **100% Valid JSON** |

---

## 7. Complete Regression Test Execution Results

### 7.1 Rust Regression Suites (96 Tests Passed, 0 Failed)

| Test Suite | Command | Tests Run | Passed | Failed | Duration | Exit Code |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| `cargo check` | `cargo check --tests` | N/A | OK | 0 | 22.47s | 0 |
| `utf8_safety_test` | `cargo test --test utf8_safety_test` | 14 | 14 | 0 | 1.38s | 0 |
| `mcp_boundary_test` | `cargo test --test mcp_boundary_test` | 11 | 11 | 0 | 9.54s | 0 |
| `cargo test --lib` | `cargo test --lib` | 24 | 24 | 0 | 0.53s | 0 |
| `context_budget_test` | `cargo test --test context_budget_test` | 20 | 20 | 0 | 0.01s | 0 |
| `inspect_symbol_budget_test` | `cargo test --test inspect_symbol_budget_test` | 13 | 13 | 0 | 3.02s | 0 |
| `mcp_consolidation_test` | `cargo test --test mcp_consolidation_test` | 13 | 13 | 0 | 9.48s | 0 |
| `mcp_test` | `cargo test --test mcp_test` | 1 | 1 | 0 | 0.72s | 0 |
| **Total Rust** | | **96** | **96** | **0** | **47.15s** | **0** |

### 7.2 Python Harness Suites (62 Tests Passed, 0 Failed)

| Test Suite | Command | Tests Run | Passed | Failed | Duration | Exit Code |
| :--- | :--- | :---: | :---: | :---: | :---: | :---: |
| `test_adaptive_activation.py` | `python -m unittest ...` | 12 | 12 | 0 | 43.20s | 0 |
| `test_tetrad_harness.py` | `python -m unittest ...` | 1 | 1 | 0 | 41.10s | 0 |
| `test_task_b_readiness_fixes.py` & `test_task_b_evaluator.py` | `python -m unittest ...` | 35 | 35 | 0 | 4.89s | 0 |
| `test_triad.py` & `test_mcp_runtime_activation.py` | `python -m unittest ...` | 14 | 14 | 0 | 5.48s | 0 |
| **Total Python** | | **62** | **62** | **0** | **94.67s** | **0** |

---

## 8. Harness Integration & Candidate 4.2 Dynamic Resolution

`benchmarks/matched-pair-harness/src/orchestrator.py` was updated to support Candidate 4.2:
- Added `cand4_2_bin` resolving to `benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe`.
- Registered authoritative expected hash: `78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2`.
- Added dynamic environment flag: `ENGRAM_USE_CANDIDATE_4_2=1` routes Arms C and D directly to Candidate 4.2.
- Maintained backward compatibility: without environment flags, default routes to Candidate 4; with `ENGRAM_USE_CANDIDATE_4_1=1`, routes to Candidate 4.1.

---

## 9. Final Working Tree Status & Remaining Risks

### Working Tree Status
```
Changes not staged for commit:
  modified:   benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json
  modified:   benchmarks/matched-pair-harness/src/classifier.py
  modified:   benchmarks/matched-pair-harness/src/orchestrator.py
  modified:   benchmarks/matched-pair-harness/src/runner.py
  modified:   src/impact.rs
  modified:   src/mcp.rs
  modified:   src/storage.rs

Untracked files:
  tests/utf8_safety_test.rs
  benchmarks/matched-pair-harness/bin/candidate-phase4.2/engram.exe
  PHASE_5_4_3_UTF8_SAFETY_AND_CANDIDATE_4_2_REPORT.md
```

### Remaining Risks & Mitigations
1. **Tree-sitter `const` limitation:** Top-level TypeScript `export const authLogin = createAction(...)` variables are not indexed as named symbols. This is not a crash defect; agents can navigate via module files or subpath context queries.
2. **Token Ceiling Headroom:** Task B requires multiple turns; Candidate 4.2's strict $\le 16$ KB response boundary protects against token ceiling breaches observed in Candidate 4.

---

## 10. Final Recommendation

Candidate 4.2 is fully verified, sealed, and ready for Task B preflight verification.

**Status:** **`READY FOR TASK B PREFLIGHT RECHECK`**
