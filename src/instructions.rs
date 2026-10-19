//! Safe, non-destructive instruction file manager using managed delimiter blocks.
//! Supports .github/copilot-instructions.md, .cursorrules, .windsurfrules, and CLAUDE.md.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

pub const START_MARKER: &str = "<!-- BEGIN ENGRAM MANAGED BLOCK -->";
pub const END_MARKER: &str = "<!-- END ENGRAM MANAGED BLOCK -->";

pub const DEFAULT_INSTRUCTION_BODY: &str = "\
## Engram Architecture Context & Task-Aware Intelligence
When working on tasks in this repository, follow these context-efficient principles:
- **Focused Context**: If an active Change Contract (`ctr_...`) or task context is already provided in the prompt or `.engram/active_task.md`, DO NOT call `engram_get_context` or `engram_preflight`. The contract is already established; proceed directly to implementation within authorized targets. Otherwise, call `engram_get_context` with your task description before editing for complex non-trivial tasks. Trivial leaf edits may bypass Engram when safe.
- **Dependency & Blast Radius**: When modifying shared services, state, or APIs, call `engram_get_impact` to evaluate reverse dependencies and avoid breaking callers.
- **One-Shot Preflight**: When no active contract is provided, call `engram_preflight` with your task and `planned_files` to validate boundaries and lock a Change Contract. Reuse an existing valid change contract where available; revalidate with preflight only when material plan changes or new files require re-authorization.
- **Minimal Blast Radius**: Confine edits strictly to authorized expected files and modules. Avoid unrelated refactors, unrequested UI components/dialogs, and bulk localization edits (e.g. editing multiple translation JSONs) unless explicitly requested.
- **Verification & Postflight**: Run targeted tests for modified components. After modifying code under a Change Contract, call `engram_postflight` with the contract ID to audit compliance and record candidate memories.
- **Design Rationale**: Call `engram_why` to retrieve rationale, governing decisions, and commit archaeology when investigating design decisions.
- **Integrity**: Never fabricate tool outputs, invent fake token counts, or claim a tool was invoked when it was not.";

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum ManagedBlockAction {
    Created,
    UpdatedInPlace,
    AppendedPreserved,
    Unchanged,
    RemovedPreserved,
    DeletedEmptyFile,
    NotFound,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManagedBlockReport {
    pub file: PathBuf,
    pub action: ManagedBlockAction,
    pub preserved_lines: usize,
    pub total_lines: usize,
}

/// Detects the predominant line ending in the content (\r\n vs \n).
pub fn detect_line_ending(content: &str) -> &'static str {
    if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Normalizes a block template to use the target line ending.
pub fn format_managed_block(body: &str, line_ending: &str) -> String {
    let normalized_body = body.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized_body.lines().collect();
    let joined = lines.join(line_ending);
    format!(
        "{}{}{}{}{}",
        START_MARKER, line_ending, joined, line_ending, END_MARKER
    )
}

/// Atomically replaces or creates a file via a sibling temporary file.
pub fn write_atomic_preserves(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let parent_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let temp_name = format!(
        ".tmp_{}_{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        std::process::id()
    );
    let temp_path = parent_dir.join(temp_name);

    {
        let mut f = File::create(&temp_path)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
    }

    #[cfg(windows)]
    {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        if let Err(e) = fs::rename(&temp_path, path) {
            let _ = fs::copy(&temp_path, path);
            let _ = fs::remove_file(&temp_path);
            if !path.exists() {
                return Err(anyhow!("Failed to replace {}: {}", path.display(), e));
            }
        }
    }

    #[cfg(not(windows))]
    {
        fs::rename(&temp_path, path)?;
    }

    Ok(())
}

/// Inspects and safely updates a file with a managed block.
///
/// Failure Semantics:
/// - Missing file: created with managed block (Action: Created).
/// - Existing file, no block: safely appended preserving all existing bytes (Action: AppendedPreserved).
/// - Existing file, exactly 1 valid block: replaced in-place, preserving surrounding content (Action: UpdatedInPlace or Unchanged).
/// - Malformed block (missing end marker): returns Err without modifying file.
/// - Duplicate blocks (multiple start or end markers): returns Err without modifying file.
pub fn sync_managed_block(path: &Path, body: &str, dry_run: bool) -> Result<ManagedBlockReport> {
    if !path.exists() {
        let line_ending = if cfg!(windows) { "\r\n" } else { "\n" };
        let block = format_managed_block(body, line_ending);
        let final_content = format!("{}{}", block, line_ending);
        let total_lines = final_content.lines().count();

        if !dry_run {
            write_atomic_preserves(path, &final_content)?;
        }

        return Ok(ManagedBlockReport {
            file: path.to_path_buf(),
            action: ManagedBlockAction::Created,
            preserved_lines: 0,
            total_lines,
        });
    }

    let raw = fs::read_to_string(path)?;
    let line_ending = detect_line_ending(&raw);
    let start_count = raw.matches(START_MARKER).count();
    let end_count = raw.matches(END_MARKER).count();

    // Malformed block validation
    if start_count != end_count {
        return Err(anyhow!(
            "Malformed Engram managed block in {}: found {} start marker(s) and {} end marker(s). Manual correction required before modifying.",
            path.display(),
            start_count,
            end_count
        ));
    }

    // Duplicate block validation
    if start_count > 1 {
        return Err(anyhow!(
            "Duplicate Engram managed blocks in {}: found {} blocks. Exactly one managed block is permitted.",
            path.display(),
            start_count
        ));
    }

    let managed_block = format_managed_block(body, line_ending);

    if start_count == 0 {
        // Safe append preserving 100% of existing content
        let mut new_content = raw.clone();
        if !new_content.is_empty() && !new_content.ends_with('\n') && !new_content.ends_with('\r') {
            new_content.push_str(line_ending);
        }
        if !new_content.is_empty()
            && !new_content.ends_with(&format!("{}{}", line_ending, line_ending))
        {
            new_content.push_str(line_ending);
        }
        new_content.push_str(&managed_block);
        new_content.push_str(line_ending);

        let preserved_lines = raw.lines().count();
        let total_lines = new_content.lines().count();

        if !dry_run {
            write_atomic_preserves(path, &new_content)?;
        }

        Ok(ManagedBlockReport {
            file: path.to_path_buf(),
            action: ManagedBlockAction::AppendedPreserved,
            preserved_lines,
            total_lines,
        })
    } else {
        // Exactly one valid block: replace in-place
        let start_pos = raw.find(START_MARKER).unwrap();
        let end_marker_pos = raw.find(END_MARKER).unwrap();
        let end_pos = end_marker_pos + END_MARKER.len();

        if start_pos > end_pos {
            return Err(anyhow!(
                "Corrupt Engram managed block order in {}: end marker appears before start marker.",
                path.display()
            ));
        }

        let before = &raw[..start_pos];
        let after = &raw[end_pos..];

        let mut updated = String::with_capacity(before.len() + managed_block.len() + after.len());
        updated.push_str(before);
        updated.push_str(&managed_block);
        updated.push_str(after);

        let existing_block = &raw[start_pos..end_pos];
        let is_unchanged = existing_block.trim() == managed_block.trim();

        let action = if is_unchanged {
            ManagedBlockAction::Unchanged
        } else {
            ManagedBlockAction::UpdatedInPlace
        };

        let preserved_lines = before.lines().count() + after.lines().count();
        let total_lines = updated.lines().count();

        if !dry_run && !is_unchanged {
            write_atomic_preserves(path, &updated)?;
        }

        Ok(ManagedBlockReport {
            file: path.to_path_buf(),
            action,
            preserved_lines,
            total_lines,
        })
    }
}

/// Strips the Engram managed block from an instruction file.
///
/// Behavior:
/// - If file does not exist or has no managed block: returns NotFound without error.
/// - If file only contained the managed block: removes the file (DeletedEmptyFile) if !dry_run.
/// - If file contains other user content: strips only the managed block, preserving all user lines (RemovedPreserved).
/// - If block is malformed or duplicated: returns Err to prevent data loss.
pub fn strip_managed_block(path: &Path, dry_run: bool) -> Result<ManagedBlockReport> {
    if !path.exists() {
        return Ok(ManagedBlockReport {
            file: path.to_path_buf(),
            action: ManagedBlockAction::NotFound,
            preserved_lines: 0,
            total_lines: 0,
        });
    }

    let raw = fs::read_to_string(path)?;
    let start_count = raw.matches(START_MARKER).count();
    let end_count = raw.matches(END_MARKER).count();

    if start_count == 0 && end_count == 0 {
        let total_lines = raw.lines().count();
        return Ok(ManagedBlockReport {
            file: path.to_path_buf(),
            action: ManagedBlockAction::NotFound,
            preserved_lines: total_lines,
            total_lines,
        });
    }

    if start_count != end_count || start_count > 1 {
        return Err(anyhow!(
            "Cannot safely remove Engram managed block in {}: block is malformed or duplicated. Manual inspection required.",
            path.display()
        ));
    }

    let start_pos = raw.find(START_MARKER).unwrap();
    let end_marker_pos = raw.find(END_MARKER).unwrap();
    let end_pos = end_marker_pos + END_MARKER.len();

    let before = &raw[..start_pos];
    let after = &raw[end_pos..];

    // Check if remaining user content is essentially empty
    let before_trimmed = before.trim();
    let after_trimmed = after.trim();

    if before_trimmed.is_empty() && after_trimmed.is_empty() {
        if !dry_run {
            let _ = fs::remove_file(path);
        }
        return Ok(ManagedBlockReport {
            file: path.to_path_buf(),
            action: ManagedBlockAction::DeletedEmptyFile,
            preserved_lines: 0,
            total_lines: 0,
        });
    }

    let line_ending = detect_line_ending(&raw);
    let mut cleaned = String::with_capacity(before.len() + after.len());
    if !before_trimmed.is_empty() {
        cleaned.push_str(before.trim_end());
    }
    if !after_trimmed.is_empty() {
        if !cleaned.is_empty() {
            cleaned.push_str(line_ending);
            cleaned.push_str(line_ending);
        }
        cleaned.push_str(after.trim_start());
    }
    if !cleaned.is_empty() && !cleaned.ends_with('\n') {
        cleaned.push_str(line_ending);
    }

    let total_lines = cleaned.lines().count();
    if !dry_run {
        write_atomic_preserves(path, &cleaned)?;
    }

    Ok(ManagedBlockReport {
        file: path.to_path_buf(),
        action: ManagedBlockAction::RemovedPreserved,
        preserved_lines: total_lines,
        total_lines,
    })
}

/// Known workspace instruction file locations relative to workspace root and their client IDs.
pub const KNOWN_INSTRUCTION_FILES: &[(&str, &str)] = &[
    ("vscode", ".github/copilot-instructions.md"),
    ("cursor", ".cursorrules"),
    ("windsurf", ".windsurfrules"),
    ("claude", "CLAUDE.md"),
];

/// Formats a Turn-0 task-aware instruction body from a prepared task output.
/// Injects active contract directives, boundaries, AST context, and agent instructions.
pub fn format_task_instruction_body(prep: &crate::prompt::PreparedTaskOutput) -> String {
    let mut body = String::new();
    body.push_str("## Engram Turn-0 Task Intelligence & Directives\n");

    if prep.bypass_active {
        body.push_str("### ⚡ Adaptive Small-Task Bypass Active\n");
        body.push_str(&format!("- **Task:** {}\n", prep.task));
        if let Some(ref sig) = prep.classification_signals {
            body.push_str(&format!("- **Classification:** TRIVIAL ({})\n", sig.reason));
        } else {
            body.push_str("- **Classification:** TRIVIAL\n");
        }
        body.push_str("- **Directive:** This task is classified as a safe, single-file edit. **DO NOT call `engram_get_context` or `engram_preflight`.** Proceed directly to editing the target file using standard editor tools.\n");
        body.push_str("- **Postflight:** Run project tests to verify correctness. Postflight compliance auditing (`engram_postflight`) is optional.\n");
        return body;
    }

    body.push_str(&format!("- **Task:** {}\n", prep.task));
    body.push_str(&format!(
        "- **Active Change Contract:** `{}` (v{}, Status: {})\n",
        prep.contract_id, prep.contract_version, prep.status
    ));
    body.push_str(&format!(
        "- **Baseline Revision:** `{}`\n",
        prep.baseline_commit
    ));
    if !prep.expected_files.is_empty() {
        body.push_str(&format!(
            "- **Authorized Target Files:** {}\n",
            prep.expected_files.join(", ")
        ));
    }
    if !prep.forbidden_boundaries.is_empty() {
        body.push_str(&format!(
            "- **Forbidden Boundaries:** {}\n",
            prep.forbidden_boundaries.join(", ")
        ));
    }

    body.push_str("\n### 🚨 Mandatory AI Agent Directives\n");
    body.push_str(&format!(
        "1. **ZERO-TURN EXECUTION:** Change Contract `{}` is ALREADY LOCKED to baseline revision `{}`.\n   **DO NOT CALL `engram_get_context` OR `engram_preflight`.** All required context and boundaries are provided below.\n",
        prep.contract_id, prep.baseline_commit
    ));
    body.push_str("2. **BOUNDARY COMPLIANCE:** Confine code edits strictly to authorized target files. Do not modify files in forbidden boundaries or make unrequested cosmetic refactors.\n");
    body.push_str(&format!(
        "3. **POSTFLIGHT AUDIT:** When code edits and unit tests are complete, call `engram_postflight` with `contract_id: \"{}\"` to audit compliance.\n\n",
        prep.contract_id
    ));

    // Append the grounded context and constraints from prep.content
    if let Some(idx) = prep.content.find("## Relevant Context") {
        body.push_str(&prep.content[idx..]);
    } else {
        body.push_str(&prep.content);
    }

    body
}

/// Resolves target instruction file paths based on explicit options or workspace discovery.
pub fn resolve_instruction_targets(
    root: &Path,
    target: Option<&Path>,
    client: Option<&str>,
) -> Vec<PathBuf> {
    if let Some(t) = target {
        let p = if t.is_absolute() {
            t.to_path_buf()
        } else {
            root.join(t)
        };
        return vec![p];
    }

    if let Some(c) = client {
        let c_lower = c.to_lowercase();
        if c_lower == "all" {
            return KNOWN_INSTRUCTION_FILES
                .iter()
                .map(|(_, rel)| root.join(rel))
                .collect();
        }
        for (id, rel) in KNOWN_INSTRUCTION_FILES {
            if c_lower == *id || (c_lower == "copilot" && *id == "vscode") {
                return vec![root.join(rel)];
            }
        }
    }

    // Auto-detect existing instruction files in the workspace
    let mut detected = Vec::new();
    for (_, rel) in KNOWN_INSTRUCTION_FILES {
        let p = root.join(rel);
        if p.exists() {
            detected.push(p);
        }
    }

    if !detected.is_empty() {
        return detected;
    }

    // Default fallback: .github/copilot-instructions.md
    vec![root.join(".github").join("copilot-instructions.md")]
}

/// Injects task-aware Turn-0 context and contract directives into resolved workspace instruction files.
pub fn inject_task_instructions(
    root: &Path,
    prep: &crate::prompt::PreparedTaskOutput,
    target: Option<&Path>,
    client: Option<&str>,
    dry_run: bool,
) -> Result<Vec<ManagedBlockReport>> {
    let targets = resolve_instruction_targets(root, target, client);
    let body = format_task_instruction_body(prep);
    let mut reports = Vec::new();

    for file in targets {
        let report = sync_managed_block(&file, &body, dry_run)?;
        reports.push(report);
    }

    Ok(reports)
}

/// Reverts task-specific instruction blocks back to the default Engram architectural principles.
pub fn clear_task_instructions(
    root: &Path,
    target: Option<&Path>,
    client: Option<&str>,
    dry_run: bool,
) -> Result<Vec<ManagedBlockReport>> {
    let targets = resolve_instruction_targets(root, target, client);
    let mut reports = Vec::new();

    for file in targets {
        let report = sync_managed_block(&file, DEFAULT_INSTRUCTION_BODY, dry_run)?;
        reports.push(report);
    }

    Ok(reports)
}

/// Strips the Engram managed block entirely from resolved workspace instruction files.
pub fn strip_task_instructions(
    root: &Path,
    target: Option<&Path>,
    client: Option<&str>,
    dry_run: bool,
) -> Result<Vec<ManagedBlockReport>> {
    let targets = resolve_instruction_targets(root, target, client);
    let mut reports = Vec::new();

    for file in targets {
        let report = strip_managed_block(&file, dry_run)?;
        reports.push(report);
    }

    Ok(reports)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_instruction_targets_precedence() {
        let temp_dir =
            std::env::temp_dir().join(format!("engram_inst_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // 1. Explicit target wins
        let explicit = temp_dir.join("my-custom-rules.md");
        let res = resolve_instruction_targets(&temp_dir, Some(&explicit), None);
        assert_eq!(res, vec![explicit]);

        // 2. Client flag wins when no target
        let res = resolve_instruction_targets(&temp_dir, None, Some("cursor"));
        assert_eq!(res, vec![temp_dir.join(".cursorrules")]);

        let res = resolve_instruction_targets(&temp_dir, None, Some("vscode"));
        assert_eq!(
            res,
            vec![temp_dir.join(".github").join("copilot-instructions.md")]
        );

        // 3. Fallback when none exist
        let res = resolve_instruction_targets(&temp_dir, None, None);
        assert_eq!(
            res,
            vec![temp_dir.join(".github").join("copilot-instructions.md")]
        );

        // 4. Auto-detect when instruction file exists
        let claude_file = temp_dir.join("CLAUDE.md");
        fs::write(&claude_file, "# Claude instructions\n").unwrap();
        let res = resolve_instruction_targets(&temp_dir, None, None);
        assert_eq!(res, vec![claude_file]);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_inject_and_clear_task_instructions_lifecycle() {
        let temp_dir =
            std::env::temp_dir().join(format!("engram_inst_cycle_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        let inst_file = temp_dir.join(".github").join("copilot-instructions.md");
        fs::create_dir_all(inst_file.parent().unwrap()).unwrap();
        fs::write(
            &inst_file,
            "# Existing User Section\nAlways follow style guide.\n",
        )
        .unwrap();

        let dummy_prep = crate::prompt::PreparedTaskOutput {
            task: "Add remember-me checkbox to login page".to_string(),
            contract_id: "ctr_test_12345678".to_string(),
            contract_version: 1,
            baseline_commit: "abc1234".to_string(),
            status: "Locked".to_string(),
            expected_files: vec!["src/login.tsx".to_string(), "src/auth.ts".to_string()],
            forbidden_boundaries: vec!["src/admin/**".to_string()],
            content: "## Relevant Context (AST Grounded)\n### src/login.tsx\n```text\nexport function Login() {}\n```\n".to_string(),
            estimated_tokens: 450,
            token_budget: 1200,
            files_included: 2,
            files_omitted: 0,
            completeness_status: "SUFFICIENT_GROUNDING".to_string(),
            clipboard_copied: false,
            active_file_path: None,
            bypass_active: false,
            classification_signals: None,
            injected_files: vec![],
        };

        // 1. Dry run injection: does not modify file
        let dry_reports =
            inject_task_instructions(&temp_dir, &dummy_prep, None, None, true).unwrap();
        assert_eq!(dry_reports.len(), 1);
        let content_after_dry = fs::read_to_string(&inst_file).unwrap();
        assert!(!content_after_dry.contains("ctr_test_12345678"));

        // 2. Real injection: modifies file and preserves existing user section
        let real_reports =
            inject_task_instructions(&temp_dir, &dummy_prep, None, None, false).unwrap();
        assert_eq!(real_reports.len(), 1);
        let content_after_inject = fs::read_to_string(&inst_file).unwrap();
        assert!(content_after_inject.contains("# Existing User Section"));
        assert!(content_after_inject.contains("Always follow style guide."));
        assert!(content_after_inject.contains(START_MARKER));
        assert!(content_after_inject.contains("ctr_test_12345678"));
        assert!(content_after_inject.contains("ZERO-TURN EXECUTION"));
        assert!(content_after_inject.contains("src/login.tsx"));
        assert!(content_after_inject.contains(END_MARKER));

        // 3. Clear task instructions: reverts to DEFAULT_INSTRUCTION_BODY, still preserving user section
        let clear_reports = clear_task_instructions(&temp_dir, None, None, false).unwrap();
        assert_eq!(clear_reports.len(), 1);
        let content_after_clear = fs::read_to_string(&inst_file).unwrap();
        assert!(content_after_clear.contains("# Existing User Section"));
        assert!(!content_after_clear.contains("ctr_test_12345678"));
        assert!(content_after_clear.contains(START_MARKER));
        assert!(
            content_after_clear.contains("Engram Architecture Context & Task-Aware Intelligence")
        );
        assert!(content_after_clear.contains(END_MARKER));

        // 4. Strip task instructions: removes the managed block completely
        let strip_reports = strip_task_instructions(&temp_dir, None, None, false).unwrap();
        assert_eq!(strip_reports.len(), 1);
        let content_after_strip = fs::read_to_string(&inst_file).unwrap();
        assert!(content_after_strip.contains("# Existing User Section"));
        assert!(!content_after_strip.contains(START_MARKER));
        assert!(!content_after_strip.contains(END_MARKER));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
