# CI Guard Sovereign Security Architecture

Engram's CI Guard enforces Change Contracts and Project Constitution rules on Pull Requests with strict fail-closed boundaries:

## Security Invariants

1. **Sovereign Enforcement**: Guard workflows checkout and execute the action definition from `${{ github.base_ref }}` rather than untrusted PR workspace definitions.
2. **Untrusted Policy Immutability**: Any modification to `.engram/constitution.yaml`, `action.yml`, or `.github/workflows/` within an unratified PR immediately halts CI with `CONSTITUTION_MODIFIED_IN_PR`.
3. **Fail-Closed Boundary**: Unknown or unratified mutations outside explicit Change Contract scopes block merge and annotate violations in PR checks.
