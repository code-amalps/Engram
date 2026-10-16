//! Local Engineering Provenance & Telemetry Subsystem:
//! Tamper-evident append-only cryptographic hash chain stored in `.engram/events/`.
//! Verifies chain integrity (`EVENT_CHAIN_VALID`, `EVENT_CHAIN_BROKEN`, `EVENT_MISSING`).

use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};
use uuid::Uuid;

use crate::storage::{hash, revision, root_path};

pub const PROTOCOL_VERSION: &str = "1.1";

/// SHA-256 hash of empty byte slice: standard cryptographic genesis anchor.
pub const GENESIS_HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

static TELEMETRY_MUTEX: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventChainStatus {
    #[serde(rename = "EVENT_CHAIN_VALID")]
    Valid,
    #[serde(rename = "EVENT_CHAIN_BROKEN")]
    Broken,
    #[serde(rename = "EVENT_MISSING")]
    Missing,
}

impl std::fmt::Display for EventChainStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventChainStatus::Valid => write!(f, "EVENT_CHAIN_VALID"),
            EventChainStatus::Broken => write!(f, "EVENT_CHAIN_BROKEN"),
            EventChainStatus::Missing => write!(f, "EVENT_MISSING"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub sequence_number: u64,
    pub event_id: String,
    pub timestamp: String,
    pub event_type: String,
    pub repository_revision: String,
    #[serde(default)]
    pub contract_id: Option<String>,
    pub actor: String,
    pub payload: serde_json::Value,
    pub previous_event_hash: String,
    pub event_hash: String,
}

pub fn events_dir(root: &Path) -> Result<PathBuf> {
    let dir = root.join(".engram").join("events");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// Compute canonical cryptographic SHA-256 hash for a telemetry event record.
#[allow(clippy::too_many_arguments)]
pub fn compute_event_hash(
    seq: u64,
    event_id: &str,
    timestamp: &str,
    event_type: &str,
    repo_rev: &str,
    contract_id: Option<&str>,
    actor: &str,
    payload: &serde_json::Value,
    prev_hash: &str,
) -> String {
    let canonical = format!(
        "{seq}|{event_id}|{timestamp}|{event_type}|{repo_rev}|{}|{actor}|{prev_hash}|{}",
        contract_id.unwrap_or(""),
        serde_json::to_string(payload).unwrap_or_default()
    );
    hash(canonical.as_bytes())
}

/// Load all recorded events from `.engram/events/` sorted by sequence number.
pub fn load_events(root: &Path) -> Result<Vec<TelemetryEvent>> {
    let dir = root.join(".engram").join("events");
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut events = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)?;
            if let Ok(ev) = serde_json::from_str::<TelemetryEvent>(&content) {
                events.push(ev);
            }
        }
    }

    if events.is_empty() {
        let jsonl_path = dir.join("events.jsonl");
        if jsonl_path.exists() {
            let content = fs::read_to_string(&jsonl_path)?;
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    #[allow(clippy::collapsible_if)]
                    if let Ok(ev) = serde_json::from_str::<TelemetryEvent>(trimmed) {
                        events.push(ev);
                    }
                }
            }
        }
    }

    events.sort_by_key(|e| e.sequence_number);
    Ok(events)
}

/// Append a new event to the cryptographic audit chain in `.engram/events/`.
pub fn record_event(
    root: &Path,
    event_type: &str,
    contract_id: Option<&str>,
    actor: &str,
    payload: serde_json::Value,
) -> Result<TelemetryEvent> {
    let _guard = TELEMETRY_MUTEX.lock().unwrap();
    let root = root_path(root)?;
    let dir = events_dir(&root)?;

    let existing = load_events(&root)?;
    let (next_seq, prev_hash) = if let Some(last) = existing.last() {
        (last.sequence_number + 1, last.event_hash.clone())
    } else {
        (1, GENESIS_HASH.to_string())
    };

    let event_id = format!("evt_{}", &Uuid::new_v4().to_string().replace('-', "")[..13]);
    let timestamp = Utc::now().to_rfc3339();
    let repo_rev = revision(&root);

    let event_hash = compute_event_hash(
        next_seq,
        &event_id,
        &timestamp,
        event_type,
        &repo_rev,
        contract_id,
        actor,
        &payload,
        &prev_hash,
    );

    let event = TelemetryEvent {
        sequence_number: next_seq,
        event_id: event_id.clone(),
        timestamp,
        event_type: event_type.to_string(),
        repository_revision: repo_rev,
        contract_id: contract_id.map(String::from),
        actor: actor.to_string(),
        payload,
        previous_event_hash: prev_hash,
        event_hash,
    };

    // 1. Write individual JSON file via atomic .tmp rename
    let file_path = dir.join(format!("{:06}_{}.json", next_seq, event_id));
    let tmp_path = dir.join(format!(
        "{:06}_{}.json.{}.tmp",
        next_seq,
        event_id,
        Uuid::new_v4().simple()
    ));
    let json_text = serde_json::to_string_pretty(&event)?;
    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(json_text.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp_path, &file_path)?;

    // 2. Append to events.jsonl
    let jsonl_path = dir.join("events.jsonl");
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&jsonl_path)?;
    let line = serde_json::to_string(&event)?;
    writeln!(f, "{line}")?;
    f.sync_all()?;

    Ok(event)
}

/// Verify the entire event hash chain for tamper evidence.
/// Returns:
/// - (EVENT_CHAIN_VALID, summary) if chain is pristine
/// - (EVENT_CHAIN_BROKEN, explanation) if tampering or hash mismatch detected
/// - (EVENT_MISSING, explanation) if sequence numbers have gaps
pub fn verify_event_chain(root: &Path) -> Result<(EventChainStatus, String)> {
    let root = root_path(root)?;
    let dir = root.join(".engram").join("events");
    if !dir.exists() {
        return Ok((
            EventChainStatus::Valid,
            "EVENT_CHAIN_VALID: 0 events recorded (empty chain pristine)".to_string(),
        ));
    }

    // 1. Check for malformed individual event JSON files
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            let content = fs::read_to_string(&path)?;
            if let Err(e) = serde_json::from_str::<TelemetryEvent>(&content) {
                return Ok((
                    EventChainStatus::Broken,
                    format!(
                        "EVENT_CHAIN_BROKEN: Malformed telemetry event file '{}': {}",
                        path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                        e
                    ),
                ));
            }
        }
    }

    // 2. Check for malformed lines in events.jsonl
    let jsonl_path = dir.join("events.jsonl");
    let mut jsonl_events = Vec::new();
    if jsonl_path.exists() {
        let content = fs::read_to_string(&jsonl_path)?;
        for (line_num, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                match serde_json::from_str::<TelemetryEvent>(trimmed) {
                    Ok(ev) => jsonl_events.push(ev),
                    Err(e) => {
                        return Ok((
                            EventChainStatus::Broken,
                            format!(
                                "EVENT_CHAIN_BROKEN: Malformed line in events.jsonl at line {}: {}",
                                line_num + 1,
                                e
                            ),
                        ));
                    }
                }
            }
        }
    }

    let events = load_events(&root)?;

    if events.is_empty() {
        if !jsonl_events.is_empty() {
            return Ok((
                EventChainStatus::Broken,
                "EVENT_CHAIN_BROKEN: events.jsonl contains records but individual event files are missing".to_string(),
            ));
        }
        return Ok((
            EventChainStatus::Valid,
            "EVENT_CHAIN_VALID: 0 events recorded (empty chain pristine)".to_string(),
        ));
    }

    // 3. Verify sequence continuity and missing events
    let first = &events[0];
    if first.sequence_number != 1 {
        return Ok((
            EventChainStatus::Missing,
            format!(
                "EVENT_MISSING: Expected initial event sequence 1, found {}",
                first.sequence_number
            ),
        ));
    }
    for (i, ev) in events.iter().enumerate() {
        if i > 0 {
            let prev = &events[i - 1];
            if ev.sequence_number > prev.sequence_number + 1 {
                return Ok((
                    EventChainStatus::Missing,
                    format!(
                        "EVENT_MISSING: Sequence gap detected between event {} and {}",
                        prev.sequence_number, ev.sequence_number
                    ),
                ));
            }
        }
    }

    // 4. Dual-store consistency check
    if !jsonl_events.is_empty() {
        if jsonl_events.len() != events.len() {
            return Ok((
                EventChainStatus::Broken,
                format!(
                    "EVENT_CHAIN_BROKEN: Record count mismatch between events.jsonl ({}) and event files ({})",
                    jsonl_events.len(),
                    events.len()
                ),
            ));
        }
        for (file_ev, jsonl_ev) in events.iter().zip(jsonl_events.iter()) {
            if file_ev.event_hash != jsonl_ev.event_hash
                || file_ev.sequence_number != jsonl_ev.sequence_number
            {
                return Ok((
                    EventChainStatus::Broken,
                    format!(
                        "EVENT_CHAIN_BROKEN: Dual-store hash mismatch at sequence {}: file hash {} != jsonl hash {}",
                        file_ev.sequence_number, file_ev.event_hash, jsonl_ev.event_hash
                    ),
                ));
            }
        }
    }

    // 5. Genesis anchor check
    if first.previous_event_hash != GENESIS_HASH {
        return Ok((
            EventChainStatus::Broken,
            format!(
                "EVENT_CHAIN_BROKEN: Genesis event previous_event_hash mismatch (expected {}, found {})",
                GENESIS_HASH, first.previous_event_hash
            ),
        ));
    }

    // 5. Cryptographic hash chain verification
    for (i, ev) in events.iter().enumerate() {
        // Verify individual event hash integrity
        let computed = compute_event_hash(
            ev.sequence_number,
            &ev.event_id,
            &ev.timestamp,
            &ev.event_type,
            &ev.repository_revision,
            ev.contract_id.as_deref(),
            &ev.actor,
            &ev.payload,
            &ev.previous_event_hash,
        );

        if computed != ev.event_hash {
            return Ok((
                EventChainStatus::Broken,
                format!(
                    "EVENT_CHAIN_BROKEN: Hash verification failed for event {} ('{}'): expected {}, recorded {}",
                    ev.sequence_number, ev.event_id, computed, ev.event_hash
                ),
            ));
        }

        // Verify predecessor chain link
        if i > 0 {
            let prev = &events[i - 1];
            if ev.sequence_number <= prev.sequence_number {
                return Ok((
                    EventChainStatus::Broken,
                    format!(
                        "EVENT_CHAIN_BROKEN: Duplicate or inverted sequence detected between event {} and {}",
                        prev.sequence_number, ev.sequence_number
                    ),
                ));
            }

            if ev.sequence_number > prev.sequence_number + 1 {
                return Ok((
                    EventChainStatus::Missing,
                    format!(
                        "EVENT_MISSING: Sequence gap detected between event {} and {}",
                        prev.sequence_number, ev.sequence_number
                    ),
                ));
            }

            if ev.previous_event_hash != prev.event_hash {
                return Ok((
                    EventChainStatus::Broken,
                    format!(
                        "EVENT_CHAIN_BROKEN: Chain link broken between event {} and {}: previous_event_hash mismatch",
                        prev.sequence_number, ev.sequence_number
                    ),
                ));
            }
        }
    }

    Ok((
        EventChainStatus::Valid,
        format!(
            "EVENT_CHAIN_VALID: Verified {} event(s) across tamper-evident cryptographic hash chain",
            events.len()
        ),
    ))
}
