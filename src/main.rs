pub mod analyzer;
pub mod archaeology;
pub mod bench;
pub mod compiler;
pub mod constitution;
pub mod contract;
pub mod doctor;
pub mod drift;
pub mod flywheel;
pub mod gc;
pub mod git;
pub mod git_history;
pub mod go_parser;
pub mod java_parser;
pub mod graph;
pub mod guard;
pub mod impact;
pub mod incremental;
pub mod manifest;
pub mod mcp;
pub mod memory;
pub mod memory_graph;
pub mod model;
pub mod printer;
pub mod instructions;
pub mod prompt;
pub mod recovery;
pub mod setup;
pub mod storage;
pub mod telemetry;

use anyhow::{Result, anyhow};
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
    Analyze {
        /// Use two-tier incremental invalidation engine
        #[arg(long)]
        incremental: bool,
    },
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
    /// Manage elastic Change Contracts
    Contract {
        #[command(subcommand)]
        action: Option<ContractAction>,
        /// Task ID or goal (shortcut)
        task_id: Option<String>,
    },
    /// Manage durable engineering memory (.engram/memory.yaml)
    Memory {
        #[command(subcommand)]
        action: MemoryAction,
    },
    /// Mine and query Git historical co-change patterns
    History {
        #[command(subcommand)]
        action: HistoryAction,
    },
    /// Calculate change impact or query reverse dependencies for a symbol
    Impact {
        target: String,
    },
    /// Deeply inspect symbol change impact, causal provenance, active contract status, and memories
    Inspect {
        target: String,
        #[arg(long, default_value_t = 2)]
        depth: usize,
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
    /// Configure and inspect editor and MCP integrations (Cursor, VS Code, Claude, Windsurf)
    Setup {
        client: Option<String>,
        #[arg(long)]
        list: bool,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        apply: bool,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        status: bool,
    },
    /// Generate structured prompt context with AST symbols, boundaries, and completeness tracking
    Prompt {
        task: String,
        #[arg(long, default_value_t = 1200)]
        token_budget: usize,
        #[arg(long, default_value = "markdown")]
        format: String,
        #[arg(long)]
        stdout: bool,
        #[arg(long)]
        no_clipboard: bool,
    },
    /// Start the Model Context Protocol (MCP) server over stdio
    Mcp,
    /// Run scientific empirical benchmarks evaluating discovery, grounding, contracts, and memory lift
    Bench {
        #[arg(long, short = 'v')]
        verbose: bool,
        #[arg(long)]
        external: Option<std::path::PathBuf>,
    },
    /// Answer why code was designed this way, with active policy, historical evolution, and provenance
    Why {
        target: String,
    },
    /// Inspect memory liveness, scope resolution, and knowledge health without mutating state
    Doctor,
    /// Controlled garbage collection with dry-run inspection and explicit operational flags
    Gc {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        candidates: bool,
        #[arg(long)]
        archive: bool,
        #[arg(long)]
        vacuum: bool,
    },
    /// Orchestrated Pre-Flight Handshake: compile context, predict blast radius, discover decisions, lock contract
    Preflight {
        task: String,
        #[arg(long, default_value_t = 1200)]
        token_budget: usize,
        #[arg(long)]
        forbidden: Option<Vec<String>>,
    },
    /// Orchestrated Post-Flight Guard: evaluate mutation against contract and constitution, generate candidates
    Postflight {
        #[arg(long)]
        contract_id: Option<String>,
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        staged: bool,
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        ci: bool,
    },
    /// Formulate structured recovery repair plan for a contract violation
    Recover {
        /// Change Contract ID to recover (defaults to active contract if omitted)
        contract_id: Option<String>,
    },
    /// Analyze architectural decision drift, boundary erosion, and scoped staleness
    Drift {
        /// Filter drift analysis to a specific module
        #[arg(long)]
        module: Option<String>,
        /// Filter drift analysis to a specific decision ID
        #[arg(long)]
        decision: Option<String>,
        /// Commit hash to calculate drift since
        #[arg(long)]
        since: Option<String>,
        /// Configured mutation threshold for STALE_REVIEW (default 5)
        #[arg(long, default_value_t = drift::DEFAULT_STALENESS_THRESHOLD, alias = "threshold")]
        staleness_threshold: usize,
    },
}

#[derive(Subcommand)]
enum ContractAction {
    /// Create an elastic Change Contract
    Create {
        task: String,
        #[arg(long)]
        modules: Option<Vec<String>>,
        #[arg(long)]
        files: Option<Vec<String>>,
        #[arg(long)]
        forbidden: Option<Vec<String>>,
    },
    /// Check working tree, staged index, or PR diff against active Change Contract
    Check {
        contract_id: Option<String>,
        #[arg(long)]
        strict: bool,
        #[arg(long)]
        staged: bool,
        #[arg(long)]
        base: Option<String>,
        #[arg(long)]
        check_provenance: bool,
    },
    /// Ratify a Change Contract boundary expansion with human authorization
    Ratify {
        contract_id: String,
        #[arg(long, default_value = "developer")]
        by: String,
        #[arg(long, default_value = "Human ratified boundary expansion")]
        reason: String,
    },
    /// Reconcile contract with human approval/additions
    Reconcile {
        contract_id: String,
        reason: String,
        #[arg(long)]
        actor: Option<String>,
        #[arg(long)]
        add_files: Vec<String>,
        #[arg(long)]
        add_modules: Vec<String>,
    },
    /// Install pre-commit hook into .git/hooks/pre-commit
    InstallHook {
        #[arg(long)]
        dry_run: bool,
    },
    /// Uninstall pre-commit hook from .git/hooks/pre-commit
    UninstallHook {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum MemoryAction {
    /// List all durable engineering memories
    List,
    /// Record an architectural decision and rationale into durable memory
    Record {
        title: String,
        statement: String,
        reason: String,
        #[arg(long)]
        modules: Vec<String>,
        #[arg(long)]
        files: Vec<String>,
        #[arg(long)]
        accepted_by: Option<String>,
    },
    /// List pending memory candidates awaiting review
    Candidates,
    /// Promote a candidate to durable memory after review
    Promote {
        candidate_id: String,
        #[arg(long)]
        accepted_by: Option<String>,
        #[arg(long)]
        reason: Option<String>,
    },
    /// Reject a candidate, preventing promotion
    Reject {
        candidate_id: String,
        reason: String,
        #[arg(long)]
        rejected_by: Option<String>,
    },
    /// Re-ratify an architectural decision with current or specified commit and reviewer name
    #[command(name = "re-ratify")]
    ReRatify {
        decision_id: String,
        #[arg(long)]
        commit: Option<String>,
        #[arg(long, default_value = "developer")]
        by: String,
    },
}

#[derive(Subcommand)]
enum HistoryAction {
    /// Query co-change signals for a file
    Cochanges {
        file: String,
        #[arg(long, default_value_t = 0.25)]
        min_probability: f64,
    },
    /// Mine and cache Git co-changes
    Mine {
        #[arg(long, default_value_t = 200)]
        window: usize,
    },
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
            &format!(
                "schema_version: 1\nanalyzer_version: engram-{}\nlanguages: [typescript, javascript, python, csharp, rust, go, java]\ndefault_token_budget: 1200\nhistorical_half_life_commits: 200\n",
                env!("CARGO_PKG_VERSION")
            ),
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
        None => {
            let res = analyze(root)?;
            if force_json {
                print_json(res)?;
            } else {
                printer::print_analysis_summary(&res);
            }
        }
        Some(CommandLine::Analyze { incremental }) => {
            if incremental {
                let res = incremental::incremental_analyze(root)?;
                if force_json {
                    print_json(&res)?;
                } else {
                    printer::print_incremental_summary(&res);
                }
            } else {
                let res = analyze(root)?;
                if force_json {
                    print_json(res)?;
                } else {
                    printer::print_analysis_summary(&res);
                }
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
        Some(CommandLine::Contract { action, task_id }) => {
            match action {
                Some(ContractAction::Create { task, modules, files, forbidden }) => {
                    let c = contract::create_contract(root, &task, modules, files, forbidden)?;
                    print_json(c)?;
                }
                Some(ContractAction::Check {
                    contract_id,
                    strict,
                    staged,
                    base,
                    check_provenance,
                }) => {
                    let options = contract::CheckOptions {
                        contract_id,
                        strict,
                        staged,
                        base_commit: base,
                        check_provenance,
                    };
                    let result = contract::check_contract(root, &options);
                    if force_json {
                        print_json(&result)?;
                    } else {
                        let is_github_ci = std::env::var("GITHUB_ACTIONS").is_ok()
                            || std::env::var("CI").is_ok();
                        print!("{}", contract::format_check_report(&result, is_github_ci));
                    }
                    if result.exit_code != 0 {
                        std::process::exit(result.exit_code);
                    }
                }
                Some(ContractAction::Ratify {
                    contract_id,
                    by,
                    reason,
                }) => {
                    let c = contract::ratify_contract(root, &contract_id, &by, &reason)?;
                    if force_json {
                        print_json(&c)?;
                    } else {
                        println!("✓ Ratified Change Contract '{}' by {}", c.contract_id, by);
                    }
                }
                Some(ContractAction::Reconcile { contract_id, reason, actor, add_files, add_modules }) => {
                    let act = actor.as_deref().unwrap_or("developer");
                    let c = contract::reconcile_contract(root, &contract_id, &reason, act, add_files, add_modules)?;
                    print_json(c)?;
                }
                Some(ContractAction::InstallHook { dry_run }) => {
                    let msg = contract::install_pre_commit_hook(root, dry_run)?;
                    println!("{msg}");
                }
                Some(ContractAction::UninstallHook { dry_run }) => {
                    let msg = contract::uninstall_pre_commit_hook(root, dry_run)?;
                    println!("{msg}");
                }
                None => {
                    if let Some(id) = task_id {
                        print_json(make_contract(root, &id)?)?;
                    } else if let Ok(Some(active)) = contract::get_active_contract(root) {
                        print_json(active)?;
                    } else {
                        println!("No active contract found. Create one with: engram contract create \"<goal>\"");
                    }
                }
            }
        }
        Some(CommandLine::Memory { action }) => {
            match action {
                MemoryAction::List => {
                    let doc = memory::load_durable_memories(root)?;
                    if force_json {
                        print_json(doc)?;
                    } else {
                        println!("🧠 Durable Engineering Memory (.engram/memory.yaml)");
                        println!("──────────────────────────────────────────");
                        if doc.memories.is_empty() {
                            println!("  No durable memories recorded yet.");
                        }
                        for m in doc.memories {
                            println!("  ✓ [{}] {}", m.id, m.title);
                            println!("    Statement: {}", m.statement);
                            println!("    Reason:    {}", m.reason);
                            println!("    Accepted:  {} by {}", m.accepted_at.format("%Y-%m-%d"), m.accepted_by);
                            println!();
                        }
                    }
                }
                MemoryAction::Record { title, statement, reason, modules, files, accepted_by } => {
                    let act = accepted_by.as_deref().unwrap_or("developer");
                    let m = memory::record_decision(root, &title, &statement, &reason, modules, files, None, act)?;
                    print_json(m)?;
                }
                MemoryAction::Candidates => {
                    let cands = memory::list_candidates(root)?;
                    print_json(cands)?;
                }
                MemoryAction::Promote { candidate_id, accepted_by, reason } => {
                    let act = accepted_by.as_deref().unwrap_or("developer");
                    let m = memory::promote_candidate(root, &candidate_id, act, reason.as_deref())?;
                    print_json(m)?;
                }
                MemoryAction::Reject { candidate_id, reason, rejected_by } => {
                    let act = rejected_by.as_deref().unwrap_or("developer");
                    memory::reject_candidate(root, &candidate_id, act, &reason)?;
                    print_json(json!({"rejected": true, "candidate_id": candidate_id}))?;
                }
                MemoryAction::ReRatify { decision_id, commit, by } => {
                    let mem = memory::re_ratify_decision(root, &decision_id, commit.as_deref(), &by)?;
                    if force_json {
                        print_json(&mem)?;
                    } else {
                        println!("✓ Decision '{}' successfully re-ratified.", mem.id);
                        println!(
                            "  Ratified at commit: {}",
                            mem.ratified_at_commit.as_deref().unwrap_or("WORKTREE")
                        );
                        println!(
                            "  Ratified by:        {}",
                            mem.ratified_by.as_deref().unwrap_or(&by)
                        );
                        println!("  Lifecycle State:    {:?}", mem.lifecycle_state);
                    }
                }
            }
        }
        Some(CommandLine::History { action }) => {
            match action {
                HistoryAction::Cochanges { file, min_probability } => {
                    let sigs = git_history::get_cochanges_for_file(root, &file, min_probability)?;
                    if force_json {
                        print_json(sigs)?;
                    } else {
                        println!("⚡ Historical Co-Change Signals for: {file}");
                        println!("──────────────────────────────────────────");
                        if sigs.is_empty() {
                            println!("  No significant co-changes detected in historical commit window.");
                        }
                        for s in sigs {
                            println!("  • {}", s.advisory_message);
                        }
                    }
                }
                HistoryAction::Mine { window } => {
                    let count = git_history::mine_and_persist_cochanges(root, window)?;
                    if force_json {
                        print_json(json!({"cochange_edges_mined": count, "window": window}))?;
                    } else {
                        println!("✓ Mined and cached {} co-change relationships from the last {} commits.", count, window);
                    }
                }
            }
        }
        Some(CommandLine::Impact { target }) => {
            let res = impact(root, &target)?;
            if force_json {
                print_json(res)?;
            } else {
                printer::print_impact(&res);
            }
        }
        Some(CommandLine::Inspect { target, depth }) => {
            analyzer::ensure_fresh(root)?;
            let conn = storage::db(root)?;
            let cone = impact::compute_impact_cone(&conn, root, &target, depth)?;
            let active_contract = contract::get_active_contract(root).ok().flatten();
            let authorization = active_contract.as_ref().map(|c| {
                contract::evaluate_impact_against_contract(c, &cone)
            });
            let affected_paths = cone.all_affected_paths();
            let audit_memories = memory::query_scoped_memories(root, &target, &affected_paths, std::slice::from_ref(&target), true).unwrap_or_default();
            let memory_conflicts = memory::detect_memory_conflicts(&audit_memories);

            if force_json {
                print_json(json!({
                    "target": cone.target_symbol,
                    "target_file": cone.target_file,
                    "impact_cone": cone,
                    "active_contract": active_contract,
                    "authorization": authorization,
                    "memories": audit_memories,
                    "memory_conflicts": memory_conflicts,
                }))?;
            } else {
                print!("{}", impact::format_impact_explanation(&cone));
                if let Some(ref c) = active_contract {
                    println!("\n📜 CONTRACT STATUS");
                    println!("  Active Contract: {} (v{}, Status: {:?})", c.contract_id, c.version, c.status);
                    if let Some(ref auth) = authorization {
                        if !auth.authorized_targets.is_empty() {
                            println!("  Authorized Targets (Expected):");
                            for t in &auth.authorized_targets {
                                println!("    ✓ {}", t);
                            }
                        }
                        if !auth.authorized_expansions.is_empty() {
                            println!("  Authorized Expansions (Allowed Modules):");
                            for e in &auth.authorized_expansions {
                                println!("    ✓ {}", e);
                            }
                        }
                        if !auth.unauthorized_relevant.is_empty() {
                            println!("  ⚠️ UNAUTHORIZED RELEVANT (Predicted in cone, but outside contract):");
                            for u in &auth.unauthorized_relevant {
                                println!("    • {}", u);
                            }
                        }
                        if !auth.forbidden_impacts.is_empty() {
                            println!("  🚫 FORBIDDEN BOUNDARY VIOLATIONS:");
                            for f in &auth.forbidden_impacts {
                                println!("    ❌ {}", f);
                            }
                        }
                    }
                } else {
                    println!("\n📜 CONTRACT STATUS");
                    println!("  No active Change Contract found.");
                }

                if !audit_memories.is_empty() {
                    println!("\n🧠 MEMORY");
                    for (idx, m) in audit_memories.iter().enumerate() {
                        let is_last = idx == audit_memories.len() - 1;
                        let branch = if is_last { "└──" } else { "├──" };
                        let pipe = if is_last { "    " } else { "│   " };
                        let target_desc = m.scope.target.as_deref().unwrap_or(match m.scope.level {
                            memory::ScopeLevel::Global => "Global",
                            memory::ScopeLevel::Repository => "Repo",
                            _ => "Project",
                        });
                        println!("  {} {} ({})", branch, m.id, target_desc);
                        println!("  {}  Scope:     {:?}", pipe, m.scope.level);
                        match m.lifecycle_state {
                            memory::MemoryLifecycleState::Superseded => {
                                let superseding = m.superseded_by.as_deref().unwrap_or("newer policy");
                                println!("  {}  Status:    SUPERSEDED (by {})", pipe, superseding);
                            }
                            memory::MemoryLifecycleState::Durable => {
                                if let Some(ref old) = m.supersedes {
                                    println!("  {}  Status:    DURABLE (supersedes {})", pipe, old);
                                } else {
                                    println!("  {}  Status:    DURABLE", pipe);
                                }
                            }
                            memory::MemoryLifecycleState::StaleReview => {
                                println!("  {}  Status:    STALE_REVIEW (re-ratification required)", pipe);
                            }
                            _ => {
                                println!("  {}  Status:    {:?}", pipe, m.lifecycle_state);
                            }
                        }
                        println!("  {}  Statement: {}", pipe, m.statement);
                        println!("  {}  Reason:    {}", pipe, m.reason);
                        if !is_last {
                            println!("  │");
                        }
                    }
                }

                if !memory_conflicts.is_empty() {
                    println!("\n⚠️ MEMORY CONFLICTS / SHADOWING");
                    for c in &memory_conflicts {
                        println!("  • [{:?}] {}", c.conflict_type, c.explanation);
                        println!("    Resolution: {}", c.resolution);
                    }
                }
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
            check,
            dry_run,
            apply,
            force,
            status,
        }) => {
            if list {
                if force_json {
                    print_json(json!({
                        "supported": [
                            "cursor (Cursor IDE: .cursor/mcp.json + .cursorrules)",
                            "vscode (VS Code: .vscode/mcp.json + .github/copilot-instructions.md)",
                            "claude (Claude Desktop: claude_desktop_config.json + CLAUDE.md)",
                            "windsurf (Windsurf IDE: mcp_config.json + .windsurfrules)"
                        ]
                    }))?;
                } else {
                    println!("🧠 Engram Supported Integrations:");
                    println!("  ✓ Cursor:   .cursor/mcp.json               | .cursorrules");
                    println!("  ✓ VS Code:  .vscode/mcp.json               | .github/copilot-instructions.md");
                    println!("  ✓ Claude:   claude_desktop_config.json     | CLAUDE.md");
                    println!("  ✓ Windsurf: mcp_config.json                | .windsurfrules");
                    println!("\nCommands:");
                    println!("  engram setup                   # Read-only auto-detection and dry-run plan");
                    println!("  engram setup --check           # Verify health of configured integrations");
                    println!("  engram setup --apply           # Apply proposed changes safely");
                    println!("  engram setup <target> --apply  # Configure specific editor");
                }
            } else {
                let is_dry_run = dry_run || (!apply && !check && !status);
                let opts = setup::SetupOptions {
                    client,
                    check,
                    dry_run: is_dry_run,
                    apply,
                    force,
                    status,
                };
                let report = setup::orchestrate_setup(root, opts)?;
                if force_json || status {
                    print_json(&report)?;
                } else {
                    println!("🧠 Engram Integration Setup ({})", report.mode);
                    println!("──────────────────────────────────────────────────────────");
                    if report.integrations.is_empty() {
                        println!("  No matching or detected editor configurations found.");
                        println!("  Run `engram setup --list` to view all supported targets.");
                    }
                    for item in &report.integrations {
                        let icon = match item.config_status.as_str() {
                            "CONFIGURED_HEALTHY" => "✓",
                            "CONFLICT_DETECTED" => "⚠️",
                            _ => "○",
                        };
                        println!("  {} {} ({:?})", icon, item.display_name, item.detection);
                        println!("     Config:      {} [{}]", item.config_path, item.config_status);
                        if let Some(ref ip) = item.instruction_path {
                            let istat = item.instruction_status.as_deref().unwrap_or("none");
                            println!("     Instruction: {} [{}]", ip, istat);
                        }
                        if let Some(ref conflict) = item.conflict_details {
                            println!("     Conflict:    {}", conflict);
                        }
                    }
                    println!("──────────────────────────────────────────────────────────");
                    if is_dry_run && !apply {
                        println!("  ℹ️ DRY-RUN ONLY. No files were modified.");
                        println!("  Run `engram setup --apply` to execute this integration plan.");
                    } else if apply {
                        println!("  ✓ Applied {} integration changes (0 data loss).", report.changes_applied);
                    }
                }
            }
        }
        Some(CommandLine::Prompt {
            task,
            token_budget,
            format,
            stdout,
            no_clipboard,
        }) => {
            let fmt = match format.to_lowercase().as_str() {
                "text" => prompt::PromptFormat::Text,
                _ => prompt::PromptFormat::Markdown,
            };
            let copy_clip = !no_clipboard && !stdout;
            let res = prompt::generate_prompt(root, &task, token_budget, fmt, copy_clip)?;
            if force_json {
                print_json(&res)?;
            } else {
                if stdout || !res.clipboard_copied {
                    println!("{}", res.content);
                }
                if res.clipboard_copied {
                    eprintln!("✓ Copied ~{} tokens of structured AST context to clipboard.", res.estimated_tokens);
                    eprintln!("  Status: {}", res.completeness_status);
                    eprintln!("  Paste directly into your AI chat box (Cursor, Copilot, Visual Studio, ChatGPT).");
                }
            }
        }
        Some(CommandLine::Mcp) => mcp(root)?,
        Some(CommandLine::Bench { verbose, external }) => {
            if let Some(ext_path) = external {
                let results = bench::run_external_benchmark(&ext_path)?;
                if force_json {
                    print_json(results)?;
                } else {
                    print!("{}", bench::format_external_benchmark_report(&results));
                }
            } else {
                let rev = storage::revision(root);
                let results = bench::run_benchmark_suite(&rev)?;
                if force_json {
                    print_json(results)?;
                } else {
                    print!("{}", bench::format_human_benchmark_report(&results, verbose));
                }
            }
        }
        Some(CommandLine::Why { target }) => {
            analyzer::ensure_fresh(root)?;
            let expl = archaeology::explain_why(root, &target)?;
            if force_json {
                print_json(expl)?;
            } else {
                print!("{}", archaeology::format_archaeology_explanation(&expl));
            }
        }
        Some(CommandLine::Doctor) => {
            let report = doctor::diagnose_memory_health(root)?;
            if force_json {
                print_json(report)?;
            } else {
                print!("{}", doctor::format_doctor_report(&report));
            }
        }
        Some(CommandLine::Gc {
            dry_run,
            candidates,
            archive,
            vacuum,
        }) => {
            let options = gc::GcOptions {
                dry_run,
                prune_candidates: candidates || (!archive && !vacuum),
                archive_superseded: archive || (!candidates && !vacuum),
                vacuum: vacuum || (!candidates && !archive),
            };
            let result = gc::execute_gc(root, &options)?;
            if force_json {
                print_json(result)?;
            } else {
                print!("{}", gc::format_gc_plan(&result));
            }
        }
        Some(CommandLine::Preflight { task, token_budget, forbidden }) => {
            let opts = flywheel::PreflightOptions {
                task,
                token_budget,
                forbidden_boundaries: forbidden,
            };
            let res = flywheel::preflight_task(root, &opts)?;
            if force_json {
                print_json(res)?;
            } else {
                print!("{}", flywheel::format_preflight_report(&res));
            }
        }
        Some(CommandLine::Postflight { contract_id, strict, staged, base, ci }) => {
            let opts = flywheel::PostflightOptions {
                contract_id,
                strict,
                staged,
                base_commit: base,
                ci_mode: ci,
            };
            let res = flywheel::postflight_check(root, &opts)?;
            if force_json {
                print_json(&res)?;
            } else {
                print!("{}", flywheel::format_postflight_report(&res, ci));
            }
            if res.exit_code != 0 {
                std::process::exit(res.exit_code);
            }
        }
        Some(CommandLine::Recover { contract_id }) => {
            let id = match contract_id {
                Some(id) => id,
                None => {
                    contract::get_active_contract(root)?
                        .map(|c| c.contract_id)
                        .ok_or_else(|| anyhow!("No contract ID specified and no active contract found. Usage: engram recover <contract-id>"))?
                }
            };
            let rep = recovery::recover_contract(root, &id)?;
            if force_json {
                print_json(&rep)?;
            } else {
                print!("{}", recovery::format_recovery_report(&rep));
            }
        }
        Some(CommandLine::Drift {
            module,
            decision,
            since,
            staleness_threshold,
        }) => {
            let opts = drift::DriftOptions {
                module,
                decision,
                since,
                staleness_threshold,
            };
            let rep = drift::analyze_drift(root, &opts)?;
            if force_json {
                print_json(&rep)?;
            } else {
                printer::print_drift_report(&rep);
            }
        }
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
        let key1 = analysis_key("hash_a", env!("CARGO_PKG_VERSION"), "ts-0.25", "python", "cfg_1");
        let key2 = analysis_key("hash_a", env!("CARGO_PKG_VERSION"), "ts-0.25", "python", "cfg_1");
        let key3 = analysis_key("hash_b", env!("CARGO_PKG_VERSION"), "ts-0.25", "python", "cfg_1");
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
        assert_eq!(language(Path::new("main.go")), Some("go"));
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

    #[test]
    fn test_tree_sitter_rust_extraction() {
        let code = b"pub struct OrderService;\nimpl OrderService {\n    pub fn process_order(&self) {}\n}\npub trait OrderRepository {}\n";
        let (record, symbols, _) = parse_file(Path::new("."), "order.rs", code, "rev1").unwrap();
        assert_eq!(record.language, "rust");
        assert!(symbols.iter().any(|s| s.name == "OrderService" && s.kind == "STRUCT"));
        assert!(symbols.iter().any(|s| s.name == "process_order" && s.kind == "FUNCTION"));
        assert!(symbols.iter().any(|s| s.name == "OrderRepository" && s.kind == "TRAIT"));
    }

    #[test]
    fn test_tree_sitter_go_extraction() {
        let code = b"package orders\n\ntype OrderService struct {}\n\nfunc (s *OrderService) ExportCSV() error { return nil }\n\ntype Exporter interface {\n    Export() error\n}\n";
        let (record, symbols, _) = parse_file(Path::new("."), "order.go", code, "rev1").unwrap();
        assert_eq!(record.language, "go");
        assert!(symbols.iter().any(|s| s.name == "OrderService" && s.kind == "STRUCT"));
        assert!(symbols.iter().any(|s| s.name == "OrderService.ExportCSV" && s.kind == "METHOD"));
        assert!(symbols.iter().any(|s| s.name == "Exporter" && s.kind == "INTERFACE"));
        assert!(symbols.iter().any(|s| s.name == "orders" && s.kind == "MODULE"));
    }
}
