use serde_json::json;
use std::fs;
use std::path::PathBuf;

use engram::analyzer::analyze;
use engram::budget::DetailLevel;
use engram::compiler::{clear_context_cache, context_scoped_optimized, context_scoped_with_detail};
use engram::mcp::mcp_call;

fn setup_test_workspace(name: &str) -> PathBuf {
    let ws = std::env::temp_dir().join(format!(
        "engram_token_eff_{}_{}",
        name,
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&ws).unwrap();
    ws
}

#[test]
fn test_progressive_disclosure_compact_vs_contracted_vs_full() {
    let ws = setup_test_workspace("progressive");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    let code = r#"
pub struct UserAccount {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub is_active: bool,
}

impl UserAccount {
    pub fn new(id: u64, name: String, email: String) -> Self {
        UserAccount { id, name, email, is_active: true }
    }

    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    pub fn update_email(&mut self, new_email: String) {
        self.email = new_email;
    }
}

pub fn calculate_user_tax(account: &UserAccount, rate: f64) -> f64 {
    if !account.is_active {
        return 0.0;
    }
    100.0 * rate
}
"#;
    fs::write(src.join("user.rs"), code).unwrap();
    analyze(&ws).unwrap();

    // 1. Compile with DetailLevel::Full
    clear_context_cache();
    let ctx_full = context_scoped_with_detail(
        &ws,
        "calculate user tax",
        2000,
        None,
        None,
        DetailLevel::Full,
    )
    .unwrap();
    let tokens_full = ctx_full.estimated_tokens;

    // 2. Compile with DetailLevel::Contracted
    clear_context_cache();
    let ctx_contracted = context_scoped_with_detail(
        &ws,
        "calculate user tax",
        2000,
        None,
        None,
        DetailLevel::Contracted,
    )
    .unwrap();
    let tokens_contracted = ctx_contracted.estimated_tokens;

    // 3. Compile with DetailLevel::Compact
    clear_context_cache();
    let ctx_compact = context_scoped_with_detail(
        &ws,
        "calculate user tax",
        2000,
        None,
        None,
        DetailLevel::Compact,
    )
    .unwrap();
    let tokens_compact = ctx_compact.estimated_tokens;

    println!(
        "Full tokens: {}, Contracted tokens: {}, Compact tokens: {}",
        tokens_full, tokens_contracted, tokens_compact
    );

    assert!(
        tokens_compact < tokens_contracted,
        "Compact ({}) must use fewer tokens than Contracted ({})",
        tokens_compact,
        tokens_contracted
    );
    assert!(
        tokens_contracted <= tokens_full,
        "Contracted ({}) must use <= tokens than Full ({})",
        tokens_contracted,
        tokens_full
    );

    // Verify compact content contains metadata only and no complete block bodies
    let compact_file = ctx_compact.files.first().expect("File should be present");
    assert!(compact_file.content.contains("SYMBOLS (COMPACT METADATA)"));
    assert!(!compact_file.content.contains("[COMPLETE_BLOCK]"));

    // Verify significant reduction (at least 30% reduction from Full to Compact)
    let reduction_pct =
        (tokens_full.saturating_sub(tokens_compact) as f64) / (tokens_full as f64) * 100.0;
    println!(
        "Measured token reduction from Full to Compact: {:.1}%",
        reduction_pct
    );
    assert!(
        reduction_pct >= 30.0,
        "Progressive compact disclosure must achieve at least 30% reduction (measured: {:.1}%)",
        reduction_pct
    );

    fs::remove_dir_all(&ws).ok();
}

#[test]
fn test_targeted_symbols_filtering() {
    let ws = setup_test_workspace("targeted");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    let code = r#"
pub fn helper_alpha() -> &'static str { "alpha" }
pub fn helper_beta() -> &'static str { "beta" }
pub fn helper_gamma() -> &'static str { "gamma" }
pub fn calculate_tax(amount: f64) -> f64 { amount * 0.15 }
pub fn helper_omega() -> &'static str { "omega" }
"#;
    fs::write(src.join("lib.rs"), code).unwrap();
    analyze(&ws).unwrap();

    clear_context_cache();
    // Request only calculate_tax
    let targets = vec!["calculate_tax".to_string()];
    let ctx_targeted = context_scoped_optimized(
        &ws,
        "tax calculation logic",
        2000,
        None,
        None,
        DetailLevel::Contracted,
        Some(&targets),
    )
    .unwrap();

    let file = ctx_targeted.files.first().expect("File should be present");
    // calculate_tax should have excerpt, others should not have multi-line excerpt
    assert!(file.content.contains("calculate_tax"));
    println!("Targeted content:\n{}", file.content);

    fs::remove_dir_all(&ws).ok();
}

#[test]
fn test_context_deduplication() {
    let ws = setup_test_workspace("dedup");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    // Two files with identical helper signature
    fs::write(
        src.join("a.rs"),
        "pub fn common_utility(x: i32) -> i32 { x + 1 }\npub fn unique_a() {}\n",
    )
    .unwrap();
    fs::write(
        src.join("b.rs"),
        "pub fn common_utility(x: i32) -> i32 { x + 1 }\npub fn unique_b() {}\n",
    )
    .unwrap();
    analyze(&ws).unwrap();

    clear_context_cache();
    let ctx = context_scoped_with_detail(
        &ws,
        "common utility work",
        2000,
        None,
        None,
        DetailLevel::Compact,
    )
    .unwrap();
    let all_content: String = ctx.files.iter().map(|f| f.content.as_str()).collect();

    println!("All files content:\n{}", all_content);
    assert!(
        all_content.contains("[DEDUPLICATED]"),
        "Second occurrence of identical signature must be tagged [DEDUPLICATED]"
    );

    fs::remove_dir_all(&ws).ok();
}

#[test]
fn test_adaptive_bypass_trivial_in_mcp() {
    let ws = setup_test_workspace("mcp_adaptive");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("main.rs"), "fn main() { println!(\"Hello\"); }\n").unwrap();
    analyze(&ws).unwrap();

    // 1. Trivial task with adaptive = true
    let trivial_args = json!({
        "task": "fix typo in doc comment in main.rs",
        "adaptive": true
    });
    let res = mcp_call(&ws, "engram_get_context", &trivial_args).unwrap();
    let text = res["content"][0]["text"].as_str().unwrap();
    let val: serde_json::Value = serde_json::from_str(text).unwrap();

    println!("Adaptive MCP response:\n{}", text);
    assert_eq!(val["status"], "BYPASS_TRIVIAL");
    assert_eq!(val["estimated_tokens"], 0);
    assert!(val["target_files"].as_array().unwrap().is_empty());

    // 2. Complex task with adaptive = true should NOT bypass
    let complex_args = json!({
        "task": "refactor database transaction storage and add rollback recovery mechanism",
        "adaptive": true
    });
    let res_complex = mcp_call(&ws, "engram_get_context", &complex_args).unwrap();
    let text_complex = res_complex["content"][0]["text"].as_str().unwrap();
    let val_complex: serde_json::Value = serde_json::from_str(text_complex).unwrap();
    assert_ne!(val_complex["status"], "BYPASS_TRIVIAL");

    fs::remove_dir_all(&ws).ok();
}

#[test]
fn test_compiled_context_query_cache() {
    let ws = setup_test_workspace("cache");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("lib.rs"), "pub fn cached_work() {}\n").unwrap();
    analyze(&ws).unwrap();

    clear_context_cache();
    let ctx1 = context_scoped_with_detail(
        &ws,
        "cached work query",
        1000,
        None,
        None,
        DetailLevel::Compact,
    )
    .unwrap();
    let ctx2 = context_scoped_with_detail(
        &ws,
        "cached work query",
        1000,
        None,
        None,
        DetailLevel::Compact,
    )
    .unwrap();

    assert_eq!(ctx1.task, ctx2.task);
    assert_eq!(ctx1.estimated_tokens, ctx2.estimated_tokens);
    assert_eq!(ctx1.files.len(), ctx2.files.len());

    clear_context_cache();
    fs::remove_dir_all(&ws).ok();
}
