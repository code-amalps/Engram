# Engram Phase 5.4.31: License Findings & AGPL-3.0 Compliance Investigation

**Phase:** 5.4.31  
**Scope:** Intellectual Property Review, Root License vs Upstream Dependency Compatibility  
**Repository:** `C:\Users\Amal P S\OneDrive\Projects\Engram`  
**Baseline Git Commit:** `7106a2f369fe79faf5f95449378a900ab8e8eddf`  

---

## 1. Executive Summary & Legal Hygiene Findings

This investigation addressed the relationship between the repository's root license (`AGPL-3.0-or-later`), its 114 external dependencies (100% permissively licensed), and binary distribution obligations.

### Core Determinations:
1. **Root License Identity:** Engram itself is explicitly licensed under the GNU Affero General Public License v3.0 or later (`AGPL-3.0-or-later`), as declared in `Cargo.toml` (`license = "AGPL-3.0-or-later"`) and substantiated by the 35,652-byte root `LICENSE` file.
2. **Zero Upstream Copyleft:** Not a single external dependency is licensed under AGPL, GPL, LGPL, or MPL. All 114 external packages utilize permissive open-source licenses:
   - 92 packages: Dual-licensed `MIT / Apache-2.0`
   - 16 packages: `MIT` only
   - 4 packages: `Apache-2.0` only
   - 1 package: `BSD-2-Clause / Apache-2.0 / MIT` (`zerocopy`)
   - 1 package: `MIT / Unlicense` (`aho-corasick`, `walkdir`, `same-file`, `memchr`, `ignore`)
3. **Compatibility Analysis:**
   - Permissive licenses (MIT, Apache-2.0, BSD) are legally compatible with the AGPL-3.0. When compiled into a unified binary, the combined work may be distributed under the terms of the AGPL-3.0.
   - The permissive licenses do not conflict with or restrict the copyleft provisions of AGPL-3.0.
4. **Distribution Obligations & Remediation Gap (Finding LIC-02):**
   - Both MIT and BSD licenses require that their original copyright notices and permission notices accompany all substantial portions of the software or binary distributions.
   - While source distribution via Git satisfies this by retaining upstream crate manifests, distributing standalone compiled binaries (`engram.exe`) or Windows installers requires bundling an aggregated notices file.
   - Currently, the repository root contains `LICENSE` (AGPL-3.0) and `PRIVACY_NOTICE.txt`, but lacks a consolidated `NOTICE` or `THIRD_PARTY_LICENSES.md` file.

---

## 2. Dependency License Matrix

| Upstream License | Total Package Instances | AGPL-3.0 Ingestion Compatibility | Binary Attribution Requirement |
|---|---|---|---|
| **MIT / Apache-2.0** | 92 | **Compatible** | Retain copyright notice & permission notice |
| **MIT** | 16 | **Compatible** | Retain copyright notice & permission notice |
| **Apache-2.0** | 4 | **Compatible** | Retain Apache notice & state changes |
| **BSD-2-Clause / BSD-3-Clause** | 1 | **Compatible** | Retain 2/3-clause BSD copyright disclaimer |
| **MIT / Unlicense** | 1 | **Compatible** | Dedicated to public domain / MIT notice |
| **GPL / AGPL / LGPL (External)** | **0** | N/A | No external copyleft obligations |

---

## 3. Recommended Remediation Action

Prior to publishing public binary release archives (in Phase 5.7):
1. Create a consolidated `THIRD_PARTY_LICENSES.md` in the release archive using `cargo-about` or a deterministic license extraction script.
2. Bundle `THIRD_PARTY_LICENSES.md` into the InnoSetup installer alongside `PRIVACY_NOTICE.txt`.
3. Note for legal review: Any cloud or network-hosted version of Engram will trigger AGPL Section 13 ("Remote Network Interaction; Providing Corresponding Source"). Because Engram is distributed as a strictly local-first desktop binary that makes zero outbound network calls, Section 13 network copyleft does not impose server-side source-offering obligations on end users.
