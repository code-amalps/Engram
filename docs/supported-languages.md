# Supported Languages & Client Integration Matrix

Engram is a polyglot repository intelligence tool. Because different language ecosystems have distinct type systems, package formats, and module semantics, capabilities differ by language.

Below is an honest, verified capability matrix grounded in the codebase and test suite.

---

## 1. Language Capability Support Matrix

| Language | Tree-Sitter AST | Extracted Symbols | Dependency / Manifest Resolution | Cross-File Relational Resolution | Test Association Convention | Contract Symbol Normalization |
|---|:---:|---|---|---|---|---|
| **Rust** | `tree-sitter-rust` | Functions, structs, enums, traits, impls, modules, types | `Cargo.toml` discovery, `use`, `mod` statements | File and directory module path linking | `tests/*.rs`, `*_test.rs` $\to$ `src/*.rs` | Full symbol matching |
| **Go** | `tree-sitter-go` | Structs, interfaces, methods, functions, packages | Full `go.mod` directives (`module`, `require`, `replace`, `exclude`, `toolchain`) | Two-stage structural interface matching, module subpaths | `*_test.go` $\to$ `*.go` | Receiver pointer normalization (`*Service.Method` == `Service.Method`) |
| **TypeScript** | `tree-sitter-typescript` | Classes, interfaces, types, functions, methods | `package.json`, ES6 `import`, CommonJS `require` | Relative path resolution (`./`, `../`) | `*.spec.ts`, `*.test.ts` $\to$ `*.ts` | Full symbol matching |
| **JavaScript** | `tree-sitter-javascript` | Classes, functions, methods | `package.json`, ES6 `import`, CommonJS `require` | Relative path resolution (`./`, `../`) | `*.spec.js`, `*.test.js` $\to$ `*.js` | Full symbol matching |
| **Python** | `tree-sitter-python` | Classes, functions, methods | `pyproject.toml`, `from ... import`, `import` | Module name to relative directory path | `test_*.py`, `*_test.py` $\to$ module | Full symbol matching |
| **C#** | `tree-sitter-c-sharp` | Classes, interfaces, structs, enums, methods, constructors | `.csproj`, `using` namespace directives | Namespace and type resolution heuristics | `*Test.cs`, `*Tests.cs` $\to$ `*.cs` | Full symbol matching |
| **Java** | `tree-sitter-java` | Classes, interfaces, records, enums, methods, constructors, annotations | Maven `pom.xml` & Gradle `build.gradle` dependency parsing | Package to directory hierarchy (`src/main/java/...`) | `src/test/java/.../*Test.java` $\to$ `src/main/java/...` | Full symbol matching (`ClassName.methodName`) |

---

## 2. Supported AI Coding Clients (MCP)

Engram runs a background JSON-RPC 2.0 MCP server over standard I/O (`engram mcp`).

| Client | Integration Status | Automated Setup Support | Configuration Target File |
|---|:---:|:---:|---|
| **Cursor** | **Verified & Supported** | `engram setup cursor` | `.cursor/mcp.json` (Workspace or Global) |
| **Claude Desktop** | **Verified & Supported** | `engram setup claude` | `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows) |
| **Windsurf** | **Verified & Supported** | `engram setup windsurf` | `~/.codeium/windsurf/mcp_config.json` |
| **Generic MCP Clients** | **Supported** | Manual JSON configuration | Starts `engram mcp` via stdio |
| **GitHub Copilot / VS Code** | Advisory / Manual | Manual configuration | Requires configuring VS Code MCP extension to invoke `engram mcp` |

### Automated Setup Safety Guarantee
Running `engram setup <client>` performs a safe merge:
- It **never** wipes out existing third-party MCP servers (e.g. Postgres, Brave Search, GitHub).
- It only inserts or updates the `"engram"` key in the client's configuration file.
- Use `engram setup <client> --dry-run` to preview edits before modifying disk.
