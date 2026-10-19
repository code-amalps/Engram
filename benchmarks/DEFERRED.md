# Deferred Features & Scope Deviations

## Scope Deviation: Iteration 2B Local Deterministic Postflight Prototype

### 1. Description of Scope Deviation
During Phase 2 preparation (Iteration 2A hardening), an early prototype of **Iteration 2B Local Deterministic Postflight** was introduced in the codebase ahead of scheduled pilot freeze:
- Added `validate_local_postflight` and associated status enums to [`src/flywheel.rs`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/flywheel.rs#L1021-L1356).
- Added the CLI subcommand `engram validate-local` to [`src/main.rs`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/src/main.rs#L233-L244,L1275-L1298).
- Added 5 isolated tests in [`tests/context_efficiency_and_blast_radius_test.rs`](file:///C:/Users/Amal%20P%20S/OneDrive/Projects/Engram/tests/context_efficiency_and_blast_radius_test.rs#L622-L880).

### 2. Isolation & Impact Assessment
- **MCP Surface**: `src/mcp.rs` is completely unchanged by Iteration 2B. Tool `engram_postflight` continues to route strictly to `crate::flywheel::postflight_check`.
- **Agent / Pilot Runtime Behavior**: The Copilot extension and MCP client do not invoke `engram validate-local`. All agent interactions during the Phase 0–2 measurement pilot utilize the standard MCP interface.
- **Pilot Treatment Parity**: Because `validate-local` is strictly an inert, uncalled CLI subcommand in the agent workflow, the treatment behavior evaluated in the pilot is completely decoupled from Iteration 2B.
- **Decision**: Iteration 2B is formally marked as **DEFERRED / EXPERIMENTAL PROTOTYPE**. It will not be incorporated into the pilot workflow or treated as an active efficiency feature during Phase 0–2 baseline measurement.
