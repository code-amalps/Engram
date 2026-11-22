# Engram Phase 5.8: Raw Test Log Index

**Date:** 2026-11-22  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Execution Environment:** Windows 11 x86_64, Offline, `CARGO_NET_OFFLINE=true`  
**Compiler:** `rustc 1.83.0 (90b35a623 2024-11-26)`

---

## 1. Index of Test Invocations

| Log Entry ID | Command Executed | Working Directory | Duration | Exit Code | High-Level Result |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **LOG-5.8-TEST-01** | `cargo test --manifest-path Cargo.toml --lib --bin engram --test ast_test --test benchmark_harness_test --test token_efficiency_test --offline` | `C:\Users\Amal P S\OneDrive\Projects\Engram` | 154.2s | 0 | 87 passed, 0 failed |
| **LOG-5.8-TEST-02** | `cargo test --manifest-path Cargo.toml --offline` | `C:\Users\Amal P S\OneDrive\Projects\Engram` | 1184.9s | 101 | 49 suites passed, 1 suite failed |
| **LOG-5.8-TEST-03** | `cargo test --manifest-path Cargo.toml --test query_router_test --offline` | `C:\Users\Amal P S\OneDrive\Projects\Engram` | 30.8s | 101 | 14 passed, 1 failed (`test_15`) |
| **LOG-5.8-TEST-04** | `python -m pytest benchmarks/matched-pair-harness/tests/test_tetrad_harness.py` | `C:\Users\Amal P S\OneDrive\Projects\Engram` | 257.44s | 0 | 1 passed, 0 failed |
| **LOG-5.8-TEST-05** | `cargo check --manifest-path $env:TEMP\engram_isolated_5_8\Cargo.toml --offline` | Isolated Build Env | 96.8s | 0 | Clean build check |
| **LOG-5.8-TEST-06** | `cargo build --manifest-path $env:TEMP\engram_isolated_5_8\Cargo.toml --release --offline` | Isolated Build Env | 508.9s | 0 | 18,865,664 bytes binary |

---

## 2. Key Excerpts from Test Logs

### LOG-5.8-TEST-01: Targeted Phase 5.5 Test Suite
```text
running 24 tests
test ... ok (24 passed)
running 35 tests
test ... ok (35 passed)
running 13 tests
test ... ok (13 passed)
running 10 tests
test ... ok (10 passed)
running 5 tests
test test_optimization_a_tree_sitter_signatures ... ok
test test_optimization_b_exact_type_pruning ... ok
test test_optimization_c_tier2_budget_capping ... ok
test test_optimization_d_whitespace_comment_stripping ... ok
test test_optimization_e_query_dedup ... ok
test result: ok. 5 passed; 0 failed; finished in 0.42s
```

### LOG-5.8-TEST-03: `query_router_test` Regression Detail
```text
running 15 tests
test test_01_generic_query_routing_without_framework_adapter ... ok
test test_02_angular_ngrx_adapter_discovery ... ok
test test_03_exact_symbol_matching ... ok
test test_04_task_terminology_matching ... ok
test test_05_cross_file_relationship_discovery ... ok
test test_06_actions_effects_services_store_registration ... ok
test test_07_scenario_a_golden_angular_http_error_interceptor ... ok
test test_08_scenario_b_golden_angular_auth_effects ... ok
test test_09_scenario_c_golden_angular_remember_me ... ok
test test_10_relevance_ranking ... ok
test test_11_provenance_and_explanation_completeness ... ok
test test_12_deterministic_repeated_retrieval ... ok
test test_13_external_and_unresolved_dependency_handling ... ok
test test_14_go_and_java_compatibility ... ok
test test_15_missing_or_invalidated_entities ... FAILED

failures:
---- test_15_missing_or_invalidated_entities stdout ----
thread 'test_15_missing_or_invalidated_entities' (6020) panicked at tests\query_router_test.rs:555:5:
assertion failed: !res2.files.iter().any(|f| f.path.contains("temp_service.rs"))

failures:
    test_15_missing_or_invalidated_entities
test result: FAILED. 14 passed; 1 failed; 0 ignored; finished in 17.36s
```

### LOG-5.8-TEST-04: Python Tetrad Harness Execution
```text
============================= test session starts =============================
platform win32 -- Python 3.11.9, pytest-8.3.3, pluggy-1.5.0
rootdir: C:\Users\Amal P S\OneDrive\Projects\Engram\benchmarks\matched-pair-harness
collected 1 item

benchmarks/matched-pair-harness/tests/test_tetrad_harness.py .           [100%]
============================== 1 passed in 257.44s ==============================
```

---

## 3. Storage and Reproducibility

All logs were captured directly from deterministic offline process executions. No remote resources, live API keys, or network connections were utilized during any of the above test invocations.
