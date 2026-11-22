"""
Deterministic Task Complexity Classifier & Adaptive Activation Router for Engram Benchmark Harness.

Classification Rules:
- SIMPLE  -> BYPASS (Zero MCP tools, zero overhead, pure native execution)
- MEDIUM  -> LEAN   (Lightweight context tools, read-only, strict 1,200 token budget)
- COMPLEX -> FULL   (Full bounded tool suite, 16 KB response limit, impact cone pruning)

Invariants:
1. Deterministic: No LLM calls used for task classification.
2. Latency Measurement: Measures actual classification time in milliseconds using time.perf_counter().
3. Pre-Dispatch Registration: Classification result is sealed before any LLM inference or tool execution.
4. Arm Integrity: Escalation records transitions without silently mutating the assigned benchmark arm identity.
5. Override Support: Explicit overrides are strictly tracked and audited.
"""

import re
import time
import datetime
from dataclasses import dataclass, field
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple, Any


SIMPLE_TRIGGERS: Set[str] = {
    "typo", "fix typo", "spelling", "rename symbol", "single line",
    "off-by-one", "grammar", "misspelling", "one-line", "comment fix"
}

COMPLEX_TRIGGERS: Set[str] = {
    "feature", "implement", "architecture", "cross-cutting", "refactor",
    "state management", "ngrx", "redux", "effect", "effects", "store",
    "remember me", "theme toggle", "full-stack", "authentication",
    "migrate", "migration", "integration", "persistence", "localstorage"
}

FILE_PATTERN = re.compile(r'[\w\-\.\/]+\.(?:ts|js|tsx|jsx|rs|py|html|css|json|go|java|cs|sql)')


@dataclass
class EscalationEvent:
    from_mode: str
    to_mode: str
    reason: str
    timestamp: str

    def to_dict(self) -> Dict[str, Any]:
        return {
            "from_mode": self.from_mode,
            "to_mode": self.to_mode,
            "reason": self.reason,
            "timestamp": self.timestamp,
        }


@dataclass
class ClassificationResult:
    tier: str  # "SIMPLE", "MEDIUM", "COMPLEX"
    routed_mode: str  # "BYPASS", "LEAN", "FULL"
    reasoning: str
    execution_time_ms: float
    signals: Dict[str, Any]
    override_applied: Optional[str] = None
    escalation_history: List[EscalationEvent] = field(default_factory=list)

    def escalate(self, target_mode: str, reason: str) -> None:
        """Records an explicit escalation without modifying the initial classification."""
        valid_modes = {"BYPASS", "LEAN", "FULL"}
        if target_mode not in valid_modes:
            raise ValueError(f"Invalid escalation target mode: {target_mode}. Must be one of {valid_modes}")
        event = EscalationEvent(
            from_mode=self.routed_mode,
            to_mode=target_mode,
            reason=reason,
            timestamp=datetime.datetime.now(datetime.timezone.utc).isoformat(),
        )
        self.escalation_history.append(event)
        self.routed_mode = target_mode

    def to_dict(self) -> Dict[str, Any]:
        return {
            "tier": self.tier,
            "routed_mode": self.routed_mode,
            "reasoning": self.reasoning,
            "execution_time_ms": round(self.execution_time_ms, 4),
            "signals": self.signals,
            "override_applied": self.override_applied,
            "escalation_count": len(self.escalation_history),
            "escalation_history": [e.to_dict() for e in self.escalation_history],
        }


class TaskComplexityClassifier:
    """
    Deterministic rule-based task classifier and router.
    Evaluates input task prompts and repository context to select the optimal Engram operating mode.
    """

    @staticmethod
    def extract_files(text: str) -> List[str]:
        return sorted(list(set(FILE_PATTERN.findall(text))))

    @classmethod
    def classify(
        cls,
        task_prompt: str,
        repo_path: Optional[Path] = None,
        explicit_override: Optional[str] = None,
    ) -> ClassificationResult:
        start_time = time.perf_counter()

        # Handle explicit override
        if explicit_override:
            override_upper = explicit_override.strip().upper()
            valid_modes = {"BYPASS": "SIMPLE", "LEAN": "MEDIUM", "FULL": "COMPLEX"}
            if override_upper in valid_modes:
                elapsed_ms = (time.perf_counter() - start_time) * 1000.0
                return ClassificationResult(
                    tier=valid_modes[override_upper],
                    routed_mode=override_upper,
                    reasoning=f"Explicit override applied: {override_upper}",
                    execution_time_ms=elapsed_ms,
                    signals={"override": override_upper},
                    override_applied=override_upper,
                )

        prompt_lower = task_prompt.lower()
        words = prompt_lower.split()
        word_count = len(words)

        # Detect files mentioned and extract instruction text separate from paths
        files_mentioned = cls.extract_files(task_prompt)
        file_count = len(files_mentioned)
        prompt_instruction_only = FILE_PATTERN.sub(' ', prompt_lower)

        # Detect triggers on instruction text using word boundaries
        matched_simple = [
            kw for kw in SIMPLE_TRIGGERS
            if re.search(r'\b' + re.escape(kw) + r'\b', prompt_instruction_only)
        ]
        matched_complex = [
            kw for kw in COMPLEX_TRIGGERS
            if re.search(r'\b' + re.escape(kw) + r'\b', prompt_instruction_only)
        ]

        signals = {
            "word_count": word_count,
            "files_mentioned": files_mentioned,
            "file_count": file_count,
            "matched_simple_triggers": matched_simple,
            "matched_complex_triggers": matched_complex,
        }

        # Classification decision hierarchy:
        # 1. Tier 1 (SIMPLE) -> BYPASS
        if matched_simple and not matched_complex:
            tier = "SIMPLE"
            routed_mode = "BYPASS"
            reasoning = f"Isolated fix detected ({', '.join(matched_simple)}); routing to BYPASS to eliminate protocol overhead."
        elif file_count == 1 and not matched_complex and word_count < 120:
            tier = "SIMPLE"
            routed_mode = "BYPASS"
            reasoning = f"Single file edit ({files_mentioned[0]}) with narrow scope; routing to BYPASS."
        # 2. Tier 3 (COMPLEX) -> FULL
        elif matched_complex or file_count >= 3 or word_count >= 250:
            tier = "COMPLEX"
            routed_mode = "FULL"
            triggers_desc = f"triggers: {', '.join(matched_complex)}" if matched_complex else f"{file_count} files mentioned"
            reasoning = f"Cross-cutting architectural feature detected ({triggers_desc}); routing to FULL with bounded impact retrieval."
        # 3. Tier 2 (MEDIUM) -> LEAN
        else:
            tier = "MEDIUM"
            routed_mode = "LEAN"
            reasoning = "Scoped multi-file modification; routing to LEAN read-only context retrieval."

        elapsed_ms = (time.perf_counter() - start_time) * 1000.0

        return ClassificationResult(
            tier=tier,
            routed_mode=routed_mode,
            reasoning=reasoning,
            execution_time_ms=elapsed_ms,
            signals=signals,
            override_applied=None,
        )


# ==============================================================================
# Backward-Compatible Two-Arm / Triad Task Efficiency Classifier (Phases 1-3)
# ==============================================================================

@dataclass
class TaskClassificationResult:
    """Outcome of conservative task classification for legacy 2-arm / triad runners."""
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

