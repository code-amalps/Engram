# Exploration Tool Call Breakdown Before First Edit (Blocks 06, 07, 08)

**Document Identifier:** `ENGRAM_DRAFT05_EXPLORATION_BREAKDOWN`  
**Governing Context:** Engram Amendment 05 Pass 4 Offline Feasibility Investigation  
**Authoritative Evidence:** Native `events.jsonl` and session rollouts in `benchmarks/matched-pair-harness/reports/runs/`  

---

## 1. Methodology and Categorization Criteria

### 1.1 Category Definitions
Tool calls issued prior to the first confirmed file edit are partitioned into four standardized categories:
1. **File Reads:** Retrieval of file content (`Get-Content`, `Get-Content -Raw`, `cat`).
2. **Directory Listings / Git State:** Repository status and tree inspection (`git status --short`, directory queries).
3. **Searches:** Substring, pattern, and regex queries (`rg -n`, `grep`).
4. **Engram MCP Calls:** Semantic index or AST symbol queries (`engram_inspect_symbol`, `engram_query_context`).

### 1.2 First-Edit Timing Verification
In strict accordance with evidence-integrity requirements:
- A "first edit" is counted **only** upon the first successful write to a task source file in the isolated workspace, verified by a `file_change` completion event and subsequent non-empty diff.
- **Block 06 Baseline Arm 1:** First edit occurred at **Turn 7** (`item_10`, `file_change` modifying `core.module.ts`).
- **Block 07 Candidate Arm 3:** **NO EDIT OCCURRED.** The agent authored zero file modifications across all 11 turns. All 11 turns are classified as pre-edit exploration.
- **Block 08 Candidate Arm 3:** First edit occurred at **Turn 9** (`item_11`, `file_change` modifying `core.module.ts`).

### 1.3 Token Attribution Method
Because the OpenAI API reports token usage per model request (turn) rather than per shell sub-process:
- When a turn dispatches a single command, that turn's incremental token delta ($\Delta$) is attributed directly to that category.
- When a turn executes a compound command (e.g. `git status; rg ...`), the turn's tokens are attributed to the joint category `Directory Listing & Search`. Fine-grained internal token splits between the two sub-commands are marked `UNKNOWN`.

---

## 2. Block 06: Baseline Arm 1 (`baseline_arm1`)

- **Cumulative Tokens Before First Edit:** **145,999 tokens** (Turns 1–6)
- **First Edit Event:** Turn 7 (`item_10`, `core.module.ts`), cumulative tokens: 180,866 tokens

| Turn | Item ID | Tool Command / Invocation | Category | Incremental Tokens ($\Delta$) | Cumulative Tokens |
| :---: | :---: | :--- | :--- | :---: | :---: |
| **Turn 1** | `item_3` | `Get-Content projects/.../core.module.ts` | File Read | 13,457 | 13,457 |
| **Turn 2** | `item_4` | `Get-Content projects/.../core.module.ts -Raw` | File Read | 21,828 | 35,285 |
| **Turn 3** | `item_5` | `rg -n "remember\|login\|authLogin..."` | Search | 23,084 | 58,369 |
| **Turn 4** | `item_6` | `Get-Content app.component.ts; local-storage...` | File Read | 25,468 | 83,837 |
| **Turn 5** | `item_7` | `git status --short; rg -n "CoreModule..."` | Dir Listing & Search | 30,132 | 113,969 |
| **Turn 6** | `item_8` | `Get-Content core.module.spec.ts ...` | File Read | 32,030 | 145,999 |

### Block 06 Category Summary:
- **File Reads (4 calls):** $13,457 + 21,828 + 25,468 + 32,030 = \mathbf{92,783\text{ tokens}}$ ($63.55\%$ of pre-edit exploration)
- **Searches (1 pure call):** $\mathbf{23,084\text{ tokens}}$ ($15.81\%$)
- **Directory Listing & Compound Search (1 compound call):** $\mathbf{30,132\text{ tokens}}$ ($20.64\%$)
- **Engram MCP Calls:** **0 calls** ($0\text{ tokens}$)

---

## 3. Block 07: Candidate Arm 3 (`v154_arm3`)

- **Cumulative Tokens Before First Edit:** **270,506 tokens** (Entire 11-turn run; no edits made)
- **First Edit Event:** `NONE`

| Turn | Item ID | Tool Command / Invocation | Category | Incremental Tokens ($\Delta$) | Cumulative Tokens |
| :---: | :---: | :--- | :--- | :---: | :---: |
| **Turn 1** | `item_2` | Initial preflight (Agent message) | Preflight Analysis | 15,914 | 15,914 |
| **Turn 2** | `item_3` | `engram_inspect_symbol(core.module.ts)` | Engram MCP (Failed) | 16,483 | 32,397 |
| **Turn 3** | `item_4` | `rg -n "remember\|login..."` | Search | 16,674 | 49,071 |
| **Turn 4** | `item_5` | `Get-Content core.module.ts` | File Read | 21,480 | 70,551 |
| **Turn 5** | `item_6` | `Get-Content app.component.ts ...` | File Read | 24,404 | 94,955 |
| **Turn 6** | `item_7` | Context reflection (Agent message) | Analysis / Reflection | 26,812 | 121,767 |
| **Turn 7** | `item_8` | `git status --short; rg -n "auth..."` | Dir Listing & Search | 29,513 | 151,280 |
| **Turn 8** | `item_9` | Diagnostic evaluation (Agent message) | Analysis / Reflection | 29,625 | 180,905 |
| **Turn 9** | `item_10`| `npx tsc -p .../tsconfig.app.json` | Diagnostic Typecheck | 29,736 | 210,641 |
| **Turn 10**| `item_11`| Analysis of compilation diagnostics | Analysis / Reflection | 29,863 | 240,504 |
| **Turn 11**| `item_12`| Additional shell inspection (Halted) | Shell Exploration | 30,002 | 270,506 |

### Block 07 Category Summary:
- **Engram MCP Calls (1 attempted call):** $\mathbf{16,483\text{ tokens}}$ (Turn 2 request cost)
- **File Reads (2 calls):** $21,480 + 24,404 = \mathbf{45,884\text{ tokens}}$
- **Searches (1 pure call):** $\mathbf{16,674\text{ tokens}}$
- **Directory Listing & Compound Search (1 compound call):** $\mathbf{29,513\text{ tokens}}$
- **Diagnostic Typecheck (1 compiler run):** $\mathbf{29,736\text{ tokens}}$
- **Model Reflection / Plan Reasoning (4 turns):** $15,914 + 26,812 + 29,625 + 29,863 + 30,002 = \mathbf{132,216\text{ tokens}}$

---

## 4. Block 08: Candidate Arm 3 (`v154_arm3`)

- **Cumulative Tokens Before First Edit:** **151,470 tokens** (Turns 1–8)
- **First Edit Event:** Turn 9 (`item_11`, `tools.apply_patch` on `core.module.ts`), cumulative tokens: 177,588 tokens

| Turn | Item ID | Tool Command / Invocation | Category | Incremental Tokens ($\Delta$) | Cumulative Tokens |
| :---: | :---: | :--- | :--- | :---: | :---: |
| **Turn 1** | `item_3` | `Get-Content core.module.ts` | File Read | 16,383 | 16,383 |
| **Turn 2** | `item_3` | Subprocess wait / stdout collection | Process Polling | 16,451 | 32,834 |
| **Turn 3** | `item_3` | Subprocess wait / stdout collection | Process Polling | 16,524 | 49,358 |
| **Turn 4** | `item_4` | `Get-Content core.module.ts -Raw` | File Read | 16,681 | 66,039 |
| **Turn 5** | `item_5` | `rg -n "remember\|login\|authLogin..."` | Search | 18,048 | 84,087 |
| **Turn 6** | `item_6` | `Get-Content app.component.ts; local-storage...` | File Read | 20,456 | 104,543 |
| **Turn 7** | `item_8` | `git status --short; rg -n "CoreModule..."` | Dir Listing & Search | 22,152 | 126,695 |
| **Turn 8** | `item_9` | `Get-Content auth.effects.ts; core.state.ts` | File Read | 24,775 | 151,470 |

### Block 08 Category Summary:
- **File Reads (4 distinct read commands):** $16,383 + 16,681 + 20,456 + 24,775 = \mathbf{78,295\text{ tokens}}$ ($51.69\%$ of pre-edit exploration)
- **Subprocess Polling Overhead (2 wait turns):** $16,451 + 16,524 = \mathbf{32,975\text{ tokens}}$ ($21.77\%$)
- **Searches (1 pure call):** $\mathbf{18,048\text{ tokens}}$ ($11.92\%$)
- **Directory Listing & Compound Search (1 compound call):** $\mathbf{22,152\text{ tokens}}$ ($14.62\%$)
- **Engram MCP Calls:** **0 calls** ($0\text{ tokens}$)

---

## 5. Comparative Exploration Analysis

| Metric | Block 06 Baseline | Block 07 Candidate | Block 08 Candidate | Cross-Run Finding |
| :--- | :---: | :---: | :---: | :--- |
| **Pre-Edit Exploration Tokens** | 145,999 tokens | 270,506 tokens | 151,470 tokens | Minimum baseline exploration is ~146k–151k |
| **Exploration Share of Total Run**| 57.54% | 100.00% (or `UNKNOWN`) | 41.48% | Exploration dominates initial context |
| **Primary Tool Used** | PowerShell `Get-Content` | Native shell fallback | PowerShell `Get-Content` | File reading consumes $>50\%$ of exploration |
| **Engram MCP Contribution** | 0% (Disabled) | 6.09% (1 call, failed) | 0% (Ignored) | MCP retrieval was never successfully utilized |
| **Redundant Reads** | 4 multi-file reads | 2 multi-file reads | 4 multi-file reads | Repeatedly read `core.module.ts` and `app.component.ts` |
