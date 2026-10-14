//! Git Correctness Subsystem:
//! Robust Git inspection handling staged vs unstaged diffs, file renames, deletions,
//! copies, merge commits, detached HEAD, and shallow clone fallback.

use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

pub const SHALLOW_HISTORY_TRUNCATED: &str = "SHALLOW_HISTORY_TRUNCATED";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitFileDiffKind {
    Added,
    Modified,
    Deleted,
    Renamed { old_path: String },
    Copied { old_path: String },
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitChangedFile {
    pub path: String,
    pub kind: GitFileDiffKind,
    pub staged: bool,
}

/// Normalize relative repository paths (forward slashes, trimmed, unquoted).
pub fn normalize_git_path(p: &str) -> String {
    p.trim()
        .trim_matches('"')
        .replace('\\', "/")
}

/// Check if a path is internal metadata (.engram, .git) that should be excluded from repository diffs.
pub fn is_internal_metadata_path(p: &str) -> bool {
    let clean = p.trim_start_matches("./").trim_end_matches('/');
    clean == ".engram"
        || clean.starts_with(".engram/")
        || clean == ".git"
        || clean.starts_with(".git/")
}

/// Check if the path is inside a Git repository.
pub fn is_git_repo(root: &Path) -> bool {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(root)
        .output();
    matches!(output, Ok(out) if out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true")
}

/// Detect shallow repository via `.git/shallow` or `git rev-parse --is-shallow-repository`.
pub fn is_shallow_repository(root: &Path) -> bool {
    // 1. Check direct .git/shallow marker
    let dot_git = root.join(".git");
    if dot_git.is_dir() {
        if dot_git.join("shallow").exists() {
            return true;
        }
    } else if dot_git.is_file() {
        // Submodule or worktree: .git contains 'gitdir: <path>'
        if let Ok(content) = fs::read_to_string(&dot_git) {
            for line in content.lines() {
                if let Some(gitdir_rel) = line.trim().strip_prefix("gitdir:") {
                    let gitdir = root.join(gitdir_rel.trim());
                    if gitdir.join("shallow").exists() {
                        return true;
                    }
                }
            }
        }
    }

    // 2. Query Git rev-parse directly
    let output = Command::new("git")
        .args(["rev-parse", "--is-shallow-repository"])
        .current_dir(root)
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
            return s == "true";
        }
    }

    false
}

/// Check if the repository is currently in a detached HEAD state.
pub fn is_detached_head(root: &Path) -> bool {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output();

    if let Ok(out) = output {
        if out.status.success() {
            let branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            return branch == "HEAD";
        }
    }

    false
}

/// Get authoritative current commit revision hash (works seamlessly on detached HEAD).
pub fn current_revision(root: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;

    if output.status.success() {
        let rev = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !rev.is_empty() {
            return Ok(rev);
        }
    }

    Ok("WORKTREE".to_string())
}

/// Get current branch name, or None if in detached HEAD state.
pub fn current_branch(root: &Path) -> Result<Option<String>> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(root)
        .output()?;

    if output.status.success() {
        let b = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !b.is_empty() && b != "HEAD" {
            return Ok(Some(b));
        }
    }

    Ok(None)
}

/// Verify whether a commit hash exists in local Git history.
pub fn verify_commit_exists(root: &Path, commit_sha: &str) -> bool {
    if commit_sha == "WORKTREE" || commit_sha.is_empty() {
        return true;
    }

    let output = Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", &format!("{commit_sha}^{{commit}}")])
        .current_dir(root)
        .output();

    matches!(output, Ok(out) if out.status.success())
}

/// Check if a commit is a merge commit (has more than 1 parent).
pub fn is_merge_commit(root: &Path, rev: &str) -> bool {
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", &format!("{rev}^2")])
        .current_dir(root)
        .output();

    matches!(output, Ok(out) if out.status.success())
}

/// Resolve a baseline commit hash.
/// Returns:
/// - Ok(Ok(full_commit_sha)): valid commit in history
/// - Ok(Err(SHALLOW_HISTORY_TRUNCATED)): commit missing because repo history is shallow
/// - Ok(Err(error_msg)): commit missing in a full repository
pub fn resolve_baseline(root: &Path, baseline: &str) -> Result<Result<String, String>> {
    let trimmed = baseline.trim();
    if trimmed == "WORKTREE" || trimmed.is_empty() {
        return Ok(Ok("WORKTREE".to_string()));
    }

    // Strip dirty snapshot suffix if present (e.g. <commit>+dirty:<hash>)
    let commit_target = if let Some((commit, _)) = trimmed.split_once("+dirty:") {
        commit.trim()
    } else {
        trimmed
    };

    if commit_target == "WORKTREE" || commit_target.is_empty() {
        return Ok(Ok("WORKTREE".to_string()));
    }

    // Try resolving exact SHA
    let output = Command::new("git")
        .args(["rev-parse", "--verify", "--quiet", &format!("{commit_target}^{{commit}}")])
        .current_dir(root)
        .output()?;

    if output.status.success() {
        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Ok(Ok(sha));
    }

    // Commit not found: check if repository is shallow
    if is_shallow_repository(root) {
        return Ok(Err(SHALLOW_HISTORY_TRUNCATED.to_string()));
    }

    Ok(Err(format!("Baseline commit '{trimmed}' is unavailable in current Git history.")))
}

/// Parse a single line from `git status --porcelain=v1`.
pub fn parse_porcelain_line(line: &str) -> Option<GitChangedFile> {
    let trimmed = line.trim_end();
    if trimmed.len() < 3 || trimmed.starts_with("##") {
        return None;
    }

    let index_status = trimmed.as_bytes()[0] as char;
    let work_status = trimmed.as_bytes()[1] as char;
    let rest = trimmed[3..].trim();

    // Check for rename "old -> new"
    if let Some((old_p, new_p)) = rest.split_once(" -> ") {
        let old = normalize_git_path(old_p);
        let new = normalize_git_path(new_p);
        if is_internal_metadata_path(&new) {
            return None;
        }
        let staged = index_status == 'R';
        return Some(GitChangedFile {
            path: new,
            kind: GitFileDiffKind::Renamed { old_path: old },
            staged,
        });
    }

    let path = normalize_git_path(rest);
    let path = path.trim_end_matches('/').to_string();
    if path.is_empty() || is_internal_metadata_path(&path) {
        return None;
    }

    let staged = index_status != ' ' && index_status != '?';

    let kind = match (index_status, work_status) {
        ('?', '?') => GitFileDiffKind::Untracked,
        ('A', _) => GitFileDiffKind::Added,
        ('D', _) | (_, 'D') => GitFileDiffKind::Deleted,
        ('C', _) => GitFileDiffKind::Copied { old_path: path.clone() },
        _ => GitFileDiffKind::Modified,
    };

    Some(GitChangedFile { path, kind, staged })
}

/// Parse a line from `git diff --name-status`.
pub fn parse_name_status_line(line: &str, staged: bool) -> Option<GitChangedFile> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts: Vec<&str> = trimmed.split('\t').collect();
    if parts.is_empty() {
        return None;
    }

    let status_code = parts[0].trim();
    let status_char = status_code.chars().next()?;

    match status_char {
        'A' => {
            let path = normalize_git_path(parts.get(1)?);
            if is_internal_metadata_path(&path) {
                return None;
            }
            Some(GitChangedFile {
                path,
                kind: GitFileDiffKind::Added,
                staged,
            })
        }
        'M' => {
            let path = normalize_git_path(parts.get(1)?);
            if is_internal_metadata_path(&path) {
                return None;
            }
            Some(GitChangedFile {
                path,
                kind: GitFileDiffKind::Modified,
                staged,
            })
        }
        'D' => {
            let path = normalize_git_path(parts.get(1)?);
            if is_internal_metadata_path(&path) {
                return None;
            }
            Some(GitChangedFile {
                path,
                kind: GitFileDiffKind::Deleted,
                staged,
            })
        }
        'R' => {
            let old = normalize_git_path(parts.get(1)?);
            let new = normalize_git_path(parts.get(2)?);
            if is_internal_metadata_path(&new) {
                return None;
            }
            Some(GitChangedFile {
                path: new,
                kind: GitFileDiffKind::Renamed { old_path: old },
                staged,
            })
        }
        'C' => {
            let old = normalize_git_path(parts.get(1)?);
            let new = normalize_git_path(parts.get(2)?);
            if is_internal_metadata_path(&new) {
                return None;
            }
            Some(GitChangedFile {
                path: new,
                kind: GitFileDiffKind::Copied { old_path: old },
                staged,
            })
        }
        _ => {
            let path = normalize_git_path(parts.get(1).unwrap_or(&parts[0]));
            if is_internal_metadata_path(&path) {
                return None;
            }
            Some(GitChangedFile {
                path,
                kind: GitFileDiffKind::Modified,
                staged,
            })
        }
    }
}

/// Retrieve comprehensive changed files list with robust parsing of staged/unstaged,
/// renames, copies, deletions, and shallow fallback.
pub fn get_diff_changed_files(
    root: &Path,
    baseline: Option<&str>,
    staged: bool,
    base_override: Option<&str>,
) -> Result<Result<(Vec<GitChangedFile>, String), String>> {
    // 1. PR / Base branch override (e.g., origin/main...HEAD)
    if let Some(base) = base_override {
        let output = Command::new("git")
            .args(["diff", "--name-status", &format!("{base}...HEAD")])
            .current_dir(root)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            if is_shallow_repository(root) {
                return Ok(Err(SHALLOW_HISTORY_TRUNCATED.to_string()));
            }
            return Ok(Err(format!("PR base comparison failed: {err}")));
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let files = text
            .lines()
            .filter_map(|l| parse_name_status_line(l, false))
            .collect();
        return Ok(Ok((files, format!("PR_HEAD (base: {base})"))));
    }

    // 2. Check baseline if provided
    let has_baseline = baseline.is_some() && baseline != Some("WORKTREE") && !baseline.unwrap_or("").is_empty();
    if let Some(b) = baseline {
        if has_baseline {
            match resolve_baseline(root, b)? {
                Ok(_) => {}
                Err(err) => return Ok(Err(err)),
            }
        }
    }

    // 3. Staged only
    if staged {
        let (args, desc) = if has_baseline {
            let b = baseline.unwrap();
            (
                vec!["diff".to_string(), "--name-status".to_string(), "--cached".to_string(), b.to_string()],
                format!("STAGED (baseline: {b})"),
            )
        } else {
            (
                vec!["diff".to_string(), "--name-status".to_string(), "--cached".to_string()],
                "STAGED".to_string(),
            )
        };

        let output = Command::new("git").args(&args).current_dir(root).output()?;
        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            if is_shallow_repository(root) {
                return Ok(Err(SHALLOW_HISTORY_TRUNCATED.to_string()));
            }
            return Ok(Err(format!("Git staged diff failed: {err}")));
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let files = text
            .lines()
            .filter_map(|l| parse_name_status_line(l, true))
            .collect();
        return Ok(Ok((files, desc)));
    }

    // 4. Working tree against baseline
    if has_baseline {
        let b = baseline.unwrap();
        let output = Command::new("git")
            .args(["diff", "--name-status", b])
            .current_dir(root)
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            if is_shallow_repository(root) {
                return Ok(Err(SHALLOW_HISTORY_TRUNCATED.to_string()));
            }
            return Ok(Err(format!("Git baseline diff failed: {err}")));
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let mut files: Vec<GitChangedFile> = text
            .lines()
            .filter_map(|l| parse_name_status_line(l, false))
            .collect();

        // Also incorporate untracked files
        let untracked_out = Command::new("git")
            .args(["status", "--porcelain=v1", "-uall"])
            .current_dir(root)
            .output()?;

        if untracked_out.status.success() {
            let status_text = String::from_utf8_lossy(&untracked_out.stdout);
            for l in status_text.lines() {
                if l.starts_with("??") {
                    if let Some(cf) = parse_porcelain_line(l) {
                        if !files.iter().any(|f| f.path == cf.path) {
                            files.push(cf);
                        }
                    }
                }
            }
        }

        let files = expand_changed_files(root, files);
        return Ok(Ok((files, format!("WORKING_TREE (baseline: {b})"))));
    }

    // 5. Default: porcelain status (both staged and unstaged working tree)
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "-uall"])
        .current_dir(root)
        .output()?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Ok(Err(format!("Git status failed: {err}")));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let files: Vec<GitChangedFile> = text
        .lines()
        .filter_map(parse_porcelain_line)
        .collect();

    let files = expand_changed_files(root, files);
    Ok(Ok((files, "WORKING_TREE".to_string())))
}

fn expand_directory_files(root: &Path, rel_dir: &str) -> Vec<String> {
    let mut result = Vec::new();
    let abs_dir = root.join(rel_dir);
    if let Ok(entries) = fs::read_dir(&abs_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if let Ok(rel) = p.strip_prefix(root) {
                let rel_str = rel.to_string_lossy().replace('\\', "/");
                if is_internal_metadata_path(&rel_str) {
                    continue;
                }
                if p.is_dir() {
                    result.extend(expand_directory_files(root, &rel_str));
                } else {
                    result.push(rel_str);
                }
            }
        }
    }
    result
}

fn expand_changed_files(root: &Path, files: Vec<GitChangedFile>) -> Vec<GitChangedFile> {
    let mut expanded = Vec::new();
    for f in files {
        let abs = root.join(&f.path);
        if abs.is_dir() {
            for sub in expand_directory_files(root, &f.path) {
                expanded.push(GitChangedFile {
                    path: sub,
                    kind: f.kind.clone(),
                    staged: f.staged,
                });
            }
        } else {
            expanded.push(f);
        }
    }
    expanded
}

/// Helper returning deduplicated changed file paths.
pub fn extract_changed_paths(
    root: &Path,
    baseline: Option<&str>,
    staged: bool,
    base_override: Option<&str>,
) -> Result<Result<(Vec<String>, String), String>> {
    match get_diff_changed_files(root, baseline, staged, base_override)? {
        Ok((files, snapshot)) => {
            let mut paths = Vec::new();
            for f in files {
                if !paths.contains(&f.path) {
                    paths.push(f.path.clone());
                }
                if let GitFileDiffKind::Renamed { ref old_path } = f.kind {
                    if !paths.contains(old_path) {
                        paths.push(old_path.clone());
                    }
                }
            }
            Ok(Ok((paths, snapshot)))
        }
        Err(err) => Ok(Err(err)),
    }
}
