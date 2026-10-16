//! Specification Test Harness for Engram Pillar 5:
//! Append-Only Event Telemetry & Tamper-Evident Provenance Subsystem (`.engram/events/`).
//!
//! Validates:
//! 1. Sequential SHA-256 Chaining (Genesis anchor -> Sequence increment -> Hash link integrity)
//! 2. Tampering Detection (`EVENT_CHAIN_BROKEN`) on payload / actor mutation
//! 3. Sequence Gap / Truncation Detection (`EVENT_MISSING`)
//! 4. Malformed Record Detection (`EVENT_CHAIN_BROKEN`) without panicking
//! 5. Dual-Store Consistency (individual JSON files vs. events.jsonl)
//! 6. `engram doctor` Diagnostic Integration (`EVENT_CHAIN_VALID` / `EVENT_CHAIN_BROKEN` / `EVENT_MISSING`)
//! 7. Recovery Engine Provenance Events Integration
//! 8. Concurrency Protection under multithreaded logging

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Barrier};
use std::thread;

use engram::doctor::{diagnose_memory_health, format_doctor_report};
use engram::flywheel::{PreflightOptions, preflight_task};
use engram::recovery::recover_contract;
use engram::telemetry::{
    EventChainStatus, GENESIS_HASH, PROTOCOL_VERSION, compute_event_hash, events_dir, load_events,
    record_event, verify_event_chain,
};
use serde_json::json;
use uuid::Uuid;

fn setup_temp_repo(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("engram_telemetry_test_{}_{}", name, Uuid::new_v4()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();

    fs::write(root.join("main.go"), "package main\n\nfunc main() {}\n").unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "TelemetryTest"])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "telemetry@engram.dev"])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&root)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&root)
        .output()
        .unwrap();

    root
}

/// Test 1: Sequential SHA-256 cryptographic hash chaining and genesis anchor verification.
#[test]
fn test_1_sequential_sha256_cryptographic_hash_chaining() {
    let root = setup_temp_repo("seq_chain");

    // Initially, zero events -> pristine chain
    let (status0, msg0) = verify_event_chain(&root).unwrap();
    assert_eq!(status0, EventChainStatus::Valid);
    assert!(msg0.contains("0 events recorded"));

    // Record Event 1
    let ev1 = record_event(
        &root,
        "PREFLIGHT_TASK",
        Some("contract_001"),
        "agent_planner",
        json!({ "task": "Implement payment provider", "budget": 1200 }),
    )
    .unwrap();

    assert_eq!(ev1.sequence_number, 1);
    assert_eq!(ev1.previous_event_hash, GENESIS_HASH);
    let expected_hash1 = compute_event_hash(
        1,
        &ev1.event_id,
        &ev1.timestamp,
        "PREFLIGHT_TASK",
        &ev1.repository_revision,
        Some("contract_001"),
        "agent_planner",
        &ev1.payload,
        GENESIS_HASH,
    );
    assert_eq!(ev1.event_hash, expected_hash1);

    // Record Event 2
    let ev2 = record_event(
        &root,
        "BOUNDARY_CHECK",
        Some("contract_001"),
        "guard_enforcer",
        json!({ "violations": 0, "status": "COMPLIANT" }),
    )
    .unwrap();

    assert_eq!(ev2.sequence_number, 2);
    assert_eq!(ev2.previous_event_hash, ev1.event_hash);

    // Record Event 3
    let ev3 = record_event(
        &root,
        "DECISION_RATIFIED",
        Some("contract_001"),
        "human_lead",
        json!({ "decision_id": "ADR-042", "rationale": "Use Stripe API v3" }),
    )
    .unwrap();

    assert_eq!(ev3.sequence_number, 3);
    assert_eq!(ev3.previous_event_hash, ev2.event_hash);

    // Verify chain integrity
    let (status, msg) = verify_event_chain(&root).unwrap();
    assert_eq!(status, EventChainStatus::Valid);
    assert!(msg.contains("Verified 3 event(s) across tamper-evident cryptographic hash chain"));

    // Verify individual files on disk exist
    let dir = events_dir(&root).unwrap();
    assert!(dir.join(format!("{:06}_{}.json", 1, ev1.event_id)).exists());
    assert!(dir.join(format!("{:06}_{}.json", 2, ev2.event_id)).exists());
    assert!(dir.join(format!("{:06}_{}.json", 3, ev3.event_id)).exists());
    assert!(dir.join("events.jsonl").exists());

    let _ = fs::remove_dir_all(&root);
}

/// Test 2: Tampering detection: modifying a recorded payload breaks the cryptographic hash.
#[test]
fn test_2_tampering_detection_returns_event_chain_broken() {
    let root = setup_temp_repo("tamper");

    let ev1 = record_event(&root, "EVT_1", None, "actor_a", json!({ "key": "val1" })).unwrap();
    let _ev2 = record_event(&root, "EVT_2", None, "actor_b", json!({ "key": "val2" })).unwrap();

    // Pristine check
    let (status, _) = verify_event_chain(&root).unwrap();
    assert_eq!(status, EventChainStatus::Valid);

    // Tamper with ev1 individual JSON file on disk (change actor from actor_a to adversary)
    let dir = events_dir(&root).unwrap();
    let ev1_path = dir.join(format!("{:06}_{}.json", 1, ev1.event_id));
    let mut tampered_ev1 = ev1.clone();
    tampered_ev1.actor = "malicious_adversary".to_string();
    fs::write(
        &ev1_path,
        serde_json::to_string_pretty(&tampered_ev1).unwrap(),
    )
    .unwrap();

    // Verify: Tamper must be detected immediately as EVENT_CHAIN_BROKEN
    let (broken_status, broken_msg) = verify_event_chain(&root).unwrap();
    assert_eq!(broken_status, EventChainStatus::Broken);
    assert!(broken_msg.contains("EVENT_CHAIN_BROKEN"));

    let _ = fs::remove_dir_all(&root);
}

/// Test 3: Sequence gap detection: removing an intermediate event returns EVENT_MISSING.
#[test]
fn test_3_sequence_gap_detection_returns_event_missing() {
    let root = setup_temp_repo("seq_gap");

    let _ev1 = record_event(&root, "EVT_1", None, "actor_a", json!({})).unwrap();
    let ev2 = record_event(&root, "EVT_2", None, "actor_b", json!({})).unwrap();
    let _ev3 = record_event(&root, "EVT_3", None, "actor_c", json!({})).unwrap();

    // Delete intermediate event 2 file on disk
    let dir = events_dir(&root).unwrap();
    let ev2_path = dir.join(format!("{:06}_{}.json", 2, ev2.event_id));
    fs::remove_file(&ev2_path).unwrap();

    // Also remove ev2 from events.jsonl to isolate the file sequence gap test
    let jsonl_path = dir.join("events.jsonl");
    let content = fs::read_to_string(&jsonl_path).unwrap();
    let filtered_lines: Vec<&str> = content
        .lines()
        .filter(|line| !line.contains(&ev2.event_id))
        .collect();
    fs::write(&jsonl_path, filtered_lines.join("\n") + "\n").unwrap();

    let (missing_status, missing_msg) = verify_event_chain(&root).unwrap();
    assert_eq!(missing_status, EventChainStatus::Missing);
    assert!(missing_msg.contains("EVENT_MISSING"));
    assert!(missing_msg.contains("Sequence gap detected between event 1 and 3"));

    let _ = fs::remove_dir_all(&root);
}

/// Test 4: Malformed record detection: corrupted JSON in events directory reports EVENT_CHAIN_BROKEN cleanly.
#[test]
fn test_4_malformed_json_quarantine_and_broken_detection() {
    let root = setup_temp_repo("malformed");

    let _ev1 = record_event(&root, "EVT_1", None, "actor_a", json!({})).unwrap();

    // Write a corrupted file into .engram/events/
    let dir = events_dir(&root).unwrap();
    let corrupt_path = dir.join("000002_corrupted.json");
    fs::write(&corrupt_path, "{ NOT_VALID_JSON: [ missing brackets ...").unwrap();

    let (broken_status, broken_msg) = verify_event_chain(&root).unwrap();
    assert_eq!(broken_status, EventChainStatus::Broken);
    assert!(broken_msg.contains("EVENT_CHAIN_BROKEN"));
    assert!(broken_msg.contains("Malformed telemetry event file"));

    // Clean up corrupted file and chain is valid again
    fs::remove_file(&corrupt_path).unwrap();
    let (valid_status, _) = verify_event_chain(&root).unwrap();
    assert_eq!(valid_status, EventChainStatus::Valid);

    let _ = fs::remove_dir_all(&root);
}

/// Test 5: Dual-store consistency: detects desynchronization between individual JSON files and events.jsonl.
#[test]
fn test_5_dual_store_inconsistency_detection() {
    let root = setup_temp_repo("dual_store");

    let _ev1 = record_event(&root, "EVT_1", None, "actor_a", json!({})).unwrap();
    let _ev2 = record_event(&root, "EVT_2", None, "actor_b", json!({})).unwrap();

    let (status_pristine, _) = verify_event_chain(&root).unwrap();
    assert_eq!(status_pristine, EventChainStatus::Valid);

    // Append an extra rogue record only into events.jsonl
    let dir = events_dir(&root).unwrap();
    let jsonl_path = dir.join("events.jsonl");
    let mut f = OpenOptions::new().append(true).open(&jsonl_path).unwrap();
    let fake_ev = json!({
        "sequence_number": 3,
        "event_id": "evt_fake000000",
        "timestamp": "2026-10-16T12:00:00Z",
        "event_type": "ROGUE_INJECTION",
        "repository_revision": "HEAD",
        "contract_id": null,
        "actor": "intruder",
        "payload": {},
        "previous_event_hash": "deadbeef",
        "event_hash": "cafebabe"
    });
    writeln!(f, "{}", fake_ev).unwrap();

    let (broken_status, broken_msg) = verify_event_chain(&root).unwrap();
    assert_eq!(broken_status, EventChainStatus::Broken);
    assert!(broken_msg.contains("EVENT_CHAIN_BROKEN"));
    assert!(broken_msg.contains("Record count mismatch"));

    let _ = fs::remove_dir_all(&root);
}

/// Test 6: `engram doctor` diagnostic integration: verifies memory health diagnostic includes event chain integrity.
#[test]
fn test_6_engram_doctor_diagnoses_event_chain_integrity() {
    let root = setup_temp_repo("doctor_diag");

    // 1. Record an event
    let _ev = record_event(&root, "PREFLIGHT", None, "agent", json!({})).unwrap();

    // 2. Run memory doctor diagnostic
    let report = diagnose_memory_health(&root).unwrap();
    assert_eq!(report.protocol_version, PROTOCOL_VERSION);
    assert_eq!(
        report.event_chain_status,
        Some("EVENT_CHAIN_VALID".to_string())
    );
    assert!(
        report
            .event_chain_details
            .as_deref()
            .unwrap()
            .contains("Verified 1 event(s)")
    );

    // 3. Format doctor report for terminal and assert human-readable output
    let formatted = format_doctor_report(&report);
    assert!(formatted.contains("Engram Memory Health & Liveness Diagnostic"));
    assert!(formatted.contains("Audit Event Telemetry Chain: EVENT_CHAIN_VALID"));

    // 4. Corrupt event chain and verify doctor catches it
    let dir = events_dir(&root).unwrap();
    fs::write(dir.join("999999_corrupt.json"), "broken").unwrap();

    let report_corrupt = diagnose_memory_health(&root).unwrap();
    assert_eq!(
        report_corrupt.event_chain_status,
        Some("EVENT_CHAIN_BROKEN".to_string())
    );

    let _ = fs::remove_dir_all(&root);
}

/// Test 7: Recovery Engine Provenance: recovery lifecycle operations record auditable telemetry events.
#[test]
fn test_7_recovery_engine_telemetry_event_provenance() {
    let root = setup_temp_repo("recovery_telemetry");

    let pre_opts = PreflightOptions {
        task: "Update export".to_string(),
        token_budget: 1200,
        forbidden_boundaries: Some(vec!["internal/secrets/**".to_string()]),
    };
    let pre = preflight_task(&root, &pre_opts).unwrap();
    let contract_id = pre.contract_id.clone();

    // Mutate forbidden file
    fs::create_dir_all(root.join("internal/secrets")).unwrap();
    fs::write(
        root.join("internal/secrets/vault.go"),
        "package secrets\n\ntype Vault struct {}\n// UNAUTHORIZED\n",
    )
    .unwrap();

    // Call recovery -> records RECOVERY_PLAN_FORMULATED
    let rep1 = recover_contract(&root, &contract_id).unwrap();
    assert_eq!(rep1.attempt_count, 1);

    // Call recovery again -> records RECOVERY_PLAN_FORMULATED (attempt 2)
    let rep2 = recover_contract(&root, &contract_id).unwrap();
    assert_eq!(rep2.attempt_count, 2);

    // Verify recorded telemetry events
    let events = load_events(&root).unwrap();
    let formulated_events: Vec<&engram::telemetry::TelemetryEvent> = events
        .iter()
        .filter(|e| e.event_type == "RECOVERY_PLAN_FORMULATED")
        .collect();
    assert_eq!(
        formulated_events.len(),
        2,
        "Expected 2 formulated recovery plan events"
    );

    // Cryptographic chain remains valid across all recovery events
    let (status, _) = verify_event_chain(&root).unwrap();
    assert_eq!(status, EventChainStatus::Valid);

    let _ = fs::remove_dir_all(&root);
}

/// Test 8: Concurrency protection: simultaneous multithreaded recording maintains hash chain integrity.
#[test]
fn test_8_concurrent_telemetry_recording() {
    let root = setup_temp_repo("concurrent_telemetry");
    let thread_count = 8;
    let events_per_thread = 5;

    let barrier = Arc::new(Barrier::new(thread_count));
    let root_arc = Arc::new(root.clone());
    let mut handles = Vec::new();

    for t in 0..thread_count {
        let b = Arc::clone(&barrier);
        let r = Arc::clone(&root_arc);
        handles.push(thread::spawn(move || {
            b.wait();
            for i in 0..events_per_thread {
                record_event(
                    &r,
                    "CONCURRENT_EVENT",
                    None,
                    &format!("worker_thread_{}", t),
                    json!({ "iteration": i, "thread_id": t }),
                )
                .unwrap();
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify all 40 events recorded
    let events = load_events(&root).unwrap();
    assert_eq!(events.len(), thread_count * events_per_thread);

    // Invariant: Sequence numbers strictly increment 1..40 without collisions or inversions
    for (idx, ev) in events.iter().enumerate() {
        assert_eq!(ev.sequence_number, (idx + 1) as u64);
    }

    // Invariant: The entire hash chain is 100% valid
    let (status, msg) = verify_event_chain(&root).unwrap();
    assert_eq!(status, EventChainStatus::Valid);
    assert!(msg.contains("Verified 40 event(s)"));

    let _ = fs::remove_dir_all(&root);
}
