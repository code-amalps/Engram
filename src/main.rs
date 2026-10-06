pub mod analyzer;
pub mod compiler;
pub mod constitution;
pub mod graph;
pub mod guard;
pub mod manifest;
pub mod mcp;
pub mod model;
pub mod printer;
pub mod setup;
pub mod storage;

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

use analyzer::analyze;
use compiler::context;
use guard::{analyze_task, impact, make_contract, record_decision, update_memory, validate};
use manifest::detect_ecosystem;
use mcp::mcp;
use setup::{list_targets, setup_client};
use storage::{cache_dir, project_id, root_path};

#[derive(Parser)]
#[command(
    name = "engram",
    version,
    about = "Local engineering memory and change intelligence for AI coding agents"
)]
struct Cli {
    #[arg(short = 'C', long, global = true, default_value = ".")]
    root: PathBuf,

    /// Force machine-readable JSON output
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Option<CommandLine>,
}

#[derive(Subcommand)]
enum CommandLine {
    /// Initialize .engram configuration and rules in repository
    Init,
    /// Analyze repository structure, symbols, and dependencies
    #[command(alias = ".")]
    Analyze,
    /// Compile minimum sufficient context for a given task
    Context {
        task: String,
        #[arg(long, default_value_t = 1200)]
        token_budget: usize,
        #[arg(long)]
        explain: bool,
    },
    /// Ground a task, detect gaps, and generate task record
    AnalyzeTask {
        task: String,
        #[arg(long, default_value_t = 1200)]
        token_budget: usize,
    },
    /// Generate an advisory Change Contract for an analyzed task
    Contract {
        task_id: String,
    },
    /// Calculate change impact or query reverse dependencies for a symbol
    Impact {
        target: String,
    },
    /// Validate working-tree changes against architectural rules
    Validate {
        task_id: String,
    },
    /// Record a developer decision or rule override with rationale
    Decision {
        task_id: String,
        decision: String,
        reason: String,
        #[arg(long)]
        rule_id: Option<String>,
        #[arg(long)]
        violation_id: Option<String>,
    },
    /// Promote accepted task knowledge into local engineering memory
    UpdateMemory {
        task_id: String,
    },
    /// Inspect declared and resolved ecosystem packages and scripts
    Ecosystem,
    /// Configure 1-click MCP integrations for Cursor, Claude Desktop, or Windsurf
    Setup {
        client: Option<String>,
        #[arg(long)]
        list: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        remove: bool,
        #[arg(long)]
        status: bool,
    },
    /// Start the Model Context Protocol (MCP) server over stdio
    Mcp,
}

fn print_json(v: impl Serialize) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

fn init(root: &Path) -> Result<Value> {
    let root = root_path(root)?;
    let dir = root.join(".engram");
    fs::create_dir_all(&dir)?;
    let defaults = [
        (
            "constitution.yaml",
            "schema_version: 1\narchitecture:\n  pattern: modular-clean\nrules:\n  - id: RULE-001\n    name: \"No direct fetch in components\"\n    type: PATTERN\n    severity: WARNING\n    target_files: \"**/*.component.ts\"\n    pattern: \"fetch\\\\(\"\n    description: \"Use dedicated ApiService or state effects instead of direct window.fetch.\"\n  - id: RULE-002\n    name: \"Domain boundary isolation\"\n    type: DEPENDENCY\n    severity: ERROR\n    source: \"src/domain/**\"\n    forbidden: \"src/infrastructure/**\"\n    description: \"Domain entities must not depend on infrastructure implementations.\"\n",
        ),
        (
            "config.yaml",
            "schema_version: 1\nanalyzer_version: engram-0.1.0\nlanguages: [typescript, javascript, python, csharp]\ndefault_token_budget: 1200\nhistorical_half_life_commits: 200\n",
        ),
    ];
    for (name, text) in defaults {
        let p = dir.join(name);
        if !p.exists() {
            fs::write(p, text)?;
        }
    }
    let ig = root.join(".engramignore");
    if !ig.exists() {
        fs::write(
            ig,
            "# Categorized ignore patterns\nSECRET:.env\nSECRET:.env.*\nSECRET:*.pem\nSECRET:*.key\nVENDORED:node_modules/\nBUILD:dist/\nBUILD:build/\nBUILD:coverage/\nGENERATED:*.min.js\n",
        )?;
    }
    Ok(json!({
        "root": root,
        "project_id": project_id(&root),
        "cache": cache_dir(&root)?,
        "initialized": true
    }))
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let root = &cli.root;
    let force_json = cli.json;

    match cli.command {
        None | Some(CommandLine::Analyze) => {
            let res = analyze(root)?;
            if force_json {
                print_json(res)?;
            } else {
                printer::print_analysis_summary(&res);
            }
        }
        Some(CommandLine::Init) => print_json(init(root)?)?,
        Some(CommandLine::Context {
            task,
            token_budget,
            explain,
        }) => {
            let res = context(root, &task, token_budget)?;
            if force_json {
                print_json(res)?;
            } else {
                printer::print_context_result(&res, explain);
            }
        }
        Some(CommandLine::AnalyzeTask { task, token_budget }) => {
            let (id, ctx) = analyze_task(root, &task, token_budget)?;
            print_json(json!({ "task_id": id, "context": ctx }))?;
        }
        Some(CommandLine::Contract { task_id }) => print_json(make_contract(root, &task_id)?)?,
        Some(CommandLine::Impact { target }) => {
            let res = impact(root, &target)?;
            if force_json {
                print_json(res)?;
            } else {
                printer::print_impact(&res);
            }
        }
        Some(CommandLine::Validate { task_id }) => {
            let res = validate(root, &task_id)?;
            if force_json {
                print_json(res)?;
            } else {
                printer::print_validation(&res);
            }
        }
        Some(CommandLine::Decision {
            task_id,
            decision: choice,
            reason,
            rule_id,
            violation_id,
        }) => {
            print_json(record_decision(
                root,
                &task_id,
                &choice,
                &reason,
                rule_id.as_deref(),
                violation_id.as_deref(),
            )?)?;
        }
        Some(CommandLine::UpdateMemory { task_id }) => {
            print_json(update_memory(root, &task_id)?)?;
        }
        Some(CommandLine::Ecosystem) => {
            let report = detect_ecosystem(root)?;
            if force_json {
                print_json(report)?;
            } else {
                printer::print_ecosystem(&report);
            }
        }
        Some(CommandLine::Setup {
            client,
            list,
            dry_run,
            remove,
            status,
        }) => {
            if list || client.is_none() {
                let targets = list_targets();
                if force_json {
                    print_json(targets)?;
                } else {
                    println!("Supported integrations:\n  ✓ Cursor (.cursor/mcp.json)\n  ✓ Claude Desktop (claude_desktop_config.json)\n  ✓ Windsurf (mcp_config.json)\n\nRoadmap integrations:\n  ○ Claude Code CLI\n  ○ VS Code / Cline\n  ○ VS Code / Roo Code\n\nUsage: engram setup <cursor|claude|windsurf> [--dry-run] [--status] [--remove]");
                }
            } else if let Some(c) = client {
                let res = setup_client(root, &c, dry_run, remove, status)?;
                print_json(res)?;
            }
        }
        Some(CommandLine::Mcp) => mcp(root)?,
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("engram: {e:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use analyzer::{
        estimate_tokens, language, matches_pattern, parse_file, redact, tokens,
    };
    use model::{Evidence, EvidenceTier, analysis_key};

    #[test]
    fn test_secret_redaction() {
        let input = "export API_KEY='sk-1234567890abcdef1234567890' and password='mysecretpassword'";
        let redacted = redact(input);
        assert!(!redacted.contains("sk-1234567890abcdef1234567890"));
        assert!(!redacted.contains("mysecretpassword"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_analysis_key_composition() {
        let key1 = analysis_key("hash_a", "0.1.0", "ts-0.25", "python", "cfg_1");
        let key2 = analysis_key("hash_a", "0.1.0", "ts-0.25", "python", "cfg_1");
        let key3 = analysis_key("hash_b", "0.1.0", "ts-0.25", "python", "cfg_1");
        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_evidence_decay_and_precedence() {
        let ev = Evidence {
            evidence_id: "ev_1".into(),
            target_id: "ent_1".into(),
            target_type: "SYMBOL".into(),
            tier: EvidenceTier::Historical,
            confidence: 1.0,
            source_type: "COMMIT_DELTA".into(),
            source_path: None,
            source_line: None,
            recorded_at: chrono::Utc::now(),
            decay_half_life_commits: Some(200),
            verification_status: "ACTIVE".into(),
            details: std::collections::BTreeMap::new(),
        };
        assert!((ev.effective_confidence(0) - 1.0).abs() < 1e-6);
        assert!((ev.effective_confidence(200) - 0.5).abs() < 1e-6);
        assert!((ev.effective_confidence(400) - 0.25).abs() < 1e-6);
        assert!(EvidenceTier::Explicit.precedence() > EvidenceTier::Verified.precedence());
        assert!(EvidenceTier::Verified.precedence() > EvidenceTier::Extracted.precedence());
        assert!(EvidenceTier::Extracted.precedence() > EvidenceTier::Historical.precedence());
        assert!(EvidenceTier::Historical.precedence() > EvidenceTier::Inferred.precedence());
        assert!(EvidenceTier::Inferred.precedence() > EvidenceTier::Provisional.precedence());
    }

    #[test]
    fn test_tokenization_and_estimation() {
        let s = "function calculateOrderTotal(subtotal: number, taxRate: number) { return subtotal * (1 + taxRate); }";
        let tok = tokens(s);
        assert!(tok.contains(&"calculateordertotal".to_string()));
        assert!(tok.contains(&"subtotal".to_string()));
        let est = estimate_tokens(s);
        assert!(est > 10 && est < 50);
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(language(Path::new("app.ts")), Some("typescript"));
        assert_eq!(language(Path::new("app.tsx")), Some("typescript"));
        assert_eq!(language(Path::new("script.js")), Some("javascript"));
        assert_eq!(language(Path::new("module.py")), Some("python"));
        assert_eq!(language(Path::new("Program.cs")), Some("csharp"));
        assert_eq!(language(Path::new("unknown.xyz")), None);
    }

    #[test]
    fn test_matches_pattern() {
        assert!(matches_pattern("src/main.ts", "*.ts"));
        assert!(matches_pattern("src/app/core.ts", "src/**"));
        assert!(matches_pattern(".env.local", ".env.*"));
        assert!(!matches_pattern("src/main.ts", "*.js"));
    }

    #[test]
    fn test_tree_sitter_python_extraction() {
        let code = b"class OrderProcessor:\n    def process_order(self, order_id):\n        pass\n";
        let (record, symbols, _) = parse_file(Path::new("."), "order.py", code, "rev1").unwrap();
        assert_eq!(record.language, "python");
        assert!(symbols.iter().any(|s| s.name == "OrderProcessor" && s.kind == "CLASS"));
        assert!(symbols.iter().any(|s| s.name == "process_order" && s.kind == "FUNCTION"));
    }

    #[test]
    fn test_tree_sitter_typescript_extraction() {
        let code = b"export interface IOrder { id: string; }\nexport class OrderService implements IOrder {\n  getOrder(id: string) { return id; }\n}\n";
        let (record, symbols, _) = parse_file(Path::new("."), "order.ts", code, "rev1").unwrap();
        assert_eq!(record.language, "typescript");
        assert!(symbols.iter().any(|s| s.name == "IOrder" && s.kind == "INTERFACE"));
        assert!(symbols.iter().any(|s| s.name == "OrderService" && s.kind == "CLASS"));
        assert!(symbols.iter().any(|s| s.name == "getOrder" && s.kind == "METHOD"));
    }

    #[test]
    fn test_tree_sitter_csharp_extraction() {
        let code = b"namespace Shop {\n    public class CartService {\n        public void Checkout() {}\n    }\n}\n";
        let (record, symbols, _) = parse_file(Path::new("."), "CartService.cs", code, "rev1").unwrap();
        assert_eq!(record.language, "csharp");
        assert!(symbols.iter().any(|s| s.name == "CartService" && s.kind == "CLASS"));
        assert!(symbols.iter().any(|s| s.name == "Checkout" && s.kind == "METHOD"));
    }
}
