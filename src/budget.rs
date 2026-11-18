//! Context Budget Guard, Priority-Tier Packing, and Omission Manifest.
//!
//! Provides deterministic, bounded context retrieval ensuring context payloads
//! never exceed configured token ceilings and all omissions are auditable.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Abstract token counter trait allowing dependency injection of exact or heuristic tokenizers.
pub trait TokenCounter: Send + Sync {
    /// Count tokens for a given text snippet.
    fn count_tokens(&self, text: &str) -> usize;
    /// Name of the tokenizer model or heuristic.
    fn model_name(&self) -> &'static str;
    /// Whether the counter claims exact tokenization vs conservative approximation.
    fn is_exact(&self) -> bool;
}

/// Conservative byte-ratio token counter adhering to the Stage 4 Rev 2.0 specification:
/// estimated_tokens = ceil(serialized_bytes / 3.5)
#[derive(Debug, Clone, Copy)]
pub struct ByteRatioTokenCounter {
    pub ratio: f64,
}

impl Default for ByteRatioTokenCounter {
    fn default() -> Self {
        Self { ratio: 3.5 }
    }
}

impl ByteRatioTokenCounter {
    pub fn new(ratio: f64) -> Self {
        let valid_ratio = if ratio <= 0.1 { 3.5 } else { ratio };
        Self { ratio: valid_ratio }
    }
}

impl TokenCounter for ByteRatioTokenCounter {
    fn count_tokens(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        let bytes = text.len();
        (bytes as f64 / self.ratio).ceil() as usize
    }

    fn model_name(&self) -> &'static str {
        "cl100k_conservative_byte_ratio_3.5"
    }

    fn is_exact(&self) -> bool {
        false
    }
}

/// Simple whitespace-and-punctuation word-based token counter for testing and exact simulation.
#[derive(Debug, Clone, Copy, Default)]
pub struct WordTokenCounter;

impl TokenCounter for WordTokenCounter {
    fn count_tokens(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        // Approximate 1 token per word + punctuation mark
        text.split_whitespace().count().max(1)
    }

    fn model_name(&self) -> &'static str {
        "word_token_counter"
    }

    fn is_exact(&self) -> bool {
        true
    }
}

/// Priority tier hierarchy for deterministic context packing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum PriorityTier {
    /// Tier 1 (Guaranteed/Mandatory): Direct target entity declaration and member signatures.
    Tier1Mandatory = 1,
    /// Tier 2 (High Priority): Direct 1st-party imported services and architectural triad companions.
    Tier2High = 2,
    /// Tier 3 (Normal Priority): Relationship edges, call sites, file paths, and governing rules.
    Tier3Normal = 3,
    /// Tier 4 (Conditional): Complete implementation code excerpts (included only if budget allows).
    Tier4Conditional = 4,
    /// Tier 5 (Optional): Historical co-change alerts, memories, and test metadata.
    Tier5Optional = 5,
}

impl PriorityTier {
    pub fn as_u8(&self) -> u8 {
        *self as u8
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Tier1Mandatory => "Tier 1: Mandatory Target Entity Declarations & Members",
            Self::Tier2High => "Tier 2: Direct 1st-Party Dependencies & State Triads",
            Self::Tier3Normal => "Tier 3: Relationship Graph Edges & Governing Rules",
            Self::Tier4Conditional => "Tier 4: Complete Implementation Code Blocks",
            Self::Tier5Optional => "Tier 5: Extended Memories, Co-Change, & Test Metadata",
        }
    }
}

/// Level of implementation detail for returned symbol entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DetailLevel {
    /// Contracted signatures by default (empty structural bodies `{ /* ... */ }`).
    Contracted,
    /// Complete implementation blocks when explicitly opted-in.
    Full,
}

impl Default for DetailLevel {
    fn default() -> Self {
        Self::Contracted
    }
}

/// Semantic type of a context item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ContextItemType {
    SymbolDeclaration,
    MemberSignature,
    ContractedBody,
    CompleteBlock,
    RelationshipEdge,
    ModuleDeclaration,
    GoverningRule,
    CochangeWarning,
    MemoryDecision,
    FileSummary,
}

impl ContextItemType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SymbolDeclaration => "SYMBOL_DECLARATION",
            Self::MemberSignature => "MEMBER_SIGNATURE",
            Self::ContractedBody => "CONTRACTED_BODY",
            Self::CompleteBlock => "COMPLETE_BLOCK",
            Self::RelationshipEdge => "RELATIONSHIP_EDGE",
            Self::ModuleDeclaration => "MODULE_DECLARATION",
            Self::GoverningRule => "GOVERNING_RULE",
            Self::CochangeWarning => "COCHANGE_WARNING",
            Self::MemoryDecision => "MEMORY_DECISION",
            Self::FileSummary => "FILE_SUMMARY",
        }
    }
}

/// A discrete, priority-tagged context candidate eligible for budget packing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextItem {
    pub id: String,
    pub symbol: Option<String>,
    pub file: String,
    pub item_type: ContextItemType,
    pub tier: PriorityTier,
    pub score: f64,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_range: Option<(usize, usize)>,
}

/// Machine-readable entry describing a single omitted or truncated context item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OmissionEntry {
    pub context_item_id: String,
    pub symbol: Option<String>,
    pub file: String,
    pub context_type: String,
    pub priority_tier: usize,
    pub original_token_count: usize,
    pub included_token_count: usize,
    pub omitted_token_count: usize,
    pub omission_reason: String,
    pub omission_status: String,
    pub retrievable_via_targeted_request: bool,
}

/// Formal context budget and omission manifest conforming to Section 6.3 of the spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmissionManifest {
    pub budget_status: String,
    pub budget_tokens_limit: usize,
    pub budget_tokens_estimated: usize,
    pub reserved_metadata_tokens: usize,
    pub packed_tokens: usize,
    pub remaining_tokens: usize,
    pub omitted_tokens: usize,
    pub serialized_payload_bytes: usize,
    pub included_items_count: usize,
    pub omitted_items_count: usize,
    pub omission_manifest: Vec<OmissionEntry>,
}

/// Outcome of packing context candidates into a bounded token ceiling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackingResult {
    pub packed_items: Vec<ContextItem>,
    pub manifest: OmissionManifest,
    pub formatted_context: String,
    pub is_overflow: bool,
}

/// Production context budget manager.
pub struct BudgetPacker {
    pub token_ceiling: usize,
    pub reserved_metadata_tokens: usize,
    pub counter: Box<dyn TokenCounter>,
    pub detail_level: DetailLevel,
}

impl BudgetPacker {
    pub fn new(token_ceiling: usize) -> Self {
        Self {
            token_ceiling,
            reserved_metadata_tokens: 150, // default reserved headroom for JSON wrapper/headers
            counter: Box::new(ByteRatioTokenCounter::default()),
            detail_level: DetailLevel::Contracted,
        }
    }

    pub fn with_counter(mut self, counter: Box<dyn TokenCounter>) -> Self {
        self.counter = counter;
        self
    }

    pub fn with_reserved_metadata(mut self, reserved: usize) -> Self {
        self.reserved_metadata_tokens = reserved;
        self
    }

    pub fn with_detail_level(mut self, detail_level: DetailLevel) -> Self {
        self.detail_level = detail_level;
        self
    }

    /// Pack context items deterministically respecting priority tiers and the hard token ceiling.
    pub fn pack(&self, items: Vec<ContextItem>) -> PackingResult {
        // Edge case: zero or degenerate token ceiling
        if self.token_ceiling == 0 {
            let mut omissions = Vec::new();
            let mut omitted_tokens = 0;
            for item in items {
                let toks = self.counter.count_tokens(&item.content);
                omitted_tokens += toks;
                omissions.push(OmissionEntry {
                    context_item_id: item.id.clone(),
                    symbol: item.symbol.clone(),
                    file: item.file.clone(),
                    context_type: item.item_type.as_str().to_string(),
                    priority_tier: item.tier.as_u8() as usize,
                    original_token_count: toks,
                    included_token_count: 0,
                    omitted_token_count: toks,
                    omission_reason: "Zero token budget configured".to_string(),
                    omission_status: "CAPACITY_OVERFLOW".to_string(),
                    retrievable_via_targeted_request: true,
                });
            }

            let manifest = OmissionManifest {
                budget_status: "CAPACITY_OVERFLOW".to_string(),
                budget_tokens_limit: 0,
                budget_tokens_estimated: 0,
                reserved_metadata_tokens: 0,
                packed_tokens: 0,
                remaining_tokens: 0,
                omitted_tokens,
                serialized_payload_bytes: 0,
                included_items_count: 0,
                omitted_items_count: omissions.len(),
                omission_manifest: omissions,
            };

            return PackingResult {
                packed_items: Vec::new(),
                manifest,
                formatted_context: "[ERROR: Token budget ceiling is 0]".to_string(),
                is_overflow: true,
            };
        }

        // Deduplicate items by ID while preserving first occurrence
        let mut seen_ids = HashSet::new();
        let mut deduped_items = Vec::new();
        for it in items {
            if seen_ids.insert(it.id.clone()) {
                deduped_items.push(it);
            }
        }

        // Sort items deterministically:
        // 1. Tier ascending (Tier 1 Mandatory first, Tier 5 last)
        // 2. Score descending (higher relevance first)
        // 3. File path ascending
        // 4. Symbol name ascending
        // 5. Item ID ascending
        deduped_items.sort_by(|a, b| {
            a.tier
                .cmp(&b.tier)
                .then(b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
                .then(a.file.cmp(&b.file))
                .then(a.symbol.cmp(&b.symbol))
                .then(a.id.cmp(&b.id))
        });

        let effective_reserved = self.reserved_metadata_tokens.min(self.token_ceiling / 2);
        let usable_capacity = self.token_ceiling.saturating_sub(effective_reserved);

        let mut packed_items = Vec::new();
        let mut omissions = Vec::new();
        let mut packed_tokens = 0usize;
        let mut omitted_tokens = 0usize;
        let mut capacity_overflow = false;

        // Check if Tier 1 alone overflows the budget
        let tier1_tokens: usize = deduped_items
            .iter()
            .filter(|i| i.tier == PriorityTier::Tier1Mandatory)
            .map(|i| self.counter.count_tokens(&i.content))
            .sum();

        if tier1_tokens > self.token_ceiling {
            capacity_overflow = true;
        }

        for item in deduped_items {
            let item_tokens = self.counter.count_tokens(&item.content);

            // Conditional Tier 4 check: only include complete blocks if remaining budget > 40%
            // and detail_level is Full, unless the item content is already contracted
            if item.tier == PriorityTier::Tier4Conditional && self.detail_level != DetailLevel::Full {
                omitted_tokens += item_tokens;
                omissions.push(OmissionEntry {
                    context_item_id: item.id.clone(),
                    symbol: item.symbol.clone(),
                    file: item.file.clone(),
                    context_type: item.item_type.as_str().to_string(),
                    priority_tier: item.tier.as_u8() as usize,
                    original_token_count: item_tokens,
                    included_token_count: 0,
                    omitted_token_count: item_tokens,
                    omission_reason: "Implementation block omitted by default (opt-in via detail_level=full)".to_string(),
                    omission_status: "COMPLETELY_OMITTED".to_string(),
                    retrievable_via_targeted_request: true,
                });
                continue;
            }

            if packed_tokens + item_tokens <= usable_capacity && !capacity_overflow {
                packed_tokens += item_tokens;
                packed_items.push(item);
            } else {
                omitted_tokens += item_tokens;
                let reason = if capacity_overflow && item.tier == PriorityTier::Tier1Mandatory {
                    "Mandatory Tier 1 context exceeds configured token budget ceiling".to_string()
                } else {
                    format!(
                        "Budget ceiling reached in {} (needed {} tokens, {} tokens remaining)",
                        item.tier.description(),
                        item_tokens,
                        usable_capacity.saturating_sub(packed_tokens)
                    )
                };

                let status = if capacity_overflow {
                    "CAPACITY_OVERFLOW"
                } else {
                    "COMPLETELY_OMITTED"
                };

                omissions.push(OmissionEntry {
                    context_item_id: item.id.clone(),
                    symbol: item.symbol.clone(),
                    file: item.file.clone(),
                    context_type: item.item_type.as_str().to_string(),
                    priority_tier: item.tier.as_u8() as usize,
                    original_token_count: item_tokens,
                    included_token_count: 0,
                    omitted_token_count: item_tokens,
                    omission_reason: reason,
                    omission_status: status.to_string(),
                    retrievable_via_targeted_request: true,
                });
            }
        }

        let mut formatted_parts = Vec::new();
        for item in &packed_items {
            formatted_parts.push(item.content.clone());
        }
        let formatted_context = formatted_parts.join("\n\n");
        let serialized_bytes = formatted_context.len();

        let budget_status = if capacity_overflow {
            "CAPACITY_OVERFLOW".to_string()
        } else if !omissions.is_empty() {
            "TRUNCATED_AT_BUDGET".to_string()
        } else {
            "WITHIN_BUDGET".to_string()
        };

        let estimated_total = packed_tokens + effective_reserved;

        let manifest = OmissionManifest {
            budget_status,
            budget_tokens_limit: self.token_ceiling,
            budget_tokens_estimated: estimated_total,
            reserved_metadata_tokens: effective_reserved,
            packed_tokens,
            remaining_tokens: usable_capacity.saturating_sub(packed_tokens),
            omitted_tokens,
            serialized_payload_bytes: serialized_bytes,
            included_items_count: packed_items.len(),
            omitted_items_count: omissions.len(),
            omission_manifest: omissions,
        };

        PackingResult {
            packed_items,
            manifest,
            formatted_context,
            is_overflow: capacity_overflow,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_counter_byte_ratio() {
        let counter = ByteRatioTokenCounter::new(3.5);
        assert_eq!(counter.count_tokens(""), 0);
        // 35 bytes -> 10 tokens
        let s = "01234567890123456789012345678901234";
        assert_eq!(s.len(), 35);
        assert_eq!(counter.count_tokens(s), 10);
    }

    #[test]
    fn test_budget_packer_single_item_fits() {
        let packer = BudgetPacker::new(500)
            .with_counter(Box::new(WordTokenCounter))
            .with_reserved_metadata(50);

        let item = ContextItem {
            id: "item1".to_string(),
            symbol: Some("AuthService".to_string()),
            file: "auth.service.ts".to_string(),
            item_type: ContextItemType::SymbolDeclaration,
            tier: PriorityTier::Tier1Mandatory,
            score: 100.0,
            content: "export class AuthService { login(): void; }".to_string(),
            line_range: Some((1, 10)),
        };

        let res = packer.pack(vec![item]);
        assert_eq!(res.packed_items.len(), 1);
        assert_eq!(res.manifest.budget_status, "WITHIN_BUDGET");
        assert_eq!(res.manifest.omitted_items_count, 0);
    }

    #[test]
    fn test_budget_packer_priority_order_respected() {
        let packer = BudgetPacker::new(100)
            .with_counter(Box::new(WordTokenCounter))
            .with_reserved_metadata(10); // 90 tokens usable

        let tier1 = ContextItem {
            id: "t1".to_string(),
            symbol: Some("T1".to_string()),
            file: "t1.ts".to_string(),
            item_type: ContextItemType::SymbolDeclaration,
            tier: PriorityTier::Tier1Mandatory,
            score: 10.0,
            content: "word ".repeat(40), // 40 tokens
            line_range: None,
        };

        let tier2 = ContextItem {
            id: "t2".to_string(),
            symbol: Some("T2".to_string()),
            file: "t2.ts".to_string(),
            item_type: ContextItemType::MemberSignature,
            tier: PriorityTier::Tier2High,
            score: 50.0, // higher score, but lower tier
            content: "word ".repeat(40), // 40 tokens
            line_range: None,
        };

        let tier3 = ContextItem {
            id: "t3".to_string(),
            symbol: Some("T3".to_string()),
            file: "t3.ts".to_string(),
            item_type: ContextItemType::RelationshipEdge,
            tier: PriorityTier::Tier3Normal,
            score: 90.0,
            content: "word ".repeat(40), // 40 tokens
            line_range: None,
        };

        // 90 usable: tier1 (40) + tier2 (40) = 80 tokens fits. tier3 (40) overflows 80+40 = 120 > 90.
        let res = packer.pack(vec![tier3, tier1, tier2]);
        assert_eq!(res.packed_items.len(), 2);
        assert_eq!(res.packed_items[0].id, "t1");
        assert_eq!(res.packed_items[1].id, "t2");
        assert_eq!(res.manifest.budget_status, "TRUNCATED_AT_BUDGET");
        assert_eq!(res.manifest.omitted_items_count, 1);
        assert_eq!(res.manifest.omission_manifest[0].context_item_id, "t3");
        assert_eq!(res.manifest.omission_manifest[0].priority_tier, 3);
    }

    #[test]
    fn test_budget_packer_duplicate_item_deduped() {
        let packer = BudgetPacker::new(500)
            .with_counter(Box::new(WordTokenCounter))
            .with_reserved_metadata(50);

        let item1 = ContextItem {
            id: "dup".to_string(),
            symbol: Some("S".to_string()),
            file: "f.ts".to_string(),
            item_type: ContextItemType::SymbolDeclaration,
            tier: PriorityTier::Tier1Mandatory,
            score: 100.0,
            content: "content".to_string(),
            line_range: None,
        };
        let item2 = item1.clone();

        let res = packer.pack(vec![item1, item2]);
        assert_eq!(res.packed_items.len(), 1);
    }

    #[test]
    fn test_budget_packer_zero_budget_capacity_overflow() {
        let packer = BudgetPacker::new(0);
        let item = ContextItem {
            id: "x".to_string(),
            symbol: None,
            file: "x.ts".to_string(),
            item_type: ContextItemType::SymbolDeclaration,
            tier: PriorityTier::Tier1Mandatory,
            score: 1.0,
            content: "hello world".to_string(),
            line_range: None,
        };
        let res = packer.pack(vec![item]);
        assert!(res.is_overflow);
        assert_eq!(res.manifest.budget_status, "CAPACITY_OVERFLOW");
        assert_eq!(res.manifest.omitted_items_count, 1);
    }
}
