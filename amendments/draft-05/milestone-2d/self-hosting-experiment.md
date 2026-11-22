# Engram Amendment 05 — Milestone 2D: Track B/D Self-Hosting Experiment & Evaluation

**Document Version:** 1.0.0  
**Date:** 2026-10-30  
**Status:** Completed Offline Experiment  
**Target Investigation:** Execution Architecture of Engram Matched-Pair Harness

---

## 1. Executive Summary

This document details an offline self-development experiment conducted with the Candidate Phase 1 Engram binary (`engram.exe`, SHA-256: `F56347...`) on an isolated snapshot of the Engram codebase.

The experiment investigated the following engineering task:
> *"Explain the execution architecture of Engram's matched-pair harness and identify the minimum set of files involved in adding a new execution mode."*

The experiment establishes that Engram's AST indexing and graph-traversal engine successfully identified key architectural files, test suites, and reverse dependency relationships without live inference. However, cross-language lexical competition (between the Rust internal benchmark runner and the Python harness) and schema file omissions represent areas where human guidance or sub-directory scoping remains essential.

---

## 2. Experimental Setup

- **Workspace:** Disposable copy at `C:\Users\AMALPS~1\AppData\Local\Temp\engram_self_hosting_2d`.
- **Included Content:** Full `src/` (Rust), `tests/`, `Cargo.toml`, and `benchmarks/matched-pair-harness/` (Python source, config, schemas, tests).
- **Excluded Content:** `.git`, secrets, `.env`, benchmark report archives, golden repository.
- **Engram Binary:** `benchmarks/matched-pair-harness/bin/candidate-phase1/engram.exe`.
- **Inference Status:** 100% Offline. Zero model API calls, zero network activity, zero financial spend ($0.00).

---

## 3. Engram-Generated Findings

### 3.1 Context & Prompt Compilation Output
When queried with `engram prompt -C <workspace> --stdout --no-clipboard "Explain the execution architecture of Engram's matched-pair harness and identify the minimum set of files involved in adding a new execution mode"`, Engram produced:
1. **Top Priority Selected File (Budget: 1,200 tokens):**
   - `benchmarks/matched-pair-harness/tests/test_milestone_2c_remediation.py` (Relevance Score: 659.89)
   - Extracted AST Symbols:
     - `CLASS TestSingleArmModeRemediation` (Line 95)
     - `FUNCTION test_run_single_dispatches_only_requested_arm` (Line 105)
     - `FUNCTION test_run_single_rejects_invalid_arm` (Line 137)
     - `CLASS TestCLIArgumentParsing` (Line 521)
     - `FUNCTION test_cli_parser_accepts_mode_single_and_output_dir` (Line 524)
2. **Top Priority Selected File (Budget: 4,000 tokens):**
   - `src/empirical_runner.rs` (Relevance Score: 695.42)
   - Extracted AST Symbols:
     - `STRUCT MultiArmOrchestrator` (Line 817)
     - `STRUCT Sandbox` (Line 234)
     - `TRAIT AgentDriver` (Line 567)
     - `STRUCT BenchmarkRunReport` (Line 695)
3. **Budget Accounting & Omission Signals:**
   - Engram explicitly warned: `Status: PARTIAL_GROUNDING (1 files omitted due to budget)`.
   - Identified `src/empirical_runner.rs` as exceeding the 1,200-token ceiling (+1,154 tokens).

### 3.2 Symbol Blast Radius Inspection
Running `engram inspect -C <workspace> "Orchestrator"` produced:
- **Direct Target:** `benchmarks/matched-pair-harness/src/orchestrator.py`
- **1-Hop Imports:** `argparse`, `datetime`, `evaluator`, `isolation`, `json`, `os`, `pathlib`, `random`, `runner`, `schema_validator`, `sys`, `telemetry`, `typing`.
- **2-Hop Dependent Files:**
  - `benchmarks/matched-pair-harness/src/classifier.py`
  - `benchmarks/matched-pair-harness/src/evaluator.py`
  - `benchmarks/matched-pair-harness/src/isolation.py`
  - `benchmarks/matched-pair-harness/src/runner.py`
  - `benchmarks/matched-pair-harness/src/schema_validator.py`
  - `benchmarks/matched-pair-harness/src/telemetry.py`
  - `benchmarks/matched-pair-harness/src/verification.py`
  - 12 regression test files in `benchmarks/matched-pair-harness/tests/`.

---

## 4. Ground Truth Comparison (Independent Manual Source Audit)

To validate Engram's findings, an independent source audit was conducted on the Milestone 2C implementation to establish the exact minimum set of files required to add a new execution mode (specifically, `--mode single`):

| Required Change Area | Actual Necessary Files | Identified by Engram? | Classification |
|:---|:---|:---:|:---:|
| **CLI & Execution Routing** | `benchmarks/matched-pair-harness/src/orchestrator.py` | **YES** | **CORRECT** (Mapped as Direct Target) |
| **Agent Runner & Flags** | `benchmarks/matched-pair-harness/src/runner.py` | **PARTIAL** | **PARTIAL** (Identified in 2-hop blast radius, but omitted from 1,200-token top prompt) |
| **Task Configuration** | `benchmarks/matched-pair-harness/config/tasks/smoke_test_read_only.json` | **NO** | **FALSE NEGATIVE** (JSON files are not indexed in symbol table) |
| **Schema Definitions** | `benchmarks/matched-pair-harness/schemas/*.schema.json` | **NO** | **FALSE NEGATIVE** (JSON schemas lack tree-sitter AST symbol definitions) |
| **Remediation & Unit Tests**| `benchmarks/matched-pair-harness/tests/test_milestone_2c_remediation.py` | **YES** | **CORRECT** (Ranked #1 in context ranking) |
| **Cross-Language Noise** | `src/empirical_runner.rs` | **NO (False Positive)**| **FALSE POSITIVE** (Rust benchmark runner selected due to lexical keyword overlap) |

---

## 5. Assessment of Context Quality for Coding Agents

### 5.1 Strengths
1. **Extreme Token Density:**  
   Instead of dumping the entire 1,586-line `orchestrator.py` (~15,000 tokens), Engram compiled the relevant classes, method signatures, line numbers, and docstrings into ~1,200 tokens. This represents a **>90% context reduction**.
2. **Actionable Symbol Anchors:**  
   By citing exact line numbers and symbol names (`TestSingleArmModeRemediation`, `test_run_single_dispatches_only_requested_arm`), an agent (such as Antigravity or Codex) can immediately construct targeted edits without performing iterative codebase searches (`grep`, `find`, directory walking).
3. **Ecosystem & Boundary Invariants:**  
   The context package injects approved dependencies from `Cargo.toml` / package manifests and explicit guard warnings, preventing an agent from hallucinating or introducing unauthorized external libraries.

### 5.2 Areas for Improvement
1. **Non-Code Artifact Indexing:**  
   JSON task configs and schemas are critical to the matched-pair harness architecture. Because tree-sitter indexing targets programming languages (`.rs`, `.py`, `.ts`), configuration files were omitted from symbol ranking. Engram should expand manifest/schema awareness to include structured JSON/YAML task descriptors.
2. **Language/Scope Disambiguation:**  
   When a project contains multiple sub-ecosystems (e.g., a Rust core tool and a Python evaluation harness), query terms like "orchestrator" create BM25 competition. Scoping flags (e.g. `--path benchmarks/matched-pair-harness`) prevent cross-language false positives.

---

## 6. Track D: Token Efficiency Measurement Design

To determine empirically whether Engram provides net token savings, the following rigorous experimental protocol is designed for future authorized benchmarking:

### 6.1 Experimental Arm Definitions
- **Control Arm (Standard Agent Exploration):**  
  The agent receives the raw task prompt and an empty workspace instruction context. It must explore the codebase autonomously using file listing (`ls`), search (`grep`), and file reading (`read_file`) tools to discover the execution architecture.
- **Treatment Arm (Engram-Assisted Context):**  
  The agent receives the Turn-0 structured prompt generated by `engram prompt` (~1,200 tokens of AST-grounded symbols, relevant files, and boundary rules).

### 6.2 Primary Metrics
1. **Total Input Tokens:** Aggregate input tokens consumed across all agent turns.
2. **Exploration Overhead Tokens:** Input tokens expended specifically on directory listings, broad file reads, and grep searches prior to the first source file edit.
3. **Turn Count to First Edit:** Number of agent tool interactions before making the first implementation change.
4. **Cached Input Tokens:** Tokens served from platform prefix cache.
5. **Output / Reasoning Tokens:** Generation and reasoning token expenditure.
6. **Functional Acceptance Score:** Binary pass/fail across automated gates (L0–L3).

### 6.3 Net Token Efficiency Equation
$$\text{Net Token Savings} = \text{Input Tokens}_{\text{Control}} - \left( \text{Input Tokens}_{\text{Treatment}} + \text{Engram Context Overhead} \right)$$

Where $\text{Engram Context Overhead} \approx 1,200 \text{ tokens}$. If the control arm expends 15,000–30,000 tokens exploring the 160-file repository, a treatment overhead of 1,200 tokens produces a net saving of **13,800 to 28,800 tokens per task turn**.

### 6.4 Controlled Invariants
- Identical model (`gpt-5.6-luna`) and reasoning effort (`low`).
- Identical baseline commit (`1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`).
- Identical operational ceilings (10,000 tokens) and pre-dispatch reserves (2,000 tokens).
- Read-only sandbox boundary enforcement across both arms.
