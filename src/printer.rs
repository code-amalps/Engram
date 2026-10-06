//! Decoupled human presentation layer for interactive terminals.
//! Core logic never depends on this module.

use serde_json::Value;

use crate::analyzer::AnalysisSummary;
use crate::compiler::ContextResult;
use crate::model::EcosystemReport;

pub fn print_analysis_summary(s: &AnalysisSummary) {
    println!("🧠 Engram Repository Analysis");
    println!("──────────────────────────────────────────");
    println!("  Revision:       {}", s.revision);
    println!("  Indexed Files:  {}", s.indexed_files);
    println!("  Changed Files:  {}", s.changed_files);
    println!("  Active Symbols: {}", s.symbols);
    println!("  Removed Files:  {}", s.removed_files);
    println!("──────────────────────────────────────────");
    if s.changed_files == 0 {
        println!("  Status: Up to date (cache hit) ⚡");
    } else {
        println!("  Status: Analysis refreshed in WAL storage ✓");
    }
}

pub fn print_context_result(ctx: &ContextResult, explain: bool) {
    if explain {
        println!("{}", crate::compiler::format_explain(ctx));
        return;
    }

    println!("🧠 Task Context: \"{}\"", ctx.task);
    println!("──────────────────────────────────────────");
    println!("  Project:     {}", ctx.project);
    println!("  Confidence:  {}", ctx.confidence);
    println!("  Budget:      {} / {} tokens", ctx.estimated_tokens, ctx.token_budget);
    println!("  Files:       {}", ctx.files.len());
    println!("──────────────────────────────────────────");

    if !ctx.constraints.is_empty() {
        println!("\nActive Constitution Constraints:");
        for c in &ctx.constraints {
            println!("  ⚠️  {}", c);
        }
    }

    if let Some(guard) = &ctx.ecosystem_guard {
        println!("\n{}", guard);
    }

    if !ctx.files.is_empty() {
        println!("\nSelected Files:");
        for f in &ctx.files {
            println!("  • {} (score: {:.2}, Level {})", f.path, f.score, f.level);
            for p in &f.provenance {
                println!("      → {}", p);
            }
        }
    }

    if !ctx.gaps.is_empty() {
        println!("\nDetected Gaps & Ambiguities:");
        for g in &ctx.gaps {
            println!("  ❓ {}", g);
        }
    }
}

pub fn print_ecosystem(eco: &EcosystemReport) {
    println!("📦 Project Ecosystem Intelligence");
    println!("──────────────────────────────────────────");

    if !eco.frameworks.is_empty() {
        println!("\nFrameworks:");
        for p in &eco.frameworks {
            let res = p.resolved_version.as_deref().unwrap_or(&p.declared_version);
            println!("  • {} @ {} ({})", p.name, res, p.ecosystem);
        }
    }

    if !eco.libraries.is_empty() {
        println!("\nLibraries ({} total):", eco.libraries.len());
        for p in eco.libraries.iter().take(25) {
            let res = p.resolved_version.as_deref().unwrap_or(&p.declared_version);
            println!("  • {} @ {}", p.name, res);
        }
        if eco.libraries.len() > 25 {
            println!("    ... and {} more", eco.libraries.len() - 25);
        }
    }

    if !eco.dev_tools.is_empty() {
        println!("\nDev Tools & Compilers:");
        for p in &eco.dev_tools {
            let res = p.resolved_version.as_deref().unwrap_or(&p.declared_version);
            println!("  • {} @ {}", p.name, res);
        }
    }

    if !eco.scripts.is_empty() {
        println!("\nAvailable Scripts:");
        for (k, v) in &eco.scripts {
            println!("  • {}: {}", k, v);
        }
    }
}

pub fn print_impact(val: &Value) {
    if let Some(target) = val.get("target").and_then(|t| t.as_str()) {
        println!("🔍 Reverse Dependency Impact: '{}'", target);
        println!("──────────────────────────────────────────");
        if let Some(revs) = val.get("reverse_dependencies").and_then(|r| r.as_array()) {
            if revs.is_empty() {
                println!("  No dependent files or callers recorded in graph.");
            } else {
                for r in revs {
                    println!(
                        "  ← [{}] {} (evidence: {}, confidence: {})",
                        r["relationship"].as_str().unwrap_or(""),
                        r["source"].as_str().unwrap_or(""),
                        r["evidence"].as_str().unwrap_or(""),
                        r["confidence"]
                    );
                }
            }
        }
        return;
    }

    println!("🛡️ Change Impact Summary");
    println!("──────────────────────────────────────────");
    let actual_files = val["actual"]["files"].as_array().map(|a| a.len()).unwrap_or(0);
    let expected_files = val["expected"]["files"].as_array().map(|a| a.len()).unwrap_or(0);
    let unexpected = val["unexpected_files"].as_array().map(|a| a.len()).unwrap_or(0);

    println!("  Expected Files:  {}", expected_files);
    println!("  Actual Files:    {}", actual_files);
    println!("  Unexpected:      {}", unexpected);

    if let Some(warning) = val.get("warning").and_then(|w| w.as_str()) {
        println!("\n  ⚠️  {}", warning);
    } else {
        println!("\n  ✓ Changes stayed inside expected contract boundary.");
    }
}

pub fn print_validation(val: &Value) {
    println!("🛡️ Change Guard Validation");
    println!("──────────────────────────────────────────");

    let is_valid = val.get("valid").and_then(|v| v.as_bool()).unwrap_or(true);
    println!("  Advisory Check: {}", if is_valid { "PASSED" } else { "FAILED" });

    if let Some(violations) = val.get("rule_violations").and_then(|v| v.as_array()) {
        if !violations.is_empty() {
            println!("\nActive Constitution Rule Violations:");
            for v in violations {
                println!(
                    "  ⚠️  [{}] {}: {} ({}:{})",
                    v["severity"].as_str().unwrap_or(""),
                    v["rule_id"].as_str().unwrap_or(""),
                    v["message"].as_str().unwrap_or(""),
                    v["file"].as_str().unwrap_or(""),
                    v["line"]
                );
            }
        }
    }

    if let Some(overrides) = val.get("overridden_violations").and_then(|v| v.as_array()) {
        if !overrides.is_empty() {
            println!("\nAudited Overrides (Engineering Memory):");
            for o in overrides {
                println!(
                    "  ✓ [{}] {} overridden: \"{}\"",
                    o["rule_id"].as_str().unwrap_or(""),
                    o["file"].as_str().unwrap_or(""),
                    o["override_reason"].as_str().unwrap_or("")
                );
            }
        }
    }
}
