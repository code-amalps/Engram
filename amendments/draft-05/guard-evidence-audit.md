# Engram Guard Telemetry Forensic Audit (Blocks 06, 07, and 08)

**Document Identifier:** `ENGRAM_DRAFT05_GUARD_EVIDENCE_AUDIT`  
**Evaluation Scope:** Engram Amendment 05 Pass 5 Read-Only Forensic Investigation  
**Governing Standard:** Strict Evidence Labeling (`OBSERVED`, `RECONSTRUCTED`, `UNKNOWN`)  
**Workspace Execution State:** Strictly Offline — Zero Model/API Inference Credits Consumed  

---

## 1. Executive Summary & Telemetry Architecture

### 1.1 The Dual-Channel Telemetry Reality
A rigorous read-only forensic audit of historical runs (`task_b_remember_me_block_06_baseline_arm1`, `task_b_remember_me_block_07_v154_arm3`, and `task_b_remember_me_block_08_v154_arm3`) reveals a fundamental distinction between what occurred in live execution and what was subsequently reconstructed from disk artifacts:

1. **Child Process Standard Output Stream (`events.jsonl`):**  
   During execution, the Codex CLI (`codex.exe exec --json ...`) emitted streaming events to stdout. Inspection of the archived stdout event logs ([`baseline_arm1/events.jsonl`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_06/baseline_arm1/events.jsonl), [`v154_arm3/events.jsonl`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/reports/runs/task_b_remember_me_block_07/v154_arm3/events.jsonl)) confirms that stdout contained **ONLY** lifecycle and execution events:
   - `thread.started`
   - `turn.started`
   - `item.started` (`command_execution`, `mcp_tool_call`, `file_change`)
   - `item.completed` (`command_execution`, `mcp_tool_call`, `file_change`, `agent_message`, `error`)
   **Critical Telemetry Finding:** The Codex CLI did **NOT** emit `token_usage_record`, `token_count`, or intermediate token usage numbers to stdout.

2. **Internal Native Session Rollouts (`~/.codex/sessions/**/*.jsonl`):**  
   Codex wrote authoritative token records asynchronously directly to local session rollout files on disk. These files contain `token_usage_record` and `event_msg` (`type: "token_count"`), which document exact per-turn token usage reported by the OpenAI API upon the conclusion of each remote model inference request.

3. **Historical Runner Architecture (Blocks 06, 07, 08):**  
   Inspection of `benchmarks/matched-pair-harness/src/runner.py` at commit `HEAD` (`c70845b410522197609f98485cb487f5d08409e5`) confirms that during Blocks 06, 07, and 08:
   - The runner invoked `proc = subprocess.Popen(...)` followed immediately by `proc.communicate(timeout=self.timeout_seconds)` ([`runner.py#L171-L183`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L171-L183)).
   - The historical runner performed **no live streaming polling**, **no live rollout discovery**, and maintained **no streaming token estimate** while the child process was running.
   - All token usage data was extracted post-hoc by [`telemetry.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/telemetry.py) from the session rollout *after* Codex had already exited.

4. **Remediation Code (`StreamingTokenTracker`):**  
   The `StreamingTokenTracker` and dynamic rollout-tailing loop (`time.sleep(0.05)`) currently present in the working tree diff of [`benchmarks/matched-pair-harness/src/runner.py`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py) represent **offline remediation code authored after Block 07**, not the harness implementation that executed during Block 07.

---

## 2. Turn-by-Turn Telemetry Reconstruction Across Historical Runs

Data points are categorized in strict compliance with audit instructions:
- `OBSERVED`: Directly extracted from authoritative event timestamps, response IDs, and rollout payloads.
- `RECONSTRUCTED`: Derived from historical records via documented post-hoc calculations.
- `UNKNOWN`: Missing from the evidence; cannot be determined offline without speculation.

### 2.1 Block 06: Baseline Arm 1 (`task_b_remember_me_block_06_baseline_arm1`)
- **Rollout File:** `~/.codex/sessions/2026/10/29/rollout-2026-10-29T11-31-31-01a1863f-cd55-7ea0-a9fd-7c567ddfc3d5.jsonl`
- **Total Turns:** 9 turns
- **Configured Ceiling:** 250,000 tokens
- **Final Outcome:** `TOKEN_CEILING_BREACHED` (253,748 tokens)

| Turn | Request / Response ID | Dispatch Timestamp | Streaming Estimate Visible to Runner | Authoritative Usage Event Timestamp | Authoritative Turn Delta ($\Delta_{\text{auth}}$) | Authoritative Cumulative ($T_{\text{auth}}$) | Visibility to Runner Timestamp | Estimation Error | Observation Lag | Next Request Dispatched Before Guard Observed? |
| :---: | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **1** | `resp_01b69d4203aee792016ae2e14a29188191b95353fa9a4580ba` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T06:01:37.905Z` | 13,457 | 13,457 | Post-run (`06:03:19Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 2) |
| **2** | `resp_01b69d4203aee792016ae2e166328481919d5e3d2a2ab09142` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T06:02:05.532Z` | 21,828 | 35,285 | Post-run (`06:03:19Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 3) |
| **3** | `resp_01b69d4203aee792016ae2e16c98bc81918d820364095f0409` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T06:02:11.564Z` | 23,084 | 58,369 | Post-run (`06:03:19Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 4) |
| **4** | `resp_01b69d4203aee792016ae2e171003c8191a01a051bc66496d9` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T06:02:16.268Z` | 25,468 | 83,837 | Post-run (`06:03:19Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 5) |
| **5** | `resp_01b69d4203aee792016ae2e177aa9081918f10496c90ba3f98` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T06:02:23.215Z` | 30,132 | 113,969 | Post-run (`06:03:19Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 6) |
| **6** | `resp_01b69d4203aee792016ae2e17eb4288191bcdc26b71343778e` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T06:02:32.238Z` | 32,030 | 145,999 | Post-run (`06:03:19Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 7) |
| **7** | `resp_01b69d4203aee792016ae2e1872f8c8191968defc7a0d789b9` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T06:02:47.402Z` | 34,867 | 180,866 | Post-run (`06:03:19Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 8) |
| **8** | `resp_01b69d4203aee792016ae2e1a7df40819196ac12fde3f6414f` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T06:03:12.291Z` | 35,231 | 216,097 | Post-run (`06:03:19Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 9) |
| **9** | `resp_01b69d4203aee792016ae2e1aeffe88191a5a7281220d063f1` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T06:03:19.147Z` | 37,651 | 253,748 | Post-run (`06:03:19Z`) | `UNKNOWN` | `UNKNOWN` | `N/A` (Process halted on ceiling breach) |

*Evidence Citation:* Rollout line numbers: L17, L27, L34, L41, L48, L55, L64, L71, L78.

---

### 2.2 Block 07: Candidate Arm 3 (`task_b_remember_me_block_07_v154_arm3`)
- **Rollout File:** `~/.codex/sessions/2026/10/29/rollout-2026-10-29T19-07-19-01a187e1-1ad8-7240-8423-44e8c283c0c3.jsonl`
- **Total Turns:** 11 turns
- **Configured Ceiling:** 250,000 tokens
- **Final Outcome:** `TOKEN_CEILING_BREACHED` (270,506 tokens)

| Turn | Request / Response ID | Dispatch Timestamp | Streaming Estimate Visible to Runner | Authoritative Usage Event Timestamp | Authoritative Turn Delta ($\Delta_{\text{auth}}$) | Authoritative Cumulative ($T_{\text{auth}}$) | Visibility to Runner Timestamp | Estimation Error | Observation Lag | Next Request Dispatched Before Guard Observed? |
| :---: | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **1** | `resp_0de14df1458eb60f016ae34c1f2108819183c695b1b0a92fee` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T13:37:26.928Z` | 15,914 | 15,914 | Post-run (`13:39:02Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 2) |
| **2** | `resp_0de14df1458eb60f016ae34c22d738819194df5135a3510344` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T13:37:29.255Z` | 16,483 | 32,397 | Post-run (`13:39:02Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 3) |
| **3** | `resp_0de14df1458eb60f016ae34c24d89081918d84e31999baffa7` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T13:37:32.128Z` | 16,674 | 49,071 | Post-run (`13:39:02Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 4) |
| **4** | `resp_0de14df1458eb60f016ae34c407594819194d4b88ca674fed7` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T13:38:00.099Z` | 21,480 | 70,551 | Post-run (`13:39:02Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 5) |
| **5** | `resp_0de14df1458eb60f016ae34c47f51081919a1619a5c98b2fa1` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T13:38:08.139Z` | 24,404 | 94,955 | Post-run (`13:39:02Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 6) |
| **6** | `resp_0de14df1458eb60f016ae34c50045c819195db9d9bcf7fb88f` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T13:38:19.895Z` | 26,812 | 121,767 | Post-run (`13:39:02Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 7) |
| **7** | `resp_0de14df1458eb60f016ae34c5a5b7c81918262b44e6381ddb4` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T13:38:27.839Z` | 29,513 | 151,280 | Post-run (`13:39:02Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 8) |
| **8** | `resp_0de14df1458eb60f016ae34c6b32c48191954f311cd708eec3` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T13:38:41.352Z` | 29,625 | 180,905 | Post-run (`13:39:02Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 9) |
| **9** | `resp_0de14df1458eb60f016ae34c71d27c81919f617109cb8a4b2e` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T13:38:47.949Z` | 29,736 | 210,641 | Post-run (`13:39:02Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 10) |
| **10** | `resp_0de14df1458eb60f016ae34c7884b08191b98455c026fb8ae3` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T13:38:54.729Z` | 29,863 | 240,504 | Post-run (`13:39:02Z`) | `UNKNOWN` | `UNKNOWN` | `YES` (Auto-dispatched Turn 11) |
| **11** | `resp_0de14df1458eb60f016ae34c7f45a08191a09a12bc54f83dac` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T13:39:01.965Z` | 30,002 | 270,506 | Post-run (`13:39:02Z`) | `UNKNOWN` | `UNKNOWN` | `N/A` (Harness terminated after breach) |

*Evidence Citation:* Rollout line numbers: L17, L23, L30, L37, L44, L53, L62, L68, L74, L80, L87.

---

### 2.3 Block 08: Candidate Arm 3 (`task_b_remember_me_block_08_v154_arm3`)
- **Rollout File:** `~/.codex/sessions/2026/10/29/rollout-2026-10-29T12-46-05-01a18684-12ec-7e42-b098-386e03b049a2.jsonl`
- **Total Turns:** 16 turns
- **Configured Ceiling:** 350,000 tokens
- **Final Outcome:** `TOKEN_CEILING_BREACHED` (365,176 tokens)

| Turn | Request / Response ID | Dispatch Timestamp | Streaming Estimate Visible to Runner | Authoritative Usage Event Timestamp | Authoritative Turn Delta ($\Delta_{\text{auth}}$) | Authoritative Cumulative ($T_{\text{auth}}$) | Visibility to Runner Timestamp | Estimation Error | Observation Lag | Next Request Dispatched Before Guard Observed? |
| :---: | :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **1** | `resp_0d7e64cf568cfcc8016ae2f2c47bd88191b97057bab1b40b14` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:16:12.170Z` | 16,383 | 16,383 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **2** | `resp_0d7e64cf568cfcc8016ae2f2e7b66081919620dc0e648c7796` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:16:46.157Z` | 16,451 | 32,834 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **3** | `resp_0d7e64cf568cfcc8016ae2f2f43cb481919f5fcc6c00aec0b1` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:16:58.277Z` | 16,524 | 49,358 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **4** | `resp_0d7e64cf568cfcc8016ae2f3006fac8191a49f37e98ee2cbcd` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:17:11.561Z` | 16,681 | 66,039 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **5** | `resp_0d7e64cf568cfcc8016ae2f31094188191aafe1fb114818a02` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:17:27.086Z` | 18,048 | 84,087 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **6** | `resp_0d7e64cf568cfcc8016ae2f31cf2008191ae85ae5bacf758fd` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:17:40.144Z` | 20,456 | 104,543 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **7** | `resp_0d7e64cf568cfcc8016ae2f32d895481918987036cc74b85a1` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:17:58.620Z` | 22,152 | 126,695 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **8** | `resp_0d7e64cf568cfcc8016ae2f33d12f0819186cb929e55abfdbb` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:18:12.257Z` | 24,775 | 151,470 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **9** | `resp_0d7e64cf568cfcc8016ae2f34de6f08191a1bcf1e9762fb89b` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:18:32.652Z` | 26,118 | 177,588 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **10**| `resp_0d7e64cf568cfcc8016ae2f372adc8819192fb557f3bc47820` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:19:04.656Z` | 26,185 | 203,773 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **11**| `resp_0d7e64cf568cfcc8016ae2f37ed01c81918d63ddfab56583e9` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:19:16.987Z` | 26,259 | 230,032 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **12**| `resp_0d7e64cf568cfcc8016ae2f38343ec81919db3185bcfaeb3ca` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:19:21.848Z` | 26,421 | 256,453 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **13**| `resp_0d7e64cf568cfcc8016ae2f393f5c081919b533d9f0ae24bd5` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:19:38.058Z` | 26,977 | 283,430 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **14**| `resp_0d7e64cf568cfcc8016ae2f39a3c788191b73ee9a973fd6438` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:19:44.681Z` | 27,089 | 310,519 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **15**| `resp_0d7e64cf568cfcc8016ae2f3a0cd688191b46a90ab5d65a3f9` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:19:51.145Z` | 27,225 | 337,744 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `YES` |
| **16**| `resp_0d7e64cf568cfcc8016ae2f3a746348191b62043d9beeeaf52` | `UNKNOWN` | `UNKNOWN` | `2026-10-29T07:19:58.193Z` | 27,432 | 365,176 | Post-run (`07:19:58Z`) | `UNKNOWN` | `UNKNOWN` | `N/A` (Harness terminated after breach) |

*Evidence Citation:* Rollout line numbers: L17, L23, L29, L35, L43, L50, L59, L66, L75, L81, L87, L94, L100, L106, L112, L120.

---

## 3. Forensic Reconstruction: Block 07 Turn 10 to Turn 11 Overrun

### 3.1 What Actually Happened Under the Historical Harness
The historical run `task_b_remember_me_block_07_v154_arm3` executed under [`runner.py` at commit e341108](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py):
1. **Turn 10 Completion (`13:38:54.729Z`):**  
   Turn 10 finished with 29,795 input tokens and 68 output tokens ($\Delta = 29,863$ tokens), bringing cumulative tokens to **240,504**. The configured ceiling was **250,000**. Headroom remaining was **9,496 tokens**.
2. **Harness State:**  
   The runner was blocked synchronously in `proc.communicate(timeout=self.timeout_seconds)`. It had **no polling loop**, **no token listener**, and evaluated ceiling limits only after process completion.
3. **Turn 11 Dispatch (`~13:38:59.786Z`):**  
   At `13:38:59.759Z`, tool output from Turn 10 was received. At `13:38:59.786Z`, Codex logged `token_count` (`total_tokens: 240,504`). Immediately thereafter, Codex CLI formatted the conversational history and dispatched Turn 11 to the OpenAI API with an input prompt context of **29,918 tokens**.
4. **Remote Execution:**  
   The OpenAI API executed Turn 11 without knowledge of local client ceilings.
5. **Turn 11 Completion (`13:39:01.965Z`):**  
   OpenAI returned 29,918 input tokens and 84 output tokens ($\Delta = 30,002$ tokens), bringing total cumulative tokens to **270,506**.
6. **Breach Handling:**  
   The process exited or was terminated, and the orchestrator parsed the session rollout, flagged $270,506 > 250,000$, and marked `execution_status: TOKEN_CEILING_BREACHED`.

### 3.2 What Would Occur Under Remediation Code (`StreamingTokenTracker`)
If the uncommitted remediation implementation in `runner.py` had been active:
1. At `13:38:54.729Z`, the rollout tailer would observe Turn 10 completion at **240,504 cumulative tokens**.
2. Remaining headroom calculation:
   $$\text{remaining} = 250,000 - 240,504 = 9,496\text{ tokens}$$
3. Next-turn cost estimation:
   $$\text{estimated} = \max(\text{last\_turn\_input\_tokens}, \text{conservative\_reserve}) = \max(29,795, 15,000) = 29,795\text{ tokens}$$
4. Pre-dispatch check on line 205:
   $$\text{has\_headroom} = (9,496 \ge 29,795) \implies \mathbf{FALSE}$$
5. **Outcome:** `pre_dispatch_guard_triggered` would become `True`. The runner would execute `terminate_pid_tree(proc.pid)` at **240,504 cumulative tokens**, stopping execution with `stop_reason = "PRE_DISPATCH_HEADROOM_EXHAUSTED"` before Turn 11 could be dispatched.

---

## 4. Evidence Limitations and Residual Concurrency Risks

1. **Absence of Dispatch Timestamps:**  
   Neither `events.jsonl` nor `rollout-*.jsonl` records the exact millisecond when the Codex binary initiated the outbound HTTP request to OpenAI. Network dispatch latency is strictly internal to the child process. Any assertion of an exact sub-50ms race window is `UNKNOWN`.
2. **External Polling Concurrency Risk:**  
   As documented in [`runner.py` lines 537–542](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/runner.py#L537-L542), an external poller sampling at 50ms intervals cannot provide a mathematically guaranteed hard ceiling. If an agent initiates an outbound HTTP request within the polling window before the poller reads the latest file append, an outbound request may already be in flight when the process tree is killed. A true hard guarantee requires synchronous pre-request hooks inside the Codex binary itself.
