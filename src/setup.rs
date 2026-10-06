//! Safe, verified 1-click MCP setup for AI coding assistants: Cursor, Claude Desktop, Windsurf.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::{
    env, fs,
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub struct SetupTarget {
    pub name: &'static str,
    pub display_name: &'static str,
    pub default_path: fn(&Path) -> PathBuf,
}

pub const SUPPORTED_TARGETS: &[SetupTarget] = &[
    SetupTarget {
        name: "cursor",
        display_name: "Cursor (.cursor/mcp.json)",
        default_path: |root| root.join(".cursor").join("mcp.json"),
    },
    SetupTarget {
        name: "claude",
        display_name: "Claude Desktop",
        default_path: |_| {
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
    },
    SetupTarget {
        name: "windsurf",
        display_name: "Windsurf (~/.codeium/windsurf/mcp_config.json)",
        default_path: |_| {
            let home = if cfg!(windows) {
                env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\Default".into())
            } else {
                env::var("HOME").unwrap_or_else(|_| "/home/default".into())
            };
            PathBuf::from(home).join(".codeium").join("windsurf").join("mcp_config.json")
        },
    },
];

pub fn list_targets() -> Value {
    json!({
        "supported": [
            "cursor (Cursor IDE: .cursor/mcp.json)",
            "claude (Claude Desktop App)",
            "windsurf (Windsurf IDE: mcp_config.json)"
        ],
        "roadmap": [
            "claude-code (Claude Code CLI)",
            "cline (VS Code Cline extension)",
            "roo-code (VS Code Roo Code extension)"
        ]
    })
}

pub fn get_engram_binary_path() -> String {
    env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "engram".to_string())
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
        .find(|t| t.name.eq_ignore_ascii_case(client))
        .ok_or_else(|| {
            anyhow!(
                "Unsupported client '{}'. Supported: cursor, claude, windsurf. Run 'engram setup --list' for details.",
                client
            )
        })?;

    let config_path = (target.default_path)(root);
    let binary = get_engram_binary_path();
    let root_canon = fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .to_string_lossy()
        .to_string();

    // Check status
    if status_only {
        let exists = config_path.exists();
        let mut configured = false;
        if exists {
            if let Ok(c) = fs::read_to_string(&config_path) {
                if let Ok(v) = serde_json::from_str::<Value>(&c) {
                    configured = v.get("mcpServers").and_then(|s| s.get("engram")).is_some();
                }
            }
        }
        let binary_exists = Path::new(&binary).exists();
        let health = if configured && binary_exists {
            verify_mcp_health(&binary, root).is_ok()
        } else {
            false
        };

        return Ok(json!({
            "client": target.name,
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
            write_atomic(&config_path, &config)?;
        }
        return Ok(json!({
            "client": target.name,
            "config_path": config_path.to_string_lossy(),
            "removed": removed,
            "dry_run": dry_run
        }));
    }

    // Safe merge: preserve other servers, set or update "engram"
    let engram_server_block = json!({
        "command": binary,
        "args": ["-C", root_canon, "mcp"]
    });

    servers_obj.insert("engram".to_string(), engram_server_block);

    if dry_run {
        return Ok(json!({
            "client": target.name,
            "config_path": config_path.to_string_lossy(),
            "proposed_config": config,
            "dry_run": true,
            "message": "Dry-run: configuration would be safely updated."
        }));
    }

    // Backup and atomic write
    if config_path.exists() {
        let bak = config_path.with_extension("json.bak");
        let _ = fs::copy(&config_path, &bak);
    }
    write_atomic(&config_path, &config)?;

    // Verify MCP health
    let health = verify_mcp_health(&binary, root);

    Ok(json!({
        "client": target.name,
        "config_path": config_path.to_string_lossy(),
        "configured": true,
        "health_check": match health {
            Ok(tool_count) => format!("✓ Passed ({} Engram tools verified)", tool_count),
            Err(e) => format!("⚠️ Configured but handshake failed: {e}"),
        }
    }))
}

fn write_atomic(dest: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = dest.with_extension(format!("tmp_{}", std::process::id()));
    let text = serde_json::to_string_pretty(value)?;
    fs::write(&temp, text)?;
    fs::rename(temp, dest)?;
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
