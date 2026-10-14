//! Phase 2 Hardening Test Suite (Steps 6, 7, 8):
//! - Step 6: Incremental Analysis Engine (`src/incremental.rs`):
//!   - Content/blob hash authoritative (mtime/size as fast-path hint).
//!   - Tier 1 syntactic invalidation + Tier 2 relational re-linking.
//! - Step 7: Machine-Readable Agent Protocol (`protocol_version: "1.1"`):
//!   - Strongly-typed JSON serialization across preflight, postflight, bench, check, doctor.
//! - Step 8: Local Engineering Provenance & Telemetry (`src/telemetry.rs`):
//!   - Append-only cryptographic hash chain in `.engram/events/`.
//!   - EVENT_CHAIN_VALID, EVENT_CHAIN_BROKEN, and EVENT_MISSING detection.
//!   - Preflight, postflight, check, candidate creation, promotion event generation.
//!   - engram doctor verification integration.

use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use engram::analyzer::analyze;
use engram::bench::PROTOCOL_VERSION as BENCH_PROTO;
use engram::contract::{check_contract, CheckOptions, PROTOCOL_VERSION as CONTRACT_PROTO};
use engram::doctor::{diagnose_memory_health, PROTOCOL_VERSION as DOCTOR_PROTO};
use engram::flywheel::{
    postflight_check, preflight_task, PostflightOptions, PreflightOptions,
    PROTOCOL_VERSION as FLYWHEEL_PROTO,
};
use engram::incremental::{
    get_authoritative_content_hash, incremental_analyze, init_meta_cache,
    PROTOCOL_VERSION as INCR_PROTO,
};
use engram::memory::{promote_candidate, record_task_candidate};
use engram::storage::db;
use engram::telemetry::{
    load_events, record_event, verify_event_chain, EventChainStatus, GENESIS_HASH,
    PROTOCOL_VERSION as TELEM_PROTO,
};

fn temp_repo_dir(prefix: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("engram_p2_harden_{prefix}_{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();

    let _ = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&dir)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.name", "Test Agent"])
        .current_dir(&dir)
        .output();
    let _ = std::process::Command::new("git")
        .args(["config", "user.email", "agent@engram.dev"])
        .current_dir(&dir)
        .output();

    dir
}

#[test]
fn test_protocol_version_uniformity() {
    assert_eq!(FLYWHEEL_PROTO, "1.1");
    assert_eq!(CONTRACT_PROTO, "1.1");
    assert_eq!(DOCTOR_PROTO, "1.1");
    assert_eq!(BENCH_PROTO, "1.1");
    assert_eq!(INCR_PROTO, "1.1");
    assert_eq!(TELEM_PROTO, "1.1");
}

#[test]
fn test_two_tier_incremental_analysis_engine() {
    let root = temp_repo_dir("incremental_engine");

    // Create a multi-file dependency scenario:
    // service.ts imports utils.ts and calls formatCurrency
    // controller.ts imports service.ts
    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    fs::write(
        src_dir.join("utils.ts"),
        "export function formatCurrency(amount: number): string { return `$${amount}`; }\n",
    )
    .unwrap();

    fs::write(
        src_dir.join("service.ts"),
        "import { formatCurrency } from './utils';\nexport class OrderService {\n  format(amount: number) { return formatCurrency(amount); }\n}\n",
    )
    .unwrap();

    fs::write(
        src_dir.join("controller.ts"),
        "import { OrderService } from './service';\nexport class OrderController {}\n",
    )
    .unwrap();

    // Commit baseline files
    let _ = std::process::Command::new("git").args(["add", "."]).current_dir(&root).output();
    let _ = std::process::Command::new("git").args(["commit", "-m", "Initial"]).current_dir(&root).output();

    // Cold analysis
    let cold_res = incremental_analyze(&root).expect("Cold incremental analysis should succeed");
    assert_eq!(cold_res.protocol_version, "1.1");
    assert_eq!(cold_res.total_files, 3);
    assert_eq!(cold_res.tier1_reparsed_files.len(), 3);
    assert_eq!(cold_res.unchanged_files_count, 0);

    // Second run with ZERO file modifications:
    // All 3 files must be recognized as unchanged (Tier 1 skipped via fast-path / hash match)
    let warm_clean = incremental_analyze(&root).expect("Warm clean analysis should succeed");
    assert_eq!(warm_clean.total_files, 3);
    assert_eq!(warm_clean.tier1_reparsed_files.len(), 0, "No files modified, Tier 1 must re-parse 0");
    assert_eq!(warm_clean.unchanged_files_count, 3);

    // Modify ONLY utils.ts:
    // Tier 1 must re-parse only src/utils.ts (1 file)
    // Tier 2 must detect service.ts (which imports utils.ts) as relational dependent and re-link
    fs::write(
        src_dir.join("utils.ts"),
        "export function formatCurrency(amount: number): string { return `USD ${amount}`; }\n",
    )
    .unwrap();

    let warm_modified = incremental_analyze(&root).expect("Warm modified analysis should succeed");
    assert_eq!(
        warm_modified.tier1_reparsed_files.len(), 1,
        "Tier 1 must re-parse strictly the 1 modified file"
    );
    assert_eq!(warm_modified.tier1_reparsed_files[0], "src/utils.ts");
    assert!(
        warm_modified.tier2_relinked_files.contains(&"src/service.ts".to_string()),
        "Tier 2 must identify src/service.ts as dependent on utils.ts"
    );
    assert!(warm_modified.unchanged_files_count >= 1);

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_authoritative_content_hash_and_fast_path_hint() {
    let root = temp_repo_dir("hash_fastpath");
    let file_path = root.join("test.ts");
    fs::write(&file_path, "export const x = 1;\n").unwrap();

    let conn = db(&root).unwrap();
    init_meta_cache(&conn).unwrap();

    // 1st call: Fast-path miss, computes authoritative hash
    let (h1, bytes1, is_fast1) = get_authoritative_content_hash(&file_path, "test.ts", &conn).unwrap();
    assert!(!is_fast1, "First lookup must be fast-path miss");
    assert!(!h1.is_empty());
    assert!(!bytes1.is_empty());

    // 2nd call without touch: Fast-path hit
    let (h2, _, is_fast2) = get_authoritative_content_hash(&file_path, "test.ts", &conn).unwrap();
    assert!(is_fast2, "Second lookup with untouched file must be fast-path hit");
    assert_eq!(h1, h2, "Cached hash must match authoritative hash");

    // Modify file with identical length: authoritative hash must change!
    fs::write(&file_path, "export const y = 2;\n").unwrap();
    // Ensure mtime changes
    std::thread::sleep(std::time::Duration::from_millis(15));

    let (h3, _, is_fast3) = get_authoritative_content_hash(&file_path, "test.ts", &conn).unwrap();
    assert!(!is_fast3, "Modified file must be fast-path miss");
    assert_ne!(h1, h3, "Content hash must change when content changes");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_telemetry_append_only_hash_chain_and_verification() {
    let root = temp_repo_dir("telemetry_chain");

    // Initially empty event chain
    let (status0, _) = verify_event_chain(&root).unwrap();
    assert_eq!(status0, EventChainStatus::Valid);

    // Record Event 1 (Genesis)
    let ev1 = record_event(
        &root,
        "PREFLIGHT_HANDSHAKE",
        Some("ctr_001"),
        "agent_sonnet",
        serde_json::json!({"task": "Order export"}),
    )
    .unwrap();

    assert_eq!(ev1.sequence_number, 1);
    assert_eq!(ev1.previous_event_hash, GENESIS_HASH);
    assert_ne!(ev1.event_hash, GENESIS_HASH);

    // Record Event 2
    let ev2 = record_event(
        &root,
        "POSTFLIGHT_CHECK",
        Some("ctr_001"),
        "agent_sonnet",
        serde_json::json!({"verdict": "PASS"}),
    )
    .unwrap();

    assert_eq!(ev2.sequence_number, 2);
    assert_eq!(ev2.previous_event_hash, ev1.event_hash);

    // Record Event 3
    let ev3 = record_event(
        &root,
        "CANDIDATE_PROMOTION",
        Some("ctr_001"),
        "developer_alice",
        serde_json::json!({"promoted": true}),
    )
    .unwrap();

    assert_eq!(ev3.sequence_number, 3);
    assert_eq!(ev3.previous_event_hash, ev2.event_hash);

    // 1. Verify clean chain
    let (status_clean, msg_clean) = verify_event_chain(&root).unwrap();
    assert_eq!(status_clean, EventChainStatus::Valid);
    assert!(msg_clean.contains("EVENT_CHAIN_VALID"));
    assert!(msg_clean.contains("3 event(s)"));

    let loaded = load_events(&root).unwrap();
    assert_eq!(loaded.len(), 3);

    // 2. Test Tampering Detection -> EVENT_CHAIN_BROKEN
    // Modify Event 2's payload on disk
    let ev2_path = root.join(".engram").join("events").join(format!("{:06}_{}.json", ev2.sequence_number, ev2.event_id));
    let mut ev2_tampered = ev2.clone();
    ev2_tampered.actor = "malicious_actor".to_string();
    fs::write(&ev2_path, serde_json::to_string_pretty(&ev2_tampered).unwrap()).unwrap();

    let (status_tampered, msg_tampered) = verify_event_chain(&root).unwrap();
    assert_eq!(status_tampered, EventChainStatus::Broken);
    assert!(msg_tampered.contains("EVENT_CHAIN_BROKEN"));

    // Restore clean ev2
    fs::write(&ev2_path, serde_json::to_string_pretty(&ev2).unwrap()).unwrap();
    assert_eq!(verify_event_chain(&root).unwrap().0, EventChainStatus::Valid);

    // 3. Test Sequence Gap Detection -> EVENT_MISSING
    // Delete Event 2 from disk
    fs::remove_file(&ev2_path).unwrap();

    let (status_gap, msg_gap) = verify_event_chain(&root).unwrap();
    assert_eq!(status_gap, EventChainStatus::Missing);
    assert!(msg_gap.contains("EVENT_MISSING"));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_closed_loop_telemetry_and_doctor_integration() {
    let root = temp_repo_dir("doctor_telemetry");

    // Initialize files and analyze
    fs::write(root.join("index.ts"), "export const APP = 'Engram';\n").unwrap();
    let _ = std::process::Command::new("git").args(["add", "."]).current_dir(&root).output();
    let _ = std::process::Command::new("git").args(["commit", "-m", "init"]).current_dir(&root).output();

    analyze(&root).unwrap();

    // 1. Preflight handshake -> records PREFLIGHT_HANDSHAKE event
    let pre_opts = PreflightOptions {
        task: "Add payment gateway".into(),
        token_budget: 1000,
        forbidden_boundaries: None,
    };
    let pre_res = preflight_task(&root, &pre_opts).unwrap();
    assert_eq!(pre_res.protocol_version, "1.1");

    // 2. Contract check -> records BOUNDARY_CHECK event
    let check_opts = CheckOptions::default();
    let check_res = check_contract(&root, &check_opts);
    assert_eq!(check_res.protocol_version, "1.1");

    // 3. Candidate creation -> records CANDIDATE_CREATION event
    let cand = record_task_candidate(
        &root,
        &pre_res.task_id,
        "Payment Policy",
        "Payment policy statement",
        "Reason for policy",
        vec!["payment".into()],
        vec!["index.ts".into()],
        "agent",
    )
    .unwrap();

    // 4. Candidate promotion -> records CANDIDATE_PROMOTION event
    let _promoted = promote_candidate(&root, &cand.candidate_id, "lead_developer", None).unwrap();

    // 5. Postflight check -> records POSTFLIGHT_CHECK event
    let post_opts = PostflightOptions {
        contract_id: Some(pre_res.contract_id),
        strict: false,
        staged: false,
        base_commit: None,
        ci_mode: false,
    };
    let post_res = postflight_check(&root, &post_opts).unwrap();
    assert_eq!(post_res.protocol_version, "1.1");

    // Verify all 5 telemetry events were created and chain is valid
    let events = load_events(&root).unwrap();
    assert!(events.len() >= 4, "Expected at least 4 recorded audit events, got {}", events.len());

    let (chain_status, _) = verify_event_chain(&root).unwrap();
    assert_eq!(chain_status, EventChainStatus::Valid);

    // 6. Engram Doctor diagnostic: verifies event chain integrity integration
    let doc_report = diagnose_memory_health(&root).unwrap();
    assert_eq!(doc_report.protocol_version, "1.1");
    assert_eq!(doc_report.event_chain_status, Some("EVENT_CHAIN_VALID".to_string()));

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_strongly_typed_json_serialization_across_all_commands() {
    let root = temp_repo_dir("json_serialization");
    fs::write(root.join("app.ts"), "export class App {}\n").unwrap();
    let _ = std::process::Command::new("git").args(["add", "."]).current_dir(&root).output();
    let _ = std::process::Command::new("git").args(["commit", "-m", "init"]).current_dir(&root).output();

    analyze(&root).unwrap();

    // 1. Preflight JSON
    let pre = preflight_task(&root, &PreflightOptions {
        task: "App feature".into(),
        token_budget: 1000,
        forbidden_boundaries: None,
    }).unwrap();
    let pre_json = serde_json::to_string(&pre).unwrap();
    let pre_val: serde_json::Value = serde_json::from_str(&pre_json).unwrap();
    assert_eq!(pre_val["protocol_version"], "1.1");

    // 2. Postflight JSON
    let post = postflight_check(&root, &PostflightOptions {
        contract_id: Some(pre.contract_id),
        strict: false,
        staged: false,
        base_commit: None,
        ci_mode: false,
    }).unwrap();
    let post_json = serde_json::to_string(&post).unwrap();
    let post_val: serde_json::Value = serde_json::from_str(&post_json).unwrap();
    assert_eq!(post_val["protocol_version"], "1.1");

    // 3. Contract Check JSON
    let check = check_contract(&root, &CheckOptions::default());
    let check_json = serde_json::to_string(&check).unwrap();
    let check_val: serde_json::Value = serde_json::from_str(&check_json).unwrap();
    assert_eq!(check_val["protocol_version"], "1.1");

    // 4. Doctor JSON
    let doc = diagnose_memory_health(&root).unwrap();
    let doc_json = serde_json::to_string(&doc).unwrap();
    let doc_val: serde_json::Value = serde_json::from_str(&doc_json).unwrap();
    assert_eq!(doc_val["protocol_version"], "1.1");

    // 5. Incremental JSON
    let incr = incremental_analyze(&root).unwrap();
    let incr_json = serde_json::to_string(&incr).unwrap();
    let incr_val: serde_json::Value = serde_json::from_str(&incr_json).unwrap();
    assert_eq!(incr_val["protocol_version"], "1.1");

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_duplicate_sequence_tampering_detected_as_broken() {
    let root = temp_repo_dir("dup_sequence");

    let _ev1 = record_event(
        &root,
        "PREFLIGHT_HANDSHAKE",
        Some("ctr_001"),
        "agent",
        serde_json::json!({"test": 1}),
    )
    .unwrap();

    // Create a forged second event with duplicate sequence number 1
    let forged_ev = engram::telemetry::TelemetryEvent {
        sequence_number: 1,
        event_id: "evt_forged001".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        event_type: "FORGED_EVENT".to_string(),
        repository_revision: "rev_forged".to_string(),
        contract_id: None,
        actor: "attacker".to_string(),
        payload: serde_json::json!({"tamper": true}),
        previous_event_hash: GENESIS_HASH.to_string(),
        event_hash: "deadbeef".to_string(),
    };

    let forged_path = root.join(".engram").join("events").join("000001_evt_forged001.json");
    fs::write(&forged_path, serde_json::to_string_pretty(&forged_ev).unwrap()).unwrap();

    let (status, msg) = verify_event_chain(&root).unwrap();
    // Must be flagged as BROKEN, not missing
    assert_eq!(status, EventChainStatus::Broken);
    assert!(
        msg.contains("EVENT_CHAIN_BROKEN"),
        "Duplicate sequence must report EVENT_CHAIN_BROKEN, got: {}",
        msg
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn test_zero_disk_read_on_warm_cache_fast_path() {
    let root = temp_repo_dir("fast_path_zero_read");
    let file_path = root.join("service.ts");
    fs::write(&file_path, "export class OrderService {}\n").unwrap();

    let conn = db(&root).unwrap();
    init_meta_cache(&conn).unwrap();

    // 1st call: cold read
    let (h1, bytes1, is_fast1) = get_authoritative_content_hash(&file_path, "service.ts", &conn).unwrap();
    assert!(!is_fast1);
    assert!(!bytes1.is_empty());

    // 2nd call: warm hit -> bytes is empty because disk read was short-circuited
    let (h2, bytes2, is_fast2) = get_authoritative_content_hash(&file_path, "service.ts", &conn).unwrap();
    assert!(is_fast2);
    assert_eq!(h1, h2);
    assert!(bytes2.is_empty(), "Warm fast-path hit must short-circuit disk read");

    let _ = fs::remove_dir_all(&root);
}
