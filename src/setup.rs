//! Safe, verified, capability-based editor and MCP setup for AI coding assistants.
//! Supports Cursor, VS Code, Claude Desktop, and Windsurf through a provider/adapter architecture,
//! with safe JSON merge, managed instruction blocks, conflict detection, and dry-run planning.

use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

use crate::instructions::{self, ManagedBlockAction, DEFAULT_INSTRUCTION_BODY};

pub const GOVERNED_TOOL_NAMES: &[&str] = &[
    "engram_check_constitution",
    "engram_doctor",
    "engram_drift",
    "engram_get_cochanges",
    "engram_get_context",
    "engram_get_ecosystem",
    "engram_get_impact",
    "engram_get_memory",
    "engram_inspect_symbol",
    "engram_postflight",
    "engram_preflight",
    "engram_record_decision",
    "engram_recover",
    "engram_why",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetupScope {
    Project,
    User,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCapabilities {
    pub project_mcp: bool,
    pub user_mcp: bool,
    pub instruction_files: bool,
    pub supports_stdio: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetectionConfidence {
    Confirmed,
    Likely,
    NotDetected,
}

/// Provider trait abstracting editor-specific configuration, instruction rules, and detection.
pub trait EditorProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn scope(&self) -> SetupScope;
    fn capabilities(&self) -> EditorCapabilities;
    fn default_config_path(&self, workspace_root: &Path) -> PathBuf;
    fn default_instruction_path(&self, workspace_root: &Path) -> Option<PathBuf>;
    fn detect(&self, workspace_root: &Path) -> DetectionConfidence;
}

pub struct CursorProvider;
impl EditorProvider for CursorProvider {
    fn id(&self) -> &'static str { "cursor" }
    fn display_name(&self) -> &'static str { "Cursor IDE" }
    fn scope(&self) -> SetupScope { SetupScope::Project }
    fn capabilities(&self) -> EditorCapabilities {
        EditorCapabilities {
            project_mcp: true,
            user_mcp: false,
            instruction_files: true,
            supports_stdio: true,
        }
    }
    fn default_config_path(&self, root: &Path) -> PathBuf {
        root.join(".cursor").join("mcp.json")
    }
    fn default_instruction_path(&self, root: &Path) -> Option<PathBuf> {
        Some(root.join(".cursorrules"))
    }
    fn detect(&self, root: &Path) -> DetectionConfidence {
        if root.join(".cursor").exists() {
            DetectionConfidence::Confirmed
        } else {
            DetectionConfidence::NotDetected
        }
    }
}

pub struct VsCodeProvider;
impl EditorProvider for VsCodeProvider {
    fn id(&self) -> &'static str { "vscode" }
    fn display_name(&self) -> &'static str { "Visual Studio Code (Copilot / Cline / Roo)" }
    fn scope(&self) -> SetupScope { SetupScope::Project }
    fn capabilities(&self) -> EditorCapabilities {
        EditorCapabilities {
            project_mcp: true,
            user_mcp: false,
            instruction_files: true,
            supports_stdio: true,
        }
    }
    fn default_config_path(&self, root: &Path) -> PathBuf {
        root.join(".vscode").join("mcp.json")
    }
    fn default_instruction_path(&self, root: &Path) -> Option<PathBuf> {
        Some(root.join(".github").join("copilot-instructions.md"))
    }
    fn detect(&self, root: &Path) -> DetectionConfidence {
        if root.join(".vscode").exists() || root.join(".github").join("copilot-instructions.md").exists() {
            DetectionConfidence::Confirmed
        } else {
            DetectionConfidence::NotDetected
        }
    }
}

pub struct ClaudeDesktopProvider;
impl EditorProvider for ClaudeDesktopProvider {
    fn id(&self) -> &'static str { "claude" }
    fn display_name(&self) -> &'static str { "Claude Desktop" }
    fn scope(&self) -> SetupScope { SetupScope::User }
    fn capabilities(&self) -> EditorCapabilities {
        EditorCapabilities {
            project_mcp: false,
            user_mcp: true,
            instruction_files: true,
            supports_stdio: true,
        }
    }
    fn default_config_path(&self, _root: &Path) -> PathBuf {
        if cfg!(windows) {
            let appdata = env::var("APPDATA").unwrap_or_else(|_| "C:\\Users\\Default\\AppData\\Roaming".into());
            PathBuf::from(appdata).join("Claude").join("claude_desktop_config.json")
        } else if cfg!(target_os = "macos") {
            let home = env::var("HOME").unwrap_or_else(|_| "/Users/default".into());
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Claude")
                .join("claude_desktop_config.json")
        } else {
            let home = env::var("HOME").unwrap_or_else(|_| "/home/default".into());
            PathBuf::from(home)
                .join(".config")
                .join("Claude")
                .join("claude_desktop_config.json")
        }
    }
    fn default_instruction_path(&self, root: &Path) -> Option<PathBuf> {
        Some(root.join("CLAUDE.md"))
    }
    fn detect(&self, _root: &Path) -> DetectionConfidence {
        let path = if cfg!(windows) {
            env::var("APPDATA").ok().map(|a| PathBuf::from(a).join("Claude"))
        } else if cfg!(target_os = "macos") {
            env::var("HOME").ok().map(|h| PathBuf::from(h).join("Library/Application Support/Claude"))
        } else {
            env::var("HOME").ok().map(|h| PathBuf::from(h).join(".config/Claude"))
        };
        if let Some(p) = path {
            if p.exists() {
                return DetectionConfidence::Confirmed;
            }
        }
        DetectionConfidence::NotDetected
    }
}

pub struct WindsurfProvider;
impl EditorProvider for WindsurfProvider {
    fn id(&self) -> &'static str { "windsurf" }
    fn display_name(&self) -> &'static str { "Windsurf IDE" }
    fn scope(&self) -> SetupScope { SetupScope::User }
    fn capabilities(&self) -> EditorCapabilities {
        EditorCapabilities {
            project_mcp: false,
            user_mcp: true,
            instruction_files: true,
            supports_stdio: true,
        }
    }
    fn default_config_path(&self, _root: &Path) -> PathBuf {
        let home = if cfg!(windows) {
            env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into())
        } else {
            env::var("HOME").unwrap_or_else(|_| "/home/default".into())
        };
        PathBuf::from(home).join(".codeium").join("windsurf").join("mcp_config.json")
    }
    fn default_instruction_path(&self, root: &Path) -> Option<PathBuf> {
        Some(root.join(".windsurfrules"))
    }
    fn detect(&self, _root: &Path) -> DetectionConfidence {
        let home = if cfg!(windows) {
            env::var("USERPROFILE").ok()
        } else {
            env::var("HOME").ok()
        };
        if let Some(h) = home {
            if PathBuf::from(h).join(".codeium").join("windsurf").exists() {
                return DetectionConfidence::Confirmed;
            }
        }
        DetectionConfidence::NotDetected
    }
}

pub fn all_providers() -> Vec<Box<dyn EditorProvider>> {
    vec![
        Box::new(CursorProvider),
        Box::new(VsCodeProvider),
        Box::new(ClaudeDesktopProvider),
        Box::new(WindsurfProvider),
    ]
}

pub fn get_provider(id: &str) -> Option<Box<dyn EditorProvider>> {
    all_providers().into_iter().find(|p| {
        p.id().eq_ignore_ascii_case(id) || p.display_name().to_lowercase().contains(&id.to_lowercase())
    })
}

// Backward-compatible struct and SUPPORTED_TARGETS for existing tests
pub struct EditorIntegration {
    pub id: &'static str,
    pub display_name: &'static str,
    pub capabilities: EditorCapabilities,
    pub default_config_path: fn(&Path) -> PathBuf,
    pub default_instruction_path: Option<fn(&Path) -> PathBuf>,
    pub detect_fn: fn(&Path) -> DetectionConfidence,
}

pub const SUPPORTED_TARGETS: &[EditorIntegration] = &[
    EditorIntegration {
        id: "cursor",
        display_name: "Cursor IDE",
        capabilities: EditorCapabilities {
            project_mcp: true,
            user_mcp: false,
            instruction_files: true,
            supports_stdio: true,
        },
        default_config_path: |root| CursorProvider.default_config_path(root),
        default_instruction_path: Some(|root| CursorProvider.default_instruction_path(root).unwrap()),
        detect_fn: |root| CursorProvider.detect(root),
    },
    EditorIntegration {
        id: "vscode",
        display_name: "Visual Studio Code (Copilot / Cline / Roo)",
        capabilities: EditorCapabilities {
            project_mcp: true,
            user_mcp: false,
            instruction_files: true,
            supports_stdio: true,
        },
        default_config_path: |root| VsCodeProvider.default_config_path(root),
        default_instruction_path: Some(|root| VsCodeProvider.default_instruction_path(root).unwrap()),
        detect_fn: |root| VsCodeProvider.detect(root),
    },
    EditorIntegration {
        id: "claude",
        display_name: "Claude Desktop",
        capabilities: EditorCapabilities {
            project_mcp: false,
            user_mcp: true,
            instruction_files: true,
            supports_stdio: true,
        },
        default_config_path: |root| ClaudeDesktopProvider.default_config_path(root),
        default_instruction_path: Some(|root| ClaudeDesktopProvider.default_instruction_path(root).unwrap()),
        detect_fn: |root| ClaudeDesktopProvider.detect(root),
    },
    EditorIntegration {
        id: "windsurf",
        display_name: "Windsurf IDE",
        capabilities: EditorCapabilities {
            project_mcp: false,
            user_mcp: true,
            instruction_files: true,
            supports_stdio: true,
        },
        default_config_path: |root| WindsurfProvider.default_config_path(root),
        default_instruction_path: Some(|root| WindsurfProvider.default_instruction_path(root).unwrap()),
        detect_fn: |root| WindsurfProvider.detect(root),
    },
];

pub fn get_engram_binary_path() -> String {
    env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "engram".to_string())
}

pub fn list_targets() -> Value {
    json!({
        "supported": [
            "cursor (Cursor IDE: .cursor/mcp.json + .cursorrules)",
            "vscode (VS Code: .vscode/mcp.json + .github/copilot-instructions.md)",
            "claude (Claude Desktop: claude_desktop_config.json + CLAUDE.md)",
            "windsurf (Windsurf IDE: mcp_config.json + .windsurfrules)"
        ]
    })
}

pub fn setup_client(
    root: &Path,
    client: &str,
    dry_run: bool,
    remove: bool,
    status_only: bool,
) -> Result<Value> {
    let provider = get_provider(client).ok_or_else(|| {
        anyhow!(
            "Unsupported client '{}'. Supported: cursor, vscode, claude, windsurf. Run 'engram setup --list' for details.",
            client
        )
    })?;

    let config_path = provider.default_config_path(root);
    let binary = get_engram_binary_path();
    let root_canon = fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .to_string();

    // Check status
    if status_only {
        let exists = config_path.exists();
        let configured = fs::read_to_string(&config_path)
            .ok()
            .and_then(|c| serde_json::from_str::<Value>(&c).ok())
            .and_then(|v| v.get("mcpServers")?.get("engram").cloned())
            .is_some();
        let binary_exists = Path::new(&binary).exists();
        let health = if configured && binary_exists {
            verify_mcp_health(&binary, root).is_ok()
        } else {
            false
        };

        return Ok(json!({
            "client": provider.id(),
            "scope": format!("{:?}", provider.scope()),
            "config_path": config_path.to_string_lossy(),
            "config_exists": exists,
            "engram_configured": configured,
            "binary_path": binary,
            "binary_exists": binary_exists,
            "health_verified": health,
            "status": if configured && health { "HEALTHY" } else if configured { "CONFIGURED_UNVERIFIED" } else { "NOT_CONFIGURED" }
        }));
    }

    if remove {
        let removed = remove_mcp_config(&config_path)?;
        return Ok(json!({
            "client": provider.id(),
            "scope": format!("{:?}", provider.scope()),
            "config_path": config_path.to_string_lossy(),
            "removed": removed,
            "dry_run": dry_run
        }));
    }

    if dry_run {
        let mut config_preview = if config_path.exists() {
            let text = fs::read_to_string(&config_path)?;
            serde_json::from_str(&text).unwrap_or(json!({ "mcpServers": {} }))
        } else {
            json!({ "mcpServers": {} })
        };
        let servers = config_preview
            .as_object_mut()
            .unwrap()
            .entry("mcpServers")
            .or_insert_with(|| json!({}));
        servers.as_object_mut().unwrap().insert(
            "engram".to_string(),
            json!({ "command": binary, "args": ["-C", root_canon, "mcp"] }),
        );

        return Ok(json!({
            "client": provider.id(),
            "scope": format!("{:?}", provider.scope()),
            "config_path": config_path.to_string_lossy(),
            "proposed_config": config_preview,
            "dry_run": true,
            "message": "Dry-run: configuration would be safely updated."
        }));
    }

    // Apply
    if config_path.exists() {
        let bak = config_path.with_extension("json.bak");
        let _ = fs::copy(&config_path, &bak);
    }
    apply_mcp_config(&config_path, &binary, &root_canon)?;

    let health = verify_mcp_health(&binary, root);

    Ok(json!({
        "client": provider.id(),
        "scope": format!("{:?}", provider.scope()),
        "config_path": config_path.to_string_lossy(),
        "configured": true,
        "health_check": match health {
            Ok(tool_count) => format!("✓ Passed ({} Engram tools verified)", tool_count),
            Err(e) => format!("⚠️ Configured but handshake failed: {e}"),
        }
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationPlanItem {
    pub client_id: String,
    pub display_name: String,
    pub scope: SetupScope,
    pub detection: DetectionConfidence,
    pub config_path: String,
    pub config_status: String,
    pub instruction_path: Option<String>,
    pub instruction_status: Option<String>,
    pub conflict: bool,
    pub conflict_details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupReport {
    pub mode: String,
    pub integrations: Vec<IntegrationPlanItem>,
    pub changes_applied: usize,
    pub conflicts_detected: usize,
}

pub struct SetupOptions {
    pub client: Option<String>,
    pub check: bool,
    pub dry_run: bool,
    pub apply: bool,
    pub force: bool,
    pub status: bool,
    pub remove: bool,
}

/// Orchestrates setup planning, detection, conflict resolution, and atomic application.
pub fn orchestrate_setup(root: &Path, opts: SetupOptions) -> Result<SetupReport> {
    let binary = get_engram_binary_path();
    let root_canon = fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .to_string();

    let providers: Vec<Box<dyn EditorProvider>> = if let Some(client) = &opts.client {
        vec![get_provider(client).ok_or_else(|| {
            anyhow!(
                "Unsupported client '{}'. Supported: cursor, vscode, claude, windsurf. Run 'engram setup --list' for details.",
                client
            )
        })?]
    } else {
        all_providers()
    };

    let mut items = Vec::new();
    let mut changes_applied = 0;
    let mut conflicts_detected = 0;

    if opts.remove {
        for provider in providers {
            let config_path = provider.default_config_path(root);
            let instruction_path = provider.default_instruction_path(root);

            let mcp_configured = config_path.exists() && {
                fs::read_to_string(&config_path)
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .and_then(|v| v.get("mcpServers")?.get("engram").cloned())
                    .is_some()
            };

            let instruction_has_block = if let Some(ref ip) = instruction_path {
                ip.exists() && {
                    fs::read_to_string(ip)
                        .map(|t| t.contains(instructions::START_MARKER))
                        .unwrap_or(false)
                }
            } else {
                false
            };

            if opts.client.is_none() && !mcp_configured && !instruction_has_block {
                continue;
            }

            let detection = provider.detect(root);

            let config_status = if mcp_configured {
                "CONFIGURED (Will remove)".to_string()
            } else {
                "NOT_CONFIGURED".to_string()
            };

            let instruction_status = if instruction_has_block {
                Some("MANAGED_BLOCK (Will strip)".to_string())
            } else if instruction_path.is_some() {
                Some("NOT_PRESENT".to_string())
            } else {
                None
            };

            if opts.apply {
                if mcp_configured && remove_mcp_config(&config_path)? {
                    changes_applied += 1;
                }
                if let Some(ref ip) = instruction_path {
                    if instruction_has_block {
                        let res = instructions::strip_managed_block(ip, false)?;
                        if res.action != instructions::ManagedBlockAction::NotFound {
                            changes_applied += 1;
                        }
                    }
                }
            }

            items.push(IntegrationPlanItem {
                client_id: provider.id().to_string(),
                display_name: provider.display_name().to_string(),
                scope: provider.scope(),
                detection,
                config_path: config_path.to_string_lossy().to_string(),
                config_status,
                instruction_path: instruction_path.map(|p| p.to_string_lossy().to_string()),
                instruction_status,
                conflict: false,
                conflict_details: None,
            });
        }

        let mode = if opts.apply {
            "REMOVED"
        } else {
            "DRY_RUN (REMOVAL)"
        };

        return Ok(SetupReport {
            mode: mode.to_string(),
            integrations: items,
            changes_applied,
            conflicts_detected: 0,
        });
    }

    for provider in providers {
        let detection = provider.detect(root);
        if opts.client.is_none() && detection == DetectionConfidence::NotDetected {
            continue;
        }

        let config_path = provider.default_config_path(root);
        let instruction_path = provider.default_instruction_path(root);

        // 1. Inspect MCP Config
        let (config_status, conflict, conflict_details) = inspect_mcp_config(&config_path, &binary, &root_canon)?;
        if conflict {
            conflicts_detected += 1;
        }

        // 2. Inspect Instruction File
        let instruction_status = if let Some(ref ip) = instruction_path {
            let res = instructions::sync_managed_block(ip, DEFAULT_INSTRUCTION_BODY, true)?;
            Some(format!("{:?}", res.action))
        } else {
            None
        };

        // 3. Apply mutations if authorized
        if opts.apply {
            if conflict && !opts.force {
                return Err(anyhow!(
                    "Conflict detected in {}: {}. Run with --force to overwrite conflicting configuration.",
                    config_path.display(),
                    conflict_details.as_deref().unwrap_or("Mismatched server command")
                ));
            }

            let applied_mcp = apply_mcp_config(&config_path, &binary, &root_canon)?;
            if applied_mcp {
                changes_applied += 1;
            }

            if let Some(ref ip) = instruction_path {
                let res = instructions::sync_managed_block(ip, DEFAULT_INSTRUCTION_BODY, false)?;
                if res.action != ManagedBlockAction::Unchanged {
                    changes_applied += 1;
                }
            }
        }

        items.push(IntegrationPlanItem {
            client_id: provider.id().to_string(),
            display_name: provider.display_name().to_string(),
            scope: provider.scope(),
            detection,
            config_path: config_path.to_string_lossy().to_string(),
            config_status,
            instruction_path: instruction_path.map(|p| p.to_string_lossy().to_string()),
            instruction_status,
            conflict,
            conflict_details,
        });
    }

    let mode = if opts.apply {
        "APPLIED"
    } else if opts.check {
        "CHECK"
    } else if opts.status {
        "STATUS"
    } else {
        "DRY_RUN"
    };

    Ok(SetupReport {
        mode: mode.to_string(),
        integrations: items,
        changes_applied,
        conflicts_detected,
    })
}

pub fn inspect_mcp_config(
    path: &Path,
    binary: &str,
    root_canon: &str,
) -> Result<(String, bool, Option<String>)> {
    if !path.exists() {
        return Ok(("NOT_CONFIGURED (File missing)".to_string(), false, None));
    }

    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => return Ok((format!("UNREADABLE: {e}"), false, None)),
    };

    let json: Value = match serde_json::from_str(&text) {
        Ok(j) => j,
        Err(e) => {
            let details = format!("Configuration file '{}' contains malformed JSON: {}", path.display(), e);
            return Ok(("MALFORMED_JSON".to_string(), true, Some(details)));
        }
    };

    let engram = json
        .get("mcpServers")
        .and_then(|s| s.get("engram"));

    if let Some(cfg) = engram {
        let cmd = cfg.get("command").and_then(|c| c.as_str()).unwrap_or("");
        let args = cfg.get("args").and_then(|a| a.as_array());

        let expected_cmd = binary;
        let expected_args = [Value::from("-C"), Value::from(root_canon), Value::from("mcp")];

        let cmd_matches = cmd == expected_cmd || Path::new(cmd).file_name() == Path::new(expected_cmd).file_name();
        let args_match = args.map(|a| a.as_slice() == expected_args.as_slice()).unwrap_or(false);

        if cmd_matches && args_match {
            Ok(("CONFIGURED_HEALTHY".to_string(), false, None))
        } else {
            let details = format!(
                "Existing configuration: command='{}', args={:?} (Expected: command='{}', args={:?})",
                cmd, args, expected_cmd, expected_args
            );
            Ok(("CONFLICT_DETECTED".to_string(), true, Some(details)))
        }
    } else {
        Ok(("NOT_CONFIGURED (mcpServers entry absent)".to_string(), false, None))
    }
}

pub fn apply_mcp_config(dest: &Path, binary: &str, root_canon: &str) -> Result<bool> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut config: Value = if dest.exists() {
        let text = fs::read_to_string(dest)?;
        serde_json::from_str(&text).map_err(|e| {
            anyhow!("Cannot apply: configuration file '{}' contains malformed JSON: {}", dest.display(), e)
        })?
    } else {
        json!({ "mcpServers": {} })
    };

    if !config.is_object() {
        config = json!({ "mcpServers": {} });
    }

    let servers = config
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| json!({}));

    if !servers.is_object() {
        *servers = json!({});
    }

    let servers_obj = servers.as_object_mut().unwrap();

    let engram_server_block = json!({
        "command": binary,
        "args": ["-C", root_canon, "mcp"]
    });

    let already_present = servers_obj.get("engram") == Some(&engram_server_block);
    if already_present {
        return Ok(false);
    }

    servers_obj.insert("engram".to_string(), engram_server_block);

    // Atomic write
    write_atomic_json(dest, &config)?;
    Ok(true)
}

pub fn remove_mcp_config(dest: &Path) -> Result<bool> {
    if !dest.exists() {
        return Ok(false);
    }
    let text = match fs::read_to_string(dest) {
        Ok(t) => t,
        Err(_) => return Ok(false),
    };
    let mut config: Value = serde_json::from_str(&text).map_err(|e| {
        anyhow!("Cannot remove: configuration file '{}' contains malformed JSON: {}", dest.display(), e)
    })?;

    if let Some(servers) = config.get_mut("mcpServers").and_then(|s| s.as_object_mut()) {
        if servers.remove("engram").is_some() {
            let bak = dest.with_extension("json.bak");
            let _ = fs::copy(dest, bak);
            write_atomic_json(dest, &config)?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn write_atomic_json(dest: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = dest.with_extension(format!("tmp_{}", std::process::id()));
    let text = serde_json::to_string_pretty(value)?;
    fs::write(&temp, text)?;

    #[cfg(windows)]
    {
        if dest.exists() {
            let _ = fs::remove_file(dest);
        }
        if let Err(e) = fs::rename(&temp, dest) {
            let _ = fs::copy(&temp, dest);
            let _ = fs::remove_file(&temp);
            if !dest.exists() {
                return Err(anyhow!("Failed to replace {}: {}", dest.display(), e));
            }
        }
    }

    #[cfg(not(windows))]
    {
        fs::rename(temp, dest)?;
    }

    Ok(())
}

/// Spawns engram mcp over stdio, sends initialize + tools/list, and verifies protocol handshake.
/// Enforces a bounded 5-second timeout, validates server identity ("engram"), protocol ("2024-11-05"),
/// capabilities ({tools, resources, prompts}), and exact 14 tool surface with zero duplicates.
pub fn verify_mcp_health(binary: &str, root: &Path) -> Result<usize> {
    verify_mcp_health_with_timeout(binary, root, 5)
}

pub fn verify_mcp_health_with_timeout(binary: &str, root: &Path, timeout_secs: u64) -> Result<usize> {
    if !Path::new(binary).exists() {
        bail!("Engram binary executable not found at '{}'", binary);
    }

    let mut child = Command::new(binary)
        .arg("-C")
        .arg(root)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn Engram binary '{}': {}", binary, e))?;

    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no child stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("no child stdout"))?;

    let (tx, rx) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || {
        let res = (|| -> Result<usize> {
            let mut reader = BufReader::new(stdout);

            // 1. Send initialize
            let init_req = json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {},
                    "clientInfo": { "name": "engram-setup-health", "version": "1.5.0" }
                }
            });
            writeln!(stdin, "{}", init_req)?;
            stdin.flush()?;

            let mut line = String::new();
            reader.read_line(&mut line)?;
            let init_res: Value = serde_json::from_str(&line)
                .map_err(|e| anyhow!("Failed to parse initialize response JSON: {}", e))?;

            if let Some(err) = init_res.get("error") {
                return Err(anyhow!("MCP initialize error: {:?}", err));
            }

            let result = init_res.get("result").ok_or_else(|| anyhow!("MCP initialize missing result"))?;
            let server_name = result.get("serverInfo").and_then(|s| s.get("name")).and_then(|n| n.as_str());
            if server_name != Some("engram") {
                return Err(anyhow!("Server identity mismatch: expected 'engram', got '{:?}'", server_name));
            }

            let protocol_ver = result.get("protocolVersion").and_then(|v| v.as_str());
            if protocol_ver != Some("2024-11-05") {
                return Err(anyhow!("Protocol negotiation mismatch: expected '2024-11-05', got '{:?}'", protocol_ver));
            }

            let caps = result.get("capabilities").and_then(|c| c.as_object());
            if let Some(c) = caps {
                if !c.contains_key("tools") || !c.contains_key("resources") || !c.contains_key("prompts") {
                    return Err(anyhow!("Server missing required capabilities: expected tools, resources, prompts"));
                }
            } else {
                return Err(anyhow!("Server missing capabilities object"));
            }

            // 2. Send tools/list
            let tools_req = json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/list",
                "params": {}
            });
            writeln!(stdin, "{}", tools_req)?;
            stdin.flush()?;

            line.clear();
            reader.read_line(&mut line)?;
            let tools_res: Value = serde_json::from_str(&line)
                .map_err(|e| anyhow!("Failed to parse tools/list response JSON: {}", e))?;

            if let Some(err) = tools_res.get("error") {
                return Err(anyhow!("MCP tools/list error: {:?}", err));
            }

            let tools_arr = tools_res
                .get("result")
                .and_then(|r| r.get("tools"))
                .and_then(|t| t.as_array())
                .ok_or_else(|| anyhow!("MCP tools/list missing tools array"))?;

            let mut tool_names: Vec<String> = tools_arr
                .iter()
                .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(str::to_string))
                .collect();

            let raw_count = tool_names.len();
            tool_names.sort();
            let before_dedup = tool_names.len();
            tool_names.dedup();
            if tool_names.len() != before_dedup {
                return Err(anyhow!("MCP tools/list contains duplicate tool names"));
            }

            if tool_names.iter().any(|n| n == "engram_gc") {
                return Err(anyhow!("Unsafe administrative tool 'engram_gc' leaked into MCP tool surface"));
            }

            for expected in GOVERNED_TOOL_NAMES {
                if !tool_names.contains(&expected.to_string()) {
                    return Err(anyhow!("Missing expected governed tool '{}' from MCP server", expected));
                }
            }

            if raw_count != 14 {
                return Err(anyhow!("Expected exactly 14 governed tools, server returned {}", raw_count));
            }

            Ok(raw_count)
        })();
        let _ = tx.send(res);
    });

    let res = match rx.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(result) => result,
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(anyhow!("MCP health check timed out after {}s", timeout_secs));
        }
    };

    let _ = child.kill();
    let _ = child.wait();
    let _ = handle.join();
    res
}
