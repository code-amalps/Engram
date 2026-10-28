use serde_json::json;
use std::fs;
use std::path::PathBuf;

use engram::instructions::{END_MARKER, START_MARKER};
use engram::setup::{
    all_providers, get_provider, inspect_mcp_config, list_targets, orchestrate_setup, setup_client,
    verify_mcp_health, verify_mcp_health_with_timeout, SetupOptions, SetupScope,
    GOVERNED_TOOL_NAMES,
};

fn temp_workspace(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "engram_setup_test_{}_{}",
        name,
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn test_setup_providers_target_matrix_and_scopes() {
    let providers = all_providers();
    assert_eq!(providers.len(), 4, "Must have exactly 4 providers");

    let cursor = get_provider("cursor").expect("cursor provider must exist");
    assert_eq!(cursor.id(), "cursor");
    assert_eq!(cursor.scope(), SetupScope::Project);
    assert!(cursor.capabilities().project_mcp);
    assert!(!cursor.capabilities().user_mcp);

    let vscode = get_provider("vscode").expect("vscode provider must exist");
    assert_eq!(vscode.id(), "vscode");
    assert_eq!(vscode.scope(), SetupScope::Project);
    assert!(vscode.capabilities().project_mcp);

    let claude = get_provider("claude").expect("claude provider must exist");
    assert_eq!(claude.id(), "claude");
    assert_eq!(claude.scope(), SetupScope::User);
    assert!(claude.capabilities().user_mcp);

    let windsurf = get_provider("windsurf").expect("windsurf provider must exist");
    assert_eq!(windsurf.id(), "windsurf");
    assert_eq!(windsurf.scope(), SetupScope::User);
    assert!(windsurf.capabilities().user_mcp);

    // List targets output
    let list = list_targets();
    let supp = list["supported"].as_array().unwrap();
    assert_eq!(supp.len(), 4);

    // Verify instruction file paths strictly target project workspace root
    let dummy_root = PathBuf::from(if cfg!(windows) {
        "C:\\projects\\my_app"
    } else {
        "/projects/my_app"
    });
    assert_eq!(
        cursor.default_instruction_path(&dummy_root).unwrap(),
        dummy_root.join(".cursorrules")
    );
    assert_eq!(
        vscode.default_instruction_path(&dummy_root).unwrap(),
        dummy_root.join(".github").join("copilot-instructions.md")
    );
    assert_eq!(
        claude.default_instruction_path(&dummy_root).unwrap(),
        dummy_root.join("CLAUDE.md")
    );
    assert_eq!(
        windsurf.default_instruction_path(&dummy_root).unwrap(),
        dummy_root.join(".windsurfrules")
    );

    assert!(
        cursor
            .default_instruction_path(&dummy_root)
            .unwrap()
            .starts_with(&dummy_root)
    );
    assert!(
        vscode
            .default_instruction_path(&dummy_root)
            .unwrap()
            .starts_with(&dummy_root)
    );
    assert!(
        claude
            .default_instruction_path(&dummy_root)
            .unwrap()
            .starts_with(&dummy_root)
    );
    assert!(
        windsurf
            .default_instruction_path(&dummy_root)
            .unwrap()
            .starts_with(&dummy_root)
    );
}

#[test]
fn test_cursor_setup_lifecycle_dry_run_apply_and_remove() {
    let ws = temp_workspace("cursor_lifecycle");
    let cursor_dir = ws.join(".cursor");
    fs::create_dir_all(&cursor_dir).unwrap();
    let mcp_file = cursor_dir.join("mcp.json");
    let rules_file = ws.join(".cursorrules");

    // 1. Dry run on clean workspace: must not modify disk
    let dry_res = setup_client(&ws, "cursor", true, false, false).unwrap();
    assert_eq!(dry_res["dry_run"], true);
    assert!(!mcp_file.exists(), "Dry run must not create config file");

    let dry_orch = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("cursor".to_string()),
            check: false,
            dry_run: true,
            apply: false,
            force: false,
            status: false,
            remove: false,
        },
    )
    .unwrap();
    assert_eq!(dry_orch.mode, "DRY_RUN");
    assert_eq!(dry_orch.changes_applied, 0);
    assert!(!mcp_file.exists());
    assert!(!rules_file.exists());

    // 2. Apply on clean workspace: creates config and managed instruction rules
    let apply_orch = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("cursor".to_string()),
            check: false,
            dry_run: false,
            apply: true,
            force: false,
            status: false,
            remove: false,
        },
    )
    .unwrap();
    assert_eq!(apply_orch.mode, "APPLIED");
    assert_eq!(apply_orch.changes_applied, 2); // 1 mcp + 1 instruction
    assert!(mcp_file.exists(), "mcp.json must be created");
    assert!(rules_file.exists(), ".cursorrules must be created");

    let mcp_content = fs::read_to_string(&mcp_file).unwrap();
    let mcp_json: serde_json::Value = serde_json::from_str(&mcp_content).unwrap();
    assert!(mcp_json["mcpServers"]["engram"].is_object());

    let rules_content = fs::read_to_string(&rules_file).unwrap();
    assert!(rules_content.contains(START_MARKER));
    assert!(rules_content.contains(END_MARKER));
    assert!(rules_content.contains("engram_preflight"));

    // 3. Repeated Apply: Strict Idempotency (zero change, zero duplicate blocks)
    let reapply_orch = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("cursor".to_string()),
            check: false,
            dry_run: false,
            apply: true,
            force: false,
            status: false,
            remove: false,
        },
    )
    .unwrap();
    assert_eq!(
        reapply_orch.changes_applied, 0,
        "Repeated apply must be strictly idempotent"
    );

    let rules_reapply = fs::read_to_string(&rules_file).unwrap();
    assert_eq!(
        rules_reapply.matches(START_MARKER).count(),
        1,
        "Must have exactly 1 start marker"
    );
    assert_eq!(
        rules_reapply.matches(END_MARKER).count(),
        1,
        "Must have exactly 1 end marker"
    );
    assert_eq!(rules_reapply, rules_content, "File bytes must be identical");

    // 4. Surgical Remove: Removes Engram and strips managed block
    let remove_orch = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("cursor".to_string()),
            check: false,
            dry_run: false,
            apply: true,
            force: false,
            status: false,
            remove: true,
        },
    )
    .unwrap();
    assert_eq!(remove_orch.mode, "REMOVED");

    // Since mcp.json and .cursorrules only had Engram content, both empty files are safely cleaned
    assert!(
        !rules_file.exists(),
        ".cursorrules should be cleaned as it had zero user content"
    );
}

#[test]
fn test_safe_merge_preserves_third_party_mcp_servers_and_user_instructions() {
    let ws = temp_workspace("safe_merge");
    let vscode_dir = ws.join(".vscode");
    fs::create_dir_all(&vscode_dir).unwrap();
    let mcp_file = vscode_dir.join("mcp.json");

    let github_dir = ws.join(".github");
    fs::create_dir_all(&github_dir).unwrap();
    let copilot_file = github_dir.join("copilot-instructions.md");

    // Pre-existing user third-party MCP configuration with unrelated servers and settings
    let initial_mcp = json!({
        "customSetting": "preserve_this_value",
        "mcpServers": {
            "github": {
                "command": "gh",
                "args": ["mcp", "serve"]
            },
            "postgres": {
                "command": "npx",
                "args": ["-y", "@modelcontextprotocol/server-postgres", "postgresql://localhost/mydb"]
            }
        }
    });
    fs::write(
        &mcp_file,
        serde_json::to_string_pretty(&initial_mcp).unwrap(),
    )
    .unwrap();

    // Pre-existing user instruction rules
    let initial_instructions =
        "# User Project Guidelines\n- Always run tests before push\n- Keep functions pure\n";
    fs::write(&copilot_file, initial_instructions).unwrap();

    // Apply VS Code setup
    let res = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("vscode".to_string()),
            check: false,
            dry_run: false,
            apply: true,
            force: false,
            status: false,
            remove: false,
        },
    )
    .unwrap();
    assert_eq!(res.mode, "APPLIED");

    // Verify mcp.json contains github, postgres, customSetting, AND engram
    let mcp_after = fs::read_to_string(&mcp_file).unwrap();
    let mcp_json: serde_json::Value = serde_json::from_str(&mcp_after).unwrap();
    assert_eq!(mcp_json["customSetting"], "preserve_this_value");
    assert!(
        mcp_json["mcpServers"]["github"].is_object(),
        "Must preserve github server"
    );
    assert!(
        mcp_json["mcpServers"]["postgres"].is_object(),
        "Must preserve postgres server"
    );
    assert!(
        mcp_json["mcpServers"]["engram"].is_object(),
        "Must add engram server"
    );

    // Verify copilot-instructions.md contains user guidelines AND engram managed block
    let inst_after = fs::read_to_string(&copilot_file).unwrap();
    assert!(inst_after.contains("# User Project Guidelines"));
    assert!(inst_after.contains("- Always run tests before push"));
    assert!(inst_after.contains(START_MARKER));
    assert!(inst_after.contains("engram_preflight"));

    // Remove Engram: Must surgically remove ONLY engram and preserve github, postgres, customSetting, and user guidelines
    let rem_res = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("vscode".to_string()),
            check: false,
            dry_run: false,
            apply: true,
            force: false,
            status: false,
            remove: true,
        },
    )
    .unwrap();
    assert_eq!(rem_res.mode, "REMOVED");

    let mcp_final = fs::read_to_string(&mcp_file).unwrap();
    let mcp_final_json: serde_json::Value = serde_json::from_str(&mcp_final).unwrap();
    assert_eq!(mcp_final_json["customSetting"], "preserve_this_value");
    assert!(
        mcp_final_json["mcpServers"]["github"].is_object(),
        "Must still preserve github"
    );
    assert!(
        mcp_final_json["mcpServers"]["postgres"].is_object(),
        "Must still preserve postgres"
    );
    assert!(
        mcp_final_json["mcpServers"].get("engram").is_none(),
        "Engram must be removed"
    );

    let inst_final = fs::read_to_string(&copilot_file).unwrap();
    assert!(inst_final.contains("# User Project Guidelines"));
    assert!(inst_final.contains("- Always run tests before push"));
    assert!(
        !inst_final.contains(START_MARKER),
        "Engram managed block must be stripped"
    );
}

#[test]
fn test_malformed_json_fails_safely_without_corruption() {
    let ws = temp_workspace("malformed_json");
    let cursor_dir = ws.join(".cursor");
    fs::create_dir_all(&cursor_dir).unwrap();
    let mcp_file = cursor_dir.join("mcp.json");

    // Malformed JSON with syntax error
    let malformed_text = "{\n  \"mcpServers\": {\n    \"broken\": [ missing quote\n  }\n}";
    fs::write(&mcp_file, malformed_text).unwrap();

    // Inspect config: must flag MALFORMED_JSON conflict
    let (status, conflict, details) =
        inspect_mcp_config(&mcp_file, "engram", &ws.to_string_lossy()).unwrap();
    assert_eq!(status, "MALFORMED_JSON");
    assert!(conflict);
    assert!(details.unwrap().contains("malformed JSON"));

    // Attempt apply without force: must return Err and NOT corrupt the file
    let apply_attempt = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("cursor".to_string()),
            check: false,
            dry_run: false,
            apply: true,
            force: false,
            status: false,
            remove: false,
        },
    );
    assert!(apply_attempt.is_err(), "Must fail safely on malformed JSON");
    let err_str = apply_attempt.unwrap_err().to_string();
    assert!(err_str.contains("Conflict detected") || err_str.contains("malformed JSON"));

    // Verify file content is 100% untouched
    let content_after = fs::read_to_string(&mcp_file).unwrap();
    assert_eq!(
        content_after, malformed_text,
        "Malformed file must not be modified or overwritten"
    );
}

#[test]
fn test_conflict_detection_and_force_override() {
    let ws = temp_workspace("conflict_detection");
    let cursor_dir = ws.join(".cursor");
    fs::create_dir_all(&cursor_dir).unwrap();
    let mcp_file = cursor_dir.join("mcp.json");

    // Conflicting engram configuration pointing to /usr/local/bin/old_engram
    let conflict_json = json!({
        "mcpServers": {
            "engram": {
                "command": "/usr/local/bin/old_engram",
                "args": ["--legacy-flag"]
            }
        }
    });
    fs::write(
        &mcp_file,
        serde_json::to_string_pretty(&conflict_json).unwrap(),
    )
    .unwrap();

    // Setup without force: should detect conflict and fail
    let res_no_force = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("cursor".to_string()),
            check: false,
            dry_run: false,
            apply: true,
            force: false,
            status: false,
            remove: false,
        },
    );
    assert!(res_no_force.is_err());
    assert!(
        res_no_force
            .unwrap_err()
            .to_string()
            .contains("Conflict detected")
    );

    // Setup with force: overrides conflicting engram configuration safely
    let res_force = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("cursor".to_string()),
            check: false,
            dry_run: false,
            apply: true,
            force: true,
            status: false,
            remove: false,
        },
    );
    assert!(res_force.is_ok(), "Setup with --force must succeed");

    let content_after = fs::read_to_string(&mcp_file).unwrap();
    let v: serde_json::Value = serde_json::from_str(&content_after).unwrap();
    let engram_cfg = &v["mcpServers"]["engram"];
    let args = engram_cfg["args"].as_array().unwrap();
    assert_eq!(args[0], "-C");
    assert_eq!(args[2], "mcp");
}

#[test]
fn test_verify_mcp_health_validation_and_clean_termination() {
    let bin = env!("CARGO_BIN_EXE_engram");
    let ws = temp_workspace("health_check");

    // Initialize workspace
    let init_status = std::process::Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .status()
        .expect("failed to run engram init");
    assert!(init_status.success());

    // Health check on real binary: must succeed and verify all 15 governed tools
    let health = verify_mcp_health(bin, &ws);
    assert!(
        health.is_ok(),
        "Health check must succeed on real binary: {:?}",
        health.err()
    );
    let tool_count = health.unwrap();
    assert_eq!(tool_count, 16, "Must verify exactly 16 governed tools");
    assert_eq!(tool_count, GOVERNED_TOOL_NAMES.len());
    assert!(
        GOVERNED_TOOL_NAMES.contains(&"engram_prepare_task"),
        "engram_prepare_task must be included in governed tools"
    );
    assert!(
        GOVERNED_TOOL_NAMES.contains(&"engram_ratify_decision"),
        "engram_ratify_decision must be included in governed tools"
    );
}

#[test]
fn test_verify_mcp_health_nonexistent_binary_fails_cleanly() {
    let ws = temp_workspace("health_nonexistent");
    let bogus_bin = if cfg!(windows) {
        "C:\\nonexistent_dir_12345\\bogus_engram.exe"
    } else {
        "/tmp/nonexistent_dir_12345/bogus_engram"
    };

    let health = verify_mcp_health(bogus_bin, &ws);
    assert!(health.is_err());
    let err_msg = health.unwrap_err().to_string();
    assert!(err_msg.contains("not found") || err_msg.contains("Failed to spawn"));
}

#[test]
fn test_verify_mcp_health_bounded_timeout() {
    let ws = temp_workspace("health_timeout");
    // Run bounded timeout with 1 second on real binary - should complete well within 5s
    let bin = env!("CARGO_BIN_EXE_engram");
    let init_status = std::process::Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .status()
        .expect("init failed");
    assert!(init_status.success());

    let health = verify_mcp_health_with_timeout(bin, &ws, 10);
    assert!(health.is_ok());
    assert_eq!(health.unwrap(), 16);
}

#[test]
fn test_vscode_native_schema_lifecycle_and_workspace_folder_expansion() {
    let ws = temp_workspace("vscode_native_schema");
    let bin = env!("CARGO_BIN_EXE_engram");

    // Initialize workspace
    let init_status = std::process::Command::new(bin)
        .arg("-C")
        .arg(&ws)
        .arg("init")
        .status()
        .expect("init failed");
    assert!(init_status.success());

    let vscode_dir = ws.join(".vscode");
    let mcp_file = vscode_dir.join("mcp.json");

    // 1. Apply VS Code setup on fresh workspace
    let res = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("vscode".to_string()),
            check: false,
            dry_run: false,
            apply: true,
            force: false,
            status: false,
            remove: false,
        },
    )
    .unwrap();
    assert_eq!(res.mode, "APPLIED");
    assert!(mcp_file.exists());

    // 2. Assert native VS Code "servers" schema format
    let content = fs::read_to_string(&mcp_file).unwrap();
    let json: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(
        json["servers"]["engram"].is_object(),
        "Native VS Code schema must use 'servers' root key: {}",
        content
    );
    assert_eq!(
        json["servers"]["engram"]["type"], "stdio",
        "Must specify type stdio"
    );
    let args = json["servers"]["engram"]["args"].as_array().unwrap();
    assert_eq!(args[0], "-C");
    assert_eq!(args[1], "${workspaceFolder}");
    assert_eq!(args[2], "mcp");

    // 3. Inspect configuration
    let root_canon = fs::canonicalize(&ws)
        .unwrap()
        .to_string_lossy()
        .to_string();
    let current_bin = engram::setup::get_engram_binary_path();
    let (status, conflict, details) = inspect_mcp_config(&mcp_file, &current_bin, &root_canon).unwrap();
    assert_eq!(status, "CONFIGURED_HEALTHY", "Details: {:?}", details);
    assert!(!conflict);
    assert!(details.is_none());

    // 4. Repeated apply is idempotent
    let reapply = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("vscode".to_string()),
            check: false,
            dry_run: false,
            apply: true,
            force: false,
            status: false,
            remove: false,
        },
    )
    .unwrap();
    assert_eq!(reapply.changes_applied, 0, "Repeated apply must be strictly idempotent");

    // 5. Remove surgically cleans engram from servers
    let rem = orchestrate_setup(
        &ws,
        SetupOptions {
            client: Some("vscode".to_string()),
            check: false,
            dry_run: false,
            apply: true,
            force: false,
            status: false,
            remove: true,
        },
    )
    .unwrap();
    assert_eq!(rem.mode, "REMOVED");
    if mcp_file.exists() {
        let cleaned: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&mcp_file).unwrap()).unwrap();
        assert!(cleaned.get("servers").and_then(|s| s.get("engram")).is_none());
    }
}
