//! Structured, editor-agnostic prompt generator for non-MCP environments.
//! Provides minimal AST context, explicit change boundaries, and completeness telemetry.
//! Includes deterministic task-complexity classification for adaptive bypass (Iteration 2A).

use anyhow::Result;
use serde::Serialize;
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

use crate::compiler::context;
use crate::flywheel::{parse_task_and_planned_files, preflight_task, PreflightOptions};
use crate::storage::root_path;

// ---------------------------------------------------------------------------
// Iteration 2A: Deterministic task-complexity classifier
// ---------------------------------------------------------------------------

/// Task complexity classification result. No LLM calls are used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum TaskComplexity {
    /// Single-file, clearly bounded, non-security, non-architectural change.
    /// Engram preparation and contract workflow can be safely bypassed.
    Trivial,
    /// Multi-file, security-sensitive, architecturally significant, or
    /// insufficiently understood scope. Full Engram workflow is required.
    Standard,
}

/// Detailed signals that contributed to the complexity classification.
#[derive(Debug, Clone, Serialize)]
pub struct ClassificationSignals {
    /// Number of distinct file paths detected in the task description.
    pub file_count: usize,
    /// Whether security-sensitive keywords were detected.
    pub has_security_keywords: bool,
    /// Whether architecture/structural keywords were detected.
    pub has_architecture_keywords: bool,
    /// Whether multi-file coordination keywords were detected.
    pub has_multi_file_keywords: bool,
    /// Whether the task mentions tests, test creation, or test modification.
    pub has_test_keywords: bool,
    /// Whether ambiguity signals were detected (vague scope, open-ended language).
    pub has_ambiguity_signals: bool,
    /// Whether the task explicitly names exactly one target file.
    pub single_explicit_file: bool,
    /// Whether manifest or dependency file changes were detected.
    pub has_manifest_keywords: bool,
    /// Whether trivial-action keywords were detected (typo, rename, fix casing, etc.).
    pub has_trivial_action_keywords: bool,
    /// The final classification.
    pub classification: TaskComplexity,
    /// Human-readable reason for the classification.
    pub reason: String,
}

/// Security-sensitive keywords and variants that force Standard classification.
const SECURITY_KEYWORDS: &[&str] = &[
    "auth",
    "authentication",
    "authenticate",
    "authenticated",
    "authorization",
    "authorize",
    "authorized",
    "login",
    "log in",
    "logout",
    "log out",
    "signin",
    "sign in",
    "signout",
    "sign out",
    "password",
    "passwords",
    "credential",
    "credentials",
    "secret",
    "secrets",
    "token",
    "tokens",
    "session",
    "sessions",
    "cookie",
    "cookies",
    "jwt",
    "jwts",
    "oauth",
    "oauth2",
    "remember-me",
    "remember_me",
    "remember me",
    "encryption",
    "encrypt",
    "encrypted",
    "encrypting",
    "decryption",
    "decrypt",
    "decrypted",
    "certificate",
    "certificates",
    "cert",
    "certs",
    "ssl",
    "tls",
    "permission",
    "permissions",
    "role",
    "roles",
    "rbac",
    "acl",
    "acls",
    "sanitize",
    "sanitization",
    "sanitizing",
    "sanitized",
    "xss",
    "csrf",
    "injection",
    "sqli",
    "vulnerability",
    "vulnerabilities",
    "api key",
    "api_key",
    "apikey",
    "private key",
    "private_key",
];

/// Architecture/structural keywords that force Standard classification.
const ARCHITECTURE_KEYWORDS: &[&str] = &[
    "refactor",
    "refactoring",
    "refactored",
    "restructure",
    "restructuring",
    "restructured",
    "redesign",
    "redesigning",
    "redesigned",
    "migrate",
    "migration",
    "migrations",
    "migrating",
    "architecture",
    "architectural",
    "middleware",
    "middlewares",
    "interceptor",
    "interceptors",
    "schema",
    "schemas",
    "database",
    "databases",
    "db",
    "state management",
    "state-management",
    "inheritance",
    "polymorphism",
    "breaking change",
    "breaking changes",
    "rearchitect",
    "re-architect",
];

/// Multi-file coordination keywords that force Standard classification.
const MULTI_FILE_KEYWORDS: &[&str] = &[
    "across",
    "all files",
    "every file",
    "throughout",
    "multiple files",
    "several files",
    "entire",
    "globally",
    "everywhere",
    "bulk",
    "batch",
    "all usages",
    "all usage",
    "all references",
    "all occurrences",
    "all instances",
    "project-wide",
    "repo-wide",
    "workspace-wide",
    "codebase-wide",
];

/// Test-related keywords that force Standard classification.
const TEST_KEYWORDS: &[&str] = &[
    "add test",
    "write test",
    "create test",
    "update test",
    "modify test",
    "delete test",
    "remove test",
    "fix test",
    "unit test",
    "integration test",
    "e2e test",
    "spec",
    "specs",
    "test coverage",
    "test case",
    "test cases",
    "test suite",
    "test suites",
];

/// Manifest and dependency files that must not be bypassed.
const MANIFEST_KEYWORDS: &[&str] = &[
    "package.json",
    "package-lock.json",
    "cargo.toml",
    "cargo.lock",
    "go.mod",
    "go.sum",
    "pom.xml",
    "build.gradle",
    "requirements.txt",
    "gemfile",
    "dockerfile",
    "docker-compose",
    "kubernetes",
    "k8s",
    "workflow",
    "pipeline",
    "ci/cd",
    "dependency",
    "dependencies",
];

/// Ambiguity signals that force Standard classification.
const AMBIGUITY_KEYWORDS: &[&str] = &[
    "improve",
    "optimize",
    "optimization",
    "enhance",
    "enhancement",
    "clean up",
    "cleanup",
    "tidy",
    "best practice",
    "best practices",
    "modernize",
    "modernization",
    "update all",
    "fix all",
    "change all",
    "as needed",
    "where appropriate",
    "if necessary",
    "when necessary",
    "and related",
    "and dependencies",
    "and its",
    "and other",
    "wherever",
    "anything",
    "everything",
    "somewhere",
];

/// Trivial-action keywords that contribute toward Trivial classification.
const TRIVIAL_ACTION_KEYWORDS: &[&str] = &[
    "typo",
    "casing",
    "capitalization",
    "capitalize",
    "capitalise",
    "rename",
    "spelling",
    "misspelling",
    "whitespace",
    "indent",
    "comment",
    "uncomment",
    "version bump",
    "bump version",
    "change string",
    "update string",
    "fix string",
    "change value",
    "update value",
    "set value",
    "change name",
    "update name",
    "correct name",
    "branding",
    "label",
    "placeholder",
    "tooltip",
    "change text",
    "update text",
    "fix text",
];

/// Normalizes text into a canonical space-padded string of lowercase alphanumeric words.
fn normalize_for_matching(text: &str) -> String {
    let words: Vec<&str> = text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    if words.is_empty() {
        " ".to_string()
    } else {
        format!(" {} ", words.join(" ").to_ascii_lowercase())
    }
}

/// Checks if a keyword or phrase appears as distinct words in the normalized text.
fn contains_keyword_phrase(normalized_text: &str, kw: &str) -> bool {
    let norm_kw = normalize_for_matching(kw);
    normalized_text.contains(&norm_kw)
}

/// Checks if a path targets a test or specification file.
fn is_test_file_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains(".spec.")
        || lower.contains(".test.")
        || lower.contains("_test.")
        || lower.contains("test_")
        || lower.starts_with("test/")
        || lower.starts_with("tests/")
        || lower.contains("/test/")
        || lower.contains("/tests/")
        || lower.contains("\\test\\")
        || lower.contains("\\tests\\")
}

/// Checks if a path targets a security, crypto, or credential file.
fn is_security_file_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.contains("/auth/")
        || lower.contains("\\auth\\")
        || lower.contains("/crypto/")
        || lower.contains("\\crypto\\")
        || lower.contains("/secrets/")
        || lower.contains("\\secrets\\")
        || lower.contains("secret")
        || lower.contains("password")
        || lower.contains("token")
        || lower.contains("credential")
        || lower.contains("cert")
        || lower.contains("key")
}

/// Checks if a path targets a manifest, lockfile, or deployment configuration.
fn is_manifest_file_path(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with("package.json")
        || lower.ends_with("package-lock.json")
        || lower.ends_with("cargo.toml")
        || lower.ends_with("cargo.lock")
        || lower.ends_with("go.mod")
        || lower.ends_with("go.sum")
        || lower.ends_with("pom.xml")
        || lower.ends_with("build.gradle")
        || lower.ends_with("requirements.txt")
        || lower.ends_with("gemfile")
        || lower.ends_with("dockerfile")
}

/// Classifies task complexity deterministically from the task description.
///
/// Classification rules (applied in order; first match wins):
/// 1. If ≥2 distinct file paths are detected → Standard.
/// 2. If 0 target files are detected → Standard (scope not established).
/// 3. If any security keyword or security path is found → Standard.
/// 4. If any architecture keyword is found → Standard.
/// 5. If any multi-file coordination keyword is found → Standard.
/// 6. If any test keyword or test file path is found → Standard.
/// 7. If any manifest/dependency keyword or file path is found → Standard.
/// 8. If any ambiguity signal is found → Standard.
/// 9. If exactly 1 clean file path is detected AND at least one trivial-action keyword → Trivial.
/// 10. Otherwise → Standard (safe fallback).
pub fn classify_task_complexity(task: &str) -> ClassificationSignals {
    let normalized = normalize_for_matching(task);

    // Parse file paths from the task description
    let (_, planned_files) = parse_task_and_planned_files(task, None);
    let mut detected_files = planned_files;

    // Collect inline file references
    for inline in collect_inline_file_references(task) {
        if !detected_files.contains(&inline) {
            detected_files.push(inline);
        }
    }
    let effective_file_count = detected_files.len();

    let mut has_security_keywords = SECURITY_KEYWORDS
        .iter()
        .any(|kw| contains_keyword_phrase(&normalized, kw));
    let has_architecture_keywords = ARCHITECTURE_KEYWORDS
        .iter()
        .any(|kw| contains_keyword_phrase(&normalized, kw));
    let has_multi_file_keywords = MULTI_FILE_KEYWORDS
        .iter()
        .any(|kw| contains_keyword_phrase(&normalized, kw));
    let mut has_test_keywords = TEST_KEYWORDS
        .iter()
        .any(|kw| contains_keyword_phrase(&normalized, kw));
    let mut has_manifest_keywords = MANIFEST_KEYWORDS
        .iter()
        .any(|kw| contains_keyword_phrase(&normalized, kw));
    let has_ambiguity_signals = AMBIGUITY_KEYWORDS
        .iter()
        .any(|kw| contains_keyword_phrase(&normalized, kw));
    let has_trivial_action_keywords = TRIVIAL_ACTION_KEYWORDS
        .iter()
        .any(|kw| contains_keyword_phrase(&normalized, kw));

    // Inspect file path sensitivities for the single target file if present
    if effective_file_count == 1 {
        let target = &detected_files[0];
        if is_security_file_path(target) {
            has_security_keywords = true;
        }
        if is_test_file_path(target) {
            has_test_keywords = true;
        }
        if is_manifest_file_path(target) {
            has_manifest_keywords = true;
        }
    }

    let single_explicit_file = effective_file_count == 1;

    // Classification logic — deterministic, zero-cost, local-first
    let (classification, reason) = if effective_file_count >= 2 {
        (TaskComplexity::Standard, format!(
            "Multiple files detected ({} file references: {}); requires full context and contract workflow.",
            effective_file_count, detected_files.join(", ")
        ))
    } else if effective_file_count == 0 {
        (TaskComplexity::Standard,
         "No target files specified; scope cannot be established safely. Defaulting to full workflow.".to_string())
    } else if has_security_keywords {
        (
            TaskComplexity::Standard,
            "Security-sensitive keywords or security path detected; full safety workflow required."
                .to_string(),
        )
    } else if has_architecture_keywords {
        (
            TaskComplexity::Standard,
            "Architecture/structural keywords detected; full context and impact analysis required."
                .to_string(),
        )
    } else if has_multi_file_keywords {
        (
            TaskComplexity::Standard,
            "Multi-file coordination keywords detected; full context workflow required."
                .to_string(),
        )
    } else if has_test_keywords {
        (
            TaskComplexity::Standard,
            "Test creation, modification, or test file detected; full context workflow required."
                .to_string(),
        )
    } else if has_manifest_keywords {
        (TaskComplexity::Standard,
         "Dependency, manifest, or deployment configuration change detected; full workflow required.".to_string())
    } else if has_ambiguity_signals {
        (
            TaskComplexity::Standard,
            "Ambiguous or open-ended scope detected; defaulting to full workflow for safety."
                .to_string(),
        )
    } else if single_explicit_file && has_trivial_action_keywords {
        (TaskComplexity::Trivial,
         format!("Single file ('{}') with trivial action keyword and clean boundaries; eligible for bypass.", detected_files[0]))
    } else {
        (TaskComplexity::Standard,
         "Insufficient trivial signals or unrecognized task pattern; defaulting to full workflow.".to_string())
    };

    ClassificationSignals {
        file_count: effective_file_count,
        has_security_keywords,
        has_architecture_keywords,
        has_multi_file_keywords,
        has_test_keywords,
        has_ambiguity_signals,
        has_manifest_keywords,
        single_explicit_file,
        has_trivial_action_keywords,
        classification,
        reason,
    }
}

/// Collects distinct file-like references from the task text.
/// Looks for patterns like `path/to/file.ext` or standalone files like `README.md` that weren't captured by planned-file parsing.
fn collect_inline_file_references(raw_task: &str) -> Vec<String> {
    let mut files = Vec::new();
    // Split on whitespace, commas, backticks, and quotes
    for token in
        raw_task.split(|c: char| c.is_whitespace() || c == ',' || c == '`' || c == '\'' || c == '"')
    {
        let token = token.trim_matches(|c: char| {
            c == '.' || c == ';' || c == ':' || c == ')' || c == '(' || c == ']'
        });
        if token.is_empty() {
            continue;
        }
        // Must contain a file extension
        let has_extension = token.rfind('.').is_some_and(|dot_pos| {
            let ext = &token[dot_pos + 1..];
            !ext.is_empty() && ext.len() <= 10 && ext.chars().all(|c| c.is_ascii_alphanumeric())
        });
        let has_path_sep = token.contains('/') || token.contains('\\');
        // Recognize paths with separators, or known code/doc files in repo root
        let is_root_file = token.ends_with(".md")
            || token.ends_with(".json")
            || token.ends_with(".ts")
            || token.ends_with(".rs")
            || token.ends_with(".py")
            || token.ends_with(".toml")
            || token.ends_with(".yaml")
            || token.ends_with(".yml")
            || token.ends_with(".txt")
            || token.ends_with(".html")
            || token.ends_with(".css");

        if has_extension && (has_path_sep || is_root_file) {
            let norm = crate::contract::normalize_rel_path(token);
            if !files.contains(&norm) {
                files.push(norm);
            }
        }
    }
    files
}

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
                body.push_str(
                    "_No matching declarations or symbols found in repository index._\n\n",
                );
            } else {
                for f in &ctx.files {
                    body.push_str(&format!(
                        "### {}\n```text\n{}\n```\n\n",
                        f.path,
                        f.content.trim()
                    ));
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
            body.push_str(&format!(
                "- **Estimated Tokens:** ~{} (BM25 + AST estimation)\n",
                ctx.estimated_tokens
            ));
            body.push_str(&format!("- **Token Budget:** {}\n", ctx.token_budget));
            body.push_str(&format!("- **Confidence:** {}\n", ctx.confidence));
            body.push_str(&format!("- **Included Files:** {}\n", ctx.files.len()));

            if !ctx.excluded_files.is_empty() {
                body.push_str(&format!(
                    "- **Omitted Files ({}) due to budget ceiling:**\n",
                    ctx.excluded_files.len()
                ));
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
        format!(
            "PARTIAL_GROUNDING ({} files omitted due to budget)",
            ctx.excluded_files.len()
        )
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

#[derive(Debug, Clone, Serialize)]
pub struct PreparedTaskOutput {
    pub task: String,
    pub contract_id: String,
    pub contract_version: u32,
    pub baseline_commit: String,
    pub status: String,
    pub expected_files: Vec<String>,
    pub forbidden_boundaries: Vec<String>,
    pub content: String,
    pub estimated_tokens: usize,
    pub token_budget: usize,
    pub files_included: usize,
    pub files_omitted: usize,
    pub completeness_status: String,
    pub clipboard_copied: bool,
    pub active_file_path: Option<String>,
    /// Iteration 2A: Whether the task was classified as trivial and preparation was bypassed.
    pub bypass_active: bool,
    /// Iteration 2A: Classification signals explaining the bypass/non-bypass decision.
    pub classification_signals: Option<ClassificationSignals>,
    /// Turn-0 injection: Paths of workspace instruction files injected with this task's directives.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub injected_files: Vec<String>,
}

/// Prepares task-aware context, establishes/reuses a Change Contract locked to the baseline commit,
/// and formats a Turn-0 prompt packet so coding agents do not require interactive get_context or preflight tool turns.
pub fn prepare_task_context(
    root: &Path,
    task: &str,
    budget: usize,
    format: PromptFormat,
    copy_clipboard: bool,
    write_active_file: bool,
) -> Result<PreparedTaskOutput> {
    let root = root_path(root)?;
    let (core_task, mut planned_files) = parse_task_and_planned_files(task, None);

    // 1. Compile AST grounded context within token budget
    let ctx = context(&root, &core_task, budget)?;

    // 2. If no planned files specified explicitly, seed from top ranked context files
    if planned_files.is_empty() {
        for f in ctx.files.iter().take(5) {
            if !planned_files.contains(&f.path) {
                planned_files.push(f.path.clone());
            }
        }
    }

    // 3. Establish or reuse active Change Contract locked to baseline commit
    let preflight_opts = PreflightOptions {
        task: core_task.clone(),
        token_budget: budget,
        forbidden_boundaries: None,
        planned_files: if planned_files.is_empty() {
            None
        } else {
            Some(planned_files.clone())
        },
    };
    let preflight = preflight_task(&root, &preflight_opts)?;

    // 4. Construct Turn-0 structured prompt
    let mut body = String::new();

    match format {
        PromptFormat::Markdown => {
            body.push_str(&format!("# Task\n{}\n\n", core_task));

            body.push_str("## Engram Active Contract & Directives\n");
            body.push_str(&format!(
                "- **Contract ID:** `{}` (v{}, Status: {:?})\n",
                preflight.contract_id, preflight.contract_version, preflight.status
            ));
            body.push_str(&format!(
                "- **Baseline Revision:** `{}`\n",
                preflight.baseline_commit
            ));
            if !preflight.expected_files.is_empty() {
                body.push_str(&format!(
                    "- **Authorized Targets:** {}\n",
                    preflight.expected_files.join(", ")
                ));
            }
            if !preflight.forbidden_boundaries.is_empty() {
                body.push_str(&format!(
                    "- **Forbidden Boundaries:** {}\n",
                    preflight.forbidden_boundaries.join(", ")
                ));
            }
            body.push_str("- **Directives for AI Agent:**\n");
            body.push_str(&format!(
                "  1. Change Contract `{}` is already locked to the baseline commit. Do NOT call `engram_get_context` or `engram_preflight`.\n",
                preflight.contract_id
            ));
            body.push_str(
                "  2. Proceed directly to editing authorized files within the target boundary.\n",
            );
            body.push_str(&format!(
                "  3. When edits are complete, call `engram_postflight` with `contract_id: \"{}\"` to audit compliance.\n\n",
                preflight.contract_id
            ));

            body.push_str("## Relevant Context (AST Grounded)\n");
            if ctx.files.is_empty() {
                body.push_str(
                    "_No matching declarations or symbols found in repository index._\n\n",
                );
            } else {
                for f in &ctx.files {
                    body.push_str(&format!(
                        "### {}\n```text\n{}\n```\n\n",
                        f.path,
                        f.content.trim()
                    ));
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
            body.push_str(&format!(
                "- **Estimated Tokens:** ~{} (BM25 + AST estimation)\n",
                ctx.estimated_tokens
            ));
            body.push_str(&format!("- **Token Budget:** {}\n", ctx.token_budget));
            body.push_str(&format!("- **Included Files:** {}\n", ctx.files.len()));

            if !ctx.excluded_files.is_empty() {
                body.push_str(&format!(
                    "- **Omitted Files ({}) due to budget ceiling:**\n",
                    ctx.excluded_files.len()
                ));
                for (path, reason) in &ctx.excluded_files {
                    body.push_str(&format!("  - `{}`: {}\n", path, reason));
                }
            }
        }
        PromptFormat::Text => {
            body.push_str(&format!("=== TASK ===\n{}\n\n", core_task));
            body.push_str(&format!(
                "=== ENGRAM ACTIVE CONTRACT ===\nContract ID: {}\nBaseline: {}\nDirectives: Contract locked. Do NOT call get_context or preflight. Edit authorized files directly.\n\n",
                preflight.contract_id, preflight.baseline_commit
            ));

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
        }
    }

    let completeness_status = if ctx.excluded_files.is_empty() {
        "SUFFICIENT_GROUNDING".to_string()
    } else {
        format!(
            "PARTIAL_GROUNDING ({} files omitted due to budget)",
            ctx.excluded_files.len()
        )
    };

    let mut active_file_path = None;
    if write_active_file {
        let engram_dir = root.join(".engram");
        if engram_dir.exists() {
            let active_file = engram_dir.join("active_task.md");
            if fs::write(&active_file, &body).is_ok() {
                active_file_path = Some(active_file.to_string_lossy().to_string());
            }
        }
    }

    let mut clipboard_copied = false;
    if copy_clipboard {
        clipboard_copied = try_copy_to_clipboard(&body).is_ok();
    }

    Ok(PreparedTaskOutput {
        task: core_task,
        contract_id: preflight.contract_id,
        contract_version: preflight.contract_version,
        baseline_commit: preflight.baseline_commit,
        status: format!("{:?}", preflight.status),
        expected_files: preflight.expected_files,
        forbidden_boundaries: preflight.forbidden_boundaries,
        content: body,
        estimated_tokens: ctx.estimated_tokens,
        token_budget: ctx.token_budget,
        files_included: ctx.files.len(),
        files_omitted: ctx.excluded_files.len(),
        completeness_status,
        clipboard_copied,
        active_file_path,
        bypass_active: false,
        classification_signals: None,
        injected_files: vec![],
    })
}

/// Iteration 2A: Adaptive task preparation that classifies complexity before deciding
/// whether to run the full Engram preparation workflow.
///
/// - **Trivial tasks** (single-file typo fixes, simple config changes with no security,
///   architecture, or multi-file keywords): bypass contract creation and context compilation.
///   The agent can proceed directly with the native tool set.
/// - **Standard tasks**: delegate to `prepare_task_context` for the full Turn-0 workflow.
///
/// A `--no-bypass` flag can be used to force the full workflow regardless of classification.
pub fn prepare_task_context_adaptive(
    root: &Path,
    task: &str,
    budget: usize,
    format: PromptFormat,
    copy_clipboard: bool,
    write_active_file: bool,
    force_no_bypass: bool,
) -> Result<PreparedTaskOutput> {
    let signals = classify_task_complexity(task);

    if signals.classification == TaskComplexity::Trivial && !force_no_bypass {
        // Bypass path: no contract, no AST context, no preflight overhead
        let (core_task, _) = parse_task_and_planned_files(task, None);

        let body = match format {
            PromptFormat::Markdown => {
                format!(
                    "# Task\n{}\n\n## Engram Bypass Notice\n\
                     This task was classified as **trivial** by Engram's deterministic classifier.\n\
                     - **Reason:** {}\n\
                     - No Change Contract was created.\n\
                     - No preflight or context compilation was performed.\n\
                     - Proceed directly with the edit. Call `engram_postflight` only if you want compliance auditing.\n",
                    core_task, signals.reason
                )
            }
            PromptFormat::Text => {
                format!(
                    "=== TASK ===\n{}\n\n=== ENGRAM BYPASS ===\n\
                     Classification: TRIVIAL\n\
                     Reason: {}\n\
                     No contract or preflight was generated. Proceed directly.\n",
                    core_task, signals.reason
                )
            }
        };

        let mut clipboard_copied = false;
        if copy_clipboard {
            clipboard_copied = try_copy_to_clipboard(&body).is_ok();
        }

        return Ok(PreparedTaskOutput {
            task: core_task,
            contract_id: String::new(),
            contract_version: 0,
            baseline_commit: String::new(),
            status: "Bypassed".to_string(),
            expected_files: vec![],
            forbidden_boundaries: vec![],
            content: body,
            estimated_tokens: 0,
            token_budget: budget,
            files_included: 0,
            files_omitted: 0,
            completeness_status: "BYPASSED_TRIVIAL".to_string(),
            clipboard_copied,
            active_file_path: None,
            bypass_active: true,
            classification_signals: Some(signals),
            injected_files: vec![],
        });
    }

    // Standard path: delegate to full preparation
    let mut result = prepare_task_context(
        root,
        task,
        budget,
        format,
        copy_clipboard,
        write_active_file,
    )?;
    result.classification_signals = Some(signals);
    Ok(result)
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
