# Model Context Protocol (MCP) Server Integration

Engram implements the [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) specification, running as a local JSON-RPC 2.0 server over standard input/output (`stdio`).

This allows AI coding assistants—including **Claude Desktop, Cursor, VS Code (Cline/Roo Code), and Windsurf**—to discover, inspect, and enforce boundaries on your codebase dynamically.

---

## 1. Running the MCP Server

Engram exposes its MCP server directly via the `mcp` subcommand:

```bash
engram mcp
```

When an AI assistant launches Engram, it spawns `engram mcp` as a subprocess and communicates strictly over `stdio`. No network sockets or HTTP ports are opened.

---

## 2. Complete Reference of 17 Registered MCP Tools

Engram registers 17 purpose-built MCP tools across 5 functional categories:

### Category A: Core Context & Single-Shot Planning (3 Tools)

#### 1. `engram_engage`
Consolidated single-shot engagement: binds repository baseline, compiles context, evaluates constitution, locks Change Contract, and receives execution guidance.
- **Parameters:**
  - `task` (*string*, required): Coding task goal, user prompt, or intent.
  - `token_budget` (*integer*, optional): Context token budget limit (default 1,200).
  - `planned_files` (*array of strings*, optional): Explicit list of relative workspace file paths the agent intends to edit.
  - `forbidden` (*array of strings*, optional): Forbidden boundary patterns beyond the Project Constitution.
  - `symbols` (*array of strings*, optional): Targeted symbol filter to prioritize excerpting.
  - `detail_level` (*string*, optional): `"compact"`, `"standard"`, or `"full"`.
- **Returns:** Grounded context block, active contract ID, blast radius analysis, and execution directives.

#### 2. `engram_prepare_task`
Single-shot task preparation: compiles focused context, discovers governing rules, validates boundaries, and locks Change Contract in one atomic round-trip.
- **Parameters:**
  - `task` (*string*, required): Coding task goal or intent.
- **Returns:** Pre-flight context and locked contract state.

#### 3. `engram_get_context`
Compiles focused context with durable memories and co-change alerts.
- **Parameters:**
  - `task` (*string*, required): Coding task goal or intent.
  - `token_budget` (*integer*, optional): Context token budget limit.
  - `detail_level` (*string*, optional): `"compact"`, `"contracted"`, or `"full"`.
  - `symbols` (*array of strings*, optional): Specific symbol filter.
  - `adaptive` (*boolean*, optional): Enable adaptive small-task bypass to save tokens.
- **Returns:** Budgeted context payload with AST excerpts and relevant ADRs.

---

### Category B: Graph & Code Intelligence (5 Tools)

#### 4. `engram_inspect_symbol`
Queries blast radius impact, contract authorization, and memory lookup for a target symbol or file.
- **Parameters:**
  - `target` (*string*, required): Target symbol name or file path.
  - `depth` (*integer*, optional): Traversal depth (1–3, default 2).
  - `max_entities` (*integer*, optional): Maximum entities per category (default 10).
- **Returns:** Downstream dependents, impact categories, and contract status.

#### 5. `engram_why`
Architectural archaeology: explains rationale, governing decisions, and commit history for a symbol or file.
- **Parameters:**
  - `target` (*string*, required): Symbol name or file path to explain.
- **Returns:** Causal provenance trail connecting commits, ADRs, and structural relationships.

#### 6. `engram_get_impact`
Fast 1-hop reverse dependency and blast radius impact query for a target symbol or file.
- **Parameters:**
  - `target` (*string*, required): Target symbol or file path.
- **Returns:** Immediate callers, interface implementers, and dependent tests.

#### 7. `engram_get_cochanges`
Retrieves historical Git co-change coupling signals and statistics for a given file.
- **Parameters:**
  - `file` (*string*, required): Target relative file path.
- **Returns:** Statistically coupled files frequently modified in tandem.

#### 8. `engram_drift`
Analyzes architectural decision drift, boundary erosion, and scoped staleness across commits.
- **Parameters:**
  - `scope` (*string*, optional): Scope filter for drift analysis.
- **Returns:** Stale ADRs, broken constraints, and drift severity metrics.

---

### Category C: Engineering Memory & Architectural Records (3 Tools)

#### 9. `engram_get_memory`
Retrieves durable architectural decisions, policies, and institutional engineering memories.
- **Parameters:**
  - `query` (*string*, optional): Keyword query or topic filter.
  - `scope` (*string*, optional): Scope filter.
- **Returns:** Active architectural decision records (ADRs).

#### 10. `engram_record_decision`
Proposes an architectural decision into engineering memory (marked `PROVISIONAL` until human CLI ratification).
- **Parameters:**
  - `title` (*string*, required): Title of the architectural decision.
  - `statement` (*string*, required): Core architectural policy statement.
  - `reason` (*string*, required): Technical rationale.
  - `scope` (*string*, optional): Scope level (workspace, package, symbol).
- **Returns:** Created ADR identifier and provisional state.

#### 11. `engram_ratify_decision`
Explicitly ratifies an architectural decision with human reviewer authorization and provenance tracking.
- **Parameters:**
  - `decision_id` (*string*, required): The decision ID to ratify.
  - `reviewer` (*string*, required): Reviewer identifier.
- **Returns:** Ratification confirmation and updated status (`ACCEPTED`).

---

### Category D: Boundary Guard & Recovery (4 Tools)

#### 12. `engram_check_constitution`
Evaluates architectural constitution rules and boundary isolation policies against changed files.
- **Parameters:**
  - `changed_files` (*array of strings*, required): List of modified files to validate.
- **Returns:** Evaluation result (`PASSED` or `VIOLATED`) with specific rule citations.

#### 13. `engram_preflight`
Pre-flight handshake: compiles context, checks blast radius, and locks Change Contract before editing.
- **Parameters:**
  - `task` (*string*, required): Task intent.
  - `planned_files` (*array of strings*, optional): Files planned for modification.
- **Returns:** Active contract ID locked to current Git HEAD.

#### 14. `engram_postflight`
Post-flight guard check: evaluates working tree mutation against active Change Contract boundaries and tests.
- **Parameters:**
  - `contract_id` (*string*, required): Active contract identifier.
- **Returns:** Boundary audit outcome (`VALIDATED` or `VIOLATED`).

#### 15. `engram_recover`
Formulates structured, non-destructive recovery recipe for contract violations with a max-2 retry gate.
- **Parameters:**
  - `contract_id` (*string*, required): Contract identifier that tripped violation.
- **Returns:** Safe Git revert commands to discard unapproved edits while preserving valid progress.

---

### Category E: Diagnostics & Ecosystem (2 Tools)

#### 16. `engram_doctor`
Runs diagnostic health checks on memory storage, index integrity, and append-only cryptographic chains.
- **Parameters:** None.
- **Returns:** Health diagnostic check results.

#### 17. `engram_get_ecosystem`
Detects workspace ecosystem, primary language, manifest dependencies, build tools, and framework rules.
- **Parameters:** None.
- **Returns:** Workspace ecosystem profile and configuration rules.

---

## 3. Registered MCP Resources & Prompts

### Resources
- `engram://memories`: Durable architectural decisions, rationale, and institutional memory (`text/markdown`).
- `engram://constitution`: Architectural constraints, boundary definitions, and validation rules (`application/json`).
- `engram://contract/active`: Currently active change contract governing boundary enforcement (`application/json`).
- `engram://ecosystem`: Detected project language, framework, dependencies, and ecosystem rules (`application/json`).

### Prompts
- `engram_preflight_handshake`: Orchestrated pre-flight prompt compiling context, discovering decisions, predicting blast radius, and locking contract.
- `engram_postflight_audit`: Orchestrated post-flight guard evaluating changes against Change Contract boundaries.

---

## 4. Client Configuration Snippets

### Claude Desktop
Add Engram to your Claude Desktop configuration (`%APPDATA%\Claude\claude_desktop_config.json` on Windows):

```json
{
  "mcpServers": {
    "engram": {
      "command": "engram",
      "args": ["mcp"]
    }
  }
}
```

### Cursor IDE
Configure in `.cursor/mcp.json` at your project root:

```json
{
  "mcpServers": {
    "engram": {
      "command": "engram",
      "args": ["mcp"]
    }
  }
}
```

### VS Code (Cline / Roo Code)
Configure in `cline_mcp_settings.json`:

```json
{
  "mcpServers": {
    "engram": {
      "command": "engram",
      "args": ["mcp"],
      "autoApprove": [
        "engram_get_context",
        "engram_inspect_symbol",
        "engram_why",
        "engram_get_impact",
        "engram_get_memory",
        "engram_check_constitution"
      ]
    }
  }
}
```

### Windsurf
Configure in `~/.codeium/windsurf/mcp_config.json`:

```json
{
  "mcpServers": {
    "engram": {
      "command": "engram",
      "args": ["mcp"]
    }
  }
}
```
