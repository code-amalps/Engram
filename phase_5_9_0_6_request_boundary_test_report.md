# Engram Phase 5.9.0.6: Pre-Dispatch Request Boundary Test Report

**Phase:** 5.9.0.6  
**Test Suite:** `benchmarks/matched-pair-harness/tests/test_request_size_gate.py`  
**Execution Mode:** STRICTLY OFFLINE TEST EXECUTION  
**Date:** 2026-11-22  

---

## 1. Test Suite Summary

The unit test suite `test_request_size_gate.py` was authored and executed to prove the functional correctness, fail-closed enforcement, tokenizer fallback, and zero-truncation reject-only policy of `RequestSizeGate`.

| Metric | Value |
| :--- | :--- |
| **Total Tests Executed** | 8 |
| **Passed Tests** | 8 (100%) |
| **Failed Tests** | 0 (0%) |
| **Skipped Tests** | 0 |
| **Execution Duration** | 1.05 seconds |
| **Pytest Version** | 9.1.1 on Python 3.14.7 |

---

## 2. Test Case Breakdown and Evidence

### Test 1: `test_01_encoder_initialization_offline`
- **Objective:** Verify that `RequestSizeGate` initializes an encoder without requiring network access.
- **Evidence:** Verified `self.gate._encoding_name == "tiktoken_o200k_base"`. The vocabulary loaded from disk cleanly.
- **Result:** **PASSED**

### Test 2: `test_02_token_counting_basic_text`
- **Objective:** Verify accurate token and byte counting on typical programming task instructions.
- **Evidence:** Input text was analyzed; returned exact token count (11 tokens, 73 bytes, method: `tiktoken_o200k_base`).
- **Result:** **PASSED**

### Test 3: `test_03_conservative_heuristic_fallback`
- **Objective:** Verify conservative heuristic fallback calculation ($\lceil \text{bytes} / 3.0 \rceil \times 1.1$) when `tiktoken` is absent.
- **Evidence:** Instantiated gate with `_encoder = None`. Evaluated TypeScript sample. Returned method `heuristic_bytes_div_3_upper_bound` with upper-bound token count matching mathematical expectation.
- **Result:** **PASSED**

### Test 4: `test_04_measure_complete_request_includes_all_parts`
- **Objective:** Verify request measurement inspects prompt text, `.github/copilot-instructions.md`, and CLI parameters.
- **Evidence:** Created `.github/copilot-instructions.md` in sandbox. Verified breakdown components:
  - `breakdown["components"]["prompt"]`
  - `breakdown["components"]["instructions"]`
  - `breakdown["components"]["cli_arguments"]`
  - Total tokens accounted for all three components combined.
- **Result:** **PASSED**

### Test 5: `test_05_check_pre_dispatch_passes_under_limit`
- **Objective:** Verify normal requests within the boundary pass pre-dispatch validation.
- **Evidence:** Evaluated short request (<60,000 tokens). Gate returned `(True, tokens, bytes, "tiktoken_o200k_base", None, breakdown)`.
- **Result:** **PASSED**

### Test 6: `test_06_check_pre_dispatch_rejects_over_limit_fail_closed`
- **Objective:** Verify oversized requests (>60,000 tokens) are strictly rejected with zero truncation.
- **Evidence:** Created oversized prompt (75,000 tokens). Gate returned `passed = False`. Error message contained: `"Pre-dispatch request boundary violation: Complete outbound request is ... tokens ... exceeds configured limit of 60,000 tokens ... Zero-truncation policy enforced"`.
- **Result:** **PASSED**

### Test 7: `test_07_runner_rejection_without_process_spawn`
- **Objective:** Verify `CodexRunner.run()` halts immediately on gate violation without invoking subprocess.
- **Evidence:** Configured runner with `max_request_tokens = 500`. Passed 800-token prompt.
  - Returned `exit_code = 105`.
  - Returned `stop_reason = "PRE_DISPATCH_REQUEST_SIZE_EXCEEDED"`.
  - `request_size_gate_passed = False`.
  - Subprocess was never spawned.
  - Rejection record written to `events.jsonl`.
- **Result:** **PASSED**

### Test 8: `test_08_dry_run_gate_passing`
- **Objective:** Verify dry run mode correctly runs through the gate and populates execution metadata.
- **Evidence:** Normal task prompt evaluated in dry run. Returned `exit_code = 0`, `request_size_gate_passed = True`, `measured_request_tokens` recorded.
- **Result:** **PASSED**

---

## 3. Forensic Conclusion

The pre-dispatch request-size gate is fully operational, deterministic, fails closed, rejects without truncation, and completely prevents unmonitored or oversized prompts from reaching the provider.
