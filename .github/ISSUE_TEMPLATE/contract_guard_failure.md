---
name: Change Contract / Change Guard Failure
about: Report false boundary violations, missed mutations, or hook/CI enforcement issues
title: '[GUARD/CONTRACT] '
labels: change-guard, contracts
assignees: ''
---

### Environment Information
- **OS & Architecture**:
- **Engram Version**:
- **Git Version**: (`git --version`)
- **Hook or CI Mode**: (Pre-commit hook, CI pull_request workflow, or manual CLI)

### Contract Definition
```json
<!-- Include your contract JSON or relevant snippet (sanitized) -->
```

### Git Diff Observed
```text
<!-- Run `git diff --stat` or `git status` (sanitized) -->
```

### Change Guard Output
```text
<!-- Paste stdout/stderr from `engram contract check` or `engram postflight` -->
```

### Expected vs Actual Behavior
- **Expected evaluation**: (e.g. should have PASS, or should have detected SCOPE_EXPANSION on file X)
- **Actual evaluation**: (e.g. exit code, reported violations or lack thereof)

### Sanitized Diagnostics
```text
<!-- Please ensure all paths and contents are sanitized of private information -->
```
