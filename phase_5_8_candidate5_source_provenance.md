# Phase 5.8: Candidate 5 Source Provenance & Build Environment

**Document ID:** `ENGRAM-PHASE-5.8-CANDIDATE5-PROVENANCE`  
**Date:** 2026-11-22  
**Target Candidate:** Candidate 5 (Engram v1.5.4 Optimized Engine)  
**Binary Location:** `target/release/engram.exe`  
**Binary Size:** 18,865,664 bytes  
**SHA-256 Digest:** `ED851AF3791EAC42529690EC41314548365166C4FF9DD93FAEBA15B9D1F69804`  

---

## 1. Build Environment & Toolchain Pinning

Candidate 5 was compiled in a pinned, local, offline toolchain:

| Component | Pinned Specification | Observed Reality |
| :--- | :--- | :--- |
| **Rust Compiler** | `rustc 1.99.0 (b940084d7 2026-09-28)` | `rustc 1.99.0 (b940084d7 2026-09-28)` |
| **Cargo Package Manager** | `cargo 1.99.0` | `cargo 1.99.0` |
| **Target Architecture** | `x86_64-pc-windows-msvc` | `x86_64-pc-windows-msvc` |
| **Rust Edition** | 2021 | 2021 (`Cargo.toml`) |
| **Host Toolchain Config** | `rust-toolchain.toml` | `channel = "stable"`, `profile = "minimal"` |
| **Build Flags** | `--release --offline` | Verified offline execution |
| **Optimization Profile** | `opt-level = 3`, `lto = "thin"` | Verified in `Cargo.toml` |

---

## 2. Compile-Time Build Script (`build.rs`) Mechanics

The `build.rs` script generates compile-time environment variables that are baked directly into the binary's `.rdata` section via `cargo:rustc-env`:

```rust
println!("cargo:rustc-env=ENGRAM_BUILD_GIT_COMMIT={}", git_commit);
println!("cargo:rustc-env=ENGRAM_BUILD_GIT_DIRTY={}", dirty_str);
println!("cargo:rustc-env=ENGRAM_BUILD_RUSTC={}", rustc_version);
println!("cargo:rustc-env=ENGRAM_BUILD_PROFILE={}", profile);
```

### Forensic Implication of `.git` Exclusion:
When building in a pure snapshot workspace where `.git` is omitted, `git rev-parse HEAD` returns failure. `build.rs` falls back to:
- `ENGRAM_BUILD_GIT_COMMIT="unknown"`
- `ENGRAM_BUILD_GIT_DIRTY="unknown"`
This explains the metadata difference between the repository build and the isolated snapshot build.

---

## 3. Statically Linked Language Parsers

Tree-sitter grammars are compiled as C code during the build process via `cc` and statically embedded into the PE binary:
1. `tree-sitter-rust` (v0.23.3)
2. `tree-sitter-python` (v0.23.6)
3. `tree-sitter-javascript` (v0.23.1)
4. `tree-sitter-typescript` (v0.23.2)
5. `tree-sitter-go` (v0.23.4)
6. `tree-sitter-java` (v0.23.5)
7. `tree-sitter-c-sharp` (v0.23.5)

Zero external shared libraries (`.dll`) are required at runtime.
