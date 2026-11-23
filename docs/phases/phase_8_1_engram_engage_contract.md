# Phase 8.1: `engram_engage` Contract Specification
## Consolidated Engagement Protocol & Schema Definition

**Document Identifier:** `ENGRAM-CONTRACT-ENGAGE-8-1-20261123`  
**Status:** `PROTOTYPE_SPECIFICATION`  
**Version:** `1.0.0-prototype`  
**Date:** 2026-11-23  
**Target:** Model Context Protocol (MCP) `tools/call` with tool name `engram_engage`  
**Governing Standard:** JSON-RPC 2.0 / MCP Tool Protocol Specification  

---

## 1. Request Contract Specification

### 1.1 JSON Schema Definition

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "EngramEngageRequest",
  "type": "object",
  "properties": {
    "task": {
      "type": "string",
      "description": "The coding task goal, user prompt, or intent. Must not be empty or whitespace-only."
    },
    "token_budget": {
      "type": "integer",
      "minimum": 100,
      "maximum": 32000,
      "default": 1200,
      "description": "Context token budget limit for compiled AST excerpts and symbol signatures."
    },
    "planned_files": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Explicit list of relative workspace file paths the agent intends or expects to modify."
    },
    "forbidden": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Explicit forbidden boundary patterns beyond the Project Constitution (e.g. ['legacy/**', 'config/prod.json'])."
    },
    "symbols": {
      "type": "array",
      "items": { "type": "string" },
      "description": "Targeted symbol filter: prioritize excerpting these specific functions, classes, or types."
    },
    "subpath": {
      "type": "string",
      "description": "Optional repository subdirectory scope to restrict AST traversal (e.g. 'src/services')."
    },
    "language": {
      "type": "string",
      "description": "Optional programming language filter (e.g. 'rust', 'python', 'typescript')."
    },
    "detail_level": {
      "type": "string",
      "enum": ["compact", "standard", "full"],
      "default": "standard",
      "description": "Level of excerpt detail: 'compact' (signatures only), 'standard' (bounded context), or 'full' (complete function bodies)."
    },
    "no_bypass": {
      "type": "boolean",
      "default": false,
      "description": "Force full Change Contract locking and AST compilation even if the task qualifies as trivial."
    },
    "inject_instructions": {
      "type": "boolean",
      "default": false,
      "description": "Also inject Turn-0 context and directives into workspace agent instruction files (.github/copilot-instructions.md, .cursorrules, etc.)."
    },
    "max_bytes": {
      "type": "integer",
      "minimum": 1024,
      "maximum": 65536,
      "default": 16384,
      "description": "Maximum serialized JSON-RPC response size in bytes, enforced via progressive payload condensation."
    }
  },
  "required": ["task"]
}
```

### 1.2 Request Field Validation Rules

1. **`task` (Required):**
   - Must be non-empty after trimming.
   - Max length: 4,096 characters (prevents denial-of-service via huge strings).
   - If empty or missing, returns a structured error: `MISSING_REQUIRED_FIELD: 'task' must not be empty`.
2. **`planned_files` (Optional):**
   - Each entry must be validated against `validate_workspace_path`:
     - Must be relative or resolve strictly within the workspace root.
     - Path traversal elements (`../`, `..\\`) attempting to escape the root are strictly rejected with `SECURITY_PATH_TRAVERSAL`.
3. **`forbidden` (Optional):**
   - Each entry must resolve within the workspace root.
4. **`max_bytes` (Optional):**
   - Clamped strictly to range `[1024, 65536]` bytes.

---

## 2. Response Contract Specification

### 2.1 State Model (`status`)

The response guarantees one of four definitive status codes:

| Status Code | Operational Meaning | Agent Directives |
| :--- | :--- | :--- |
| `READY` | Preflight succeeded, Change Contract locked, context compiled. | Authorized to modify `expected_files`. Do not edit `forbidden_boundaries`. |
| `BYPASS_TRIVIAL` | Task is a self-contained trivial leaf edit. | Bypasses Change Contract overhead. Proceed directly to edit without MCP tax. |
| `BLOCKED` | Request violates Project Constitution or boundary constraints. | **Do not modify code.** Reconcile `blocking_violations` or request human override. |
| `NEEDS_CLARIFICATION` | Task objective is ambiguous; multiple uncoordinated paths found. | **Do not modify code.** Ask the user for clarification on ambiguous items. |

### 2.2 Response Schema Definition

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "EngramEngageResponse",
  "type": "object",
  "properties": {
    "status": {
      "type": "string",
      "enum": ["READY", "BYPASS_TRIVIAL", "BLOCKED", "NEEDS_CLARIFICATION"]
    },
    "repository": {
      "type": "object",
      "properties": {
        "root": { "type": "string", "description": "Canonical workspace directory" },
        "baseline_commit": { "type": "string", "description": "Git HEAD SHA or revision string" },
        "branch": { "type": "string", "description": "Active git branch" },
        "is_clean": { "type": "boolean", "description": "Whether working tree has uncommitted modifications" }
      },
      "required": ["root", "baseline_commit", "branch", "is_clean"]
    },
    "task": {
      "type": "object",
      "properties": {
        "goal": { "type": "string", "description": "Normalized core task objective" },
        "classification": { "type": "string", "enum": ["TRIVIAL", "MODERATE", "COMPLEX", "UNKNOWN"] },
        "classification_reason": { "type": "string" },
        "bypass_active": { "type": "boolean" }
      },
      "required": ["goal", "classification", "bypass_active"]
    },
    "contract": {
      "type": "object",
      "properties": {
        "contract_id": { "type": ["string", "null"], "description": "Locked Change Contract identifier (e.g. 'ctr_...')" },
        "contract_version": { "type": ["integer", "null"] },
        "is_reused": { "type": "boolean", "description": "Whether an existing matching contract was reused" },
        "reused_reason": { "type": ["string", "null"] }
      },
      "required": ["contract_id", "is_reused"]
    },
    "boundaries": {
      "type": "object",
      "properties": {
        "expected_files": { "type": "array", "items": { "type": "string" } },
        "expected_modules": { "type": "array", "items": { "type": "string" } },
        "forbidden_boundaries": { "type": "array", "items": { "type": "string" } }
      },
      "required": ["expected_files", "expected_modules", "forbidden_boundaries"]
    },
    "context": {
      "type": "object",
      "properties": {
        "target_files": {
          "type": "array",
          "items": {
            "type": "object",
            "properties": {
              "path": { "type": "string" },
              "level": { "type": "string" },
              "score": { "type": "number" }
            },
            "required": ["path", "level", "score"]
          }
        },
        "governing_rules": { "type": "array", "items": { "type": "string" } },
        "cochange_warnings": { "type": "array", "items": { "type": "string" } },
        "relevant_decisions": { "type": "array", "items": { "type": "string" } },
        "ambiguity_gaps": { "type": "array", "items": { "type": "string" } },
        "token_budget": { "type": "integer" },
        "estimated_tokens": { "type": "integer" },
        "omission_manifest": { "type": ["object", "null"] }
      },
      "required": ["target_files", "governing_rules", "cochange_warnings", "relevant_decisions", "token_budget", "estimated_tokens"]
    },
    "instructions": {
      "type": "object",
      "properties": {
        "injected_files": { "type": "array", "items": { "type": "string" } },
        "directives": { "type": "array", "items": { "type": "string" } }
      },
      "required": ["injected_files", "directives"]
    },
    "blocking_violations": {
      "type": "array",
      "items": { "type": "string" },
      "description": "List of blocking reasons if status is BLOCKED"
    },
    "next_action": {
      "type": "string",
      "description": "Deterministic instruction for the agent's turn"
    }
  },
  "required": [
    "status", "repository", "task", "contract", "boundaries", "context", "instructions", "blocking_violations", "next_action"
  ]
}
```

---

## 3. Realistic Contract Examples

### Example 1: Standard Successful Engagement (`status: READY`)

#### Request:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "engram_engage",
    "arguments": {
      "task": "Add exponential backoff retry to storage SQLite connection pool",
      "planned_files": ["src/storage.rs", "tests/storage_test.rs"],
      "token_budget": 1500
    }
  }
}
```

#### Response:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [{
      "type": "text",
      "text": "{\n  \"status\": \"READY\",\n  \"repository\": {\n    \"root\": \"C:\\\\Users\\\\Amal P S\\\\OneDrive\\\\Projects\\\\Engram\",\n    \"baseline_commit\": \"e0d1103596322ee13b5838c34b20a3a7fc12be5e\",\n    \"branch\": \"feature/phase2-stage5-remediation\",\n    \"is_clean\": true\n  },\n  \"task\": {\n    \"goal\": \"Add exponential backoff retry to storage SQLite connection pool\",\n    \"classification\": \"MODERATE\",\n    \"classification_reason\": \"Core architectural storage enhancement\",\n    \"bypass_active\": false\n  },\n  \"contract\": {\n    \"contract_id\": \"ctr_9b4e18ac62f849aa8265012a9e105e91\",\n    \"contract_version\": 1,\n    \"is_reused\": false,\n    \"reused_reason\": null\n  },\n  \"boundaries\": {\n    \"expected_files\": [\"src/storage.rs\", \"tests/storage_test.rs\"],\n    \"expected_modules\": [\"storage\"],\n    \"forbidden_boundaries\": [\".engram/**\", \"releases/**\", \"dist/**\"]\n  },\n  \"context\": {\n    \"target_files\": [\n      {\"path\": \"src/storage.rs\", \"level\": \"L1\", \"score\": 0.95},\n      {\"path\": \"tests/storage_test.rs\", \"level\": \"L1\", \"score\": 0.88}\n    ],\n    \"governing_rules\": [\n      \"POL-01: SQLite connection busy timeout must be >= 15 seconds\",\n      \"POL-02: Zero raw pointers outside empirical_live module\"\n    ],\n    \"cochange_warnings\": [\n      \"src/storage.rs frequently co-changes with tests/storage_test.rs (94% confidence)\"\n    ],\n    \"relevant_decisions\": [\n      \"DEC-04: WAL mode enabled on SQLite connections for concurrency resilience\"\n    ],\n    \"ambiguity_gaps\": [],\n    \"token_budget\": 1500,\n    \"estimated_tokens\": 642,\n    \"omission_manifest\": null\n  },\n  \"instructions\": {\n    \"injected_files\": [],\n    \"directives\": [\n      \"Modify only files listed in expected_files.\",\n      \"Do not touch forbidden_boundaries.\",\n      \"Run cargo test --test storage_test before calling engram_postflight.\"\n    ]\n  },\n  \"blocking_violations\": [],\n  \"next_action\": \"Change Contract locked. Proceed directly to implement changes in authorized files: src/storage.rs, tests/storage_test.rs. Call engram_postflight when finished.\"\n}"
    }],
    "isError": false
  }
}
```

---

### Example 2: Adaptive Trivial Task Bypass (`status: BYPASS_TRIVIAL`)

#### Request:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "engram_engage",
    "arguments": {
      "task": "Fix typo in comment in README.md"
    }
  }
}
```

#### Response:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [{
      "type": "text",
      "text": "{\n  \"status\": \"BYPASS_TRIVIAL\",\n  \"repository\": {\n    \"root\": \"C:\\\\Users\\\\Amal P S\\\\OneDrive\\\\Projects\\\\Engram\",\n    \"baseline_commit\": \"e0d1103596322ee13b5838c34b20a3a7fc12be5e\",\n    \"branch\": \"feature/phase2-stage5-remediation\",\n    \"is_clean\": true\n  },\n  \"task\": {\n    \"goal\": \"Fix typo in comment in README.md\",\n    \"classification\": \"TRIVIAL\",\n    \"classification_reason\": \"Markdown typo fix classified as trivial leaf edit\",\n    \"bypass_active\": true\n  },\n  \"contract\": {\n    \"contract_id\": null,\n    \"contract_version\": null,\n    \"is_reused\": false,\n    \"reused_reason\": null\n  },\n  \"boundaries\": {\n    \"expected_files\": [],\n    \"expected_modules\": [],\n    \"forbidden_boundaries\": []\n  },\n  \"context\": {\n    \"target_files\": [],\n    \"governing_rules\": [],\n    \"cochange_warnings\": [],\n    \"relevant_decisions\": [],\n    \"ambiguity_gaps\": [],\n    \"token_budget\": 1200,\n    \"estimated_tokens\": 0,\n    \"omission_manifest\": null\n  },\n  \"instructions\": {\n    \"injected_files\": [],\n    \"directives\": [\"Task classified as trivial leaf edit. Proceed directly with edit without MCP overhead.\"]\n  },\n  \"blocking_violations\": [],\n  \"next_action\": \"Task classified as trivial leaf edit. Proceed directly to implementation without Change Contract overhead. Call engram_postflight only if compliance audit is desired.\"\n}"
    }],
    "isError": false
  }
}
```

---

### Example 3: Blocked Boundary Violation (`status: BLOCKED`)

#### Request:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "engram_engage",
    "arguments": {
      "task": "Modify release binaries directly",
      "planned_files": ["releases/engram-preview-v1.5.4-cand5-clean-windows-x64/bin/engram.exe"]
    }
  }
}
```

#### Response:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [{
      "type": "text",
      "text": "{\n  \"status\": \"BLOCKED\",\n  \"repository\": {\n    \"root\": \"C:\\\\Users\\\\Amal P S\\\\OneDrive\\\\Projects\\\\Engram\",\n    \"baseline_commit\": \"e0d1103596322ee13b5838c34b20a3a7fc12be5e\",\n    \"branch\": \"feature/phase2-stage5-remediation\",\n    \"is_clean\": true\n  },\n  \"task\": {\n    \"goal\": \"Modify release binaries directly\",\n    \"classification\": \"COMPLEX\",\n    \"classification_reason\": \"Attempted modification of forbidden release directory\",\n    \"bypass_active\": false\n  },\n  \"contract\": {\n    \"contract_id\": null,\n    \"contract_version\": null,\n    \"is_reused\": false,\n    \"reused_reason\": null\n  },\n  \"boundaries\": {\n    \"expected_files\": [],\n    \"expected_modules\": [],\n    \"forbidden_boundaries\": [\".engram/**\", \"releases/**\", \"dist/**\"]\n  },\n  \"context\": {\n    \"target_files\": [],\n    \"governing_rules\": [\"POL-RELEASE: Published release assets in releases/ are permanently immutable\"],\n    \"cochange_warnings\": [],\n    \"relevant_decisions\": [],\n    \"ambiguity_gaps\": [\"Planned file 'releases/.../engram.exe' falls inside forbidden boundary pattern 'releases/**'\"],\n    \"token_budget\": 1200,\n    \"estimated_tokens\": 0,\n    \"omission_manifest\": null\n  },\n  \"instructions\": {\n    \"injected_files\": [],\n    \"directives\": [\"STOP: Planned modifications violate repository boundaries. Do not edit forbidden files.\"]\n  },\n  \"blocking_violations\": [\n    \"Planned file 'releases/engram-preview-v1.5.4-cand5-clean-windows-x64/bin/engram.exe' falls inside forbidden boundary pattern 'releases/**'\"\n  ],\n  \"next_action\": \"Reconcile blocking violations before modifying code or request human override.\"\n}"
    }],
    "isError": false
  }
}
```

---

### Example 4: Path Traversal Security Rejection (`isError: true`)

#### Request:
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "engram_engage",
    "arguments": {
      "task": "Read parent credentials",
      "planned_files": ["../../../../etc/shadow"]
    }
  }
}
```

#### Response:
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "result": {
    "content": [{
      "type": "text",
      "text": "Planned file validation failed: Path validation error: path '../../../../etc/shadow' escapes workspace root"
    }],
    "isError": true
  }
}
```
