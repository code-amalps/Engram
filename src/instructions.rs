//! Safe, non-destructive instruction file manager using managed delimiter blocks.
//! Supports .github/copilot-instructions.md, .cursorrules, .windsurfrules, and CLAUDE.md.

use anyhow::{Result, anyhow};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

pub const START_MARKER: &str = "<!-- BEGIN ENGRAM MANAGED BLOCK -->";
pub const END_MARKER: &str = "<!-- END ENGRAM MANAGED BLOCK -->";

pub const DEFAULT_INSTRUCTION_BODY: &str = "\
## Engram Architecture Context
Before modifying code in this repository:
1. Consult `.engram/` or invoke `engram context \"<task>\"` for minimal AST symbol slices and boundary rules.
2. If using MCP tools, call `engram_analyze_task` and `engram_create_change_contract` before editing files.
3. Keep changes strictly within declared task boundaries.";

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ManagedBlockAction {
    Created,
    UpdatedInPlace,
    AppendedPreserved,
    Unchanged,
}

#[derive(Debug, Clone)]
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
pub fn sync_managed_block(
    path: &Path,
    body: &str,
    dry_run: bool,
) -> Result<ManagedBlockReport> {
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
        if !new_content.is_empty() && !new_content.ends_with(&format!("{}{}", line_ending, line_ending)) {
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
