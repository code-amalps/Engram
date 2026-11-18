use std::fs;
use std::path::PathBuf;
use std::process::Command;

use engram::analyzer::analyze;
use engram::compiler::{canonicalize_language, context, context_scoped, normalize_subpath};
use engram::prompt::{generate_prompt, generate_prompt_scoped, PromptFormat};

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
