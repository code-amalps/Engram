use anyhow::{Context, Result};
use chrono::Utc;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

pub const CURRENT_SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ParseStatus {
    Complete,
    Partial,
    NotAvailable,
    Error,
}

impl ParseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseStatus::Complete => "COMPLETE",
            ParseStatus::Partial => "PARTIAL",
            ParseStatus::NotAvailable => "NOT_AVAILABLE",
            ParseStatus::Error => "ERROR",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "PARTIAL" => ParseStatus::Partial,
            "NOT_AVAILABLE" => ParseStatus::NotAvailable,
            "ERROR" => ParseStatus::Error,
            _ => ParseStatus::Complete,
        }
    }
}

impl std::fmt::Display for ParseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EntityMember {
    pub member_id: String,
    pub entity_id: String,
    pub name: String,
    pub member_kind: String,
    pub signature: String,
    pub return_type: Option<String>,
    pub start_line: usize,
    pub end_line: usize,
    pub visibility: String,
    pub is_async: bool,
    pub is_static: bool,
    pub decorators: Option<String>,
}

impl EntityMember {
    pub fn new(
        entity_id: impl Into<String>,
        name: impl Into<String>,
        member_kind: impl Into<String>,
        signature: impl Into<String>,
        start_line: usize,
        end_line: usize,
    ) -> Self {
        let entity_id = entity_id.into();
        let name = name.into();
        let signature = signature.into();
        let member_id = member_id(&entity_id, &name, &signature);
        Self {
            member_id,
            entity_id,
            name,
            member_kind: member_kind.into(),
            signature,
            return_type: None,
            start_line,
            end_line,
            visibility: "public".to_string(),
            is_async: false,
            is_static: false,
            decorators: None,
        }
    }
}

pub fn member_id(entity_id: &str, name: &str, signature: &str) -> String {
    let payload = format!("{entity_id}:{name}:{signature}");
    hash(payload.as_bytes())[..16].to_string()
}

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

pub fn get_schema_version(conn: &Connection) -> Result<u32> {
    let version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(version)
}

pub fn migrate_database(conn: &mut Connection) -> Result<()> {
    let version = get_schema_version(conn)?;
    if version >= CURRENT_SCHEMA_VERSION {
        return Ok(());
    }

    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    // Step 1: Ensure entities has parse_status and is_external
    {
        let mut stmt = tx.prepare("PRAGMA table_info(entities)")?;
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get(1))?
            .collect::<rusqlite::Result<_>>()?;
        if !cols.iter().any(|c| c == "parse_status") {
            tx.execute_batch(
                "ALTER TABLE entities ADD COLUMN parse_status TEXT NOT NULL DEFAULT 'COMPLETE';",
            )?;
        }
        if !cols.iter().any(|c| c == "is_external") {
            tx.execute_batch(
                "ALTER TABLE entities ADD COLUMN is_external INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
    }

    // Step 2: Ensure relationships has is_external
    {
        let mut stmt = tx.prepare("PRAGMA table_info(relationships)")?;
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get(1))?
            .collect::<rusqlite::Result<_>>()?;
        if !cols.iter().any(|c| c == "is_external") {
            tx.execute_batch(
                "ALTER TABLE relationships ADD COLUMN is_external INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
    }

    // Step 3: Create entity_members table and indexes
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS entity_members (
            member_id TEXT PRIMARY KEY,
            entity_id TEXT NOT NULL,
            name TEXT NOT NULL,
            member_kind TEXT NOT NULL,
            signature TEXT NOT NULL,
            return_type TEXT,
            start_line INTEGER NOT NULL,
            end_line INTEGER NOT NULL,
            visibility TEXT NOT NULL DEFAULT 'public',
            is_async BOOLEAN NOT NULL DEFAULT 0,
            is_static BOOLEAN NOT NULL DEFAULT 0,
            decorators TEXT,
            FOREIGN KEY(entity_id) REFERENCES entities(entity_id) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_members_entity ON entity_members(entity_id);
        CREATE INDEX IF NOT EXISTS idx_members_entity_name ON entity_members(entity_id, name);
        CREATE INDEX IF NOT EXISTS idx_members_name ON entity_members(name);
        CREATE INDEX IF NOT EXISTS idx_relationships_ext ON relationships(source_id, is_external);",
    )?;

    tx.commit()?;

    conn.pragma_update(None, "user_version", CURRENT_SCHEMA_VERSION)?;

    Ok(())
}

pub fn init_db(conn: &mut Connection) -> Result<()> {
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
            status TEXT NOT NULL DEFAULT 'OBSERVED',
            parse_status TEXT NOT NULL DEFAULT 'COMPLETE',
            is_external INTEGER NOT NULL DEFAULT 0
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
            invalidated_at_revision TEXT,
            is_external INTEGER NOT NULL DEFAULT 0
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

    conn.execute(
        "INSERT OR IGNORE INTO metadata(key, value) VALUES('index_generation', '1')",
        [],
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

    migrate_database(conn)?;

    conn.execute_batch(
        "INSERT OR IGNORE INTO entity_identity(entity_id, kind, created_at_revision)
         SELECT entity_id, kind, created_at_revision FROM entities;
         INSERT OR IGNORE INTO entity_versions(version_id, entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, invalidated_at_revision, status)
         SELECT lower(hex(randomblob(16))), entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, invalidated_at_revision, status FROM entities;"
    )?;

    Ok(())
}

pub fn db(root: &Path) -> Result<Connection> {
    let dir = cache_dir(root)?;
    fs::create_dir_all(&dir)?;
    let mut conn = Connection::open(dir.join("derived_symbols.db"))?;
    conn.busy_timeout(std::time::Duration::from_secs(15))?;
    init_db(&mut conn)?;
    Ok(conn)
}

pub fn db_at_path(db_file: &Path) -> Result<Connection> {
    if let Some(parent) = db_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut conn = Connection::open(db_file)?;
    conn.busy_timeout(std::time::Duration::from_secs(15))?;
    init_db(&mut conn)?;
    Ok(conn)
}

pub fn insert_entity_members(conn: &Connection, members: &[EntityMember]) -> Result<usize> {
    if members.is_empty() {
        return Ok(0);
    }
    use rusqlite::OptionalExtension;

    let mut check_stmt =
        conn.prepare("SELECT entity_id, name, signature FROM entity_members WHERE member_id = ?1")?;

    let mut insert_stmt = conn.prepare(
        "INSERT INTO entity_members (
            member_id, entity_id, name, member_kind, signature, return_type,
            start_line, end_line, visibility, is_async, is_static, decorators
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
    )?;

    let mut update_stmt = conn.prepare(
        "UPDATE entity_members SET
            entity_id = ?2, name = ?3, member_kind = ?4, signature = ?5, return_type = ?6,
            start_line = ?7, end_line = ?8, visibility = ?9, is_async = ?10, is_static = ?11,
            decorators = ?12
         WHERE member_id = ?1",
    )?;

    let mut count = 0;
    for m in members {
        let existing: Option<(String, String, String)> = check_stmt
            .query_row([&m.member_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .optional()?;

        match existing {
            None => {
                insert_stmt.execute(rusqlite::params![
                    m.member_id,
                    m.entity_id,
                    m.name,
                    m.member_kind,
                    m.signature,
                    m.return_type,
                    m.start_line as i64,
                    m.end_line as i64,
                    m.visibility,
                    m.is_async as i64,
                    m.is_static as i64,
                    m.decorators,
                ])?;
                count += 1;
            }
            Some((ref ex_entity, ref ex_name, ref ex_sig)) => {
                if ex_entity == &m.entity_id && ex_name == &m.name && ex_sig == &m.signature {
                    update_stmt.execute(rusqlite::params![
                        m.member_id,
                        m.entity_id,
                        m.name,
                        m.member_kind,
                        m.signature,
                        m.return_type,
                        m.start_line as i64,
                        m.end_line as i64,
                        m.visibility,
                        m.is_async as i64,
                        m.is_static as i64,
                        m.decorators,
                    ])?;
                    count += 1;
                } else {
                    anyhow::bail!(
                        "Hash collision detected in entity_members for member_id '{}'. Existing member ({}, {}, {}) cannot be overwritten by ({}, {}, {})",
                        m.member_id,
                        ex_entity,
                        ex_name,
                        ex_sig,
                        m.entity_id,
                        m.name,
                        m.signature
                    );
                }
            }
        }
    }
    Ok(count)
}

pub fn get_entity_members(conn: &Connection, entity_id: &str) -> Result<Vec<EntityMember>> {
    let mut stmt = conn.prepare(
        "SELECT member_id, entity_id, name, member_kind, signature, return_type,
                start_line, end_line, visibility, is_async, is_static, decorators
         FROM entity_members
         WHERE entity_id = ?1
         ORDER BY start_line ASC, name ASC",
    )?;

    let rows = stmt.query_map([entity_id], |row| {
        Ok(EntityMember {
            member_id: row.get(0)?,
            entity_id: row.get(1)?,
            name: row.get(2)?,
            member_kind: row.get(3)?,
            signature: row.get(4)?,
            return_type: row.get(5)?,
            start_line: row.get::<_, i64>(6)? as usize,
            end_line: row.get::<_, i64>(7)? as usize,
            visibility: row.get(8)?,
            is_async: row.get::<_, i64>(9)? != 0,
            is_static: row.get::<_, i64>(10)? != 0,
            decorators: row.get(11)?,
        })
    })?;

    let mut results = Vec::new();
    for r in rows {
        results.push(r?);
    }
    Ok(results)
}

pub fn get_entity_members_by_name(
    conn: &Connection,
    entity_id: &str,
    member_name: &str,
) -> Result<Vec<EntityMember>> {
    let mut stmt = conn.prepare(
        "SELECT member_id, entity_id, name, member_kind, signature, return_type,
                start_line, end_line, visibility, is_async, is_static, decorators
         FROM entity_members
         WHERE entity_id = ?1 AND name = ?2
         ORDER BY start_line ASC",
    )?;

    let rows = stmt.query_map(rusqlite::params![entity_id, member_name], |row| {
        Ok(EntityMember {
            member_id: row.get(0)?,
            entity_id: row.get(1)?,
            name: row.get(2)?,
            member_kind: row.get(3)?,
            signature: row.get(4)?,
            return_type: row.get(5)?,
            start_line: row.get::<_, i64>(6)? as usize,
            end_line: row.get::<_, i64>(7)? as usize,
            visibility: row.get(8)?,
            is_async: row.get::<_, i64>(9)? != 0,
            is_static: row.get::<_, i64>(10)? != 0,
            decorators: row.get(11)?,
        })
    })?;

    let mut results = Vec::new();
    for r in rows {
        results.push(r?);
    }
    Ok(results)
}

pub fn count_entity_members(conn: &Connection, entity_id: &str) -> Result<usize> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM entity_members WHERE entity_id = ?1",
        [entity_id],
        |row| row.get(0),
    )?;
    Ok(count as usize)
}

pub fn update_entity_parse_status(
    conn: &Connection,
    entity_id: &str,
    status: ParseStatus,
) -> Result<()> {
    conn.execute(
        "UPDATE entities SET parse_status = ?1 WHERE entity_id = ?2",
        rusqlite::params![status.as_str(), entity_id],
    )?;
    Ok(())
}

pub fn get_entity_parse_status(conn: &Connection, entity_id: &str) -> Result<ParseStatus> {
    let status_str: String = conn.query_row(
        "SELECT parse_status FROM entities WHERE entity_id = ?1",
        [entity_id],
        |row| row.get(0),
    )?;
    Ok(ParseStatus::from_str(&status_str))
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

/// Record of an authorized trivial task bypass.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct BypassTrivialRecord {
    pub task: String,
    pub reason: String,
    pub target_files: Vec<String>,
    pub created_at: i64,
}

/// Saves a trivial task bypass marker into .engram/bypass_trivial.json.
pub fn save_bypass_trivial(
    root: &Path,
    task: &str,
    reason: &str,
    target_files: &[String],
) -> Result<()> {
    let root = root_path(root)?;
    let engram_dir = root.join(".engram");
    if !engram_dir.exists() {
        fs::create_dir_all(&engram_dir)?;
    }
    let record = BypassTrivialRecord {
        task: task.to_string(),
        reason: reason.to_string(),
        target_files: target_files.to_vec(),
        created_at: chrono::Utc::now().timestamp(),
    };
    let data = serde_json::to_string_pretty(&record)?;
    fs::write(engram_dir.join("bypass_trivial.json"), data)?;
    Ok(())
}

/// Loads a trivial task bypass marker from .engram/bypass_trivial.json if present.
pub fn load_bypass_trivial(root: &Path) -> Result<Option<BypassTrivialRecord>> {
    let root = root_path(root)?;
    let file = root.join(".engram").join("bypass_trivial.json");
    if !file.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&file)?;
    let record: BypassTrivialRecord = serde_json::from_str(&content)?;
    Ok(Some(record))
}

/// Removes any trivial task bypass marker from .engram/bypass_trivial.json.
pub fn remove_bypass_trivial(root: &Path) -> Result<()> {
    let root = root_path(root)?;
    let file = root.join(".engram").join("bypass_trivial.json");
    if file.exists() {
        let _ = fs::remove_file(file);
    }
    Ok(())
}

/// Checks whether all staged changes consist exclusively of non-code documentation
/// or project configuration files (e.g. .md, docs/*, .txt, .gitignore, LICENSE).
pub fn is_pure_documentation_change(changed_files: &[String]) -> bool {
    if changed_files.is_empty() {
        return false;
    }
    changed_files.iter().all(|f| {
        let lower = f.to_lowercase();
        let path = Path::new(&lower);
        lower.ends_with(".md")
            || lower.ends_with(".txt")
            || lower.ends_with(".rst")
            || lower.ends_with(".adoc")
            || lower.starts_with("docs/")
            || lower.starts_with("docs\\")
            || lower.contains("/docs/")
            || lower.contains("\\docs\\")
            || path
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n == ".gitignore" || n == "license" || n == "readme" || n == "notice")
                .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_member_id_deterministic() {
        let id1 = member_id(
            "entity_1",
            "setItem",
            "setItem(key: string, value: any): void",
        );
        let id2 = member_id(
            "entity_1",
            "setItem",
            "setItem(key: string, value: any): void",
        );
        let id3 = member_id("entity_1", "getItem", "getItem<T>(key: string): T | null");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn test_parse_status_conversion() {
        assert_eq!(ParseStatus::Complete.as_str(), "COMPLETE");
        assert_eq!(ParseStatus::Partial.as_str(), "PARTIAL");
        assert_eq!(ParseStatus::NotAvailable.as_str(), "NOT_AVAILABLE");
        assert_eq!(ParseStatus::Error.as_str(), "ERROR");

        assert_eq!(ParseStatus::from_str("COMPLETE"), ParseStatus::Complete);
        assert_eq!(ParseStatus::from_str("PARTIAL"), ParseStatus::Partial);
        assert_eq!(
            ParseStatus::from_str("NOT_AVAILABLE"),
            ParseStatus::NotAvailable
        );
        assert_eq!(ParseStatus::from_str("ERROR"), ParseStatus::Error);
        assert_eq!(ParseStatus::from_str("UNKNOWN"), ParseStatus::Complete);
    }
}
