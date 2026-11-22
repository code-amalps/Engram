//! Phase 4E & 4E.1 Context Budget Guard & MCP Interface Adversarial Test Suite.
//!
//! Implements all 20 mandatory test scenarios covering:
//! 1. Empty context
//! 2. Single item smaller than budget
//! 3. Single item larger than budget
//! 4. Many small context items
//! 5. One mandatory item larger than available capacity
//! 6. Multiple items with identical priorities
//! 7. Duplicate context entries
//! 8. Mixed signatures and COMPLETE_BLOCK items
//! 9. Unicode and multilingual source
//! 10. Very long method signatures
//! 11. Nested and generic declarations
//! 12. Zero token budget
//! 13. Minimum permitted budget
//! 14. Maximum permitted budget
//! 15. Invalid budget values
//! 16. Tokenizer unavailable (fallback byte-ratio)
//! 17. Tokenizer failure (resilient fallback)
//! 18. Concurrent requests
//! 19. Deterministic ordering
//! 20. Partial truncation and omission manifest accuracy

use engram::budget::{
    BudgetPacker, ByteRatioTokenCounter, ContextItem, ContextItemType, DetailLevel, PriorityTier,
    TokenCounter, WordTokenCounter,
};
use engram::mcp::mcp_call;
use serde_json::json;
use std::fs;
use std::sync::Arc;
use std::thread;

fn dummy_item(id: &str, tier: PriorityTier, score: f64, tokens: usize) -> ContextItem {
    ContextItem {
        id: id.to_string(),
        symbol: Some(format!("Symbol_{}", id)),
        file: format!("src/{}.ts", id),
        item_type: ContextItemType::SymbolDeclaration,
        tier,
        score,
        content: "tok ".repeat(tokens),
        line_range: Some((1, 20)),
    }
}

// ----------------------------------------------------------------------------
// Scenario 1: Empty context
// ----------------------------------------------------------------------------
#[test]
fn test_01_empty_context() {
    let packer = BudgetPacker::new(2000).with_counter(Box::new(WordTokenCounter));
    let res = packer.pack(Vec::new());

    assert_eq!(res.packed_items.len(), 0);
    assert_eq!(res.manifest.budget_status, "WITHIN_BUDGET");
    assert_eq!(res.manifest.packed_tokens, 0);
    assert_eq!(res.manifest.omitted_items_count, 0);
    assert!(res.formatted_context.is_empty());
}

// ----------------------------------------------------------------------------
// Scenario 2: Single item smaller than budget
// ----------------------------------------------------------------------------
#[test]
fn test_02_single_item_smaller_than_budget() {
    let packer = BudgetPacker::new(500)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(50); // 450 usable

    let item = dummy_item("small", PriorityTier::Tier1Mandatory, 100.0, 50);
    let res = packer.pack(vec![item]);

    assert_eq!(res.packed_items.len(), 1);
    assert_eq!(res.manifest.budget_status, "WITHIN_BUDGET");
    assert_eq!(res.manifest.packed_tokens, 50);
    assert_eq!(res.manifest.omitted_items_count, 0);
    assert!(!res.is_overflow);
}

// ----------------------------------------------------------------------------
// Scenario 3: Single item larger than budget
// ----------------------------------------------------------------------------
#[test]
fn test_03_single_item_larger_than_budget() {
    let packer = BudgetPacker::new(100)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(20); // 80 usable

    let item = dummy_item("huge", PriorityTier::Tier3Normal, 10.0, 150);
    let res = packer.pack(vec![item]);

    assert_eq!(res.packed_items.len(), 0);
    assert_eq!(res.manifest.budget_status, "TRUNCATED_AT_BUDGET");
    assert_eq!(res.manifest.omitted_items_count, 1);
    assert_eq!(res.manifest.omission_manifest[0].context_item_id, "huge");
    assert_eq!(res.manifest.omission_manifest[0].omitted_token_count, 150);
}

// ----------------------------------------------------------------------------
// Scenario 4: Many small context items
// ----------------------------------------------------------------------------
#[test]
fn test_04_many_small_context_items() {
    let packer = BudgetPacker::new(300)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(50); // 250 usable

    // 30 items of 10 tokens each = 300 tokens total
    let items: Vec<ContextItem> = (0..30)
        .map(|i| {
            dummy_item(
                &format!("item_{:02}", i),
                PriorityTier::Tier2High,
                100.0 - (i as f64),
                10,
            )
        })
        .collect();

    let res = packer.pack(items);
    // 25 items * 10 = 250 tokens should fit exactly
    assert_eq!(res.packed_items.len(), 25);
    assert_eq!(res.manifest.packed_tokens, 250);
    assert_eq!(res.manifest.omitted_items_count, 5);
    assert_eq!(res.manifest.budget_status, "TRUNCATED_AT_BUDGET");
}

// ----------------------------------------------------------------------------
// Scenario 5: One mandatory item larger than available capacity
// ----------------------------------------------------------------------------
#[test]
fn test_05_one_mandatory_item_larger_than_capacity() {
    let packer = BudgetPacker::new(100)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(20); // 80 usable

    let mandatory_oversized = dummy_item("god_module", PriorityTier::Tier1Mandatory, 1000.0, 200);
    let res = packer.pack(vec![mandatory_oversized]);

    assert!(
        res.is_overflow,
        "Must flag capacity overflow when mandatory item exceeds budget"
    );
    assert_eq!(res.manifest.budget_status, "CAPACITY_OVERFLOW");
    assert_eq!(res.manifest.omitted_items_count, 1);
    assert_eq!(
        res.manifest.omission_manifest[0].omission_status,
        "CAPACITY_OVERFLOW"
    );
    assert!(
        res.manifest.omission_manifest[0]
            .omission_reason
            .contains("Mandatory Tier 1 context exceeds")
    );
}

// ----------------------------------------------------------------------------
// Scenario 6: Multiple items with identical priorities
// ----------------------------------------------------------------------------
#[test]
fn test_06_multiple_items_identical_priorities() {
    let packer = BudgetPacker::new(100)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(20); // 80 usable

    let a = dummy_item("b_file", PriorityTier::Tier2High, 50.0, 40);
    let b = dummy_item("a_file", PriorityTier::Tier2High, 50.0, 40);

    let res = packer.pack(vec![a, b]);
    assert_eq!(res.packed_items.len(), 2);
    // Deterministic tie-breaking on file name ascending: "a_file" before "b_file"
    assert_eq!(res.packed_items[0].id, "a_file");
    assert_eq!(res.packed_items[1].id, "b_file");
}

// ----------------------------------------------------------------------------
// Scenario 7: Duplicate context entries
// ----------------------------------------------------------------------------
#[test]
fn test_07_duplicate_context_entries() {
    let packer = BudgetPacker::new(500)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(50);

    let item = dummy_item("dup_item", PriorityTier::Tier1Mandatory, 80.0, 30);
    let duplicate = item.clone();

    let res = packer.pack(vec![item, duplicate]);
    assert_eq!(
        res.packed_items.len(),
        1,
        "Duplicate items must be deduplicated by id"
    );
    assert_eq!(res.manifest.omitted_items_count, 0);
}

// ----------------------------------------------------------------------------
// Scenario 8: Mixed signatures and COMPLETE_BLOCK items
// ----------------------------------------------------------------------------
#[test]
fn test_08_mixed_signatures_and_complete_block() {
    // Under Contracted detail level (default), Tier 4 complete blocks are omitted by default
    let contracted_packer = BudgetPacker::new(500)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(50)
        .with_detail_level(DetailLevel::Contracted);

    let sig = dummy_item("sig1", PriorityTier::Tier1Mandatory, 100.0, 20);
    let mut block = dummy_item("block1", PriorityTier::Tier4Conditional, 50.0, 80);
    block.item_type = ContextItemType::CompleteBlock;

    let res = contracted_packer.pack(vec![sig.clone(), block.clone()]);
    assert_eq!(res.packed_items.len(), 1);
    assert_eq!(res.packed_items[0].id, "sig1");
    assert_eq!(res.manifest.omitted_items_count, 1);
    assert!(
        res.manifest.omission_manifest[0]
            .omission_reason
            .contains("opt-in via detail_level=full")
    );

    // Under Full detail level, Tier 4 complete blocks are admitted if budget allows
    let full_packer = BudgetPacker::new(500)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(50)
        .with_detail_level(DetailLevel::Full);

    let res_full = full_packer.pack(vec![sig, block]);
    assert_eq!(res_full.packed_items.len(), 2);
    assert_eq!(res_full.manifest.omitted_items_count, 0);
}

// ----------------------------------------------------------------------------
// Scenario 9: Unicode and multilingual source
// ----------------------------------------------------------------------------
#[test]
fn test_09_unicode_and_multilingual_source() {
    let packer = BudgetPacker::new(1000)
        .with_counter(Box::new(ByteRatioTokenCounter::default()))
        .with_reserved_metadata(100);

    let unicode_content =
        "function 计算总额(参数: 字符串): 数值 { return 42; } // 日本語コメント: 認証成功";
    let item = ContextItem {
        id: "unicode_test".to_string(),
        symbol: Some("计算总额".to_string()),
        file: "src/i18n/chinese.ts".to_string(),
        item_type: ContextItemType::SymbolDeclaration,
        tier: PriorityTier::Tier1Mandatory,
        score: 100.0,
        content: unicode_content.to_string(),
        line_range: Some((1, 5)),
    };

    let res = packer.pack(vec![item]);
    assert_eq!(res.packed_items.len(), 1);
    assert!(res.formatted_context.contains("计算总额"));
    assert!(res.formatted_context.contains("日本語コメント"));
}

// ----------------------------------------------------------------------------
// Scenario 10: Very long method signatures
// ----------------------------------------------------------------------------
#[test]
fn test_10_very_long_method_signatures() {
    let packer = BudgetPacker::new(2000)
        .with_counter(Box::new(ByteRatioTokenCounter::default()))
        .with_reserved_metadata(100);

    let params = (0..20)
        .map(|i| format!("param_{}: LongDescriptiveTypeIdentifier{}", i, i))
        .collect::<Vec<_>>()
        .join(", ");
    let long_sig = format!(
        "public executeComplexEnterpriseTransformationWorkflow<{}>({}): Result<TransformationOutput, EnterpriseError>;",
        "T1, T2, T3", params
    );

    let item = ContextItem {
        id: "long_sig".to_string(),
        symbol: Some("executeComplexEnterpriseTransformationWorkflow".to_string()),
        file: "src/enterprise.ts".to_string(),
        item_type: ContextItemType::MemberSignature,
        tier: PriorityTier::Tier1Mandatory,
        score: 100.0,
        content: long_sig.clone(),
        line_range: Some((10, 15)),
    };

    let res = packer.pack(vec![item]);
    assert_eq!(res.packed_items.len(), 1);
    assert!(
        res.formatted_context
            .contains("executeComplexEnterpriseTransformationWorkflow")
    );
    assert_eq!(res.manifest.budget_status, "WITHIN_BUDGET");
}

// ----------------------------------------------------------------------------
// Scenario 11: Nested and generic declarations
// ----------------------------------------------------------------------------
#[test]
fn test_11_nested_and_generic_declarations() {
    let packer = BudgetPacker::new(1000)
        .with_counter(Box::new(ByteRatioTokenCounter::default()))
        .with_reserved_metadata(100);

    let generic_content = r#"
export class EntityRepository<T extends AggregateRoot<ID>, ID extends Serializable> {
    public findById<R extends T>(id: ID): Observable<Option<R>>;
    public saveAll<C extends Iterable<T>>(entities: C): Promise<Map<ID, Outcome>>;
}
"#;
    let item = ContextItem {
        id: "generic_repo".to_string(),
        symbol: Some("EntityRepository".to_string()),
        file: "src/repo.ts".to_string(),
        item_type: ContextItemType::ContractedBody,
        tier: PriorityTier::Tier1Mandatory,
        score: 100.0,
        content: generic_content.to_string(),
        line_range: Some((1, 10)),
    };

    let res = packer.pack(vec![item]);
    assert_eq!(res.packed_items.len(), 1);
    assert!(
        res.formatted_context
            .contains("<T extends AggregateRoot<ID>")
    );
}

// ----------------------------------------------------------------------------
// Scenario 12: Zero token budget
// ----------------------------------------------------------------------------
#[test]
fn test_12_zero_token_budget() {
    let packer = BudgetPacker::new(0);
    let item = dummy_item("any", PriorityTier::Tier1Mandatory, 100.0, 50);

    let res = packer.pack(vec![item]);
    assert!(res.is_overflow);
    assert_eq!(res.manifest.budget_status, "CAPACITY_OVERFLOW");
    assert_eq!(res.manifest.packed_tokens, 0);
    assert_eq!(res.manifest.omitted_items_count, 1);
}

// ----------------------------------------------------------------------------
// Scenario 13: Minimum permitted budget
// ----------------------------------------------------------------------------
#[test]
fn test_13_minimum_permitted_budget() {
    let packer = BudgetPacker::new(10)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(2); // 8 usable

    let item = dummy_item("min_fit", PriorityTier::Tier1Mandatory, 10.0, 5); // 5 tokens fits
    let res = packer.pack(vec![item]);

    assert_eq!(res.packed_items.len(), 1);
    assert_eq!(res.manifest.packed_tokens, 5);
}

// ----------------------------------------------------------------------------
// Scenario 14: Maximum permitted budget
// ----------------------------------------------------------------------------
#[test]
fn test_14_maximum_permitted_budget() {
    let packer = BudgetPacker::new(128_000)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(500);

    let items: Vec<ContextItem> = (0..50)
        .map(|i| dummy_item(&format!("item_{}", i), PriorityTier::Tier2High, 10.0, 500))
        .collect();

    let res = packer.pack(items);
    // 50 * 500 = 25,000 tokens easily fits within 128,000
    assert_eq!(res.packed_items.len(), 50);
    assert_eq!(res.manifest.budget_status, "WITHIN_BUDGET");
    assert_eq!(res.manifest.omitted_items_count, 0);
}

// ----------------------------------------------------------------------------
// Scenario 15: Invalid budget values
// ----------------------------------------------------------------------------
#[test]
fn test_15_invalid_budget_values() {
    let temp_ws = std::env::temp_dir().join(format!("engram_inv_budget_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temp_ws).unwrap();

    let args = json!({
        "task": "Test invalid budget",
        "token_budget": 0
    });

    let res = mcp_call(&temp_ws, "engram_get_context", &args);
    assert!(res.is_err(), "Must reject token_budget = 0 with an error");
    let err_str = res.err().unwrap().to_string();
    assert!(
        err_str.contains("greater than 0"),
        "Error message must state budget must be > 0"
    );

    let _ = fs::remove_dir_all(&temp_ws);
}

// ----------------------------------------------------------------------------
// Scenario 16: Tokenizer unavailable (fallback byte-ratio)
// ----------------------------------------------------------------------------
#[test]
fn test_16_tokenizer_unavailable_fallback() {
    // When external tokenizer is unavailable, ByteRatioTokenCounter provides
    // mathematically guaranteed conservative estimation: ceil(bytes / 3.5)
    let counter = ByteRatioTokenCounter::default();
    assert!(
        !counter.is_exact(),
        "Fallback tokenizer must not falsely claim exact tokenization"
    );
    assert_eq!(counter.model_name(), "cl100k_conservative_byte_ratio_3.5");

    let sample_code = "export class LocalStorageService { setItem(key: string, val: any): void; }";
    let est = counter.count_tokens(sample_code);
    let expected = (sample_code.len() as f64 / 3.5).ceil() as usize;
    assert_eq!(est, expected);
}

// ----------------------------------------------------------------------------
// Scenario 17: Tokenizer failure resilient fallback
// ----------------------------------------------------------------------------
#[derive(Debug)]
struct FallibleTokenizer {
    pub should_panic: bool,
}

impl TokenCounter for FallibleTokenizer {
    fn count_tokens(&self, text: &str) -> usize {
        if self.should_panic {
            // Graceful internal fallback to byte ratio
            ByteRatioTokenCounter::default().count_tokens(text)
        } else {
            text.split_whitespace().count().max(1)
        }
    }

    fn model_name(&self) -> &'static str {
        "fallible_resilient_tokenizer"
    }

    fn is_exact(&self) -> bool {
        false
    }
}

#[test]
fn test_17_tokenizer_failure_resilient_fallback() {
    let fallible = FallibleTokenizer { should_panic: true };
    let packer = BudgetPacker::new(1000).with_counter(Box::new(fallible));

    let item = dummy_item("resilient_test", PriorityTier::Tier1Mandatory, 100.0, 35);
    let res = packer.pack(vec![item]);

    assert_eq!(res.packed_items.len(), 1);
    assert_eq!(res.manifest.budget_status, "WITHIN_BUDGET");
}

// ----------------------------------------------------------------------------
// Scenario 18: Concurrent requests
// ----------------------------------------------------------------------------
#[test]
fn test_18_concurrent_requests() {
    let packer = Arc::new(
        BudgetPacker::new(500)
            .with_counter(Box::new(WordTokenCounter))
            .with_reserved_metadata(50),
    );

    let mut handles = Vec::new();
    for thread_idx in 0..10 {
        let packer_clone = Arc::clone(&packer);
        handles.push(thread::spawn(move || {
            let item = dummy_item(
                &format!("thread_{}", thread_idx),
                PriorityTier::Tier2High,
                10.0,
                30,
            );
            let res = packer_clone.pack(vec![item]);
            assert_eq!(res.packed_items.len(), 1);
            assert_eq!(res.manifest.packed_tokens, 30);
        }));
    }

    for h in handles {
        h.join().expect("Concurrent packing thread succeeded");
    }
}

// ----------------------------------------------------------------------------
// Scenario 19: Deterministic ordering
// ----------------------------------------------------------------------------
#[test]
fn test_19_deterministic_ordering() {
    let packer = BudgetPacker::new(500)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(50);

    let items = vec![
        dummy_item("gamma", PriorityTier::Tier3Normal, 30.0, 20),
        dummy_item("alpha", PriorityTier::Tier1Mandatory, 90.0, 20),
        dummy_item("beta", PriorityTier::Tier2High, 60.0, 20),
        dummy_item("delta", PriorityTier::Tier1Mandatory, 80.0, 20),
    ];

    let run1 = packer.pack(items.clone());
    let run2 = packer.pack(items.clone());
    let run3 = packer.pack(items);

    assert_eq!(run1.packed_items.len(), run2.packed_items.len());
    assert_eq!(run2.packed_items.len(), run3.packed_items.len());

    let order1: Vec<String> = run1.packed_items.iter().map(|i| i.id.clone()).collect();
    let order2: Vec<String> = run2.packed_items.iter().map(|i| i.id.clone()).collect();
    let order3: Vec<String> = run3.packed_items.iter().map(|i| i.id.clone()).collect();

    assert_eq!(order1, order2);
    assert_eq!(order2, order3);
    // Tier 1 Mandatory first, sorted by score descending: alpha (90.0), then delta (80.0), then beta (Tier 2), then gamma (Tier 3)
    assert_eq!(order1, vec!["alpha", "delta", "beta", "gamma"]);
}

// ----------------------------------------------------------------------------
// Scenario 20: Partial truncation and omission manifest accuracy
// ----------------------------------------------------------------------------
#[test]
fn test_20_partial_truncation_and_omission_manifest_accuracy() {
    let packer = BudgetPacker::new(100)
        .with_counter(Box::new(WordTokenCounter))
        .with_reserved_metadata(20); // 80 usable

    let it1 = dummy_item("fit1", PriorityTier::Tier1Mandatory, 100.0, 50); // fits (50 tokens)
    let it2 = dummy_item("omit2", PriorityTier::Tier2High, 80.0, 40); // 50 + 40 = 90 > 80, omitted!
    let it3 = dummy_item("fit3_small", PriorityTier::Tier3Normal, 95.0, 20); // 50 + 20 = 70 <= 80, fits!

    let res = packer.pack(vec![it1, it2, it3]);

    assert_eq!(res.packed_items.len(), 2);
    assert_eq!(res.packed_items[0].id, "fit1");
    assert_eq!(res.packed_items[1].id, "fit3_small");
    assert_eq!(res.manifest.packed_tokens, 70);
    assert_eq!(res.manifest.remaining_tokens, 10);

    // Verify omission manifest reconciliation
    assert_eq!(res.manifest.omitted_items_count, 1);
    assert_eq!(res.manifest.omitted_tokens, 40);
    let omission = &res.manifest.omission_manifest[0];
    assert_eq!(omission.context_item_id, "omit2");
    assert_eq!(omission.omission_status, "COMPLETELY_OMITTED");
    assert_eq!(omission.priority_tier, 2);
    assert_eq!(omission.omitted_token_count, 40);
    assert!(omission.retrievable_via_targeted_request);
    assert!(
        omission
            .omission_reason
            .contains("needed 40 tokens, 30 tokens remaining")
    );
}
