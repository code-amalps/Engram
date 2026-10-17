# Engram v1.5.1 Release Packaging Checklist & Gate Procedures

**Document Identifier:** `ENGRAM-V151-PACKAGING-CHECKLIST-20261017`  
**Target Path:** `docs/v1_5_1_release_packaging_checklist.md`  
**Target Engine Release:** `v1.5.1`  
**Target Protocol Specification:** Protocol `1.1` (`PROTOCOL_VERSION = "1.1"`)  
**Target Branch:** `main`  
**Execution Posture:** PRE-RELEASE LOCAL VALIDATION & CHECKLIST  

---

## 1. Release Invariants & Safety Rules

1. **Independent Gates:** No release step may proceed until its predecessor gate is fully verified and documented.
2. **Explicit Staging Only:** Never execute `git add .` or `git add -A`. Every staged file must be explicitly enumerated.
3. **No Working-Tree Pollution:** The 46 unstaged formatting files and 8 untracked B6 empirical research files must remain strictly preserved and untouched.
4. **No Version Downgrade:** Engine package version is locked at `1.5.1` (delivering Protocol `1.1`). Historical tags (`v1.1.0` through `v1.5.0`) must remain immutable.
5. **Separate Authorizations:** Release commit, local validation, tag creation, push, and publication require distinct, explicit authorizations.

---

## 2. Release Gates Matrix

### Gate 1: Fix Inclusion & Hosted CI Re-Verification
*Status: PASS (Verified)*
- [x] Confirm fix commit `a26a01a45fecc33e37110a2fc3350013348c801f` is HEAD of `main`.
- [x] Confirm hosted CI run `39017318962` on `main` passed across Ubuntu, macOS, and Windows.

---

### Gate 2: Version Manifest Consistency
*Status: PASS (Verified)*
- [x] Confirm `Cargo.toml` engine package version is `1.5.1`.
- [x] Confirm `Cargo.lock` dependency package version is `1.5.1`.
- [x] Confirm `extensions/vscode/package.json` extension version is `1.5.1`.
- [x] Confirm `extensions/vscode/package-lock.json` extension version is `1.5.1`.
- [x] Confirm wire protocol remains `PROTOCOL_VERSION = "1.1"`.

---

### Gate 3: Working-Tree Audit & B6 Exclusion Verification
*Status: PASS (Verified)*
- [x] Verify that untracked B6 empirical research files remain strictly untracked.
- [x] Confirm that only the 6 approved release files are staged.

---

### Gate 4: Dedicated Release Commit Authorization
*Status: AUTHORIZED*
- [x] Stage only explicitly authorized release files:
  - `Cargo.toml`
  - `Cargo.lock`
  - `extensions/vscode/package.json`
  - `extensions/vscode/package-lock.json`
  - `docs/v1_5_1_release_notes.md`
  - `docs/v1_5_1_release_packaging_checklist.md`
- [x] Commit message: `chore(release): prepare Engram v1.5.1 patch release`.

---

### Gate 5: Local Binary Compilation & Smoke Test
*Status: PENDING COMMIT*
- [ ] Build optimized release binary:
  ```powershell
  cargo build --release --bin engram
  ```
- [ ] Verify `.\target\release\engram.exe --version` outputs `engram 1.5.1`.
- [ ] Run `cargo test --test setup_integration_test`.

---

### Gate 6: VS Code Extension Packaging
*Status: PENDING GATE 5*
- [ ] Run extension compilation and packaging:
  ```powershell
  cd extensions/vscode
  npx vsce package
  ```
- [ ] Verify `engram-1.5.1.vsix` is created and version metadata is `1.5.1`.

---

### Gate 7: Tag & Push Authorization
*Status: NOT AUTHORIZED — PENDING SEPARATE APPROVAL*
- [ ] Request human approval for tag creation:
  ```powershell
  git tag -a v1.5.1 -m "Engram v1.5.1 — MCP Health-Check Protocol 1.1 Alignment"
  ```
- [ ] Request human approval to push `main` and `v1.5.1` tag to `origin`.

---

### Gate 8: Hosted Release Workflow & Attestation Audit
*Status: NOT AUTHORIZED — PENDING GATE 7*
- [ ] Monitor GitHub Actions `release.yml` for multi-platform compilation.
- [ ] Verify `SHA256SUMS` and `release-manifest.json`.
- [ ] Verify Sigstore provenance attestations in public Rekor log.
- [ ] Attach `extensions/vscode/engram-1.5.1.vsix` to GitHub Release `v1.5.1`.
