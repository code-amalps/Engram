---
name: Context or Impact-Analysis Failure
about: Report missing symbols, irrelevant context, or faulty change-impact predictions
title: '[CONTEXT/IMPACT] '
labels: intelligence, diagnostics
assignees: ''
---

### Environment Information
- **OS & Architecture**: (e.g., Linux x86_64, macOS Apple Silicon, Windows 11)
- **Engram Version**: (run `engram --version`)
- **Language / Framework**: (e.g., Go 1.22 with modules, Java Spring Boot Maven, TypeScript React)
- **AI Coding Client**: (e.g., Cursor, Claude Desktop, MCP)

### Task Query Attempted
```text
engram context "<task description>" --explain
```
or
```text
engram inspect <symbol_or_path>
```

### What Context / Impact Was Expected?
- Expected relevant files:
- Expected relevant symbols:
- Expected blast radius / downstream dependencies:

### What Context / Impact Was Actually Returned?
- Files returned:
- Missing files / false negatives:
- Irrelevant files / false positives:

### Sanitized Diagnostics
```text
<!-- Run with --explain and sanitize before pasting. Do NOT share credentials or sensitive code. -->
```
