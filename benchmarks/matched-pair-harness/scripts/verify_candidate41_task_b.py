#!/usr/bin/env python3
"""
Verify Task B context retrieval and response-boundary enforcement
across Candidate 3, Candidate 4, and Candidate 4.1.
"""

import json
import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path("C:/Users/Amal P S/OneDrive/Projects/Engram")
GOLDEN_REPO = Path("C:/Users/Amal P S/OneDrive/Projects/Test/angular-ngrx-material-starter")

CANDIDATES = {
    "Candidate 3 (Legacy)": REPO_ROOT / "benchmarks/matched-pair-harness/bin/candidate-phase3/engram.exe",
    "Candidate 4 (Live 02)": REPO_ROOT / "benchmarks/matched-pair-harness/bin/candidate-phase4/engram.exe",
    "Candidate 4.1 (Remediated)": REPO_ROOT / "benchmarks/matched-pair-harness/bin/candidate-phase4.1/engram.exe",
}

TASK_B_PROMPT = "Add a remember-me option to the authentication flow that persists the username across reloads."

def call_mcp(bin_path: Path, cwd: Path, requests: list[dict]) -> list[tuple[bytes, dict]]:
    cmd = [str(bin_path), "-C", str(cwd), "mcp"]
    proc = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    
    input_data = b"".join(json.dumps(r).encode("utf-8") + b"\n" for r in requests)
    stdout, stderr = proc.communicate(input=input_data, timeout=45)
    
    results = []
    for line in stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            parsed = json.loads(line.decode("utf-8"))
            results.append((line, parsed))
        except Exception as e:
            results.append((line, {"error": f"JSON parse error: {e}"}))
    return results

def main():
    print("=" * 80)
    print("TASK B CONTEXT RETRIEVAL & BOUNDARY VERIFICATION")
    print(f"Target Repo: {GOLDEN_REPO}")
    print("=" * 80)
    
    requests = [
        {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "boundary_verifier", "version": "1.0"}
            }
        },
        {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "engram_get_context",
                "arguments": {
                    "task": TASK_B_PROMPT,
                    "token_budget": 2000
                }
            }
        },
        {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "engram_inspect_symbol",
                "arguments": {
                    "target": "AuthEffects"
                }
            }
        },
        {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "engram_preflight",
                "arguments": {
                    "planned_files": [
                        "projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts",
                        "projects/angular-ngrx-material-starter/src/app/core/settings/settings.reducer.ts"
                    ]
                }
            }
        },
        {
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "engram_postflight",
                "arguments": {
                    "changed_files": [
                        "projects/angular-ngrx-material-starter/src/app/core/auth/auth.effects.ts"
                    ]
                }
            }
        }
    ]

    summary_table = []

    for name, bin_path in CANDIDATES.items():
        print(f"\n--- Testing {name} ---")
        print(f"Binary: {bin_path}")
        assert bin_path.exists(), f"Binary {bin_path} does not exist!"
        
        raw_results = call_mcp(bin_path, GOLDEN_REPO, requests)
        print(f"Received {len(raw_results)} response lines.")
        
        cand_summary = {"candidate": name}
        
        for raw_line, parsed in raw_results:
            req_id = parsed.get("id")
            line_bytes = len(raw_line)
            
            if req_id == 1:
                print(f"  [init] Response size: {line_bytes} bytes")
            elif req_id == 2:
                # engram_get_context
                cand_summary["get_context_bytes"] = line_bytes
                tool_res = parsed.get("result", {})
                content = tool_res.get("content", [{}])[0].get("text", "")
                content_json = {}
                try:
                    content_json = json.loads(content)
                except Exception:
                    pass
                
                target_files = [f.get("path") for f in content_json.get("target_files", [])]
                omission_manifest = content_json.get("omission_manifest") or {}
                omitted_count = omission_manifest.get("omitted_items_count", 0)
                sample_count = len(omission_manifest.get("sample_omissions", omission_manifest.get("omission_manifest", [])))
                manifest_truncated = omission_manifest.get("manifest_truncated", False)
                
                cand_summary["get_context_target_files"] = target_files
                cand_summary["get_context_omitted_count"] = omitted_count
                cand_summary["get_context_sample_count"] = sample_count
                cand_summary["get_context_manifest_truncated"] = manifest_truncated
                
                print(f"  [engram_get_context] Response size: {line_bytes:,} bytes")
                print(f"    Target files ({len(target_files)}): {target_files[:3]}...")
                print(f"    Omitted items count: {omitted_count}")
                print(f"    Sample omissions count: {sample_count}")
                print(f"    Manifest truncated: {manifest_truncated}")
            elif req_id == 3:
                # engram_inspect_symbol (AuthEffects)
                cand_summary["inspect_symbol_bytes"] = line_bytes
                tool_res = parsed.get("result", {})
                content = tool_res.get("content", [{}])[0].get("text", "")
                content_json = {}
                try:
                    content_json = json.loads(content)
                except Exception:
                    pass
                cand_summary["inspect_status"] = content_json.get("status")
                cand_summary["inspect_symbol"] = content_json.get("symbol")
                print(f"  [engram_inspect_symbol] Response size: {line_bytes:,} bytes")
                print(f"    Status: {content_json.get('status')}, Symbol: {content_json.get('symbol')}")
            elif req_id == 4:
                # engram_preflight
                cand_summary["preflight_bytes"] = line_bytes
                print(f"  [engram_preflight] Response size: {line_bytes:,} bytes")
            elif req_id == 5:
                # engram_postflight
                cand_summary["postflight_bytes"] = line_bytes
                print(f"  [engram_postflight] Response size: {line_bytes:,} bytes")
                
        summary_table.append(cand_summary)

    print("\n" + "=" * 95)
    print("COMPARATIVE EVALUATION MATRIX (CANDIDATE 3 vs CANDIDATE 4 vs CANDIDATE 4.1)")
    print("=" * 95)
    hdr = f"{'Metric / MCP Tool':<35} | {'Candidate 3 (Legacy)':<18} | {'Candidate 4 (Live 02)':<18} | {'Candidate 4.1 (Remediated)':<20}"
    print(hdr)
    print("-" * 95)
    
    c3 = summary_table[0]
    c4 = summary_table[1]
    c41 = summary_table[2]
    
    print(f"{'engram_get_context Size':<35} | {c3.get('get_context_bytes', 0):>13,} B | {c4.get('get_context_bytes', 0):>13,} B | {c41.get('get_context_bytes', 0):>15,} B")
    print(f"{'  <= 16,384 Ceiling Compliant?':<35} | {str(c3.get('get_context_bytes', 0) <= 16384):>18} | {str(c4.get('get_context_bytes', 0) <= 16384):>18} | {str(c41.get('get_context_bytes', 0) <= 16384):>20}")
    print(f"{'  Target Files Retained':<35} | {len(c3.get('get_context_target_files', [])):>18} | {len(c4.get('get_context_target_files', [])):>18} | {len(c41.get('get_context_target_files', [])):>20}")
    print(f"{'  Total Omissions Recorded':<35} | {c3.get('get_context_omitted_count', 0):>18} | {c4.get('get_context_omitted_count', 0):>18} | {c41.get('get_context_omitted_count', 0):>20}")
    print(f"{'  Sample Omissions Emitted':<35} | {c3.get('get_context_sample_count', 0):>18} | {c4.get('get_context_sample_count', 0):>18} | {c41.get('get_context_sample_count', 0):>20}")
    print(f"{'  Manifest Truncated Flag':<35} | {str(c3.get('get_context_manifest_truncated', False)):>18} | {str(c4.get('get_context_manifest_truncated', False)):>18} | {str(c41.get('get_context_manifest_truncated', False)):>20}")
    print(f"{'engram_inspect_symbol Size':<35} | {c3.get('inspect_symbol_bytes', 0):>13,} B | {c4.get('inspect_symbol_bytes', 0):>13,} B | {c41.get('inspect_symbol_bytes', 0):>15,} B")
    print(f"{'  <= 16,384 Ceiling Compliant?':<35} | {str(c3.get('inspect_symbol_bytes', 0) <= 16384):>18} | {str(c4.get('inspect_symbol_bytes', 0) <= 16384):>18} | {str(c41.get('inspect_symbol_bytes', 0) <= 16384):>20}")
    print(f"{'engram_preflight Size':<35} | {c3.get('preflight_bytes', 0):>13,} B | {c4.get('preflight_bytes', 0):>13,} B | {c41.get('preflight_bytes', 0):>15,} B")
    print(f"{'engram_postflight Size':<35} | {c3.get('postflight_bytes', 0):>13,} B | {c4.get('postflight_bytes', 0):>13,} B | {c41.get('postflight_bytes', 0):>15,} B")
    print("=" * 95)
    
    # Assertions for Candidate 4.1 readiness
    assert c41.get('get_context_bytes', 0) <= 16384, f"Candidate 4.1 engram_get_context exceeded 16,384: {c41.get('get_context_bytes')}"
    assert c41.get('inspect_symbol_bytes', 0) <= 16384, f"Candidate 4.1 engram_inspect_symbol exceeded 16,384: {c41.get('inspect_symbol_bytes')}"
    assert c41.get('preflight_bytes', 0) <= 16384, f"Candidate 4.1 engram_preflight exceeded 16,384: {c41.get('preflight_bytes')}"
    assert c41.get('postflight_bytes', 0) <= 16384, f"Candidate 4.1 engram_postflight exceeded 16,384: {c41.get('postflight_bytes')}"
    print("\nALL CANDIDATE 4.1 MCP BOUNDARY ASSERTIONS PASSED!")

if __name__ == "__main__":
    main()
