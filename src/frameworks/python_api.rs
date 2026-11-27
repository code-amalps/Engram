//! Python API & CLI Framework Adapter.
//!
//! Provides architectural entry-point and relationship discovery for:
//! - FastAPI & Flask HTTP routes (`@app.get`, `@app.post`, `@router.get`, etc.)
//! - Typer & Click CLI command handlers (`@app.command`, `@cli.command`, etc.)
//! - Service orchestrator and repository companions

use rusqlite::Connection;
use std::fs;
use std::path::Path;

use super::{
    AdapterConfidence, ArchitecturalCompanion, CompanionKind, DiscoveredEntryPoint,
    FrameworkRelationshipProvider,
};

pub struct PythonApiAdapter;

impl PythonApiAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PythonApiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameworkRelationshipProvider for PythonApiAdapter {
    fn name(&self) -> &'static str {
        "python_api"
    }

    fn detect(&self, workspace_root: &Path, files: &[String]) -> bool {
        self.detect_confidence(workspace_root, files).is_some()
    }

    fn detect_confidence(
        &self,
        workspace_root: &Path,
        files: &[String],
    ) -> Option<AdapterConfidence> {
        // 1. Manifest detection: pyproject.toml / requirements.txt / setup.py
        for manifest in &["pyproject.toml", "requirements.txt", "setup.py"] {
            let p = workspace_root.join(manifest);
            if p.exists() {
                if let Ok(content) = fs::read_to_string(&p) {
                    let c_low = content.to_ascii_lowercase();
                    if c_low.contains("fastapi")
                        || c_low.contains("flask")
                        || c_low.contains("typer")
                        || c_low.contains("click")
                    {
                        return Some(AdapterConfidence::Exact);
                    }
                }
            }
        }

        // 2. File pattern detection in workspace files list
        let has_pattern = files.iter().any(|f| {
            let l = f.to_ascii_lowercase().replace('\\', "/");
            (l.contains("/api/")
                || l.starts_with("api/")
                || l.contains("cli.py")
                || l.contains("app.py"))
                && l.ends_with(".py")
        });
        if has_pattern {
            return Some(AdapterConfidence::Pattern);
        }

        // 3. Fallback: Any Python files present
        if files.iter().any(|f| f.ends_with(".py")) {
            return Some(AdapterConfidence::Candidate);
        }

        None
    }

    fn expand_companions(
        &self,
        _workspace_root: &Path,
        _conn: &Connection,
        seed_path: &str,
    ) -> Vec<ArchitecturalCompanion> {
        let mut companions = Vec::new();
        let seed_norm = seed_path.replace('\\', "/");

        if seed_norm.contains("/api/") || seed_norm.ends_with("app.py") {
            // Find companion models / schemas
            let dir = Path::new(&seed_norm).parent().unwrap_or(Path::new(""));
            let models_path = dir.join("models.py").to_string_lossy().replace('\\', "/");
            companions.push(ArchitecturalCompanion {
                path: models_path,
                kind: CompanionKind::InterfaceModel,
                confidence: 0.85,
                reason: "API route request/response models".to_string(),
                exists_on_disk: true,
            });
        }

        companions
    }

    fn task_companion_seeds(
        &self,
        _workspace_root: &Path,
        _conn: &Connection,
        _task: &str,
    ) -> Vec<ArchitecturalCompanion> {
        Vec::new()
    }

    fn discover_entry_points(
        &self,
        workspace_root: &Path,
        conn: &Connection,
        files: &[String],
    ) -> Vec<DiscoveredEntryPoint> {
        let mut entry_points = Vec::new();

        // Target relevant Python files likely to declare routes or CLI commands
        for f in files {
            let f_norm = f.replace('\\', "/");
            if !f_norm.ends_with(".py") {
                continue;
            }

            // Skip non-python or virtualenv/vendor files
            if f_norm.contains("/site-packages/")
                || f_norm.contains("/venv/")
                || f_norm.contains("/.venv/")
            {
                continue;
            }

            let abs_path = workspace_root.join(f);
            let content = match fs::read_to_string(&abs_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let lines: Vec<&str> = content.lines().collect();
            for i in 0..lines.len() {
                let trimmed = lines[i].trim();

                // Detect FastAPI / Flask route decorators
                // e.g. @app.get("/api/v1/health") or @app.post("/api/v1/audit", ...)
                let is_route_decorator = (trimmed.starts_with("@app.")
                    || trimmed.starts_with("@router.")
                    || trimmed.starts_with("@api."))
                    && (trimmed.contains(".get(")
                        || trimmed.contains(".post(")
                        || trimmed.contains(".put(")
                        || trimmed.contains(".delete(")
                        || trimmed.contains(".patch("));

                if is_route_decorator {
                    let method = if trimmed.contains(".get(") {
                        "GET"
                    } else if trimmed.contains(".post(") {
                        "POST"
                    } else if trimmed.contains(".put(") {
                        "PUT"
                    } else if trimmed.contains(".delete(") {
                        "DELETE"
                    } else if trimmed.contains(".patch(") {
                        "PATCH"
                    } else {
                        "ROUTE"
                    };

                    let route_path = if let Some(start_quote) = trimmed.find('"') {
                        if let Some(end_quote) = trimmed[start_quote + 1..].find('"') {
                            &trimmed[start_quote + 1..start_quote + 1 + end_quote]
                        } else {
                            ""
                        }
                    } else if let Some(start_quote) = trimmed.find('\'') {
                        if let Some(end_quote) = trimmed[start_quote + 1..].find('\'') {
                            &trimmed[start_quote + 1..start_quote + 1 + end_quote]
                        } else {
                            ""
                        }
                    } else {
                        ""
                    };

                    // Find handler function on next lines
                    let mut fn_name = String::new();
                    let mut fn_line = (i + 1) as u32;
                    for j in (i + 1)..lines.len().min(i + 10) {
                        let l = lines[j].trim();
                        if l.starts_with("def ") {
                            let after_def = l[4..].trim();
                            if let Some(paren) = after_def.find('(') {
                                fn_name = after_def[..paren].trim().to_string();
                                fn_line = (j + 1) as u32;
                                break;
                            }
                        }
                    }

                    if !fn_name.is_empty() {
                        let route_str = format!("{method} {route_path}");
                        // Lookup node ID in kg_nodes
                        let node_id = conn
                            .query_row(
                                "SELECT id FROM kg_nodes WHERE relative_path = ?1 AND label = ?2",
                                rusqlite::params![f, fn_name],
                                |r| r.get::<_, String>(0),
                            )
                            .unwrap_or_else(|_| format!("symbol:{f}::FUNCTION::{fn_name}::route"));

                        entry_points.push(DiscoveredEntryPoint {
                            node_id,
                            kind: "HTTP_ROUTE".to_string(),
                            display_name: format!("{route_str} -> {fn_name}()"),
                            source_path: f.clone(),
                            line: fn_line,
                            route_or_command: Some(route_str),
                            confidence: AdapterConfidence::Exact,
                        });
                    }
                }

                // Detect Typer / Click CLI command decorators
                // e.g. @app.command("status") or @cli.command()
                let is_cli_command = trimmed.contains(".command(") || trimmed == "@app.command()";
                if is_cli_command {
                    let explicit_cmd = if let Some(start_quote) = trimmed.find('"') {
                        if let Some(end_quote) = trimmed[start_quote + 1..].find('"') {
                            Some(trimmed[start_quote + 1..start_quote + 1 + end_quote].to_string())
                        } else {
                            None
                        }
                    } else if let Some(start_quote) = trimmed.find('\'') {
                        if let Some(end_quote) = trimmed[start_quote + 1..].find('\'') {
                            Some(trimmed[start_quote + 1..start_quote + 1 + end_quote].to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Find handler function on next lines
                    let mut fn_name = String::new();
                    let mut fn_line = (i + 1) as u32;
                    for j in (i + 1)..lines.len().min(i + 10) {
                        let l = lines[j].trim();
                        if l.starts_with("def ") {
                            let after_def = l[4..].trim();
                            if let Some(paren) = after_def.find('(') {
                                fn_name = after_def[..paren].trim().to_string();
                                fn_line = (j + 1) as u32;
                                break;
                            }
                        }
                    }

                    if !fn_name.is_empty() {
                        let cmd_name = explicit_cmd.unwrap_or_else(|| {
                            fn_name
                                .trim_end_matches("_cmd")
                                .replace('_', "-")
                                .to_string()
                        });

                        let node_id = conn
                            .query_row(
                                "SELECT id FROM kg_nodes WHERE relative_path = ?1 AND label = ?2",
                                rusqlite::params![f, fn_name],
                                |r| r.get::<_, String>(0),
                            )
                            .unwrap_or_else(|_| format!("symbol:{f}::FUNCTION::{fn_name}::cli"));

                        entry_points.push(DiscoveredEntryPoint {
                            node_id,
                            kind: "CLI_COMMAND".to_string(),
                            display_name: format!("CLI command: {cmd_name} -> {fn_name}()"),
                            source_path: f.clone(),
                            line: fn_line,
                            route_or_command: Some(cmd_name),
                            confidence: AdapterConfidence::Exact,
                        });
                    }
                }
            }
        }

        entry_points
    }
}
