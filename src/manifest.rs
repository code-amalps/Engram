//! Package and ecosystem intelligence: manifest & lockfile parsing, declared vs resolved versions.

use anyhow::Result;
use std::{
    collections::HashMap,
    fs,
    path::Path,
};

use crate::model::{EcosystemReport, PackageInfo};

pub fn detect_ecosystem(root: &Path) -> Result<EcosystemReport> {
    let mut report = EcosystemReport::default();

    // 1. Node.js / TypeScript / JavaScript (package.json + package-lock.json)
    let package_json_path = root.join("package.json");
    if package_json_path.exists() {
        if let Ok(content) = fs::read_to_string(&package_json_path) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
                let resolved_map = load_package_lock(root);

                if let Some(scripts) = v.get("scripts").and_then(|s| s.as_object()) {
                    for (k, val) in scripts {
                        if let Some(cmd) = val.as_str() {
                            report.scripts.insert(k.clone(), cmd.to_string());
                        }
                    }
                }

                if let Some(deps) = v.get("dependencies").and_then(|d| d.as_object()) {
                    for (pkg, ver) in deps {
                        let declared = ver.as_str().unwrap_or("").to_string();
                        let resolved = resolved_map.get(pkg).cloned();
                        classify_npm_package(pkg, declared, resolved, false, &mut report);
                    }
                }

                if let Some(dev_deps) = v.get("devDependencies").and_then(|d| d.as_object()) {
                    for (pkg, ver) in dev_deps {
                        let declared = ver.as_str().unwrap_or("").to_string();
                        let resolved = resolved_map.get(pkg).cloned();
                        classify_npm_package(pkg, declared, resolved, true, &mut report);
                    }
                }
            }
        }
    }

    // 2. Rust (Cargo.toml + Cargo.lock)
    let cargo_toml_path = root.join("Cargo.toml");
    if cargo_toml_path.exists() {
        if let Ok(content) = fs::read_to_string(&cargo_toml_path) {
            let cargo_lock_map = load_cargo_lock(root);
            parse_cargo_toml(&content, &cargo_lock_map, &mut report);
        }
    }

    // 3. Python (requirements.txt / pyproject.toml)
    let req_path = root.join("requirements.txt");
    if req_path.exists() {
        if let Ok(content) = fs::read_to_string(&req_path) {
            parse_requirements_txt(&content, &mut report);
        }
    }

    Ok(report)
}

fn load_package_lock(root: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let lock_path = root.join("package-lock.json");
    if let Ok(content) = fs::read_to_string(&lock_path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            // npm v7+ packages map
            if let Some(packages) = v.get("packages").and_then(|p| p.as_object()) {
                for (key, val) in packages {
                    if let Some(pkg_name) = key.strip_prefix("node_modules/") {
                        if let Some(ver) = val.get("version").and_then(|x| x.as_str()) {
                            map.insert(pkg_name.to_string(), ver.to_string());
                        }
                    }
                }
            }
            // npm v6 legacy dependencies map
            if let Some(deps) = v.get("dependencies").and_then(|d| d.as_object()) {
                for (pkg_name, val) in deps {
                    if let Some(ver) = val.get("version").and_then(|x| x.as_str()) {
                        map.entry(pkg_name.clone()).or_insert_with(|| ver.to_string());
                    }
                }
            }
        }
    }
    map
}

fn classify_npm_package(
    pkg: &str,
    declared: String,
    resolved: Option<String>,
    is_dev: bool,
    report: &mut EcosystemReport,
) {
    let info = PackageInfo {
        name: pkg.to_string(),
        declared_version: declared,
        resolved_version: resolved,
        is_dev,
        ecosystem: "npm".to_string(),
    };

    let p_lower = pkg.to_ascii_lowercase();
    let is_framework = p_lower.starts_with("@angular/")
        || p_lower == "react"
        || p_lower == "react-dom"
        || p_lower == "vue"
        || p_lower == "svelte"
        || p_lower == "next"
        || p_lower == "nuxt"
        || p_lower == "express"
        || p_lower == "@nestjs/core";

    let is_dev_tool = is_dev
        || p_lower == "typescript"
        || p_lower.contains("eslint")
        || p_lower.contains("prettier")
        || p_lower.contains("jest")
        || p_lower.contains("karma")
        || p_lower.contains("jasmine")
        || p_lower.contains("cypress")
        || p_lower.contains("playwright")
        || p_lower.contains("webpack")
        || p_lower.contains("vite");

    if is_framework {
        report.frameworks.push(info);
    } else if is_dev_tool {
        report.dev_tools.push(info);
    } else {
        report.libraries.push(info);
    }
}

fn load_cargo_lock(root: &Path) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let lock_path = root.join("Cargo.lock");
    if let Ok(content) = fs::read_to_string(&lock_path) {
        let mut cur_name = None;
        for line in content.lines() {
            let line = line.trim();
            if let Some(name) = line.strip_prefix("name = \"") {
                if let Some(n) = name.strip_suffix('"') {
                    cur_name = Some(n.to_string());
                }
            } else if let Some(ver) = line.strip_prefix("version = \"") {
                if let Some(v) = ver.strip_suffix('"') {
                    if let Some(n) = cur_name.take() {
                        map.insert(n, v.to_string());
                    }
                }
            }
        }
    }
    map
}

fn parse_cargo_toml(content: &str, lock_map: &HashMap<String, String>, report: &mut EcosystemReport) {
    let mut in_deps = false;
    let mut in_dev_deps = false;
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_deps = line == "[dependencies]";
            in_dev_deps = line == "[dev-dependencies]";
            continue;
        }
        if (in_deps || in_dev_deps) && !line.starts_with('#') && line.contains('=') {
            if let Some((name, rest)) = line.split_once('=') {
                let pkg_name = name.trim().to_string();
                let declared = rest.trim().trim_matches('"').to_string();
                let resolved = lock_map.get(&pkg_name).cloned();
                let info = PackageInfo {
                    name: pkg_name,
                    declared_version: declared,
                    resolved_version: resolved,
                    is_dev: in_dev_deps,
                    ecosystem: "cargo".to_string(),
                };
                if in_dev_deps {
                    report.dev_tools.push(info);
                } else {
                    report.libraries.push(info);
                }
            }
        }
    }
}

fn parse_requirements_txt(content: &str, report: &mut EcosystemReport) {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (pkg, ver) = if let Some((p, v)) = line.split_once("==") {
            (p.trim(), v.trim())
        } else if let Some((p, v)) = line.split_once(">=") {
            (p.trim(), v.trim())
        } else {
            (line, "*")
        };
        report.libraries.push(PackageInfo {
            name: pkg.to_string(),
            declared_version: ver.to_string(),
            resolved_version: None,
            is_dev: pkg.to_ascii_lowercase().contains("test"),
            ecosystem: "pip".to_string(),
        });
    }
}

pub fn sync_ecosystem_to_db(conn: &rusqlite::Connection, report: &EcosystemReport) -> Result<()> {
    let mut all_pkgs = Vec::new();
    all_pkgs.extend(report.frameworks.iter().map(|p| (p, false)));
    all_pkgs.extend(report.libraries.iter().map(|p| (p, false)));
    all_pkgs.extend(report.dev_tools.iter().map(|p| (p, true)));

    for (pkg, is_dev) in all_pkgs {
        conn.execute(
            "INSERT INTO project_ecosystem(name, ecosystem, declared_version, resolved_version, is_dev, source_manifest)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(name, ecosystem) DO UPDATE SET
                declared_version=excluded.declared_version,
                resolved_version=excluded.resolved_version,
                is_dev=excluded.is_dev",
            rusqlite::params![
                pkg.name,
                pkg.ecosystem,
                pkg.declared_version,
                pkg.resolved_version,
                if is_dev { 1 } else { 0 },
                format!("manifest:{}", pkg.ecosystem)
            ],
        )?;
    }
    Ok(())
}

pub fn format_agent_guard(report: &EcosystemReport) -> String {
    let mut out = String::new();
    if !report.frameworks.is_empty() || !report.libraries.is_empty() {
        out.push_str("AVAILABLE PACKAGES:\n");
        for p in &report.frameworks {
            let res = p.resolved_version.as_deref().unwrap_or(&p.declared_version);
            out.push_str(&format!("  {}@{} [framework]\n", p.name, res));
        }
        for p in report.libraries.iter().take(20) {
            let res = p.resolved_version.as_deref().unwrap_or(&p.declared_version);
            out.push_str(&format!("  {}@{}\n", p.name, res));
        }
        if report.libraries.len() > 20 {
            out.push_str(&format!("  ... (+{} more libraries)\n", report.libraries.len() - 20));
        }
        if !report.scripts.is_empty() {
            out.push_str("AVAILABLE SCRIPTS:\n");
            for (k, v) in report.scripts.iter().take(6) {
                out.push_str(&format!("  {}: {}\n", k, v));
            }
        }
        out.push_str("GUARD: Do NOT introduce or import packages not listed in the ecosystem manifest.\n");
    }
    out
}
