# Engram / AMstraLift — Phase B Completion Report: Ecosystem Adapters & Benchmark Safety Audit

**Date:** October 29, 2026  
**Status:** COMPLETE — READY FOR REVIEW  
**Execution Mode:** Hermetic Offline Only (Zero Model Calls, Zero Benchmark Execution, Zero Quota Consumed)

---

## 1. Executive Summary

Phase B adapter work has been audited, implemented, and validated across the three planned ecosystems:
1. **Python Ecosystem Adapter:** Standard PEP 621 / Poetry `pyproject.toml` parsing via `tomllib`, PEP 508 requirement & extras handling, PEP 503 PyPI canonicalization, resilient network client, and safe fallback on malformed TOML.
2. **.NET Ecosystem Adapter:** NuGet flat-container API version intelligence, semantic SemVer sorting overcoming unordered upstream API arrays, dual-layer prerelease filtering, and major version constraint enforcement.
3. **React Ecosystem Adapter:** npm registry version intelligence, SemVer comparison with canary/prerelease rejection, target-major bounding, and resilient corporate proxy / Windows CA cert handling.

### Key Metrics & Verification Overview
- **Adapter Unit & Regression Suite (`AMstraLift`):** 33 / 33 tests passed (`pytest` in 69.27s).
- **Governance & Safe Remediation Suite (`AMstraLift`):** 24 / 24 tests passed (`pytest` in 9.56s).
- **Benchmark Harness Suite (`Engram`):** 128 / 128 tests passed (`unittest` in 69.13s).
- **Network Calls:** 100% mocked offline in test suites; zero live API/model queries.
- **Benchmark Baselines:** `benchmarks/cost-baseline/` and `benchmarks/profile/` remain completely untouched.

---

## 2. Phase 1 — Repository Audit Before Editing

### 2.1 Workspace Architecture & Repository Alignment
- **AMstraLift Workspace (`c:\Users\Amal P S\OneDrive\Projects\AMstraLift`):**
  - Branch: `Dev`
  - Base Commit: `c0fb925` (pre-edit working tree was clean)
  - Contains: Core migration engine, `BaseAdapter` contract (`amstralift/adapters/base.py`), ecosystem adapters (`python.py`, `dotnet.py`, `react.py`, `angular.py`), security clients (`osv_client.py`), and test suites (`tests/`).
- **Engram Workspace (`C:\Users\Amal P S\OneDrive\Projects\Engram`):**
  - Branch: `feature/async-precommit-guard`
  - Contains: Matched-pair benchmark harness (`benchmarks/matched-pair-harness/`), task definitions, frozen binary hashes, and telemetry schemas.

### 2.2 Pre-Implementation Adapter Audit Matrix

| Adapter | Status Before Phase B | Confirmed Gaps Identified | Action Taken |
| :--- | :--- | :--- | :--- |
| **Python** (`python.py`) | `PARTIAL` | • Regex-only parsing failed on PEP 508 extras (e.g. `uvicorn[standard]`) and markers.<br>• Ignored `[project.optional-dependencies]` and Poetry tables.<br>• Non-canonicalized PyPI queries.<br>• Unresilient `httpx.Client`.<br>• Duplicate stub method in class body. | • Implemented `tomllib` structured parsing.<br>• Integrated `packaging.requirements.Requirement`.<br>• Implemented `canonicalize_name` for PEP 503.<br>• Added `_get_http_client` resilient factory.<br>• Cleaned up duplicate stub. |
| **.NET** (`dotnet.py`) | `PARTIAL` | • Assumed NuGet `versions` array was SemVer sorted (`matching[-1]`, `stable[-1]`).<br>• Incomplete prerelease rejection for edge-case SemVer tags.<br>• Lack of resilience tests for timeouts and malformed JSON. | • Implemented semantic version parsing with `packaging.version.Version`.<br>• Added explicit semantic sorting key (`key=lambda item: item[0]`).<br>• Dual-layer prerelease filter (`"-" not in v` + `not v_obj.is_prerelease`). |
| **React** (`react.py`) | `PARTIAL` | • Restrictive 3-segment regex `(\d+)\.(\d+)\.(\d+)` rejected versions with build metadata or 4 segments.<br>• Unresilient standard `httpx.Client`.<br>• Lack of offline tests for error handling and timeouts. | • Integrated `packaging.version.Version` sorting.<br>• Implemented resilient HTTP client wrapper.<br>• Enhanced canary / prerelease filter. |

---

## 3. Phase 2 — Implementation Summary

### 3.1 Python Adapter (`amstralift/adapters/python.py`)
- **Structured Manifest Parsing (`tomllib`):**
  - Parses standard PEP 621 `[project.dependencies]` (list of strings).
  - Parses `[project.optional-dependencies]` (dict of groups to lists of dependency strings).
  - Parses Poetry manifests: `[tool.poetry.dependencies]` and `[tool.poetry.group.<name>.dependencies]`.
  - Extracts package names, extras, and version specifiers via `packaging.requirements.Requirement`.
  - Preserves extras during `apply_upgrade` (e.g. `uvicorn[standard]>=0.20.0` updates to `uvicorn[standard]>=0.32.0`).
  - Supports Poetry table updates in `apply_upgrade`.
  - Added robust fallback: if `pyproject.toml` contains invalid TOML syntax (`TOMLDecodeError`), the adapter catches the exception and falls back to regex parsing without crashing.
- **PyPI Version Intelligence:**
  - Standardizes package lookup using PEP 503 name normalization (`canonicalize_name`).
  - Queries `https://pypi.org/pypi/{canonical}/json`.
  - Rejects pre-release and dev versions (`v_obj.is_prerelease` and `v_obj.is_devrelease`).
  - Employs `_get_http_client` which defaults to `create_resilient_httpx_client` (native Windows certificate store and corporate proxy support) while seamlessly honoring unit test mock patches.
  - Returns `None` gracefully on HTTP 404, HTTP 500, timeouts, or malformed payloads.

### 3.2 .NET Adapter (`amstralift/adapters/dotnet.py`)
- **NuGet Flat-Container Contract Compliance:**
  - Queries `https://api.nuget.org/v3-flatcontainer/{package_name.lower()}/index.json`.
  - Parses version array semantically using `packaging.version.Version`.
  - Enforces true SemVer sorting: resolves cases where NuGet API arrays are unsorted or string-sorted (e.g. `1.10.0` vs `1.2.0`).
  - Applies dual-layer prerelease filtering: rejects hyphens (`"-" in v`) per NuGet specification and verifies `not v_obj.is_prerelease and not v_obj.is_devrelease`.
  - Implements `target_major` filtering (`item[0].major == target_major`), correctly selecting the highest stable release within the specified major line.
  - Wrapped HTTP access in resilient client with robust exception handling.

### 3.3 React Adapter (`amstralift/adapters/react.py`)
- **npm Registry Version Intelligence:**
  - Queries `https://registry.npmjs.org/{package_name}`.
  - Implemented `Version` comparison for version keys in `versions` dictionary, removing rigid 3-segment regex restrictions.
  - Filters out canary, next, rc, beta, alpha, and preview versions from both `dist-tags.latest` and the `versions` map.
  - Semantically sorts matching versions to determine the true maximum release.
  - Preserves multi-major coordination (React 18 vs 19) and native binary override deduplication (`esbuild`, `@rollup/*`).
  - Uses `create_resilient_httpx_client` for corporate proxy and certificate trust.

---

## 4. Phase 3 — Offline Test Coverage & Verification

### 4.1 New Unit Tests Authored
10 new offline unit tests were added across the test suite to validate all Phase B capabilities:

1. **`tests/test_python_adapter.py` (+4 tests):**
   - `test_python_pyproject_toml_parsing_pep621_and_poetry`: Validates structured extraction of PEP 621 dependencies, optional extras, and Poetry tables.
   - `test_python_pyproject_toml_malformed_syntax_fallback`: Verifies graceful fallback to regex when `pyproject.toml` has invalid TOML syntax.
   - `test_python_apply_upgrade_preserves_extras_and_poetry`: Confirms PEP 508 extras and Poetry table entries are preserved and correctly updated.
   - `test_python_fetch_latest_canonicalization_and_resilience`: Verifies PEP 503 canonicalization (`Py_YAML` -> `py-yaml`) and 404/error handling.
2. **`tests/test_dotnet_adapter.py` (+3 tests):**
   - `test_dotnet_nuget_flatcontainer_unsorted_versions_resolution`: Validates that unsorted NuGet version arrays (e.g. `["1.0.0", "1.10.0", "1.2.0"]`) correctly resolve to `1.10.0`.
   - `test_dotnet_nuget_flatcontainer_target_major_with_prerelease_filtering`: Tests major bounding and prerelease/preview rejection across multiple major versions.
   - `test_dotnet_nuget_flatcontainer_resilience_and_errors`: Tests HTTP 404, empty versions array, malformed JSON, and timeouts.
3. **`tests/test_react_adapter.py` (+3 tests):**
   - `test_react_npm_registry_target_major_resolution`: Verifies target major bounding across React 17, 18, and 19.
   - `test_react_npm_registry_unsorted_and_metadata_versions`: Verifies semantic resolution over unordered version dictionary keys.
   - `test_react_npm_registry_resilience_and_errors`: Tests 404, timeouts, and empty responses.

### 4.2 Test Suite Execution Results

#### AMstraLift Adapter Suite
```
pytest tests/test_python_adapter.py tests/test_dotnet_adapter.py tests/test_react_adapter.py
======================== 33 passed in 69.27s (0:01:09) ========================
```
- `tests/test_python_adapter.py`: 10 passed (6 baseline + 4 new)
- `tests/test_dotnet_adapter.py`: 14 passed (11 baseline + 3 new)
- `tests/test_react_adapter.py`: 9 passed (6 baseline + 3 new)

#### AMstraLift Governance & Remediation Suite
```
pytest tests/test_governance_policies.py tests/test_safe_remediation.py
============================= 24 passed in 9.56s ==============================
```

#### Engram Benchmark Harness Regression Suite
```
python -m unittest discover -s benchmarks/matched-pair-harness/tests -p "test_*.py"
----------------------------------------------------------------------
Ran 128 tests in 69.132s

OK
```

---

## 5. Phase 4 — Benchmark Invariant and Safety Review

| Safety Invariant | Audit Result | Evidence |
| :--- | :--- | :--- |
| **No Live Model / API Calls** | **CONFIRMED** | All unit tests operate with hermetic mock clients or local git repositories. No Codex processes spawned. |
| **No Benchmark Launches** | **CONFIRMED** | Blocks 06–10 remain strictly unexecuted. Harness orchestrator was not called in live mode. |
| **No Quota Consumed** | **CONFIRMED** | Model token exposure during Phase B: exactly 0 tokens. |
| **Cost Baseline & Profile Untouched** | **CONFIRMED** | `git status` confirms zero modifications in `benchmarks/cost-baseline/` and `benchmarks/profile/`. |
| **Frozen Binaries & Hashes Preserved** | **CONFIRMED** | v1.5.1 and v1.5.4 binaries in `benchmarks/matched-pair-harness/bin/` remain sealed and unmodified. |
| **Clean Git Boundary** | **CONFIRMED** | AMstraLift changes isolated strictly to 3 adapter files and 3 test files. Engram working tree has zero adapter code pollution. |

---

## 6. Git Working-Tree Verification

### AMstraLift Working Tree:
```
 M amstralift/adapters/dotnet.py
 M amstralift/adapters/python.py
 M amstralift/adapters/react.py
 M tests/test_dotnet_adapter.py
 M tests/test_python_adapter.py
 M tests/test_react_adapter.py
```
- Exactly 6 files modified (3 implementations, 3 test files).
- Zero untracked files generated.

### Engram Working Tree:
- Untouched by Phase B adapter edits.
- Only benchmark reports and previously authorized resilience repair files present.

---

## 7. Conclusion & Next Steps

Phase B adapter work is **COMPLETE** and verified against all architectural contracts, governance policies, and benchmark safety invariants.

- **Current Status:** Clean, review-ready working tree.
- **Commit / Push Status:** Halted per instructions; awaiting explicit user instruction before staging, committing, or pushing.
