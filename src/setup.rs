//! Safe, verified, capability-based editor and MCP setup for AI coding assistants.
//! Supports Cursor, VS Code, Claude Desktop, and Windsurf with conflict detection and dry-run planning.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::instructions::{self, ManagedBlockAction, DEFAULT_INSTRUCTION_BODY};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorCapabilities {
    pub project_mcp: bool,
    pub user_mcp: bool,
    pub instruction_files: bool,
    pub supports_stdio: bool,
}

pub struct EditorIntegration {
    pub id: &'static str,
    pub display_name: &'static str,
    pub capabilities: EditorCapabilities,
    pub default_config_path: fn(&Path) -> PathBuf,
    pub default_instruction_path: Option<fn(&Path) -> PathBuf>,
    pub detect_fn: fn(&Path) -> DetectionConfidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DetectionConfidence {
    Confirmed,
    Likely,
    NotDetected,
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
        default_config_path: |root| root.join(".cursor").join("mcp.json"),
        default_instruction_path: Some(|root| root.join(".cursorrules")),
        detect_fn: |root| {
            if root.join(".cursor").exists() {
                DetectionConfidence::Confirmed
            } else {
                DetectionConfidence::NotDetected
            }
        },
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
        default_config_path: |root| root.join(".vscode").join("mcp.json"),
        default_instruction_path: Some(|root| root.join(".github").join("copilot-instructions.md")),
        detect_fn: |root| {
            if root.join(".vscode").exists() || root.join(".github").join("copilot-instructions.md").exists() {
                DetectionConfidence::Confirmed
            } else {
                DetectionConfidence::NotDetected
            }
        },
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
        default_config_path: |_| {
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
        },
        default_instruction_path: Some(|root| root.join("CLAUDE.md")),
        detect_fn: |_| {
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
        },
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
        default_config_path: |_| {
            let home = if cfg!(windows) {
                env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into())
            } else {
                env::var("HOME").unwrap_or_else(|_| "/home/default".into())
            };
            PathBuf::from(home).join(".codeium").join("windsurf").join("mcp_config.json")
        },
        default_instruction_path: Some(|root| root.join(".windsurfrules")),
        detect_fn: |_| {
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
        },
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
    let target = SUPPORTED_TARGETS
        .iter()
        .find(|t| t.id.eq_ignore_ascii_case(client) || t.display_name.to_lowercase().contains(&client.to_lowercase()))
        .ok_or_else(|| {
            anyhow!(
                "Unsupported client '{}'. Supported: cursor, vscode, claude, windsurf. Run 'engram setup --list' for details.",
                client
            )
        })?;

    let config_path = (target.default_config_path)(root);
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
            "client": target.id,
            "config_path": config_path.to_string_lossy(),
            "config_exists": exists,
            "engram_configured": configured,
            "binary_path": binary,
            "binary_exists": binary_exists,
            "health_verified": health,
            "status": if configured && health { "HEALTHY" } else if configured { "CONFIGURED_UNVERIFIED" } else { "NOT_CONFIGURED" }
        }));
    }

    // Read or initialize configuration
    let mut config: Value = if config_path.exists() {
        let text = fs::read_to_string(&config_path)?;
        serde_json::from_str(&text).unwrap_or(json!({ "mcpServers": {} }))
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

    if remove {
        let removed = servers_obj.remove("engram").is_some();
        if !dry_run && removed {
            write_atomic_json(&config_path, &config)?;
        }
        return Ok(json!({
            "client": target.id,
            "config_path": config_path.to_string_lossy(),
            "removed": removed,
            "dry_run": dry_run
        }));
    }

    if dry_run {
        return Ok(json!({
            "client": target.id,
            "config_path": config_path.to_string_lossy(),
            "proposed_config": config,
            "dry_run": true,
            "message": "Dry-run: configuration would be safely updated."
        }));
    }

    // Safe merge: preserve other servers, set or update "engram"
    let engram_server_block = json!({
        "command": binary,
        "args": ["-C", root_canon, "mcp"]
    });
    servers_obj.insert("engram".to_string(), engram_server_block);

    // Backup and atomic write
    if config_path.exists() {
        let bak = config_path.with_extension("json.bak");
        let _ = fs::copy(&config_path, &bak);
    }
    write_atomic_json(&config_path, &config)?;

    // Verify MCP health
    let health = verify_mcp_health(&binary, root);

    Ok(json!({
        "client": target.id,
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
}

/// Orchestrates setup planning, detection, conflict resolution, and atomic application.
pub fn orchestrate_setup(root: &Path, opts: SetupOptions) -> Result<SetupReport> {
    let binary = get_engram_binary_path();
    let root_canon = fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .to_string();

    let targets: Vec<&EditorIntegration> = if let Some(client) = &opts.client {
        vec![SUPPORTED_TARGETS
            .iter()
            .find(|t| t.id.eq_ignore_ascii_case(client))
            .ok_or_else(|| {
                anyhow!(
                    "Unsupported client '{}'. Supported: cursor, vscode, claude, windsurf. Run 'engram setup --list' for details.",
                    client
                )
            })?]
    } else {
        // Multi-target inspection: check all supported targets
        SUPPORTED_TARGETS.iter().collect()
    };

    let mut items = Vec::new();
    let mut changes_applied = 0;
    let mut conflicts_detected = 0;

    for target in targets {
        let detection = (target.detect_fn)(root);
        // If no explicit client was specified, only target editors with positive detection
        if opts.client.is_none() && detection == DetectionConfidence::NotDetected {
            continue;
        }

        let config_path = (target.default_config_path)(root);
        let instruction_path = target.default_instruction_path.map(|f| f(root));

        // 1. Inspect MCP Config
        let (config_status, conflict, conflict_details) = inspect_mcp_config(&config_path, &binary, &root_canon)?;
        if conflict {
            conflicts_detected += 1;
        }

        // 2. Inspect Instruction File
        let instruction_status = if let Some(ip) = &instruction_path {
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

            // Apply MCP update
            let applied_mcp = apply_mcp_config(&config_path, &binary, &root_canon)?;
            if applied_mcp {
                changes_applied += 1;
            }

            // Apply instruction file update
            if let Some(ip) = &instruction_path {
                let res = instructions::sync_managed_block(ip, DEFAULT_INSTRUCTION_BODY, false)?;
                if res.action != ManagedBlockAction::Unchanged {
                    changes_applied += 1;
                }
            }
        }

        items.push(IntegrationPlanItem {
            client_id: target.id.to_string(),
            display_name: target.display_name.to_string(),
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

fn inspect_mcp_config(
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
        Err(_) => return Ok(("MALFORMED_JSON".to_string(), false, None)),
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

fn apply_mcp_config(dest: &Path, binary: &str, root_canon: &str) -> Result<bool> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut config: Value = if dest.exists() {
        let text = fs::read_to_string(dest)?;
        serde_json::from_str(&text).unwrap_or(json!({ "mcpServers": {} }))
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
pub fn verify_mcp_health(binary: &str, root: &Path) -> Result<usize> {
    let mut child = Command::new(binary)
        .arg("-C")
        .arg(root)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no child stdin"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("no child stdout"))?;
    let mut reader = BufReader::new(stdout);

    // 1. Send initialize
    let init_req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "engram-setup-health", "version": "1.0" }
        }
    });
    writeln!(stdin, "{}", init_req)?;
    stdin.flush()?;

    let mut line = String::new();
    reader.read_line(&mut line)?;
    let init_res: Value = serde_json::from_str(&line)?;
    if init_res.get("error").is_some() {
        let _ = child.kill();
        return Err(anyhow!("MCP initialize error: {:?}", init_res["error"]));
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
    let tools_res: Value = serde_json::from_str(&line)?;
    let count = tools_res
        .get("result")
        .and_then(|r| r.get("tools"))
        .and_then(|t| t.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let _ = child.kill();
    if count >= 7 {
        Ok(count)
    } else {
        Err(anyhow!("Expected at least 7 tools, found {count}"))
    }
}
