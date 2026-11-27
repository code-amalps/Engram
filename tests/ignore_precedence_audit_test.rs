use engram::discovery::DiscoveryEngine;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn setup_repo(name: &str) -> PathBuf {
    let ws = std::env::temp_dir().join(format!("engram_audit_b_{}_{}", name, uuid::Uuid::new_v4()));
    fs::create_dir_all(&ws).unwrap();

    let _ = Command::new("git").args(["init"]).current_dir(&ws).output();
    let _ = Command::new("git")
        .args(["config", "user.name", "Audit B Tester"])
        .current_dir(&ws)
        .output();
    let _ = Command::new("git")
        .args(["config", "user.email", "audit_b@engram.local"])
        .current_dir(&ws)
        .output();

    ws
}

#[derive(serde::Serialize)]
struct MatrixRow {
    scenario: String,
    root_config: String,
    engramignore: String,
    gitignore: String,
    path_tested: String,
    is_dir: bool,
    is_excluded: bool,
    rule_source: Option<String>,
    reason: String,
    expected_outcome: String,
    matches_expected: bool,
}

#[test]
fn test_audit_b_interaction_matrix() {
    let ws = setup_repo("matrix");

    let build_dir = ws.join("build");
    fs::create_dir_all(&build_dir).unwrap();
    let keep_rs = build_dir.join("keep.rs");
    let other_rs = build_dir.join("other.rs");
    fs::write(&keep_rs, "pub fn keep() {}\n").unwrap();
    fs::write(&other_rs, "pub fn other() {}\n").unwrap();

    let mut rows: Vec<MatrixRow> = Vec::new();

    // 1. Root config: Exclude build/, engramignore: None, gitignore: None
    {
        fs::write(
            ws.join("engram.yaml"),
            "ignore:\n  exclude:\n    - 'build/'\n",
        )
        .unwrap();
        let engine = DiscoveryEngine::for_root(&ws);
        let dec_file = engine.explain(&keep_rs, false);
        let dec_dir = engine.explain(&build_dir, true);
        assert!(dec_file.is_excluded);
        assert!(dec_dir.is_excluded);
        rows.push(MatrixRow {
            scenario: "1. Root exclude build/ only".into(),
            root_config: "exclude: ['build/']".into(),
            engramignore: "None".into(),
            gitignore: "None".into(),
            path_tested: "build/keep.rs".into(),
            is_dir: false,
            is_excluded: dec_file.is_excluded,
            rule_source: dec_file.rule_source,
            reason: dec_file.reason,
            expected_outcome: "Excluded by engram.yaml".into(),
            matches_expected: true,
        });
        let _ = fs::remove_file(ws.join("engram.yaml"));
    }

    // 2. Root config: Exclude build/, engramignore: Include build/keep.rs
    // In gitignore semantics: if parent dir build/ is fully excluded by higher layer, can lower layer un-ignore?
    // In our precedence: engram.yaml (root config) has HIGHER precedence than .engramignore.
    // So if root excludes build/, .engramignore cannot override it!
    {
        fs::write(
            ws.join("engram.yaml"),
            "ignore:\n  exclude:\n    - 'build/'\n",
        )
        .unwrap();
        fs::write(ws.join(".engramignore"), "!build/keep.rs\n").unwrap();
        let engine = DiscoveryEngine::for_root(&ws);
        let dec = engine.explain(&keep_rs, false);
        // Precedence: engram.yaml excludes parent directory 'build', so it remains excluded
        rows.push(MatrixRow {
            scenario: "2. Root exclude build/, engramignore include keep.rs".into(),
            root_config: "exclude: ['build/']".into(),
            engramignore: "!build/keep.rs".into(),
            gitignore: "None".into(),
            path_tested: "build/keep.rs".into(),
            is_dir: false,
            is_excluded: dec.is_excluded,
            rule_source: dec.rule_source.clone(),
            reason: dec.reason.clone(),
            expected_outcome: "Excluded (Root config has higher precedence than .engramignore)"
                .into(),
            matches_expected: dec.is_excluded,
        });
        let _ = fs::remove_file(ws.join("engram.yaml"));
        let _ = fs::remove_file(ws.join(".engramignore"));
    }

    // 3. No root rule; engramignore: Exclude build/; gitignore: Include build/keep.rs
    // .engramignore has HIGHER precedence than .gitignore.
    // So build/ is excluded by .engramignore, .gitignore cannot override it.
    {
        fs::write(ws.join(".engramignore"), "build/\n").unwrap();
        fs::write(ws.join(".gitignore"), "!build/keep.rs\n").unwrap();
        let engine = DiscoveryEngine::for_root(&ws);
        let dec = engine.explain(&keep_rs, false);
        rows.push(MatrixRow {
            scenario: "3. No root rule, engramignore exclude build/, gitignore include keep.rs"
                .into(),
            root_config: "None".into(),
            engramignore: "build/".into(),
            gitignore: "!build/keep.rs".into(),
            path_tested: "build/keep.rs".into(),
            is_dir: false,
            is_excluded: dec.is_excluded,
            rule_source: dec.rule_source.clone(),
            reason: dec.reason.clone(),
            expected_outcome: "Excluded (.engramignore has higher precedence than .gitignore)"
                .into(),
            matches_expected: dec.is_excluded
                && dec.rule_source.as_deref() == Some(".engramignore"),
        });
        let _ = fs::remove_file(ws.join(".engramignore"));
        let _ = fs::remove_file(ws.join(".gitignore"));
    }

    // 4. No root rule; engramignore: Include build/keep.rs; gitignore: Exclude build/
    // .engramignore has HIGHER precedence than .gitignore.
    // .engramignore includes build/keep.rs, but .gitignore excludes build/.
    // Does .engramignore's whitelist override .gitignore?
    {
        fs::write(ws.join(".engramignore"), "!build/keep.rs\n").unwrap();
        fs::write(ws.join(".gitignore"), "build/*\n").unwrap();
        let engine = DiscoveryEngine::for_root(&ws);
        let dec = engine.explain(&keep_rs, false);
        let dec_other = engine.explain(&other_rs, false);
        assert!(
            !dec.is_excluded,
            "keep.rs should be whitelisted by .engramignore"
        );
        assert!(
            dec_other.is_excluded,
            "other.rs should be excluded by .gitignore"
        );
        rows.push(MatrixRow {
            scenario: "4. No root rule, engramignore include keep.rs, gitignore exclude build/*"
                .into(),
            root_config: "None".into(),
            engramignore: "!build/keep.rs".into(),
            gitignore: "build/*".into(),
            path_tested: "build/keep.rs".into(),
            is_dir: false,
            is_excluded: dec.is_excluded,
            rule_source: dec.rule_source.clone(),
            reason: dec.reason.clone(),
            expected_outcome: "Included (Whitelisted in .engramignore over gitignore exclude)"
                .into(),
            matches_expected: !dec.is_excluded,
        });
        let _ = fs::remove_file(ws.join(".engramignore"));
        let _ = fs::remove_file(ws.join(".gitignore"));
    }

    // 5. Exclude build/ in root, Include build/keep.rs in engramignore AND gitignore
    {
        fs::write(
            ws.join("engram.yaml"),
            "ignore:\n  exclude:\n    - 'build/'\n",
        )
        .unwrap();
        fs::write(ws.join(".engramignore"), "!build/keep.rs\n").unwrap();
        fs::write(ws.join(".gitignore"), "!build/keep.rs\n").unwrap();
        let engine = DiscoveryEngine::for_root(&ws);
        let dec = engine.explain(&keep_rs, false);
        println!(
            "Scenario 5 decision: is_excluded={}, reason={}, source={:?}",
            dec.is_excluded, dec.reason, dec.rule_source
        );
        assert!(dec.is_excluded);
        rows.push(MatrixRow {
            scenario: "5. Root exclude build/, both engramignore & gitignore include keep.rs"
                .into(),
            root_config: "exclude: ['build/']".into(),
            engramignore: "!build/keep.rs".into(),
            gitignore: "!build/keep.rs".into(),
            path_tested: "build/keep.rs".into(),
            is_dir: false,
            is_excluded: dec.is_excluded,
            rule_source: dec.rule_source.clone(),
            reason: dec.reason.clone(),
            expected_outcome: "Excluded (Root config takes precedence over lower includes)".into(),
            matches_expected: dec.is_excluded,
        });
        let _ = fs::remove_file(ws.join("engram.yaml"));
        let _ = fs::remove_file(ws.join(".engramignore"));
        let _ = fs::remove_file(ws.join(".gitignore"));
    }

    // 6. No root rule, Exclude build/ in engramignore, No override in gitignore
    {
        fs::write(ws.join(".engramignore"), "build/\n").unwrap();
        let engine = DiscoveryEngine::for_root(&ws);
        let dec = engine.explain(&keep_rs, false);
        assert!(dec.is_excluded);
        rows.push(MatrixRow {
            scenario: "6. No root rule, engramignore exclude build/, gitignore None".into(),
            root_config: "None".into(),
            engramignore: "build/".into(),
            gitignore: "None".into(),
            path_tested: "build/keep.rs".into(),
            is_dir: false,
            is_excluded: dec.is_excluded,
            rule_source: dec.rule_source.clone(),
            reason: dec.reason.clone(),
            expected_outcome: "Excluded by .engramignore".into(),
            matches_expected: dec.is_excluded
                && dec.rule_source.as_deref() == Some(".engramignore"),
        });
        let _ = fs::remove_file(ws.join(".engramignore"));
    }

    // 7. Universal safety exclusion vs negation in every lower layer
    // node_modules/ is a universal safety directory.
    // Attempt negation !node_modules/** in engram.yaml, .engramignore, and .gitignore.
    {
        let nm = ws.join("node_modules").join("pkg");
        fs::create_dir_all(&nm).unwrap();
        let nm_file = nm.join("index.js");
        fs::write(&nm_file, "console.log('pwn');\n").unwrap();

        fs::write(
            ws.join("engram.yaml"),
            "ignore:\n  include:\n    - '!node_modules/**'\n",
        )
        .unwrap();
        fs::write(ws.join(".engramignore"), "!node_modules/**\n").unwrap();
        fs::write(ws.join(".gitignore"), "!node_modules/**\n").unwrap();

        let engine = DiscoveryEngine::for_root(&ws);
        let dec = engine.explain(&nm_file, false);
        assert!(
            dec.is_excluded,
            "node_modules must remain excluded despite all lower negation rules!"
        );
        assert_eq!(dec.rule_source.as_deref(), Some("UniversalSafety"));

        rows.push(MatrixRow {
            scenario:
                "7. Universal safety directory (node_modules) vs negations in all lower layers"
                    .into(),
            root_config: "include: ['!node_modules/**']".into(),
            engramignore: "!node_modules/**".into(),
            gitignore: "!node_modules/**".into(),
            path_tested: "node_modules/pkg/index.js".into(),
            is_dir: false,
            is_excluded: dec.is_excluded,
            rule_source: dec.rule_source.clone(),
            reason: dec.reason.clone(),
            expected_outcome: "Excluded by UniversalSafety (Universal rules cannot be bypassed)"
                .into(),
            matches_expected: dec.is_excluded
                && dec.rule_source.as_deref() == Some("UniversalSafety"),
        });

        let _ = fs::remove_file(ws.join("engram.yaml"));
        let _ = fs::remove_file(ws.join(".engramignore"));
        let _ = fs::remove_file(ws.join(".gitignore"));
    }

    // 8. Universal secret exclusion (.env) vs negations in all lower layers
    {
        let env_file = ws.join(".env");
        fs::write(&env_file, "SECRET_TOKEN=xyz\n").unwrap();

        fs::write(
            ws.join("engram.yaml"),
            "ignore:\n  include:\n    - '!.env'\n",
        )
        .unwrap();
        fs::write(ws.join(".engramignore"), "!.env\n").unwrap();
        fs::write(ws.join(".gitignore"), "!.env\n").unwrap();

        let engine = DiscoveryEngine::for_root(&ws);
        let dec = engine.explain(&env_file, false);
        assert!(
            dec.is_excluded,
            ".env must remain excluded despite all lower negation rules!"
        );
        assert_eq!(dec.rule_source.as_deref(), Some("SecretFile"));

        rows.push(MatrixRow {
            scenario: "8. Universal secret file (.env) vs negations in all lower layers".into(),
            root_config: "include: ['!.env']".into(),
            engramignore: "!.env".into(),
            gitignore: "!.env".into(),
            path_tested: ".env".into(),
            is_dir: false,
            is_excluded: dec.is_excluded,
            rule_source: dec.rule_source.clone(),
            reason: dec.reason.clone(),
            expected_outcome: "Excluded by SecretFile (Secrets cannot be included by lower rules)"
                .into(),
            matches_expected: dec.is_excluded && dec.rule_source.as_deref() == Some("SecretFile"),
        });

        let _ = fs::remove_file(ws.join("engram.yaml"));
        let _ = fs::remove_file(ws.join(".engramignore"));
        let _ = fs::remove_file(ws.join(".gitignore"));
    }

    // Save truth table to an explicit env-configured path if provided, else inside isolated test workspace
    let out_path = std::env::var_os("ENGRAM_AUDIT_B_TRUTH_TABLE_OUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| ws.join("phase_9_5_3_1_audit_b_truth_table.json"));
    if let Some(parent) = out_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json_bytes = serde_json::to_string_pretty(&rows).unwrap();
    fs::write(&out_path, &json_bytes).unwrap();
    assert!(out_path.exists(), "Truth table output must be written");

    let _ = fs::remove_dir_all(&ws);
}

#[test]
fn test_audit_b_pattern_matching_semantics() {
    let ws = setup_repo("patterns");

    // Test cases for:
    // - Multiple matching rules (last match behavior)
    // - Directory-only rules (build/ vs build file)
    // - Wildcards (*, **, ?)
    // - Leading slash vs relative
    // - Empty configuration file
    // - Duplicate / conflicting rules

    // 1. Last match behavior in .engramignore
    fs::write(
        ws.join(".engramignore"),
        "*.tmp\n!important.tmp\nignore_me.rs\n!ignore_me.rs\n",
    )
    .unwrap();
    let engine = DiscoveryEngine::for_root(&ws);

    let imp_tmp = ws.join("important.tmp");
    let unimp_tmp = ws.join("unimportant.tmp");
    let ign_rs = ws.join("ignore_me.rs");

    assert!(
        !engine.explain(&imp_tmp, false).is_excluded,
        "important.tmp should be included by last match"
    );
    assert!(
        engine.explain(&unimp_tmp, false).is_excluded,
        "unimportant.tmp should be excluded"
    );
    assert!(
        !engine.explain(&ign_rs, false).is_excluded,
        "!ignore_me.rs should be included by last match"
    );

    // 2. Directory-only rule with trailing slash
    fs::write(ws.join(".engramignore"), "target_dir/\n").unwrap();
    let engine2 = DiscoveryEngine::for_root(&ws);
    let dir = ws.join("target_dir");
    let file_named_dir = ws.join("target_dir");
    assert!(
        engine2.explain(&dir, true).is_excluded,
        "directory target_dir/ is excluded"
    );
    let dec_f = engine2.explain(&file_named_dir, false);
    println!(
        "Dir-only test: is_excluded={}, reason={}, source={:?}",
        dec_f.is_excluded, dec_f.reason, dec_f.rule_source
    );
    assert!(!dec_f.is_excluded, "file named target_dir is NOT excluded");

    // 3. Empty config file fails safely and produces zero exclusions
    fs::write(ws.join(".engramignore"), "").unwrap();
    let engine3 = DiscoveryEngine::for_root(&ws);
    let normal_rs = ws.join("src").join("lib.rs");
    assert!(!engine3.explain(&normal_rs, false).is_excluded);

    let _ = fs::remove_dir_all(&ws);
}
