//! Phase 1 Verification Suite:
//! - Deterministic Fixtures across all 7 languages (TS, JS, Python, C#, Rust, Go, Java/JVM)
//! - Adversarial fixtures (cyclic, deep hierarchy, syntax edges, missing imports)
//! - Incremental Efficiency measurement framework
//! - External Benchmark runner (`engram bench --external <path>`)
//! - Strict separation from standard test suites and zero side-effects.

use engram::bench::{
    format_external_benchmark_report, run_external_benchmark,
    run_incremental_efficiency_benchmark,
};
use std::path::Path;
use std::sync::Mutex;

static FIXTURE_BENCH_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_all_seven_language_fixtures_exist() {
    let languages = [
        ("typescript", "package.json"),
        ("javascript", "package.json"),
        ("python", "pyproject.toml"),
        ("csharp", "Commerce.csproj"),
        ("rust", "Cargo.toml"),
        ("go", "go.mod"),
        ("java", "pom.xml"),
    ];

    for (lang, manifest) in &languages {
        let dir = Path::new("tests/fixtures/real_world").join(lang);
        assert!(
            dir.exists(),
            "Fixture directory for {} must exist at {}",
            lang,
            dir.display()
        );
        let manifest_file = dir.join(manifest);
        assert!(
            manifest_file.exists(),
            "Manifest file for {} must exist at {}",
            lang,
            manifest_file.display()
        );
    }

    // Check Java ecosystem parity requirements (both Maven pom.xml and Gradle build.gradle)
    let java_dir = Path::new("tests/fixtures/real_world/java");
    assert!(java_dir.join("pom.xml").exists(), "pom.xml must exist for Java");
    assert!(
        java_dir.join("build.gradle").exists(),
        "build.gradle must exist for Java"
    );
    assert!(
        java_dir
            .join("src/main/java/com/engram/billing/InvoiceService.java")
            .exists(),
        "Java service class must exist"
    );
    assert!(
        java_dir
            .join("src/test/java/com/engram/billing/InvoiceServiceTest.java")
            .exists(),
        "Java JUnit test must exist"
    );
}

#[test]
fn test_adversarial_fixtures_exist() {
    let cyclic = Path::new("tests/adversarial/cyclic_deps/src");
    assert!(cyclic.join("module_a.ts").exists());
    assert!(cyclic.join("module_b.ts").exists());

    let deep = Path::new("tests/adversarial/deep_hierarchy/level1/level2/level3/level4/level5");
    assert!(deep.join("deep_service.py").exists());

    let edge = Path::new("tests/adversarial/syntax_edge_cases/src");
    assert!(edge.join("edge_cases.rs").exists());

    let missing = Path::new("tests/adversarial/missing_imports/src");
    assert!(missing.join("broken.go").exists());
}

#[test]
fn test_incremental_efficiency_measurement_runner() {
    let _lock = FIXTURE_BENCH_LOCK.lock().unwrap();
    let metrics = run_incremental_efficiency_benchmark().expect("Incremental efficiency benchmark should succeed");

    assert!(
        metrics.cold_latency_ms >= 0.0,
        "Cold latency must be recorded"
    );
    assert!(
        metrics.warm_incremental_latency_ms >= 0.0,
        "Warm latency must be recorded"
    );
    assert!(
        metrics.speedup_factor >= 1.0,
        "Speedup factor must be at least 1.0"
    );
    assert_eq!(
        metrics.cold_files_reparsed, 10,
        "Cold scan must parse all 10 fixture files"
    );
    assert_eq!(
        metrics.warm_files_reparsed, 1,
        "Warm scan must re-parse only the 1 modified file in Tier 1"
    );
    assert!(
        metrics.reparsed_reduction_pct >= 85.0,
        "Surface reduction must be >= 85%, got {:.1}%",
        metrics.reparsed_reduction_pct
    );
    assert!(
        metrics.cache_hit_rate_pct >= 85.0,
        "Cache hit rate must be >= 85%, got {:.1}%",
        metrics.cache_hit_rate_pct
    );
    assert!(
        metrics.cold_symbols_relinked > 0,
        "Cold scan must extract symbols"
    );
    assert!(
        metrics.warm_symbols_relinked > 0,
        "Warm scan must re-link symbols in modified file"
    );
    assert!(
        metrics.relinked_reduction_pct > 0.0,
        "Symbol relinking reduction must be positive"
    );
}

#[test]
fn test_external_benchmark_runner_and_report_formatting() {
    let _lock = FIXTURE_BENCH_LOCK.lock().unwrap();
    let target = Path::new("tests/fixtures/real_world/typescript");
    let result = run_external_benchmark(target).expect("External benchmark should succeed on typescript fixture");

    assert!(result.indexed_files >= 4, "Must index at least 4 files");
    assert!(result.total_symbols >= 4, "Must extract symbols");
    assert!(
        result.languages_detected.contains(&"typescript".to_string()),
        "Must detect typescript"
    );
    assert!(
        result.cache_hit_rate_pct >= 90.0,
        "Warm untouched re-analysis must have >= 90% cache hit rate"
    );
    assert!(
        result.cold_analysis_latency_ms >= 0.0,
        "Must measure cold latency"
    );
    assert!(
        result.warm_incremental_latency_ms >= 0.0,
        "Must measure warm latency"
    );

    // Verify formatted report
    let report = format_external_benchmark_report(&result);
    assert!(report.contains("Engram External Repository Benchmark"));
    assert!(report.contains("DISCOVERY & SCALE"));
    assert!(report.contains("TWO-TIER INVALIDATION"));
    assert!(report.contains("typescript"));
}

#[test]
fn test_external_benchmark_nonexistent_path_fails_gracefully() {
    let bad_path = Path::new("non_existent_folder_xyz_123");
    let result = run_external_benchmark(bad_path);
    assert!(result.is_err(), "Must fail gracefully for non-existent path");
    assert!(result.unwrap_err().to_string().contains("does not exist"));
}

#[test]
fn test_external_benchmark_file_path_rejected() {
    let file_path = Path::new("Cargo.toml");
    let result = run_external_benchmark(file_path);
    assert!(result.is_err(), "Must reject non-directory file path");
    assert!(result.unwrap_err().to_string().contains("must be a directory"));
}

#[test]
fn test_all_language_fixtures_analyze_cleanly() {
    let _lock = FIXTURE_BENCH_LOCK.lock().unwrap();
    let fixtures = ["typescript", "javascript", "python", "csharp", "rust", "go", "java"];
    for lang in &fixtures {
        let path = Path::new("tests/fixtures/real_world").join(lang);
        let res = run_external_benchmark(&path)
            .unwrap_or_else(|e| panic!("Benchmark failed for {}: {}", lang, e));
        assert!(res.cold_analysis_latency_ms >= 0.0);
        assert!(res.warm_incremental_latency_ms >= 0.0);
    }
}

#[test]
fn test_all_adversarial_fixtures_analyze_cleanly() {
    let _lock = FIXTURE_BENCH_LOCK.lock().unwrap();
    let fixtures = ["cyclic_deps", "deep_hierarchy", "syntax_edge_cases", "missing_imports"];
    for adv in &fixtures {
        let path = Path::new("tests/adversarial").join(adv);
        let res = run_external_benchmark(&path)
            .unwrap_or_else(|e| panic!("Adversarial benchmark failed for {}: {}", adv, e));
        assert!(res.indexed_files > 0, "Must index files for {}", adv);
        assert!(res.total_symbols > 0, "Must extract symbols for {}", adv);
    }
}
