//! Comprehensive test suite validating frictionless, editor-agnostic Engram integration:
//! - Non-destructive managed blocks
//! - Malformed/duplicate block rejection
//! - CRLF/LF preservation
//! - MCP configuration merging & conflict detection
//! - Non-intrusive `init` invariants (0 git hook / editor interference)
//! - Prompt generation with boundaries & completeness tracking

use std::{fs, path::PathBuf};

use engram::instructions::{
    self, DEFAULT_INSTRUCTION_BODY, END_MARKER, ManagedBlockAction, START_MARKER,
};
use engram::prompt::{self, PromptFormat};
use engram::setup::{self, SetupOptions};

fn temp_test_dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!("engram_frictionless_test_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn test_instruction_managed_block_creation_when_file_missing() {
    let dir = temp_test_dir();
    let file = dir.join(".cursorrules");

    assert!(!file.exists());
    let res = instructions::sync_managed_block(&file, DEFAULT_INSTRUCTION_BODY, false).unwrap();
    assert_eq!(res.action, ManagedBlockAction::Created);
    assert!(file.exists());

    let content = fs::read_to_string(&file).unwrap();
    assert!(content.contains(START_MARKER));
    assert!(content.contains(END_MARKER));
    assert!(content.contains("Engram Architecture Context"));
}

#[test]
fn test_instruction_managed_block_safe_append_preserves_existing_user_rules() {
    let dir = temp_test_dir();
    let file = dir.join(".github").join("copilot-instructions.md");
    fs::create_dir_all(file.parent().unwrap()).unwrap();

    let existing_rules = "\
# Team Guidelines (DO NOT DELETE)
- Always use kebab-case for filenames
- All Angular components must use standalone: true
- Avoid direct DOM manipulation
";
    fs::write(&file, existing_rules).unwrap();

    let res = instructions::sync_managed_block(&file, DEFAULT_INSTRUCTION_BODY, false).unwrap();
    assert_eq!(res.action, ManagedBlockAction::AppendedPreserved);
    assert_eq!(res.preserved_lines, 4);

    let updated = fs::read_to_string(&file).unwrap();
    // 100% of existing content must be strictly preserved at the top
    assert!(updated.starts_with(existing_rules.trim_end()));
    // Managed block must be appended at the bottom
    assert!(updated.contains(START_MARKER));
    assert!(updated.contains(END_MARKER));
}

#[test]
fn test_instruction_managed_block_idempotency() {
    let dir = temp_test_dir();
    let file = dir.join("CLAUDE.md");
    let initial_text = "# Claude Rules\n\nBe concise.\n";
    fs::write(&file, initial_text).unwrap();

    // First run: appends
    let r1 = instructions::sync_managed_block(&file, DEFAULT_INSTRUCTION_BODY, false).unwrap();
    assert_eq!(r1.action, ManagedBlockAction::AppendedPreserved);
    let content_after_r1 = fs::read_to_string(&file).unwrap();

    // Second run: must be completely Unchanged
    let r2 = instructions::sync_managed_block(&file, DEFAULT_INSTRUCTION_BODY, false).unwrap();
    assert_eq!(r2.action, ManagedBlockAction::Unchanged);
    let content_after_r2 = fs::read_to_string(&file).unwrap();

    // Exactly zero diffs
    assert_eq!(content_after_r1, content_after_r2);
    // Exactly one start marker and one end marker
    assert_eq!(content_after_r2.matches(START_MARKER).count(), 1);
    assert_eq!(content_after_r2.matches(END_MARKER).count(), 1);
}

#[test]
fn test_instruction_managed_block_updates_in_place_when_body_changes() {
    let dir = temp_test_dir();
    let file = dir.join(".cursorrules");
    let initial = format!(
        "# Header\n\n{}\nOld instruction text\n{}\n\n# Footer\n",
        START_MARKER, END_MARKER
    );
    fs::write(&file, initial).unwrap();

    let new_body = "Updated instruction text v2";
    let res = instructions::sync_managed_block(&file, new_body, false).unwrap();
    assert_eq!(res.action, ManagedBlockAction::UpdatedInPlace);

    let updated = fs::read_to_string(&file).unwrap();
    assert!(updated.contains("# Header"));
    assert!(updated.contains("# Footer"));
    assert!(updated.contains("Updated instruction text v2"));
    assert!(!updated.contains("Old instruction text"));
}

#[test]
fn test_instruction_managed_block_refuses_malformed_missing_end_marker() {
    let dir = temp_test_dir();
    let file = dir.join(".windsurfrules");
    let malformed = format!(
        "# Rules\n{}\nSome instructions without end tag\n",
        START_MARKER
    );
    fs::write(&file, &malformed).unwrap();

    let res = instructions::sync_managed_block(&file, DEFAULT_INSTRUCTION_BODY, false);
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("Malformed Engram managed block"));

    // File content must remain 100% untouched
    let after = fs::read_to_string(&file).unwrap();
    assert_eq!(malformed, after);
}

#[test]
fn test_instruction_managed_block_refuses_duplicate_markers() {
    let dir = temp_test_dir();
    let file = dir.join("CLAUDE.md");
    let duplicate = format!(
        "{}\nblock1\n{}\n{}\nblock2\n{}\n",
        START_MARKER, END_MARKER, START_MARKER, END_MARKER
    );
    fs::write(&file, &duplicate).unwrap();

    let res = instructions::sync_managed_block(&file, DEFAULT_INSTRUCTION_BODY, false);
    assert!(res.is_err());
    let err_msg = res.unwrap_err().to_string();
    assert!(err_msg.contains("Duplicate Engram managed blocks"));

    // File content must remain untouched
    let after = fs::read_to_string(&file).unwrap();
    assert_eq!(duplicate, after);
}

#[test]
fn test_instruction_preserves_crlf_line_endings() {
    let dir = temp_test_dir();
    let file = dir.join(".cursorrules");
    let crlf_content = "# Windows Rules\r\n- Keep CRLF\r\n";
    fs::write(&file, crlf_content).unwrap();

    let res = instructions::sync_managed_block(&file, DEFAULT_INSTRUCTION_BODY, false).unwrap();
    assert_eq!(res.action, ManagedBlockAction::AppendedPreserved);

    let raw_bytes = fs::read(&file).unwrap();
    let as_string = String::from_utf8(raw_bytes).unwrap();
    assert!(as_string.contains("\r\n"));
    assert!(!as_string.contains("\n\n")); // No bare LFs introduced
}

#[test]
fn test_mcp_config_safe_merge_preserves_third_party_servers() {
    let dir = temp_test_dir();
    let mcp_path = dir.join(".cursor").join("mcp.json");
    fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();

    let initial_json = r#"{
  "mcpServers": {
    "postgres": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-postgres", "postgresql://localhost/mydb"]
    },
    "github": {
      "command": "docker",
      "args": ["run", "-i", "mcp/github"]
    }
  }
}"#;
    fs::write(&mcp_path, initial_json).unwrap();

    let opts = SetupOptions {
        client: Some("cursor".to_string()),
        check: false,
        dry_run: false,
        apply: true,
        force: false,
        status: false,
        remove: false,
    };
    let report = setup::orchestrate_setup(&dir, opts).unwrap();
    assert_eq!(report.mode, "APPLIED");
    assert_eq!(report.changes_applied, 2); // 1 MCP config + 1 instruction file

    let updated_text = fs::read_to_string(&mcp_path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&updated_text).unwrap();

    let servers = parsed["mcpServers"].as_object().unwrap();
    // Both original third-party servers MUST still exist intact!
    assert!(servers.contains_key("postgres"));
    assert!(servers.contains_key("github"));
    // Engram server must now be present
    assert!(servers.contains_key("engram"));
    assert_eq!(servers["postgres"]["command"], "npx");
    assert_eq!(servers["github"]["command"], "docker");
}

#[test]
fn test_mcp_config_detects_conflict_when_server_diverges() {
    let dir = temp_test_dir();
    let mcp_path = dir.join(".vscode").join("mcp.json");
    fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();

    // Existing engram server with different custom args
    let conflicting_json = r#"{
  "mcpServers": {
    "engram": {
      "command": "different-engram-binary",
      "args": ["--custom-arg"]
    }
  }
}"#;
    fs::write(&mcp_path, conflicting_json).unwrap();

    let opts = SetupOptions {
        client: Some("vscode".to_string()),
        check: true,
        dry_run: false,
        apply: false,
        force: false,
        status: false,
        remove: false,
    };
    let report = setup::orchestrate_setup(&dir, opts).unwrap();
    assert_eq!(report.conflicts_detected, 1);
    let item = &report.integrations[0];
    assert!(item.conflict);
    assert_eq!(item.config_status, "CONFLICT_DETECTED");

    // Applying without force MUST fail safely
    let apply_opts = SetupOptions {
        client: Some("vscode".to_string()),
        check: false,
        dry_run: false,
        apply: true,
        force: false,
        status: false,
        remove: false,
    };
    let apply_res = setup::orchestrate_setup(&dir, apply_opts);
    assert!(apply_res.is_err());
    assert!(
        apply_res
            .unwrap_err()
            .to_string()
            .contains("Conflict detected")
    );

    // File content must not have changed
    let after_text = fs::read_to_string(&mcp_path).unwrap();
    assert!(after_text.contains("different-engram-binary"));
}

#[test]
fn test_setup_dry_run_leaves_files_untouched() {
    let dir = temp_test_dir();
    let cursor_dir = dir.join(".cursor");
    fs::create_dir_all(&cursor_dir).unwrap();

    let opts = SetupOptions {
        client: Some("cursor".to_string()),
        check: false,
        dry_run: true,
        apply: false,
        force: false,
        status: false,
        remove: false,
    };
    let report = setup::orchestrate_setup(&dir, opts).unwrap();
    assert_eq!(report.mode, "DRY_RUN");
    assert_eq!(report.changes_applied, 0);

    // .cursor/mcp.json and .cursorrules MUST NOT be created on disk during dry-run
    assert!(!cursor_dir.join("mcp.json").exists());
    assert!(!dir.join(".cursorrules").exists());
}

#[test]
fn test_prompt_generation_completeness_and_bounds() {
    let dir = temp_test_dir();
    // Initialize minimal repository
    let engram_dir = dir.join(".engram");
    fs::create_dir_all(&engram_dir).unwrap();
    fs::write(
        engram_dir.join("constitution.yaml"),
        "schema_version: 1\nrules: []\n",
    )
    .unwrap();
    fs::write(
        engram_dir.join("config.yaml"),
        "schema_version: 1\ndefault_token_budget: 1200\n",
    )
    .unwrap();

    let src_dir = dir.join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        src_dir.join("auth.rs"),
        "pub struct AuthService;\nimpl AuthService {\n    pub fn authenticate(&self) -> bool { true }\n}\n",
    ).unwrap();

    // Run analyzer
    engram::analyzer::analyze(&dir).unwrap();

    // Generate prompt in markdown format
    let prompt_out = prompt::generate_prompt(
        &dir,
        "authenticate request with AuthService",
        800,
        PromptFormat::Markdown,
        false,
    )
    .unwrap();

    assert_eq!(prompt_out.task, "authenticate request with AuthService");
    assert!(prompt_out.content.contains("# Task"));
    assert!(
        prompt_out
            .content
            .contains("## Relevant Context (AST Grounded)")
    );
    assert!(prompt_out.content.contains("AuthService"));
    assert!(
        prompt_out
            .content
            .contains("## Change Boundaries & Constraints")
    );
    assert!(prompt_out.content.contains("## Context Completeness"));
    assert_eq!(prompt_out.completeness_status, "SUFFICIENT_GROUNDING");
}

#[test]
fn test_instructions_strip_managed_block_preserves_custom_content() {
    let dir = temp_test_dir();
    let file_path = dir.join(".github").join("copilot-instructions.md");
    fs::create_dir_all(file_path.parent().unwrap()).unwrap();

    let initial = "# Custom Project Guidelines\n- Always run npm test before opening PR\n\n<!-- BEGIN ENGRAM MANAGED BLOCK -->\n## Engram Architecture Context\n1. Check boundaries.\n<!-- END ENGRAM MANAGED BLOCK -->\n\n- Write unit tests for new features.\n";
    fs::write(&file_path, initial).unwrap();

    // Strip block
    let report = instructions::strip_managed_block(&file_path, false).unwrap();
    assert_eq!(
        report.action,
        instructions::ManagedBlockAction::RemovedPreserved
    );

    let content_after = fs::read_to_string(&file_path).unwrap();
    // Engram block must be removed
    assert!(!content_after.contains(instructions::START_MARKER));
    assert!(!content_after.contains(instructions::END_MARKER));
    // User content MUST be preserved!
    assert!(content_after.contains("# Custom Project Guidelines"));
    assert!(content_after.contains("Always run npm test"));
    assert!(content_after.contains("Write unit tests for new features"));
}

#[test]
fn test_instructions_strip_managed_block_deletes_empty_file() {
    let dir = temp_test_dir();
    let file_path = dir.join(".cursorrules");

    // File only contains the managed block
    let initial = "<!-- BEGIN ENGRAM MANAGED BLOCK -->\n## Engram Architecture Context\n<!-- END ENGRAM MANAGED BLOCK -->\n";
    fs::write(&file_path, initial).unwrap();

    let report = instructions::strip_managed_block(&file_path, false).unwrap();
    assert_eq!(
        report.action,
        instructions::ManagedBlockAction::DeletedEmptyFile
    );
    // File must be deleted since it had no user content
    assert!(!file_path.exists());
}

#[test]
fn test_setup_remove_preserves_unrelated_mcp_servers() {
    let dir = temp_test_dir();
    let mcp_path = dir.join(".vscode").join("mcp.json");
    fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();

    let initial = r#"{
  "mcpServers": {
    "weather": { "command": "weather-cli", "args": [] },
    "engram": { "command": "engram.exe", "args": ["mcp"] }
  }
}"#;
    fs::write(&mcp_path, initial).unwrap();

    let removed = setup::remove_mcp_config(&mcp_path).unwrap();
    assert!(removed);

    let updated: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mcp_path).unwrap()).unwrap();
    let servers = updated["mcpServers"].as_object().unwrap();
    assert!(!servers.contains_key("engram"));
    assert!(servers.contains_key("weather"));
}

#[test]
fn test_setup_remove_dry_run_vs_apply() {
    let dir = temp_test_dir();
    let mcp_path = dir.join(".vscode").join("mcp.json");
    let instruction_path = dir.join(".github").join("copilot-instructions.md");
    fs::create_dir_all(mcp_path.parent().unwrap()).unwrap();
    fs::create_dir_all(instruction_path.parent().unwrap()).unwrap();

    fs::write(
        &mcp_path,
        r#"{"mcpServers":{"engram":{"command":"engram","args":["mcp"]}}}"#,
    )
    .unwrap();
    fs::write(
        &instruction_path,
        "<!-- BEGIN ENGRAM MANAGED BLOCK -->\nEngram\n<!-- END ENGRAM MANAGED BLOCK -->\n",
    )
    .unwrap();

    // 1. Dry run removal
    let dry_opts = SetupOptions {
        client: Some("vscode".to_string()),
        check: false,
        dry_run: true,
        apply: false,
        force: false,
        status: false,
        remove: true,
    };
    let dry_report = setup::orchestrate_setup(&dir, dry_opts).unwrap();
    assert_eq!(dry_report.mode, "DRY_RUN (REMOVAL)");
    assert_eq!(dry_report.changes_applied, 0);
    // Files must still exist untouched
    assert!(mcp_path.exists());
    assert!(instruction_path.exists());

    // 2. Apply removal
    let apply_opts = SetupOptions {
        client: Some("vscode".to_string()),
        check: false,
        dry_run: false,
        apply: true,
        force: false,
        status: false,
        remove: true,
    };
    let apply_report = setup::orchestrate_setup(&dir, apply_opts).unwrap();
    assert_eq!(apply_report.mode, "REMOVED");
    assert_eq!(apply_report.changes_applied, 2);

    let mcp_after: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&mcp_path).unwrap()).unwrap();
    assert!(
        !mcp_after["mcpServers"]
            .as_object()
            .unwrap()
            .contains_key("engram")
    );
    // Instruction file had only engram block, so it was cleanly deleted
    assert!(!instruction_path.exists());
}
