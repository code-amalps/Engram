# Security, Privacy & Data Handling in Engram

> [!NOTE]
> The comprehensive, audited security specification and known limitations are documented in [docs/SECURITY_AND_LIMITATIONS.md](SECURITY_AND_LIMITATIONS.md).

---

## Executive Summary

- **Zero Outbound Telemetry or Cloud Transmission:** Engram contains no networking dependencies (`reqwest`, `hyper`, `curl`). It never transmits source code, symbols, prompts, context payloads, or analytics to any external server.
- **Local Git Subprocesses:** Engram executes local `git` CLI subcommands (`rev-parse`, `status`, `diff`) strictly for working tree inspection. It never executes `git push`, `git fetch`, or network-bound commands.
- **MCP Communication:** Engram communicates with AI coding assistants (Cursor, Claude Desktop, Windsurf) strictly over standard input/output (`stdio`) using local JSON-RPC 2.0 messages.
- **Secret Redaction:** Code excerpts persisted to `.engram/index.db` are filtered with high-entropy regex patterns to redact API keys, tokens, and private keys.
- **Symbolic Link Policy:** Symbolic links and junctions are skipped during indexing to prevent directory traversal loops.

For the complete technical specification, ignore policies, and limitations, please consult [docs/SECURITY_AND_LIMITATIONS.md](SECURITY_AND_LIMITATIONS.md).\n