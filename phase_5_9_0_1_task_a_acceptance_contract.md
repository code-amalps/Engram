# Engram Phase 5.9.0.1: Task A Functional Acceptance Specification & Contract

**Phase:** 5.9.0.1  
**Task Identifier:** `task_a_typo`  
**Task Fixture:** `benchmarks/matched-pair-harness/config/tasks/task_a_typo.json`  
**Evaluator Implementation:** `benchmarks/matched-pair-harness/src/evaluator.py`  
**Execution Mode:** Strictly Offline Specification & Verification  

---

## 1. Path & Task Identity Discrepancy Resolution

Phase 5.9.0 documentation erroneously described Task A as a typo fix in `src/environments/environment.prod.ts` (`prdo: true` $\to$ `prod: true`).
Forensic inspection of the authoritative task fixture [task_a_typo.json](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/config/tasks/task_a_typo.json) and [evaluator.py](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/benchmarks/matched-pair-harness/src/evaluator.py) reveals the true, verified task definition:
- **Actual Task Title:** "Fix Capitalization Typo in About Component Header"
- **Actual Target File:** `projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html`
- **Original Baseline Text:** `Angular Cli Tips`
- **Expected Corrected Text:** `Angular CLI Tips`

The `environment.prod.ts` reference was a legacy artifact from an earlier synthetic evaluation and does not exist in the Angular CLI starter repository.

---

## 2. Objective Acceptance Contract

The harness enforces a three-tier acceptance evaluation for Task A:

```
+-----------------------------------------------------------------------------------+
| Task A Evaluation Gates (evaluator.py)                                            |
+--------------------------+------------------------------+-------------------------+
| Tier 1: Static Content   | Tier 2: Blast Radius (Diff)  | Tier 3: Compiler Gate   |
| (REQ-1: Weight 0.50)     | (REQ-2: Weight 0.50)         | (L0_typecheck: Gate)    |
+--------------------------+------------------------------+-------------------------+
| 'Angular CLI Tips' in    | Only about.component.html    | npx tsc --noEmit        |
| HTML without comments;   | modified; zero edits to      | passes without errors   |
| 'Angular Cli Tips' gone  | core, manifests, or modules  | (exit code 0)           |
+--------------------------+------------------------------+-------------------------+
```

### 2.1 Specification Details

1. **Exact Expected Target File:**  
   `projects/angular-ngrx-material-starter/src/app/features/about/about/about.component.html`  
   (Resolved safely via `resolve_safe_target_path(sandbox_path, rel_file)` to prevent path traversal).
2. **Exact Expected Change:**  
   The anchor/header label must read `Angular CLI Tips` (acronym uppercase) instead of `Angular Cli Tips`.
3. **Comment Stripping & False Positive Prevention:**  
   `evaluator.py` executes:
   ```python
   uncommented = re.sub(r'<!--.*?-->', '', content, flags=re.DOTALL)
   ```
   This ensures that commented-out text cannot satisfy or fail the requirement.
4. **Whitespace Tolerance & Case Sensitivity:**  
   ```python
   norm_content = re.sub(r'\s+', ' ', uncommented)
   norm_exp = re.sub(r'\s+', ' ', exp_text)
   norm_orig = re.sub(r'\s+', ' ', orig_text)
   return (norm_exp in norm_content) and (norm_orig not in norm_content)
   ```
   - **Case-sensitive:** `norm_exp in norm_content` strictly requires uppercase `CLI`.
   - **Whitespace-tolerant:** Collapses arbitrary tabs, linebreaks, and multiple spaces to a single space.
5. **Blast Radius & Boundary Check (`REQ-2`):**  
   - `git_impact.dependency_mutated == False` (zero changes to `package.json`, `package-lock.json`, or `node_modules`).
   - `git_impact.unrelated_files_count == 0` (zero changes to any file outside `allowed_file_patterns`).
   - `git_impact.files_changed_count > 0` (prevents false positive on unchanged repositories).
6. **Compiler Typecheck Gate (`L0_typecheck`):**  
   Executes `npx tsc -p projects/angular-ngrx-material-starter/tsconfig.app.json --noEmit`. Must exit with code 0.

---

## 3. Automated Verification Script Reference

The acceptance check is fully automated in `evaluator.py`:
- Requirement Score: $\text{Score} = (0.50 \times \text{REQ-1}) + (0.50 \times \text{REQ-2})$
- Correctness Verdict: `PASS` if and only if $\text{Score} == 1.0$ AND `L0_typecheck` == `PASS`.
- Any unexpected modification to `package.json` instantly marks all requirements `FAIL` (`git_impact.dependency_mutated`).
