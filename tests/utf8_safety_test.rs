use engram::mcp::{compute_envelope_bytes, enforce_mcp_envelope_boundary, safe_truncate_str};
use serde_json::{Value, json};
use std::panic::{AssertUnwindSafe, catch_unwind};

/// 1. ASCII text at exactly 1,500 bytes.
#[test]
fn test_utf8_01_ascii_exact_boundary() {
    let input = "a".repeat(1500);
    let truncated = safe_truncate_str(&input, 1500);
    assert_eq!(truncated.len(), 1500);
    assert_eq!(truncated, input);
}

/// 2. ASCII text exceeding 1,500 bytes.
#[test]
fn test_utf8_02_ascii_exceeding_boundary() {
    let input = "a".repeat(2000);
    let truncated = safe_truncate_str(&input, 1500);
    assert_eq!(truncated.len(), 1500);
    assert_eq!(truncated, &input[..1500]);
}

/// 3. A multibyte character ending exactly at the boundary.
#[test]
fn test_utf8_03_multibyte_ending_exactly_at_boundary() {
    // 1497 ASCII bytes ('x') + 1 3-byte bullet '•' (bytes 1497..1500). Total 1500 bytes.
    let mut input = "x".repeat(1497);
    input.push('•');
    assert_eq!(input.len(), 1500);
    let truncated = safe_truncate_str(&input, 1500);
    assert_eq!(truncated.len(), 1500);
    assert!(truncated.ends_with('•'));
}

/// 4. A multibyte character crossing the boundary.
#[test]
fn test_utf8_04_multibyte_crossing_boundary() {
    // 1498 ASCII bytes ('x') + 1 3-byte bullet '•' (bytes 1498..1501) + suffix. Total 1514 bytes.
    let mut input = "x".repeat(1498);
    input.push('•');
    input.push_str("extra_content");
    assert_eq!(input.len(), 1514);

    // In previous implementation: &input[..1500] panics because byte 1500 is within '•' (bytes 1498..1501)
    let truncated = safe_truncate_str(&input, 1500);
    assert_eq!(truncated.len(), 1498);
    assert_eq!(truncated, &input[..1498]);
    assert!(!truncated.ends_with('•'));
    assert!(truncated.chars().all(|c| c == 'x'));
}

/// 5. Unicode bullets.
#[test]
fn test_utf8_05_unicode_bullets() {
    // String of bullets: '•' is 3 bytes (0xE2, 0x80, 0xA2)
    let input = "•".repeat(10); // 30 bytes
    // Test boundary 10 bytes: 3 bullets = 9 bytes. 10th byte would split the 4th bullet.
    let truncated = safe_truncate_str(&input, 10);
    assert_eq!(truncated.len(), 9);
    assert_eq!(truncated, "•••");
}

/// 6. Unicode dashes.
#[test]
fn test_utf8_06_unicode_dashes() {
    // Em dash '—' (3 bytes: 0xE2, 0x80, 0x94) and En dash '–' (3 bytes: 0xE2, 0x80, 0x93)
    let input = "—–—"; // 9 bytes
    // Limit 5 bytes: 1 em-dash = 3 bytes; 5th byte splits the en-dash.
    let truncated = safe_truncate_str(input, 5);
    assert_eq!(truncated.len(), 3);
    assert_eq!(truncated, "—");
}

/// 7. Emoji.
#[test]
fn test_utf8_07_emoji() {
    // Crab emoji '🦀' is 4 bytes (0xF0, 0x9F, 0xA6, 0x80)
    // Rocket emoji '🚀' is 4 bytes (0xF0, 0x9F, 0x9A, 0x80)
    let input = "🦀🚀💡"; // 12 bytes
    // Boundary 5: should only keep the 1st emoji (4 bytes), omitting the incomplete rocket.
    let truncated = safe_truncate_str(input, 5);
    assert_eq!(truncated.len(), 4);
    assert_eq!(truncated, "🦀");

    // Boundary 7: still only keeps the 1st emoji (4 bytes).
    let truncated2 = safe_truncate_str(input, 7);
    assert_eq!(truncated2.len(), 4);
    assert_eq!(truncated2, "🦀");

    // Boundary 8: keeps both crab and rocket (8 bytes).
    let truncated3 = safe_truncate_str(input, 8);
    assert_eq!(truncated3.len(), 8);
    assert_eq!(truncated3, "🦀🚀");
}

/// 8. Mixed ASCII and multibyte text.
#[test]
fn test_utf8_08_mixed_ascii_and_multibyte_sweep() {
    let input = "Impact: • auth.effects.ts (15 changes) — critical path 🚀 [Verified ✔]";
    // Exhaustive sweep: test EVERY possible byte boundary from 0 to input.len() + 10
    for limit in 0..=(input.len() + 10) {
        let truncated = safe_truncate_str(input, limit);
        assert!(
            truncated.len() <= limit,
            "Truncated length {} must be <= limit {}",
            truncated.len(),
            limit
        );
        // Assert valid UTF-8 and valid prefix substring
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
        assert!(input.starts_with(truncated));
    }
}

/// 9. Empty strings.
#[test]
fn test_utf8_09_empty_strings() {
    assert_eq!(safe_truncate_str("", 1500), "");
    assert_eq!(safe_truncate_str("", 0), "");
    assert_eq!(safe_truncate_str("", 1), "");
}

/// 10. Strings shorter than the truncation threshold.
#[test]
fn test_utf8_10_strings_shorter_than_threshold() {
    let input = "hello world";
    assert_eq!(safe_truncate_str(input, 1500), "hello world");
    assert_eq!(safe_truncate_str(input, 11), "hello world");
    assert_eq!(safe_truncate_str(input, 12), "hello world");
}

/// 11. Repeated truncation.
#[test]
fn test_utf8_11_repeated_truncation() {
    let input = "•".repeat(500); // 1500 bytes
    let t1 = safe_truncate_str(&input, 1000); // 999 bytes (333 bullets)
    assert_eq!(t1.len(), 999);

    // Idempotent when called with same or larger limit
    let t2 = safe_truncate_str(t1, 1000);
    assert_eq!(t1, t2);
    let t3 = safe_truncate_str(t1, 1500);
    assert_eq!(t1, t3);

    // Further reduction
    let t4 = safe_truncate_str(t1, 500); // 498 bytes (166 bullets)
    assert_eq!(t4.len(), 498);
}

/// 12. Valid serialized JSON after truncation.
#[test]
fn test_utf8_12_valid_serialized_json_after_truncation() {
    // Construct an envelope payload with multibyte characters at compaction boundary thresholds
    let payload = json!({
        "status": "READY",
        "explanation": format!("Impact: {}", "•".repeat(100)), // 308 bytes
        "summary": format!("Summary: {}", "—".repeat(50)),      // 159 bytes
        "task": format!("Task: {}", "🚀".repeat(30)),            // 126 bytes
        "target": "src/app/core/auth/auth.effects.ts",
        "target_files": [
            {
                "path": "src/app/core/auth/auth.effects.ts",
                "level": "L1",
                "score": 1.0,
                "content": format!("// Comment: {}\nexport class AuthEffects {{}}", "★".repeat(200))
            }
        ]
    });

    // Enforce under tight 1024-byte boundary which exercises multiple compaction levels
    let res = enforce_mcp_envelope_boundary("engram_inspect_symbol", payload, 1024, &json!(1));
    let env_bytes = compute_envelope_bytes(&json!(1), &res);
    assert!(
        env_bytes <= 1024,
        "Final envelope bytes ({}) must be <= 1024",
        env_bytes
    );

    let content_text = res["content"][0]["text"].as_str().expect("Valid text");
    let parsed: Value =
        serde_json::from_str(content_text).expect("Must parse cleanly as valid JSON");
    assert!(parsed.is_object());
}

/// Deterministic panic reproduction test demonstrating that naive slicing panics and safe_truncate_str succeeds.
#[test]
fn test_utf8_deterministic_panic_reproduction() {
    // Exact reproduction of Candidate 4 / Candidate 4.1 bug at src/mcp.rs:860:
    // When raw_explanation has 1498 ASCII bytes followed by 3-byte bullet '•' (bytes 1498..1501).
    let mut buggy_input = "a".repeat(1498);
    buggy_input.push('•'); // bytes 1498, 1499, 1500
    buggy_input.push_str("rest of explanation");

    // 1. Assert that naive &buggy_input[..1500] PANICS deterministically!
    let naive_result = catch_unwind(AssertUnwindSafe(|| {
        let _ = &buggy_input[..1500];
    }));
    assert!(
        naive_result.is_err(),
        "Naive slicing at byte 1500 MUST panic because byte 1500 is inside '•'!"
    );

    // 2. Assert that safe_truncate_str DOES NOT panic and returns clean valid UTF-8!
    let safe_result = catch_unwind(AssertUnwindSafe(|| safe_truncate_str(&buggy_input, 1500)));
    assert!(safe_result.is_ok(), "safe_truncate_str MUST NOT panic!");
    let safe_str = safe_result.unwrap();
    assert_eq!(safe_str.len(), 1498);
    assert_eq!(safe_str, &buggy_input[..1498]);
}

/// End-to-end MCP integration test: engram_inspect_symbol on a large fan-out producing an explanation
/// with Unicode bullets >1500 bytes must not panic.
#[test]
fn test_utf8_14_mcp_inspect_symbol_end_to_end_no_panic() {
    let ws = std::env::temp_dir().join(format!("engram_utf8_mcp_{}", uuid::Uuid::new_v4()));
    let src = ws.join("src");
    std::fs::create_dir_all(&src).unwrap();

    std::fs::write(
        src.join("hub.ts"),
        r#"export class MainHubService {
  process() { return "ok"; }
}
"#,
    )
    .unwrap();

    for i in 1..=25 {
        std::fs::write(
            src.join(format!("consumer_{:02}.ts", i)),
            format!(
                "import {{ MainHubService }} from './hub';\nexport class Consumer{:02}Service {{\n  hub = new MainHubService();\n}}\n",
                i
            ),
        ).unwrap();
    }

    engram::analyzer::analyze(&ws).unwrap();

    let args = json!({
        "target": "MainHubService",
        "max_bytes": 16384
    });

    let res = engram::mcp::mcp_call(&ws, "engram_inspect_symbol", &args)
        .expect("mcp_call engram_inspect_symbol must succeed without panicking");

    let env_bytes = compute_envelope_bytes(&json!(1), &res);
    assert!(
        env_bytes <= 16384,
        "Envelope bytes {} must be <= 16384",
        env_bytes
    );

    let content_text = res["content"][0]["text"].as_str().expect("text");
    let parsed: Value = serde_json::from_str(content_text).expect("Valid JSON");
    assert!(parsed.get("completeness").is_some() || parsed.get("status").is_some());

    let _ = std::fs::remove_dir_all(&ws);
}
