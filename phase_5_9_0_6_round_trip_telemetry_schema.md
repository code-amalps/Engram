# Engram Phase 5.9.0.6: Standardized Round-Trip Telemetry Schema Specification

**Phase:** 5.9.0.6  
**Schema Document:** `benchmarks/matched-pair-harness/schemas/run_telemetry.schema.json`  
**Execution Mode:** STRICTLY OFFLINE SCHEMA CONTRACT & COMPATIBILITY AUDIT  
**Date:** 2026-11-22  

---

## 1. Schema Design Principles

Prior to Phase 5.9.0.6, benchmark telemetry measured token consumption (prompt, cached, uncached, output, reasoning) and broad file-replacement metrics, but lacked standardized fields to objectively assess:
1. Did Engram reduce model-agent conversational round trips?
2. Did Engram eliminate repetitive filesystem searches and retries?
3. What latency penalty or efficiency was incurred during MCP tool invocations?
4. What was the exact payload overhead (in bytes) of tool arguments vs results?

To resolve these questions without breaking existing historical log parsers, Phase 5.9.0.6 introduces two non-breaking extensions to `run_telemetry.schema.json`:
- `telemetry.round_trip_summary`: Aggregated statistical metrics for the run.
- `telemetry.round_trip_events`: Detailed array of granular request-response tool invocations.
- `request_size_gate`: Root-level pre-dispatch request inspection metadata.

---

## 2. Formal Schema Definitions

### 2.1 `round_trip_summary` Definition
Added under `properties.telemetry`:

```json
"round_trip_summary": {
  "type": ["object", "null"],
  "properties": {
    "total_agent_turns": { "type": "integer", "minimum": 0 },
    "total_round_trips": { "type": "integer", "minimum": 0 },
    "engram_mcp_calls": { "type": "integer", "minimum": 0 },
    "standard_tool_calls": { "type": "integer", "minimum": 0 },
    "total_retries": { "type": "integer", "minimum": 0 },
    "latency_mean_ms": { "type": "number", "minimum": 0 },
    "latency_median_ms": { "type": "number", "minimum": 0 },
    "latency_p95_ms": { "type": "number", "minimum": 0 },
    "latency_min_ms": { "type": "number", "minimum": 0 },
    "latency_max_ms": { "type": "number", "minimum": 0 },
    "total_request_bytes": { "type": "integer", "minimum": 0 },
    "total_response_bytes": { "type": "integer", "minimum": 0 },
    "adaptive_bypass_count": { "type": "integer", "minimum": 0 },
    "status": { "type": "string" }
  }
}
```

### 2.2 `round_trip_events` Definition
Added under `properties.telemetry`:

```json
"round_trip_events": {
  "type": ["array", "null"],
  "items": {
    "type": "object",
    "properties": {
      "call_id": { "type": "string" },
      "tool_name": { "type": "string" },
      "is_engram": { "type": "boolean" },
      "is_retry": { "type": "boolean" },
      "attempt_number": { "type": "integer", "minimum": 1 },
      "request_bytes": { "type": "integer", "minimum": 0 },
      "response_bytes": { "type": "integer", "minimum": 0 },
      "duration_ms": { "type": "number", "minimum": 0 },
      "status": { "type": "string" },
      "timestamp": { "type": ["string", "null"] }
    }
  }
}
```

### 2.3 `request_size_gate` Definition
Added under root `properties`:

```json
"request_size_gate": {
  "type": ["object", "null"],
  "properties": {
    "gate_passed": { "type": "boolean" },
    "measured_request_tokens": { "type": "integer", "minimum": 0 },
    "measured_request_bytes": { "type": "integer", "minimum": 0 },
    "counting_method": { "type": "string" },
    "limit_tokens": { "type": "integer", "minimum": 0 },
    "breakdown": { "type": ["object", "null"] }
  }
}
```

---

## 3. Backward Compatibility Audit

1. **Required Fields Preserved:** The top-level `required` list (`schema_version`, `run_id`, `pair_id`, `task_id`, `arm`, `execution_order_in_pair`, `timestamps`, `model`, `isolation`, `telemetry`, `git_impact`, `correctness`) remains unchanged.
2. **Optional Properties:** `round_trip_summary`, `round_trip_events`, and `request_size_gate` are optional properties that permit `null` or omission.
3. **Historical Telemetry Validity:** Legacy telemetry records generated during Phase 5.4, 5.8, or early 5.9 remain 100% compliant and validate successfully against this schema without modification.
4. **Validation Test Evidence:** Verified using `jsonschema.validate` across existing benchmark outputs and newly generated records.
