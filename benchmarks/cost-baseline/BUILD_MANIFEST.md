# Engram Candidate Binary Build Manifest

**Document Version:** 1.0.0  
**Status:** FROZEN CANDIDATE AUDIT (PRE-FREEZE)  
**Target Architecture:** `x86_64-pc-windows-msvc`  
**Git HEAD Revision:** `17fdf4a2e98f98e6d192afc28271cb8de47aa497`  
**Build Profile:** `release` (`cargo build --release`)

---

## 1. Candidate Binary Identification

| Property | Value |
|---|---|
| **Binary Path** | `target\release\engram.exe` |
| **File Size** | 17,722,880 bytes |
| **SHA-256 Hash** | `474a1a7aafbb5a5b52418aa51a978a58aeae4a177c62ec17c526e165610f44e0` |
| **Reported Version** | `engram 1.5.1` |
| **Build Timestamp** | 2026-10-18T05:50:23Z |

---

## 2. Toolchain & Compiler Environment

- **Rust Compiler:** `rustc 1.99.0`
- **Compiler Commit Hash:** `b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`
- **Compiler Commit Date:** 2026-09-28
- **Cargo Version:** `cargo 1.99.0 (5f94df478 2026-08-27)`
- **Host Triple:** `x86_64-pc-windows-msvc`
- **LLVM Version:** 23.1.1
- **Linker:** Microsoft MSVC Linker (`link.exe`) via default Visual Studio C++ Build Tools
- **Build Script (`build.rs`):** None (Pure Cargo compilation with bundled rusqlite)
- **Local Cargo Configuration (`.cargo/config.toml`):** None (Standard system Cargo defaults)

---

## 3. Cargo Release Profile Configuration

From `Cargo.toml`:
```toml
[profile.release]
lto = "thin"
codegen-units = 1
strip = true
```

---

## 4. Complete Build-Influencing Source Manifest

### Project Declarations
- `Cargo.toml`: `6b29a424f18a6c269ae68c263fab3fa0761b5b8cf777d98d885b6b8d8b81ef81` (952 bytes)
- `Cargo.lock`: `83ca397e6ac1d37d35b0b59d1f725b4144a10544310554923b43ab700e2a741f` (26,958 bytes)

### Source Modules (`src/`) — All 46 Rust Source Files

| SHA-256 Hash | Size (Bytes) | Relative Path |
|---|---:|---|
| `042e3e38e254b65513e649f69e991e88e836acf3f0cde3b1d6c4634e5c31dc72` | 33,885 | `src\analyzer.rs` |
| `ef5fa4d4f4e6fd6d2a7d3d2ccb9ef2398a3a8cd7402fe46bc59184dd22149466` | 6,626 | `src\archaeology.rs` |
| `62209b68d72fb0cea364626b3594be7e88b0454ec82c8fcad41dfdc1ac30c1e3` | 129,706 | `src\bench.rs` |
| `5ab3376bd19440d59b8b7bf92d86223d248881412c4976c763457c8d9575cd51` | 52,252 | `src\ci_guard.rs` |
| `aa0f8e161f06cdad5d8e95bdbe983b9055f594d082de06637a6f57c4c6d3b862` | 14,007 | `src\compiler.rs` |
| `0110f3cd6a589ab696a5f35b0fb2946e4036c646fdc4a0ca29073d691715ba40` | 10,898 | `src\constitution.rs` |
| `9381c6e3a87758dbeffd84aed321be7291b8be470dc507ead4c6a11a672448c5` | 72,879 | `src\contract.rs` |
| `2cf97c1c7d58dc99078add048d5e3087b7ce7b28d9c08c262cf4f59e385f963f` | 13,142 | `src\doctor.rs` |
| `160aeb9d91831ef927c183d60c19a34aeeed7c20390af67206cd4392a232829c` | 48,004 | `src\drift.rs` |
| `5c49ba544005a55e1e917827b245ac35aa8e63e937b6df2a99bb0e0acc0a8124` | 45,538 | `src\empirical.rs` |
| `3d302e94c08b0dca836e0e2cf034b601b8ab355fcab8144f214aca68eab363a5` | 13,475 | `src\empirical_live\anthropic.rs` |
| `ea63a2586cc395657183c9ad1f3b42859d3b8dc462374e0454d5ce4aff68412b` | 52,480 | `src\empirical_live\budget.rs` |
| `58a3be45fcbe81b007d2725a36f91064bac635932e13d0a4041d3b182b277fd8` | 37,535 | `src\empirical_live\dispatcher.rs` |
| `af234ed6c724adc074620bb1532738d0e17d3327f6a13dd9d8cdfdab73410b23` | 4,214 | `src\empirical_live\mock.rs` |
| `47f75050d8d3c29515dc50041b0dec4bfec61ad1b67b714c44782e179934f482` | 1,397 | `src\empirical_live\mod.rs` |
| `840813f58b5150eee30ac0cc91d3ed2a8a3355e3592efcc26b6bd8c34c3cc176` | 17,862 | `src\empirical_live\orchestrator.rs` |
| `31ae56d5887aa517606e64ac3ead84ef1653cba797daf84b98ac12ad7bd10752` | 14,018 | `src\empirical_live\provider.rs` |
| `cb0e46a0cd5e0c8b69405facf380b00ff179c20a056cd596f502da1a24ab1a6e` | 20,428 | `src\empirical_live\report.rs` |
| `339d9eff27400242479cb5cc00efcfbdd1bcbee864205dccba644f7224a0b3a8` | 16,451 | `src\empirical_live\schema.rs` |
| `32c0f15704c5cfe617470aeb0466edfd37f1945d69f6c2f050ca6acc92169bb7` | 160,716 | `src\empirical_live\security.rs` |
| `3651de3b54c4b1a91adbe90906165ed68a3352b460ba7f8d472e2ba6f9d01ebc` | 34,588 | `src\empirical_live\stats.rs` |
| `1aa4fd12a7d446bfe3f4687333f17abeac91858a9ee9a55dae1ce023cc519ab5` | 37,537 | `src\empirical_runner.rs` |
| `e62a175ec244e793cde834f5a281e120aa55253a5df176edcc59b6c16d639aeb` | 51,417 | `src\flywheel.rs` |
| `f1852457215d5efac87a22b66c215f2cdb79172145bf199dcd4b1163246c19d5` | 8,243 | `src\gc.rs` |
| `bb5eee3721db9eb944999e2e4c3a21fa2d7e01cecae326cceb376303ba430ccb` | 18,803 | `src\git.rs` |
| `15f9eeced3034fe498b0573caa5355e2afe2da45faf6b252bd35c299e5e3a466` | 8,427 | `src\git_history.rs` |
| `a42af291bc2d2f8788dafd94532ea352b0c7ebcb4bbf31ab102ddd0045ad8e9d` | 41,917 | `src\go_parser.rs` |
| `5e78f1ddad6db2012f9b32a9010ffac6e6c08bbec1dc55d78b3c082d2deb3f2a` | 6,566 | `src\graph.rs` |
| `9b0aa1d94d85254c3b4437d4ae5c8a0f2a18b9e4305099da43c6bff4353ff8a6` | 16,066 | `src\guard.rs` |
| `29c776b332e4e8c8f88d07e1b1632e2a01438ecfaf2e129fd9f5d6a87b863b89` | 24,846 | `src\impact.rs` |
| `a7c02db275bd264c7453eb593dba772534654cec4e4adee30150e20a9f86fcec` | 26,176 | `src\incremental.rs` |
| `059d6fcfd53e3fa3dba1907b391a12fde5563b6c101789fadfa6610d5a746ff3` | 11,973 | `src\instructions.rs` |
| `1d7ecd4948fe91a8a454c48696060e29976bb5b557c6e04ac1de19bc1b0271a7` | 29,772 | `src\java_parser.rs` |
| `5619546b44d071221abd299d8574fe0082c44ed4eb5c8bfc87d452b843f779b0` | 693 | `src\lib.rs` |
| `e854011f7f788d1a5f4a2d0585253200452e2e2dc67ac1c1756b0bc9a491406b` | 59,220 | `src\main.rs` |
| `f1bfe8173194544219b736e69b5411f303184e7dfdeb2a0c84323a2e89b3b447` | 10,193 | `src\manifest.rs` |
| `825838516c91a517e9217a3e286fb3f8389375f63a7b001eaa2fb8acac5c2e2b` | 46,600 | `src\mcp.rs` |
| `3a7c902431941af5220970482fbcf17d97f64f341db8685c4bf6b48288ebdc2c` | 51,286 | `src\memory.rs` |
| `2dc1a1985ea27951c7b903002cafc6087ef8c3c3e185841a9cd7594f0666ec59` | 12,707 | `src\memory_graph.rs` |
| `337f644f9f946c508ef4bd44fe2c6e5c0f25cf3d323af79b530ebe6cb1c52b16` | 9,944 | `src\model.rs` |
| `68d5cb53b46d44857ba063412c5038d67a088a724708dc07a686369632abf69b` | 9,001 | `src\printer.rs` |
| `2222a3c908a9e427f9dabc4ba80f6f7565716f16a471d1a1d27fce86853697ed` | 34,640 | `src\prompt.rs` |
| `e870f12b16eeb7b6c475bd80c6a930c748d0a9d9d5a35923349f88e5337b557b` | 32,149 | `src\recovery.rs` |
| `738fc08b5f8b497a8b455a7087714541f17d22c9e0b8e7950b575deb2ec8b6c4` | 36,022 | `src\setup.rs` |
| `eb85d251168588db33bd70ff520cc88f12c706b83b72ecf993ff54cee9f226e5` | 11,781 | `src\storage.rs` |
| `0b947c1b63c8a1b33ee8d22a55278addc1684172a8e5e8aa2318e66dc52b900a` | 13,444 | `src\telemetry.rs` |

---

## 5. Sensitive Inputs & External Environment Audit

- **Sensitive Inputs Audit:** Zero secret tokens, cloud credentials, or private keys are compiled into `engram.exe`. All MCP operations and graph analysis run locally on SQLite WAL files and local AST parsers.
- **Dynamic System Dependencies:** Dynamic link dependencies on Windows OS runtime libraries (`ntdll.dll`, `kernel32.dll`, `advapi32.dll`, `ws2_32.dll`, `ucrtbase.dll`).
- **Target Golden Workspace Master:** `C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter` at commit `1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2`.
