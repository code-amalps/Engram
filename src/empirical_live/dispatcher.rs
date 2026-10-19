//! Sandboxed tool dispatch loop and execution boundary for Milestone B3.
//!
//! # Architecture & Security Invariants
//! 1. **Path Containment:** File operations (`read_file`, `write_file`, `list_directory`)
//!    are strictly contained within the sandbox root directory. Explicit parent traversal
//!    (`..`), null bytes (`\0`), and absolute path escapes are rejected fail-closed.
//! 2. **Symlink Rejection:** Canonical path verification alone is insufficient against
//!    symlink/junction race conditions. This dispatcher inspects intermediate components
//!    via `symlink_metadata` and rejects any symlinks or reparse points targeting paths
//!    outside or within the sandbox.
//! 3. **Command Allowlist & Argument Sanitization:** `execute_command` permits only a fixed,
//!    explicitly allowlisted set of build and test runners (`cargo`, `npm`, `npx`, `pytest`,
//!    `python`, `node`). Shell interpreters (`cmd`, `powershell`, `sh`, `bash`) and dangerous
//!    evaluation flags (`-c`, `/c`, `-e`, `--eval`, shell redirects, command chains) are strictly prohibited.
//! 4. **Duplicate Call Detection:** Repeated identical tool invocations within a session are
//!    tracked and bounded to prevent runaway infinite looping and budget draining.
//! 5. **Scope Boundary:** This module provides bounded offline tool execution for Milestone B3.
//!    Full OS-level process isolation, network egress restriction, and Windows Job Object
//!    containment are explicitly implemented in Milestone B5.

use crate::empirical_live::schema::{ContentBlock, ToolDefinition};
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Fixed, allowlisted executable binary names for benchmark execution.
/// Shell interpreters (`cmd.exe`, `powershell.exe`, `sh`, `bash`) are excluded to prevent shell injection.
pub const ALLOWLISTED_COMMANDS: &[&str] = &[
    "cargo",
    "cargo.exe",
    "npm",
    "npm.cmd",
    "npx",
    "npx.cmd",
    "pytest",
    "python",
    "python.exe",
    "node",
    "node.exe",
];

/// Prohibited command argument flags that enable arbitrary code or script evaluation.
pub const PROHIBITED_ARG_FLAGS: &[&str] = &[
    "-c",
    "/c",
    "-e",
    "--eval",
    "--command",
    "-command",
    "-encodedcommand",
    "--encodedcommand",
    "-exec",
];

/// Errors occurring during tool dispatch or sandboxed execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatcherError {
    EmptyPath,
    NullByteInPath,
    PathTraversalForbidden(String),
    PathEscapesSandbox { path: String, root: String },
    SymlinkForbidden(String),
    FileNotFound(String),
    NotAFile(String),
    NotADirectory(String),
    UnknownTool(String),
    MalformedArguments(String),
    DisallowedCommand { command: String },
    DisallowedArgument { arg: String, reason: String },
    CommandExecutionFailed(String),
    CommandTimeout { timeout_secs: u64 },
    DuplicateCallLimitExceeded { tool: String, count: usize },
    SecurityContainmentFailed(String),
    Io(String),
}

impl std::fmt::Display for DispatcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyPath => write!(f, "Path cannot be empty or whitespace"),
            Self::NullByteInPath => write!(f, "Path contains null bytes"),
            Self::PathTraversalForbidden(p) => {
                write!(
                    f,
                    "Directory traversal ('..') is strictly prohibited: '{p}'"
                )
            }
            Self::PathEscapesSandbox { path, root } => {
                write!(f, "Path '{path}' escapes sandbox boundary '{root}'")
            }
            Self::SymlinkForbidden(p) => {
                write!(
                    f,
                    "Symlink or junction access is strictly prohibited: '{p}'"
                )
            }
            Self::FileNotFound(p) => write!(f, "File not found: '{p}'"),
            Self::NotAFile(p) => write!(f, "Path is not a regular file: '{p}'"),
            Self::NotADirectory(p) => write!(f, "Path is not a directory: '{p}'"),
            Self::UnknownTool(name) => write!(f, "Unknown tool: '{name}'"),
            Self::MalformedArguments(msg) => write!(f, "Malformed tool arguments: {msg}"),
            Self::DisallowedCommand { command } => {
                write!(f, "Command '{command}' is not in the approved allowlist")
            }
            Self::DisallowedArgument { arg, reason } => {
                write!(f, "Argument '{arg}' rejected: {reason}")
            }
            Self::CommandExecutionFailed(msg) => write!(f, "Command execution failed: {msg}"),
            Self::CommandTimeout { timeout_secs } => {
                write!(f, "Command execution timed out after {timeout_secs}s")
            }
            Self::DuplicateCallLimitExceeded { tool, count } => {
                write!(
                    f,
                    "Tool '{tool}' called {count} times consecutively with identical arguments"
                )
            }
            Self::SecurityContainmentFailed(msg) => {
                write!(f, "Security containment failure: {msg}")
            }
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
        }
    }
}

impl std::error::Error for DispatcherError {}

use serde::{Deserialize, Serialize};

/// Abstract interface for executing tools invoked by an autonomous agent.
pub trait ToolDispatcher: Send + Sync {
    /// Return the list of tool definitions provided to the LLM.
    fn tool_definitions(&self) -> Vec<ToolDefinition>;

    /// Dispatch and execute a single tool call, returning a [`ContentBlock::ToolResult`].
    fn dispatch(
        &self,
        tool_name: &str,
        tool_use_id: &str,
        input: &serde_json::Value,
    ) -> ContentBlock;

    /// Return the task finish declaration if the `finish` tool was called.
    fn finish_declaration(&self) -> Option<TaskFinishDeclaration>;
}

/// Record of an executed tool call within a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchedCallRecord {
    pub tool_name: String,
    pub input_json: String,
    pub is_error: bool,
    pub duration_ms: u64,
}

/// Result of calling the `finish` tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFinishDeclaration {
    pub summary: String,
    pub success: bool,
}

/// Sandboxed tool dispatcher for offline agent evaluation.
pub struct SandboxToolDispatcher {
    sandbox_root: PathBuf,
    canonical_root: PathBuf,
    command_timeout: Duration,
    max_consecutive_duplicates: usize,
    allowlisted_commands: HashSet<String>,
    appcontainer_enabled: bool,
    call_history: Mutex<Vec<DispatchedCallRecord>>,
    last_call_signature: Mutex<Option<String>>,
    consecutive_duplicate_count: Mutex<usize>,
    finish_declaration: Mutex<Option<TaskFinishDeclaration>>,
}

impl SandboxToolDispatcher {
    /// Construct a new dispatcher rooted at the given sandbox path.
    pub fn new(sandbox_root: impl AsRef<Path>) -> Result<Self, DispatcherError> {
        let root = sandbox_root.as_ref().to_path_buf();
        if !root.exists() {
            fs::create_dir_all(&root).map_err(|e| DispatcherError::Io(e.to_string()))?;
        }
        let canonical_root = root.canonicalize().map_err(|e| {
            DispatcherError::Io(format!("Failed to canonicalize sandbox root: {e}"))
        })?;

        let mut allowlist = HashSet::new();
        for cmd in ALLOWLISTED_COMMANDS {
            allowlist.insert((*cmd).to_string());
        }

        Ok(Self {
            sandbox_root: root,
            canonical_root,
            command_timeout: Duration::from_secs(30),
            max_consecutive_duplicates: 3,
            allowlisted_commands: allowlist,
            appcontainer_enabled: false,
            call_history: Mutex::new(Vec::new()),
            last_call_signature: Mutex::new(None),
            consecutive_duplicate_count: Mutex::new(0),
            finish_declaration: Mutex::new(None),
        })
    }

    /// Enable or disable Windows AppContainer network containment (BLK-B6-02).
    pub fn with_appcontainer(mut self, enabled: bool) -> Self {
        self.appcontainer_enabled = enabled;
        self
    }

    /// Check if AppContainer containment is enabled.
    pub fn is_appcontainer_enabled(&self) -> bool {
        self.appcontainer_enabled
    }

    /// Set a custom command timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.command_timeout = timeout;
        self
    }

    /// Set a custom consecutive duplicate limit.
    pub fn with_duplicate_limit(mut self, limit: usize) -> Self {
        self.max_consecutive_duplicates = limit;
        self
    }

    /// Retrieve the canonical sandbox root path.
    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    /// Retrieve the uncanonicalized sandbox root path.
    pub fn sandbox_root(&self) -> &Path {
        &self.sandbox_root
    }

    /// Retrieve recorded tool call history.
    pub fn call_history(&self) -> Vec<DispatchedCallRecord> {
        self.call_history
            .lock()
            .expect("call history mutex poisoned")
            .clone()
    }

    /// Check if the task was explicitly finished via the `finish` tool.
    pub fn finish_declaration(&self) -> Option<TaskFinishDeclaration> {
        self.finish_declaration
            .lock()
            .expect("finish mutex poisoned")
            .clone()
    }

    /// Check whether the current operating system supports kernel process tree containment.
    ///
    /// Fails closed on all non-Windows platforms (AUD-B5-02).
    pub fn check_platform_containment_support(&self) -> Result<(), DispatcherError> {
        Self::check_platform_containment_for_os(std::env::consts::OS)
    }

    /// Pure helper to evaluate OS platform support for process containment without spawning.
    pub fn check_platform_containment_for_os(os_name: &str) -> Result<(), DispatcherError> {
        if os_name.eq_ignore_ascii_case("windows") {
            Ok(())
        } else {
            Err(DispatcherError::SecurityContainmentFailed(format!(
                "Platform containment unsupported on '{os_name}': Windows Job Objects are only supported on Windows NT (fail-closed)"
            )))
        }
    }

    /// Validate path containment and reject traversal, escapes, and symlinks.
    pub fn validate_path(&self, user_path: &str) -> Result<PathBuf, DispatcherError> {
        if user_path.contains('\0') {
            return Err(DispatcherError::NullByteInPath);
        }
        let trimmed = user_path.trim();
        if trimmed.is_empty() {
            return Err(DispatcherError::EmptyPath);
        }

        let path_obj = Path::new(trimmed);

        // 1. Prohibit explicit parent directory traversal components ('..')
        for comp in path_obj.components() {
            if comp == Component::ParentDir {
                return Err(DispatcherError::PathTraversalForbidden(trimmed.to_string()));
            }
        }

        // 2. Resolve candidate path
        let candidate = if path_obj.is_absolute() {
            path_obj.to_path_buf()
        } else {
            self.canonical_root.join(path_obj)
        };

        // 3. Inspect existing intermediate components for symlinks
        let mut check_ancestor = candidate.as_path();
        while !check_ancestor.exists() {
            match check_ancestor.parent() {
                Some(p) => check_ancestor = p,
                None => break,
            }
        }

        if check_ancestor.exists() {
            // Walk from canonical_root to check_ancestor verifying no symlinks
            let mut current = self.canonical_root.clone();
            if let Ok(rel) = check_ancestor.strip_prefix(&self.canonical_root) {
                for comp in rel.components() {
                    current.push(comp);
                    if let Ok(meta) = fs::symlink_metadata(&current)
                        && meta.file_type().is_symlink()
                    {
                        return Err(DispatcherError::SymlinkForbidden(
                            current.display().to_string(),
                        ));
                    }
                }
            }

            // Also check the check_ancestor itself
            if let Ok(meta) = fs::symlink_metadata(check_ancestor)
                && meta.file_type().is_symlink()
            {
                return Err(DispatcherError::SymlinkForbidden(
                    check_ancestor.display().to_string(),
                ));
            }
        }

        // 4. Verify canonical boundary containment if target exists
        if candidate.exists() {
            let canonical_target = candidate.canonicalize().map_err(|e| {
                DispatcherError::Io(format!(
                    "Failed to resolve path '{}': {e}",
                    candidate.display()
                ))
            })?;

            if !canonical_target.starts_with(&self.canonical_root) {
                return Err(DispatcherError::PathEscapesSandbox {
                    path: trimmed.to_string(),
                    root: self.canonical_root.display().to_string(),
                });
            }
            Ok(canonical_target)
        } else {
            // Target does not exist yet. Ensure existing ancestor is strictly within sandbox root.
            let canonical_ancestor = check_ancestor.canonicalize().map_err(|e| {
                DispatcherError::Io(format!(
                    "Failed to resolve ancestor path '{}': {e}",
                    check_ancestor.display()
                ))
            })?;

            if !canonical_ancestor.starts_with(&self.canonical_root) {
                return Err(DispatcherError::PathEscapesSandbox {
                    path: trimmed.to_string(),
                    root: self.canonical_root.display().to_string(),
                });
            }
            Ok(candidate)
        }
    }

    /// Execute the `read_file` tool.
    fn execute_read_file(&self, input: &serde_json::Value) -> Result<String, DispatcherError> {
        let path_str = input.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            DispatcherError::MalformedArguments("Missing required field 'path'".to_string())
        })?;

        let resolved = self.validate_path(path_str)?;
        if !resolved.exists() {
            return Err(DispatcherError::FileNotFound(path_str.to_string()));
        }
        if !resolved.is_file() {
            return Err(DispatcherError::NotAFile(path_str.to_string()));
        }

        fs::read_to_string(&resolved).map_err(|e| DispatcherError::Io(e.to_string()))
    }

    /// Execute the `write_file` tool.
    fn execute_write_file(&self, input: &serde_json::Value) -> Result<String, DispatcherError> {
        let path_str = input.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
            DispatcherError::MalformedArguments("Missing required field 'path'".to_string())
        })?;

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                DispatcherError::MalformedArguments("Missing required field 'content'".to_string())
            })?;

        let resolved = self.validate_path(path_str)?;
        if let Some(parent) = resolved.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|e| DispatcherError::Io(e.to_string()))?;
        }

        fs::write(&resolved, content).map_err(|e| DispatcherError::Io(e.to_string()))?;
        Ok(format!(
            "Successfully wrote {} bytes to '{}'",
            content.len(),
            path_str
        ))
    }

    /// Execute the `list_directory` tool.
    fn execute_list_directory(&self, input: &serde_json::Value) -> Result<String, DispatcherError> {
        let path_str = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let resolved = self.validate_path(path_str)?;
        if !resolved.exists() {
            return Err(DispatcherError::FileNotFound(path_str.to_string()));
        }
        if !resolved.is_dir() {
            return Err(DispatcherError::NotADirectory(path_str.to_string()));
        }

        let mut entries = Vec::new();
        let read_dir = fs::read_dir(&resolved).map_err(|e| DispatcherError::Io(e.to_string()))?;
        for entry in read_dir {
            let entry = entry.map_err(|e| DispatcherError::Io(e.to_string()))?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry
                .file_type()
                .map_err(|e| DispatcherError::Io(e.to_string()))?;
            if file_type.is_dir() {
                entries.push(format!("[dir]  {file_name}"));
            } else if file_type.is_file() {
                entries.push(format!("[file] {file_name}"));
            } else if file_type.is_symlink() {
                entries.push(format!("[symlink] {file_name} (unsupported)"));
            }
        }

        entries.sort();
        if entries.is_empty() {
            Ok("(directory is empty)".to_string())
        } else {
            Ok(entries.join("\n"))
        }
    }

    /// Execute the `execute_command` tool.
    fn execute_command_tool(&self, input: &serde_json::Value) -> Result<String, DispatcherError> {
        let cmd_str = input
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                DispatcherError::MalformedArguments("Missing required field 'command'".to_string())
            })?
            .trim();

        // Binary name must not contain directory slashes to prevent running arbitrary executables
        if cmd_str.contains('/') || cmd_str.contains('\\') {
            return Err(DispatcherError::DisallowedCommand {
                command: cmd_str.to_string(),
            });
        }

        let cmd_lower = cmd_str.to_ascii_lowercase();
        if !self.allowlisted_commands.contains(&cmd_lower) {
            return Err(DispatcherError::DisallowedCommand {
                command: cmd_str.to_string(),
            });
        }

        let args_val = input.get("args").and_then(|v| v.as_array());
        let mut args = Vec::new();
        if let Some(arr) = args_val {
            for item in arr {
                let s = item.as_str().ok_or_else(|| {
                    DispatcherError::MalformedArguments(
                        "Command args elements must be strings".to_string(),
                    )
                })?;

                // Argument validation: check for prohibited script evaluation flags
                let s_lower = s.to_ascii_lowercase();
                for prohibited in PROHIBITED_ARG_FLAGS {
                    if s_lower == *prohibited {
                        return Err(DispatcherError::DisallowedArgument {
                            arg: s.to_string(),
                            reason: "Arbitrary script evaluation flag is prohibited".to_string(),
                        });
                    }
                }

                // Check for shell metacharacters that attempt command chaining or redirection
                for ch in &[";", "&", "|", ">", "<", "`", "$", "\n", "\r"] {
                    if s.contains(ch) {
                        return Err(DispatcherError::DisallowedArgument {
                            arg: s.to_string(),
                            reason: format!("Shell metacharacter '{ch}' is prohibited"),
                        });
                    }
                }

                args.push(s.to_string());
            }
        }

        // 1. Resolve executable via trusted system search paths (AUD-B5-01)
        let resolved_exec = crate::empirical_live::security::resolve_trusted_executable(cmd_str)
            .map_err(|e| {
                DispatcherError::SecurityContainmentFailed(format!(
                    "Executable resolution failed for '{cmd_str}': {e}"
                ))
            })?;

        let mut cmd = Command::new(&resolved_exec);
        cmd.args(&args)
            .current_dir(&self.canonical_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // 2. Apply explicit environment sanitization policy with approved tool directory
        let mut env_policy =
            crate::empirical_live::security::EnvironmentPolicy::new(&self.canonical_root);
        if let Some(tool_dir) = resolved_exec.parent() {
            env_policy = env_policy.with_approved_tool_dir(tool_dir);
        }

        // If AppContainer containment is requested (BLK-B6-02)
        if self.appcontainer_enabled {
            #[cfg(not(windows))]
            {
                return Err(DispatcherError::SecurityContainmentFailed(format!(
                    "Platform containment unsupported on '{}': Windows AppContainer isolation is only supported on Windows NT (fail-closed)",
                    std::env::consts::OS
                )));
            }

            #[cfg(windows)]
            {
                let profile_name = format!("engram_sb_{}", uuid::Uuid::new_v4().simple());
                let profile =
                    crate::empirical_live::security::AppContainerProfile::create(&profile_name)
                        .map_err(|e| {
                            DispatcherError::SecurityContainmentFailed(format!(
                                "AppContainer profile creation failed: {e}"
                            ))
                        })?;

                // Grant access to WindowStation and Desktop (prevents 0xC0000142 STATUS_DLL_INIT_FAILED)
                profile.grant_window_station_and_desktop().map_err(|e| {
                    DispatcherError::SecurityContainmentFailed(format!(
                        "WindowStation/Desktop grant failed: {e}"
                    ))
                })?;

                // Grant access to canonical sandbox root
                const GENERIC_ALL: u32 = 0x10000000;
                profile
                    .grant_path_access(&self.canonical_root, GENERIC_ALL)
                    .map_err(|e| {
                        DispatcherError::SecurityContainmentFailed(format!(
                            "Sandbox root grant failed: {e}"
                        ))
                    })?;

                // Grant access to sandbox redirected cache directories
                for cache_sub in &[
                    ".engram_cargo_home",
                    ".engram_npm_cache",
                    ".engram_pycache",
                    ".engram_tmp",
                ] {
                    let cache_dir = self.canonical_root.join(cache_sub);
                    let _ = fs::create_dir_all(&cache_dir);
                    let _ = profile.grant_path_access(&cache_dir, GENERIC_ALL);
                }

                // Grant read/execute access to executable's directory
                const GENERIC_READ_EXECUTE: u32 = 0x80000000 | 0x20000000;
                if let Some(tool_dir) = resolved_exec.parent() {
                    let _ = profile.grant_path_access(tool_dir, GENERIC_READ_EXECUTE);
                }

                // If rustup home exists, grant read/execute access so cargo can locate toolchains
                if let Ok(rustup) = std::env::var("RUSTUP_HOME") {
                    let rpath = PathBuf::from(rustup);
                    if rpath.exists() {
                        let _ = profile.grant_path_access(&rpath, GENERIC_READ_EXECUTE);
                    }
                } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
                    let default_rustup = PathBuf::from(userprofile).join(".rustup");
                    if default_rustup.exists() {
                        let _ = profile.grant_path_access(&default_rustup, GENERIC_READ_EXECUTE);
                    }
                }

                let job =
                    crate::empirical_live::security::ProcessJobObject::create().map_err(|e| {
                        DispatcherError::SecurityContainmentFailed(format!(
                            "Job Object creation failed: {e}"
                        ))
                    })?;

                let sanitized_env = env_policy.build_sanitized_environment().map_err(|e| {
                    DispatcherError::SecurityContainmentFailed(format!(
                        "Environment policy failed: {e}"
                    ))
                })?;

                let output = crate::empirical_live::security::spawn_contained_appcontainer_process(
                    &profile,
                    &job,
                    &resolved_exec,
                    &args,
                    &self.canonical_root,
                    &sanitized_env,
                    self.command_timeout,
                )
                .map_err(|e| match e {
                    crate::empirical_live::security::SecurityError::CommandTimeout {
                        timeout_secs,
                    } => DispatcherError::CommandTimeout { timeout_secs },
                    other => DispatcherError::SecurityContainmentFailed(format!(
                        "AppContainer execution failed: {other}"
                    )),
                })?;

                return Ok(format!(
                    "Exit code: {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
                    output.exit_code, output.stdout, output.stderr
                ));
            }
        }

        env_policy.apply_to_command(&mut cmd).map_err(|e| {
            DispatcherError::SecurityContainmentFailed(format!(
                "Environment policy application failed: {e}"
            ))
        })?;

        // 3. Process-tree containment check: fail closed on unsupported platforms BEFORE spawning (AUD-B5-02)
        #[cfg(not(windows))]
        {
            return Err(DispatcherError::SecurityContainmentFailed(format!(
                "Platform containment unsupported on '{}': Kernel process tree containment via Windows Job Objects is only supported on Windows NT (fail-closed)",
                std::env::consts::OS
            )));
        }

        // Windows Job Object setup
        #[cfg(windows)]
        let job = match crate::empirical_live::security::ProcessJobObject::create() {
            Ok(j) => Some(j),
            Err(e) => {
                return Err(DispatcherError::SecurityContainmentFailed(format!(
                    "Failed to create Windows Job Object for process tree containment: {e}"
                )));
            }
        };

        // 4. Eliminate spawn-then-assign race condition: spawn suspended on Windows
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x00000004); // CREATE_SUSPENDED
        }

        let start = Instant::now();
        let mut child = cmd.spawn().map_err(|e| {
            DispatcherError::CommandExecutionFailed(format!(
                "Failed to spawn command '{cmd_str}': {e}"
            ))
        })?;

        // 5. Assign to Job Object before any user code can execute, then resume with strict accounting (AUD-B5-03)
        #[cfg(windows)]
        if let Some(ref j) = job {
            if let Err(e) = j.assign_process(&child) {
                let _ = child.kill();
                let _ = j.terminate(1);
                return Err(DispatcherError::SecurityContainmentFailed(format!(
                    "Failed to assign child process to Job Object (fail-closed): {e}"
                )));
            }
            if let Err(e) = j.resume_child(&mut child) {
                let _ = child.kill();
                let _ = j.terminate(1);
                return Err(DispatcherError::SecurityContainmentFailed(format!(
                    "Failed to resume suspended child process: {e}"
                )));
            }
        }

        // Wait bounded by timeout
        let timeout = self.command_timeout;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    let output = child.wait_with_output().map_err(|e| {
                        DispatcherError::CommandExecutionFailed(format!(
                            "Failed to collect output: {e}"
                        ))
                    })?;

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let exit_code = status.code().unwrap_or(-1);

                    return Ok(format!(
                        "Exit code: {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
                        exit_code, stdout, stderr
                    ));
                }
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        #[cfg(windows)]
                        if let Some(ref j) = job {
                            let _ = j.terminate(1);
                        }
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err(DispatcherError::CommandTimeout {
                            timeout_secs: timeout.as_secs(),
                        });
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    #[cfg(windows)]
                    if let Some(ref j) = job {
                        let _ = j.terminate(1);
                    }
                    let _ = child.kill();
                    return Err(DispatcherError::CommandExecutionFailed(format!(
                        "Wait error: {e}"
                    )));
                }
            }
        }
    }

    /// Execute the `finish` tool.
    fn execute_finish(&self, input: &serde_json::Value) -> Result<String, DispatcherError> {
        let summary = input
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("Task completed without detailed summary.")
            .to_string();

        let success = input
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let decl = TaskFinishDeclaration {
            summary: summary.clone(),
            success,
        };
        let mut lock = self
            .finish_declaration
            .lock()
            .expect("finish declaration mutex poisoned");
        *lock = Some(decl);

        Ok(format!(
            "Finish acknowledged: success={}, summary='{}'",
            success, summary
        ))
    }
}

impl ToolDispatcher for SandboxToolDispatcher {
    fn tool_definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "read_file".to_string(),
                description: "Read the UTF-8 text contents of a file within the sandbox.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative path to the file inside the sandbox"
                        }
                    },
                    "required": ["path"]
                }),
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "Write UTF-8 text content to a file within the sandbox.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative path to the file inside the sandbox"
                        },
                        "content": {
                            "type": "string",
                            "description": "UTF-8 text content to write"
                        }
                    },
                    "required": ["path", "content"]
                }),
            },
            ToolDefinition {
                name: "list_directory".to_string(),
                description: "List files and subdirectories inside a directory within the sandbox.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Relative directory path (defaults to root if omitted)"
                        }
                    }
                }),
            },
            ToolDefinition {
                name: "execute_command".to_string(),
                description: "Execute an allowlisted build or test command (cargo, npm, npx, pytest, python, node) inside the sandbox.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "command": {
                            "type": "string",
                            "description": "Binary name from allowlist (no path slashes)"
                        },
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "List of command line arguments"
                        }
                    },
                    "required": ["command"]
                }),
            },
            ToolDefinition {
                name: "finish".to_string(),
                description: "Declare that the engineering task is finished.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "summary": {
                            "type": "string",
                            "description": "Brief explanation of the changes made and testing performed"
                        },
                        "success": {
                            "type": "boolean",
                            "description": "True if the task succeeded, false if an unresolvable error occurred"
                        }
                    },
                    "required": ["summary"]
                }),
            },
        ]
    }

    fn dispatch(
        &self,
        tool_name: &str,
        tool_use_id: &str,
        input: &serde_json::Value,
    ) -> ContentBlock {
        let start = Instant::now();
        let input_str = serde_json::to_string(input).unwrap_or_default();
        let signature = format!("{tool_name}:{input_str}");

        // Duplicate call checking
        {
            let mut last_sig = self
                .last_call_signature
                .lock()
                .expect("signature mutex poisoned");
            let mut dup_count = self
                .consecutive_duplicate_count
                .lock()
                .expect("duplicate count mutex poisoned");

            if last_sig.as_deref() == Some(&signature) {
                *dup_count += 1;
                if *dup_count >= self.max_consecutive_duplicates {
                    let err = DispatcherError::DuplicateCallLimitExceeded {
                        tool: tool_name.to_string(),
                        count: *dup_count,
                    };
                    let duration_ms = start.elapsed().as_millis() as u64;
                    self.call_history
                        .lock()
                        .expect("call history mutex poisoned")
                        .push(DispatchedCallRecord {
                            tool_name: tool_name.to_string(),
                            input_json: input_str,
                            is_error: true,
                            duration_ms,
                        });
                    return ContentBlock::tool_result(tool_use_id, format!("Error: {err}"), true);
                }
            } else {
                *last_sig = Some(signature);
                *dup_count = 1;
            }
        }

        let result = match tool_name {
            "read_file" => self.execute_read_file(input),
            "write_file" => self.execute_write_file(input),
            "list_directory" => self.execute_list_directory(input),
            "execute_command" => self.execute_command_tool(input),
            "finish" => self.execute_finish(input),
            other => Err(DispatcherError::UnknownTool(other.to_string())),
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let (content, is_error) = match result {
            Ok(output) => (output, false),
            Err(e) => (format!("Error: {e}"), true),
        };

        self.call_history
            .lock()
            .expect("call history mutex poisoned")
            .push(DispatchedCallRecord {
                tool_name: tool_name.to_string(),
                input_json: input_str,
                is_error,
                duration_ms,
            });

        ContentBlock::tool_result(tool_use_id, content, is_error)
    }

    fn finish_declaration(&self) -> Option<TaskFinishDeclaration> {
        self.finish_declaration()
    }
}
