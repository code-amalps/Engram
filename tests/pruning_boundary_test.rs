use engram::analyzer::analyze;
use engram::compiler::context;
use engram::discovery::DiscoveryEngine;
use engram::incremental::incremental_analyze;
use std::fs;
use std::process::Command;

#[test]
fn test_fixture_a_legitimate_root_level_source_discoverable() {
    let ws = std::env::temp_dir().join(format!("engram_fixture_a_{}", uuid::Uuid::new_v4()));
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

    // Customer-A layout across 5 root directories in representative languages:
    // Rust, TypeScript, Python, and Go
    let root_dirs = [
        (
            "reports",
            "report_generator.rs",
            "pub fn generate_financial_report() -> &'static str { \"financial_summary\" }\n",
        ),
        (
            "datasets",
            "loader.ts",
            "export function loadCustomerDataset(): number[] { return [1, 2, 3]; }\n",
        ),
        (
            "assets",
            "resource_loader.py",
            "def load_theme_assets():\n    return {'theme': 'dark'}\n",
        ),
        (
            "build",
            "build_config.go",
            "package build\nfunc BuildConfig() string { return \"production\" }\n",
        ),
        (
            "results",
            "result_processor.rs",
            "pub fn process_evaluation_results() -> bool { true }\n",
        ),
    ];

    for (dir, file, content) in &root_dirs {
        let dir_path = ws.join(dir);
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(dir_path.join(file), content).unwrap();
    }

    // 1. Without explicit ignore rules, all 5 root-level directories are discovered
    let summary = analyze(&ws).expect("analyze must succeed on customer-a");
    assert_eq!(
        summary.indexed_files, 5,
        "All 5 legitimate root-level customer source directories must be discoverable and indexed"
    );

    // 2. KnowledgeGraph parity: must match Analyzer exactly
    let graph = engram::knowledge_graph::sync_knowledge_graph(&ws, false)
        .expect("sync_knowledge_graph must succeed");
    assert_eq!(
        graph.indexed_files_count, summary.indexed_files,
        "KnowledgeGraph and Analyzer parity verified on Fixture A"
    );

    // 3. Retrieval can locate representative symbols across the directories
    let ctx = context(&ws, "generate_financial_report", 4000).expect("context must succeed");
    assert!(
        ctx.files
            .iter()
            .any(|f| f.path.contains("report_generator.rs")),
        "Retrieval must find file from reports/ directory"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_fixture_a_with_explicit_gitignore() {
    let ws = std::env::temp_dir().join(format!("engram_fixture_a_git_{}", uuid::Uuid::new_v4()));
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

    let root_dirs = [
        ("reports", "report.rs", "pub fn report() {}\n"),
        ("datasets", "data.rs", "pub fn data() {}\n"),
        ("assets", "asset.rs", "pub fn asset() {}\n"),
        ("build", "build.rs", "pub fn build_step() {}\n"),
        ("results", "res.rs", "pub fn results() {}\n"),
    ];

    for (dir, file, content) in &root_dirs {
        let dir_path = ws.join(dir);
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(dir_path.join(file), content).unwrap();
    }

    // Exclude build/ and results/ via .gitignore
    fs::write(ws.join(".gitignore"), "build/\nresults/\n").unwrap();

    let summary = analyze(&ws).expect("analyze must succeed");
    assert_eq!(
        summary.indexed_files, 3,
        "Only reports, datasets, and assets should be indexed (build and results ignored)"
    );

    let graph = engram::knowledge_graph::sync_knowledge_graph(&ws, false)
        .expect("sync_knowledge_graph must succeed");
    assert_eq!(
        graph.indexed_files_count, 3,
        "KnowledgeGraph must agree with Analyzer on .gitignore exclusions"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_fixture_a_with_engramignore_and_negation() {
    let ws = std::env::temp_dir().join(format!("engram_fixture_a_eng_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();

    let files = [
        ("reports/gen.rs", "pub fn gen() {}\n"),
        ("reports/keep.rs", "pub fn keep() {}\n"),
        ("datasets/data.rs", "pub fn data() {}\n"),
    ];
    for (rel, content) in &files {
        let p = ws.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    // .engramignore excludes reports/* and datasets/, but re-includes !reports/keep.rs
    fs::write(
        ws.join(".engramignore"),
        "reports/*\n!reports/keep.rs\ndatasets/\n",
    )
    .unwrap();

    let summary = analyze(&ws).expect("analyze must succeed");
    assert_eq!(
        summary.indexed_files, 1,
        "Only reports/keep.rs should be indexed due to negation rule"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_fixture_a_with_engram_yaml() {
    let ws = std::env::temp_dir().join(format!("engram_fixture_a_yaml_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();

    let files = [
        ("reports/rep.rs", "pub fn rep() {}\n"),
        ("assets/theme.rs", "pub fn theme() {}\n"),
        ("build/bld.rs", "pub fn bld() {}\n"),
    ];
    for (rel, content) in &files {
        let p = ws.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    // engram.yaml excludes assets/ and build/*, but whitelists !build/bld.rs
    let yaml_content = r#"
ignore:
  use_gitignore: true
  use_engramignore: true
  exclude:
    - "assets/"
    - "build/*"
  include:
    - "!build/bld.rs"
"#;
    fs::write(ws.join("engram.yaml"), yaml_content).unwrap();

    let summary = analyze(&ws).expect("analyze must succeed");
    assert_eq!(
        summary.indexed_files, 2,
        "reports/rep.rs and build/bld.rs (whitelisted) should be indexed; assets/ excluded"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_incremental_indexing_reconciles_configuration_invalidation() {
    let ws = std::env::temp_dir().join(format!("engram_invalidation_{}", uuid::Uuid::new_v4()));
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

    let src = ws.join("src");
    let reports = ws.join("reports");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&reports).unwrap();

    fs::write(src.join("main.rs"), "pub fn main_app() {}\n").unwrap();
    fs::write(reports.join("report.rs"), "pub fn report_app() {}\n").unwrap();

    // 1. Initial indexing: 2 files
    let summary1 = analyze(&ws).unwrap();
    assert_eq!(summary1.indexed_files, 2);

    // 2. Add .engramignore to ignore reports/
    fs::write(ws.join(".engramignore"), "reports/\n").unwrap();

    // 3. Incremental indexing must remove reports/ from indexed files
    let summary2 = incremental_analyze(&ws).unwrap();
    assert_eq!(
        summary2.total_files, 1,
        "Incremental reindex must remove ignored directory"
    );

    // 4. Remove .engramignore -> reports/ must become discoverable again
    fs::remove_file(ws.join(".engramignore")).unwrap();
    let summary3 = incremental_analyze(&ws).unwrap();
    assert_eq!(
        summary3.total_files, 2,
        "Incremental reindex must restore unignored directory"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_diagnostic_explanations() {
    let ws = std::env::temp_dir().join(format!("engram_diag_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();

    fs::write(ws.join(".gitignore"), "dist/\n").unwrap();
    fs::write(ws.join(".engramignore"), "custom_cache/\n").unwrap();

    let engine = DiscoveryEngine::for_root(&ws);

    // Universal safety directory
    let dec_node = engine.explain(&ws.join("node_modules").join("pkg"), true);
    assert!(dec_node.is_excluded);
    assert_eq!(dec_node.rule_source.as_deref(), Some("UniversalSafety"));

    // Secret file
    let dec_env = engine.explain(&ws.join(".env.production"), false);
    assert!(dec_env.is_excluded);
    assert_eq!(dec_env.rule_source.as_deref(), Some("SecretFile"));

    // GitIgnore rule
    let dec_dist = engine.explain(&ws.join("dist").join("app.js"), false);
    assert!(dec_dist.is_excluded);
    assert_eq!(dec_dist.rule_source.as_deref(), Some(".gitignore"));

    // EngramIgnore rule
    let dec_custom = engine.explain(&ws.join("custom_cache").join("file.rs"), false);
    assert!(dec_custom.is_excluded);
    assert_eq!(dec_custom.rule_source.as_deref(), Some(".engramignore"));

    // Legitimate customer file
    let dec_source = engine.explain(&ws.join("src").join("main.rs"), false);
    assert!(!dec_source.is_excluded);
    assert!(dec_source.rule_source.is_none());

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_fixture_b_generated_root_level_output() {
    let ws = std::env::temp_dir().join(format!("engram_fixture_b_{}", uuid::Uuid::new_v4()));
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

    // Legitimate file in root src
    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("main.rs"), "fn main() {}\n").unwrap();

    // Generated root output (universal safety dirs)
    let gen_dirs = [
        ws.join("target").join("debug"),
        ws.join("node_modules").join("pkg"),
        ws.join(".git").join("hooks"),
        ws.join(".engram").join("cache"),
    ];
    for dir in &gen_dirs {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("dummy.py"), "def dummy(): pass\n").unwrap();
    }

    let summary = analyze(&ws).expect("analyze must succeed on customer-b");
    let graph = engram::knowledge_graph::sync_knowledge_graph(&ws, false)
        .expect("sync_knowledge_graph must succeed");
    assert_eq!(
        graph.indexed_files_count, summary.indexed_files,
        "Graph parity for Fixture B"
    );
    let _ = fs::remove_dir_all(&ws);

    assert_eq!(
        summary.indexed_files, 1,
        "Only src/main.rs should be indexed; universal safety dirs must be excluded"
    );
}

#[test]
fn test_fixture_c_legitimate_nested_source() {
    let ws = std::env::temp_dir().join(format!("engram_fixture_c_{}", uuid::Uuid::new_v4()));
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

    let nested_dirs = [
        ("src/reports", "rep.py", "def rep(): pass\n"),
        ("src/datasets", "data.py", "def data(): pass\n"),
        ("src/assets", "asset.py", "def asset(): pass\n"),
        ("src/build", "bld.py", "def bld(): pass\n"),
        ("src/results", "res.py", "def res(): pass\n"),
    ];

    for (dir, file, content) in &nested_dirs {
        let dir_path = ws.join(dir);
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(dir_path.join(file), content).unwrap();
    }

    let summary = analyze(&ws).expect("analyze must succeed on customer-c");
    let graph = engram::knowledge_graph::sync_knowledge_graph(&ws, false)
        .expect("sync_knowledge_graph must succeed");
    assert_eq!(
        graph.indexed_files_count, summary.indexed_files,
        "Graph parity for Fixture C"
    );
    let _ = fs::remove_dir_all(&ws);

    assert_eq!(
        summary.indexed_files, 5,
        "All 5 legitimate nested source files in customer-c must be indexed"
    );
}

#[test]
fn test_fixture_d_unrelated_benchmark_directory() {
    let ws = std::env::temp_dir().join(format!("engram_fixture_d_{}", uuid::Uuid::new_v4()));
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

    let files = [
        (
            "src/benchmarks/matched-pair-harness",
            "source.py",
            "def s(): pass\n",
        ),
        (
            "src/benchmarks/custom_suite",
            "source.py",
            "def custom(): pass\n",
        ),
        (
            "src/tools/benchmarks/matched-pair-harness/runs",
            "run_task.py",
            "def task(): pass\n",
        ),
    ];

    for (dir, file, content) in &files {
        let dir_path = ws.join(dir);
        fs::create_dir_all(&dir_path).unwrap();
        fs::write(dir_path.join(file), content).unwrap();
    }

    let summary = analyze(&ws).expect("analyze must succeed on customer-d");
    let graph = engram::knowledge_graph::sync_knowledge_graph(&ws, false)
        .expect("sync_knowledge_graph must succeed");
    assert_eq!(
        graph.indexed_files_count, summary.indexed_files,
        "Graph parity for Fixture D"
    );
    let _ = fs::remove_dir_all(&ws);

    assert_eq!(
        summary.indexed_files, 3,
        "All 3 files in customer-d unrelated benchmarks must be indexed; found {}",
        summary.indexed_files
    );
}

#[test]
fn test_fixture_e_actual_engram_benchmark_hierarchy() {
    let ws = std::env::temp_dir().join(format!("engram_fixture_e_{}", uuid::Uuid::new_v4()));
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

    let src_dir = ws.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("core.rs"), "pub fn core() {}\n").unwrap();

    let harness_src = ws
        .join("benchmarks")
        .join("matched-pair-harness")
        .join("src");
    fs::create_dir_all(&harness_src).unwrap();
    fs::write(harness_src.join("orchestrator.py"), "def run(): pass\n").unwrap();

    let pruned = [
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
        ws.join("benchmarks")
            .join("matched-pair-harness")
            .join("bin"),
        ws.join("benchmarks")
            .join("matched-pair-harness")
            .join("scratch"),
        ws.join("benchmarks").join("cost-baseline").join("src"),
        ws.join("benchmarks").join("profile").join("src"),
    ];

    for dir in &pruned {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("dummy.py"), "def dummy(): pass\n").unwrap();
    }

    let summary = analyze(&ws).expect("analyze must succeed on Engram layout");
    let graph = engram::knowledge_graph::sync_knowledge_graph(&ws, false)
        .expect("sync_knowledge_graph must succeed");
    assert_eq!(
        graph.indexed_files_count, summary.indexed_files,
        "Graph parity for Fixture E"
    );
    let _ = fs::remove_dir_all(&ws);

    assert_eq!(
        summary.indexed_files, 2,
        "Only src/core.rs and orchestrator.py should be indexed in Engram layout; found {}",
        summary.indexed_files
    );
}

#[test]
fn test_nested_directory_ignore_and_wildcard_patterns() {
    let ws = std::env::temp_dir().join(format!("engram_nested_ig_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();

    let files = [
        ("packages/pkg_a/dist/bundle.js", "// dist\n"),
        ("packages/pkg_a/src/index.ts", "export const a = 1;\n"),
        ("packages/pkg_b/src/generated.gen.ts", "// gen\n"),
        ("packages/pkg_b/src/valid.ts", "export const b = 2;\n"),
    ];
    for (rel, content) in &files {
        let p = ws.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    // Ignore nested dist directories and *.gen.ts files
    fs::write(ws.join(".gitignore"), "**/dist/\n*.gen.ts\n").unwrap();

    let summary = analyze(&ws).expect("analyze must succeed");
    assert_eq!(
        summary.indexed_files, 2,
        "Only index.ts and valid.ts should be indexed; dist/ and *.gen.ts ignored"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_directory_only_rule_does_not_exclude_same_named_file() {
    let ws = std::env::temp_dir().join(format!("engram_dir_rule_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();

    // build/ directory with dummy file
    let bld_dir = ws.join("build");
    fs::create_dir_all(&bld_dir).unwrap();
    fs::write(bld_dir.join("artifact.rs"), "pub fn artifact() {}\n").unwrap();

    // root-level build.rs file
    fs::write(ws.join("build.rs"), "fn main() {}\n").unwrap();

    // Rule has trailing slash: build/ (directory only!)
    fs::write(ws.join(".gitignore"), "build/\n").unwrap();

    let summary = analyze(&ws).expect("analyze must succeed");
    assert_eq!(
        summary.indexed_files, 1,
        "build.rs must remain discoverable while build/ directory is ignored"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_malformed_yaml_fails_safely_and_continues_discovery() {
    let ws = std::env::temp_dir().join(format!("engram_malformed_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();

    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join("main.rs"), "pub fn main() {}\n").unwrap();

    // Malformed YAML in engram.yaml
    fs::write(ws.join("engram.yaml"), ": bad [ yaml { structure").unwrap();

    let summary = analyze(&ws).expect("analyze must succeed despite malformed yaml");
    assert_eq!(
        summary.indexed_files, 1,
        "Normal discovery continues despite malformed yaml configuration"
    );

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_path_separators_windows_and_unix() {
    let ws = std::env::temp_dir().join(format!("engram_separators_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();

    let files = [
        ("temp/cache/file1.rs", "pub fn f1() {}\n"),
        ("temp/output/file2.rs", "pub fn f2() {}\n"),
        ("src/app.rs", "pub fn app() {}\n"),
    ];
    for (rel, content) in &files {
        let p = ws.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    // Windows backslash in .engramignore
    fs::write(ws.join(".engramignore"), "temp\\cache\\\ntemp/output/\n").unwrap();

    let summary = analyze(&ws).expect("analyze must succeed");
    assert_eq!(
        summary.indexed_files, 1,
        "Only src/app.rs should be indexed; both temp\\cache\\ and temp/output/ ignored"
    );

    let _ = fs::remove_dir_all(&ws);
}
