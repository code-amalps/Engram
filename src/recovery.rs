//! Recovery State Machine, Planner, and Validation Loop (Engram v1.1.0 Phase 3):
//! Manages persistent multi-turn recovery sessions at `.engram/cache/recovery/<contract-id>.json`,
//! formulates structured repair plans without modifying source code, and enforces a strict
//! 2-attempt exhaustion gate that blocks on the 3rd attempt awaiting human intervention.

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

use crate::contract::{self, ChangeContract, ContractStatus, FileValidationCategory};
use crate::model::RuleSeverity;
use crate::storage::root_path;

static SESSION_MUTEX: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LockState {
    Unlocked,
    Locked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecoveryStatus {
    InProgress,
    Resolved,
    RecoveryExhausted,
    BlockedAwaitingHuman,
}

impl RecoveryStatus {
    pub fn is_exhausted(&self) -> bool {
        matches!(
            self,
            RecoveryStatus::RecoveryExhausted | RecoveryStatus::BlockedAwaitingHuman
        )
    }

    pub fn is_resolved(&self) -> bool {
        matches!(self, RecoveryStatus::Resolved)
    }

    pub fn is_in_progress(&self) -> bool {
        matches!(self, RecoveryStatus::InProgress)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RecoveryViolationDetails {
    pub boundary_violations: Vec<String>,
    pub unexpected_files: Vec<String>,
    pub constitution_violations: Vec<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryActionType {
    RevertMutation,
    PreserveRequestedChange,
    Rerun,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryAction {
    pub action_type: RecoveryActionType,
    pub target: String,
    pub description: String,
}

impl RecoveryAction {
    pub fn revert_mutation(path: impl Into<String>) -> Self {
        let target = path.into();
        Self {
            action_type: RecoveryActionType::RevertMutation,
            description: format!("Revert unauthorized mutation at `{}`", target),
            target,
        }
    }

    pub fn preserve_requested_change(scope: impl Into<String>) -> Self {
        let target = scope.into();
        Self {
            action_type: RecoveryActionType::PreserveRequestedChange,
            description: format!("Preserve authorized change within `{}`", target),
            target,
        }
    }

    pub fn rerun(checks: impl Into<String>) -> Self {
        let target = checks.into();
        Self {
            action_type: RecoveryActionType::Rerun,
            description: format!("Rerun checks: `{}`", target),
            target,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecoveryPlan {
    pub plan_id: String,
    pub contract_id: String,
    pub attempt: u32,
    pub actions: Vec<RecoveryAction>,
    pub instructions: Vec<String>,
    pub summary: String,
}

fn default_max_attempts() -> u32 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoverySession {
    pub contract_id: String,
    pub session_id: String,
    pub session_version: u64,
    pub lock_state: LockState,
    pub attempt_count: u32,
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
    pub status: RecoveryStatus,
    pub last_violation: Option<RecoveryViolationDetails>,
    pub last_recovery_plan: Option<RecoveryPlan>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl RecoverySession {
    pub fn is_exhausted(&self) -> bool {
        self.status.is_exhausted()
    }

    pub fn is_resolved(&self) -> bool {
        self.status.is_resolved()
    }

    pub fn is_in_progress(&self) -> bool {
        self.status.is_in_progress()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryReport {
    pub contract_id: String,
    pub session_id: String,
    pub session_version: u64,
    pub attempt_count: u32,
    pub max_attempts: u32,
    pub status: RecoveryStatus,
    pub plan: RecoveryPlan,
    pub summary: String,
}

pub fn recovery_cache_dir(root: &Path) -> PathBuf {
    root.join(".engram").join("cache").join("recovery")
}

pub fn recovery_session_path(root: &Path, contract_id: &str) -> PathBuf {
    recovery_cache_dir(root).join(format!("{}.json", contract_id))
}

pub fn create_recovery_session(contract_id: &str) -> RecoverySession {
    RecoverySession {
        contract_id: contract_id.to_string(),
        session_id: format!("rec_{}", Uuid::new_v4().simple()),
        session_version: 0,
        lock_state: LockState::Unlocked,
        attempt_count: 0,
        max_attempts: 2,
        status: RecoveryStatus::InProgress,
        last_violation: None,
        last_recovery_plan: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

pub fn lock_file_path(root: &Path, contract_id: &str) -> PathBuf {
    recovery_cache_dir(root).join(format!("{}.lock", contract_id))
}

pub struct FileLockGuard {
    lock_path: PathBuf,
}

impl Drop for FileLockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

pub fn acquire_file_lock(root: &Path, contract_id: &str) -> Result<FileLockGuard> {
    let dir = recovery_cache_dir(root);
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let lock_path = lock_file_path(root, contract_id);
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(500);

    loop {
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(_) => return Ok(FileLockGuard { lock_path }),
            Err(_) => {
                let is_stale = fs::metadata(&lock_path)
                    .and_then(|m| m.modified())
                    .map(|t| t.elapsed().map(|d| d > std::time::Duration::from_secs(5)).unwrap_or(false))
                    .unwrap_or(false);
                if is_stale {
                    let _ = fs::remove_file(&lock_path);
                }
                if start.elapsed() >= timeout {
                    return Err(anyhow!(
                        "CONCURRENCY_LOCK_ACQUISITION_FAILED: File lock contention for contract '{}'",
                        contract_id
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
}

pub fn load_session(root: &Path, contract_id: &str) -> Result<Option<RecoverySession>> {
    let _guard = SESSION_MUTEX.lock().unwrap();
    let _file_guard = acquire_file_lock(root, contract_id)?;
    load_session_internal(root, contract_id)
}

fn load_session_internal(root: &Path, contract_id: &str) -> Result<Option<RecoverySession>> {
    let path = recovery_session_path(root, contract_id);
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)?;
    let session: RecoverySession = serde_json::from_str(&content)?;
    Ok(Some(session))
}

pub fn get_recovery_session(root: &Path, contract_id: &str) -> Result<Option<RecoverySession>> {
    load_session(root, contract_id)
}

pub fn save_session(root: &Path, session: &RecoverySession) -> Result<()> {
    let _guard = SESSION_MUTEX.lock().unwrap();
    let _file_guard = acquire_file_lock(root, &session.contract_id)?;
    save_session_internal(root, session)
}

fn save_session_internal(root: &Path, session: &RecoverySession) -> Result<()> {
    let dir = recovery_cache_dir(root);
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    let path = recovery_session_path(root, &session.contract_id);
    let json = serde_json::to_string_pretty(session)?;
    let tmp_path = dir.join(format!("{}.{}.tmp", session.contract_id, Uuid::new_v4().simple()));
    fs::write(&tmp_path, &json)?;
    if fs::rename(&tmp_path, &path).is_err() {
        let _ = fs::remove_file(&path);
        if fs::rename(&tmp_path, &path).is_err() {
            fs::write(&path, &json)?;
            let _ = fs::remove_file(&tmp_path);
        }
    }
    Ok(())
}

pub fn acquire_lock(root: &Path, contract_id: &str, expected_version: u64) -> Result<RecoverySession> {
    let _guard = SESSION_MUTEX.lock().unwrap();
    let _file_guard = acquire_file_lock(root, contract_id)?;
    let mut session = load_session_internal(root, contract_id)?
        .ok_or_else(|| anyhow!("Recovery session for contract '{}' not found", contract_id))?;

    if session.session_version != expected_version {
        return Err(anyhow!(
            "CAS_VERSION_MISMATCH: expected version {}, found version {}",
            expected_version,
            session.session_version
        ));
    }

    if session.lock_state == LockState::Locked {
        return Err(anyhow!(
            "CONCURRENCY_LOCK_ACQUISITION_FAILED: recovery session for contract '{}' is already LOCKED",
            contract_id
        ));
    }

    session.lock_state = LockState::Locked;
    session.session_version += 1;
    session.updated_at = Utc::now();
    save_session_internal(root, &session)?;
    Ok(session)
}

pub fn release_lock(root: &Path, contract_id: &str, expected_version: u64) -> Result<RecoverySession> {
    let _guard = SESSION_MUTEX.lock().unwrap();
    let _file_guard = acquire_file_lock(root, contract_id)?;
    let mut session = load_session_internal(root, contract_id)?
        .ok_or_else(|| anyhow!("Recovery session for contract '{}' not found", contract_id))?;

    if session.session_version != expected_version {
        return Err(anyhow!(
            "CAS_VERSION_MISMATCH: expected version {}, found version {}",
            expected_version,
            session.session_version
        ));
    }

    session.lock_state = LockState::Unlocked;
    session.session_version += 1;
    session.updated_at = Utc::now();
    save_session_internal(root, &session)?;
    Ok(session)
}

pub fn update_session_cas<F>(
    root: &Path,
    contract_id: &str,
    expected_version: u64,
    mutate: F,
) -> Result<RecoverySession>
where
    F: FnOnce(&mut RecoverySession) -> Result<()>,
{
    let _guard = SESSION_MUTEX.lock().unwrap();
    let _file_guard = acquire_file_lock(root, contract_id)?;
    let mut session = load_session_internal(root, contract_id)?
        .ok_or_else(|| anyhow!("Recovery session for contract '{}' not found", contract_id))?;

    if session.session_version != expected_version {
        return Err(anyhow!(
            "CAS_VERSION_MISMATCH: expected version {}, found version {}",
            expected_version,
            session.session_version
        ));
    }

    if session.lock_state == LockState::Locked {
        return Err(anyhow!(
            "SESSION_LOCKED: recovery session for contract '{}' is currently LOCKED",
            contract_id
        ));
    }

    mutate(&mut session)?;

    session.session_version += 1;
    session.updated_at = Utc::now();
    save_session_internal(root, &session)?;
    Ok(session)
}

pub fn resolve_session(root: &Path, contract_id: &str) -> Result<Option<RecoverySession>> {
    let _guard = SESSION_MUTEX.lock().unwrap();
    let _file_guard = acquire_file_lock(root, contract_id)?;
    let session_opt = load_session_internal(root, contract_id)?;
    if let Some(mut session) = session_opt {
        if session.status == RecoveryStatus::InProgress {
            session.status = RecoveryStatus::Resolved;
            session.lock_state = LockState::Unlocked;
            session.session_version += 1;
            session.updated_at = Utc::now();
            save_session_internal(root, &session)?;

            let _ = crate::telemetry::record_event(
                root,
                "RECOVERY_SESSION_RESOLVED",
                Some(contract_id),
                "recovery_engine",
                serde_json::json!({
                    "session_id": session.session_id,
                    "session_version": session.session_version,
                    "attempt_count": session.attempt_count,
                }),
            );

            return Ok(Some(session));
        }
        return Ok(Some(session));
    }
    Ok(None)
}

pub fn unblock_session(root: &Path, contract_id: &str) -> Result<Option<RecoverySession>> {
    let _guard = SESSION_MUTEX.lock().unwrap();
    let _file_guard = acquire_file_lock(root, contract_id)?;
    let session_opt = load_session_internal(root, contract_id)?;
    if let Some(mut session) = session_opt {
        session.status = RecoveryStatus::Resolved;
        session.lock_state = LockState::Unlocked;
        session.attempt_count = 0;
        session.session_version += 1;
        session.updated_at = Utc::now();
        save_session_internal(root, &session)?;
        return Ok(Some(session));
    }
    Ok(None)
}

pub fn formulate_recovery_plan(
    contract: &ChangeContract,
    violations: &RecoveryViolationDetails,
    attempt: u32,
    preserved_files: &[String],
) -> RecoveryPlan {
    let mut actions = Vec::new();
    let mut instructions = Vec::new();

    // Invariant: Never modifies source files directly.
    // Invariant: Never expands contract boundaries silently.
    // Invariant: Never modifies Constitution.
    // Invariant: Never auto-promotes memory.

    // 1. Revert boundary violations
    for v in &violations.boundary_violations {
        actions.push(RecoveryAction::revert_mutation(v.clone()));
        instructions.push(format!(
            "Revert changes to `{}`. This path is outside authorized contract boundaries.",
            v
        ));
    }

    // 2. Revert unexpected files
    for u in &violations.unexpected_files {
        actions.push(RecoveryAction::revert_mutation(u.clone()));
        instructions.push(format!(
            "Revert unexpected changes to `{}` or request human contract reconciliation.",
            u
        ));
    }

    // 3. Address constitution rule violations
    for c in &violations.constitution_violations {
        instructions.push(format!(
            "Address Constitution rule error: {}. (Constitution rules are immutable by automated recovery)",
            c
        ));
    }

    // 4. Preserve requested changes
    if !preserved_files.is_empty() {
        for p in preserved_files {
            actions.push(RecoveryAction::preserve_requested_change(p.clone()));
            instructions.push(format!("Preserve authorized change to `{}`.", p));
        }
    } else {
        for exp in &contract.boundary.expected_files {
            actions.push(RecoveryAction::preserve_requested_change(exp.clone()));
            instructions.push(format!("Preserve authorized scope `{}`.", exp));
        }
    }

    // 5. Verification rerun
    actions.push(RecoveryAction::rerun("engram postflight"));
    instructions.push("Rerun `engram postflight` to verify compliance with Change Contract.".to_string());

    let summary = format!(
        "Recovery Plan (Attempt {}): {} revert action(s), {} preserve action(s), rerun check required.",
        attempt,
        violations.boundary_violations.len() + violations.unexpected_files.len(),
        if !preserved_files.is_empty() {
            preserved_files.len()
        } else {
            contract.boundary.expected_files.len()
        }
    );

    RecoveryPlan {
        plan_id: format!("plan_{}", Uuid::new_v4().simple()),
        contract_id: contract.contract_id.clone(),
        attempt,
        actions,
        instructions,
        summary,
    }
}

pub fn recover_contract(root: &Path, contract_id: &str) -> Result<RecoveryReport> {
    let root = root_path(root)?;
    let contract = contract::get_contract(&root, contract_id)?;

    if contract.status == ContractStatus::BlockedAwaitingHuman {
        return Err(anyhow!(
            "RECOVERY_EXHAUSTED: Contract '{}' is BLOCKED_AWAITING_HUMAN. Automated recovery cannot proceed without human intervention.",
            contract_id
        ));
    }
    if contract.status == ContractStatus::Completed {
        return Err(anyhow!(
            "CONTRACT_COMPLETED: Contract '{}' is already COMPLETED. Recovery is not applicable.",
            contract_id
        ));
    }
    if contract.status == ContractStatus::Cancelled {
        return Err(anyhow!(
            "CONTRACT_CANCELLED: Contract '{}' is CANCELLED. Recovery is not applicable.",
            contract_id
        ));
    }

    let _guard = SESSION_MUTEX.lock().unwrap();
    let _file_guard = acquire_file_lock(&root, contract_id)?;

    let session_opt = load_session_internal(&root, contract_id)?;
    let mut session = match session_opt {
        Some(s) => s,
        None => {
            let s = create_recovery_session(contract_id);
            save_session_internal(&root, &s)?;
            s
        }
    };

    if session.lock_state == LockState::Locked {
        return Err(anyhow!(
            "SESSION_LOCKED: recovery session for contract '{}' is currently LOCKED",
            contract_id
        ));
    }

    if session.status == RecoveryStatus::RecoveryExhausted
        || session.status == RecoveryStatus::BlockedAwaitingHuman
    {
        return Err(anyhow!(
            "RECOVERY_EXHAUSTED: Recovery exhausted for contract '{}'. State is BLOCKED_AWAITING_HUMAN.",
            contract_id
        ));
    }

    let expected_version = session.session_version;

    // Semantics: 3rd recovery request is strictly rejected with RECOVERY_EXHAUSTED,
    // transitioning contract/session to BLOCKED_AWAITING_HUMAN.
    if session.attempt_count >= session.max_attempts {
        session.status = RecoveryStatus::RecoveryExhausted;
        session.session_version = expected_version + 1;
        session.updated_at = Utc::now();
        save_session_internal(&root, &session)?;

        let mut updated_contract = contract.clone();
        updated_contract.status = ContractStatus::BlockedAwaitingHuman;
        updated_contract.updated_at = Utc::now();
        contract::save_contract(&root, &updated_contract)?;

        return Err(anyhow!(
            "RECOVERY_EXHAUSTED: Maximum recovery attempts ({}) reached for contract '{}'. Transitioned to BLOCKED_AWAITING_HUMAN.",
            session.max_attempts,
            contract_id
        ));
    }

    // Detect violations in working tree
    let changed_files = crate::storage::git_changed(&root);
    let contract_val = contract::validate_contract_diff(&contract, &changed_files);

    let mut boundary_violations = Vec::new();
    let mut unexpected_files = Vec::new();
    let mut preserved_files = Vec::new();

    for c in &contract_val.classifications {
        match c.category {
            FileValidationCategory::BoundaryViolation => {
                boundary_violations.push(c.path.clone());
            }
            FileValidationCategory::Unexpected => {
                unexpected_files.push(c.path.clone());
            }
            FileValidationCategory::Expected | FileValidationCategory::ExpectedExpansion => {
                preserved_files.push(c.path.clone());
            }
        }
    }

    let conn = crate::storage::db(&root)?;
    let rule_violations = crate::constitution::evaluate_rules(&root, &changed_files, &conn)?;
    let constitution_errors: Vec<String> = rule_violations
        .iter()
        .filter(|v| !v.overridden && v.severity == RuleSeverity::Error)
        .map(|v| format!("{}: {} ({})", v.rule_id, v.message, v.file_path))
        .collect();

    let has_violations = !boundary_violations.is_empty()
        || !unexpected_files.is_empty()
        || !constitution_errors.is_empty();

    if !has_violations {
        session.status = RecoveryStatus::Resolved;
        session.session_version = expected_version + 1;
        session.updated_at = Utc::now();
        session.last_violation = None;
        save_session_internal(&root, &session)?;

        let plan = RecoveryPlan {
            plan_id: format!("plan_{}", Uuid::new_v4().simple()),
            contract_id: contract_id.to_string(),
            attempt: session.attempt_count,
            actions: vec![RecoveryAction::rerun("engram postflight")],
            instructions: vec!["No violations detected. Working tree conforms to Change Contract.".to_string()],
            summary: "Working tree conforms to Change Contract. No repair required.".to_string(),
        };

        return Ok(RecoveryReport {
            contract_id: contract_id.to_string(),
            session_id: session.session_id.clone(),
            session_version: session.session_version,
            attempt_count: session.attempt_count,
            max_attempts: session.max_attempts,
            status: session.status,
            plan,
            summary: format!("Contract '{}' has no violations. Recovery session is RESOLVED.", contract_id),
        });
    }

    session.attempt_count += 1;
    session.status = RecoveryStatus::InProgress;

    let violation_details = RecoveryViolationDetails {
        boundary_violations: boundary_violations.clone(),
        unexpected_files: unexpected_files.clone(),
        constitution_violations: constitution_errors.clone(),
        summary: format!(
            "{} boundary violation(s), {} unexpected file(s), {} constitution error(s)",
            boundary_violations.len(),
            unexpected_files.len(),
            constitution_errors.len()
        ),
    };

    let plan = formulate_recovery_plan(
        &contract,
        &violation_details,
        session.attempt_count,
        &preserved_files,
    );

    session.last_violation = Some(violation_details);
    session.last_recovery_plan = Some(plan.clone());
    session.session_version = expected_version + 1;
    session.updated_at = Utc::now();
    save_session_internal(&root, &session)?;

    let _ = crate::telemetry::record_event(
        &root,
        "RECOVERY_PLAN_FORMULATED",
        Some(contract_id),
        "recovery_planner",
        serde_json::json!({
            "session_id": session.session_id,
            "session_version": session.session_version,
            "attempt_count": session.attempt_count,
            "actions_count": plan.actions.len(),
        }),
    );

    Ok(RecoveryReport {
        contract_id: contract_id.to_string(),
        session_id: session.session_id.clone(),
        session_version: session.session_version,
        attempt_count: session.attempt_count,
        max_attempts: session.max_attempts,
        status: session.status,
        plan,
        summary: format!(
            "Formulated recovery plan for contract '{}' (attempt {}/{})",
            contract_id, session.attempt_count, session.max_attempts
        ),
    })
}

pub fn format_recovery_report(rep: &RecoveryReport) -> String {
    let mut out = String::new();
    out.push_str("🛠️  Engram Recovery Planner\n");
    out.push_str("──────────────────────────────────────────────────────────────────────────\n");
    out.push_str(&format!("Contract:        {}\n", rep.contract_id));
    out.push_str(&format!("Session ID:      {}\n", rep.session_id));
    out.push_str(&format!("Session Version: {}\n", rep.session_version));
    out.push_str(&format!(
        "Attempt:         {}/{} \n",
        rep.attempt_count, rep.max_attempts
    ));
    out.push_str(&format!("Status:          {:?}\n\n", rep.status));

    out.push_str("REPAIR ACTIONS:\n");
    for (i, action) in rep.plan.actions.iter().enumerate() {
        out.push_str(&format!(
            "  {}. [{:?}] {}\n",
            i + 1,
            action.action_type,
            action.description
        ));
    }
    out.push_str("\nINSTRUCTIONS:\n");
    for inst in &rep.plan.instructions {
        out.push_str(&format!("  • {}\n", inst));
    }
    out.push('\n');
    out.push_str(&format!("Summary: {}\n", rep.summary));
    out
}
