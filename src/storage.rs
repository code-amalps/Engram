use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

pub fn now() -> String {
    Utc::now().to_rfc3339()
}

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn root_path(root: &Path) -> Result<PathBuf> {
    fs::canonicalize(root)
        .with_context(|| format!("Cannot resolve repository root {}", root.display()))
}

pub fn project_id(root: &Path) -> String {
    let clean = root
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .to_lowercase()
        .replace('\\', "/");
    hash(clean.as_bytes())[..24].to_string()
}

pub fn dirs_fallback() -> PathBuf {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(env::temp_dir)
}

pub fn cache_dir(root: &Path) -> Result<PathBuf> {
    let root = root_path(root)?;
    let base = if cfg!(windows) {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(env::temp_dir)
            .join("Engram")
            .join("cache")
    } else {
        env::var_os("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| dirs_fallback().join(".cache"))
            .join("engram")
    };
    Ok(base.join(project_id(&root)))
}

pub fn db(root: &Path) -> Result<Connection> {
    let dir = cache_dir(root)?;
    fs::create_dir_all(&dir)?;
    let conn = Connection::open(dir.join("derived_symbols.db"))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS files(
            path TEXT PRIMARY KEY,
            content_hash TEXT NOT NULL,
            analysis_key TEXT NOT NULL DEFAULT '',
            language TEXT NOT NULL,
            excerpt TEXT NOT NULL,
            revision TEXT NOT NULL,
            analyzed_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS entities(
            entity_id TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            signature TEXT NOT NULL,
            ast_fingerprint TEXT NOT NULL,
            created_at_revision TEXT NOT NULL,
            invalidated_at_revision TEXT,
            status TEXT NOT NULL DEFAULT 'OBSERVED'
        );
        CREATE INDEX IF NOT EXISTS entities_path ON entities(path);
        CREATE TABLE IF NOT EXISTS entity_identity(
            entity_id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            created_at_revision TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS entity_versions(
            version_id TEXT PRIMARY KEY,
            entity_id TEXT NOT NULL,
            path TEXT NOT NULL,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            signature TEXT NOT NULL,
            ast_fingerprint TEXT NOT NULL,
            created_at_revision TEXT NOT NULL,
            invalidated_at_revision TEXT,
            status TEXT NOT NULL DEFAULT 'OBSERVED',
            UNIQUE(entity_id, created_at_revision)
        );
        CREATE INDEX IF NOT EXISTS entity_versions_path ON entity_versions(path, invalidated_at_revision);
        CREATE TABLE IF NOT EXISTS relationships(
            source_id TEXT NOT NULL,
            target TEXT NOT NULL,
            kind TEXT NOT NULL,
            evidence TEXT NOT NULL,
            confidence REAL NOT NULL,
            path TEXT NOT NULL,
            line INTEGER NOT NULL,
            created_at_revision TEXT NOT NULL,
            invalidated_at_revision TEXT
        );
        CREATE TABLE IF NOT EXISTS revisions(
            revision_id TEXT PRIMARY KEY,
            commit_hash TEXT NOT NULL,
            branch_name TEXT,
            dirty INTEGER NOT NULL,
            parent_hash TEXT,
            analysis_key TEXT NOT NULL DEFAULT '',
            recorded_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS evidence(
            evidence_id TEXT PRIMARY KEY,
            target_id TEXT NOT NULL,
            target_type TEXT NOT NULL,
            tier TEXT NOT NULL,
            confidence REAL NOT NULL,
            source_type TEXT NOT NULL,
            source_path TEXT,
            source_line INTEGER,
            recorded_at TEXT NOT NULL,
            decay_half_life_commits INTEGER,
            verification_status TEXT NOT NULL,
            details_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS evidence_target ON evidence(target_id);
        CREATE TABLE IF NOT EXISTS observations(
            observation_id TEXT PRIMARY KEY,
            target_id TEXT NOT NULL,
            revision_id TEXT NOT NULL,
            property_name TEXT NOT NULL,
            value_json TEXT NOT NULL,
            observed_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS contracts(
            task_id TEXT NOT NULL,
            version INTEGER NOT NULL,
            revision_id TEXT NOT NULL,
            body_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            PRIMARY KEY(task_id, version)
        );
        CREATE TABLE IF NOT EXISTS task_gaps(
            gap_id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            code TEXT NOT NULL,
            description TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS change_events(
            change_id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL,
            revision_id TEXT NOT NULL,
            path TEXT NOT NULL,
            symbol_name TEXT,
            change_kind TEXT NOT NULL,
            origin TEXT NOT NULL,
            evidence_tier TEXT NOT NULL,
            observed_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS tasks(
            task_id TEXT PRIMARY KEY,
            task TEXT NOT NULL,
            created_at TEXT NOT NULL,
            state TEXT NOT NULL,
            baseline_json TEXT NOT NULL,
            contract_json TEXT
        );
        CREATE TABLE IF NOT EXISTS decisions(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL,
            decision TEXT NOT NULL,
            reason TEXT NOT NULL,
            rule_id TEXT,
            violation_id TEXT,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS memories(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id TEXT NOT NULL,
            content TEXT NOT NULL,
            tier TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS project_ecosystem(
            name TEXT NOT NULL,
            ecosystem TEXT NOT NULL,
            declared_version TEXT NOT NULL,
            resolved_version TEXT,
            is_dev INTEGER NOT NULL DEFAULT 0,
            source_manifest TEXT NOT NULL,
            PRIMARY KEY(name, ecosystem)
        );
        CREATE INDEX IF NOT EXISTS relationships_src ON relationships(source_id);
        CREATE INDEX IF NOT EXISTS relationships_tgt ON relationships(target);
        CREATE INDEX IF NOT EXISTS relationships_path ON relationships(path);
        CREATE TABLE IF NOT EXISTS metadata(
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );"
    )?;

    for (table, column, definition) in [
        ("files", "analysis_key", "TEXT NOT NULL DEFAULT ''"),
        ("revisions", "analysis_key", "TEXT NOT NULL DEFAULT ''"),
        ("decisions", "rule_id", "TEXT"),
        ("decisions", "violation_id", "TEXT"),
    ] {
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let columns: Vec<String> = stmt
            .query_map([], |row| row.get(1))?
            .collect::<rusqlite::Result<_>>()?;
        if !columns.iter().any(|name| name == column) {
            conn.execute_batch(&format!(
                "ALTER TABLE {table} ADD COLUMN {column} {definition}"
            ))?;
        }
    }

    conn.execute_batch(
        "INSERT OR IGNORE INTO entity_identity(entity_id, kind, created_at_revision) 
         SELECT entity_id, kind, created_at_revision FROM entities;
         INSERT OR IGNORE INTO entity_versions(version_id, entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, invalidated_at_revision, status)
         SELECT lower(hex(randomblob(16))), entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, invalidated_at_revision, status FROM entities;"
    )?;

    Ok(conn)
}

pub fn git(root: &Path, args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim_end().to_string())
        .unwrap_or_default()
}

pub fn revision(root: &Path) -> String {
    let head = git(root, &["rev-parse", "HEAD"]);
    let head = if head.is_empty() { "WORKTREE" } else { &head };
    let status = git(root, &["status", "--porcelain=v1", "-uall"]);
    if status.is_empty() {
        return head.to_string();
    }
    let mut snapshot = status.clone();
    for line in status.lines() {
        let trimmed_line = line.trim_start();
        let path = if trimmed_line.len() > 2 {
            trimmed_line[2..].trim().trim_matches('"')
        } else {
            ""
        };
        if !path.is_empty() {
            #[allow(clippy::collapsible_if)]
            if let Ok(bytes) = fs::read(root.join(path)) {
                snapshot.push_str(&hash(&bytes));
            }
        }
    }
    format!("{head}+dirty:{}", &hash(snapshot.as_bytes())[..16])
}

pub fn config_hash(root: &Path) -> String {
    let mut config = Vec::new();
    for name in [
        ".engram/config.yaml",
        ".engram/constitution.yaml",
        ".engramignore",
    ] {
        config.extend_from_slice(name.as_bytes());
        if let Ok(bytes) = fs::read(root.join(name)) {
            config.extend_from_slice(&bytes);
        }
        config.push(0);
    }
    hash(&config)
}

pub fn git_changed(root: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "-z", "-uall"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| o.stdout)
        .unwrap_or_default();

    let mut changed = Vec::new();
    for entry in output.split(|&b| b == 0) {
        if entry.len() < 3 {
            continue;
        }
        // In porcelain v1, first 2 bytes are status (e.g. ' M', 'M ', '??'), followed by space
        let path_bytes = if entry[2] == b' ' {
            &entry[3..]
        } else {
            &entry[2..]
        };
        let path = String::from_utf8_lossy(path_bytes)
            .trim()
            .trim_matches('"')
            .replace('\\', "/");
        let clean = path.trim_start_matches("./").trim_end_matches('/');
        if clean == ".engram"
            || clean.starts_with(".engram/")
            || clean == ".git"
            || clean.starts_with(".git/")
        {
            continue;
        }
        if !path.is_empty() {
            changed.push(path);
        }
    }
    changed
}
