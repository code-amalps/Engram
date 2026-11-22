# Engram Headroom Reserve Simulation Audit (Strictly Offline)

**Document Identifier:** `ENGRAM_DRAFT05_RESERVE_SIMULATION`  
**Governing Evaluation:** Retrospective Offline Replay of Historical Runs  
**Operational Ceiling Evaluated:** 175,000 Model Tokens per Arm  
**Candidate Reserves Tested:** 30,000 / 35,000 / 40,000 Tokens  
**Parent Directory:** `amendments/draft-05/`  
**Status:** Strictly Offline Simulation — No Code or Protocol Configuration Mutated  

---

## 1. Simulation Methodology & Guard Models

We evaluate the behavior of three candidate headroom reserves against the observed historical turn sequences of Task B Remember Me under a nominal per-arm operational ceiling of **175,000 tokens**:
- **Option 1: 30,000 Reserve** $\implies$ Nominal Stop Trigger: **145,000 tokens**
- **Option 2: 35,000 Reserve** $\implies$ Nominal Stop Trigger: **140,000 tokens**
- **Option 3: 40,000 Reserve** $\implies$ Nominal Stop Trigger: **135,000 tokens**

Two guard evaluation models are simulated:
1. **Model A (Static Cumulative Trigger):** The runner halts execution after Turn $N$ if cumulative tokens $T_{\text{cum}} \ge (175,000 - \text{Reserve})$.
2. **Model B (Dynamic Pre-Dispatch Headroom Guard):** Before dispatching Turn $N+1$, the runner checks:
   $$\text{Remaining Headroom} = 175,000 - T_{\text{cum}}$$
   $$\text{Estimated Turn Cost} = \max(\Delta_{N}, \text{Reserve})$$
   Execution halts if $\text{Remaining Headroom} < \text{Estimated Turn Cost}$.

---

## 2. Replay Simulation Across Historical Runs

### 2.1 Block 06: Baseline Arm 1
- **Historical First Edit:** **Turn 7** (Cumulative tokens at completion: **180,866 tokens**)
- **Turn Sequence:** 
  - Turn 1: $\Delta = 13,457 \implies T_{\text{cum}} = 13,457$
  - Turn 2: $\Delta = 21,828 \implies T_{\text{cum}} = 35,285$
  - Turn 3: $\Delta = 23,084 \implies T_{\text{cum}} = 58,369$
  - Turn 4: $\Delta = 25,468 \implies T_{\text{cum}} = 83,837$
  - Turn 5: $\Delta = 30,132 \implies T_{\text{cum}} = 113,969$
  - Turn 6: $\Delta = 32,030 \implies T_{\text{cum}} = 145,999$
  - Turn 7: $\Delta = 34,867 \implies T_{\text{cum}} = 180,866$ (First edit: 6 files modified)

#### Simulation Outcomes:
| Candidate Reserve | Nominal Stop Trigger | Stop Turn | Cumulative Tokens at Stop | Remaining Budget vs. Historical $\Delta_{7}$ (34,867) | Reached First Edit? |
| :---: | :---: | :---: | :---: | :---: | :---: |
| **30,000** | 145,000 | Before Turn 7 | **145,999 tokens** | Rem: $29,001 < 34,867 \implies$ **Prevented Overrun** | **NO** (Stopped at Turn 6) |
| **35,000** | 140,000 | Before Turn 7 | **145,999 tokens** | Rem: $29,001 < 34,867 \implies$ **Prevented Overrun** | **NO** (Stopped at Turn 6) |
| **40,000** | 135,000 | Before Turn 7 | **145,999 tokens** | Rem: $29,001 < 34,867 \implies$ **Prevented Overrun** | **NO** (Stopped at Turn 6) |

*Key Finding for B06:* Across all three reserves, the guard halts execution at **145,999 tokens** immediately before Turn 7. If Turn 7 were dispatched, consumption would reach **180,866 tokens**, breaching the 175,000 ceiling by 5,866 tokens. The guard successfully prevents the overrun, but **prevents the agent from ever reaching its first edit**.

---

### 2.2 Block 07: Candidate Arm 3
- **Historical First Edit:** **`NO EDIT OBSERVED`** (Entire 11 turns spent in pre-edit exploration)
- **Turn Sequence:**
  - Turn 1: $\Delta = 15,914 \implies T_{\text{cum}} = 15,914$
  - Turn 2: $\Delta = 16,483 \implies T_{\text{cum}} = 32,397$
  - Turn 3: $\Delta = 16,674 \implies T_{\text{cum}} = 49,071$
  - Turn 4: $\Delta = 21,480 \implies T_{\text{cum}} = 70,551$
  - Turn 5: $\Delta = 24,404 \implies T_{\text{cum}} = 94,955$
  - Turn 6: $\Delta = 26,812 \implies T_{\text{cum}} = 121,767$
  - Turn 7: $\Delta = 29,513 \implies T_{\text{cum}} = 151,280$
  - Turn 8: $\Delta = 29,625 \implies T_{\text{cum}} = 180,905$

#### Simulation Outcomes:
| Candidate Reserve | Nominal Stop Trigger | Stop Turn | Cumulative Tokens at Stop | Remaining Budget vs. Historical $\Delta_{8}$ (29,625) | Reached First Edit? |
| :---: | :---: | :---: | :---: | :---: | :---: |
| **30,000** | 145,000 | Before Turn 8 | **151,280 tokens** | Rem: $23,720 < 29,625 \implies$ **Prevented Overrun** | `NO EDIT OBSERVED` |
| **35,000** | 140,000 | Before Turn 8 | **151,280 tokens** | Rem: $23,720 < 29,625 \implies$ **Prevented Overrun** | `NO EDIT OBSERVED` |
| **40,000** | 135,000 | Before Turn 8 | **151,280 tokens** | Rem: $23,720 < 29,625 \implies$ **Prevented Overrun** | `NO EDIT OBSERVED` |

*Key Finding for B07:* The guard halts execution at **151,280 tokens** after Turn 7. If Turn 8 were dispatched, consumption would reach **180,905 tokens**, breaching the 175,000 ceiling. Zero files were edited.

---

### 2.3 Block 08: Candidate Arm 3
- **Historical First Edit:** **Turn 9** (Cumulative tokens at completion: **177,588 tokens**)
- **Turn Sequence:**
  - Turn 1: $\Delta = 16,383 \implies T_{\text{cum}} = 16,383$
  - Turn 2: $\Delta = 16,451 \implies T_{\text{cum}} = 32,834$
  - Turn 3: $\Delta = 16,524 \implies T_{\text{cum}} = 49,358$
  - Turn 4: $\Delta = 16,681 \implies T_{\text{cum}} = 66,039$
  - Turn 5: $\Delta = 18,048 \implies T_{\text{cum}} = 84,087$
  - Turn 6: $\Delta = 20,456 \implies T_{\text{cum}} = 104,543$
  - Turn 7: $\Delta = 22,152 \implies T_{\text{cum}} = 126,695$
  - Turn 8: $\Delta = 24,775 \implies T_{\text{cum}} = 151,470$
  - Turn 9: $\Delta = 26,118 \implies T_{\text{cum}} = 177,588$ (First edit: `core.module.ts`)

#### Simulation Outcomes:
| Candidate Reserve | Nominal Stop Trigger | Stop Turn | Cumulative Tokens at Stop | Remaining Budget vs. Historical $\Delta_{9}$ (26,118) | Reached First Edit? |
| :---: | :---: | :---: | :---: | :---: | :---: |
| **30,000** | 145,000 | Before Turn 9 | **151,470 tokens** | Rem: $23,530 < 26,118 \implies$ **Prevented Overrun** | **NO** (Stopped at Turn 8) |
| **35,000** | 140,000 | Before Turn 9 | **151,470 tokens** | Rem: $23,530 < 26,118 \implies$ **Prevented Overrun** | **NO** (Stopped at Turn 8) |
| **40,000** | 135,000 | Before Turn 9 | **151,470 tokens** | Rem: $23,530 < 26,118 \implies$ **Prevented Overrun** | **NO** (Stopped at Turn 8) |

*Key Finding for B08:* The guard halts execution at **151,470 tokens** after Turn 8. If Turn 9 were dispatched, consumption would reach **177,588 tokens**, breaching the 175,000 ceiling by 2,588 tokens. The guard successfully prevents the overrun, but **prevents the agent from authoring its first edit on `core.module.ts`**.

---

## 3. Comparative Synthesis & Architectural Implications

### 3.1 Summary Matrix

| Metric | Reserve: 30,000 (Trigger: 145,000) | Reserve: 35,000 (Trigger: 140,000) | Reserve: 40,000 (Trigger: 135,000) |
| :--- | :---: | :---: | :---: |
| **B06 Halted Cumulative Tokens** | 145,999 tokens | 145,999 tokens | 145,999 tokens |
| **B06 First Edit Reached?** | **NO** (First edit was at 180,866) | **NO** (First edit was at 180,866) | **NO** (First edit was at 180,866) |
| **B07 Halted Cumulative Tokens** | 151,280 tokens | 151,280 tokens | 151,280 tokens |
| **B07 First Edit Reached?** | `NO EDIT OBSERVED` | `NO EDIT OBSERVED` | `NO EDIT OBSERVED` |
| **B08 Halted Cumulative Tokens** | 151,470 tokens | 151,470 tokens | 151,470 tokens |
| **B08 First Edit Reached?** | **NO** (First edit was at 177,588) | **NO** (First edit was at 177,588) | **NO** (First edit was at 177,588) |
| **Overrun Successfully Blocked?** | **YES** across all 3 runs | **YES** across all 3 runs | **YES** across all 3 runs |
| **Task Feasibility Under 175k Cap** | **0% Completion Feasibility** | **0% Completion Feasibility** | **0% Completion Feasibility** |

### 3.2 The Fundamental Operational Dilemma
This simulation exposes the central tradeoff governing Amendment 05:
1. **Safety Enforcement:** A reserve between 30,000 and 40,000 tokens is highly effective at stopping execution before a typical 26k–37k turn can breach the 175,000 ceiling.
2. **Task Infeasibility:** Because Task B requires ~146k–151k tokens of context exploration before authoring the first patch, halting at 135k–145k ensures that **zero runs can ever edit a source file**. Under a 175,000 ceiling with safety reserves, the agent is halted purely as an exploration process.

---

## 4. Methodological Limitations of Retrospective Replay

> [!WARNING]
> 1. **Historical Replay vs. Live Dynamic Behavior:** Retrospective simulation assumes turn sizes in a future run will duplicate historical turns. Future turn sizes may be smaller or larger depending on agent exploration choices.
> 2. **External Polling Concurrency Risk:** A replay cannot verify that an external 50ms polling loop will intercept a real-time request before the network socket is established.
> 3. **Safety Fallacy:** Exceeding the historical maximum ($37,651$) does not guarantee future safety; future tool outputs or prompt context could generate turns exceeding 40,000 tokens.
