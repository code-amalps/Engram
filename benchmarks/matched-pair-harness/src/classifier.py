"""
Safe Task Classifier for Matched-Pair Benchmark Harness.

Determines whether an incoming coding-agent task qualifies for the zero-overhead
Native Fast Path (bypassing MCP registration and Turn-0 directives) or must fail open
to full Engram semantic indexing and change-contract governance.

Guiding Principles:
1. Conservative by design: Only unambiguously trivial, single-file leaf edits qualify.
2. Strict fail-open: Any architectural, API, state, dependency, multi-file, or ambiguous intent
   must route to full Engram mode.
3. Word-boundary regex: Avoids substring false positives (e.g., 'capitalization' matching 'api').
"""

import re
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional


@dataclass
class TaskClassificationResult:
    """Outcome of conservative task classification."""
    is_fast_path: bool
    mode: str  # 'FAST_PATH' or 'ENGRAM_MODE'
    reasons: List[str] = field(default_factory=list)
    blocked_by: List[str] = field(default_factory=list)
    escalation_hint: Optional[str] = None


# Strict word-boundary patterns for architectural and cross-file keywords.
# Any match immediately disqualifies a task from Fast Path.
ARCHITECTURAL_KEYWORDS = [
    # State management
    r"\bstate\b", r"\bstore\b", r"\breducer\b", r"\beffect\b", r"\bselector\b",
    r"\baction\b", r"\bdispatch\b", r"\bobservable\b", r"\bsubject\b", r"\bsignal\b",
    # APIs & Network
    r"\bapi\b", r"\bendpoint\b", r"\bhttp\b", r"\brest\b", r"\bgraphql\b",
    r"\broute\b", r"\brouter\b", r"\bcontroller\b", r"\binterface\b", r"\bschema\b",
    # Authentication & Security
    r"\bauth\b", r"\blogin\b", r"\bsession\b", r"\btoken\b", r"\bcredential\b",
    r"\bpermission\b", r"\bremember\b", r"\bsecurity\b", r"\boauth\b",
    # Modules & Dependencies
    r"\bimport\b", r"\bexport\b", r"\bmodule\b", r"\bpackage(?!\.json)\b", r"\bdependency\b",
    r"\bservice\b", r"\bprovider\b", r"\binjection\b", r"\binject\b",
    # Refactoring & Multi-step features
    r"\brefactor\b", r"\bmigrate\b", r"\breorganize\b", r"\bimplement\b",
    r"\bfeature\b", r"\bintegrate\b", r"\bcreate\b", r"\bdelete\b",
    # Configuration & Build
    r"\bconfig\b", r"\bsettings\b", r"\benvironment\b", r"\benv\b",
    r"\bbuild\b", r"\bwebpack\b", r"\btsconfig\b"
]

# Explicit localized typographical/text replacement keywords.
# Fast path requires at least one of these to confirm micro-edit intent.
TYPOGRAPHICAL_KEYWORDS = [
    r"\btypo\b", r"\bcapitalization\b", r"\bspelling\b",
    r"\bspelling-mistake\b", r"\bmisspelling\b", r"\brename-string\b"
]

COMPILED_ARCH_PATTERNS = [re.compile(p, re.IGNORECASE) for p in ARCHITECTURAL_KEYWORDS]
COMPILED_TYPO_PATTERNS = [re.compile(p, re.IGNORECASE) for p in TYPOGRAPHICAL_KEYWORDS]


def classify_task_efficiency(
    prompt: str,
    target_files: Optional[List[str]] = None,
    task_config: Optional[Dict[str, Any]] = None,
    workspace_root: Optional[Path] = None
) -> TaskClassificationResult:
    """
    Classifies whether a task should execute via Fast Path or full Engram mode.

    Fail-Open Invariants:
    1. If target_files is empty, None, or has > 1 file -> FAIL OPEN to ENGRAM_MODE.
    2. If prompt matches any architectural or cross-file pattern -> FAIL OPEN to ENGRAM_MODE.
    3. If prompt does not match an explicit typographical pattern -> FAIL OPEN to ENGRAM_MODE.
    4. If the single target file matches an architectural pattern (e.g. *.reducer.ts, module.ts) -> FAIL OPEN.
    """
    reasons: List[str] = []
    blocked_by: List[str] = []

    # 1. Target files check
    effective_targets = target_files
    if effective_targets is None and task_config:
        effective_targets = (
            task_config.get("target_files")
            or task_config.get("allowed_file_patterns")
            or task_config.get("expected_files")
        )
        if not effective_targets:
            if "fixture_preflight" in task_config and isinstance(task_config["fixture_preflight"], dict):
                single = task_config["fixture_preflight"].get("target_file")
                if single:
                    effective_targets = [single]
            elif "target_file" in task_config:
                single = task_config.get("target_file")
                effective_targets = [single] if single else []

    if not effective_targets:
        blocked_by.append("Missing explicit target file specification (unbounded scope)")
        return TaskClassificationResult(
            is_fast_path=False,
            mode="ENGRAM_MODE",
            reasons=["Unbounded or unspecified target files requires full semantic context"],
            blocked_by=blocked_by
        )

    # Wildcard checks
    for pat in effective_targets:
        if "*" in pat or "?" in pat:
            blocked_by.append(f"Target pattern contains glob wildcards: '{pat}'")
            return TaskClassificationResult(
                is_fast_path=False,
                mode="ENGRAM_MODE",
                reasons=["Wildcard target patterns indicate unbounded or structural scope"],
                blocked_by=blocked_by
            )

    if len(effective_targets) != 1:
        blocked_by.append(f"Multi-file target list ({len(effective_targets)} files specified)")
        return TaskClassificationResult(
            is_fast_path=False,
            mode="ENGRAM_MODE",
            reasons=["Multi-file tasks require cross-file semantic indexing and Change Contracts"],
            blocked_by=blocked_by
        )

    single_file = effective_targets[0]
    single_file_lower = single_file.lower()

    # 2. Target file architectural naming check
    arch_file_patterns = [
        "reducer", "effect", "selector", "service", "module", "store",
        "action", "routing", "config", "environment"
    ]
    if any(pat in single_file_lower for pat in arch_file_patterns):
        blocked_by.append(f"Target file '{single_file}' is an architectural component")
        return TaskClassificationResult(
            is_fast_path=False,
            mode="ENGRAM_MODE",
            reasons=[f"File '{single_file}' carries structural blast radius risk"],
            blocked_by=blocked_by
        )

    # Strip standard harness negative safety boilerplate if present (e.g. "Do NOT modify package.json...")
    clean_prompt = prompt
    for marker in ["CRITICAL SAFETY INSTRUCTION:", "VERIFICATION GUIDANCE:"]:
        if marker in clean_prompt:
            clean_prompt = clean_prompt.split(marker)[0]

    # 3. Architectural keywords check in prompt (Word-boundary matching)
    for pattern in COMPILED_ARCH_PATTERNS:
        match = pattern.search(clean_prompt)
        if match:
            matched_term = match.group(0)
            blocked_by.append(f"Prompt matches architectural keyword: '{matched_term}'")

    if blocked_by:
        return TaskClassificationResult(
            is_fast_path=False,
            mode="ENGRAM_MODE",
            reasons=["Architectural or cross-file keywords detected in task prompt"],
            blocked_by=blocked_by
        )

    # 4. Explicit typographical intent check
    has_typo_intent = False
    matched_intent = ""
    for pattern in COMPILED_TYPO_PATTERNS:
        match = pattern.search(clean_prompt)
        if match:
            has_typo_intent = True
            matched_intent = match.group(0)
            break

    if not has_typo_intent:
        blocked_by.append("Prompt lacks explicit localized typographical or textual replacement keywords")
        return TaskClassificationResult(
            is_fast_path=False,
            mode="ENGRAM_MODE",
            reasons=["Task intent is not provably a leaf micro-edit"],
            blocked_by=blocked_by
        )

    # All conservative checks passed -> Eligible for Fast Path
    reasons.append(f"Single leaf target file: {single_file}")
    reasons.append(f"Explicit localized micro-edit intent: '{matched_intent}'")
    reasons.append("Zero architectural or cross-file keywords detected")

    escalation_hint = (
        "<!-- FAST-PATH: Single-file micro-edit mode. If unanticipated cross-file dependencies "
        "are discovered during execution, run 'engram mcp' via shell to access the semantic index. -->"
    )

    return TaskClassificationResult(
        is_fast_path=True,
        mode="FAST_PATH",
        reasons=reasons,
        blocked_by=[],
        escalation_hint=escalation_hint
    )
