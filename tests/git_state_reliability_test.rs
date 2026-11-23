//! Workstream A: Git State Detection Reliability & Non-Fail-Open Test Suite
//!
//! Verifies:
//! 1. Clean repository -> Ok(true) / CLEAN
//! 2. Repository with tracked modifications -> Ok(false) / DIRTY
//! 3. Repository with untracked files -> Ok(false) / DIRTY
//! 4. Git executable unavailable -> Err(GitStateError::SpawnFailed) / UNKNOWN
//! 5. Git command returns non-zero exit status -> Err(GitStateError::CommandFailed) / UNKNOWN
//! 6. Invalid/malformed Git output -> Err(GitStateError::InvalidUtf8) / UNKNOWN
//! 7. Git command timeout -> Err(GitStateError::Timeout) / UNKNOWN
//! 8. Engage integration: Inspection failure cannot produce CLEAN or is_clean=true

use engram::git::{GitStateError, detect_git_state, detect_git_state_with_timeout};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

fn create_temp_git_repo(name: &str) -> PathBuf {
    let tmp =
        std::env::temp_dir().join(format!("engram_git_rel_{}_{}", name, uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let run_cmd = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(&tmp)
            .output()
            .expect("failed to execute git command");
        assert!(
            output.status.success(),
            "Git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    };

    run_cmd(&["init", "-b", "main"]);
    run_cmd(&["config", "user.name", "Test Runner"]);
    run_cmd(&["config", "user.email", "test@engram.dev"]);

    // Initial commit
    fs::write(tmp.join("README.md"), "# Initial Commit\n").unwrap();
    run_cmd(&["add", "README.md"]);
    run_cmd(&["commit", "-m", "Initial commit"]);

    tmp
}

#[test]
fn test_git_state_clean_repository() {
    let tmp = create_temp_git_repo("clean");
    let res = detect_git_state(&tmp);
    assert_eq!(res, Ok(true), "Clean repository must return Ok(true)");
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_git_state_tracked_modifications_dirty() {
    let tmp = create_temp_git_repo("tracked_dirty");

    // Modify tracked file
    fs::write(tmp.join("README.md"), "# Modified Content\n").unwrap();

    let res = detect_git_state(&tmp);
    assert_eq!(
        res,
        Ok(false),
        "Tracked modifications must return Ok(false) [DIRTY]"
    );
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_git_state_untracked_files_dirty() {
    let tmp = create_temp_git_repo("untracked_dirty");

    // Create a new untracked file
    fs::write(tmp.join("new_file.txt"), "untracked content\n").unwrap();

    let res = detect_git_state(&tmp);
    assert_eq!(
        res,
        Ok(false),
        "Untracked files must return Ok(false) [DIRTY]"
    );
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_git_state_executable_unavailable_fails_closed() {
    let tmp = create_temp_git_repo("no_exec");

    let res = detect_git_state_with_timeout(
        &tmp,
        "nonexistent_git_binary_xyz_12345",
        &["status"],
        Duration::from_secs(1),
    );

    assert!(res.is_err(), "Missing git executable must return Err");
    match res.unwrap_err() {
        GitStateError::SpawnFailed(msg) => {
            assert!(!msg.is_empty());
        }
        other => panic!("Expected SpawnFailed, got {:?}", other),
    }
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_git_state_nonzero_exit_status_fails_closed() {
    // Non-git directory
    let tmp = std::env::temp_dir().join(format!("engram_nongit_{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    let res = detect_git_state(&tmp);
    assert!(res.is_err(), "Non-git directory must return Err");
    match res.unwrap_err() {
        GitStateError::CommandFailed(code, err) => {
            assert!(code.is_some() && code.unwrap() != 0);
            assert!(err.contains("not a git repository") || err.contains("fatal"));
        }
        other => panic!("Expected CommandFailed, got {:?}", other),
    }
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_git_state_invalid_utf8_output_fails_closed() {
    let tmp = std::env::temp_dir().join(format!("engram_utf8_{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    // Run python to output non-UTF8 binary bytes to stdout with exit 0
    let res = detect_git_state_with_timeout(
        &tmp,
        "python",
        &[
            "-c",
            "import sys; sys.stdout.buffer.write(b'\\xff\\xfe\\x80\\x00')",
        ],
        Duration::from_secs(5),
    );

    assert!(res.is_err(), "Invalid UTF-8 output must return Err");
    match res.unwrap_err() {
        GitStateError::InvalidUtf8 => {}
        other => panic!("Expected InvalidUtf8, got {:?}", other),
    }
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_git_state_command_timeout_fails_closed() {
    let tmp = std::env::temp_dir().join(format!("engram_timeout_{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    // Run python sleep 3s with 100ms timeout
    let res = detect_git_state_with_timeout(
        &tmp,
        "python",
        &["-c", "import time; time.sleep(3)"],
        Duration::from_millis(100),
    );

    assert!(res.is_err(), "Timed out command must return Err");
    match res.unwrap_err() {
        GitStateError::Timeout(ms) => {
            assert_eq!(ms, 100);
        }
        other => panic!("Expected Timeout(100), got {:?}", other),
    }
    let _ = fs::remove_dir_all(&tmp);
}

#[test]
fn test_engage_endpoint_cannot_fail_open_on_broken_git() {
    // Non-git directory
    let tmp = std::env::temp_dir().join(format!("engram_engage_nongit_{}", uuid::Uuid::new_v4()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).unwrap();

    // Trivial task in non-git directory
    let req = engram::engage::EngageRequest {
        task: "Fix typo in README.md".to_string(),
        subpath: None,
        language: None,
        token_budget: 4000,
        detail_level: "compact".to_string(),
        planned_files: None,
        forbidden: None,
        symbols: None,
        no_bypass: false,
        inject_instructions: false,
        max_bytes: 16384,
    };

    let res = engram::engage::execute_engage(&tmp, req)
        .expect("engage should succeed even if git unknown");
    assert_eq!(res["status"], "BYPASS_TRIVIAL");
    assert_eq!(
        res["repository"]["git_state"], "UNKNOWN",
        "Git state must be UNKNOWN when git status fails"
    );
    assert!(
        res["repository"]["is_clean"].is_null(),
        "is_clean must be null (not true) when git fails"
    );
    assert!(
        res["repository"]["git_state_error"].is_string(),
        "git_state_error must record error message"
    );

    let _ = fs::remove_dir_all(&tmp);
}
