//! Git Pre-Commit Hook Gating Subsystem (Phase 2).
//!
//! Provides deterministic pre-commit validation of staged changes against:
//! 1. Active Change Contracts (expected targets, boundary expansion)
//! 2. Sovereign Constitution rules (forbidden paths, e.g. secrets/**, monorepo isolation)
//! 3. Scope drift detection (unrequested UI modals, bulk localization sprawl)
//!
//! Exits 0 silently on clean, authorized commits without model calls.
//! Blocks commits with non-zero exit code and actionable recovery guidance on violations.

use anyhow::{anyhow, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::ci_guard::{self, FindingSeverity, GuardOptions, GuardStatus};
use crate::storage::root_path;

pub const HOOK_START_MARKER: &str = "# --- BEGIN ENGRAM CHANGE GUARD ---";
pub const HOOK_END_MARKER: &str = "# --- END ENGRAM CHANGE GUARD ---";

pub const HOOK_SCRIPT_BODY: &str = r#"# --- BEGIN ENGRAM CHANGE GUARD ---
# Engram Pre-Commit Hook: Asynchronous Change Guard verification
# Hardened binary resolution: supports explicit ENGRAM_PATH, installer executable,
# repository-relative target builds, and validates capability before execution.

engram_check_candidate() {
  cand="$1"
  [ -z "$cand" ] && return 1
  if [ -x "$cand" ] 2>/dev/null; then
    if "$cand" hook --help >/dev/null 2>&1; then
      ENGRAM_RESOLVED="$cand"
      return 0
    fi
  fi
  if [ -x "${cand}.exe" ] 2>/dev/null; then
    if "${cand}.exe" hook --help >/dev/null 2>&1; then
      ENGRAM_RESOLVED="${cand}.exe"
      return 0
    fi
  fi
  return 1
}

engram_check_path() {
  cmd_name="$1"
  [ -z "$cmd_name" ] && return 1
  if command -v "$cmd_name" >/dev/null 2>&1; then
    resolved="$(command -v "$cmd_name")"
    if "$resolved" hook --help >/dev/null 2>&1; then
      ENGRAM_RESOLVED="$resolved"
      return 0
    fi
    if [ -z "$OUTDATED_CANDIDATE" ]; then
      OUTDATED_CANDIDATE="$resolved"
    fi
  fi
  return 1
}

ENGRAM_RESOLVED=""
OUTDATED_CANDIDATE=""

# 1. Explicit ENGRAM_PATH environment override (never silently falls back)
if [ -n "$ENGRAM_PATH" ]; then
  if engram_check_candidate "$ENGRAM_PATH" || engram_check_path "$ENGRAM_PATH"; then
    "$ENGRAM_RESOLVED" hook pre-commit || "$ENGRAM_RESOLVED" hook run || exit $?
    exit 0
  else
    echo "[engram] Pre-commit hook error: Configured ENGRAM_PATH='$ENGRAM_PATH' is missing, non-executable, or does not support the required 'hook' command." >&2
    echo "[engram] Please ensure ENGRAM_PATH points to a compatible Engram binary." >&2
    echo "[engram] (Emergency bypass: use 'git commit --no-verify' to skip this hook)" >&2
    exit 1
  fi
fi

# 2. Recorded installer binary path (if available)
INSTALLER_BIN=""
if [ -n "$INSTALLER_BIN" ]; then
  if engram_check_candidate "$INSTALLER_BIN"; then
    : # Found compatible installer binary
  elif [ -z "$OUTDATED_CANDIDATE" ] && ([ -x "$INSTALLER_BIN" ] 2>/dev/null || [ -x "${INSTALLER_BIN}.exe" ] 2>/dev/null); then
    OUTDATED_CANDIDATE="$INSTALLER_BIN"
  fi
fi

# 3. Repository-root target builds (supports subfolder commits and worktrees)
if [ -z "$ENGRAM_RESOLVED" ]; then
  REPO_ROOT="$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
  engram_check_candidate "$REPO_ROOT/target/release/engram" || \
  engram_check_candidate "$REPO_ROOT/target/debug/engram" || \
  true
fi

# 4. System PATH candidates
if [ -z "$ENGRAM_RESOLVED" ]; then
  engram_check_path "engram" || true
fi

# 5. Standard installation directories
if [ -z "$ENGRAM_RESOLVED" ]; then
  if [ -n "$LOCALAPPDATA" ]; then
    engram_check_candidate "$LOCALAPPDATA/Programs/Engram/bin/engram" || true
  fi
  if [ -n "$USERPROFILE" ]; then
    engram_check_candidate "$USERPROFILE/.cargo/bin/engram" || true
  fi
  if [ -n "$HOME" ]; then
    engram_check_candidate "$HOME/.cargo/bin/engram" || \
    engram_check_candidate "$HOME/.local/bin/engram" || \
    engram_check_candidate "/usr/local/bin/engram" || \
    engram_check_candidate "/opt/homebrew/bin/engram" || \
    true
  fi
fi

# Execution or actionable diagnostic failure
if [ -n "$ENGRAM_RESOLVED" ]; then
  "$ENGRAM_RESOLVED" hook pre-commit || "$ENGRAM_RESOLVED" hook run || exit $?
else
  if [ -n "$OUTDATED_CANDIDATE" ]; then
    cand_version="$("$OUTDATED_CANDIDATE" --version 2>/dev/null || echo "unknown")"
    echo "[engram] Pre-commit hook error: Engram binary at '$OUTDATED_CANDIDATE' ($cand_version) does not support the required 'hook' command." >&2
    echo "[engram] Please upgrade your Engram installation or set ENGRAM_PATH to a compatible binary." >&2
    echo "[engram] (Emergency bypass: use 'git commit --no-verify' to skip this hook)" >&2
  else
    echo "[engram] Pre-commit hook error: No compatible 'engram' executable found on PATH or in repository target/." >&2
    echo "[engram] Please install Engram with change-guard support or set ENGRAM_PATH." >&2
    echo "[engram] (Emergency bypass: use 'git commit --no-verify' to skip this hook)" >&2
  fi
  exit 1
fi
# --- END ENGRAM CHANGE GUARD ---"#;

/// Generates the pre-commit hook script body, optionally embedding the recorded
/// installer executable path.
pub fn generate_hook_script(installer_bin: Option<&Path>) -> String {
    if let Some(bin) = installer_bin {
        let bin_str = bin.to_string_lossy().replace('\\', "/");
        HOOK_SCRIPT_BODY.replace(
            "INSTALLER_BIN=\"\"",
            &format!("INSTALLER_BIN=\"{bin_str}\""),
        )
    } else {
        HOOK_SCRIPT_BODY.to_string()
    }
}

/// Resolve the Git hooks directory, supporting standard repositories,
/// git worktrees (where `.git` is a file pointing to a gitdir), submodules,
/// and custom `core.hooksPath` configurations.
pub fn resolve_hooks_dir(root: &Path) -> Result<PathBuf> {
    // 1. Authoritative resolution via `git rev-parse --git-path hooks` (handles core.hooksPath and worktrees)
    if let Ok(out) = std::process::Command::new("git")
        .args(["rev-parse", "--git-path", "hooks"])
        .current_dir(root)
        .output()
    {
        if out.status.success() {
            let path_str = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !path_str.is_empty() {
                let p = PathBuf::from(path_str);
                if p.is_absolute() {
                    return Ok(p);
                } else {
                    return Ok(root.join(p));
                }
            }
        }
    }

    // 2. File-based fallback (synthetic test environments, offline or minimal systems)
    let dot_git = root.join(".git");
    if dot_git.is_dir() {
        let config_file = dot_git.join("config");
        if config_file.is_file() {
            if let Ok(cfg) = fs::read_to_string(&config_file) {
                for line in cfg.lines() {
                    let trimmed = line.trim();
                    if let Some(val) = trimmed.strip_prefix("hooksPath") {
                        if let Some((_, path_part)) = val.split_once('=') {
                            let hp = path_part.trim().trim_matches('"').trim_matches('\'');
                            if !hp.is_empty() {
                                let p = PathBuf::from(hp);
                                return Ok(if p.is_absolute() { p } else { root.join(p) });
                            }
                        }
                    }
                }
            }
        }
        return Ok(dot_git.join("hooks"));
    }
    if dot_git.is_file() {
        if let Ok(content) = fs::read_to_string(&dot_git) {
            for line in content.lines() {
                if let Some(gitdir_rel) = line.trim().strip_prefix("gitdir:") {
                    let gitdir_path = Path::new(gitdir_rel.trim());
                    let gitdir = if gitdir_path.is_absolute() {
                        gitdir_path.to_path_buf()
                    } else {
                        root.join(gitdir_path)
                    };
                    // Check for commondir in linked worktrees
                    let commondir_file = gitdir.join("commondir");
                    if commondir_file.is_file() {
                        if let Ok(common_rel) = fs::read_to_string(&commondir_file) {
                            let common_path = Path::new(common_rel.trim());
                            let common = if common_path.is_absolute() {
                                common_path.to_path_buf()
                            } else {
                                gitdir.join(common_path)
                            };
                            let hooks = common.join("hooks");
                            if let Ok(c) = fs::canonicalize(&hooks) {
                                return Ok(c);
                            }
                            return Ok(hooks);
                        }
                    }
                    let hooks = gitdir.join("hooks");
                    if let Ok(c) = fs::canonicalize(&hooks) {
                        return Ok(c);
                    }
                    return Ok(hooks);
                }
            }
        }
    }

    Err(anyhow!(
        "Cannot install Git hook: '{}' is not a Git repository (.git not found)",
        root.display()
    ))
}

/// Installs Engram's pre-commit hook into .git/hooks/pre-commit with dry-run support.
/// Preserves any existing foreign pre-commit hooks (Husky, Lefthook, custom shell scripts).
pub fn install_hook(root: &Path, dry_run: bool) -> Result<String> {
    let current_exe = std::env::current_exe().ok().and_then(|p| {
        let stem = p.file_stem()?.to_string_lossy().to_string();
        if stem.eq_ignore_ascii_case("engram") {
            fs::canonicalize(&p).ok().or(Some(p))
        } else {
            None
        }
    });
    install_hook_with_bin(root, dry_run, current_exe.as_deref())
}

/// Installs Engram's pre-commit hook with an explicitly specified installer executable path.
pub fn install_hook_with_bin(
    root: &Path,
    dry_run: bool,
    installer_bin: Option<&Path>,
) -> Result<String> {
    let root = root_path(root)?;
    let hooks_dir = resolve_hooks_dir(&root)?;
    if !hooks_dir.exists() && !dry_run {
        fs::create_dir_all(&hooks_dir)?;
    }

    let hook_path = hooks_dir.join("pre-commit");
    let script_body = generate_hook_script(installer_bin);

    if !hook_path.exists() {
        let content = format!("#!/bin/sh\n{}\n", script_body);
        if dry_run {
            return Ok(format!(
                "Dry-run: No hook detected.\nEngram would create: {}\nContent:\n{}",
                hook_path.display(),
                content
            ));
        }
        fs::write(&hook_path, content)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;
        }
        return Ok(format!(
            "Installed Engram pre-commit hook successfully at {}",
            hook_path.display()
        ));
    }

    let existing = fs::read_to_string(&hook_path)?;
    if existing.contains(HOOK_START_MARKER) {
        if dry_run {
            return Ok(format!(
                "Dry-run: Existing Engram hook detected in {}. No modifications needed.",
                hook_path.display()
            ));
        }
        return Ok(format!(
            "Engram Change Guard hook is already installed in {}",
            hook_path.display()
        ));
    }

    // Existing foreign hook: safely append Engram block
    let new_content = format!("{}\n\n{}\n", existing.trim_end(), script_body);
    if dry_run {
        return Ok(format!(
            "Dry-run: Existing hook detected in {}.\nEngram would safely append Change Guard block while preserving existing hook.\nNo files modified.",
            hook_path.display()
        ));
    }

    fs::write(&hook_path, new_content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;
    }

    Ok(format!(
        "Preserved existing pre-commit hook and appended Engram Change Guard block to {}",
        hook_path.display()
    ))
}

/// Uninstalls Engram's pre-commit hook cleanly with dry-run support.
/// Removes only Engram-managed markers, preserving foreign hook lines.
pub fn uninstall_hook(root: &Path, dry_run: bool) -> Result<String> {
    let root = root_path(root)?;
    let hooks_dir = resolve_hooks_dir(&root)?;
    let hook_path = hooks_dir.join("pre-commit");
    if !hook_path.exists() {
        return Ok(format!(
            "No pre-commit hook exists at {}",
            hook_path.display()
        ));
    }

    let content = fs::read_to_string(&hook_path)?;
    if !content.contains(HOOK_START_MARKER) {
        return Ok(format!(
            "Pre-commit hook at {} does not contain Engram Change Guard. Left unchanged.",
            hook_path.display()
        ));
    }

    let mut cleaned_lines = Vec::new();
    let mut skipping = false;

    for line in content.lines() {
        if line.trim() == HOOK_START_MARKER {
            skipping = true;
            continue;
        }
        if line.trim() == HOOK_END_MARKER {
            skipping = false;
            continue;
        }
        if !skipping {
            cleaned_lines.push(line);
        }
    }

    let remaining = cleaned_lines.join("\n").trim().to_string();

    if dry_run {
        if remaining.is_empty() || remaining == "#!/bin/sh" {
            return Ok(format!(
                "Dry-run: Pre-commit hook only contains Engram Change Guard.\nEngram would remove file: {}",
                hook_path.display()
            ));
        } else {
            return Ok(format!(
                "Dry-run: Pre-commit hook contains foreign scripts.\nEngram would remove only Change Guard block from: {}\nPreserved content:\n{}",
                hook_path.display(),
                remaining
            ));
        }
    }

    if remaining.is_empty() || remaining == "#!/bin/sh" {
        fs::remove_file(&hook_path)?;
        Ok(format!(
            "Removed empty pre-commit hook file cleanly from {}",
            hook_path.display()
        ))
    } else {
        fs::write(&hook_path, format!("{remaining}\n"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))?;
        }
        Ok(format!(
            "Removed Engram Change Guard block from {} while preserving foreign hook content",
            hook_path.display()
        ))
    }
}

/// Native Git pre-commit execution path.
/// Evaluates staged index changes deterministically.
///
/// Returns exit code:
/// - 0: Staged changes clean and authorized (or advisory warnings only in non-strict mode).
/// - 1: Contract violation, unauthorized file expansion, or forbidden path detected.
/// - 2: Configuration or internal execution error.
pub fn run_hook(
    root: &Path,
    strict: bool,
    enforce_contract: bool,
    policy_path: Option<&Path>,
    contract_file: Option<&str>,
    force_json: bool,
) -> Result<i32> {
    let resolved_root = root_path(root)?;

    let options = GuardOptions {
        policy_path: policy_path.map(PathBuf::from),
        base_commit: None,
        contract_file: contract_file.map(String::from),
        staged: true,
        strict,
        enforce_contract,
        diagnostic: false,
    };

    let guard_res = ci_guard::run_guard(&resolved_root, &options);

    if force_json {
        let json_str = serde_json::to_string_pretty(&guard_res)?;
        println!("{json_str}");
        return Ok(guard_res.exit_code);
    }

    if guard_res.changed_files.is_empty() {
        println!("✓ Engram Pre-Commit Guard: No staged changes to evaluate.");
        return Ok(0);
    }

    match guard_res.status {
        GuardStatus::Pass => {
            println!(
                "✓ Engram Pre-Commit Guard: Staged changes authorized ({} file(s) evaluated).",
                guard_res.changed_files.len()
            );
            if let Some(ref cid) = guard_res.context.contract_id {
                println!("  Active Contract: {cid} (Status: Locked)");
            }
            Ok(0)
        }
        GuardStatus::Warn if !strict => {
            println!(
                "⚠️  Engram Pre-Commit Guard: Staged changes accepted with {} advisory warning(s):",
                guard_res.findings.len()
            );
            for f in &guard_res.findings {
                println!("  • [{}] {}", f.rule_id, f.message);
                if let Some(ref rem) = f.remediation {
                    println!("    Remediation: {rem}");
                }
            }
            Ok(0)
        }
        GuardStatus::Block | GuardStatus::Warn => {
            eprintln!("\n🚫 Engram Pre-Commit Guard: COMMIT BLOCKED");
            eprintln!("──────────────────────────────────────────────────────────");
            eprintln!("Boundary violations detected in staged changes:");
            for f in &guard_res.findings {
                let icon = match f.severity {
                    FindingSeverity::Block => "❌",
                    FindingSeverity::Warn => "⚠️ ",
                    FindingSeverity::Info => "ℹ️ ",
                };
                eprintln!("  {} [{}] {}", icon, f.rule_id, f.message);
                if let Some(ref path) = f.path {
                    eprintln!("     Path: {path}");
                }
                if let Some(ref rem) = f.remediation {
                    eprintln!("     Remediation: {rem}");
                }
            }

            eprintln!("──────────────────────────────────────────────────────────");
            eprintln!("💡 Actionable Remediation:");
            eprintln!("  1. Unstage unauthorized file(s):");
            eprintln!("     git restore --staged <file>");
            if let Some(ref cid) = guard_res.context.contract_id {
                eprintln!("  2. If this expansion is intentional, ratify the contract:");
                eprintln!("     engram contract ratify {cid} --reason \"<human rationale>\"");
            } else {
                eprintln!("  2. To govern this task under a contract, run preflight:");
                eprintln!("     engram preflight \"<task goal>\"");
            }
            eprintln!("  3. Emergency bypass (skips all pre-commit hooks):");
            eprintln!("     git commit --no-verify\n");

            Ok(1)
        }
        GuardStatus::Error => {
            eprintln!("\n❌ Engram Pre-Commit Guard: Execution Error");
            eprintln!("  {}", guard_res.summary);
            Ok(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_lifecycle_dry_run_and_clean_removal() {
        let temp_dir =
            std::env::temp_dir().join(format!("engram_hook_lifecycle_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();

        // Must fail gracefully when .git does not exist
        let err = install_hook(&temp_dir, false);
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains(".git not found"));

        // Initialize fake .git directory
        let git_dir = temp_dir.join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        // 1. Dry run install on empty hooks dir
        let dry_install = install_hook(&temp_dir, true).unwrap();
        assert!(dry_install.contains("Dry-run: No hook detected"));
        assert!(!git_dir.join("hooks").join("pre-commit").exists());

        // 2. Real install creates new hook
        let real_install = install_hook(&temp_dir, false).unwrap();
        assert!(real_install.contains("Installed Engram pre-commit hook successfully"));
        let hook_file = git_dir.join("hooks").join("pre-commit");
        assert!(hook_file.exists());
        let content = fs::read_to_string(&hook_file).unwrap();
        assert!(content.contains(HOOK_START_MARKER));
        assert!(content.contains(HOOK_END_MARKER));
        assert!(content.contains("hook run"));

        // 3. Repeated install is idempotent
        let repeat = install_hook(&temp_dir, false).unwrap();
        assert!(repeat.contains("already installed"));

        // 4. Uninstall removes empty hook file completely
        let uninst = uninstall_hook(&temp_dir, false).unwrap();
        assert!(uninst.contains("Removed empty pre-commit hook file cleanly"));
        assert!(!hook_file.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_hook_preserves_foreign_hooks() {
        let temp_dir =
            std::env::temp_dir().join(format!("engram_hook_foreign_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&temp_dir).unwrap();
        let hooks_dir = temp_dir.join(".git").join("hooks");
        fs::create_dir_all(&hooks_dir).unwrap();
        let hook_file = hooks_dir.join("pre-commit");

        // Foreign hook (e.g. Husky/npm test)
        fs::write(&hook_file, "#!/bin/sh\n# Existing Husky Hook\nnpm test\n").unwrap();

        // 1. Install appends Engram block without overwriting foreign script
        let install_res = install_hook(&temp_dir, false).unwrap();
        assert!(install_res.contains("Preserved existing pre-commit hook"));

        let content_after = fs::read_to_string(&hook_file).unwrap();
        assert!(content_after.contains("# Existing Husky Hook"));
        assert!(content_after.contains("npm test"));
        assert!(content_after.contains(HOOK_START_MARKER));

        // 2. Uninstall removes ONLY Engram block, preserving foreign script
        let uninst_res = uninstall_hook(&temp_dir, false).unwrap();
        assert!(uninst_res.contains("preserving foreign hook content"));

        let content_final = fs::read_to_string(&hook_file).unwrap();
        assert!(content_final.contains("# Existing Husky Hook"));
        assert!(content_final.contains("npm test"));
        assert!(!content_final.contains(HOOK_START_MARKER));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_hook_resolves_git_worktree_hooks_dir() {
        let base_dir =
            std::env::temp_dir().join(format!("engram_worktree_test_{}", uuid::Uuid::new_v4()));
        let main_repo = base_dir.join("main_repo");
        let worktree_dir = base_dir.join("wt_repo");
        fs::create_dir_all(&main_repo).unwrap();
        fs::create_dir_all(&worktree_dir).unwrap();

        // Main repo .git directory
        let main_git = main_repo.join(".git");
        let main_hooks = main_git.join("hooks");
        fs::create_dir_all(&main_hooks).unwrap();

        // Worktree metadata in main repo
        let wt_meta = main_git.join("worktrees").join("wt_branch");
        fs::create_dir_all(&wt_meta).unwrap();
        fs::write(wt_meta.join("commondir"), "../..\n").unwrap();

        // Linked worktree .git file pointing to wt_meta
        fs::write(
            worktree_dir.join(".git"),
            format!("gitdir: {}\n", wt_meta.display()),
        )
        .unwrap();

        let resolved = resolve_hooks_dir(&worktree_dir).unwrap();
        assert_eq!(
            fs::canonicalize(&resolved).unwrap(),
            fs::canonicalize(&main_hooks).unwrap()
        );

        // Verify install and uninstall in linked worktree
        let inst_msg = install_hook(&worktree_dir, false).unwrap();
        assert!(inst_msg.contains("Installed Engram pre-commit hook"));
        assert!(main_hooks.join("pre-commit").exists());

        let uninst_msg = uninstall_hook(&worktree_dir, false).unwrap();
        assert!(uninst_msg.contains("Removed empty pre-commit hook"));
        assert!(!main_hooks.join("pre-commit").exists());

        let _ = fs::remove_dir_all(&base_dir);
    }
}
