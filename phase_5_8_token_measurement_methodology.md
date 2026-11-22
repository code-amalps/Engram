# Phase 5.8: Token Measurement Methodology & Analytical Standards

**Document ID:** `ENGRAM-PHASE-5.8-TOKEN-METHODOLOGY`  
**Date:** 2026-11-22  
**Scope:** Definition of token measurement models, estimation boundaries, and conversion standards.  

---

## 1. Measurement Levels & Taxonomies

To prevent misleading claims, Engram distinguishes four distinct levels of cost measurement:

1. **Source Bytes ($B_{\text{src}}$):** The raw UTF-8 byte length of source code files on disk.
2. **Serialized Payload Bytes ($B_{\text{payload}}$):** The byte length of the compiled JSON or JSON-RPC context response emitted over stdio or HTTP.
3. **Estimated Synthetic Tokens ($T_{\text{est}}$):** Mathematical approximation of tokens using the linear ratio:
   $$T_{\text{est}} = \left\lceil \frac{B_{\text{payload}}}{3.8} \right\rceil$$
4. **Empirical Provider Tokens ($T_{\text{live}}$):** Actual tokens billed by remote model provider APIs (OpenAI, Anthropic, Google) as returned in API response headers and usage events (`prompt_tokens`, `completion_tokens`).

---

## 2. Inherent Limitations of Synthetic Estimations

- **Tokenizer Variance:** Different model tokenizers (e.g. OpenAI `o200k_base` vs Anthropic Claude 3.5 Sonnet BPE) tokenize whitespace, indentation, and code symbols with varying efficiency. The `Bytes / 3.8` ratio is an established engineering average, but individual requests may vary by $\pm 12\%$.
- **Downstream Expansion Calls:** In progressive disclosure mode (`DetailLevel::Compact`), an initial response is small (124 tokens). If the coding agent subsequently requires full excerpts, it issues additional tool calls (`symbols: [...]`), which incur tool invocation framing overhead. Total multi-turn savings depends on agent prompting strategy.
- **Strict Prohibition on Marketing Equivalencies:** Synthetic token reduction figures (e.g. 68.2%) must never be cited in customer-facing documentation as guaranteed billing savings until live matched-pair empirical benchmarking (Phase 5.6 Checkpoint B) is conducted.
