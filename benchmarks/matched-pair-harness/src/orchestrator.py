"""
Matched-Pair Benchmark Orchestrator.
Coordinates isolated cold-start execution of paired benchmark arms (Control A vs Treatment B),
enforces pre-registered coin-flip execution ordering, verifies golden dependency tree integrity
before and after every run, runs automated verification gates, parses exact telemetry,
calculates independent correctness, and validates against JSON schemas.
"""
from typing import Dict, Any, List, Optional
from dataclasses import dataclass
from pathlib import Path
import argparse
import random
import datetime
import json
import sys
import os
import hashlib

from isolation import (
    verify_frozen_binary,
    create_sandbox,
    teardown_sandbox,
    compute_dependency_fingerprint,
    compute_comprehensive_golden_manifest,
    compute_cargo_manifest,
    verify_dependency_integrity,
    verify_sandbox_identity,
    verify_sandbox_cleanliness,
    ProcessTracker,
    SafetyViolationError,
    FROZEN_ENGRAM_SHA256,
    FROZEN_ENGRAM_V151_SHA256,
    FROZEN_ENGRAM_V154_SHA256,
)
from telemetry import TelemetryParser, TelemetryMetrics
from evaluator import (
    Evaluator,
    GitImpact,
    CorrectnessResult,
    FixturePreflightError,
    validate_fixture_preflight,
)
from runner import CodexRunner, RunExecutionResult, SUPPORTED_PROVIDERS
from schema_validator import validate_instance, SchemaValidationError


DEFAULT_GOLDEN_REPO = Path(r"C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter")
DEFAULT_CODEX_BIN = Path(r"C:\Users\Amal P S\.codex\plugins\.plugin-appserver\codex.exe")
DEFAULT_ENGRAM_BIN = Path(r"C:\Users\Amal P S\OneDrive\Projects\Engram\target\release\engram.exe")
HARNESS_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_ENGRAM_V151_BIN = HARNESS_ROOT / "bin" / "v1.5.1" / "engram.exe"
DEFAULT_ENGRAM_V154_BIN = HARNESS_ROOT / "bin" / "v1.5.4" / "engram.exe"

TRIAD_BLOCK_SCHEDULES = {
    "block_01": {"task": "task_a_typo", "order": ["baseline_arm1", "v151_arm2", "v154_arm3"], "seed": 2026102801},
    "block_02": {"task": "task_a_typo", "order": ["v151_arm2", "v154_arm3", "baseline_arm1"], "seed": 2026102802},
    "block_03": {"task": "task_a_typo", "order": ["v154_arm3", "baseline_arm1", "v151_arm2"], "seed": 2026102803},
    "block_04": {"task": "task_a_typo", "order": ["baseline_arm1", "v154_arm3", "v151_arm2"], "seed": 2026102804},
    "block_05": {"task": "task_a_typo", "order": ["v151_arm2", "baseline_arm1", "v154_arm3"], "seed": 2026102805},
    "block_06": {"task": "task_b_remember_me", "order": ["baseline_arm1", "v151_arm2", "v154_arm3"], "seed": 2026102806},
    "block_07": {"task": "task_b_remember_me", "order": ["v151_arm2", "v154_arm3", "baseline_arm1"], "seed": 2026102807},
    "block_08": {"task": "task_b_remember_me", "order": ["v154_arm3", "baseline_arm1", "v151_arm2"], "seed": 2026102808},
    "block_09": {"task": "task_b_remember_me", "order": ["baseline_arm1", "v154_arm3", "v151_arm2"], "seed": 2026102809},
    "block_10": {"task": "task_b_remember_me", "order": ["v151_arm2", "baseline_arm1", "v154_arm3"], "seed": 2026102810},
}


@dataclass(frozen=True)
class ArmCandidateIdentity:
    """
    Authoritative, immutable candidate identity for an individual benchmark arm.
    Strictly disentangles the global benchmark candidate selection from the per-arm
    binary and version metadata.
    """
    arm: str
    requested_candidate_version: str
    resolved_candidate_version: str
    binary_path: Optional[Path]
    expected_sha256: Optional[str]
    is_engram: bool

    def validate_against_ledger(self, ledger: Dict[str, Dict[str, Any]]) -> None:
        """
        Validates this arm identity against the authoritative candidate ledger.
        Fails closed on any inconsistency between arm configuration, candidate version,
        binary path, and expected hash.
        """
        if not self.is_engram:
            if self.requested_candidate_version != "native" or self.resolved_candidate_version != "native":
                raise SafetyViolationError(
                    f"Native arm '{self.arm}' candidate inconsistency: "
                    f"requested='{self.requested_candidate_version}', resolved='{self.resolved_candidate_version}'"
                )
            if self.binary_path is not None or self.expected_sha256 is not None:
                raise SafetyViolationError(
                    f"Native arm '{self.arm}' cannot specify binary path or hash: "
                    f"path='{self.binary_path}', expected_sha='{self.expected_sha256}'"
                )
            return

        if self.arm in ("treatment_B", "v151_arm2", "v154_arm3"):
            # Legacy pair and triad arms support custom test paths if specified
            return

        if self.resolved_candidate_version not in ledger:
            raise SafetyViolationError(
                f"Resolved candidate version '{self.resolved_candidate_version}' for arm '{self.arm}' "
                f"not found in candidate ledger."
            )

        ledger_entry = ledger[self.resolved_candidate_version]
        expected_bin = ledger_entry["bin"]
        expected_sha = ledger_entry["sha"]

        if expected_bin is None or expected_sha is None:
            raise SafetyViolationError(
                f"Candidate ledger entry for '{self.resolved_candidate_version}' has null binary or hash."
            )

        if self.binary_path is not None and expected_bin is not None:
            if Path(self.binary_path).resolve() != Path(expected_bin).resolve():
                raise SafetyViolationError(
                    f"Binary path mismatch for arm '{self.arm}': "
                    f"expected '{expected_bin}', got '{self.binary_path}'"
                )

        if self.expected_sha256 != expected_sha:
            raise SafetyViolationError(
                f"Expected SHA-256 mismatch for arm '{self.arm}': "
                f"expected '{expected_sha}', got '{self.expected_sha256}'"
            )

    def verify_binary_integrity(self) -> str:
        """
        Verifies the physical binary exists and has matching SHA-256.
        Returns the verified uppercase SHA-256 string, or 'NONE' for native arms.
        Fails closed on missing file or hash mismatch.
        """
        if not self.is_engram:
            return "NONE"
        if not self.binary_path or not Path(self.binary_path).exists():
            raise FileNotFoundError(
                f"Candidate binary missing for arm '{self.arm}': {self.binary_path}"
            )
        actual_sha = hashlib.sha256(Path(self.binary_path).read_bytes()).hexdigest().upper()
        if actual_sha != self.expected_sha256:
            raise SafetyViolationError(
                f"Pre-launch binary hash verification failed for arm '{self.arm}': "
                f"expected {self.expected_sha256}, got {actual_sha}"
            )
        return actual_sha


def audit_arm_candidate_identity(telemetry_record: Dict[str, Any]) -> Dict[str, Any]:
    """
    Forensic helper to inspect and audit candidate identity from a run telemetry record.
    Distinguishes historical records from corrected records, and detects metadata mismatches.
    """
    arm = telemetry_record.get("arm", "unknown")
    iso = telemetry_record.get("isolation", {})
    actual_sha = iso.get("engram_binary_actual_sha256")
    expected_sha = iso.get("engram_binary_expected_sha256")
    bin_path = iso.get("engram_binary_path")
    benchmark_ver = iso.get("benchmark_candidate_version")
    requested_ver = iso.get("requested_candidate_version")
    resolved_ver = iso.get("resolved_candidate_version")
    cand_status = iso.get("candidate_verification_status")
    mcp_verified = iso.get("mcp_registration_verified")

    inconsistencies = []

    # Detect legacy records lacking new fields
    is_legacy = benchmark_ver is None and requested_ver is None and resolved_ver is None

    # Check for known historical Arm B defect
    if arm in ("arm_b_cand3", "arm_b_cand3_always_on"):
        CAND3_SHA = "97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777"
        if resolved_ver in ("cand4", "cand4_1", "cand4_2", "cand5"):
            inconsistencies.append(
                f"Metadata defect: arm '{arm}' resolved_candidate_version='{resolved_ver}', "
                f"but actual binary is Candidate 3 ({actual_sha})"
            )
        if actual_sha and actual_sha != CAND3_SHA:
            inconsistencies.append(
                f"Binary mismatch: arm '{arm}' actual SHA '{actual_sha}' does not match Candidate 3 '{CAND3_SHA}'"
            )

    if arm == "arm_e_cand5_optimized":
        CAND5_SHA = "E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44"
        if resolved_ver and resolved_ver != "cand5":
            inconsistencies.append(
                f"Metadata defect: arm '{arm}' resolved_candidate_version='{resolved_ver}', "
                f"expected 'cand5'"
            )
        if actual_sha and actual_sha != CAND5_SHA:
            inconsistencies.append(
                f"Binary mismatch: arm '{arm}' actual SHA '{actual_sha}' does not match Candidate 5 '{CAND5_SHA}'"
            )

    if expected_sha and actual_sha and expected_sha != actual_sha:
        inconsistencies.append(f"SHA mismatch: expected '{expected_sha}' != actual '{actual_sha}'")

    if arm in ("arm_a_native", "control_A", "baseline_arm1"):
        if bin_path or actual_sha or (resolved_ver and resolved_ver != "native"):
            inconsistencies.append(f"Native arm '{arm}' has unexpected binary or candidate version")

    return {
        "arm": arm,
        "is_legacy": is_legacy,
        "benchmark_candidate_version": benchmark_ver,
        "requested_candidate_version": requested_ver,
        "resolved_candidate_version": resolved_ver,
        "engram_binary_path": bin_path,
        "engram_binary_expected_sha256": expected_sha,
        "engram_binary_actual_sha256": actual_sha,
        "candidate_verification_status": cand_status,
        "mcp_registration_verified": mcp_verified,
        "inconsistencies": inconsistencies,
        "is_consistent": len(inconsistencies) == 0,
    }


def resolve_execution_policy(
    execution_policy: Optional[str] = None,
    no_stop_on_failure: bool = False,
    parser: Optional[Any] = None,
) -> Tuple[str, bool]:
    """
    Resolves the effective execution policy and stop_on_failure boolean flag.

    Precedence and validation rules:
    1. Default (execution_policy=None, no_stop_on_failure=False) -> ('strict', True)
    2. execution_policy='strict' and no_stop_on_failure=True -> parser error (code 2) / ValueError (Conflict)
    3. execution_policy=None and no_stop_on_failure=True -> ('diagnostic_continue', False) + DeprecationWarning
    4. execution_policy='diagnostic_continue' and no_stop_on_failure=True -> ('diagnostic_continue', False) + DeprecationWarning
    5. execution_policy specified alone -> (execution_policy, stop_on_failure=(execution_policy == 'strict'))
    """
    if execution_policy == "strict" and no_stop_on_failure:
        msg = "Conflicting arguments: --no-stop-on-failure cannot be combined with --execution-policy strict."
        if parser is not None:
            parser.error(msg)
        else:
            raise ValueError(msg)

    if no_stop_on_failure:
        import warnings
        warning_msg = (
            "[DEPRECATION WARNING] --no-stop-on-failure is deprecated. "
            "Please use --execution-policy diagnostic_continue instead."
        )
        print(warning_msg, file=sys.stderr)
        warnings.warn(warning_msg, DeprecationWarning, stacklevel=2)
        return "diagnostic_continue", False

    if execution_policy is None:
        return "strict", True

    return execution_policy, (execution_policy == "strict")


class Orchestrator:
    def __init__(
        self,
        task_config_path: Path,
        pair_id: str,
        golden_repo: Path = DEFAULT_GOLDEN_REPO,
        codex_bin: Path = DEFAULT_CODEX_BIN,
        engram_bin: Path = DEFAULT_ENGRAM_BIN,
        model: str = "gpt-5.6-luna",
        reasoning_effort: str = "medium",
        output_dir: Optional[Path] = None,
        is_dry_run: bool = False,
        dependency_strategy: str = "copy",
        preserve_sandboxes: bool = False,
        seed: Optional[int] = None,
        expected_treatment_sha256: Optional[str] = None,
        engram_v151_bin: Optional[Path] = None,
        engram_v154_bin: Optional[Path] = None,
        expected_v151_sha256: Optional[str] = None,
        expected_v154_sha256: Optional[str] = None,
        stop_on_failure: bool = True,
        max_tokens_ceiling: int = 250_000,
        headroom_reserve: int = 45_000,
        sandbox_policy: Optional[str] = None,
        provider: str = "cloud",
        candidate_version: Optional[str] = None,
        execution_policy: str = "strict",
        global_tokens_ceiling: Optional[int] = None,
        runner: Optional[Any] = None,
    ):
        if provider not in SUPPORTED_PROVIDERS:
            raise ValueError(
                f"Unsupported provider '{provider}'. "
                f"Supported values: {sorted(SUPPORTED_PROVIDERS)}"
            )
        self.provider = provider
        self.task_config_path = Path(task_config_path)
        with open(self.task_config_path, "r", encoding="utf-8") as f:
            self.task_config = json.load(f)

        self.pair_id = pair_id
        self.task_id = self.task_config.get("task_id", "unknown_task")

        # Resolve golden repository from task config if specified, else use parameter
        repo_type_hint = self.task_config.get("repository_type")
        if self.task_config.get("source_repo"):
            self.golden_repo = Path(self.task_config["source_repo"]).resolve()
        elif golden_repo is not None and golden_repo != DEFAULT_GOLDEN_REPO:
            self.golden_repo = Path(golden_repo).resolve()
        elif repo_type_hint in ("cargo", "rust"):
            self.golden_repo = HARNESS_ROOT.parent.parent.resolve()
        else:
            self.golden_repo = Path(golden_repo or DEFAULT_GOLDEN_REPO).resolve()

        # Repository type detection (cargo vs angular)
        self.repo_type = self.task_config.get("repository_type")
        if not self.repo_type:
            if (self.golden_repo / "Cargo.toml").exists() and not (self.golden_repo / "package.json").exists():
                self.repo_type = "cargo"
            else:
                self.repo_type = "angular"
        self.codex_bin = Path(codex_bin)
        self.engram_bin = Path(engram_bin)
        self.engram_v151_bin = Path(engram_v151_bin) if engram_v151_bin else DEFAULT_ENGRAM_V151_BIN
        self.engram_v154_bin = Path(engram_v154_bin) if engram_v154_bin else DEFAULT_ENGRAM_V154_BIN
        self.expected_treatment_sha256 = expected_treatment_sha256 or FROZEN_ENGRAM_SHA256
        self.expected_v151_sha256 = expected_v151_sha256 or FROZEN_ENGRAM_V151_SHA256
        self.expected_v154_sha256 = expected_v154_sha256 or (FROZEN_ENGRAM_V154_SHA256 if not engram_v154_bin else None)
        if execution_policy not in ("strict", "diagnostic_continue"):
            raise ValueError(
                f"Invalid execution_policy '{execution_policy}'. Allowed choices: ('strict', 'diagnostic_continue')"
            )
        if execution_policy == "strict" and not stop_on_failure:
            raise ValueError("Conflicting arguments: stop_on_failure=False cannot be combined with execution_policy='strict'")
        self.execution_policy = execution_policy
        self.stop_on_failure = (execution_policy == "strict")

        if global_tokens_ceiling is not None:
            if not isinstance(global_tokens_ceiling, int) or global_tokens_ceiling <= 0:
                raise ValueError(
                    f"global_tokens_ceiling must be a positive integer, got: {global_tokens_ceiling}"
                )
        self.global_tokens_ceiling = global_tokens_ceiling

        self.max_tokens_ceiling = max_tokens_ceiling
        self.headroom_reserve = headroom_reserve

        cand3_bin = HARNESS_ROOT / "bin" / "candidate-phase3" / "engram.exe"
        cand4_bin = HARNESS_ROOT / "bin" / "candidate-phase4" / "engram.exe"
        cand4_1_bin = HARNESS_ROOT / "bin" / "candidate-phase4.1" / "engram.exe"
        cand4_2_bin = HARNESS_ROOT / "bin" / "candidate-phase4.2" / "engram.exe"
        cand5_bin = HARNESS_ROOT / "bin" / "candidate-phase5" / "engram.exe"
        cand3_sha = "97C5EC47E3704E16F9046623C2034057708005C35460DEAA6829EA045A96A777"
        cand4_sha = "FC605BC0CF3B6C9C5FDB1E7C83EE666FD7E049FD9AE25C893D478578681699F8"
        cand4_1_sha = "300ACE35A31E8E4AF6DD531E8B83B0A5BA9F03E70B27F5E7823E3650CE494015"
        cand4_2_sha = "78B84880AB5E7124593FD21669F8A1EB46DC223686601F2142EF6125BA559DD2"
        cand5_sha = "E4E5321624817CF20B6D300BF88897BFC1673F52308D75B21AEC3D143269AE44"

        CANDIDATE_LEDGER = {
            "native": {"bin": None, "sha": None, "label": "Native"},
            "cand3": {"bin": cand3_bin, "sha": cand3_sha, "label": "Candidate 3"},
            "cand4": {"bin": cand4_bin, "sha": cand4_sha, "label": "Candidate 4"},
            "cand4_1": {"bin": cand4_1_bin, "sha": cand4_1_sha, "label": "Candidate 4.1"},
            "cand4_2": {"bin": cand4_2_bin, "sha": cand4_2_sha, "label": "Candidate 4.2"},
            "cand5": {"bin": cand5_bin, "sha": cand5_sha, "label": "Candidate 5 (Phase 5.8.1)"},
        }

        self.requested_candidate_version = candidate_version
        self.candidate_ledger = CANDIDATE_LEDGER

        # Check for legacy environment variables
        env_cand4_2 = os.environ.get("ENGRAM_USE_CANDIDATE_4_2", "").strip().lower() in ("1", "true", "yes")
        env_cand4_1 = os.environ.get("ENGRAM_USE_CANDIDATE_4_1", "").strip().lower() in ("1", "true", "yes")

        if candidate_version is not None:
            if candidate_version not in CANDIDATE_LEDGER:
                raise ValueError(
                    f"Invalid candidate version '{candidate_version}'. Allowed choices: {sorted(list(CANDIDATE_LEDGER.keys()))}"
                )
            # Fail closed on conflicting legacy environment variables
            if candidate_version == "cand4" and env_cand4_2:
                raise ValueError(
                    "Conflicting configuration: explicit --candidate-version 'cand4' conflicts with legacy environment variable ENGRAM_USE_CANDIDATE_4_2=1"
                )
            if candidate_version == "cand4" and env_cand4_1:
                raise ValueError(
                    "Conflicting configuration: explicit --candidate-version 'cand4' conflicts with legacy environment variable ENGRAM_USE_CANDIDATE_4_1=1"
                )
            resolved_cand = candidate_version
        else:
            # Backward-compatibility fallback using legacy env vars if explicit flag is omitted
            if env_cand4_2:
                resolved_cand = "cand4_2"
            elif env_cand4_1:
                resolved_cand = "cand4_1"
            else:
                resolved_cand = "cand4"

        # Fail-closed binary existence and integrity check
        cand_entry = CANDIDATE_LEDGER[resolved_cand]
        if cand_entry["bin"] is not None:
            target_bin = cand_entry["bin"]
            if not target_bin.exists():
                raise FileNotFoundError(
                    f"Requested candidate '{resolved_cand}' binary not found at '{target_bin}'. "
                    f"Fail-closed: refusing silent fallback to another version."
                )
            actual_hash = hashlib.sha256(target_bin.read_bytes()).hexdigest().upper()
            if actual_hash != cand_entry["sha"]:
                raise SafetyViolationError(
                    f"SHA-256 mismatch for candidate '{resolved_cand}': expected {cand_entry['sha']}, got {actual_hash}"
                )
            active_cand4_bin = target_bin
            active_cand4_sha = cand_entry["sha"]
        else:
            active_cand4_bin = None
            active_cand4_sha = None

        self.resolved_candidate_version = resolved_cand
        self.benchmark_candidate_version = resolved_cand

        cand4_req = candidate_version if candidate_version is not None else resolved_cand

        self.arm_identities: Dict[str, ArmCandidateIdentity] = {
            "control_A": ArmCandidateIdentity(
                arm="control_A",
                requested_candidate_version="native",
                resolved_candidate_version="native",
                binary_path=None,
                expected_sha256=None,
                is_engram=False,
            ),
            "baseline_arm1": ArmCandidateIdentity(
                arm="baseline_arm1",
                requested_candidate_version="native",
                resolved_candidate_version="native",
                binary_path=None,
                expected_sha256=None,
                is_engram=False,
            ),
            "arm_a_native": ArmCandidateIdentity(
                arm="arm_a_native",
                requested_candidate_version="native",
                resolved_candidate_version="native",
                binary_path=None,
                expected_sha256=None,
                is_engram=False,
            ),
            "v151_arm2": ArmCandidateIdentity(
                arm="v151_arm2",
                requested_candidate_version="cand3",
                resolved_candidate_version="cand3",
                binary_path=self.engram_v151_bin or cand3_bin,
                expected_sha256=self.expected_v151_sha256 or cand3_sha,
                is_engram=True,
            ),
            "arm_b_cand3_always_on": ArmCandidateIdentity(
                arm="arm_b_cand3_always_on",
                requested_candidate_version="cand3",
                resolved_candidate_version="cand3",
                binary_path=cand3_bin,
                expected_sha256=cand3_sha,
                is_engram=True,
            ),
            "treatment_B": ArmCandidateIdentity(
                arm="treatment_B",
                requested_candidate_version=cand4_req,
                resolved_candidate_version=resolved_cand,
                binary_path=self.engram_bin,
                expected_sha256=self.expected_treatment_sha256,
                is_engram=True,
            ),
            "v154_arm3": ArmCandidateIdentity(
                arm="v154_arm3",
                requested_candidate_version=cand4_req,
                resolved_candidate_version=resolved_cand,
                binary_path=self.engram_v154_bin,
                expected_sha256=self.expected_v154_sha256,
                is_engram=True,
            ),
            "arm_c_cand4_always_on": ArmCandidateIdentity(
                arm="arm_c_cand4_always_on",
                requested_candidate_version=cand4_req,
                resolved_candidate_version=resolved_cand,
                binary_path=active_cand4_bin,
                expected_sha256=active_cand4_sha,
                is_engram=(resolved_cand != "native"),
            ),
            "arm_d_cand4_adaptive": ArmCandidateIdentity(
                arm="arm_d_cand4_adaptive",
                requested_candidate_version=cand4_req,
                resolved_candidate_version=resolved_cand,
                binary_path=active_cand4_bin,
                expected_sha256=active_cand4_sha,
                is_engram=(resolved_cand != "native"),
            ),
            "arm_c_cand4_1_always_on": ArmCandidateIdentity(
                arm="arm_c_cand4_1_always_on",
                requested_candidate_version="cand4_1",
                resolved_candidate_version="cand4_1",
                binary_path=cand4_1_bin,
                expected_sha256=cand4_1_sha,
                is_engram=True,
            ),
            "arm_d_cand4_1_adaptive": ArmCandidateIdentity(
                arm="arm_d_cand4_1_adaptive",
                requested_candidate_version="cand4_1",
                resolved_candidate_version="cand4_1",
                binary_path=cand4_1_bin,
                expected_sha256=cand4_1_sha,
                is_engram=True,
            ),
            "arm_c_cand4_2_always_on": ArmCandidateIdentity(
                arm="arm_c_cand4_2_always_on",
                requested_candidate_version="cand4_2",
                resolved_candidate_version="cand4_2",
                binary_path=cand4_2_bin,
                expected_sha256=cand4_2_sha,
                is_engram=True,
            ),
            "arm_d_cand4_2_adaptive": ArmCandidateIdentity(
                arm="arm_d_cand4_2_adaptive",
                requested_candidate_version="cand4_2",
                resolved_candidate_version="cand4_2",
                binary_path=cand4_2_bin,
                expected_sha256=cand4_2_sha,
                is_engram=True,
            ),
            "arm_b_cand3": ArmCandidateIdentity(
                arm="arm_b_cand3",
                requested_candidate_version="cand3",
                resolved_candidate_version="cand3",
                binary_path=cand3_bin,
                expected_sha256=cand3_sha,
                is_engram=True,
            ),
            "arm_c_cand42_always_on": ArmCandidateIdentity(
                arm="arm_c_cand42_always_on",
                requested_candidate_version="cand4_2",
                resolved_candidate_version="cand4_2",
                binary_path=cand4_2_bin,
                expected_sha256=cand4_2_sha,
                is_engram=True,
            ),
            "arm_d_cand42_adaptive": ArmCandidateIdentity(
                arm="arm_d_cand42_adaptive",
                requested_candidate_version="cand4_2",
                resolved_candidate_version="cand4_2",
                binary_path=cand4_2_bin,
                expected_sha256=cand4_2_sha,
                is_engram=True,
            ),
            "arm_e_cand5_optimized": ArmCandidateIdentity(
                arm="arm_e_cand5_optimized",
                requested_candidate_version="cand5",
                resolved_candidate_version="cand5",
                binary_path=cand5_bin,
                expected_sha256=cand5_sha,
                is_engram=True,
            ),
        }

        # Validate all configured arm identities against the candidate ledger
        for identity in self.arm_identities.values():
            identity.validate_against_ledger(CANDIDATE_LEDGER)

        self.arm_binaries: Dict[str, Optional[Path]] = {k: v.binary_path for k, v in self.arm_identities.items()}
        self.arm_expected_hashes: Dict[str, Optional[str]] = {k: v.expected_sha256 for k, v in self.arm_identities.items()}

        self.model = model
        self.reasoning_effort = reasoning_effort

        # Safe Output Directory Handling (Remediation D)
        evidence_file_names = {
            "run_telemetry.json", "triad_summary.json", "single_summary.json",
            "matched_pair_summary.json", "events.jsonl", "triad_report.md",
            "tetrad_summary.json", "tetrad_report.md",
            "pentad_summary.json", "pentad_report.md"
        }
        if output_dir is not None:
            out_p = Path(output_dir).resolve()
            # Path Traversal & Protected Directory Safety: Cannot resolve into source or golden repo
            protected_roots = [
                HARNESS_ROOT.parent.parent.resolve(),  # Engram repository root
                HARNESS_ROOT.resolve() / "src",
                self.golden_repo.resolve()
            ]
            for prot in protected_roots:
                if out_p == prot:
                    raise SafetyViolationError(f"Output directory cannot resolve to protected directory: {out_p}")

            # Evidence Preservation: If directory already has run evidence, create unique timestamped subdir
            has_existing_evidence = False
            if out_p.exists():
                for child in out_p.iterdir():
                    if child.name in evidence_file_names or (child.is_dir() and any(grand.name in evidence_file_names for grand in child.iterdir())):
                        has_existing_evidence = True
                        break
            if has_existing_evidence:
                timestamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
                self.output_dir = out_p / f"run_{timestamp}"
            else:
                self.output_dir = out_p
        else:
            default_dir = HARNESS_ROOT / "reports" / "runs" / f"{self.task_id}_{self.pair_id}"
            if default_dir.exists():
                has_existing_evidence = False
                for child in default_dir.iterdir():
                    if child.name in evidence_file_names or (child.is_dir() and any(grand.name in evidence_file_names for grand in child.iterdir())):
                        has_existing_evidence = True
                        break
                if has_existing_evidence:
                    timestamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
                    self.output_dir = default_dir / f"run_{timestamp}"
                else:
                    self.output_dir = default_dir
            else:
                self.output_dir = default_dir

        self.output_dir.mkdir(parents=True, exist_ok=True)
        self.is_dry_run = is_dry_run
        self.dependency_strategy = dependency_strategy
        self.preserve_sandboxes = preserve_sandboxes
        self.seed = seed if seed is not None else random.randint(1000, 9999)

        # Enforce Read-Only Agent Permissions (Remediation E)
        is_read_only = bool(self.task_config.get("read_only", False))
        if sandbox_policy:
            self.sandbox_policy = sandbox_policy
        else:
            self.sandbox_policy = "read-only" if is_read_only else "workspace-write"

        # Load schema for validation
        self.telemetry_schema_path = HARNESS_ROOT / "schemas" / "run_telemetry.schema.json"
        with open(self.telemetry_schema_path, "r", encoding="utf-8") as f:
            self.telemetry_schema = json.load(f)

        self.summary_schema_path = HARNESS_ROOT / "schemas" / "matched_pair_summary.schema.json"
        with open(self.summary_schema_path, "r", encoding="utf-8") as f:
            self.summary_schema = json.load(f)

        self.triad_schema_path = HARNESS_ROOT / "schemas" / "triad_summary.schema.json"
        if self.triad_schema_path.exists():
            with open(self.triad_schema_path, "r", encoding="utf-8") as f:
                self.triad_schema = json.load(f)
        else:
            self.triad_schema = None

        self.evaluator = Evaluator(self.task_config)
        if runner is not None:
            self.runner = runner
        elif self.provider in ("controlled", "openai", "mock"):
            try:
                from agent_runtime import ControlledRunner
            except (ImportError, ValueError):
                from .agent_runtime import ControlledRunner
            allow_live = not self.is_dry_run and self.provider in ("controlled", "openai")
            self.runner = ControlledRunner(
                model=self.model,
                reasoning_effort=self.reasoning_effort,
                max_tokens_ceiling=self.max_tokens_ceiling,
                headroom_reserve=self.headroom_reserve,
                sandbox_policy=self.sandbox_policy,
                provider=self.provider,
                allow_live_network=allow_live
            )
        else:
            self.runner = CodexRunner(
                codex_executable=self.codex_bin,
                engram_executable=self.engram_bin,
                model=self.model,
                reasoning_effort=self.reasoning_effort,
                max_tokens_ceiling=self.max_tokens_ceiling,
                headroom_reserve=self.headroom_reserve,
                sandbox_policy=self.sandbox_policy,
                provider=self.provider
            )
        self.process_tracker = ProcessTracker()

        # Capture baseline golden dependency fingerprint based on repository type
        if self.repo_type in ("cargo", "rust"):
            self.golden_baseline_fingerprint = compute_cargo_manifest(self.golden_repo)
        else:
            self.golden_baseline_fingerprint = compute_comprehensive_golden_manifest(self.golden_repo)

        # Validate task fixture preflight against baseline golden repository
        self.evaluator.validate_preflight(self.golden_repo)

    def run_pair(self, arm_order: Optional[List[str]] = None) -> Dict[str, Any]:
        """Executes a matched pair with deterministic arm order under Amendment 04 two-arm stop policy."""
        if arm_order:
            order = [(arm, idx + 1) for idx, arm in enumerate(arm_order)]
        else:
            # Deterministic default: Treatment Candidate Phase 1 runs first, baseline runs second.
            order = [("treatment_B", 1), ("control_A", 2)]

        pair_summary = {
            "schema_version": "2.0.0",
            "pair_id": self.pair_id,
            "task_id": self.task_id,
            "seed": self.seed,
            "provider": self.provider,
            "order": [arm for arm, _ in order],
            "model": self.model,
            "reasoning_effort": self.reasoning_effort if self.provider == "cloud" else "none",
            "golden_dependency_fingerprint_verified": True,
            "acceptance": {
                "control_A_accepted": False,
                "treatment_B_accepted": False,
                "pair_acceptance_parity": False
            },
            "comparative_analysis": {},
            "runs": {}
        }

        for arm, exec_order in order:
            run_data = self._execute_single_arm(arm=arm, exec_order=exec_order)
            pair_summary["runs"][arm] = run_data

            # Stop-on-failure check
            arm_status = run_data.get("execution_status", "SUCCESS")
            arm_telem = run_data.get("telemetry", {})
            telem_valid = arm_telem.get("telemetry_valid", True)
            is_failed = (arm_status != "SUCCESS") or (not telem_valid)

            if self.stop_on_failure and is_failed and exec_order == 1:
                other_arm = order[1][0]
                skip_reason = f"Execution halted: stop-on-failure triggered by {arm} ({arm_status})"
                print(f"\n[ORCHESTRATOR STOP-ON-FAILURE] Arm {arm} failed ({arm_status}). Halting pair execution immediately.")
                pair_summary["runs"][other_arm] = self._create_skipped_arm_record(other_arm, 2, skip_reason)
                break

        # Update acceptance summary
        ctrl = pair_summary["runs"].get("control_A", {})
        treat = pair_summary["runs"].get("treatment_B", {})

        ctrl_corr = ctrl.get("correctness", {})
        treat_corr = treat.get("correctness", {})

        ctrl_status = str(ctrl.get("execution_status", ""))
        treat_status = str(treat.get("execution_status", ""))
        ctrl_skipped = "SKIPPED" in ctrl_status
        treat_skipped = "SKIPPED" in treat_status

        if ctrl_skipped:
            pair_summary["acceptance"]["control_A_accepted"] = None
        else:
            pair_summary["acceptance"]["control_A_accepted"] = Orchestrator.is_run_accepted(ctrl)

        if treat_skipped:
            pair_summary["acceptance"]["treatment_B_accepted"] = None
        else:
            pair_summary["acceptance"]["treatment_B_accepted"] = Orchestrator.is_run_accepted(treat)

        if ctrl_skipped or treat_skipped:
            pair_summary["acceptance"]["pair_acceptance_parity"] = None
        else:
            pair_summary["acceptance"]["pair_acceptance_parity"] = (
                pair_summary["acceptance"]["control_A_accepted"] == pair_summary["acceptance"]["treatment_B_accepted"]
            )

        # Compute comparative analysis with strict invalidation guards
        comp_analysis = self.compute_comparative_analysis(ctrl, treat)
        pair_summary["comparative_analysis"] = comp_analysis

        # Validate against matched_pair_summary schema
        validate_instance(pair_summary, self.summary_schema)

        # Write comparative summary JSON
        summary_path = self.output_dir / "matched_pair_summary.json"
        with open(summary_path, "w", encoding="utf-8") as f:
            json.dump(pair_summary, f, indent=2)

        # Write comparative markdown report
        md_content = self.generate_markdown_report(pair_summary)
        report_path = self.output_dir / "matched_pair_report.md"
        with open(report_path, "w", encoding="utf-8") as f:
            f.write(md_content)

        # Print human-readable verdict banner
        self._print_pair_verdict(pair_summary)

        return pair_summary

    @staticmethod
    def validate_telemetry_integrity(tel: Any) -> Tuple[bool, str, Optional[Dict[str, Any]]]:
        """
        Validates telemetry completeness and mathematical integrity.
        Distinguishes missing telemetry from zero-token anomalies.
        Returns (is_valid, reason, parsed_metrics_dict).
        """
        if not isinstance(tel, dict) or not tel:
            return False, "TELEMETRY_MISSING: Telemetry dictionary is missing or empty.", None

        if tel.get("telemetry_valid") is False:
            err = tel.get("error_message") or "telemetry_valid is False"
            return False, f"TELEMETRY_INVALID: {err}.", None

        # Check for presence of required token fields (must not be None or absent)
        required_keys = ["total_model_tokens", "prompt_tokens_total", "completion_tokens"]
        missing = [k for k in required_keys if k not in tel or tel[k] is None]
        if missing:
            return False, f"TELEMETRY_MISSING: Required token fields missing or null: {missing}.", None

        for k in required_keys:
            if not isinstance(tel[k], int) or isinstance(tel[k], bool):
                return False, f"TELEMETRY_MALFORMED: Field '{k}' is not an integer (got {type(tel[k]).__name__}).", None

        total_tokens = tel["total_model_tokens"]
        prompt_tokens = tel["prompt_tokens_total"]
        completion_tokens = tel["completion_tokens"]

        # Distinct check: zero-token run is not valid model execution
        if total_tokens == 0:
            return False, "ZERO_TOKEN_ANOMALY: total_model_tokens is 0. Coding agent execution requires model tokens; 0 tokens indicates failed extraction or skipped model calls.", None

        if total_tokens < 0 or prompt_tokens <= 0 or completion_tokens < 0:
            return False, f"TELEMETRY_INVALID: Token counts must be positive (total={total_tokens}, prompt={prompt_tokens}, completion={completion_tokens}).", None

        # Mathematical invariant checks
        if prompt_tokens + completion_tokens != total_tokens:
            return False, f"TELEMETRY_INVARIANT_VIOLATION: prompt_tokens_total ({prompt_tokens}) + completion_tokens ({completion_tokens}) != total_model_tokens ({total_tokens}).", None

        cached_inferred_in = tel.get("prompt_tokens_cached_inferred")
        if cached_inferred_in is not None:
            if not isinstance(cached_inferred_in, bool):
                return False, f"TELEMETRY_MALFORMED: 'prompt_tokens_cached_inferred' must be a boolean (got {type(cached_inferred_in).__name__}: {cached_inferred_in!r}).", None

        uncached_inferred_in = tel.get("prompt_tokens_uncached_inferred")
        if uncached_inferred_in is not None:
            if not isinstance(uncached_inferred_in, bool):
                return False, f"TELEMETRY_MALFORMED: 'prompt_tokens_uncached_inferred' must be a boolean (got {type(uncached_inferred_in).__name__}: {uncached_inferred_in!r}).", None

        cached = tel.get("prompt_tokens_cached")
        if cached is not None:
            if isinstance(cached, bool) or not isinstance(cached, int) or cached < 0:
                return False, f"TELEMETRY_MALFORMED: 'prompt_tokens_cached' must be a non-negative integer (got {cached!r}).", None
            if cached > prompt_tokens:
                return False, f"TELEMETRY_INVARIANT_VIOLATION: cached tokens ({cached}) exceeds prompt_tokens_total ({prompt_tokens}).", None

        uncached = tel.get("prompt_tokens_uncached")
        if uncached is not None:
            if isinstance(uncached, bool) or not isinstance(uncached, int) or uncached < 0:
                return False, f"TELEMETRY_MALFORMED: 'prompt_tokens_uncached' must be a non-negative integer (got {uncached!r}).", None
            if uncached > prompt_tokens:
                return False, f"TELEMETRY_INVARIANT_VIOLATION: uncached tokens ({uncached}) exceeds prompt_tokens_total ({prompt_tokens}).", None

        if cached is not None and uncached is not None:
            if cached + uncached != prompt_tokens:
                return False, f"TELEMETRY_INVARIANT_VIOLATION: prompt cached ({cached}) + uncached ({uncached}) != prompt total ({prompt_tokens}).", None

        # Provenance and Normalization:
        # Rule 1: A count may be marked False only when the source establishes it was directly measured.
        # Rule 2: Whenever a count is inferred or missing during normalization, its flag MUST be True.
        # Rule 3: An existing True inference flag is preserved; never overwritten with False.
        if cached is None and uncached is None:
            # Both missing: apply fallback (0 cached, all uncached) but mark BOTH as inferred.
            # Never present cached = 0 as a measured fact.
            final_cached = 0
            final_cached_inferred = True
            final_uncached = prompt_tokens
            final_uncached_inferred = True
        elif cached is None:
            # Cached missing, inferred from prompt_tokens - uncached
            final_cached = prompt_tokens - uncached
            final_cached_inferred = True
            final_uncached = uncached
            final_uncached_inferred = uncached_inferred_in if uncached_inferred_in is not None else True
        elif uncached is None:
            # Uncached missing, inferred from prompt_tokens - cached
            final_cached = cached
            final_cached_inferred = cached_inferred_in if cached_inferred_in is not None else True
            final_uncached = prompt_tokens - cached
            final_uncached_inferred = True
        else:
            # Both present
            final_cached = cached
            final_cached_inferred = cached_inferred_in if cached_inferred_in is not None else True
            final_uncached = uncached
            final_uncached_inferred = uncached_inferred_in if uncached_inferred_in is not None else True

        peak_single = tel.get("peak_single_request_context")
        if peak_single is not None:
            if isinstance(peak_single, bool) or not isinstance(peak_single, int) or peak_single <= 0:
                return False, f"TELEMETRY_MALFORMED: 'peak_single_request_context' must be a positive integer or None (got {peak_single!r}).", None
            if peak_single > total_tokens:
                return False, f"TELEMETRY_INVARIANT_VIOLATION: peak_single_request_context ({peak_single}) cannot exceed total_model_tokens ({total_tokens}).", None

        parsed = {
            "total_model_tokens": total_tokens,
            "prompt_tokens_total": prompt_tokens,
            "completion_tokens": completion_tokens,
            "prompt_tokens_cached": final_cached,
            "prompt_tokens_cached_inferred": final_cached_inferred,
            "prompt_tokens_uncached": final_uncached,
            "prompt_tokens_uncached_inferred": final_uncached_inferred,
            "peak_single_request_context": peak_single
        }
        return True, "VALID", parsed

    ALL_TETRAD_ARMS: List[str] = [
        "arm_a_native",
        "arm_b_cand3_always_on",
        "arm_c_cand4_always_on",
        "arm_d_cand4_adaptive"
    ]

    @staticmethod
    def validate_tetrad_arm_order(arm_order: Any, allow_none: bool = False) -> Optional[List[str]]:
        """
        Authoritative validator for tetrad arm execution order.
        Invariants:
        1. If arm_order is None:
           - If allow_none=True: returns None (caller applies deterministic seeded default).
           - If allow_none=False: raises SafetyViolationError.
        2. Explicit order must be a non-empty string or list.
        3. Normalizes whitespace around comma-separated entries.
        4. Rejects empty entries, whitespace-only entries, or unsupported separators (; | etc).
        5. Rejects whitespace within individual arm identifiers.
        6. Enforces exact cardinality: len(order) == 4.
        7. Enforces strict uniqueness: len(set(order)) == 4 (no duplicates).
        8. Enforces exact set equivalence: set(order) == set(ALL_TETRAD_ARMS) (no unknown/missing arms).
        9. Enforces exact casing (case-sensitive match).
        """
        if arm_order is None:
            if allow_none:
                return None
            raise SafetyViolationError(
                "Invalid tetrad arm order: explicit order required but received None."
            )

        if isinstance(arm_order, str):
            raw_str = arm_order.strip()
            if not raw_str:
                raise SafetyViolationError(
                    "Invalid tetrad arm order: explicit order string cannot be empty or whitespace."
                )
            if any(sep in raw_str for sep in [";", "|", "\n", "\r", "\t"]):
                raise SafetyViolationError(
                    f"Invalid tetrad arm order: unsupported separator in '{raw_str}'. Expected comma-separated list."
                )
            tokens = [a.strip() for a in raw_str.split(",")]
            if any(t == "" for t in tokens):
                raise SafetyViolationError(
                    "Invalid tetrad arm order: empty entry encountered in comma-separated list."
                )
            for t in tokens:
                if " " in t:
                    raise SafetyViolationError(
                        f"Invalid tetrad arm order: whitespace within arm identifier '{t}'. Use comma-separated list without spaces in identifiers."
                    )
        elif isinstance(arm_order, (list, tuple)):
            if len(arm_order) == 0:
                raise SafetyViolationError(
                    "Invalid tetrad arm order: explicit arm list cannot be empty."
                )
            tokens = []
            for a in arm_order:
                if not isinstance(a, str):
                    raise SafetyViolationError(
                        f"Invalid tetrad arm order: arm identifier must be a string, got {type(a).__name__} ({a})."
                    )
                t = a.strip()
                if not t:
                    raise SafetyViolationError(
                        "Invalid tetrad arm order: empty or whitespace arm identifier in list."
                    )
                if " " in t:
                    raise SafetyViolationError(
                        f"Invalid tetrad arm order: whitespace within arm identifier '{t}'."
                    )
                tokens.append(t)
        else:
            raise SafetyViolationError(
                f"Invalid tetrad arm order type: expected str or list, got {type(arm_order).__name__}."
            )

        if len(tokens) != 4:
            raise SafetyViolationError(
                f"Invalid tetrad arm order: expected exactly 4 arms, received {len(tokens)}: {tokens}. "
                f"Expected exact permutation of {sorted(Orchestrator.ALL_TETRAD_ARMS)}."
            )

        seen = set()
        duplicates = []
        for t in tokens:
            if t in seen and t not in duplicates:
                duplicates.append(t)
            seen.add(t)
        if duplicates:
            raise SafetyViolationError(
                f"Invalid tetrad arm order: duplicate arm(s) detected: {duplicates} in order {tokens}."
            )

        all_set = set(Orchestrator.ALL_TETRAD_ARMS)
        order_set = set(tokens)
        unknown = order_set - all_set
        missing = all_set - order_set
        if unknown or missing:
            parts = []
            if unknown:
                parts.append(f"unregistered arm(s): {sorted(unknown)}")
            if missing:
                parts.append(f"missing registered arm(s): {sorted(missing)}")
            raise SafetyViolationError(
                f"Invalid tetrad arm order: {'; '.join(parts)}. Expected exact permutation of {sorted(Orchestrator.ALL_TETRAD_ARMS)}."
            )

        return tokens

    ALL_PENTAD_ARMS: List[str] = [
        "arm_a_native",
        "arm_b_cand3",
        "arm_c_cand42_always_on",
        "arm_d_cand42_adaptive",
        "arm_e_cand5_optimized",
    ]

    @staticmethod
    def validate_pentad_arm_order(arm_order: Any, allow_none: bool = False) -> Optional[List[str]]:
        """
        Authoritative validator for pentad arm execution order.
        Invariants:
        1. If arm_order is None:
           - If allow_none=True: returns None (caller applies deterministic seeded default).
           - If allow_none=False: raises SafetyViolationError.
        2. Explicit order must be a non-empty string or list.
        3. Normalizes whitespace around comma-separated entries.
        4. Rejects empty entries, whitespace-only entries, or unsupported separators (; | etc).
        5. Rejects whitespace within individual arm identifiers.
        6. Enforces exact cardinality: len(order) == 5.
        7. Enforces strict uniqueness: len(set(order)) == 5 (no duplicates).
        8. Enforces exact set equivalence: set(order) == set(ALL_PENTAD_ARMS) (no unknown/missing arms).
        9. Enforces exact casing (case-sensitive match).
        """
        if arm_order is None:
            if allow_none:
                return None
            raise SafetyViolationError(
                "Invalid pentad arm order: explicit order required but received None."
            )

        if isinstance(arm_order, str):
            if not arm_order.strip():
                raise SafetyViolationError("Invalid pentad arm order: empty or whitespace-only string.")
            if ";" in arm_order or "|" in arm_order:
                raise SafetyViolationError(
                    f"Invalid pentad arm order separator in '{arm_order}'. Use comma-separated list."
                )
            tokens = [token.strip() for token in arm_order.split(",")]
        elif isinstance(arm_order, list):
            if len(arm_order) == 0:
                raise SafetyViolationError("Invalid pentad arm order: empty list.")
            tokens = []
            for item in arm_order:
                if not isinstance(item, str):
                    raise SafetyViolationError(
                        f"Invalid pentad arm order element: expected str, got {type(item).__name__} ({item})"
                    )
                tokens.append(item.strip())
        else:
            raise SafetyViolationError(
                f"Invalid pentad arm order type: expected str or list, got {type(arm_order).__name__}"
            )

        for token in tokens:
            if not token:
                raise SafetyViolationError(
                    "Invalid pentad arm order: contains empty or whitespace-only element."
                )
            if any(char.isspace() for char in token):
                raise SafetyViolationError(
                    f"Invalid pentad arm identifier '{token}': contains internal whitespace."
                )

        if len(tokens) != 5:
            raise SafetyViolationError(
                f"Invalid pentad arm order length: expected exactly 5 arms, received {len(tokens)}: {tokens}. "
                f"Expected exact permutation of {sorted(Orchestrator.ALL_PENTAD_ARMS)}."
            )

        seen = set()
        duplicates = []
        for t in tokens:
            if t in seen and t not in duplicates:
                duplicates.append(t)
            seen.add(t)
        if duplicates:
            raise SafetyViolationError(
                f"Invalid pentad arm order: duplicate arm(s) detected: {duplicates} in order {tokens}."
            )

        all_set = set(Orchestrator.ALL_PENTAD_ARMS)
        order_set = set(tokens)
        unknown = order_set - all_set
        missing = all_set - order_set
        if unknown or missing:
            parts = []
            if unknown:
                parts.append(f"unregistered arm(s): {sorted(unknown)}")
            if missing:
                parts.append(f"missing registered arm(s): {sorted(missing)}")
            raise SafetyViolationError(
                f"Invalid pentad arm order: {'; '.join(parts)}. Expected exact permutation of {sorted(Orchestrator.ALL_PENTAD_ARMS)}."
            )

        return tokens

    @staticmethod
    def is_run_accepted(run_dict: Dict[str, Any]) -> bool:
        """
        Evaluates full acceptance gate criteria for a single arm execution:
        1. Execution status must be SUCCESS.
        2. Functional correctness must be proven (score == 1.0, passed_all_requirements is True).
        3. Top-level telemetry_valid must be True.
        4. Embedded telemetry object must exist and indicate telemetry_valid is True.
        5. Telemetry must be mathematically sound, complete, and trustworthy via validate_telemetry_integrity.
        6. Total model tokens must be a positive integer.
        7. Total model tokens must not exceed configured ceiling if specified.
        """
        if not isinstance(run_dict, dict) or not run_dict:
            return False
        if run_dict.get("execution_status") != "SUCCESS":
            return False
        corr = run_dict.get("correctness", {})
        if not isinstance(corr, dict) or corr.get("score") != 1.0 or not corr.get("passed_all_requirements", False):
            return False
        if run_dict.get("telemetry_valid") is not True:
            return False
        telem = run_dict.get("telemetry", {})
        if not isinstance(telem, dict) or telem.get("telemetry_valid") is not True:
            return False
        tel_valid, _, tel_data = Orchestrator.validate_telemetry_integrity(telem)
        if not tel_valid or not tel_data:
            return False
        tot = tel_data.get("total_model_tokens")
        if not isinstance(tot, int) or tot <= 0:
            return False
        guard = run_dict.get("headroom_guard", {})
        configured_ceiling = guard.get("configured_ceiling")
        if isinstance(configured_ceiling, int) and configured_ceiling > 0:
            if tot > configured_ceiling:
                return False
        gate = run_dict.get("request_size_gate")
        if isinstance(gate, dict) and gate.get("gate_passed") is False:
            return False
        return True

    @staticmethod
    def compute_comparative_analysis(
        ctrl_run: Dict[str, Any],
        treat_run: Dict[str, Any],
        baseline_label: str = "control_A",
        treatment_label: str = "treatment_B",
        block_incomplete: bool = False
    ) -> Dict[str, Any]:
        """
        Computes rigorous matched-pair comparative metrics.
        CRITICAL SCIENTIFIC INVARIANTS:
        1. Telemetry Integrity Guard: If telemetry is missing, zero, or mathematically inconsistent
           on either arm, comparative savings are STRICTLY INELIGIBLE even if functional acceptance passed.
        2. Acceptance Guard: Token savings, context reductions, and efficiency percentages are
           STRICTLY INVALIDATED whenever either arm fails functional correctness (C < 1.0 or passed_all_requirements == False).
        3. Block Completeness Guard: When block_incomplete is True (e.g. diagnostic continuation or stop-on-failure),
           comparative savings claims are STRICTLY INVALIDATED regardless of individual pairwise arm status.
        """
        def _build_metric_entry(c_val, t_val, delta, pct, status):
            entry = {
                baseline_label: c_val,
                treatment_label: t_val,
                "delta": delta,
                "savings_pct": pct,
                "status": status
            }
            if baseline_label != "control_A" or treatment_label != "treatment_B":
                entry["baseline"] = c_val
                entry["treatment"] = t_val
            return entry

        # 0. Validate block-level completeness guard
        if block_incomplete:
            ctrl_tel_valid, ctrl_tel_reason, ctrl_tel_data = Orchestrator.validate_telemetry_integrity(ctrl_run.get("telemetry"))
            treat_tel_valid, treat_tel_reason, treat_tel_data = Orchestrator.validate_telemetry_integrity(treat_run.get("telemetry"))
            ctrl_corr = ctrl_run.get("correctness", {})
            treat_corr = treat_run.get("correctness", {})
            return {
                "savings_claim_eligible": False,
                "savings_claim_status": "INVALIDATED_BY_ACCEPTANCE_FAILURE",
                "verdict": "MUTUAL_ACCEPTANCE_FAILURE",
                "explanation": (
                    f"Block execution incomplete, halted, or failed mutual acceptance across all arms; "
                    f"comparative token savings and efficiency claims are STRICTLY INVALIDATED."
                ),
                "metrics": {
                    "total_model_tokens": _build_metric_entry(
                        ctrl_tel_data["total_model_tokens"] if ctrl_tel_data else None,
                        treat_tel_data["total_model_tokens"] if treat_tel_data else None,
                        None, None, "INVALIDATED_UNACCEPTED"
                    ),
                    "prompt_tokens_total": _build_metric_entry(
                        ctrl_tel_data["prompt_tokens_total"] if ctrl_tel_data else None,
                        treat_tel_data["prompt_tokens_total"] if treat_tel_data else None,
                        None, None, "INVALIDATED_UNACCEPTED"
                    ),
                    "completion_tokens": _build_metric_entry(
                        ctrl_tel_data["completion_tokens"] if ctrl_tel_data else None,
                        treat_tel_data["completion_tokens"] if treat_tel_data else None,
                        None, None, "INVALIDATED_UNACCEPTED"
                    ),
                    "peak_single_request_context": _build_metric_entry(
                        ctrl_tel_data["peak_single_request_context"] if ctrl_tel_data else None,
                        treat_tel_data["peak_single_request_context"] if treat_tel_data else None,
                        None, None, "INVALIDATED_UNACCEPTED"
                    ),
                    "quality_adjusted_cost": _build_metric_entry(
                        float(ctrl_corr.get("quality_adjusted_cost")) if (ctrl_tel_data and "quality_adjusted_cost" in ctrl_corr) else None,
                        float(treat_corr.get("quality_adjusted_cost")) if (treat_tel_data and "quality_adjusted_cost" in treat_corr) else None,
                        None, None, "INVALIDATED_UNACCEPTED"
                    )
                }
            }

        # 1. Validate Telemetry Integrity on both arms
        ctrl_tel_valid, ctrl_tel_reason, ctrl_tel_data = Orchestrator.validate_telemetry_integrity(ctrl_run.get("telemetry"))
        treat_tel_valid, treat_tel_reason, treat_tel_data = Orchestrator.validate_telemetry_integrity(treat_run.get("telemetry"))

        # 2. Extract and validate Correctness Acceptance
        ctrl_corr = ctrl_run.get("correctness", {})
        treat_corr = treat_run.get("correctness", {})

        ctrl_status = str(ctrl_run.get("execution_status", ""))
        treat_status = str(treat_run.get("execution_status", ""))
        ctrl_skipped = "SKIPPED" in ctrl_status
        treat_skipped = "SKIPPED" in treat_status

        ctrl_score = ctrl_corr.get("score")
        treat_score = treat_corr.get("score")

        ctrl_passed = bool(ctrl_corr.get("passed_all_requirements", False) and (ctrl_score == 1.0)) if not ctrl_skipped else False
        treat_passed = bool(treat_corr.get("passed_all_requirements", False) and (treat_score == 1.0)) if not treat_skipped else False

        both_accepted = ctrl_passed and treat_passed

        # 3. Guard against functional acceptance failure (Primary Gate)
        if not both_accepted:
            if ctrl_skipped or treat_skipped:
                if baseline_label == "control_A":
                    if ctrl_skipped and treat_skipped:
                        verdict = "BOTH_ARMS_SKIPPED"
                        explanation = "Both benchmark arms were skipped. No comparative execution data exists."
                    elif ctrl_skipped:
                        verdict = "TREATMENT_FAILED_CONTROL_SKIPPED"
                        explanation = (
                            f"Treatment arm ({treatment_label}) halted early or failed ({treat_status}); "
                            f"baseline arm ({baseline_label}) was skipped via stop-on-failure. "
                            f"Comparative token savings and efficiency claims are STRICTLY INVALIDATED."
                        )
                    else:
                        verdict = "CONTROL_FAILED_TREATMENT_SKIPPED"
                        explanation = (
                            f"Baseline arm ({baseline_label}) halted early or failed ({ctrl_status}); "
                            f"treatment arm ({treatment_label}) was skipped via stop-on-failure. "
                            f"Comparative token savings and efficiency claims are STRICTLY INVALIDATED."
                        )
                else:
                    verdict = "MUTUAL_ACCEPTANCE_FAILURE"
                    explanation = (
                        f"One or more triad arms were skipped or halted early "
                        f"({baseline_label}: {ctrl_status}, {treatment_label}: {treat_status}). "
                        f"Comparative token savings and efficiency claims are STRICTLY INVALIDATED."
                    )
            elif not ctrl_passed and not treat_passed:
                ctrl_s_str = f"{ctrl_score:.2f}" if isinstance(ctrl_score, (int, float)) else "null"
                treat_s_str = f"{treat_score:.2f}" if isinstance(treat_score, (int, float)) else "null"
                verdict = "MUTUAL_ACCEPTANCE_FAILURE"
                explanation = (
                    f"Both arms failed functional correctness requirements "
                    f"({baseline_label} score: {ctrl_s_str}, {treatment_label} score: {treat_s_str}). "
                    f"Comparative token savings and efficiency claims are STRICTLY INVALIDATED."
                )
            elif not treat_passed:
                ctrl_s_str = f"{ctrl_score:.2f}" if isinstance(ctrl_score, (int, float)) else "null"
                treat_s_str = f"{treat_score:.2f}" if isinstance(treat_score, (int, float)) else "null"
                verdict = "TREATMENT_FAILED_ACCEPTANCE"
                explanation = (
                    f"Treatment arm ({treatment_label}) failed functional correctness requirements "
                    f"({treatment_label} score: {treat_s_str}, {baseline_label} score: {ctrl_s_str}). "
                    f"A defective or incomplete patch cannot claim token efficiency savings."
                )
            else:
                ctrl_s_str = f"{ctrl_score:.2f}" if isinstance(ctrl_score, (int, float)) else "null"
                treat_s_str = f"{treat_score:.2f}" if isinstance(treat_score, (int, float)) else "null"
                verdict = "CONTROL_FAILED_ACCEPTANCE"
                explanation = (
                    f"Baseline arm ({baseline_label}) failed functional correctness requirements "
                    f"({baseline_label} score: {ctrl_s_str}, {treatment_label} score: {treat_s_str}). "
                    f"Baseline implementation was not established; comparative savings cannot be computed."
                )

            return {
                "savings_claim_eligible": False,
                "savings_claim_status": "INVALIDATED_BY_ACCEPTANCE_FAILURE",
                "verdict": verdict,
                "explanation": explanation,
                "metrics": {
                    "total_model_tokens": _build_metric_entry(
                        ctrl_tel_data["total_model_tokens"] if ctrl_tel_data else None,
                        treat_tel_data["total_model_tokens"] if treat_tel_data else None,
                        None, None, "INVALIDATED_UNACCEPTED"
                    ),
                    "prompt_tokens_total": _build_metric_entry(
                        ctrl_tel_data["prompt_tokens_total"] if ctrl_tel_data else None,
                        treat_tel_data["prompt_tokens_total"] if treat_tel_data else None,
                        None, None, "INVALIDATED_UNACCEPTED"
                    ),
                    "completion_tokens": _build_metric_entry(
                        ctrl_tel_data["completion_tokens"] if ctrl_tel_data else None,
                        treat_tel_data["completion_tokens"] if treat_tel_data else None,
                        None, None, "INVALIDATED_UNACCEPTED"
                    ),
                    "peak_single_request_context": _build_metric_entry(
                        ctrl_tel_data["peak_single_request_context"] if ctrl_tel_data else None,
                        treat_tel_data["peak_single_request_context"] if treat_tel_data else None,
                        None, None, "INVALIDATED_UNACCEPTED"
                    ),
                    "quality_adjusted_cost": _build_metric_entry(
                        float(ctrl_corr.get("quality_adjusted_cost")) if (ctrl_tel_data and "quality_adjusted_cost" in ctrl_corr) else None,
                        float(treat_corr.get("quality_adjusted_cost")) if (treat_tel_data and "quality_adjusted_cost" in treat_corr) else None,
                        None, None, "INVALIDATED_UNACCEPTED"
                    )
                }
            }

        # 4. Guard against invalid, missing, or zero-token telemetry EVEN IF BOTH PASSED CORRECTNESS
        if not (ctrl_tel_valid and treat_tel_valid):
            if not ctrl_tel_valid and not treat_tel_valid:
                verdict = "MUTUAL_TELEMETRY_INVALID"
                explanation = f"Both arms passed functional correctness (C = 1.0), BUT token telemetry is invalid or missing. {baseline_label}: {ctrl_tel_reason}; {treatment_label}: {treat_tel_reason}. Comparative savings claim is strictly ineligible."
            elif not treat_tel_valid:
                verdict = "TREATMENT_TELEMETRY_INVALID"
                explanation = f"Both arms passed functional correctness (C = 1.0), BUT Treatment arm ({treatment_label}) telemetry is invalid or missing: {treat_tel_reason}. Savings claims cannot be made without verified treatment telemetry."
            else:
                verdict = "CONTROL_TELEMETRY_INVALID"
                explanation = f"Both arms passed functional correctness (C = 1.0), BUT Baseline arm ({baseline_label}) telemetry is invalid or missing: {ctrl_tel_reason}. Baseline token count cannot be established; savings claim is ineligible."

            return {
                "savings_claim_eligible": False,
                "savings_claim_status": "INVALIDATED_BY_TELEMETRY_INVALID",
                "verdict": verdict,
                "explanation": explanation,
                "metrics": {
                    "total_model_tokens": _build_metric_entry(
                        ctrl_tel_data["total_model_tokens"] if ctrl_tel_data else None,
                        treat_tel_data["total_model_tokens"] if treat_tel_data else None,
                        None, None, "INVALIDATED_TELEMETRY"
                    ),
                    "prompt_tokens_total": _build_metric_entry(
                        ctrl_tel_data["prompt_tokens_total"] if ctrl_tel_data else None,
                        treat_tel_data["prompt_tokens_total"] if treat_tel_data else None,
                        None, None, "INVALIDATED_TELEMETRY"
                    ),
                    "completion_tokens": _build_metric_entry(
                        ctrl_tel_data["completion_tokens"] if ctrl_tel_data else None,
                        treat_tel_data["completion_tokens"] if treat_tel_data else None,
                        None, None, "INVALIDATED_TELEMETRY"
                    ),
                    "peak_single_request_context": _build_metric_entry(
                        ctrl_tel_data["peak_single_request_context"] if ctrl_tel_data else None,
                        treat_tel_data["peak_single_request_context"] if treat_tel_data else None,
                        None, None, "INVALIDATED_TELEMETRY"
                    ),
                    "quality_adjusted_cost": _build_metric_entry(
                        float(ctrl_corr.get("quality_adjusted_cost")) if (ctrl_tel_data and "quality_adjusted_cost" in ctrl_corr) else None,
                        float(treat_corr.get("quality_adjusted_cost")) if (treat_tel_data and "quality_adjusted_cost" in treat_corr) else None,
                        None, None, "INVALIDATED_TELEMETRY"
                    )
                }
            }

        # 5. BOTH ACCEPTED AND BOTH TELEMETRY VALID: Compute genuine savings
        ctrl_total = ctrl_tel_data["total_model_tokens"]
        treat_total = treat_tel_data["total_model_tokens"]
        ctrl_prompt = ctrl_tel_data["prompt_tokens_total"]
        treat_prompt = treat_tel_data["prompt_tokens_total"]
        ctrl_completion = ctrl_tel_data["completion_tokens"]
        treat_completion = treat_tel_data["completion_tokens"]
        ctrl_peak = ctrl_tel_data["peak_single_request_context"]
        treat_peak = treat_tel_data["peak_single_request_context"]
        ctrl_qac = float(ctrl_corr.get("quality_adjusted_cost", ctrl_total))
        treat_qac = float(treat_corr.get("quality_adjusted_cost", treat_total))

        total_delta = ctrl_total - treat_total
        total_savings_pct = round((total_delta / ctrl_total) * 100.0, 2)

        prompt_delta = ctrl_prompt - treat_prompt
        prompt_savings_pct = round((prompt_delta / ctrl_prompt) * 100.0, 2)

        completion_delta = ctrl_completion - treat_completion
        completion_savings_pct = round((completion_delta / ctrl_completion) * 100.0, 2) if ctrl_completion > 0 else 0.0

        if ctrl_peak is not None and treat_peak is not None and ctrl_peak > 0:
            peak_delta = ctrl_peak - treat_peak
            peak_savings_pct = round((peak_delta / ctrl_peak) * 100.0, 2)
            peak_status = "VALID"
        else:
            peak_delta = None
            peak_savings_pct = None
            peak_status = "UNAVAILABLE"

        qac_delta = round(ctrl_qac - treat_qac, 2)
        qac_savings_pct = round((qac_delta / ctrl_qac) * 100.0, 2)

        if total_savings_pct > 0:
            verdict = "EFFICIENCY_GAIN"
            explanation = f"{treatment_label} achieved full acceptance with {total_savings_pct}% total token savings ({total_delta:,} fewer tokens) relative to {baseline_label}."
        elif total_savings_pct < 0:
            verdict = "EFFICIENCY_REGRESSION"
            explanation = f"{treatment_label} achieved full acceptance but incurred a {abs(total_savings_pct)}% token regression ({abs(total_delta):,} more tokens) relative to {baseline_label}."
        else:
            verdict = "EFFICIENCY_NEUTRAL"
            explanation = f"{treatment_label} and {baseline_label} used identical total token counts."

        # Round trip comparison if present on both arms
        ctrl_rt = (ctrl_run.get("telemetry") or {}).get("round_trip_summary")
        treat_rt = (treat_run.get("telemetry") or {}).get("round_trip_summary")
        rt_comp = None
        if ctrl_rt and treat_rt:
            c_rts = ctrl_rt.get("total_round_trips", 0)
            t_rts = treat_rt.get("total_round_trips", 0)
            rt_delta = c_rts - t_rts
            rt_pct = round((rt_delta / c_rts) * 100.0, 2) if c_rts > 0 else 0.0

            c_turns = ctrl_rt.get("total_agent_turns", 0)
            t_turns = treat_rt.get("total_agent_turns", 0)
            turn_delta = c_turns - t_turns
            turn_pct = round((turn_delta / c_turns) * 100.0, 2) if c_turns > 0 else 0.0

            rt_comp = {
                "baseline_round_trips": c_rts,
                "treatment_round_trips": t_rts,
                "round_trips_delta": rt_delta,
                "round_trips_reduction_pct": rt_pct,
                "baseline_agent_turns": c_turns,
                "treatment_agent_turns": t_turns,
                "agent_turns_reduction_pct": turn_pct,
                "treatment_engram_mcp_calls": treat_rt.get("engram_mcp_calls", 0),
                "baseline_p50_latency_ms": ctrl_rt.get("latency_median_ms", 0.0),
                "treatment_p50_latency_ms": treat_rt.get("latency_median_ms", 0.0),
                "verdict": "ROUND_TRIPS_REDUCED" if rt_pct > 0 else ("ROUND_TRIPS_EQUIVALENT" if rt_pct == 0 else "ROUND_TRIPS_INCREASED")
            }

        return {
            "savings_claim_eligible": True,
            "savings_claim_status": "VALID",
            "verdict": verdict,
            "explanation": explanation,
            "round_trips": rt_comp,
            "metrics": {
                "total_model_tokens": _build_metric_entry(ctrl_total, treat_total, total_delta, total_savings_pct, "VALID"),
                "prompt_tokens_total": _build_metric_entry(ctrl_prompt, treat_prompt, prompt_delta, prompt_savings_pct, "VALID"),
                "completion_tokens": _build_metric_entry(ctrl_completion, treat_completion, completion_delta, completion_savings_pct, "VALID"),
                "peak_single_request_context": _build_metric_entry(ctrl_peak, treat_peak, peak_delta, peak_savings_pct, peak_status),
                "quality_adjusted_cost": _build_metric_entry(ctrl_qac, treat_qac, qac_delta, qac_savings_pct, "VALID"),
            }
        }

    @staticmethod
    def generate_markdown_report(pair_summary: Dict[str, Any]) -> str:
        """Generates a human-readable matched-pair markdown report with explicit savings guards."""
        task_id = pair_summary.get("task_id", "")
        pair_id = pair_summary.get("pair_id", "")
        seed = pair_summary.get("seed", "")
        order = " -> ".join(pair_summary.get("order", []))
        model = pair_summary.get("model", "")
        reasoning = pair_summary.get("reasoning_effort", "")
        provider = pair_summary.get("provider", "cloud")
        provider_info = f" (Provider: `{provider}`)" if provider != "cloud" else ""

        comp = pair_summary.get("comparative_analysis", {})
        eligible = comp.get("savings_claim_eligible", False)
        verdict = comp.get("verdict", "UNKNOWN")
        explanation = comp.get("explanation", "")
        metrics = comp.get("metrics", {})

        ctrl = pair_summary.get("runs", {}).get("control_A", {})
        treat = pair_summary.get("runs", {}).get("treatment_B", {})

        ctrl_corr = ctrl.get("correctness", {})
        treat_corr = treat.get("correctness", {})

        ctrl_status = str(ctrl.get("execution_status", ""))
        treat_status = str(treat.get("execution_status", ""))

        ctrl_score = ctrl_corr.get("score")
        treat_score = treat_corr.get("score")

        ctrl_pass = ctrl_corr.get("passed_all_requirements")
        treat_pass = treat_corr.get("passed_all_requirements")

        if "SKIPPED" in ctrl_status:
            ctrl_score_str = "SKIPPED"
            ctrl_pass_str = "NOT_EVALUATED"
            ctrl_acc_str = "SKIPPED"
        else:
            ctrl_score_str = f"{ctrl_score:.2f}" if isinstance(ctrl_score, (int, float)) else "null"
            ctrl_pass_str = str(ctrl_pass)
            ctrl_acc_str = "PASS" if ctrl_pass and ctrl_score == 1.0 else "FAIL"

        if "SKIPPED" in treat_status:
            treat_score_str = "SKIPPED"
            treat_pass_str = "NOT_EVALUATED"
            treat_acc_str = "SKIPPED"
        else:
            treat_score_str = f"{treat_score:.2f}" if isinstance(treat_score, (int, float)) else "null"
            treat_pass_str = str(treat_pass)
            treat_acc_str = "PASS" if treat_pass and treat_score == 1.0 else "FAIL"

        lines = [
            f"# Matched-Pair Benchmark Run Report: `{task_id}` (`{pair_id}`)",
            "",
            f"- **Model:** `{model}`{provider_info} (Reasoning: `{reasoning}`)",
            f"- **Execution Order:** {order} (Seed: `{seed}`)",
            f"- **Golden Dependency Fingerprint:** Verified Intact",
            "",
            "---",
            "",
            "## 1. Acceptance & Correctness Gate",
            "",
            "| Arm | Engram Status | Correctness Score ($C$) | Passed All Requirements | Functional Acceptance |",
            "| :--- | :---: | :---: | :---: | :---: |",
            f"| **Control Arm A** | DISABLED | {ctrl_score_str} | {ctrl_pass_str} | {ctrl_acc_str} |",
            f"| **Treatment Arm B** | ACTIVE | {treat_score_str} | {treat_pass_str} | {treat_acc_str} |",
            "",
        ]

        if not eligible:
            lines.extend([
                "> [!WARNING]",
                "> **COMPARATIVE TOKEN SAVINGS CLAIM INVALIDATED**",
                f"> - **Verdict:** `{verdict}`",
                f"> - **Explanation:** {explanation}",
                "> - **Scientific Safeguard:** An agent run that fails functional requirements ($C < 1.0$) cannot claim token savings. All comparative percentage metrics are strictly suppressed (`null`).",
                "",
            ])
        else:
            lines.extend([
                "> [!NOTE]",
                "> **COMPARATIVE TOKEN SAVINGS CLAIM VALID**",
                f"> - **Verdict:** `{verdict}`",
                f"> - **Explanation:** {explanation}",
                "> Both arms successfully fulfilled all requirements ($C = 1.0$).",
                "",
            ])

        lines.extend([
            "## 2. Telemetry Comparison",
            "",
            "| Telemetry Metric | Control Arm A (Engram OFF) | Treatment Arm B (Engram ON) | Comparative Delta / Savings |",
            "| :--- | :---: | :---: | :---: |",
        ])

        for mkey, mlabel in [
            ("total_model_tokens", "Total Model Tokens"),
            ("prompt_tokens_total", "Prompt Tokens (Total)"),
            ("completion_tokens", "Completion Tokens"),
            ("peak_single_request_context", "Peak Single-Request Context"),
            ("quality_adjusted_cost", "Quality-Adjusted Effective Cost"),
        ]:
            m = metrics.get(mkey, {})
            c_val = m.get("control_A")
            t_val = m.get("treatment_B")
            c_str = f"{c_val:,}" if isinstance(c_val, (int, float)) else str(c_val)
            t_str = f"{t_val:,}" if isinstance(t_val, (int, float)) else str(t_val)

            if m.get("status") == "INVALIDATED_UNACCEPTED":
                delta_str = "**INVALIDATED ($C < 1.0$)**"
            elif m.get("status") == "INVALIDATED_TELEMETRY":
                delta_str = "**INVALIDATED (Telemetry Invalid/Missing)**"
            elif m.get("status") == "UNAVAILABLE":
                delta_str = "UNAVAILABLE"
            else:
                pct = m.get("savings_pct")
                delta = m.get("delta")
                delta_str = f"**{pct:+.1f}%** ({delta:+,})" if pct is not None else "N/A"

            lines.append(f"| **{mlabel}** | {c_str} | {t_str} | {delta_str} |")

        lines.append("")
        return "\n".join(lines)

    @staticmethod
    def _print_pair_verdict(pair_summary: Dict[str, Any]) -> None:
        comp = pair_summary.get("comparative_analysis", {})
        eligible = comp.get("savings_claim_eligible", False)
        verdict = comp.get("verdict", "")
        m = comp.get("metrics", {})
        tot = m.get("total_model_tokens", {})

        print("\n=======================================================")
        if eligible:
            pct = tot.get("savings_pct")
            delta = tot.get("delta")
            print(f"Matched Pair Result: [VALID EFFICIENCY CLAIM - {verdict}]")
            print(f"  - Control A Tokens:   {tot.get('control_A'):,}")
            print(f"  - Treatment B Tokens: {tot.get('treatment_B'):,}")
            print(f"  - Claimable Savings:  {pct:+.2f}% ({delta:+,} tokens)")
            print(f"  - Acceptance:         Both arms achieved 100% (C = 1.0)")
        else:
            print(f"Matched Pair Result: [SAVINGS INVALIDATED - {verdict}]")
            print(f"  - Control A Accepted:   {pair_summary['acceptance']['control_A_accepted']}")
            print(f"  - Treatment B Accepted: {pair_summary['acceptance']['treatment_B_accepted']}")
            print(f"  - Token Savings Claim:  SUPPRESSED (null)")
            print(f"  - Reason: {comp.get('explanation')}")
        print("=======================================================\n")


    def _create_skipped_arm_record(
        self,
        arm: str,
        exec_order: int,
        reason: str,
        execution_status: str = "SKIPPED_STOP_ON_FAILURE"
    ) -> Dict[str, Any]:
        """Creates a valid, structured skipped arm telemetry record adhering to schema."""
        started_dt = datetime.datetime.now(datetime.timezone.utc)
        iso_now = started_dt.isoformat()
        run_id = f"{self.task_id}_{self.pair_id}_{arm}"
        run_vault = self.output_dir / arm
        run_vault.mkdir(parents=True, exist_ok=True)
        arm_identity = self.arm_identities.get(arm)
        if arm_identity:
            is_engram = arm_identity.is_engram
            expected_sha = arm_identity.expected_sha256
            bin_path = str(arm_identity.binary_path) if arm_identity.binary_path else None
            req_cand = arm_identity.requested_candidate_version
            res_cand = arm_identity.resolved_candidate_version
            cand_status = "SKIPPED" if is_engram else "NOT_APPLICABLE"
        else:
            is_engram = arm in (
                "treatment_B", "v151_arm2", "v154_arm3",
                "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive",
                "arm_c_cand4_1_always_on", "arm_d_cand4_1_adaptive",
                "arm_c_cand4_2_always_on", "arm_d_cand4_2_adaptive"
            )
            expected_sha = self.arm_expected_hashes.get(arm)
            bin_path = str(self.arm_binaries.get(arm)) if self.arm_binaries.get(arm) else None
            req_cand = self.requested_candidate_version
            res_cand = self.resolved_candidate_version if is_engram else "native"
            cand_status = "SKIPPED" if is_engram else "NOT_APPLICABLE"
        parser = TelemetryParser(provenance="UNAVAILABLE")
        telemetry = parser.create_unavailable_metrics(reason)

        gate_results = {
            "L0_typecheck": "SKIPPED",
            "L1_lint_prettier": "SKIPPED",
            "L2_unit_tests": "SKIPPED",
            "L3_production_build": "SKIPPED"
        }
        req_results = {req["id"]: "NOT_EVALUATED" for req in self.task_config.get("requirements", [])}
        is_deferred = (execution_status == "DEFERRED_AMENDMENT_04")
        correctness = CorrectnessResult(
            score=0.0 if is_deferred else None,
            quality_adjusted_cost=0.0 if is_deferred else None,
            passed_all_requirements=False if is_deferred else None,
            gates=gate_results,
            requirements=req_results
        )
        git_impact = GitImpact(
            files_changed_count=0,
            files_changed=[],
            insertions=0,
            deletions=0,
            unrelated_files_count=0,
            diff_patch_path=""
        )

        record = {
            "schema_version": "2.0.0",
            "run_id": run_id,
            "pair_id": self.pair_id,
            "task_id": self.task_id,
            "arm": arm,
            "execution_order_in_pair": exec_order,
            "execution_status": execution_status,
            "error_message": reason,
            "telemetry_valid": False,
            "timestamps": {
                "started_at": iso_now,
                "completed_at": iso_now,
                "elapsed_seconds": 0.0,
            },
            "model": {
                "provider": "openai" if self.provider == "cloud" else self.provider,
                "name": self.model,
                "mode": "agent",
                "reasoning_effort": self.reasoning_effort if self.provider == "cloud" else "none",
                "harness_executable": str(self.codex_bin),
                "harness_version": "skipped",
            },
            "isolation": {
                "baseline_commit": str(self.task_config.get("baseline_commit", "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2")),
                "sandbox_path": "SKIPPED",
                "cold_start_verified": False,
                "engram_enabled": is_engram,
                "engram_binary_sha256": expected_sha or "NONE",
                "engram_binary_expected_sha256": expected_sha,
                "engram_binary_actual_sha256": None,
                "engram_binary_path": bin_path,
                "benchmark_candidate_version": self.benchmark_candidate_version,
                "requested_candidate_version": req_cand,
                "resolved_candidate_version": res_cand,
                "candidate_verification_status": cand_status,
                "mcp_registration_verified": False,
                "mcp_server_status": "SKIPPED",
                "dependency_fingerprint_verified": True,
                "path_sanitized": True,
            },
            "telemetry": telemetry.to_dict(),
            "git_impact": git_impact.to_dict(),
            "correctness": correctness.to_dict(),
            "headroom_guard": {
                "headroom_reserve": self.headroom_reserve,
                "pre_dispatch_guard_triggered": False,
                "stop_reason": None,
                "cumulative_tokens_at_stop": None,
                "estimated_next_turn_cost": None
            }
        }

        if self.telemetry_schema:
            validate_instance(record, self.telemetry_schema)

        # Write skipped record to disk
        with open(run_vault / "run_telemetry.json", "w", encoding="utf-8") as f:
            json.dump(record, f, indent=2)

        return record

    def _execute_single_arm(
        self,
        arm: str,
        exec_order: int,
        arm_max_tokens_ceiling: Optional[int] = None
    ) -> Dict[str, Any]:
        """Executes, evaluates, and verifies a single arm in an isolated sandbox."""
        print(f"\n=======================================================")
        print(f"Executing Arm: {arm} (Order: {exec_order}) [Task: {self.task_id}]")
        print(f"=======================================================")

        orig_runner_ceiling = getattr(self.runner, "max_tokens_ceiling", self.max_tokens_ceiling)
        if arm_max_tokens_ceiling is not None:
            self.runner.max_tokens_ceiling = arm_max_tokens_ceiling

        run_id = f"{self.task_id}_{self.pair_id}_{arm}"
        run_vault = self.output_dir / arm
        run_vault.mkdir(parents=True, exist_ok=True)
        # 1. Verify Engram binary immediately before execution if arm uses Engram
        arm_identity = self.arm_identities.get(arm)
        if arm_identity is None:
            raise SafetyViolationError(f"FATAL: Unregistered arm '{arm}' encountered in orchestrator.")

        # Fail-closed validation against candidate ledger
        arm_identity.validate_against_ledger(self.candidate_ledger)

        binary = arm_identity.binary_path
        expected_sha = arm_identity.expected_sha256
        is_engram = arm_identity.is_engram
        is_native = not is_engram

        if is_native:
            if binary is not None or is_engram:
                raise SafetyViolationError(
                    f"FATAL: Native arm '{arm}' unexpectedly configured with Engram binary '{binary}'. "
                    f"Native arms must execute with zero MCP integration."
                )
        elif is_engram:
            arm_identity.verify_binary_integrity()

        # 2. Strict Live-Run Guard: Safe Dependency Strategy (Remediation C)
        task_requires_deps = self.task_config.get("requires_dependencies", True)
        is_read_only_task = self.task_config.get("read_only", False)

        if not self.is_dry_run:
            if self.dependency_strategy == "none":
                if task_requires_deps and not is_read_only_task:
                    raise SafetyViolationError(
                        f"FATAL: Task '{self.task_id}' requires dependencies (requires_dependencies=True). "
                        f"Cannot use dependency_strategy='none' on live benchmark runs for tasks requiring compilation or testing."
                    )
            elif self.dependency_strategy != "copy":
                raise SafetyViolationError(
                    f"FATAL: Live benchmark runs must use dependency_strategy='copy' (or 'none' for read-only tasks). "
                    f"Attempted to run live arm with '{self.dependency_strategy}'."
                )

        # 3. Verify golden dependency tree integrity prior to sandbox setup
        verify_dependency_integrity(self.golden_repo, self.golden_baseline_fingerprint)

        # 4. Provision sandbox
        baseline_commit = self.task_config.get("baseline_commit", "1c2856086f3a08e3ab1c3b8573eb3ff22ff20fd2")
        sandbox_path = create_sandbox(
            task_id=self.task_id,
            pair_id=self.pair_id,
            arm=arm,
            golden_repo=self.golden_repo,
            baseline_commit=baseline_commit,
            dependency_strategy=self.dependency_strategy,
            repo_type=self.repo_type
        )
        print(f"Sandbox created at: {sandbox_path}")

        try:
            # 4.1 Assert sandbox identity and cleanliness before task execution
            if not self.is_dry_run or (Path(sandbox_path).exists() and (Path(sandbox_path) / ".git").exists()):
                verify_sandbox_identity(sandbox_path, baseline_commit)
                verify_sandbox_cleanliness(sandbox_path, repo_type=self.repo_type)

            # 4.2 Validate fixture preflight in the actual provisioned sandbox
            self.evaluator.validate_preflight(sandbox_path)

            # 4.3 Execute agent
            prompt = self.task_config.get("prompt", "")
            events_jsonl_path = run_vault / "events.jsonl"
            exec_res = self.runner.run(
                sandbox_path=sandbox_path,
                prompt=prompt,
                arm=arm,
                output_events_path=events_jsonl_path,
                is_dry_run=self.is_dry_run,
                engram_executable=binary,
                task_config=self.task_config
            )

            # Determine execution status
            if getattr(exec_res, "timed_out", False):
                execution_status = "TIMEOUT"
            elif getattr(exec_res, "stop_reason", None) == "PRE_DISPATCH_REQUEST_SIZE_EXCEEDED" or not getattr(exec_res, "request_size_gate_passed", True):
                execution_status = "PRE_DISPATCH_REQUEST_SIZE_EXCEEDED"
            elif getattr(exec_res, "pre_dispatch_guard_triggered", False):
                execution_status = "PRE_DISPATCH_HEADROOM_EXHAUSTED"
            elif getattr(exec_res, "token_ceiling_breached", False):
                execution_status = "TOKEN_CEILING_BREACHED"
            elif not getattr(exec_res, "telemetry_valid", True) or getattr(exec_res, "stop_reason", None) == "TELEMETRY_INVALID":
                execution_status = "TELEMETRY_INVALID"
            elif getattr(exec_res, "stop_reason", None) == "PROCESS_CRASH":
                execution_status = "PROCESS_CRASH"
            elif exec_res.exit_code != 0:
                execution_status = "FAILED"
            else:
                execution_status = "SUCCESS"

            # 5. Parse Telemetry safely
            is_mock_transport = getattr(getattr(self.runner, "transport", None), "provider_name", "") == "mock_transport"
            provenance = "SYNTHETIC_TEST" if (self.is_dry_run or is_mock_transport) else "DIRECT_API"
            parser = TelemetryParser(provenance=provenance)
            try:
                if events_jsonl_path.exists() and events_jsonl_path.stat().st_size > 0:
                    telemetry = parser.parse_file(events_jsonl_path, thread_id=exec_res.thread_id)
                elif getattr(exec_res, "native_rollout_path", None) and Path(exec_res.native_rollout_path).exists():
                    telemetry = parser.parse_rollout_file(exec_res.native_rollout_path)
                else:
                    telemetry = parser.create_unavailable_metrics(exec_res.error_message or "Events file missing or unwritten")
            except Exception as telem_err:
                telemetry = parser.create_unavailable_metrics(f"Telemetry parsing failed: {telem_err}")

            # Reconcile parsed telemetry against effective arm ceiling
            effective_ceiling = arm_max_tokens_ceiling if arm_max_tokens_ceiling is not None else getattr(self.runner, "max_tokens_ceiling", self.max_tokens_ceiling)
            if effective_ceiling is not None and isinstance(getattr(telemetry, "total_model_tokens", None), int):
                if telemetry.total_model_tokens > effective_ceiling:
                    execution_status = "TOKEN_CEILING_BREACHED"
                    exec_res.token_ceiling_breached = True
                    if not exec_res.error_message:
                        exec_res.error_message = (
                            f"Execution breached effective token ceiling ({telemetry.total_model_tokens:,} > {effective_ceiling:,} tokens)"
                        )

            # 6. Evaluate Git Impact & Check for Dependency Mutations
            patch_path = run_vault / "changes.patch"
            try:
                git_impact = self.evaluator.analyze_git_impact(sandbox_path, patch_path)
            except Exception:
                git_impact = GitImpact(
                    files_changed_count=0,
                    files_changed=[],
                    insertions=0,
                    deletions=0,
                    unrelated_files_count=0,
                    diff_patch_path=str(patch_path)
                )

            # 7. Automated Gates & Correctness Score
            if execution_status != "SUCCESS":
                # Arm failed execution (timeout, ceiling breached, or crashed). Suppress running live automated gates.
                gate_results = {
                    "L0_typecheck": "BLOCKED",
                    "L1_lint_prettier": "SKIPPED",
                    "L2_unit_tests": "BLOCKED",
                    "L3_production_build": "BLOCKED"
                }
                req_results = {req["id"]: "FAIL" for req in self.task_config.get("requirements", [])}
                correctness = CorrectnessResult(
                    score=0.0,
                    quality_adjusted_cost=float(telemetry.total_model_tokens * 10.0 if telemetry.total_model_tokens else 0.0),
                    passed_all_requirements=False,
                    gates=gate_results,
                    requirements=req_results
                )
            else:
                if is_read_only_task:
                    # Defense-in-depth: Any modified, deleted, or newly created file strictly fails acceptance
                    if git_impact.files_changed_count > 0 or git_impact.unrelated_files_count > 0 or git_impact.insertions > 0 or git_impact.deletions > 0:
                        print(f"[READ-ONLY INVARIANT VIOLATED] Agent mutated sandbox files in read-only task: {git_impact.files_changed}")
                        req_results = {req["id"]: "FAIL" for req in self.task_config.get("requirements", [])}
                        correctness = CorrectnessResult(
                            score=0.0,
                            quality_adjusted_cost=float(telemetry.total_model_tokens * 10.0 if telemetry.total_model_tokens else 0.0),
                            passed_all_requirements=False,
                            gates={"L0_typecheck": "SKIPPED", "L1_lint_prettier": "SKIPPED", "L2_unit_tests": "SKIPPED", "L3_production_build": "SKIPPED"},
                            requirements=req_results
                        )
                    else:
                        target_file = Path(sandbox_path) / "projects/angular-ngrx-material-starter/src/app/core/auth/auth.actions.ts"
                        symbol_present = False
                        if target_file.exists():
                            content = target_file.read_text(encoding="utf-8", errors="ignore")
                            symbol_present = ("authLogout" in content and "export const authLogout = createAction('[Auth] Logout');" in content)
                        req_results = {
                            "REQ-1": "PASS" if symbol_present else "FAIL",
                            "REQ-2": "PASS"
                        }
                        all_passed = all(status == "PASS" for status in req_results.values())
                        correctness = CorrectnessResult(
                            score=1.0 if all_passed else 0.0,
                            quality_adjusted_cost=float(telemetry.total_model_tokens if all_passed else (telemetry.total_model_tokens * 10.0 if telemetry.total_model_tokens else 0.0)),
                            passed_all_requirements=all_passed,
                            gates={"L0_typecheck": "SKIPPED", "L1_lint_prettier": "SKIPPED", "L2_unit_tests": "SKIPPED", "L3_production_build": "SKIPPED"},
                            requirements=req_results
                        )
                else:
                    gate_results = self.evaluator.run_automated_gates(
                        sandbox_path,
                        git_impact=git_impact,
                        is_dry_run=self.is_dry_run
                    )
                    req_results = self.evaluator.evaluate_requirements(sandbox_path, git_impact, gate_results)
                    correctness = self.evaluator.calculate_score(
                        req_results=req_results,
                        total_model_tokens=telemetry.total_model_tokens,
                        gate_results=gate_results,
                        dependency_mutated=git_impact.dependency_mutated
                    )

            if telemetry.replacement_accounting and correctness.passed_all_requirements and (telemetry.total_model_tokens or 0) > 0:
                telemetry.replacement_accounting.tokens_per_successful_task = float(telemetry.total_model_tokens)

            # 9. Build Telemetry Record adhering strictly to schema
            record = {
                "schema_version": "2.0.0",
                "run_id": run_id,
                "pair_id": self.pair_id,
                "task_id": self.task_id,
                "arm": arm,
                "execution_order_in_pair": exec_order,
                "execution_status": execution_status,
                "error_message": exec_res.error_message,
                "telemetry_valid": telemetry.telemetry_valid,
                "timestamps": {
                    "started_at": exec_res.started_at,
                    "completed_at": exec_res.completed_at,
                    "elapsed_seconds": exec_res.elapsed_seconds,
                },
                "model": {
                    "provider": getattr(exec_res, "provider", self.provider) if getattr(exec_res, "provider", self.provider) != "cloud" else "openai",
                    "name": exec_res.model_name,
                    "mode": "agent",
                    "reasoning_effort": exec_res.reasoning_effort if self.provider == "cloud" else "none",
                    "harness_executable": exec_res.harness_executable,
                    "harness_version": exec_res.harness_version,
                    "transport_identity": getattr(getattr(self.runner, "transport", None), "__class__", type(None)).__name__,
                    "transport_name": getattr(getattr(self.runner, "transport", None), "provider_name", "unknown"),
                    "transport_allow_live_network": getattr(getattr(self.runner, "transport", None), "allow_live_network", False),
                },
                "isolation": {
                    "baseline_commit": baseline_commit,
                    "sandbox_path": str(sandbox_path),
                    "cold_start_verified": True,
                    "engram_enabled": arm_identity.is_engram,
                    "engram_binary_sha256": arm_identity.expected_sha256 or "NONE",
                    "engram_binary_expected_sha256": arm_identity.expected_sha256,
                    "engram_binary_actual_sha256": (hashlib.sha256(binary.read_bytes()).hexdigest().upper() if (arm_identity.is_engram and binary and binary.exists()) else None),
                    "engram_binary_path": str(binary) if binary else None,
                    "benchmark_candidate_version": self.benchmark_candidate_version,
                    "requested_candidate_version": arm_identity.requested_candidate_version,
                    "resolved_candidate_version": arm_identity.resolved_candidate_version,
                    "candidate_verification_status": (
                        "NOT_APPLICABLE" if not arm_identity.is_engram
                        else ("VERIFIED" if (binary and binary.exists() and hashlib.sha256(binary.read_bytes()).hexdigest().upper() == arm_identity.expected_sha256) else "FAILED")
                    ),
                    "mcp_registration_verified": True if arm_identity.is_engram else False,
                    "mcp_server_status": (
                        "ADAPTIVE_BYPASS" if (getattr(exec_res, "adaptive_activation", {}) or {}).get("routed_mode") == "BYPASS"
                        else ("FAST_PATH_BYPASS" if getattr(exec_res, "fast_path_active", False)
                        else ("ACTIVE" if arm_identity.is_engram else "DISABLED"))
                    ),
                    "dependency_fingerprint_verified": True,
                    "path_sanitized": exec_res.path_sanitized,
                    "sandbox_policy": getattr(exec_res, "sandbox_policy", self.runner.sandbox_policy),
                },
                "telemetry": telemetry.to_dict(),
                "git_impact": git_impact.to_dict(),
                "correctness": correctness.to_dict(),
                "headroom_guard": {
                    "headroom_reserve": getattr(exec_res, "headroom_reserve", self.headroom_reserve),
                    "pre_dispatch_guard_triggered": getattr(exec_res, "pre_dispatch_guard_triggered", False),
                    "stop_reason": getattr(exec_res, "stop_reason", None),
                    "cumulative_tokens_at_stop": getattr(exec_res, "cumulative_tokens_at_stop", None),
                    "estimated_next_turn_cost": getattr(exec_res, "estimated_next_turn_cost", None),
                    "configured_ceiling": getattr(exec_res, "configured_ceiling", getattr(self.runner, "max_tokens_ceiling", None)),
                    "remaining_budget": getattr(exec_res, "remaining_budget_at_stop", None),
                    "last_turn_usage": getattr(exec_res, "last_turn_usage_at_stop", None),
                    "guard_decision": getattr(exec_res, "guard_decision", "DISPATCH_ALLOWED" if not getattr(exec_res, "pre_dispatch_guard_triggered", False) else "PREVENTIVE_HALT"),
                    "timestamp": getattr(exec_res, "guard_decision_timestamp", None) or exec_res.completed_at,
                    "relevant_event_id": getattr(exec_res, "relevant_event_id", None),
                    "telemetry_valid": getattr(exec_res, "telemetry_valid", True)
                },
                "request_size_gate": {
                    "gate_passed": getattr(exec_res, "request_size_gate_passed", True),
                    "measured_request_tokens": getattr(exec_res, "measured_request_tokens", 0),
                    "measured_request_bytes": getattr(exec_res, "measured_request_bytes", 0),
                    "counting_method": getattr(exec_res, "request_counting_method", "tiktoken_o200k_base"),
                    "limit_tokens": getattr(exec_res, "configured_request_size_limit", 60_000),
                    "breakdown": getattr(exec_res, "request_size_breakdown", None)
                }
            }

            if getattr(exec_res, "adaptive_activation", None) is not None:
                record["adaptive_activation"] = exec_res.adaptive_activation

            # 10. Validate strictly against schema
            validate_instance(record, self.telemetry_schema)

            # 11. Write sealed telemetry file
            telemetry_out = run_vault / "run_telemetry.json"
            with open(telemetry_out, "w", encoding="utf-8") as f:
                json.dump(record, f, indent=2)

            print(f"Run {run_id} completed with status: {execution_status}.")
            print(f"  - Total Model Tokens: {telemetry.total_model_tokens} (Valid: {telemetry.telemetry_valid})")
            print(f"  - Peak Context:       {telemetry.peak_single_request_context or telemetry.peak_turn_context}")
            print(f"  - Correctness Score:  {correctness.score} (Accepted: {correctness.passed_all_requirements})")
            print(f"  - Effective Cost:     {correctness.quality_adjusted_cost}")

            return record

        finally:
            if arm_max_tokens_ceiling is not None:
                self.runner.max_tokens_ceiling = orig_runner_ceiling
            # 12. Teardown sandbox safely
            teardown_sandbox(sandbox_path, self.golden_repo, preserve_archive=self.preserve_sandboxes, repo_type=self.repo_type)
            # 13. Re-verify golden dependency tree integrity post teardown
            verify_dependency_integrity(self.golden_repo, self.golden_baseline_fingerprint)
            print(f"Sandbox cleaned up safely; golden dependency integrity verified.")

    def run_triad(
        self,
        block_id: Optional[str] = None,
        arm_order: Optional[List[str]] = None
    ) -> Dict[str, Any]:
        """
        Executes a 3-arm matched block (Stage 1 Core Triad) across:
        1. baseline_arm1: Native baseline exploration without Engram MCP
        2. v151_arm2: Engram v1.5.1 interactive MCP workflow
        3. v154_arm3: Engram v1.5.4 efficiency-first workflow (PrepareTask, ChangeGuard, Local Postflight)
        Enforces Latin-square counterbalancing and verified golden dependency tree integrity.
        """
        effective_block_id = block_id or self.pair_id

        # Determine arm order and seed
        if arm_order:
            order = arm_order
        elif effective_block_id in TRIAD_BLOCK_SCHEDULES:
            sched = TRIAD_BLOCK_SCHEDULES[effective_block_id]
            order = sched["order"]
            self.seed = sched["seed"]
        else:
            rng = random.Random(self.seed)
            latin_orders = [
                ["baseline_arm1", "v151_arm2", "v154_arm3"],
                ["v151_arm2", "v154_arm3", "baseline_arm1"],
                ["v154_arm3", "baseline_arm1", "v151_arm2"],
                ["baseline_arm1", "v154_arm3", "v151_arm2"],
                ["v151_arm2", "baseline_arm1", "v154_arm3"],
                ["v154_arm3", "v151_arm2", "baseline_arm1"],
            ]
            order = rng.choice(latin_orders)

        triad_summary: Dict[str, Any] = {
            "schema_version": "2.0.0",
            "block_id": effective_block_id,
            "task_id": self.task_id,
            "seed": self.seed,
            "order": order,
            "model": self.model,
            "reasoning_effort": self.reasoning_effort,
            "golden_dependency_fingerprint_verified": True,
            "acceptance": {
                "baseline_arm1_accepted": False,
                "v151_arm2_accepted": False,
                "v154_arm3_accepted": False,
                "all_arms_accepted": False
            },
            "comparative_analysis": {},
            "runs": {}
        }

        # Execute each arm in counterbalanced order
        for exec_order, arm in enumerate(order, start=1):
            run_data = self._execute_single_arm(arm=arm, exec_order=exec_order)
            triad_summary["runs"][arm] = run_data

            # Check stop-on-failure policy
            arm_status = run_data.get("execution_status", "SUCCESS")
            arm_telem = run_data.get("telemetry", {})
            telem_valid = arm_telem.get("telemetry_valid", True)
            is_failed = (arm_status != "SUCCESS") or (not telem_valid)

            if self.stop_on_failure and is_failed and exec_order < len(order):
                print(f"\n[ORCHESTRATOR STOP-ON-FAILURE] Arm {arm} failed ({arm_status}). Halting triad execution immediately.")
                # Populate remaining unexecuted arms with structured skipped records
                for rem_idx in range(exec_order, len(order)):
                    rem_arm = order[rem_idx]
                    skip_reason = f"Execution halted: stop-on-failure triggered by {arm} ({arm_status})"
                    triad_summary["runs"][rem_arm] = self._create_skipped_arm_record(rem_arm, rem_idx + 1, skip_reason)
                break

        # Ensure all triad arms expected by triad_summary.schema.json are present in runs.
        # If an arm was omitted from execution order (e.g. deferred under Amendment 04 Alternative B),
        # create a structured skipped record with execution_status='DEFERRED_AMENDMENT_04'.
        all_triad_arms = ["baseline_arm1", "v151_arm2", "v154_arm3"]
        for triad_arm in all_triad_arms:
            if triad_arm not in triad_summary["runs"]:
                deferred_order = len(order) + 1
                skip_reason = f"Execution deferred under protocol specification (not in execution order {order})"
                triad_summary["runs"][triad_arm] = self._create_skipped_arm_record(
                    triad_arm, deferred_order, skip_reason, execution_status="DEFERRED_AMENDMENT_04"
                )

        # Determine acceptance for each arm
        arm1_data = triad_summary["runs"].get("baseline_arm1", {})
        arm2_data = triad_summary["runs"].get("v151_arm2", {})
        arm3_data = triad_summary["runs"].get("v154_arm3", {})

        def _is_accepted(run_dict: Dict[str, Any]) -> bool:
            return Orchestrator.is_run_accepted(run_dict)

        arm1_acc = _is_accepted(arm1_data)
        arm2_acc = _is_accepted(arm2_data)
        arm3_acc = _is_accepted(arm3_data)
        all_acc = bool(arm1_acc and arm2_acc and arm3_acc)

        triad_summary["acceptance"]["baseline_arm1_accepted"] = arm1_acc
        triad_summary["acceptance"]["v151_arm2_accepted"] = arm2_acc
        triad_summary["acceptance"]["v154_arm3_accepted"] = arm3_acc
        triad_summary["acceptance"]["all_arms_accepted"] = all_acc

        # Pairwise Comparative Analyses:
        # 1. Primary Predeclared Outcome: Arm 3 (v1.5.4) vs Arm 1 (Baseline)
        v154_vs_baseline = self.compute_comparative_analysis(
            ctrl_run=arm1_data,
            treat_run=arm3_data,
            baseline_label="baseline_arm1",
            treatment_label="v154_arm3"
        )

        # 2. Architectural Evolution: Arm 3 (v1.5.4) vs Arm 2 (v1.5.1)
        v154_vs_v151 = self.compute_comparative_analysis(
            ctrl_run=arm2_data,
            treat_run=arm3_data,
            baseline_label="v151_arm2",
            treatment_label="v154_arm3"
        )

        # 3. First-Generation Benchmark: Arm 2 (v1.5.1) vs Arm 1 (Baseline)
        v151_vs_baseline = self.compute_comparative_analysis(
            ctrl_run=arm1_data,
            treat_run=arm2_data,
            baseline_label="baseline_arm1",
            treatment_label="v151_arm2"
        )

        triad_summary["comparative_analysis"] = {
            "v154_vs_baseline": v154_vs_baseline,
            "v154_vs_v151": v154_vs_v151,
            "v151_vs_baseline": v151_vs_baseline,
        }

        # Schema Validation
        if self.triad_schema:
            validate_instance(triad_summary, self.triad_schema)

        # Write triad summary JSON
        summary_path = self.output_dir / "triad_summary.json"
        with open(summary_path, "w", encoding="utf-8") as f:
            json.dump(triad_summary, f, indent=2)

        # Write triad markdown report
        md_content = self.generate_triad_markdown_report(triad_summary)
        report_path = self.output_dir / "triad_report.md"
        with open(report_path, "w", encoding="utf-8") as f:
            f.write(md_content)

        # Print human-readable verdict banner
        self._print_triad_verdict(triad_summary)

        return triad_summary

    @staticmethod
    def generate_triad_markdown_report(triad_summary: Dict[str, Any]) -> str:
        """Generates a comprehensive Stage 1 Core Triad markdown report with 3-arm comparative analysis."""
        task_id = triad_summary.get("task_id", "")
        block_id = triad_summary.get("block_id", "")
        seed = triad_summary.get("seed", "")
        order = " -> ".join(triad_summary.get("order", []))
        model = triad_summary.get("model", "")
        reasoning = triad_summary.get("reasoning_effort", "")

        acceptance = triad_summary.get("acceptance", {})
        runs = triad_summary.get("runs", {})
        comp = triad_summary.get("comparative_analysis", {})

        arm1 = runs.get("baseline_arm1", {})
        arm2 = runs.get("v151_arm2", {})
        arm3 = runs.get("v154_arm3", {})

        arm1_corr = arm1.get("correctness", {})
        arm2_corr = arm2.get("correctness", {})
        arm3_corr = arm3.get("correctness", {})

        arm1_score = arm1_corr.get("score")
        arm2_score = arm2_corr.get("score")
        arm3_score = arm3_corr.get("score")

        arm1_pass = arm1_corr.get("passed_all_requirements")
        arm2_pass = arm2_corr.get("passed_all_requirements")
        arm3_pass = arm3_corr.get("passed_all_requirements")

        arm1_score_str = f"{arm1_score:.2f}" if arm1_score is not None else "SKIPPED"
        arm2_score_str = f"{arm2_score:.2f}" if arm2_score is not None else "SKIPPED"
        arm3_score_str = f"{arm3_score:.2f}" if arm3_score is not None else "SKIPPED"

        arm1_pass_str = str(arm1_pass) if arm1_pass is not None else "NOT_EVALUATED"
        arm2_pass_str = str(arm2_pass) if arm2_pass is not None else "NOT_EVALUATED"
        arm3_pass_str = str(arm3_pass) if arm3_pass is not None else "NOT_EVALUATED"

        arm1_verdict = "PASS" if arm1_pass and arm1_score == 1.0 else ("SKIPPED" if arm1_score is None else "FAIL")
        arm2_verdict = "PASS" if arm2_pass and arm2_score == 1.0 else ("SKIPPED" if arm2_score is None else "FAIL")
        arm3_verdict = "PASS" if arm3_pass and arm3_score == 1.0 else ("SKIPPED" if arm3_score is None else "FAIL")

        lines = [
            f"# Stage 1 Core Triad Benchmark Report: `{task_id}` (`{block_id}`)",
            "",
            f"- **Protocol:** Stage 1 Core Triad (Amendment 02)",
            f"- **Model:** `{model}` (Reasoning: `{reasoning}`)",
            f"- **Execution Order (Latin Square):** {order} (Seed: `{seed}`)",
            f"- **Golden Dependency Fingerprint:** Verified Intact",
            "",
            "---",
            "",
            "## 1. Acceptance & Correctness Gate",
            "",
            "| Arm | Engram Version / Status | Correctness Score ($C$) | Requirements Passed | Gate Verdict |",
            "| :--- | :---: | :---: | :---: | :---: |",
            f"| **Arm 1: Baseline** | DISABLED | {arm1_score_str} | {arm1_pass_str} | {arm1_verdict} |",
            f"| **Arm 2: v1.5.1** | v1.5.1 ACTIVE | {arm2_score_str} | {arm2_pass_str} | {arm2_verdict} |",
            f"| **Arm 3: v1.5.4** | v1.5.4 ACTIVE | {arm3_score_str} | {arm3_pass_str} | {arm3_verdict} |",
            "",
        ]

        if not acceptance.get("all_arms_accepted", False):
            lines.extend([
                "> [!WARNING]",
                "> **ONE OR MORE ARMS FAILED ACCEPTANCE CRITERIA ($C < 1.0$)**",
                "> In accordance with Section 6 of Amendment 02, any arm failing functional correctness strictly invalidates its comparative token savings claims.",
                "",
            ])
        else:
            lines.extend([
                "> [!NOTE]",
                "> **ALL THREE ARMS ACHIEVED FULL ACCEPTANCE ($C = 1.0$)**",
                "> Comparative token efficiency and quality-adjusted cost ($QAC$) metrics are scientifically valid.",
                "",
            ])

        # Telemetry Comparison Table
        arm1_tel = arm1.get("telemetry", {})
        arm2_tel = arm2.get("telemetry", {})
        arm3_tel = arm3.get("telemetry", {})

        lines.extend([
            "## 2. Telemetry Comparison",
            "",
            "| Metric | Arm 1 (Baseline) | Arm 2 (v1.5.1) | Arm 3 (v1.5.4) | Arm 3 vs Arm 1 Delta | Arm 3 vs Arm 2 Delta |",
            "| :--- | :---: | :---: | :---: | :---: | :---: |",
        ])

        v154_vs_base_m = comp.get("v154_vs_baseline", {}).get("metrics", {})
        v154_vs_v151_m = comp.get("v154_vs_v151", {}).get("metrics", {})

        metric_rows = [
            ("total_model_tokens", "Total Model Tokens", arm1_tel.get("total_model_tokens"), arm2_tel.get("total_model_tokens"), arm3_tel.get("total_model_tokens")),
            ("prompt_tokens_total", "Prompt Tokens (Total)", arm1_tel.get("prompt_tokens_total"), arm2_tel.get("prompt_tokens_total"), arm3_tel.get("prompt_tokens_total")),
            ("completion_tokens", "Completion Tokens", arm1_tel.get("completion_tokens"), arm2_tel.get("completion_tokens"), arm3_tel.get("completion_tokens")),
            ("peak_single_request_context", "Peak Single-Request Context", arm1_tel.get("peak_single_request_context"), arm2_tel.get("peak_single_request_context"), arm3_tel.get("peak_single_request_context")),
            ("quality_adjusted_cost", "Quality-Adjusted Effective Cost", arm1_corr.get("quality_adjusted_cost"), arm2_corr.get("quality_adjusted_cost"), arm3_corr.get("quality_adjusted_cost")),
        ]

        for mkey, mlabel, v1, v2, v3 in metric_rows:
            s1 = f"{v1:,}" if isinstance(v1, (int, float)) else str(v1)
            s2 = f"{v2:,}" if isinstance(v2, (int, float)) else str(v2)
            s3 = f"{v3:,}" if isinstance(v3, (int, float)) else str(v3)

            def _fmt_delta(m_dict):
                st = m_dict.get("status")
                if st == "INVALIDATED_UNACCEPTED":
                    return "**INVALIDATED ($C < 1.0$)**"
                if st == "INVALIDATED_TELEMETRY":
                    return "**INVALIDATED (Telemetry)**"
                if st == "UNAVAILABLE":
                    return "UNAVAILABLE"
                pct = m_dict.get("savings_pct")
                d = m_dict.get("delta")
                return f"**{pct:+.1f}%** ({d:+,})" if pct is not None else "N/A"

            d_base = _fmt_delta(v154_vs_base_m.get(mkey, {}))
            d_v151 = _fmt_delta(v154_vs_v151_m.get(mkey, {}))

            lines.append(f"| **{mlabel}** | {s1} | {s2} | {s3} | {d_base} | {d_v151} |")

        lines.extend([
            "",
            "## 3. Pairwise Comparative Analysis",
            "",
            "### A. Primary Outcome: Arm 3 (v1.5.4) vs Arm 1 (Baseline)",
            f"- **Verdict:** `{comp.get('v154_vs_baseline', {}).get('verdict', 'UNKNOWN')}`",
            f"- **Explanation:** {comp.get('v154_vs_baseline', {}).get('explanation', '')}",
            "",
            "### B. Architectural Evolution: Arm 3 (v1.5.4) vs Arm 2 (v1.5.1)",
            f"- **Verdict:** `{comp.get('v154_vs_v151', {}).get('verdict', 'UNKNOWN')}`",
            f"- **Explanation:** {comp.get('v154_vs_v151', {}).get('explanation', '')}",
            "",
            "### C. First-Generation Benchmark: Arm 2 (v1.5.1) vs Arm 1 (Baseline)",
            f"- **Verdict:** `{comp.get('v151_vs_baseline', {}).get('verdict', 'UNKNOWN')}`",
            f"- **Explanation:** {comp.get('v151_vs_baseline', {}).get('explanation', '')}",
            "",
            "---",
            "*Report generated deterministically by Engram Core Triad Harness.*",
            ""
        ])

        return "\n".join(lines)

    @staticmethod
    def _print_triad_verdict(triad_summary: Dict[str, Any]) -> None:
        block_id = triad_summary.get("block_id", "")
        comp = triad_summary.get("comparative_analysis", {})
        acc = triad_summary.get("acceptance", {})
        p_res = comp.get("v154_vs_baseline", {})
        p_tot = p_res.get("metrics", {}).get("total_model_tokens", {})

        print("\n=======================================================")
        print(f"Stage 1 Core Triad Result: [{block_id}]")
        print(f"  - Baseline Arm 1 Accepted: {acc.get('baseline_arm1_accepted')}")
        print(f"  - v1.5.1 Arm 2 Accepted:   {acc.get('v151_arm2_accepted')}")
        print(f"  - v1.5.4 Arm 3 Accepted:   {acc.get('v154_arm3_accepted')}")
        if p_res.get("savings_claim_eligible", False):
            pct = p_tot.get("savings_pct")
            delta = p_tot.get("delta")
            print(f"  - Primary Savings (Arm 3 vs Baseline): {pct:+.2f}% ({delta:+,} tokens) [{p_res.get('verdict')}]")
        else:
            print(f"  - Primary Savings Claim: SUPPRESSED [{p_res.get('verdict')}]")
            print(f"  - Reason: {p_res.get('explanation')}")
        print("=======================================================\n")

    def run_tetrad(
        self,
        block_id: Optional[str] = None,
        arm_order: Optional[List[str]] = None
    ) -> Dict[str, Any]:
        """
        Executes a 4-arm matched block (Phase 5.4 Architecture) across:
        1. arm_a_native: Native baseline without Engram
        2. arm_b_cand3_always_on: Candidate 3 Always-On (quantifies unpruned payload overhead)
        3. arm_c_cand4_always_on: Candidate 4 Always-On (proves engine remediation impact)
        4. arm_d_cand4_adaptive: Candidate 4 Adaptive (proves complexity-aware routing value)
        """
        effective_block_id = block_id or f"tetrad_{self.pair_id}"
        all_tetrad_arms = list(Orchestrator.ALL_TETRAD_ARMS)

        if arm_order is not None:
            order = Orchestrator.validate_tetrad_arm_order(arm_order, allow_none=False)
        else:
            rng = random.Random(self.seed)
            order = list(all_tetrad_arms)
            rng.shuffle(order)

        tetrad_summary: Dict[str, Any] = {
            "schema_version": "2.0.0",
            "block_id": effective_block_id,
            "task_id": self.task_id,
            "seed": self.seed,
            "order": order,
            "model": self.model,
            "reasoning_effort": self.reasoning_effort,
            "golden_dependency_fingerprint_verified": True,
            "execution_policy": self.execution_policy,
            "block_status": "RUNNING",
            "global_tokens_ceiling": self.global_tokens_ceiling,
            "cumulative_global_tokens": 0,
            "acceptance": {},
            "comparative_analysis": {},
            "runs": {}
        }

        cumulative_global_tokens = 0
        block_status = "COMPLETED"
        is_budget_uncertain = False

        for exec_order, arm in enumerate(order, start=1):
            # Check global headroom reserve if global_tokens_ceiling is set
            if self.global_tokens_ceiling is not None:
                global_remaining = self.global_tokens_ceiling - cumulative_global_tokens
                if global_remaining < self.headroom_reserve:
                    print(
                        f"\n[ORCHESTRATOR GLOBAL-BUDGET HALT] Global remaining tokens ({global_remaining:,}) "
                        f"below headroom reserve ({self.headroom_reserve:,}). Halting tetrad."
                    )
                    block_status = "ABORTED_GLOBAL_BUDGET"
                    for rem_idx in range(exec_order - 1, len(order)):
                        rem_arm = order[rem_idx]
                        skip_reason = (
                            f"Execution halted: global token budget headroom reserve exhausted "
                            f"(remaining {global_remaining:,} < reserve {self.headroom_reserve:,})"
                        )
                        tetrad_summary["runs"][rem_arm] = self._create_skipped_arm_record(
                            rem_arm, rem_idx + 1, skip_reason, execution_status="SKIPPED_GLOBAL_BUDGET"
                        )
                    break

                effective_arm_ceiling = min(self.max_tokens_ceiling, global_remaining)
            else:
                effective_arm_ceiling = self.max_tokens_ceiling

            try:
                run_data = self._execute_single_arm(
                    arm=arm,
                    exec_order=exec_order,
                    arm_max_tokens_ceiling=effective_arm_ceiling
                )
                tetrad_summary["runs"][arm] = run_data

                arm_telem = run_data.get("telemetry", {})
                arm_status = run_data.get("execution_status", "SUCCESS")

                # Validate telemetry integrity and fail-closed budget accounting
                tel_valid, tel_reason, tel_data = Orchestrator.validate_telemetry_integrity(arm_telem)

                # Over-ceiling safety violation detection:
                # If an arm reports SUCCESS while consuming more tokens than its effective ceiling,
                # this indicates a harness safety violation or an unconstrained test mock bypass.
                if effective_arm_ceiling is not None and tel_valid and tel_data:
                    raw_spent = tel_data.get("total_model_tokens", 0)
                    if raw_spent > effective_arm_ceiling and arm_status == "SUCCESS":
                        raise SafetyViolationError(
                            f"Arm '{arm}' reported consumption ({raw_spent:,}) exceeding effective ceiling "
                            f"({effective_arm_ceiling:,}) while reporting execution_status SUCCESS."
                        )

                # Accounting fail-closed check
                if tel_valid and tel_data and isinstance(tel_data.get("total_model_tokens"), int) and tel_data["total_model_tokens"] > 0:
                    spent = tel_data["total_model_tokens"]
                    cumulative_global_tokens += spent
                    is_budget_uncertain = False

                    # Immediate fail-closed check: if global ceiling reached or headroom reserve exhausted, halt immediately
                    if self.global_tokens_ceiling is not None and exec_order < len(order):
                        rem_global = self.global_tokens_ceiling - cumulative_global_tokens
                        if rem_global < self.headroom_reserve:
                            halt_reason = (
                                f"global token ceiling reached or exceeded ({cumulative_global_tokens:,} >= {self.global_tokens_ceiling:,})"
                                if rem_global <= 0
                                else f"global token budget headroom reserve exhausted (remaining {rem_global:,} < reserve {self.headroom_reserve:,})"
                            )
                            print(f"\n[ORCHESTRATOR GLOBAL-BUDGET HALT] Execution halted: {halt_reason}. Halting tetrad immediately.")
                            block_status = "ABORTED_GLOBAL_BUDGET"
                            for rem_idx in range(exec_order, len(order)):
                                rem_arm = order[rem_idx]
                                skip_reason = f"Execution halted: {halt_reason}"
                                tetrad_summary["runs"][rem_arm] = self._create_skipped_arm_record(
                                    rem_arm, rem_idx + 1, skip_reason, execution_status="SKIPPED_GLOBAL_BUDGET"
                                )
                            break
                else:
                    spent = None
                    is_budget_uncertain = True
                    run_data.setdefault("telemetry", {})["token_usage_trustworthy"] = False
                    run_data["telemetry"]["budget_uncertain"] = True

            except (SafetyViolationError, FileNotFoundError) as safety_err:
                print(f"\n[ORCHESTRATOR SAFETY-VIOLATION] Arm {arm} encountered safety violation: {safety_err}. Halting block immediately.")
                block_status = "ABORTED_SAFETY_FAILURE"
                tetrad_summary["runs"][arm] = self._create_skipped_arm_record(
                    arm, exec_order, f"Safety violation in {arm}: {safety_err}", execution_status="ABORTED_SAFETY_FAILURE"
                )
                for rem_idx in range(exec_order, len(order)):
                    rem_arm = order[rem_idx]
                    skip_reason = f"Execution halted: safety failure in preceding arm {arm} ({safety_err})"
                    tetrad_summary["runs"][rem_arm] = self._create_skipped_arm_record(
                        rem_arm, rem_idx + 1, skip_reason, execution_status="SKIPPED_SAFETY_FAILURE"
                    )
                break
            except KeyboardInterrupt:
                print(f"\n[ORCHESTRATOR USER-CANCEL] Execution cancelled by user during arm {arm}.")
                block_status = "ABORTED_USER_CANCEL"
                tetrad_summary["runs"][arm] = self._create_skipped_arm_record(
                    arm, exec_order, "Execution cancelled by user", execution_status="ABORTED_USER_CANCEL"
                )
                for rem_idx in range(exec_order, len(order)):
                    rem_arm = order[rem_idx]
                    skip_reason = "Execution cancelled by user"
                    tetrad_summary["runs"][rem_arm] = self._create_skipped_arm_record(
                        rem_arm, rem_idx + 1, skip_reason, execution_status="SKIPPED_USER_CANCEL"
                    )
                break

            # Fail closed immediately if budget is uncertain
            if is_budget_uncertain:
                print(
                    f"\n[ORCHESTRATOR UNCERTAIN-BUDGET HALT] Arm {arm} produced uncertain/unverified token telemetry "
                    f"({tel_reason}). Halting block immediately."
                )
                block_status = "ABORTED_UNCERTAIN_BUDGET"
                tetrad_summary["global_budget_uncertain"] = True
                for rem_idx in range(exec_order, len(order)):
                    rem_arm = order[rem_idx]
                    skip_reason = f"Execution halted: uncertain token budget in preceding arm {arm} ({tel_reason})"
                    tetrad_summary["runs"][rem_arm] = self._create_skipped_arm_record(
                        rem_arm, rem_idx + 1, skip_reason, execution_status="SKIPPED_UNCERTAIN_BUDGET"
                    )
                break

            arm_status = run_data.get("execution_status", "SUCCESS")
            arm_telem = run_data.get("telemetry", {})
            telem_valid = arm_telem.get("telemetry_valid", True)
            is_accepted = Orchestrator.is_run_accepted(run_data)
            is_failed = (arm_status != "SUCCESS") or (not telem_valid)

            if self.execution_policy == "strict" and self.stop_on_failure and is_failed and exec_order < len(order):
                print(f"\n[ORCHESTRATOR STOP-ON-FAILURE] Arm {arm} failed ({arm_status}). Halting tetrad execution immediately.")
                block_status = "MUTUAL_ACCEPTANCE_FAILURE"
                for rem_idx in range(exec_order, len(order)):
                    rem_arm = order[rem_idx]
                    skip_reason = f"Execution halted: stop-on-failure triggered by {arm} ({arm_status})"
                    tetrad_summary["runs"][rem_arm] = self._create_skipped_arm_record(
                        rem_arm, rem_idx + 1, skip_reason, execution_status="SKIPPED"
                    )
                break
            elif self.execution_policy == "diagnostic_continue" and is_failed:
                print(
                    f"\n[ORCHESTRATOR DIAGNOSTIC-CONTINUE] Arm {arm} failed ({arm_status}, accepted={is_accepted}). "
                    f"Diagnostic continuation active; continuing to subsequent arms."
                )

        for arm in all_tetrad_arms:
            if arm not in tetrad_summary["runs"]:
                deferred_order = len(order) + 1
                skip_reason = f"Execution deferred (not in execution order {order})"
                tetrad_summary["runs"][arm] = self._create_skipped_arm_record(
                    arm, deferred_order, skip_reason, execution_status="DEFERRED"
                )

        def _is_accepted(run_dict: Dict[str, Any]) -> bool:
            return Orchestrator.is_run_accepted(run_dict)

        for arm in all_tetrad_arms:
            tetrad_summary["acceptance"][f"{arm}_accepted"] = _is_accepted(tetrad_summary["runs"].get(arm, {}))

        all_arms_accepted = all(tetrad_summary["acceptance"].values())
        tetrad_summary["acceptance"]["all_arms_accepted"] = all_arms_accepted

        # Resolve final block_status
        if block_status == "COMPLETED":
            if self.execution_policy == "diagnostic_continue":
                block_status = "DIAGNOSTIC_COMPLETE" if all_arms_accepted else "DIAGNOSTIC_INCOMPLETE"
            else:
                block_status = "ACCEPTED" if all_arms_accepted else "MUTUAL_ACCEPTANCE_FAILURE"

        tetrad_summary["block_status"] = block_status
        tetrad_summary["cumulative_global_tokens"] = cumulative_global_tokens
        tetrad_summary["global_budget_uncertain"] = is_budget_uncertain

        block_incomplete = not all_arms_accepted

        # Pairwise Comparative Analyses:
        runs = tetrad_summary["runs"]
        arm_a = runs.get("arm_a_native", {})
        arm_b = runs.get("arm_b_cand3_always_on", {})
        arm_c = runs.get("arm_c_cand4_always_on", {})
        arm_d = runs.get("arm_d_cand4_adaptive", {})

        tetrad_summary["comparative_analysis"] = {
            "cand4_adaptive_vs_native": self.compute_comparative_analysis(
                ctrl_run=arm_a, treat_run=arm_d, baseline_label="arm_a_native", treatment_label="arm_d_cand4_adaptive",
                block_incomplete=block_incomplete
            ),
            "cand4_always_on_vs_native": self.compute_comparative_analysis(
                ctrl_run=arm_a, treat_run=arm_c, baseline_label="arm_a_native", treatment_label="arm_c_cand4_always_on",
                block_incomplete=block_incomplete
            ),
            "cand3_always_on_vs_native": self.compute_comparative_analysis(
                ctrl_run=arm_a, treat_run=arm_b, baseline_label="arm_a_native", treatment_label="arm_b_cand3_always_on",
                block_incomplete=block_incomplete
            ),
            "cand4_vs_cand3": self.compute_comparative_analysis(
                ctrl_run=arm_b, treat_run=arm_c, baseline_label="arm_b_cand3_always_on", treatment_label="arm_c_cand4_always_on",
                block_incomplete=block_incomplete
            ),
            "adaptive_vs_always_on": self.compute_comparative_analysis(
                ctrl_run=arm_c, treat_run=arm_d, baseline_label="arm_c_cand4_always_on", treatment_label="arm_d_cand4_adaptive",
                block_incomplete=block_incomplete
            ),
        }

        # Write tetrad summary JSON
        summary_path = self.output_dir / "tetrad_summary.json"
        with open(summary_path, "w", encoding="utf-8") as f:
            json.dump(tetrad_summary, f, indent=2)

        # Write tetrad markdown report
        md_content = self.generate_tetrad_markdown_report(tetrad_summary)
        report_path = self.output_dir / "tetrad_report.md"
        with open(report_path, "w", encoding="utf-8") as f:
            f.write(md_content)

        return tetrad_summary

    @staticmethod
    def generate_tetrad_markdown_report(tetrad_summary: Dict[str, Any]) -> str:
        """Generates a comprehensive 4-arm matched benchmark report."""
        task_id = tetrad_summary.get("task_id", "")
        block_id = tetrad_summary.get("block_id", "")
        seed = tetrad_summary.get("seed", "")
        order = " -> ".join(tetrad_summary.get("order", []))
        model = tetrad_summary.get("model", "")
        reasoning = tetrad_summary.get("reasoning_effort", "")

        acceptance = tetrad_summary.get("acceptance", {})
        runs = tetrad_summary.get("runs", {})
        comp = tetrad_summary.get("comparative_analysis", {})

        execution_policy = tetrad_summary.get("execution_policy", "strict")
        block_status = tetrad_summary.get("block_status", "UNKNOWN")

        lines = [
            f"# Phase 5.4 4-Arm Benchmark Report: `{task_id}` (`{block_id}`)",
            "",
            f"- **Protocol:** Phase 5.4 4-Arm Evaluation Model",
            f"- **Execution Policy:** `{execution_policy}`",
            f"- **Block Status:** `{block_status}`",
            f"- **Model:** `{model}` (Reasoning: `{reasoning}`)",
            f"- **Execution Order:** {order} (Seed: `{seed}`)",
            f"- **Golden Dependency Fingerprint:** Verified Intact",
            "",
            "---",
        ]

        if block_status == "ABORTED_UNCERTAIN_BUDGET":
            lines.extend([
                "",
                "> [!CAUTION]",
                "> **Global Budget Integrity Alert: Execution Aborted Due to Uncertain Budget**",
                "> One or more arms produced missing, malformed, or unverified token telemetry.",
                "> To prevent global budget leakage, the orchestrator halted execution fail-closed.",
                "> All trailing arms were marked `SKIPPED_UNCERTAIN_BUDGET`.",
                ""
            ])

        lines.extend([
            "",
            "## 1. Acceptance & Correctness Gate",
            "",
            "| Arm | Binary Candidate | Status | Expected SHA-256 | Actual SHA-256 | Correctness Score ($C$) | Requirements Passed | Gate Verdict |",
            "| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |",
        ])

        arm_labels = [
            ("arm_a_native", "Native (None)", "NATIVE"),
            ("arm_b_cand3_always_on", "Candidate 3", "ALWAYS_ON"),
            ("arm_c_cand4_always_on", "Candidate 4", "ALWAYS_ON"),
            ("arm_d_cand4_adaptive", "Candidate 4", "ADAPTIVE"),
        ]

        for arm_key, candidate_lbl, status_lbl in arm_labels:
            arm_data = runs.get(arm_key, {})
            iso = arm_data.get("isolation", {})
            res_cand = iso.get("resolved_candidate_version")
            candidate_map = {
                "native": "Native (None)",
                "cand3": "Candidate 3",
                "cand4": "Candidate 4",
                "cand4_1": "Candidate 4.1",
                "cand4_2": "Candidate 4.2",
            }
            display_candidate = candidate_map.get(res_cand, candidate_lbl)
            exp_sha = (iso.get("engram_binary_expected_sha256") or "N/A")[:8]
            act_sha = (iso.get("engram_binary_actual_sha256") or "N/A")[:8]
            corr = arm_data.get("correctness", {})
            score = corr.get("score")
            passed = corr.get("passed_all_requirements")
            arm_status = arm_data.get("execution_status", "")
            arm_accepted = acceptance.get(f"{arm_key}_accepted", False)

            if score is not None:
                score_str = f"{score:.2f}"
            elif arm_status:
                score_str = arm_status
            else:
                score_str = "SKIPPED"

            passed_str = str(passed) if passed is not None else "NOT_EVALUATED"

            if "SKIPPED" in arm_status or "DEFERRED" in arm_status:
                verdict = arm_status
            elif arm_accepted:
                verdict = "PASS"
            else:
                verdict = "FAIL"

            lines.append(f"| **{arm_key}** | {display_candidate} | {status_lbl} | `{exp_sha}` | `{act_sha}` | {score_str} | {passed_str} | {verdict} |")

        lines.extend([
            "",
            "---",
            "",
            "## 2. Comparative Analysis & Efficiency Claims",
            "",
            f"### A. Candidate 4 Adaptive vs Native: `{comp.get('cand4_adaptive_vs_native', {}).get('verdict', 'UNKNOWN')}`",
            f"- {comp.get('cand4_adaptive_vs_native', {}).get('explanation', '')}",
            "",
            f"### B. Candidate 4 Always-On vs Native: `{comp.get('cand4_always_on_vs_native', {}).get('verdict', 'UNKNOWN')}`",
            f"- {comp.get('cand4_always_on_vs_native', {}).get('explanation', '')}",
            "",
            f"### C. Candidate 3 Always-On vs Native: `{comp.get('cand3_always_on_vs_native', {}).get('verdict', 'UNKNOWN')}`",
            f"- {comp.get('cand3_always_on_vs_native', {}).get('explanation', '')}",
            "",
            f"### D. Candidate 4 vs Candidate 3 (Engine Bounding Impact): `{comp.get('cand4_vs_cand3', {}).get('verdict', 'UNKNOWN')}`",
            f"- {comp.get('cand4_vs_cand3', {}).get('explanation', '')}",
            "",
            f"### E. Adaptive vs Always-On (Routing Optimization): `{comp.get('adaptive_vs_always_on', {}).get('verdict', 'UNKNOWN')}`",
            f"- {comp.get('adaptive_vs_always_on', {}).get('explanation', '')}",
            "",
            "---",
            "*Report generated deterministically by Engram Phase 5.4 4-Arm Benchmark Harness.*",
            ""
        ])

        return "\n".join(lines)

    def run_pentad(
        self,
        block_id: Optional[str] = None,
        arm_order: Optional[List[str]] = None
    ) -> Dict[str, Any]:
        """
        Executes a 5-arm matched pentad block (Phase 5.8.1 Architecture) across:
        1. arm_a_native: Native baseline without Engram
        2. arm_b_cand3: Candidate 3 Always-On (v1.5.1 baseline, flat context)
        3. arm_c_cand42_always_on: Candidate 4.2 Always-On (v1.5.4 engine without adaptive routing)
        4. arm_d_cand42_adaptive: Candidate 4.2 Adaptive (v1.5.4 engine with adaptive complexity gating)
        5. arm_e_cand5_optimized: Candidate 5 Optimized (v1.5.4 engine with 7 Token-Efficiency Optimizations)
        """
        effective_block_id = block_id or f"pentad_{self.pair_id}"
        all_pentad_arms = list(Orchestrator.ALL_PENTAD_ARMS)

        # Phase 5.9.1.1 Mandatory Safety Guard:
        # All pentad benchmark arms must execute via ControlledAgentRuntime and ProviderTransport.
        # Legacy Codex subprocess dispatch is strictly forbidden in live pentad runs.
        if not self.is_dry_run and self.provider == "cloud":
            raise SafetyViolationError(
                "FATAL: Pentad benchmark execution requires a controlled provider transport "
                "(provider in ['controlled', 'openai', 'mock']). Legacy 'cloud' subprocess dispatch "
                "is strictly forbidden in live pentad runs."
            )

        if arm_order is not None:
            order = Orchestrator.validate_pentad_arm_order(arm_order, allow_none=False)
        else:
            rng = random.Random(self.seed)
            order = list(all_pentad_arms)
            rng.shuffle(order)

        pentad_summary: Dict[str, Any] = {
            "schema_version": "2.1.0",
            "block_id": effective_block_id,
            "task_id": self.task_id,
            "seed": self.seed,
            "order": order,
            "model": self.model,
            "reasoning_effort": self.reasoning_effort,
            "golden_dependency_fingerprint_verified": True,
            "execution_policy": self.execution_policy,
            "block_status": "RUNNING",
            "global_tokens_ceiling": self.global_tokens_ceiling,
            "cumulative_global_tokens": 0,
            "acceptance": {},
            "comparative_analysis": {},
            "runs": {}
        }

        cumulative_global_tokens = 0
        block_status = "COMPLETED"
        is_budget_uncertain = False

        for exec_order, arm in enumerate(order, start=1):
            if self.global_tokens_ceiling is not None:
                global_remaining = self.global_tokens_ceiling - cumulative_global_tokens
                if global_remaining < self.headroom_reserve:
                    print(
                        f"\n[ORCHESTRATOR GLOBAL-BUDGET HALT] Global remaining tokens ({global_remaining:,}) "
                        f"below headroom reserve ({self.headroom_reserve:,}). Halting pentad."
                    )
                    block_status = "ABORTED_GLOBAL_BUDGET"
                    for rem_idx in range(exec_order - 1, len(order)):
                        rem_arm = order[rem_idx]
                        skip_reason = (
                            f"Execution halted: global token budget headroom reserve exhausted "
                            f"(remaining {global_remaining:,} < reserve {self.headroom_reserve:,})"
                        )
                        pentad_summary["runs"][rem_arm] = self._create_skipped_arm_record(
                            rem_arm, rem_idx + 1, skip_reason, execution_status="SKIPPED_GLOBAL_BUDGET"
                        )
                    break

                effective_arm_ceiling = min(self.max_tokens_ceiling, global_remaining)
            else:
                effective_arm_ceiling = self.max_tokens_ceiling

            try:
                run_data = self._execute_single_arm(
                    arm=arm,
                    exec_order=exec_order,
                    arm_max_tokens_ceiling=effective_arm_ceiling
                )
                pentad_summary["runs"][arm] = run_data

                arm_telem = run_data.get("telemetry", {})
                arm_status = run_data.get("execution_status", "SUCCESS")

                tel_valid, tel_reason, tel_data = Orchestrator.validate_telemetry_integrity(arm_telem)

                if effective_arm_ceiling is not None and tel_valid and tel_data:
                    raw_spent = tel_data.get("total_model_tokens", 0)
                    if raw_spent > effective_arm_ceiling and arm_status == "SUCCESS":
                        print(
                            f"\n[ORCHESTRATOR SAFETY VIOLATION] Arm '{arm}' consumed {raw_spent:,} tokens, "
                            f"exceeding ceiling ({effective_arm_ceiling:,}). Overriding status to TOKEN_CEILING_BREACHED."
                        )
                        arm_status = "TOKEN_CEILING_BREACHED"
                        run_data["execution_status"] = arm_status
                        run_data["stop_reason"] = "TOKEN_CEILING_BREACHED"
                        run_data["token_ceiling_breached"] = True

                if not tel_valid:
                    is_budget_uncertain = True

                if tel_valid and tel_data:
                    spent = tel_data.get("total_model_tokens", 0)
                else:
                    spent = arm_telem.get("total_model_tokens", 0)

                cumulative_global_tokens += spent

            except Exception as e:
                print(f"\n[ORCHESTRATOR ERROR] Execution failed for arm {arm}: {e}")
                err_run = self._create_skipped_arm_record(
                    arm, exec_order, f"Execution failed with exception: {e}", execution_status="EXECUTION_ERROR"
                )
                pentad_summary["runs"][arm] = err_run
                arm_status = "EXECUTION_ERROR"

            is_accepted = Orchestrator.is_run_accepted(pentad_summary["runs"].get(arm, {}))
            is_failed = not is_accepted or arm_status in (
                "FAILED", "TIMEOUT", "EXECUTION_ERROR", "TOKEN_CEILING_BREACHED",
                "PRE_DISPATCH_HEADROOM_EXHAUSTED", "TELEMETRY_INVALID", "PROCESS_CRASH"
            )

            if is_budget_uncertain and self.global_tokens_ceiling is not None:
                block_status = "ABORTED_UNCERTAIN_BUDGET"
                for rem_idx in range(exec_order, len(order)):
                    rem_arm = order[rem_idx]
                    skip_reason = f"Execution halted: preceding arm '{arm}' produced invalid or unverified telemetry"
                    pentad_summary["runs"][rem_arm] = self._create_skipped_arm_record(
                        rem_arm, rem_idx + 1, skip_reason, execution_status="SKIPPED_UNCERTAIN_BUDGET"
                    )
                break
            elif self.stop_on_failure and is_failed:
                block_status = f"HALTED_ON_FAILURE_{arm.upper()}"
                for rem_idx in range(exec_order, len(order)):
                    rem_arm = order[rem_idx]
                    skip_reason = f"Execution halted after failure in prior arm '{arm}' (status: {arm_status}, accepted: {is_accepted})"
                    pentad_summary["runs"][rem_arm] = self._create_skipped_arm_record(
                        rem_arm, rem_idx + 1, skip_reason, execution_status="SKIPPED"
                    )
                break
            elif self.execution_policy == "diagnostic_continue" and is_failed:
                print(
                    f"\n[ORCHESTRATOR DIAGNOSTIC-CONTINUE] Arm {arm} failed ({arm_status}, accepted={is_accepted}). "
                    f"Diagnostic continuation active; continuing to subsequent arms."
                )

        for arm in all_pentad_arms:
            if arm not in pentad_summary["runs"]:
                deferred_order = len(order) + 1
                skip_reason = f"Execution deferred (not in execution order {order})"
                pentad_summary["runs"][arm] = self._create_skipped_arm_record(
                    arm, deferred_order, skip_reason, execution_status="DEFERRED"
                )

        def _is_accepted(run_dict: Dict[str, Any]) -> bool:
            return Orchestrator.is_run_accepted(run_dict)

        for arm in all_pentad_arms:
            pentad_summary["acceptance"][f"{arm}_accepted"] = _is_accepted(pentad_summary["runs"].get(arm, {}))

        all_arms_accepted = all(pentad_summary["acceptance"].values())
        pentad_summary["acceptance"]["all_arms_accepted"] = all_arms_accepted

        if block_status == "COMPLETED":
            if self.execution_policy == "diagnostic_continue":
                block_status = "DIAGNOSTIC_COMPLETE" if all_arms_accepted else "DIAGNOSTIC_INCOMPLETE"
            else:
                block_status = "ACCEPTED" if all_arms_accepted else "MUTUAL_ACCEPTANCE_FAILURE"

        pentad_summary["block_status"] = block_status
        pentad_summary["cumulative_global_tokens"] = cumulative_global_tokens
        pentad_summary["global_budget_uncertain"] = is_budget_uncertain

        block_incomplete = not all_arms_accepted

        runs = pentad_summary["runs"]
        arm_a = runs.get("arm_a_native", {})
        arm_b = runs.get("arm_b_cand3", {})
        arm_c = runs.get("arm_c_cand42_always_on", {})
        arm_d = runs.get("arm_d_cand42_adaptive", {})
        arm_e = runs.get("arm_e_cand5_optimized", {})

        pentad_summary["comparative_analysis"] = {
            "cand5_optimized_vs_native": self.compute_comparative_analysis(
                ctrl_run=arm_a, treat_run=arm_e, baseline_label="arm_a_native", treatment_label="arm_e_cand5_optimized",
                block_incomplete=block_incomplete
            ),
            "cand5_optimized_vs_cand42_adaptive": self.compute_comparative_analysis(
                ctrl_run=arm_d, treat_run=arm_e, baseline_label="arm_d_cand42_adaptive", treatment_label="arm_e_cand5_optimized",
                block_incomplete=block_incomplete
            ),
            "cand5_optimized_vs_cand42_always_on": self.compute_comparative_analysis(
                ctrl_run=arm_c, treat_run=arm_e, baseline_label="arm_c_cand42_always_on", treatment_label="arm_e_cand5_optimized",
                block_incomplete=block_incomplete
            ),
            "cand5_optimized_vs_cand3": self.compute_comparative_analysis(
                ctrl_run=arm_b, treat_run=arm_e, baseline_label="arm_b_cand3", treatment_label="arm_e_cand5_optimized",
                block_incomplete=block_incomplete
            ),
            "cand42_adaptive_vs_native": self.compute_comparative_analysis(
                ctrl_run=arm_a, treat_run=arm_d, baseline_label="arm_a_native", treatment_label="arm_d_cand42_adaptive",
                block_incomplete=block_incomplete
            ),
            "cand42_always_on_vs_native": self.compute_comparative_analysis(
                ctrl_run=arm_a, treat_run=arm_c, baseline_label="arm_a_native", treatment_label="arm_c_cand42_always_on",
                block_incomplete=block_incomplete
            ),
            "cand3_vs_native": self.compute_comparative_analysis(
                ctrl_run=arm_a, treat_run=arm_b, baseline_label="arm_a_native", treatment_label="arm_b_cand3",
                block_incomplete=block_incomplete
            ),
        }

        # Write pentad summary JSON
        summary_path = self.output_dir / "pentad_summary.json"
        with open(summary_path, "w", encoding="utf-8") as f:
            json.dump(pentad_summary, f, indent=2)

        # Write pentad markdown report
        md_content = self.generate_pentad_markdown_report(pentad_summary)
        report_path = self.output_dir / "pentad_report.md"
        with open(report_path, "w", encoding="utf-8") as f:
            f.write(md_content)

        return pentad_summary

    @staticmethod
    def generate_pentad_markdown_report(pentad_summary: Dict[str, Any]) -> str:
        """Generates a comprehensive 5-arm matched benchmark report."""
        task_id = pentad_summary.get("task_id", "")
        block_id = pentad_summary.get("block_id", "")
        seed = pentad_summary.get("seed", "")
        order = " -> ".join(pentad_summary.get("order", []))
        model = pentad_summary.get("model", "")
        reasoning = pentad_summary.get("reasoning_effort", "")

        acceptance = pentad_summary.get("acceptance", {})
        runs = pentad_summary.get("runs", {})
        comp = pentad_summary.get("comparative_analysis", {})

        execution_policy = pentad_summary.get("execution_policy", "strict")
        block_status = pentad_summary.get("block_status", "UNKNOWN")

        lines = [
            f"# Phase 5.8.1 5-Arm Benchmark Report: `{task_id}` (`{block_id}`)",
            "",
            f"- **Protocol:** Phase 5.8.1 5-Arm Pentad Evaluation Model",
            f"- **Execution Policy:** `{execution_policy}`",
            f"- **Block Status:** `{block_status}`",
            f"- **Model:** `{model}` (Reasoning: `{reasoning}`)",
            f"- **Execution Order:** {order} (Seed: `{seed}`)",
            f"- **Golden Dependency Fingerprint:** Verified Intact",
            "",
            "---",
        ]

        if block_status == "ABORTED_UNCERTAIN_BUDGET":
            lines.extend([
                "",
                "> [!CAUTION]",
                "> **Global Budget Integrity Alert: Execution Aborted Due to Uncertain Budget**",
                "> One or more arms produced missing, malformed, or unverified token telemetry.",
                "> To prevent global budget leakage, the orchestrator halted execution fail-closed.",
                "> All trailing arms were marked `SKIPPED_UNCERTAIN_BUDGET`.",
                ""
            ])

        lines.extend([
            "",
            "## 1. Acceptance & Correctness Gate",
            "",
            "| Arm | Binary Candidate | Status | Expected SHA-256 | Actual SHA-256 | Correctness Score ($C$) | Requirements Passed | Gate Verdict |",
            "| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |",
        ])

        arm_labels = [
            ("arm_a_native", "Native (None)", "NATIVE"),
            ("arm_b_cand3", "Candidate 3", "ALWAYS_ON"),
            ("arm_c_cand42_always_on", "Candidate 4.2", "ALWAYS_ON"),
            ("arm_d_cand42_adaptive", "Candidate 4.2", "ADAPTIVE"),
            ("arm_e_cand5_optimized", "Candidate 5", "OPTIMIZED_ADAPTIVE"),
        ]

        for arm_key, candidate_lbl, status_lbl in arm_labels:
            run_dict = runs.get(arm_key, {})
            iso = run_dict.get("isolation", {})
            corr = run_dict.get("correctness", {})
            reqs = corr.get("requirements", {})

            exp_sha = iso.get("engram_binary_expected_sha256") or "NONE"
            act_sha = iso.get("engram_binary_actual_sha256") or "NONE"
            score = corr.get("score", 0.0)
            req_str = f"{sum(1 for v in reqs.values() if v == 'PASSED')}/{len(reqs)}" if reqs else "N/A"
            gate_verdict = "ACCEPTED" if acceptance.get(f"{arm_key}_accepted", False) else "REJECTED"

            lines.append(
                f"| `{arm_key}` | {candidate_lbl} | `{status_lbl}` | `{exp_sha[:10]}...` | `{act_sha[:10]}...` | `{score}` | `{req_str}` | **`{gate_verdict}`** |"
            )

        lines.extend([
            "",
            "## 2. Comparative Analysis Matrix",
            "",
            f"### A. Candidate 5 Optimized vs Native Baseline: `{comp.get('cand5_optimized_vs_native', {}).get('verdict', 'UNKNOWN')}`",
            f"- {comp.get('cand5_optimized_vs_native', {}).get('explanation', '')}",
            "",
            f"### B. Candidate 5 Optimized vs Candidate 4.2 Adaptive: `{comp.get('cand5_optimized_vs_cand42_adaptive', {}).get('verdict', 'UNKNOWN')}`",
            f"- {comp.get('cand5_optimized_vs_cand42_adaptive', {}).get('explanation', '')}",
            "",
            f"### C. Candidate 5 Optimized vs Candidate 4.2 Always-On: `{comp.get('cand5_optimized_vs_cand42_always_on', {}).get('verdict', 'UNKNOWN')}`",
            f"- {comp.get('cand5_optimized_vs_cand42_always_on', {}).get('explanation', '')}",
            "",
            f"### D. Candidate 5 Optimized vs Candidate 3: `{comp.get('cand5_optimized_vs_cand3', {}).get('verdict', 'UNKNOWN')}`",
            f"- {comp.get('cand5_optimized_vs_cand3', {}).get('explanation', '')}",
            "",
            "---",
            "*Report generated deterministically by Engram Phase 5.8.1 5-Arm Pentad Benchmark Harness.*",
            ""
        ])

        return "\n".join(lines)

    def run_single(self, arm: str) -> Dict[str, Any]:
        """
        Executes a strictly isolated single arm (Remediation B for smoke tests).
        Executes exactly one requested arm without invoking control or companion arms.
        """
        allowed_arms = {
            "control_A", "treatment_B", "baseline_arm1", "v151_arm2", "v154_arm3",
            "arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"
        }
        if arm not in allowed_arms:
            raise ValueError(f"Invalid arm '{arm}' for single mode. Allowed arms: {sorted(allowed_arms)}")

        run_data = self._execute_single_arm(arm=arm, exec_order=1)
        arm_accepted = Orchestrator.is_run_accepted(run_data)

        single_summary: Dict[str, Any] = {
            "schema_version": "2.0.0",
            "mode": "single",
            "block_id": self.pair_id,
            "task_id": self.task_id,
            "arm": arm,
            "seed": self.seed,
            "model": self.model,
            "reasoning_effort": self.reasoning_effort,
            "golden_dependency_fingerprint_verified": True,
            "acceptance": {
                f"{arm}_accepted": arm_accepted,
                "all_arms_accepted": arm_accepted
            },
            "run": run_data
        }

        # Write single summary JSON
        summary_out = self.output_dir / "single_summary.json"
        with open(summary_out, "w", encoding="utf-8") as f:
            json.dump(single_summary, f, indent=2)

        # Write single markdown report
        report_out = self.output_dir / "single_report.md"
        with open(report_out, "w", encoding="utf-8") as f:
            f.write(self._generate_single_report(single_summary))

        self._print_single_verdict(single_summary)
        return single_summary

    def _generate_single_report(self, single_summary: Dict[str, Any]) -> str:
        """Generates a markdown report for a single-arm execution."""
        arm = single_summary.get("arm", "")
        run = single_summary.get("run", {})
        telem = run.get("telemetry", {})
        corr = run.get("correctness", {})
        accepted = single_summary.get("acceptance", {}).get(f"{arm}_accepted", False)

        lines = [
            f"# Single-Arm Benchmark Report: {self.task_id} [{arm}]",
            "",
            f"- **Task ID:** `{self.task_id}`",
            f"- **Arm:** `{arm}`",
            f"- **Execution Status:** `{run.get('execution_status', 'UNKNOWN')}`",
            f"- **Accepted:** `{'YES' if accepted else 'NO'}`",
            f"- **Total Model Tokens:** `{telem.get('total_model_tokens', 0):,}`",
            f"- **Correctness Score:** `{corr.get('score', 0.0)}`",
            "",
            "## Requirements Status",
            ""
        ]
        for req_id, status in corr.get("requirements", {}).items():
            lines.append(f"- **{req_id}:** `{status}`")
        lines.extend([
            "",
            "---",
            "*Report generated deterministically by Engram Benchmark Orchestrator (Single-Arm Mode).*",
            ""
        ])
        return "\n".join(lines)

    @staticmethod
    def _print_single_verdict(single_summary: Dict[str, Any]) -> None:
        arm = single_summary.get("arm", "")
        accepted = single_summary.get("acceptance", {}).get(f"{arm}_accepted", False)
        run = single_summary.get("run", {})
        status = run.get("execution_status", "UNKNOWN")
        print("\n=======================================================")
        print(f"Single-Arm Result: [{arm}] Status: {status}")
        print(f"  - Accepted: {accepted}")
        print(f"  - Score:    {run.get('correctness', {}).get('score', 0.0)}")
        print("=======================================================\n")


def main():
    parser = argparse.ArgumentParser(description="Reproducible Matched-Pair, Triad, Tetrad, and Pentad Benchmark Orchestrator")
    parser.add_argument("--mode", choices=["pair", "triad", "single", "tetrad", "pentad"], default="pair", help="Execution mode (pair, triad, single, tetrad, or pentad)")
    parser.add_argument("--task", default="task_a_typo", help="Task ID or path to task JSON")
    parser.add_argument("--pair", default="pair_01", help="Matched pair identifier (e.g. pair_01)")
    parser.add_argument("--block", default=None, help="Triad block ID (e.g. block_01 .. block_10)")
    parser.add_argument("--arm-order", "--arms", dest="arm_order", default=None, help="Comma-separated arm order (e.g. v154_arm3 for single, or v154_arm3,baseline_arm1 for triad)")
    parser.add_argument("--provider", choices=["controlled", "openai", "mock", "cloud", "ollama", "lmstudio"], default="controlled", help="Execution provider: controlled, openai, mock, cloud, ollama, or lmstudio (default: controlled)")
    parser.add_argument("--model", default="gpt-5.6-luna", help="Explicit model identifier")
    parser.add_argument("--reasoning", default="medium", help="Explicit reasoning effort (low/medium/high/xhigh/max)")
    parser.add_argument("--dry-run", action="store_true", help="Simulate execution without model API consumption")
    parser.add_argument("--candidate-version", choices=["native", "cand3", "cand4", "cand4_1", "cand4_2", "cand5"], default=None, help="Explicit candidate version selection (fail-closed: native, cand3, cand4, cand4_1, cand4_2, cand5)")
    parser.add_argument("--dependency-strategy", choices=["copy", "junction", "none"], default="copy", help="Dependency isolation mode (default: copy for full write isolation)")
    parser.add_argument("--preserve-sandboxes", action="store_true", help="Preserve sandbox folders")
    parser.add_argument("--seed", type=int, default=None, help="Randomization seed")
    parser.add_argument("--expected-treatment-sha256", default=None, help="Explicit expected SHA-256 for treatment Engram binary")
    parser.add_argument("--engram-v151-bin", default=None, help="Path to Engram v1.5.1 executable")
    parser.add_argument("--engram-v154-bin", default=None, help="Path to Engram v1.5.4 executable")
    parser.add_argument("--expected-v154-sha256", default=None, help="Explicit expected SHA-256 for candidate Engram binary")
    parser.add_argument("--max-tokens-ceiling", type=int, default=250_000, help="Per-arm token ceiling limit (default: 250,000)")
    parser.add_argument("--headroom-reserve", type=int, default=45_000, help="Pre-dispatch token headroom reserve in tokens (default: 45,000)")
    parser.add_argument("--golden-repo", "--source-repo", dest="golden_repo", default=None, help="Explicit path to source/golden repository")
    parser.add_argument("--output-dir", default=None, help="Explicit target directory for benchmark output reports")
    parser.add_argument("--no-stop-on-failure", action="store_true", help="Deprecated legacy alias for --execution-policy diagnostic_continue")
    parser.add_argument("--execution-policy", choices=["strict", "diagnostic_continue"], default=None, help="Benchmark execution policy: 'strict' (default, stop on any failure) or 'diagnostic_continue' (continue subsequent arms for diagnosis)")
    parser.add_argument("--global-tokens-ceiling", type=int, default=None, help="Global token ceiling across all arms in the block (default: None, unbounded block ceiling)")
    args = parser.parse_args()

    effective_policy, stop_on_failure = resolve_execution_policy(
        execution_policy=args.execution_policy,
        no_stop_on_failure=args.no_stop_on_failure,
        parser=parser,
    )

    if args.mode == "single":
        mode = "single"
    elif args.mode == "pentad":
        mode = "pentad"
    elif args.mode == "tetrad":
        mode = "tetrad"
    elif args.block or args.mode == "triad":
        mode = "triad"
    else:
        mode = "pair"

    task_name = args.task
    if mode == "triad" and args.block in TRIAD_BLOCK_SCHEDULES and args.task == "task_a_typo":
        task_name = TRIAD_BLOCK_SCHEDULES[args.block]["task"]

    task_path = Path(task_name)
    if not task_path.exists():
        task_path = HARNESS_ROOT / "config" / "tasks" / f"{task_name}.json"
    if not task_path.exists():
        print(f"Error: Task configuration not found: {task_name}")
        sys.exit(1)

    if mode == "pentad":
        if args.arm_order is not None:
            if not args.arm_order.strip():
                print("Error: Explicit --arm-order cannot be empty for pentad mode.")
                sys.exit(1)
            try:
                arm_order = Orchestrator.validate_pentad_arm_order(args.arm_order, allow_none=False)
            except SafetyViolationError as e:
                print(f"Error: Invalid pentad arm order: {e}")
                sys.exit(1)
        else:
            arm_order = None
    elif mode == "tetrad":
        if args.arm_order is not None:
            if not args.arm_order.strip():
                print("Error: Explicit --arm-order cannot be empty for tetrad mode.")
                sys.exit(1)
            try:
                arm_order = Orchestrator.validate_tetrad_arm_order(args.arm_order, allow_none=False)
            except SafetyViolationError as e:
                print(f"Error: Invalid tetrad arm order: {e}")
                sys.exit(1)
        else:
            arm_order = None
    elif mode == "single":
        arm_order = [a.strip() for a in args.arm_order.split(",")] if args.arm_order else None
        if not arm_order or len(arm_order) == 0:
            print("Error: Single mode requires exactly one arm specified via --arms (e.g. --arms v154_arm3). Received zero arms.")
            sys.exit(1)
        if len(arm_order) > 1:
            print(f"Error: Single mode requires exactly one arm specified via --arms. Received multiple arms: {arm_order}")
            sys.exit(1)
        allowed_arms = {
            "control_A", "treatment_B", "baseline_arm1", "v151_arm2", "v154_arm3",
            "arm_a_native", "arm_b_cand3_always_on", "arm_c_cand4_always_on", "arm_d_cand4_adaptive"
        }
        target_arm = arm_order[0]
        if target_arm not in allowed_arms:
            print(f"Error: Invalid arm identifier for single mode: '{target_arm}'. Allowed arms: {sorted(allowed_arms)}")
            sys.exit(1)
    else:
        arm_order = [a.strip() for a in args.arm_order.split(",")] if args.arm_order else None

    orchestrator = Orchestrator(
        task_config_path=task_path,
        pair_id=args.block or args.pair,
        model=args.model,
        provider=args.provider,
        reasoning_effort=args.reasoning,
        output_dir=Path(args.output_dir) if args.output_dir else None,
        is_dry_run=args.dry_run,
        dependency_strategy=args.dependency_strategy,
        preserve_sandboxes=args.preserve_sandboxes,
        seed=args.seed,
        expected_treatment_sha256=args.expected_treatment_sha256,
        engram_v151_bin=Path(args.engram_v151_bin) if args.engram_v151_bin else None,
        engram_v154_bin=Path(args.engram_v154_bin) if args.engram_v154_bin else None,
        expected_v154_sha256=args.expected_v154_sha256,
        stop_on_failure=stop_on_failure,
        max_tokens_ceiling=args.max_tokens_ceiling,
        headroom_reserve=args.headroom_reserve,
        golden_repo=Path(args.golden_repo) if args.golden_repo else None,
        candidate_version=args.candidate_version,
        execution_policy=effective_policy,
        global_tokens_ceiling=args.global_tokens_ceiling,
    )

    if mode == "single":
        results = orchestrator.run_single(arm=arm_order[0])
        print("\nSingle-Arm Execution Finished!")
        print(f"Summary saved to: {orchestrator.output_dir / 'single_summary.json'}")
        print(f"Report saved to:  {orchestrator.output_dir / 'single_report.md'}")
    elif mode == "triad":
        results = orchestrator.run_triad(block_id=args.block, arm_order=arm_order)
        print("\nStage 1 Core Triad Execution Finished!")
        print(f"Summary saved to: {orchestrator.output_dir / 'triad_summary.json'}")
        print(f"Report saved to:  {orchestrator.output_dir / 'triad_report.md'}")
    elif mode == "pentad":
        results = orchestrator.run_pentad(block_id=args.block, arm_order=arm_order)
        print("\nPhase 5.8.1 5-Arm Pentad Execution Finished!")
        print(f"Summary saved to: {orchestrator.output_dir / 'pentad_summary.json'}")
        print(f"Report saved to:  {orchestrator.output_dir / 'pentad_report.md'}")
    elif mode == "tetrad":
        results = orchestrator.run_tetrad(block_id=args.block, arm_order=arm_order)
        print("\nPhase 5.4 4-Arm Tetrad Execution Finished!")
        print(f"Summary saved to: {orchestrator.output_dir / 'tetrad_summary.json'}")
        print(f"Report saved to:  {orchestrator.output_dir / 'tetrad_report.md'}")
    else:
        results = orchestrator.run_pair(arm_order=arm_order)
        print("\nMatched Pair Execution Finished!")
        print(f"Summary saved to: {orchestrator.output_dir / 'matched_pair_summary.json'}")


if __name__ == "__main__":
    main()
