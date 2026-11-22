# Engram Phase 5.4.25: Prompt Context and Completion Limit Audit

**Phase:** 5.4.25  
**Execution Mode:** STRICTLY OFFLINE, READ-ONLY FORENSIC AUDIT  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  
**Branch:** `feature/phase2-stage5-remediation`  
**Previous Phase:** 5.4.24  
**Live Benchmark Authorization:** `NOT_AUTHORIZED`

---

## 1. Executive Summary

Phase 5.4.24 derived a theoretical maximum overshoot bound:
$$\Omega_{\max} = P_{\max} + C_{\max} - R_{\min} = 60,000 + 16,384 - 45,000 = 31,384 \text{ tokens}$$
This formula rested upon two critical assumptions:
1. Prompt context per request is bounded by $P_{\max} = 60,000$ tokens.
2. Completion generation per request is bounded by $C_{\max} = 16,384$ tokens.

This audit conducted an exhaustive, adversarial inspection of the production execution path—specifically `benchmarks/matched-pair-harness/src/runner.py`, `benchmarks/matched-pair-harness/src/orchestrator.py`, and provider CLI invocations—to verify whether $P_{\max}$ and $C_{\max}$ are enforced programmatically or represent merely historical observed quantities.

### Core Audit Verdicts
- **Prompt Token Limit ($P_{\max} = 60,000$):** **`OBSERVED_VALUE_ONLY`** (Not enforced in code or CLI flags).
- **Completion Token Limit ($C_{\max} = 16,384$):** **`OBSERVED_VALUE_ONLY`** (Not enforced in code or CLI flags).
- **Enforced Request-Level Headroom:** Zero pre-flight token gating exists on individual prompt or completion sizes prior to provider dispatch. Pre-flight gating occurs only on cumulative arm budget headroom ($G_{\text{rem}} \ge 45,000$).

---

## 2. Prompt Context Limit Audit ($P_{\max} = 60,000$)

### Adversarial Query Analysis

#### Q1. Is $P_{\max} = 60,000$ enforced in `runner.py`, `orchestrator.py`, or provider CLI flags?
**Answer: NO.**
There is no code in `runner.py`, `orchestrator.py`, or `isolation.py` that checks, asserts, truncates, or limits prompt token length to 60,000 tokens. Neither is any flag passed to the underlying runtime to restrict prompt size.

#### Q2. Does `build_codex_command()` pass `--max-prompt-tokens` or similar?
**Answer: NO.**
Inspection of `runner.py:build_codex_command()` reveals the following CLI assembly:
```python
cmd = [
    str(codex_bin),
    "exec",
    "--sandbox", sandbox_mode,
    "-m", model,
    "-c", f"model_reasoning_effort={reasoning_effort}"
]
if full_auto:
    cmd.append("--full-auto")
```
No `--max-prompt-tokens`, `--max-input-tokens`, `--context-window`, or token-capping flags exist in `build_codex_command()`.

#### Q3. Can a Task B prompt exceed 60,000 tokens if large files are read?
**Answer: YES.**
In Task B (`task_b_remember_me`), the agent inspects Angular components, service templates, and TypeScript modules. If the agent concatenates multiple large repository files into context during exploration, prompt context easily exceeds 60,000 tokens. The model's context window for `gpt-5.6-luna` exceeds 128k tokens, allowing prompt context to scale far past 60,000 tokens.

#### Q4. Does the harness truncate or reject oversized prompts?
**Answer: NO.**
The harness performs zero token counting or truncation on user prompts or conversation history prior to dispatch. Prompts are emitted directly to the provider process.

#### Q5. If prompt tokens are $P > 60,000$, does the harness detect this before or after API dispatch?
**Answer: AFTER API DISPATCH.**
The harness has zero in-flight or pre-dispatch token counting. Prompt usage is discovered exclusively after the request completes, when a `token_usage_record` or `turn.completed` event is emitted on `stdout` and parsed by `StreamingTokenTracker.process_line()`.

#### Q6. Is $P_{\max}$ an enforced limit or an observed historical maximum?
**Answer: OBSERVED HISTORICAL MAXIMUM.**
$P_{\max} = 60,000$ was derived from empirical observations during Stage 1 Block 06/07 runs where Task B turns averaged 35,000 to 58,000 prompt tokens. It has never been codified as an enforced limit.

#### Q7. Can a malicious or anomalous task produce $P = 100,000$?
**Answer: YES.**
An agent reading large data files, minified bundles, or repetitive code blocks can accumulate a 100,000+ token prompt context in a single turn without encountering any client-side obstacle.

#### Q8. What happens to the theoretical overshoot formula if $P = 100,000$?
**Answer: OVERSHOOT GROWS PROPORTIONATELY.**
If $P = 100,000$, with $C = 16,384$ and $R_{\min} = 45,000$:
$$\Omega_{\max} = 100,000 + 16,384 - 45,000 = 71,384 \text{ tokens}$$
The overshoot exceeds the previously asserted 31,384 bound by 40,000 tokens.

#### Q9. What is the actual enforced prompt ceiling, if any?
**Answer: NONE (BOUNDED ONLY BY REMOTE MODEL ARCHITECTURE).**
The only effective limit is the provider model's architectural context window limit (e.g., 128,000 or 200,000 tokens).

#### Q10. Formal classification of $P_{\max} = 60,000$
**Classification: `OBSERVED_VALUE_ONLY`**

---

## 3. Completion Token Limit Audit ($C_{\max} = 16,384$)

### Adversarial Query Analysis

#### Q1. Is $C_{\max} = 16,384$ enforced in `runner.py`, `orchestrator.py`, or provider CLI flags?
**Answer: NO.**
There is no programmatic constraint in the harness enforcing a 16,384 completion token ceiling per turn or per request.

#### Q2. Does `build_codex_command()` pass `--max-output-tokens` or similar?
**Answer: NO.**
Neither `runner.py` nor `orchestrator.py` configures `--max-output-tokens`, `--max-completion-tokens`, or `-c max_tokens=...`.

#### Q3. What is the model-level output token limit for `gpt-5.6-luna`?
**Answer: 16,384 TO 65,536 TOKENS.**
Depending on provider deployment parameters, reasoning-model architectures (`o1`/`gpt-5.6-luna`) support maximum completion bursts of 16,384, 32,768, or up to 65,536 tokens (including hidden chain-of-thought reasoning tokens).

#### Q4. Can a multi-turn completion exceed 16,384 tokens across multiple turns?
**Answer: YES.**
Across multiple turns in a single arm, cumulative completion tokens routinely exceed 16,384. In Phase 5.4.13 Live 02, completion tokens across turns totaled over 25,000 tokens.

#### Q5. Can a single turn completion exceed 16,384 tokens?
**Answer: YES.**
If the model engages in extensive internal chain-of-thought reasoning before outputting code, reasoning tokens are billed as output/completion tokens and can reach up to the provider's architectural limit (e.g. 32,768 or 65,536 tokens).

#### Q6. Does the harness terminate the process if a completion exceeds 16,384 tokens?
**Answer: NO.**
The harness only evaluates total cumulative tokens (`effective_tokens`) against the arm ceiling ($A_{\text{ceiling}} = 550,000$) or effective arm ceiling ($E_{\text{arm}}$). A single turn generating 25,000 completion tokens is not terminated unless cumulative tokens exceed the effective arm ceiling.

#### Q7. Is $C_{\max}$ an enforced limit or an observed historical maximum?
**Answer: OBSERVED HISTORICAL MAXIMUM.**
$C_{\max} = 16,384$ corresponds to historical standard generation limits, not an enforced harness policy.

#### Q8. What happens to the theoretical overshoot formula if $C = 32,768$?
**Answer: OVERSHOOT INCREASES ACCORDINGLY.**
If $P = 60,000$, $C = 32,768$, and $R_{\min} = 45,000$:
$$\Omega_{\max} = 60,000 + 32,768 - 45,000 = 47,768 \text{ tokens}$$
If $P = 100,000$ and $C = 32,768$:
$$\Omega_{\max} = 100,000 + 32,768 - 45,000 = 87,768 \text{ tokens}$$

#### Q9. What is the actual enforced completion ceiling, if any?
**Answer: NONE AT THE TURN LEVEL.**
Enforcement operates solely on total cumulative arm tokens at turn boundaries.

#### Q10. Formal classification of $C_{\max} = 16,384$
**Classification: `OBSERVED_VALUE_ONLY`**

---

## 4. Synthesis and Production Path Risk Assessment

| Parameter | Historical Stated Value | Production Enforced? | Mechanism | Classification |
|---|---|---|---|---|
| **$P_{\max}$ (Prompt)** | 60,000 tokens | **NO** | None (Provider architecture limit only) | `OBSERVED_VALUE_ONLY` |
| **$C_{\max}$ (Completion)** | 16,384 tokens | **NO** | None (Provider architecture limit only) | `OBSERVED_VALUE_ONLY` |
| **$R_{\min}$ (Pre-dispatch Reserve)** | 45,000 tokens | **YES** | `StreamingTokenTracker.check_pre_dispatch_headroom()` & `orchestrator.py` | `ENFORCED_HARD_LIMIT` |
| **$E_{\text{arm}}$ (Effective Arm Ceiling)** | Dynamic ($\le 550,000$) | **YES** | Dynamically clamped to $\min(A_{\text{ceiling}}, G_{\text{remaining}})$ | `ENFORCED_HARD_LIMIT` |

### Key Risk Conclusion
Because neither prompt length nor completion burst size is clamped by harness flags or pre-dispatch checks, any claim that single-turn overshoot is mathematically capped at 31,384 tokens is contingent on empirical behavior rather than a cryptographic or software guarantee.

The upper bound must be formally designated as a **`CONDITIONAL_UPPER_BOUND`**, conditioned upon prompt context remaining $\le 60,000$ and completion output remaining $\le 16,384$.
