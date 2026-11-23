# Engram v1.5.5 Candidate 1 Packaging & Verification Report

**Target Package:** `engram-preview-v1.5.5-cand1-windows-x64.zip`  
**Execution Date:** 2026-11-23  
**Verified Git Commit:** `ded9ca521024d18248235a33e2e292964b79f9b0`  
**Working Tree State:** Clean (`dirty: false`)  
**Package Verification Status:** **PASSED & CERTIFIED**  

---

## 1. Candidate Build & Artifact Cryptographic Integrity

The release candidate executable was compiled on Windows MSVC under the optimized release profile with `-C link-arg=/BREPRO`:

```powershell
engram.exe --version
```
```text
engram 1.5.5
commit: ded9ca521024d18248235a33e2e292964b79f9b0
dirty: false
rustc: rustc 1.99.0 (b940084d7 2026-09-28)
profile: release
```

### Cryptographic Hashes:
| Artifact | Relative Path | Size (Bytes) | SHA-256 Checksum |
|---|---|---|---|
| **Release Binary** | `bin/engram.exe` | 19,072,512 | `440845D342B68DBB1D7872F5B78D38FEA81D69D1F549CF48CDB540B80DA885E9` |
| **Release Archive** | `releases/engram-preview-v1.5.5-cand1-windows-x64.zip` | ~4.4 MB | `DD67AB9FF391182373E3AD17B2C40929206E13F33E13D4A2E77BEC0596A730BF` |

---

## 2. Archive Contents Verification

Using `tar -tf` on `engram-preview-v1.5.5-cand1-windows-x64.zip`, the archive structure was verified:
```text
bin/engram.exe
config/config.example.yaml
config/constitution.example.yaml
sample_workspace/.engram/config.yaml
sample_workspace/.engram/constitution.yaml
sample_workspace/.engram/events/000001_evt_45ef22c702c24.json
sample_workspace/.engram/events/events.jsonl
sample_workspace/src/calculator.py
sample_workspace/src/__init__.py
sample_workspace/tests/test_calculator.py
sample_workspace/tests/__init__.py
LICENSE
README.md
RELEASE_NOTES.md
```
All required files, documentation, sample workspaces, and the GNU Affero General Public License v3 text are present.

---

## 3. Clean Installation Verification

The ZIP archive was expanded into an isolated temporary directory (`$env:TEMP\engram_install_verify_*`):
1. **Version Check:**
   - Ran `engram.exe --version`.
   - Output matched declared version `1.5.5`, commit `ded9ca5`, and `dirty: false`.
2. **Invalid Argument Rejection:**
   - Executed `engram.exe --definitely-invalid-arg`.
   - Returned exit code `2` with clap error diagnostics, verifying fail-closed CLI argument parsing.
3. **Fresh Workspace Lifecycle:**
   - Initialized a brand new git repository with Python file `app.py`.
   - Executed `engram.exe init`: successfully created `.engram/` metadata and SQLite cache.
   - Executed `engram.exe analyze`: indexed workspace symbols and committed to WAL storage.
   - Executed `engram.exe context "test math addition"`: compiled task context and evaluated Project Constitution rules with zero failures.

---

## 4. Upgrade Path Verification (from v1.5.4-cand5)

The sample workspace from the certified `v1.5.4-cand5` release was extracted into a fresh directory:
1. **Metadata Reading:**
   - The `v1.5.5-cand1` binary read the existing `v1.5.4` `.engram/` configuration, constitution, and historical event log (`000001_evt_45ef22c702c24.json`) without errors.
2. **Analysis & Grounding:**
   - Executed `engram.exe analyze`: updated AST symbols seamlessly without schema conflict.
   - Executed `engram.exe context "add multiplication"`: grounded `tests/test_calculator.py` and `src/calculator.py` with `HIGH` confidence (score 60.95, Level 3).
3. **No Migration Required:**
   - Confirmed full backward compatibility with zero manual migration steps.

---

## 5. Verification Sign-Off Verdict

Engram v1.5.5 Candidate 1 (`v1.5.5-cand1`) satisfies all release engineering criteria:
- **Build Quality:** Clean release build (`dirty: false`, MSVC x86_64, `/BREPRO`).
- **Cryptographic Evidence:** Documented in candidate manifest and release registers.
- **Operational Reliability:** Verified in clean install and upgrade scenarios.
- **Verdict:** **`READY_FOR_CANDIDATE_DEPLOYMENT`**
