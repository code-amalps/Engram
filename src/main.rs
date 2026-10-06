pub mod analyzer;
pub mod compiler;
pub mod guard;
pub mod mcp;
pub mod model;
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
use guard::{analyze_task, decision, impact, make_contract, update_memory, validate};
use mcp::mcp;
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
    /// Calculate actual change impact vs expected contract boundary
    Impact {
        task_id: String,
    },
    /// Validate working-tree changes against architectural rules
    Validate {
        task_id: String,
    },
    /// Record a developer decision or override with rationale
    Decision {
        task_id: String,
        decision: String,
        reason: String,
    },
    /// Promote accepted task knowledge into local engineering memory
    UpdateMemory {
        task_id: String,
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
            "architecture:\n  pattern: unspecified\nrules: []\nconventions: {}\ntesting:\n  service_changes_require_unit_tests: false\nsecurity:\n  authentication_required: []\n",
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

    // Default to running analyze if no subcommand is provided (e.g. `engram .` or `engram`)
    match cli.command {
        None | Some(CommandLine::Analyze) => print_json(analyze(root)?),
        Some(CommandLine::Init) => print_json(init(root)?),
        Some(CommandLine::Context { task, token_budget }) => {
            print_json(context(root, &task, token_budget)?)
        }
        Some(CommandLine::AnalyzeTask { task, token_budget }) => {
            let (id, ctx) = analyze_task(root, &task, token_budget)?;
            print_json(json!({ "task_id": id, "context": ctx }))
        }
        Some(CommandLine::Contract { task_id }) => print_json(make_contract(root, &task_id)?),
        Some(CommandLine::Impact { task_id }) => print_json(impact(root, &task_id)?),
        Some(CommandLine::Validate { task_id }) => print_json(validate(root, &task_id)?),
        Some(CommandLine::Decision {
            task_id,
            decision: choice,
            reason,
        }) => print_json(decision(root, &task_id, &choice, &reason)?),
        Some(CommandLine::UpdateMemory { task_id }) => {
            print_json(update_memory(root, &task_id)?)
        }
        Some(CommandLine::Mcp) => mcp(root),
    }
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
    use chrono::Utc;
    use model::{Evidence, EvidenceTier, analysis_key};
    use std::collections::BTreeMap;

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
        let key3 = analysis_key("hash_a", "0.1.1", "ts-0.25", "python", "cfg_1");
        assert_eq!(key1, key2);
        assert_ne!(key1, key3);
        assert!(key1.starts_with("sha256:"));
    }

    #[test]
    fn test_evidence_decay_and_precedence() {
        assert!(EvidenceTier::Explicit.precedence() > EvidenceTier::Verified.precedence());
        assert!(EvidenceTier::Verified.precedence() > EvidenceTier::Extracted.precedence());
        assert!(EvidenceTier::Extracted.precedence() > EvidenceTier::Historical.precedence());
        assert!(EvidenceTier::Historical.precedence() > EvidenceTier::Inferred.precedence());
        assert!(EvidenceTier::Inferred.precedence() > EvidenceTier::Provisional.precedence());

        let ev = Evidence {
            evidence_id: "test".into(),
            target_id: "rel_1".into(),
            target_type: "RELATIONSHIP".into(),
            tier: EvidenceTier::Historical,
            confidence: 0.8,
            source_type: "GIT".into(),
            source_path: None,
            source_line: None,
            recorded_at: Utc::now(),
            decay_half_life_commits: Some(200),
            verification_status: "UNCONTESTED".into(),
            details: BTreeMap::new(),
        };

        let conf_now = ev.effective_confidence(0);
        assert!((conf_now - 0.8).abs() < 1e-6);

        let conf_after_200 = ev.effective_confidence(200);
        assert!((conf_after_200 - 0.4).abs() < 1e-6);

        let conf_after_400 = ev.effective_confidence(400);
        assert!((conf_after_400 - 0.2).abs() < 1e-6);
    }

    #[test]
    fn test_language_detection() {
        assert_eq!(language(Path::new("app/main.py")), Some("python"));
        assert_eq!(language(Path::new("src/index.ts")), Some("typescript"));
        assert_eq!(language(Path::new("src/comp.tsx")), Some("typescript"));
        assert_eq!(language(Path::new("src/app.js")), Some("javascript"));
        assert_eq!(language(Path::new("Services/Order.cs")), Some("csharp"));
        assert_eq!(language(Path::new("Cargo.toml")), None);
    }

    #[test]
    fn test_tokenization_and_estimation() {
        let text = "OrderService.ExportCsv() processes orders";
        let toks = tokens(text);
        assert!(toks.contains(&"orderservice".to_string()));
        assert!(toks.contains(&"exportcsv".to_string()));
        assert!(toks.contains(&"processes".to_string()));
        assert!(estimate_tokens(text) > 0);
    }

    #[test]
    fn test_tree_sitter_python_extraction() {
        let code = b"class OrderService:\n    def export_csv(self, order_id: str):\n        return True\n";
        let temp_dir = tempfile_dir("test_py");
        let (file, symbols, _) = parse_file(&temp_dir, "order_service.py", code, "rev1").unwrap();
        assert_eq!(file.language, "python");
        let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"OrderService"));
        assert!(names.contains(&"export_csv"));
    }

    #[test]
    fn test_tree_sitter_typescript_extraction() {
        let code = b"export interface IOrderService {\n    exportCsv(id: string): Promise<void>;\n}\nexport function processOrder() {}\n";
        let temp_dir = tempfile_dir("test_ts");
        let (file, symbols, _) = parse_file(&temp_dir, "orders.ts", code, "rev1").unwrap();
        assert_eq!(file.language, "typescript");
        let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"IOrderService"));
        assert!(names.contains(&"processOrder"));
    }

    #[test]
    fn test_tree_sitter_csharp_extraction() {
        let code = b"namespace App.Orders;\npublic class OrderService {\n    public void ExportCsv() {}\n}\n";
        let temp_dir = tempfile_dir("test_cs");
        let (file, symbols, _) = parse_file(&temp_dir, "OrderService.cs", code, "rev1").unwrap();
        assert_eq!(file.language, "csharp");
        let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"OrderService"));
        assert!(names.contains(&"ExportCsv"));
    }

    #[test]
    fn test_matches_pattern() {
        assert!(matches_pattern("dist/bundle.js", "dist/*"));
        assert!(matches_pattern("src/generated/api.ts", "**/generated/**"));
        assert!(matches_pattern(".env", ".env"));
        assert!(matches_pattern("src/controllers/order.ts", "*.ts"));
        assert!(!matches_pattern("src/controllers/order.ts", "*.py"));
    }

    fn tempfile_dir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("engram_test_{}_{}", name, uuid::Uuid::new_v4()));
        let _ = fs::create_dir_all(&p);
        p
    }
}
