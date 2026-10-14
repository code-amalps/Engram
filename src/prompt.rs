//! Structured, editor-agnostic prompt generator for non-MCP environments.
//! Provides minimal AST context, explicit change boundaries, and completeness telemetry.

use anyhow::Result;
use serde::Serialize;
use std::{
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use crate::compiler::context;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptFormat {
    Markdown,
    Text,
}

#[derive(Debug, Clone, Serialize)]
pub struct PromptOutput {
    pub task: String,
    pub content: String,
    pub estimated_tokens: usize,
    pub token_budget: usize,
    pub files_included: usize,
    pub files_omitted: usize,
    pub completeness_status: String,
    pub clipboard_copied: bool,
}

/// Generates structured prompt context for tools that cannot use Engram MCP.
pub fn generate_prompt(
    root: &Path,
    task: &str,
    budget: usize,
    format: PromptFormat,
    copy_clipboard: bool,
) -> Result<PromptOutput> {
    let ctx = context(root, task, budget)?;

    let mut body = String::new();

    match format {
        PromptFormat::Markdown => {
            body.push_str(&format!("# Task\n{}\n\n", task));

            body.push_str("## Relevant Context (AST Grounded)\n");
            if ctx.files.is_empty() {
                body.push_str("_No matching declarations or symbols found in repository index._\n\n");
            } else {
                for f in &ctx.files {
                    body.push_str(&format!("### {}\n```text\n{}\n```\n\n", f.path, f.content.trim()));
                }
            }

            body.push_str("## Change Boundaries & Constraints\n");
            if ctx.constraints.is_empty() {
                body.push_str("- No explicit architectural constraints declared in `.engram/constitution.yaml`.\n");
            } else {
                for c in &ctx.constraints {
                    body.push_str(&format!("- **Constraint:** {}\n", c));
                }
            }
            if let Some(eco) = &ctx.ecosystem_guard {
                body.push_str(&format!("- **Ecosystem Policy:** {}\n", eco));
            }
            body.push('\n');

            if !ctx.cochange_alerts.is_empty() {
                body.push_str("## Historical Co-Change Dependencies\n");
                for alert in &ctx.cochange_alerts {
                    body.push_str(&format!("- {}\n", alert));
                }
                body.push('\n');
            }

            body.push_str("## Context Completeness\n");
            body.push_str(&format!("- **Estimated Tokens:** ~{} (BM25 + AST estimation)\n", ctx.estimated_tokens));
            body.push_str(&format!("- **Token Budget:** {}\n", ctx.token_budget));
            body.push_str(&format!("- **Confidence:** {}\n", ctx.confidence));
            body.push_str(&format!("- **Included Files:** {}\n", ctx.files.len()));

            if !ctx.excluded_files.is_empty() {
                body.push_str(&format!("- **Omitted Files ({}) due to budget ceiling:**\n", ctx.excluded_files.len()));
                for (path, reason) in &ctx.excluded_files {
                    body.push_str(&format!("  - `{}`: {}\n", path, reason));
                }
            }
        }
        PromptFormat::Text => {
            body.push_str(&format!("=== TASK ===\n{}\n\n", task));

            body.push_str("=== RELEVANT CONTEXT (AST GROUNDED) ===\n");
            for f in &ctx.files {
                body.push_str(&format!("[FILE: {}]\n{}\n\n", f.path, f.content.trim()));
            }

            body.push_str("=== CHANGE BOUNDARIES & CONSTRAINTS ===\n");
            for c in &ctx.constraints {
                body.push_str(&format!("• CONSTRAINT: {}\n", c));
            }
            if let Some(eco) = &ctx.ecosystem_guard {
                body.push_str(&format!("• ECOSYSTEM POLICY: {}\n", eco));
            }
            body.push('\n');

            body.push_str("=== CONTEXT COMPLETENESS ===\n");
            body.push_str(&format!("• Estimated Tokens: ~{}\n", ctx.estimated_tokens));
            body.push_str(&format!("• Budget: {}\n", ctx.token_budget));
            body.push_str(&format!("• Confidence: {}\n", ctx.confidence));
            body.push_str(&format!("• Included Files: {}\n", ctx.files.len()));
            body.push_str(&format!("• Omitted Files: {}\n", ctx.excluded_files.len()));
        }
    }

    let completeness_status = if ctx.excluded_files.is_empty() {
        "SUFFICIENT_GROUNDING".to_string()
    } else {
        format!("PARTIAL_GROUNDING ({} files omitted due to budget)", ctx.excluded_files.len())
    };

    let mut clipboard_copied = false;
    if copy_clipboard {
        clipboard_copied = try_copy_to_clipboard(&body).is_ok();
    }

    Ok(PromptOutput {
        task: task.to_string(),
        content: body,
        estimated_tokens: ctx.estimated_tokens,
        token_budget: ctx.token_budget,
        files_included: ctx.files.len(),
        files_omitted: ctx.excluded_files.len(),
        completeness_status,
        clipboard_copied,
    })
}

/// Attempts native OS clipboard copy using built-in platform utilities.
pub fn try_copy_to_clipboard(text: &str) -> Result<()> {
    #[cfg(windows)]
    {
        // clip.exe is built into Windows
        let mut child = Command::new("clip")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        let status = child.wait()?;
        if status.success() {
            return Ok(());
        }
    }

    #[cfg(target_os = "macos")]
    {
        let mut child = Command::new("pbcopy")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        let status = child.wait()?;
        if status.success() {
            return Ok(());
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Try wl-copy (Wayland), then xclip (X11)
        if let Ok(mut child) = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return Ok(());
            }
        }

        if let Ok(mut child) = Command::new("xclip")
            .arg("-selection")
            .arg("clipboard")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes());
            }
            if child.wait().map(|s| s.success()).unwrap_or(false) {
                return Ok(());
            }
        }
    }

    anyhow::bail!("No supported clipboard utility found on system")
}
