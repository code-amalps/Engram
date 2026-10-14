# Engram Benchmarks & Scientific Measurement Architecture

This directory outlines the deterministic, reproducible measurement framework and fixture catalog for Engram v1.1.0.

## 1. Pillars of the Measurement Framework

1. **Empirical Measurement over Arbitrary Targets:**
   - Latency, token overhead, and surface reduction are measured empirically across cold and warm incremental runs.
   - Per Amendment 2, AC-4 focuses on statistically meaningful reduction and measurable latency improvement rather than artificial hardcoded constraints.
2. **Two-Tier Cache Invalidation:**
   - **Tier 1 (Syntactic):** Authoritative content/blob hashing (with `mtime`/size fast-path hints) invalidates only changed files.
   - **Tier 2 (Relational):** Re-links callers, implementors, and importers of mutated symbols.
3. **Reproducibility & Determinism:**
   - Fixtures are frozen and run 100% offline.
   - Zero repository side effects: tests run in isolated sandboxes and leave workspace git status pristine.

## 2. Real-World Deterministic Fixture Catalog

Located in `tests/fixtures/real_world/`:

| Language | Ecosystem | Directory | Key Components |
|---|---|---|---|
| **TypeScript** | Node / Express | `typescript/` | `package.json`, `tsconfig.json`, `OrderService`, models, tests |
| **JavaScript** | Node.js | `javascript/` | `package.json`, `CatalogManager`, routes, tests |
| **Python** | FastAPI / Dataclasses | `python/` | `pyproject.toml`, `requirements.txt`, `WarehouseService`, models, tests |
| **C#** | .NET 8 / Clean Arch | `csharp/` | `Commerce.csproj`, `CustomerService`, interfaces, entities, tests |
| **Rust** | Multi-module crate | `rust/` | `Cargo.toml`, `Worker`, `WorkQueue`, `EngineConfig`, tests |
| **Go** | Microservice | `go/` | `go.mod`, `PaymentProcessor`, `PaymentGateway`, models, tests |
| **Java** | Maven / Gradle Spring | `java/` | `pom.xml`, `build.gradle`, `InvoiceService`, records, tests |

## 3. Adversarial Robustness Catalog

Located in `tests/adversarial/`:
- `cyclic_deps/`: Circular module dependencies (`module_a.ts` $\leftrightarrow$ `module_b.ts`).
- `deep_hierarchy/`: Deep directory nesting (`level1/.../level5/deep_service.py`).
- `syntax_edge_cases/`: Tricky grammar elements, unicode symbols, embedded keyword strings.
- `missing_imports/`: Dangling references and unresolved symbols.

## 4. Running Benchmarks

### In-Tree Test Suite (Fast, Deterministic)
```bash
cargo test
```

### Live Benchmark CLI
```bash
# Internal deterministic benchmark suite
engram bench
engram bench --verbose
engram bench --json

# External repository benchmark runner
engram bench --external /path/to/repo
engram bench --external /path/to/repo --json
```

The external benchmark runner evaluates arbitrary target repositories cleanly without polluting `cargo test`.
