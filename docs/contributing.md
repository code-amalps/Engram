# Contributing to Engram

We welcome contributions from developers, researchers, and open-source engineers interested in local repository intelligence, tree-sitter AST extraction, and AI agent boundaries.

---

## 1. Development Environment

Engram is written in **Rust** (2024 edition).

### Prerequisites:
- Rust 1.85+ (or latest stable) with `cargo`.
- Git.

### Building from Source:
```bash
# Clone repository
git clone https://github.com/code-amalps/Engram.git
cd Engram

# Compile in debug mode
cargo build

# Compile optimized release binary
cargo build --release
```

---

## 2. Running Tests

Before submitting any code changes, ensure all automated tests pass:

```bash
# Run complete integration and unit test suite
cargo test

# Run a specific test file
cargo test --test contract_guard_test

# Run benchmarks
engram bench
```

### Engineering Guidelines:
- **Do Not Weaken Tests:** Never delete, ignore (`#[ignore]`), or alter assertion thresholds on existing tests just to make a pull request green.
- **Empirical Grounding:** Any new capability should be accompanied by deterministic fixture tests under `tests/fixtures/real_world/` or unit tests.
- **Keep Diffs Focused:** Avoid sweeping cosmetic refactors across unrelated modules.

---

## 3. Submitting Contributions

1. **Fork & Branch:** Create a feature branch from `main` (e.g. `feat/kotlin-parser` or `fix/windows-path-normalization`).
2. **Issue Discussion:** For major architectural changes or new language adapters, open an issue first using the appropriate template:
   - [Bug Report](.github/ISSUE_TEMPLATE/bug_report.md)
   - [Context or Impact Failure](.github/ISSUE_TEMPLATE/context_or_impact_failure.md)
   - [Contract Guard Failure](.github/ISSUE_TEMPLATE/contract_guard_failure.md)
   - [Feature Request](.github/ISSUE_TEMPLATE/feature_request.md)
3. **Pull Request:** Open a PR against `main`. Ensure your description clearly explains what problem is being solved and references verified test execution.
