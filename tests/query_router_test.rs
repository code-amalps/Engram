use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use engram::analyzer::analyze;
use engram::compiler::context;
use engram::frameworks::detect_framework_provider;

fn setup_test_workspace(name: &str) -> PathBuf {
    let ws = std::env::temp_dir().join(format!("engram_query_router_test_{}_{}", name, uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();
    let _ = Command::new("git").args(["config", "user.name", "Engram Tester"]).current_dir(&ws).output();
    let _ = Command::new("git").args(["config", "user.email", "tester@engram.local"]).current_dir(&ws).output();

    ws
}

// ----------------------------------------------------------------------------
// Test 1: Generic query routing without a framework adapter
// ----------------------------------------------------------------------------
#[test]
fn test_01_generic_query_routing_without_framework_adapter() {
    let ws = setup_test_workspace("generic");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("calculator.rs"),
        r#"pub struct Calculator;
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 { a + b }
}
"#,
    ).unwrap();

    fs::write(
        src.join("calculator_test.rs"),
        r#"use super::Calculator;
#[test]
fn test_add() {
    let c = Calculator;
    assert_eq!(c.add(2, 2), 4);
}
"#,
    ).unwrap();

    analyze(&ws).unwrap();

    // Verify Generic provider is selected
    let files = vec!["src/calculator.rs".to_string(), "src/calculator_test.rs".to_string()];
    let provider = detect_framework_provider(&ws, &files);
    assert_eq!(provider.name(), "generic");

    let res = context(&ws, "Calculator add method", 2000).unwrap();
    assert!(!res.files.is_empty());
    assert_eq!(res.files[0].path, "src/calculator.rs");
    assert!(res.files[0].provenance.iter().any(|p| p.contains("Exact symbol match") || p.contains("BM25")));

    let _ = fs::remove_dir_all(&ws);
}

// ----------------------------------------------------------------------------
// Test 2: Angular/NgRx adapter discovery
// ----------------------------------------------------------------------------
#[test]
fn test_02_angular_ngrx_adapter_discovery() {
    let ws = setup_test_workspace("angular_detect");
    fs::write(ws.join("angular.json"), r#"{"$schema": "./node_modules/@angular/cli/lib/config/schema.json"}"#).unwrap();

    let files = vec![
        "src/app/core/auth/auth.actions.ts".to_string(),
        "src/app/core/auth/auth.effects.ts".to_string(),
    ];

    let provider = detect_framework_provider(&ws, &files);
    assert_eq!(provider.name(), "angular_ngrx");

    let _ = fs::remove_dir_all(&ws);
}

// ----------------------------------------------------------------------------
// Test 3: Exact symbol matching
// ----------------------------------------------------------------------------
#[test]
fn test_03_exact_symbol_matching() {
    let ws = setup_test_workspace("exact_symbol");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("auth.service.ts"),
        r#"export class AuthService {
    login(): boolean { return true; }
}
"#,
    ).unwrap();

    fs::write(
        src.join("unrelated.ts"),
        r#"export class UnrelatedWidget {
    render(): void {}
}
"#,
    ).unwrap();

    analyze(&ws).unwrap();

    let res = context(&ws, "Inspect AuthService class", 2000).unwrap();
    assert!(!res.files.is_empty());
    assert_eq!(res.files[0].path, "src/auth.service.ts");
    assert!(res.files[0].provenance.iter().any(|p| p.contains("Exact symbol match: AuthService")));

    let _ = fs::remove_dir_all(&ws);
}

// ----------------------------------------------------------------------------
// Test 4: Task terminology matching
// ----------------------------------------------------------------------------
#[test]
fn test_04_task_terminology_matching() {
    let ws = setup_test_workspace("terminology");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("session_handler.rs"),
        r#"pub struct SessionHandler;
impl SessionHandler {
    pub fn authenticate_user(&self) -> bool { true }
}
"#,
    ).unwrap();

    analyze(&ws).unwrap();

    let res = context(&ws, "authenticate_user session handler", 2000).unwrap();
    assert!(!res.files.is_empty());
    assert_eq!(res.files[0].path, "src/session_handler.rs");

    let _ = fs::remove_dir_all(&ws);
}

// ----------------------------------------------------------------------------
// Test 5: Cross-file relationship discovery
// ----------------------------------------------------------------------------
#[test]
fn test_05_cross_file_relationship_discovery() {
    let ws = setup_test_workspace("cross_file");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("service.ts"),
        r#"import { Storage } from './storage';
export class DataService {
    constructor(private s: Storage) {}
}
"#,
    ).unwrap();

    fs::write(
        src.join("storage.ts"),
        r#"export class Storage {
    save(k: string, v: string): void {}
}
"#,
    ).unwrap();

    analyze(&ws).unwrap();

    let res = context(&ws, "DataService", 3000).unwrap();
    let paths: Vec<String> = res.files.iter().map(|f| f.path.clone()).collect();
    assert!(paths.contains(&"src/service.ts".to_string()));
    assert!(paths.contains(&"src/storage.ts".to_string()));

    let _ = fs::remove_dir_all(&ws);
}

// ----------------------------------------------------------------------------
// Test 6: Actions/effects/services/store registration
// ----------------------------------------------------------------------------
#[test]
fn test_06_actions_effects_services_store_registration() {
    let ws = setup_test_workspace("ngrx_triad");
    let auth = ws.join("src/app/core/auth");
    fs::create_dir_all(&auth).unwrap();

    fs::write(
        auth.join("auth.actions.ts"),
        r#"export const login = () => ({ type: '[Auth] Login' });
"#,
    ).unwrap();

    fs::write(
        auth.join("auth.effects.ts"),
        r#"import { login } from './auth.actions';
export class AuthEffects {}
"#,
    ).unwrap();

    fs::write(
        auth.join("auth.reducer.ts"),
        r#"export function authReducer(state: any) { return state; }
"#,
    ).unwrap();

    fs::write(
        ws.join("src/app/core/core.module.ts"),
        r#"import { AuthEffects } from './auth/auth.effects';
export class CoreModule {}
"#,
    ).unwrap();

    fs::write(ws.join("angular.json"), r#"{}"#).unwrap();

    analyze(&ws).unwrap();

    let res = context(&ws, "auth.actions.ts", 4000).unwrap();
    let paths: Vec<String> = res.files.iter().map(|f| f.path.replace('\\', "/")).collect();
    assert!(paths.iter().any(|p| p.ends_with("auth.actions.ts")));
    assert!(paths.iter().any(|p| p.ends_with("auth.effects.ts")));
    assert!(paths.iter().any(|p| p.ends_with("auth.reducer.ts")));

    let _ = fs::remove_dir_all(&ws);
}

// ----------------------------------------------------------------------------
// Test 7: Scenario A (HttpErrorInterceptor) on Golden Angular Repo
// ----------------------------------------------------------------------------
#[test]
fn test_07_scenario_a_golden_angular_http_error_interceptor() {
    let angular_path = Path::new(r"C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter");
    if !angular_path.exists() {
        eprintln!("Golden Angular repo not found, skipping Scenario A");
        return;
    }

    let start = std::time::Instant::now();
    let res = context(angular_path, "HttpErrorInterceptor", 4000).expect("Context compilation succeeds");
    let latency = start.elapsed();
    let json = serde_json::to_string(&res).unwrap();
    eprintln!("\n=== SCENARIO A RETRIEVAL LATENCY: {:?} ===", latency);
    eprintln!("=== SCENARIO A PAYLOAD BYTES: {} ===", json.len());

    assert!(!res.files.is_empty(), "Scenario A must return target files");
    let top_file = &res.files[0];
    assert!(
        top_file.path.contains("http-error.interceptor.ts"),
        "Rank 1 file must be http-error.interceptor.ts, got: {}",
        top_file.path
    );

    // Verify symbol and bounded code excerpt
    assert!(top_file.content.contains("HttpErrorInterceptor"), "Content must contain HttpErrorInterceptor definition");
    assert!(top_file.content.contains("intercept"), "Content must contain intercept method");
    assert!(
        top_file.content.contains("[COMPLETE_BLOCK]") || top_file.content.contains("intercept("),
        "Bounded code excerpt must be present"
    );

    // Verify directly related dependency core.module.ts is discovered
    let paths: Vec<String> = res.files.iter().map(|f| f.path.replace('\\', "/")).collect();
    assert!(
        paths.iter().any(|p| p.contains("core.module.ts")),
        "core.module.ts must be discovered as companion registration file"
    );

    // Verify explainable provenance
    assert!(top_file.provenance.iter().any(|p| p.contains("Exact symbol match: HttpErrorInterceptor")));
}

// ----------------------------------------------------------------------------
// Test 8: Scenario B (AuthEffects) on Golden Angular Repo
// ----------------------------------------------------------------------------
#[test]
fn test_08_scenario_b_golden_angular_auth_effects() {
    let angular_path = Path::new(r"C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter");
    if !angular_path.exists() {
        eprintln!("Golden Angular repo not found, skipping Scenario B");
        return;
    }

    let start = std::time::Instant::now();
    let res = context(angular_path, "AuthEffects", 6000).expect("Context compilation succeeds");
    let latency = start.elapsed();
    let json = serde_json::to_string(&res).unwrap();
    eprintln!("\n=== SCENARIO B RETRIEVAL LATENCY: {:?} ===", latency);
    eprintln!("=== SCENARIO B PAYLOAD BYTES: {} ===", json.len());

    let paths: Vec<String> = res.files.iter().map(|f| f.path.replace('\\', "/")).collect();

    // Verify essential first-party targets
    assert!(paths.iter().any(|p| p.contains("auth.effects.ts")), "Must contain auth.effects.ts");
    assert!(paths.iter().any(|p| p.contains("auth.actions.ts")), "Must discover auth.actions.ts");
    assert!(paths.iter().any(|p| p.contains("local-storage.service.ts")), "Must discover local-storage.service.ts");
    assert!(paths.iter().any(|p| p.contains("core.module.ts")), "Must discover core.module.ts");
    assert!(paths.iter().any(|p| p.contains("auth.effects.spec.ts")), "Must discover auth.effects.spec.ts");

    // Defined ground truth: 5 target files for AuthEffects cluster
    let ground_truth = [
        "auth.effects.ts",
        "auth.actions.ts",
        "local-storage.service.ts",
        "core.module.ts",
        "auth.effects.spec.ts",
    ];
    let mut matches = 0;
    for gt in &ground_truth {
        if paths.iter().any(|p| p.contains(gt)) {
            matches += 1;
        }
    }
    let coverage = matches as f64 / ground_truth.len() as f64;
    eprintln!("Scenario B Ground Truth Coverage: {:.1}% ({}/{})", coverage * 100.0, matches, ground_truth.len());
    assert!(coverage >= 0.95, "Scenario B coverage must be >= 95%, got {:.1}%", coverage * 100.0);

    // Verify external third-party implementations are excluded from context files
    for p in &paths {
        assert!(!p.starts_with("@angular/"), "External package {} must not be in context files", p);
        assert!(!p.starts_with("rxjs"), "External package {} must not be in context files", p);
    }
}

// ----------------------------------------------------------------------------
// Test 9: Scenario C (Remember Me Task) on Golden Angular Repo
// ----------------------------------------------------------------------------
#[test]
fn test_09_scenario_c_golden_angular_remember_me() {
    let angular_path = Path::new(r"C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter");
    if !angular_path.exists() {
        eprintln!("Golden Angular repo not found, skipping Scenario C");
        return;
    }

    let task = "Implement or modify the Remember Me functionality in the existing Angular application.";
    let start = std::time::Instant::now();
    let res = context(angular_path, task, 8000).expect("Context compilation succeeds");
    let latency = start.elapsed();
    let json = serde_json::to_string(&res).unwrap();
    eprintln!("\n=== SCENARIO C RETRIEVAL LATENCY: {:?} ===", latency);
    eprintln!("=== SCENARIO C PAYLOAD BYTES: {} ===", json.len());
    eprintln!("Total files returned: {}", res.files.len());
    for f in &res.files {
        eprintln!("  Selected: {} (score: {}, prov: {:?})", f.path, f.score, f.provenance);
    }
    eprintln!("Total files excluded: {}", res.excluded_files.len());
    for ex in &res.excluded_files {
        eprintln!("  Excluded: {} (reason: {})", ex.0, ex.1);
    }

    let paths: Vec<String> = res.files.iter().map(|f| f.path.replace('\\', "/")).collect();

    // Verify 5/5 Ground-Truth Target Recovery:
    // 1. auth.actions.ts
    // 2. local-storage.service.ts
    // 3. core.module.ts
    // 4. auth.effects.spec.ts
    // 5. auth.effects.ts
    let targets = [
        ("auth.actions.ts", "Action Definitions"),
        ("local-storage.service.ts", "LocalStorage Injected Service"),
        ("core.module.ts", "Core Module Registration"),
        ("auth.effects.spec.ts", "Auth Effects Test Spec"),
        ("auth.effects.ts", "Auth Effects Implementation"),
    ];

    let mut recovered = 0;
    for (target_file, role) in &targets {
        let found = paths.iter().any(|p| p.contains(target_file));
        eprintln!("Target [{}]: {} -> {}", role, target_file, if found { "RECOVERED" } else { "MISSING" });
        if found {
            recovered += 1;
        }
    }

    // Also check UI and i18n companions
    let has_html = paths.iter().any(|p| p.contains("app.component.html"));
    let has_i18n = paths.iter().any(|p| p.contains("en.json"));
    eprintln!("UI Template Companion (app.component.html): {}", if has_html { "RECOVERED" } else { "OMITTED" });
    eprintln!("i18n Translation Companion (en.json): {}", if has_i18n { "RECOVERED" } else { "OMITTED" });

    assert_eq!(recovered, 5, "Scenario C acceptance gate requires 5/5 (100.0%) recovery, got {}/5", recovered);

    // Verify explainable provenance for every target
    for f in &res.files {
        assert!(!f.provenance.is_empty(), "Every file must have explainable provenance, {} had empty", f.path);
    }
}

// ----------------------------------------------------------------------------
// Test 10: Relevance ranking
// ----------------------------------------------------------------------------
#[test]
fn test_10_relevance_ranking() {
    let ws = setup_test_workspace("relevance");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(src.join("target.rs"), "pub struct TargetSymbol;\n").unwrap();
    fs::write(src.join("neighbor.rs"), "use super::target::TargetSymbol;\n").unwrap();
    fs::write(src.join("unrelated.rs"), "pub struct UnrelatedOther;\n").unwrap();

    analyze(&ws).unwrap();

    let res = context(&ws, "TargetSymbol", 3000).unwrap();
    assert!(res.files.len() >= 2);
    assert_eq!(res.files[0].path, "src/target.rs", "Direct symbol match must be rank 1");
    assert!(res.files[0].score > res.files[1].score, "Rank 1 score must exceed neighbor score");

    let _ = fs::remove_dir_all(&ws);
}

// ----------------------------------------------------------------------------
// Test 11: Provenance and explanation completeness
// ----------------------------------------------------------------------------
#[test]
fn test_11_provenance_and_explanation_completeness() {
    let ws = setup_test_workspace("prov_complete");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(src.join("main.rs"), "fn main() { println!(\"Hello\"); }\n").unwrap();
    analyze(&ws).unwrap();

    let res = context(&ws, "main", 2000).unwrap();
    for f in &res.files {
        assert!(!f.provenance.is_empty());
        for p in &f.provenance {
            assert!(!p.trim().is_empty());
        }
    }

    let _ = fs::remove_dir_all(&ws);
}

// ----------------------------------------------------------------------------
// Test 12: Deterministic repeated retrieval
// ----------------------------------------------------------------------------
#[test]
fn test_12_deterministic_repeated_retrieval() {
    let angular_path = Path::new(r"C:\Users\Amal P S\OneDrive\Projects\Test\angular-ngrx-material-starter");
    if !angular_path.exists() {
        return;
    }

    let run1 = context(angular_path, "AuthEffects", 4000).unwrap();
    let run2 = context(angular_path, "AuthEffects", 4000).unwrap();
    let run3 = context(angular_path, "AuthEffects", 4000).unwrap();

    assert_eq!(run1.files.len(), run2.files.len());
    assert_eq!(run2.files.len(), run3.files.len());

    for i in 0..run1.files.len() {
        assert_eq!(run1.files[i].path, run2.files[i].path);
        assert_eq!(run2.files[i].path, run3.files[i].path);
        assert_eq!(run1.files[i].score, run2.files[i].score);
    }
}

// ----------------------------------------------------------------------------
// Test 13: External and unresolved dependency handling
// ----------------------------------------------------------------------------
#[test]
fn test_13_external_and_unresolved_dependency_handling() {
    let ws = setup_test_workspace("ext_unres");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    fs::write(
        src.join("app.ts"),
        r#"import { Component } from '@angular/core';
import { MissingLocal } from './missing_local';
export class App {}
"#,
    ).unwrap();

    analyze(&ws).unwrap();

    let res = context(&ws, "App", 2000).unwrap();
    let paths: Vec<String> = res.files.iter().map(|f| f.path.clone()).collect();

    // Neither @angular/core nor missing_local should be added as valid first-party context files
    assert!(!paths.contains(&"@angular/core".to_string()));
    assert!(!paths.contains(&"./missing_local".to_string()));

    let _ = fs::remove_dir_all(&ws);
}

// ----------------------------------------------------------------------------
// Test 14: Go and Java compatibility
// ----------------------------------------------------------------------------
#[test]
fn test_14_go_and_java_compatibility() {
    let ws = setup_test_workspace("go_java");
    let go_src = ws.join("go");
    let java_src = ws.join("java");
    fs::create_dir_all(&go_src).unwrap();
    fs::create_dir_all(&java_src).unwrap();

    fs::write(
        go_src.join("server.go"),
        r#"package main
type Server struct { port int }
func NewServer(p int) *Server { return &Server{port: p} }
"#,
    ).unwrap();

    fs::write(
        java_src.join("UserService.java"),
        r#"package com.example;
public class UserService {
    public boolean authenticate(String u, String p) { return true; }
}
"#,
    ).unwrap();

    analyze(&ws).unwrap();

    // Query Go Server
    let res_go = context(&ws, "NewServer", 2000).unwrap();
    assert!(res_go.files.iter().any(|f| f.path.contains("server.go")));

    // Query Java UserService
    let res_java = context(&ws, "UserService authenticate", 2000).unwrap();
    assert!(res_java.files.iter().any(|f| f.path.contains("UserService.java")));

    let _ = fs::remove_dir_all(&ws);
}

// ----------------------------------------------------------------------------
// Test 15: Missing or invalidated entities
// ----------------------------------------------------------------------------
#[test]
fn test_15_missing_or_invalidated_entities() {
    let ws = setup_test_workspace("invalidated");
    let src = ws.join("src");
    fs::create_dir_all(&src).unwrap();

    let file_path = src.join("temp_service.rs");
    fs::write(&file_path, "pub struct TempService;\n").unwrap();
    analyze(&ws).unwrap();

    // Verify it's present
    let res1 = context(&ws, "TempService", 2000).unwrap();
    assert!(res1.files.iter().any(|f| f.path.contains("temp_service.rs")));

    // Delete the file
    let _ = fs::remove_file(&file_path);
    analyze(&ws).unwrap();

    // Deleted file should no longer be returned as an active context file
    let res2 = context(&ws, "TempService", 2000).unwrap();
    assert!(!res2.files.iter().any(|f| f.path.contains("temp_service.rs")));

    let _ = fs::remove_dir_all(&ws);
}
