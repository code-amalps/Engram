use std::fs;
use std::path::PathBuf;
use std::process::Command;

use engram::analyzer::analyze;
use engram::compiler::{canonicalize_language, context, context_scoped, normalize_subpath};
use engram::prompt::{PromptFormat, generate_prompt, generate_prompt_scoped};

fn setup_test_workspace(name: &str) -> PathBuf {
    let ws = std::env::temp_dir().join(format!(
        "engram_scope_test_{}_{}",
        name,
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&ws).unwrap();

    // Initialize git repository
    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Engram Tester"])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "tester@engram.local"])
        .current_dir(&ws)
        .output();

    // 1. Rust source implementation files
    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        src_dir.join("compiler.rs"),
        r#"//! Context Compiler implementation
pub struct ContextCompiler {
    pub budget: usize,
}

impl ContextCompiler {
    pub fn compile_bm25_context(&self, query: &str) -> Vec<String> {
        vec!["compiled".to_string()]
    }
}
"#,
    )
    .unwrap();

    fs::write(
        src_dir.join("main.rs"),
        r#"//! CLI entry point
fn main() {
    println!("Engram Scoped Query Routing Engine");
}
"#,
    )
    .unwrap();

    // 2. Rust test fixture
    let tests_dir = ws.join("tests");
    fs::create_dir_all(&tests_dir).unwrap();
    fs::write(
        tests_dir.join("test_compiler.rs"),
        r#"//! Compiler integration tests asserting context compiler ranking
#[test]
fn test_compiler_ranking_mock() {
    assert!(true);
}
"#,
    )
    .unwrap();

    // 3. Python benchmark harness files
    let py_dir = ws
        .join("benchmarks")
        .join("matched-pair-harness")
        .join("src");
    fs::create_dir_all(&py_dir).unwrap();
    fs::write(
        py_dir.join("orchestrator.py"),
        r#"""Benchmark Orchestrator evaluating BM25 compiler ranking."""
class Orchestrator:
    def __init__(self, target_budget):
        self.budget = target_budget

    def run_benchmark(self, query):
        print(f"Running benchmark query on compiler: {query}")
"#,
    )
    .unwrap();

    // 4. TypeScript web file
    let web_dir = ws.join("web");
    fs::create_dir_all(&web_dir).unwrap();
    fs::write(
        web_dir.join("app.ts"),
        r#"export interface WebConfig {
    theme: string;
    refreshInterval: number;
}
export function renderDashboard(config: WebConfig): void {
    console.log("Dashboard rendered");
}
"#,
    )
    .unwrap();

    // Commit all files so git status/revision tracking is initialized
    let _ = Command::new("git")
        .args(["add", "."])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["commit", "-m", "Initial test commit"])
        .current_dir(&ws)
        .output();

    // Run Engram analysis to populate SQLite AST index
    analyze(&ws).expect("Engram analyze must succeed on test workspace");

    ws
}

#[test]
fn test_subpath_normalization_unit() {
    // Standard directories
    assert_eq!(normalize_subpath("src").unwrap(), "src");
    assert_eq!(normalize_subpath("src/").unwrap(), "src");
    assert_eq!(
        normalize_subpath("benchmarks/matched-pair-harness/").unwrap(),
        "benchmarks/matched-pair-harness"
    );

    // Windows separator normalization
    assert_eq!(
        normalize_subpath("benchmarks\\matched-pair-harness\\").unwrap(),
        "benchmarks/matched-pair-harness"
    );
    assert_eq!(
        normalize_subpath("src\\compiler.rs").unwrap(),
        "src/compiler.rs"
    );

    // Specific file subpath
    assert_eq!(
        normalize_subpath("src/compiler.rs").unwrap(),
        "src/compiler.rs"
    );

    // Traversal rejection
    assert!(normalize_subpath("..").is_err());
    assert!(normalize_subpath("../src").is_err());
    assert!(normalize_subpath("src/../benchmarks").is_err());
    assert!(normalize_subpath("benchmarks/../../etc").is_err());

    // Absolute path rejection
    assert!(normalize_subpath("/src").is_err());
    assert!(normalize_subpath("/").is_err());
    assert!(normalize_subpath("C:\\Windows").is_err());

    // Empty / whitespace rejection
    assert!(normalize_subpath("").is_err());
    assert!(normalize_subpath("   ").is_err());
}

#[test]
fn test_canonicalize_language_unit() {
    // Canonical names
    assert_eq!(canonicalize_language("rust").unwrap(), "rust");
    assert_eq!(canonicalize_language("python").unwrap(), "python");
    assert_eq!(canonicalize_language("typescript").unwrap(), "typescript");
    assert_eq!(canonicalize_language("javascript").unwrap(), "javascript");
    assert_eq!(canonicalize_language("csharp").unwrap(), "csharp");
    assert_eq!(canonicalize_language("go").unwrap(), "go");
    assert_eq!(canonicalize_language("java").unwrap(), "java");

    // Aliases & case insensitivity
    assert_eq!(canonicalize_language("RS").unwrap(), "rust");
    assert_eq!(canonicalize_language("py").unwrap(), "python");
    assert_eq!(canonicalize_language("ts").unwrap(), "typescript");
    assert_eq!(canonicalize_language("tsx").unwrap(), "typescript");
    assert_eq!(canonicalize_language("js").unwrap(), "javascript");
    assert_eq!(canonicalize_language("jsx").unwrap(), "javascript");
    assert_eq!(canonicalize_language("mjs").unwrap(), "javascript");
    assert_eq!(canonicalize_language("cs").unwrap(), "csharp");
    assert_eq!(canonicalize_language("c#").unwrap(), "csharp");
    assert_eq!(canonicalize_language("golang").unwrap(), "go");

    // Unsupported languages
    assert!(canonicalize_language("ruby").is_err());
    assert!(canonicalize_language("cpp").is_err());
    assert!(canonicalize_language("c++").is_err());
    assert!(canonicalize_language("").is_err());
}

#[test]
fn test_subpath_scoped_retrieval() {
    let ws = setup_test_workspace("subpath");

    // 1. Scoped to "src"
    let res_src = context_scoped(&ws, "compiler BM25 budget", 2000, Some("src"), None).unwrap();
    assert!(!res_src.files.is_empty(), "Should return files under src");
    assert_eq!(res_src.subpath_filter.as_deref(), Some("src"));
    for f in &res_src.files {
        assert!(
            f.path.starts_with("src/"),
            "File {} must start with src/",
            f.path
        );
    }

    // 2. Scoped to "benchmarks/matched-pair-harness"
    let res_bench = context_scoped(
        &ws,
        "compiler BM25 budget",
        2000,
        Some("benchmarks/matched-pair-harness"),
        None,
    )
    .unwrap();
    assert!(
        !res_bench.files.is_empty(),
        "Should return files under benchmarks"
    );
    assert_eq!(
        res_bench.subpath_filter.as_deref(),
        Some("benchmarks/matched-pair-harness")
    );
    for f in &res_bench.files {
        assert!(
            f.path.starts_with("benchmarks/matched-pair-harness/"),
            "File {} must start with benchmarks/matched-pair-harness/",
            f.path
        );
    }

    // 3. Scoped to non-matching subpath
    let res_empty = context_scoped(
        &ws,
        "compiler BM25 budget",
        2000,
        Some("nonexistent_dir"),
        None,
    )
    .unwrap();
    assert!(
        res_empty.files.is_empty(),
        "Non-matching subpath must return 0 files"
    );
    assert!(!res_empty.gaps.is_empty(), "Should produce a gap alert");
    assert!(
        res_empty.gaps[0].starts_with("GAP-002:"),
        "Gap alert must be GAP-002: {}",
        res_empty.gaps[0]
    );
}

#[test]
fn test_language_scoped_retrieval() {
    let ws = setup_test_workspace("lang");

    // 1. Scoped to Rust
    let res_rust = context_scoped(&ws, "compiler BM25 budget", 2000, None, Some("rust")).unwrap();
    assert!(!res_rust.files.is_empty(), "Should find Rust files");
    assert_eq!(res_rust.language_filter.as_deref(), Some("rust"));
    for f in &res_rust.files {
        assert!(
            f.path.ends_with(".rs"),
            "File {} must be a Rust file",
            f.path
        );
    }

    // 2. Scoped to Python via alias "py"
    let res_py = context_scoped(&ws, "compiler BM25 budget", 2000, None, Some("py")).unwrap();
    assert!(!res_py.files.is_empty(), "Should find Python files");
    assert_eq!(res_py.language_filter.as_deref(), Some("python"));
    for f in &res_py.files {
        assert!(
            f.path.ends_with(".py"),
            "File {} must be a Python file",
            f.path
        );
    }

    // 3. Scoped to TypeScript
    let res_ts = context_scoped(&ws, "dashboard config render", 2000, None, Some("ts")).unwrap();
    assert!(!res_ts.files.is_empty(), "Should find TypeScript file");
    assert_eq!(res_ts.language_filter.as_deref(), Some("typescript"));
    assert_eq!(res_ts.files[0].path, "web/app.ts");
}

#[test]
fn test_combined_subpath_and_language_scoping() {
    let ws = setup_test_workspace("combined");

    // 1. Rust in src/
    let res = context_scoped(&ws, "compiler BM25 budget", 2000, Some("src"), Some("rust")).unwrap();
    assert!(!res.files.is_empty());
    assert_eq!(res.subpath_filter.as_deref(), Some("src"));
    assert_eq!(res.language_filter.as_deref(), Some("rust"));
    for f in &res.files {
        assert!(f.path.starts_with("src/"));
        assert!(f.path.ends_with(".rs"));
    }

    // 2. Conflicting filter: Python in src/ (no Python files exist in src/)
    let res_empty = context_scoped(
        &ws,
        "compiler BM25 budget",
        2000,
        Some("src"),
        Some("python"),
    )
    .unwrap();
    assert!(res_empty.files.is_empty());
    assert!(res_empty.gaps[0].starts_with("GAP-002:"));
}

#[test]
fn test_backward_compatibility_unscoped() {
    let ws = setup_test_workspace("compat");

    // Unscoped context call
    let res = context(&ws, "compiler BM25 budget", 3000).unwrap();
    assert!(!res.files.is_empty());
    assert!(res.subpath_filter.is_none());
    assert!(res.language_filter.is_none());

    // Should include both Rust and Python files because BM25 matches both
    let has_rust = res.files.iter().any(|f| f.path.ends_with(".rs"));
    let has_py = res.files.iter().any(|f| f.path.ends_with(".py"));
    assert!(has_rust, "Unscoped context should include Rust files");
    assert!(has_py, "Unscoped context should include Python files");

    // Unscoped prompt call
    let prompt_out = generate_prompt(
        &ws,
        "compiler BM25 budget",
        3000,
        PromptFormat::Markdown,
        false,
    )
    .unwrap();
    assert!(!prompt_out.content.is_empty());
    assert_eq!(prompt_out.files_included, res.files.len());
}

#[test]
fn test_ranking_weight_inversion() {
    let ws = setup_test_workspace("ranking");

    // Query targeting compiler: src/compiler.rs (implementation) vs tests/test_compiler.rs (test)
    let res = context_scoped(
        &ws,
        "ContextCompiler compile_bm25_context",
        1200,
        None,
        Some("rust"),
    )
    .unwrap();
    assert!(!res.files.is_empty());

    let top_file = &res.files[0];
    assert_eq!(
        top_file.path, "src/compiler.rs",
        "Implementation file src/compiler.rs must outrank test fixture tests/test_compiler.rs"
    );
}

#[test]
fn test_prompt_completeness_scope_rendering() {
    let ws = setup_test_workspace("prompt");

    // Markdown format with scope
    let out_md = generate_prompt_scoped(
        &ws,
        "compile_bm25_context",
        1500,
        PromptFormat::Markdown,
        false,
        Some("src"),
        Some("rust"),
    )
    .unwrap();

    assert!(out_md.content.contains("- **Subpath Scope:** `src`"));
    assert!(out_md.content.contains("- **Language Scope:** `rust`"));

    // Text format with scope
    let out_txt = generate_prompt_scoped(
        &ws,
        "compile_bm25_context",
        1500,
        PromptFormat::Text,
        false,
        Some("src"),
        Some("rust"),
    )
    .unwrap();

    assert!(out_txt.content.contains("• Subpath Scope: src"));
    assert!(out_txt.content.contains("• Language Scope: rust"));
}

#[test]
fn test_cli_scoped_options() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = setup_test_workspace("cli");

    // CLI context with --subpath and --language
    let output = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("context")
        .arg("compiler BM25 budget")
        .arg("--subpath")
        .arg("src")
        .arg("--language")
        .arg("rust")
        .arg("--json")
        .output()
        .expect("CLI execution failed");

    assert!(
        output.status.success(),
        "CLI context command must exit with 0"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).expect("Output must be valid JSON");

    assert_eq!(json["subpath_filter"], "src");
    assert_eq!(json["language_filter"], "rust");

    let files = json["files"].as_array().expect("files must be array");
    assert!(!files.is_empty(), "Should return matching files");
    for f in files {
        let p = f["path"].as_str().unwrap();
        assert!(p.starts_with("src/"), "File {} must start with src/", p);
        assert!(p.ends_with(".rs"), "File {} must be .rs", p);
    }

    // CLI traversal rejection
    let output_bad = Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("context")
        .arg("test")
        .arg("--subpath")
        .arg("../bad")
        .output()
        .expect("CLI execution failed");

    assert!(
        !output_bad.status.success(),
        "CLI must fail on path traversal attempt"
    );
}

#[test]
fn test_phase_9_5_directory_pruning_and_benchmark_fixture_indexing() {
    let ws = std::env::temp_dir().join(format!("engram_prune_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Engram Tester"])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "tester@engram.local"])
        .current_dir(&ws)
        .output();

    // 1. Files that MUST be indexed:
    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        src_dir.join("core.rs"),
        "pub fn core_engine() -> i32 { 42 }\n",
    )
    .unwrap();

    let harness_src = ws
        .join("benchmarks")
        .join("matched-pair-harness")
        .join("src");
    fs::create_dir_all(&harness_src).unwrap();
    fs::write(
        harness_src.join("orchestrator.py"),
        "def run_benchmark():\n    return 'benchmarked'\n",
    )
    .unwrap();

    // 2. Files and directories that MUST be pruned:
    fs::write(
        ws.join(".gitignore"),
        "dist/\nbuild/\nassets/\nscratch/\nout/\n",
    )
    .unwrap();

    let pruned_dirs = [
        ws.join("benchmarks").join("cost-baseline").join("src"),
        ws.join("benchmarks").join("profile").join("src"),
        ws.join("benchmarks")
            .join("matched-pair-harness")
            .join("runs"),
        ws.join("benchmarks")
            .join("matched-pair-harness")
            .join("reports"),
        ws.join("benchmarks")
            .join("matched-pair-harness")
            .join("workloads"),
        ws.join("benchmarks")
            .join("matched-pair-harness")
            .join("results"),
        ws.join("benchmarks")
            .join("matched-pair-harness")
            .join("datasets"),
        ws.join("target").join("debug"),
        ws.join("node_modules").join("pkg"),
        ws.join("dist"),
        ws.join("build"),
        ws.join("coverage"),
        ws.join("test_out"),
        ws.join("releases"),
        ws.join(".pytest_cache"),
        ws.join("graphify-out"),
        ws.join("assets"),
        ws.join("scratch"),
        ws.join("out"),
    ];

    for dir in &pruned_dirs {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join("dummy.py"),
            "# should be pruned\ndef dummy(): pass\n",
        )
        .unwrap();
        fs::write(
            dir.join("dummy.rs"),
            "// should be pruned\npub fn dummy() {}\n",
        )
        .unwrap();
    }

    // Run analyzer
    let summary = analyze(&ws).expect("analyze must succeed");
    assert_eq!(
        summary.indexed_files, 2,
        "Only core.rs and orchestrator.py should be indexed, found: {}",
        summary.indexed_files
    );

    // Verify via context retrieval
    let ctx = context(&ws, "core_engine benchmark", 4000).expect("context must succeed");
    let indexed_paths: Vec<_> = ctx.files.iter().map(|f| f.path.as_str()).collect();

    assert!(
        indexed_paths.contains(&"src/core.rs"),
        "src/core.rs must be present in indexed files"
    );
    assert!(
        indexed_paths.contains(&"benchmarks/matched-pair-harness/src/orchestrator.py"),
        "benchmarks/matched-pair-harness/src/orchestrator.py must be present"
    );

    // Verify none of the pruned paths are indexed
    for p in &indexed_paths {
        assert!(!p.starts_with("benchmarks/cost-baseline"), "pruned: {}", p);
        assert!(!p.starts_with("benchmarks/profile"), "pruned: {}", p);
        assert!(!p.contains("/runs/"), "pruned: {}", p);
        assert!(!p.contains("/reports/"), "pruned: {}", p);
        assert!(!p.contains("/workloads/"), "pruned: {}", p);
        assert!(!p.contains("/results/"), "pruned: {}", p);
        assert!(!p.contains("/datasets/"), "pruned: {}", p);
        assert!(!p.starts_with("target/"), "pruned: {}", p);
        assert!(!p.starts_with("node_modules/"), "pruned: {}", p);
        assert!(!p.starts_with("dist/"), "pruned: {}", p);
        assert!(!p.starts_with("scratch/"), "pruned: {}", p);
    }

    // Verify ensure_fresh
    engram::analyzer::ensure_fresh(&ws).expect("ensure_fresh must succeed");

    // Verify incremental deletion: delete orchestrator.py
    fs::remove_file(harness_src.join("orchestrator.py")).unwrap();
    let re_summary = analyze(&ws).expect("re-analyze must succeed");
    assert_eq!(
        re_summary.indexed_files, 1,
        "After deletion, only core.rs should remain indexed"
    );
    assert_eq!(re_summary.removed_files, 1, "Removed file count must be 1");

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_phase_9_5_2_nested_customer_source_directories_and_generated_pruning() {
    let ws = std::env::temp_dir().join(format!("engram_p952_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Engram Tester"])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "tester@engram.local"])
        .current_dir(&ws)
        .output();

    // 1. Create legitimate nested customer source directories:
    let nested_sources = [
        (
            "src/reports",
            "report_generator.rs",
            "pub fn generate_financial_report() -> &'static str { \"report\" }\n",
        ),
        (
            "src/datasets",
            "dataset_loader.rs",
            "pub fn load_customer_dataset() -> usize { 100 }\n",
        ),
        (
            "src/assets",
            "app_styles.ts",
            "export const primaryColor: string = '#007acc';\n",
        ),
        (
            "src/build",
            "custom_builder.rs",
            "pub fn run_custom_build_step() -> bool { true }\n",
        ),
        (
            "src/results",
            "analysis_result.rs",
            "pub struct AnalysisResult { pub score: f64 }\n",
        ),
        (
            "src/bin",
            "custom_cli.rs",
            "fn main() { println!(\"CLI runner\"); }\n",
        ),
        (
            "src/workloads",
            "job_dispatcher.rs",
            "pub fn dispatch_workload_queue() {}\n",
        ),
        (
            "src/vendor",
            "vendored_helper.rs",
            "pub fn helper_call() -> i32 { 1 }\n",
        ),
        (
            "src/out",
            "output_formatter.rs",
            "pub fn format_text_output(s: &str) -> String { s.to_string() }\n",
        ),
        (
            "src/dist",
            "package_distributor.rs",
            "pub fn distribute_packages() {}\n",
        ),
        (
            "benchmarks/matched-pair-harness/src",
            "orchestrator.py",
            "def run_orchestrator():\n    return True\n",
        ),
    ];

    for (dir, file, content) in &nested_sources {
        let dir_path = ws.join(dir);
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(dir_path.join(file), content).unwrap();
    }

    // 2. Create generated / artifact / output directories that MUST be pruned:
    fs::write(ws.join(".gitignore"), "/dist/\n/scratch/\n").unwrap();
    let generated_dirs = [
        // Root-level generated dirs
        ("dist", "bundle.js", "// compiled bundle\n"),
        ("build", "compiler_out.o", "// object file\n"),
        ("out", "debug_output.txt", "log\n"),
        ("releases", "candidate_archive.zip", "archive\n"),
        ("scratch", "scratchpad.py", "# scratch\n"),
        ("bin", "compiled_binary.exe", "MZ\n"),
        ("target/debug", "dummy.rs", "// target dummy\n"),
        ("node_modules/pkg", "index.js", "// npm package\n"),
        ("coverage", "lcov.info", "TN:\n"),
        ("test_out", "run.log", "test log\n"),
        (".pytest_cache", "cache.json", "{}\n"),
        ("graphify-out", "graph.json", "{}\n"),
        // Benchmark output dirs under matched-pair-harness
        (
            "benchmarks/matched-pair-harness/runs/run1",
            "dump.json",
            "{}\n",
        ),
        (
            "benchmarks/matched-pair-harness/reports",
            "report.md",
            "# report\n",
        ),
        (
            "benchmarks/matched-pair-harness/workloads",
            "task.json",
            "{}\n",
        ),
        (
            "benchmarks/matched-pair-harness/results",
            "eval.json",
            "{}\n",
        ),
        (
            "benchmarks/matched-pair-harness/datasets",
            "data.csv",
            "a,b,c\n",
        ),
        (
            "benchmarks/matched-pair-harness/bin",
            "engram.exe",
            "binary\n",
        ),
        (
            "benchmarks/matched-pair-harness/scratch",
            "temp.py",
            "pass\n",
        ),
        // Sibling benchmark dirs under benchmarks/
        (
            "benchmarks/cost-baseline/src",
            "baseline.rs",
            "pub fn baseline() {}\n",
        ),
        (
            "benchmarks/profile/src",
            "profiler.rs",
            "pub fn profile() {}\n",
        ),
    ];

    for (dir, file, content) in &generated_dirs {
        let dir_path = ws.join(dir);
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(dir_path.join(file), content).unwrap();
    }

    // 3. Secret files that must be excluded:
    fs::write(ws.join("src").join(".env"), "SECRET_KEY=12345\n").unwrap();
    fs::write(ws.join("src").join("secret.key"), "PRIVATE_KEY\n").unwrap();
    fs::write(ws.join("src").join("cert.pem"), "CERTIFICATE\n").unwrap();

    // 4. Initial Analyzer Indexing:
    let summary = analyze(&ws).expect("analyze must succeed");
    assert_eq!(
        summary.indexed_files, 11,
        "All 11 legitimate nested source files must be indexed; found {}",
        summary.indexed_files
    );

    // 5. Scoped Query Routing across nested customer source directories:
    // 5a. Scoped to src/reports
    let res_reports = context_scoped(
        &ws,
        "generate financial report",
        4000,
        Some("src/reports"),
        None,
    )
    .unwrap();
    let rep_files: Vec<_> = res_reports.files.iter().map(|f| f.path.as_str()).collect();
    assert!(
        rep_files.contains(&"src/reports/report_generator.rs"),
        "Must retrieve src/reports/report_generator.rs"
    );

    // 5b. Scoped to src/datasets
    let res_datasets = context_scoped(
        &ws,
        "load customer dataset",
        4000,
        Some("src/datasets"),
        Some("rust"),
    )
    .unwrap();
    let ds_files: Vec<_> = res_datasets.files.iter().map(|f| f.path.as_str()).collect();
    assert!(
        ds_files.contains(&"src/datasets/dataset_loader.rs"),
        "Must retrieve src/datasets/dataset_loader.rs"
    );

    // 5c. Scoped to src/assets
    let res_assets = context_scoped(
        &ws,
        "primary color theme styles",
        4000,
        Some("src/assets"),
        Some("typescript"),
    )
    .unwrap();
    let asset_files: Vec<_> = res_assets.files.iter().map(|f| f.path.as_str()).collect();
    assert!(
        asset_files.contains(&"src/assets/app_styles.ts"),
        "Must retrieve src/assets/app_styles.ts"
    );

    // 5d. Scoped to src/build
    let res_build =
        context_scoped(&ws, "custom build step", 4000, Some("src/build"), None).unwrap();
    let bld_files: Vec<_> = res_build.files.iter().map(|f| f.path.as_str()).collect();
    assert!(
        bld_files.contains(&"src/build/custom_builder.rs"),
        "Must retrieve src/build/custom_builder.rs"
    );

    // 5e. Scoped to src/results
    let res_results = context_scoped(
        &ws,
        "analysis result score",
        4000,
        Some("src/results"),
        None,
    )
    .unwrap();
    let res_files: Vec<_> = res_results.files.iter().map(|f| f.path.as_str()).collect();
    assert!(
        res_files.contains(&"src/results/analysis_result.rs"),
        "Must retrieve src/results/analysis_result.rs"
    );

    // 5f. Scoped to src/bin
    let res_bin = context_scoped(&ws, "CLI runner custom", 4000, Some("src/bin"), None).unwrap();
    let bin_files: Vec<_> = res_bin.files.iter().map(|f| f.path.as_str()).collect();
    assert!(
        bin_files.contains(&"src/bin/custom_cli.rs"),
        "Must retrieve src/bin/custom_cli.rs"
    );

    // 5g. Scoped to benchmarks/matched-pair-harness
    let res_harness = context_scoped(
        &ws,
        "run orchestrator",
        4000,
        Some("benchmarks/matched-pair-harness"),
        Some("python"),
    )
    .unwrap();
    let h_files: Vec<_> = res_harness.files.iter().map(|f| f.path.as_str()).collect();
    assert!(
        h_files.contains(&"benchmarks/matched-pair-harness/src/orchestrator.py"),
        "Must retrieve orchestrator.py"
    );

    // 6. Verify that NO generated, benchmark artifact, or secret files were indexed:
    let all_ctx = context(&ws, "report dataset asset build result orchestrator", 16000).unwrap();
    let all_indexed: Vec<_> = all_ctx.files.iter().map(|f| f.path.as_str()).collect();
    for p in &all_indexed {
        assert!(
            !p.starts_with("dist/"),
            "Generated root dist must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("build/"),
            "Generated root build must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("out/"),
            "Generated root out must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("releases/"),
            "Generated root releases must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("scratch/"),
            "Generated root scratch must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("bin/"),
            "Generated root bin must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("target/"),
            "Universal target must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("node_modules/"),
            "Universal node_modules must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("coverage/"),
            "Universal coverage must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("test_out/"),
            "Universal test_out must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with(".pytest_cache/"),
            "Universal pytest cache must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("graphify-out/"),
            "Universal graphify-out must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("benchmarks/cost-baseline/"),
            "Sibling benchmark must not be indexed: {}",
            p
        );
        assert!(
            !p.starts_with("benchmarks/profile/"),
            "Sibling benchmark must not be indexed: {}",
            p
        );
        assert!(
            !p.contains("/runs/"),
            "Benchmark run artifacts must not be indexed: {}",
            p
        );
        assert!(
            !p.contains("/reports/report.md"),
            "Benchmark reports must not be indexed: {}",
            p
        );
        assert!(
            !p.contains("/workloads/"),
            "Benchmark workloads must not be indexed: {}",
            p
        );
        assert!(
            !p.contains("/results/eval.json"),
            "Benchmark results must not be indexed: {}",
            p
        );
        assert!(
            !p.contains("/datasets/data.csv"),
            "Benchmark datasets must not be indexed: {}",
            p
        );
        assert!(
            !p.ends_with(".env"),
            "Secret .env must not be indexed: {}",
            p
        );
        assert!(
            !p.ends_with(".key"),
            "Secret .key must not be indexed: {}",
            p
        );
        assert!(
            !p.ends_with(".pem"),
            "Secret .pem must not be indexed: {}",
            p
        );
    }

    // 7. Knowledge Graph Consistency Verification:
    let graph_status = engram::knowledge_graph::sync_knowledge_graph(&ws, false)
        .expect("sync_knowledge_graph must succeed");
    assert_eq!(
        graph_status.indexed_files_count, 11,
        "Knowledge graph must index the exact same 11 files as analyzer; found {}",
        graph_status.indexed_files_count
    );

    // 8. Incremental Freshness & Lifecycle Verification:
    engram::analyzer::ensure_fresh(&ws).expect("ensure_fresh must succeed");

    // Add a new file in legitimate nested directory: src/reports/monthly_report.rs
    fs::write(
        ws.join("src").join("reports").join("monthly_report.rs"),
        "pub fn monthly() {}\n",
    )
    .unwrap();
    let inc_summary = analyze(&ws).expect("incremental analyze must succeed");
    assert_eq!(
        inc_summary.indexed_files, 12,
        "Should index 12 files after addition"
    );
    assert_eq!(inc_summary.changed_files, 1, "Should detect 1 changed file");

    // Delete the file
    fs::remove_file(ws.join("src").join("reports").join("monthly_report.rs")).unwrap();
    let del_summary = analyze(&ws).expect("analyze after removal must succeed");
    assert_eq!(
        del_summary.indexed_files, 11,
        "Should index 11 files after deletion"
    );
    assert_eq!(del_summary.removed_files, 1, "Should detect 1 removed file");

    let _ = fs::remove_dir_all(&ws);
}
