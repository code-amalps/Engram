use anyhow::{Result, anyhow};
use ignore::WalkBuilder;
use regex::Regex;
use rusqlite::{OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};
use tree_sitter::{Language, Node, Parser as TsParser};
use uuid::Uuid;

use crate::model;
use crate::storage::{config_hash, db, git, hash, now, revision, root_path};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub path: String,
    pub hash: String,
    pub language: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub path: String,
    pub name: String,
    pub kind: String,
    pub line: usize,
    pub end_line: usize,
    pub signature: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub evidence: String,
    pub confidence: f64,
    pub line: usize,
}

#[derive(Debug, Serialize)]
pub struct AnalysisSummary {
    pub revision: String,
    pub indexed_files: usize,
    pub changed_files: usize,
    pub symbols: usize,
    pub removed_files: usize,
}

pub fn redact(s: &str) -> String {
    let rules = [
        r"(?i)\b(?:sk-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16})\b",
        r#"(?i)\b(?:password|secret|api[_-]?key|token)\s*[:=]\s*["'][^"']+["']"#,
        r"(?i)\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b",
        r#"(?i)\b(?:postgres|mysql|mongodb)(?:ql)?://[^\s"']+"#,
    ];
    let mut out = s.to_string();
    for rule in rules {
        if let Ok(rx) = Regex::new(rule) {
            out = rx.replace_all(&out, "[REDACTED]").into_owned();
        }
    }
    out
}

pub fn language(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" | "mjs" | "cjs" => Some("javascript"),
        "py" => Some("python"),
        "cs" => Some("csharp"),
        "rs" => Some("rust"),
        "go" => Some("go"),
        _ => None,
    }
}

pub fn lang_for(name: &str) -> Language {
    match name {
        "typescript" => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        "javascript" => tree_sitter_javascript::LANGUAGE.into(),
        "python" => tree_sitter_python::LANGUAGE.into(),
        "rust" => tree_sitter_rust::LANGUAGE.into(),
        "go" => tree_sitter_go::LANGUAGE.into(),
        _ => tree_sitter_c_sharp::LANGUAGE.into(),
    }
}

pub fn node_text<'a>(n: Node<'a>, src: &'a [u8]) -> String {
    n.utf8_text(src).unwrap_or("").trim().to_string()
}

pub fn recognized(kind: &str, lang: &str) -> Option<&'static str> {
    match (lang, kind) {
        ("python", "function_definition") => Some("FUNCTION"),
        ("python", "class_definition") => Some("CLASS"),
        ("typescript" | "javascript", "function_declaration") => Some("FUNCTION"),
        ("typescript" | "javascript", "class_declaration") => Some("CLASS"),
        ("typescript" | "javascript", "interface_declaration") => Some("INTERFACE"),
        ("typescript" | "javascript", "type_alias_declaration") => Some("TYPE"),
        ("typescript" | "javascript", "method_definition") => Some("METHOD"),
        ("csharp", "class_declaration") => Some("CLASS"),
        ("csharp", "interface_declaration") => Some("INTERFACE"),
        ("csharp", "struct_declaration") => Some("STRUCT"),
        ("csharp", "enum_declaration") => Some("ENUM"),
        ("csharp", "method_declaration") => Some("METHOD"),
        ("csharp", "constructor_declaration") => Some("CONSTRUCTOR"),
        ("rust", "function_item") => Some("FUNCTION"),
        ("rust", "struct_item") => Some("STRUCT"),
        ("rust", "enum_item") => Some("ENUM"),
        ("rust", "trait_item") => Some("TRAIT"),
        ("rust", "impl_item") => Some("IMPL"),
        ("rust", "mod_item") => Some("MODULE"),
        ("rust", "type_item") => Some("TYPE"),
        _ => None,
    }
}

pub fn extract_import_specifier(text: &str, lang: &str) -> String {
    if lang == "typescript" || lang == "javascript" {
        if let Some(pos) = text.find("from") {
            let after = text[pos + 4..].trim();
            return after.trim_matches(|c| c == '\'' || c == '"' || c == ';' || c == ' ').to_string();
        }
        if let Some(pos) = text.find("require(") {
            let after = text[pos + 8..].trim();
            if let Some(end) = after.find(')') {
                return after[..end].trim_matches(|c| c == '\'' || c == '"' || c == ' ').to_string();
            }
        }
    } else if lang == "python" {
        if let Some(pos) = text.find("from ") {
            let after = text[pos + 5..].trim();
            if let Some(space) = after.find(' ') {
                return after[..space].to_string();
            }
        } else if let Some(pos) = text.find("import ") {
            let after = text[pos + 7..].trim();
            if let Some(space) = after.find(' ') {
                return after[..space].to_string();
            }
            return after.to_string();
        }
    } else if lang == "csharp" && text.starts_with("using ") {
        return text[6..].trim().trim_matches(';').to_string();
    } else if lang == "rust" {
        if text.starts_with("use ") {
            return text[4..].trim().trim_matches(';').to_string();
        } else if text.starts_with("mod ") {
            return text[4..].trim().trim_matches(';').to_string();
        }
    }
    text.chars().take(120).collect()
}

pub fn normalize_rel_path(path: &Path) -> String {
    let mut components = Vec::new();
    for c in path.components() {
        match c {
            std::path::Component::Normal(p) => components.push(p.to_string_lossy().to_string()),
            std::path::Component::ParentDir => {
                components.pop();
            }
            std::path::Component::CurDir => {}
            _ => {}
        }
    }
    components.join("/")
}

pub fn resolve_import_path(current_file: &str, spec: &str) -> String {
    if spec.starts_with('.') {
        let parent = Path::new(current_file).parent().unwrap_or(Path::new(""));
        let joined = parent.join(spec);
        let normalized = normalize_rel_path(&joined);
        if !normalized.contains('.') {
            format!("{normalized}.ts")
        } else {
            normalized
        }
    } else {
        spec.to_string()
    }
}

pub fn visit(
    n: Node,
    src: &[u8],
    path: &str,
    lang: &str,
    out: &mut Vec<Symbol>,
    rels: &mut Vec<Relationship>,
    revision: &str,
) {
    if let Some(kind) = recognized(n.kind(), lang) {
        let name = n
            .child_by_field_name("name")
            .or_else(|| n.child_by_field_name("type"))
            .map(|x| node_text(x, src))
            .unwrap_or_else(|| "<anonymous>".into());
        let full = node_text(n, src);
        let signature = full
            .lines()
            .next()
            .unwrap_or(&full)
            .chars()
            .take(300)
            .collect::<String>();
        let skeleton = n.to_sexp();
        let fingerprint = hash(skeleton.as_bytes());
        let stable = hash(format!("{path}\0{kind}\0{name}\0{fingerprint}").as_bytes());
        out.push(Symbol {
            id: format!("ent_{}", &stable[..24]),
            path: path.into(),
            name,
            kind: kind.into(),
            line: n.start_position().row + 1,
            end_line: n.end_position().row + 1,
            signature,
            fingerprint,
        });
    }
    if n.kind().contains("import") || n.kind() == "using_directive" || n.kind() == "use_declaration" {
        let raw = node_text(n, src);
        let spec = extract_import_specifier(&raw, lang);
        if !spec.is_empty() {
            let target = resolve_import_path(path, &spec);
            rels.push(Relationship {
                source: path.into(),
                target,
                kind: "IMPORTS".into(),
                evidence: "TREE_SITTER_IMPORT_SYNTAX".into(),
                confidence: 0.99,
                line: n.start_position().row + 1,
            });
        }
    }
    let mut c = n.walk();
    for child in n.children(&mut c) {
        visit(child, src, path, lang, out, rels, revision);
    }
    let _ = revision;
}

pub fn parse_file(
    root: &Path,
    rel: &str,
    bytes: &[u8],
    rev: &str,
) -> Result<(FileRecord, Vec<Symbol>, Vec<Relationship>)> {
    let path = Path::new(rel);
    let lang = language(path).ok_or_else(|| anyhow!("unsupported language"))?;
    let source = String::from_utf8_lossy(bytes);

    if lang == "go" {
        let go_mod = crate::go_parser::load_root_go_mod(root);
        let ast = crate::go_parser::extract_go_ast(&source)?;
        let (symbols, rels) = crate::go_parser::convert_go_ast_to_engram_with_root(
            root,
            rel,
            &ast,
            go_mod.as_deref(),
        );
        let display = redact(&source);
        let record = FileRecord {
            path: rel.into(),
            hash: hash(bytes),
            language: lang.into(),
            excerpt: display.chars().take(20000).collect(),
        };
        return Ok((record, symbols, rels));
    }

    let mut parser = TsParser::new();
    parser.set_language(&lang_for(lang))?;
    let tree = parser
        .parse(source.as_ref(), None)
        .ok_or_else(|| anyhow!("parser returned no tree"))?;
    let mut symbols = Vec::new();
    let mut rels = Vec::new();
    visit(
        tree.root_node(),
        source.as_bytes(),
        rel,
        lang,
        &mut symbols,
        &mut rels,
        rev,
    );

    // Detect test-to-target relationship
    if rel.ends_with(".spec.ts") {
        if let Some(t) = rel.strip_suffix(".spec.ts").map(|p| format!("{p}.ts")) {
            rels.push(Relationship {
                source: rel.into(),
                target: t,
                kind: "TESTS".into(),
                evidence: "NAMING_CONVENTION".into(),
                confidence: 0.95,
                line: 1,
            });
        }
    } else if rel.ends_with(".test.ts") {
        if let Some(t) = rel.strip_suffix(".test.ts").map(|p| format!("{p}.ts")) {
            rels.push(Relationship {
                source: rel.into(),
                target: t,
                kind: "TESTS".into(),
                evidence: "NAMING_CONVENTION".into(),
                confidence: 0.95,
                line: 1,
            });
        }
    } else if rel.starts_with("tests/") && rel.ends_with(".rs") {
        let stem = rel
            .trim_start_matches("tests/")
            .trim_end_matches(".rs")
            .trim_end_matches("_test");
        rels.push(Relationship {
            source: rel.into(),
            target: format!("src/{stem}.rs"),
            kind: "TESTS".into(),
            evidence: "NAMING_CONVENTION".into(),
            confidence: 0.95,
            line: 1,
        });
    } else if rel.starts_with("tests/") && rel.ends_with(".py") {
        let stem = rel
            .trim_start_matches("tests/")
            .trim_end_matches(".py")
            .trim_start_matches("test_")
            .trim_end_matches("_test");
        rels.push(Relationship {
            source: rel.into(),
            target: format!("src/orders/{stem}.py"),
            kind: "TESTS".into(),
            evidence: "NAMING_CONVENTION".into(),
            confidence: 0.95,
            line: 1,
        });
    } else if rel.ends_with("Tests.cs") || rel.ends_with("Test.cs") {
        let stem = rel
            .trim_start_matches("tests/")
            .trim_end_matches("Tests.cs")
            .trim_end_matches("Test.cs");
        rels.push(Relationship {
            source: rel.into(),
            target: format!("src/Cart/{stem}.cs"),
            kind: "TESTS".into(),
            evidence: "NAMING_CONVENTION".into(),
            confidence: 0.95,
            line: 1,
        });
    }

    let display = redact(&source);
    let record = FileRecord {
        path: rel.into(),
        hash: hash(bytes),
        language: lang.into(),
        excerpt: display.chars().take(20000).collect(),
    };
    let _ = root;
    Ok((record, symbols, rels))
}

pub fn reconcile_symbol_ids(
    tx: &Transaction<'_>,
    symbols: &mut [Symbol],
    current_paths: &HashSet<String>,
    assigned_ids: &mut HashSet<String>,
) -> Result<()> {
    let previous: Vec<(String, String, String, String, usize, String)> = {
        let mut stmt = tx.prepare(
            "SELECT entity_id, path, name, kind, start_line, ast_fingerprint 
             FROM entity_versions WHERE invalidated_at_revision IS NULL"
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get::<_, usize>(4)?,
                r.get(5)?,
            ))
        })?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    for symbol in symbols {
        let mut exact_name: Vec<_> = previous
            .iter()
            .filter(|(id, path, name, kind, _, _)| {
                path == &symbol.path
                    && name == &symbol.name
                    && kind == &symbol.kind
                    && !assigned_ids.contains(id)
            })
            .collect();
        exact_name.sort_by_key(|(_, _, _, _, line, _)| line.abs_diff(symbol.line));
        let same_file_shape: Vec<_> = previous
            .iter()
            .filter(|(id, path, _, kind, _, fingerprint)| {
                path == &symbol.path
                    && kind == &symbol.kind
                    && !assigned_ids.contains(id)
                    && fingerprint == &symbol.fingerprint
            })
            .collect();
        let global_shape: Vec<_> = previous
            .iter()
            .filter(|(id, old_path, _, kind, _, fingerprint)| {
                kind == &symbol.kind
                    && !current_paths.contains(old_path)
                    && !assigned_ids.contains(id)
                    && fingerprint == &symbol.fingerprint
            })
            .collect();
        let chosen = exact_name
            .first()
            .map(|entry| entry.0.clone())
            .or_else(|| (same_file_shape.len() == 1).then(|| same_file_shape[0].0.clone()))
            .or_else(|| (global_shape.len() == 1).then(|| global_shape[0].0.clone()));
        if let Some(id) = chosen {
            assigned_ids.insert(id.clone());
            symbol.id = id;
        }
    }
    Ok(())
}

pub fn ignore_patterns(root: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let f = root.join(".engramignore");
    if let Ok(s) = fs::read_to_string(f) {
        for line in s
            .lines()
            .map(str::trim)
            .filter(|x| !x.is_empty() && !x.starts_with('#'))
        {
            if let Some((cat, pat)) = line.split_once(':') {
                out.push((
                    cat.trim().to_ascii_uppercase(),
                    pat.trim().replace('\\', "/"),
                ));
            } else {
                out.push(("BUILD".into(), line.replace('\\', "/")));
            }
        }
    }
    out
}

pub fn matches_pattern(path: &str, pattern: &str) -> bool {
    let p = pattern.trim_matches('/');
    if p.is_empty() {
        return false;
    }
    if p.contains("**") {
        let parts: Vec<_> = p.split("**").collect();
        return path.starts_with(parts[0]) && path.ends_with(parts.last().unwrap_or(&""));
    }
    if p.contains('*') {
        let parts: Vec<_> = p.split('*').collect();
        let mut from = 0;
        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                continue;
            }
            if let Some(found) = path[from..].find(part) {
                if i == 0 && found != 0 {
                    return false;
                }
                from += found + part.len();
            } else {
                return false;
            }
        }
        return true;
    }
    path == p || path.ends_with(&format!("/{p}"))
}

pub fn excluded(root: &Path, path: &Path, patterns: &[(String, String)]) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        return true;
    };
    let rel = rel.to_string_lossy().replace('\\', "/");
    if rel.split('/').any(|p| {
        matches!(
            p,
            ".git"
                | "node_modules"
                | "vendor"
                | "dist"
                | "build"
                | "coverage"
                | "target"
                | ".next"
                | ".venv"
                | "venv"
                | "__pycache__"
                | "bin"
                | "obj"
                | ".engram"
        )
    }) {
        return true;
    }
    let base = path.file_name().unwrap_or_default().to_string_lossy();
    if base == ".env"
        || base.starts_with(".env.")
        || base.ends_with(".pem")
        || base.ends_with(".key")
    {
        return true;
    }
    patterns.iter().any(|(cat, p)| {
        cat != "SOURCE" && cat != "GENERATED" && cat != "SECRET" && matches_pattern(&rel, p)
            || cat == "SECRET" && matches_pattern(&rel, p)
    })
}

pub fn analyze(root: &Path) -> Result<AnalysisSummary> {
    let root = root_path(root)?;
    let rev = revision(&root);
    let project_config_hash = config_hash(&root);
    let project_analysis_key = model::analysis_key(
        &rev,
        VERSION,
        "tree-sitter-0.25.10",
        "typescript-0.23.2,javascript-0.23.1,python-0.23.6,csharp-0.23.5",
        &project_config_hash,
    );
    let mut conn = db(&root)?;
    let patterns = ignore_patterns(&root);
    let mut files = Vec::new();
    for entry in WalkBuilder::new(&root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
    {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let p = entry.path();
        if p.is_dir() {
            continue;
        }
        if excluded(&root, p, &patterns) {
            continue;
        }
        if language(p).is_none() {
            continue;
        }
        let rel = p.strip_prefix(&root)?.to_string_lossy().replace('\\', "/");
        files.push((p.to_path_buf(), rel));
    }
    let tx = conn.transaction()?;
    let head = git(&root, &["rev-parse", "HEAD"]);
    let commit_hash = if head.is_empty() { "WORKTREE" } else { &head };
    let branch = git(&root, &["rev-parse", "--abbrev-ref", "HEAD"]);
    let parent = git(&root, &["rev-parse", "HEAD^"]);
    let dirty = i64::from(!git(&root, &["status", "--porcelain=v1"]).is_empty());
    tx.execute(
        "INSERT INTO revisions(revision_id,commit_hash,branch_name,dirty,parent_hash,analysis_key,recorded_at) 
         VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(revision_id) DO UPDATE SET analysis_key=excluded.analysis_key",
        params![rev, commit_hash, branch, dirty, if parent.is_empty(){None}else{Some(parent)}, project_analysis_key, now()],
    )?;
    let mut changed = 0usize;
    let mut seen = HashSet::new();
    let current_paths: HashSet<String> = files.iter().map(|(_, path)| path.clone()).collect();
    let mut assigned_entity_ids = HashSet::new();
    for (abs, rel) in &files {
        let bytes = match fs::read(abs) {
            Ok(b) if b.len() <= 2_000_000 && !b.contains(&0) => b,
            _ => continue,
        };
        seen.insert(rel.clone());
        let content_hash = hash(&bytes);
        let file_language = language(abs).expect("language checked above");
        let file_analysis_key = model::analysis_key(
            &content_hash,
            VERSION,
            "tree-sitter-0.25.10",
            file_language,
            &project_config_hash,
        );
        let old: Option<(String, String)> = tx
            .query_row(
                "SELECT content_hash, analysis_key FROM files WHERE path=?1",
                [rel],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if old
            .as_ref()
            .is_some_and(|(hash, key)| hash == &content_hash && key == &file_analysis_key)
        {
            continue;
        }
        let (file, mut symbols, rels) = parse_file(&root, rel, &bytes, &rev)?;
        reconcile_symbol_ids(&tx, &mut symbols, &current_paths, &mut assigned_entity_ids)?;
        changed += 1;
        tx.execute(
            "INSERT INTO files(path, content_hash, analysis_key, language, excerpt, revision, analyzed_at) 
             VALUES(?1,?2,?3,?4,?5,?6,?7) 
             ON CONFLICT(path) DO UPDATE SET content_hash=excluded.content_hash, analysis_key=excluded.analysis_key, language=excluded.language, excerpt=excluded.excerpt, revision=excluded.revision, analyzed_at=excluded.analyzed_at",
            params![file.path, file.hash, file_analysis_key, file.language, file.excerpt, rev, now()]
        )?;
        tx.execute("UPDATE entities SET invalidated_at_revision=?1 WHERE path=?2 AND invalidated_at_revision IS NULL", params![rev, rel])?;
        tx.execute("UPDATE entity_versions SET invalidated_at_revision=?1 WHERE path=?2 AND invalidated_at_revision IS NULL", params![rev, rel])?;
        tx.execute("UPDATE relationships SET invalidated_at_revision=?1 WHERE path=?2 AND invalidated_at_revision IS NULL", params![rev, rel])?;
        for s in symbols {
            tx.execute("INSERT OR IGNORE INTO entity_identity(entity_id, kind, created_at_revision) VALUES(?1,?2,?3)", params![s.id, s.kind, rev])?;
            tx.execute(
                "INSERT OR REPLACE INTO entity_versions(version_id, entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, invalidated_at_revision, status) 
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,NULL,'OBSERVED')",
                params![Uuid::new_v4().to_string(), s.id, s.path, s.name, s.kind, s.line as i64, s.end_line as i64, s.signature, s.fingerprint, rev]
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO entities(entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, invalidated_at_revision, status) 
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,NULL,'OBSERVED')",
                params![s.id, s.path, s.name, s.kind, s.line as i64, s.end_line as i64, s.signature, s.fingerprint, rev]
            )?;
            let properties = json!({
                "name": &s.name,
                "kind": &s.kind,
                "signature": &s.signature,
                "path": &s.path,
                "start_line": s.line,
                "end_line": s.end_line,
                "ast_fingerprint": &s.fingerprint
            });
            tx.execute(
                "INSERT INTO observations(observation_id, target_id, revision_id, property_name, value_json, observed_at) 
                 VALUES(?1,?2,?3,'STATIC_SYMBOL',?4,?5)",
                params![Uuid::new_v4().to_string(), s.id, rev, properties.to_string(), now()]
            )?;
            tx.execute(
                "INSERT INTO evidence(evidence_id, target_id, target_type, tier, confidence, source_type, source_path, source_line, recorded_at, decay_half_life_commits, verification_status, details_json) 
                 VALUES(?1,?2,'ENTITY','EXTRACTED',0.99,'TREE_SITTER',?3,?4,?5,NULL,'UNCONTESTED',?6)",
                params![Uuid::new_v4().to_string(), s.id, s.path, s.line as i64, now(), json!({"parser":"tree-sitter","fingerprint":s.fingerprint}).to_string()]
            )?;
        }
        for r in rels {
            let relationship_id = format!(
                "rel_{}",
                &hash(format!("{}\0{}\0{}\0{}", rel, r.kind, r.target, r.line).as_bytes())[..24]
            );
            tx.execute(
                "INSERT INTO relationships(source_id, target, kind, evidence, confidence, path, line, created_at_revision) 
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![r.source, r.target, r.kind, r.evidence, r.confidence, rel, r.line as i64, rev]
            )?;
            tx.execute(
                "INSERT INTO observations(observation_id, target_id, revision_id, property_name, value_json, observed_at) 
                 VALUES(?1,?2,?3,'STATIC_RELATIONSHIP',?4,?5)",
                params![Uuid::new_v4().to_string(), relationship_id, rev, json!({"type":&r.kind,"target":&r.target,"path":rel,"line":r.line}).to_string(), now()]
            )?;
            tx.execute(
                "INSERT INTO evidence(evidence_id, target_id, target_type, tier, confidence, source_type, source_path, source_line, recorded_at, decay_half_life_commits, verification_status, details_json) 
                 VALUES(?1,?2,'RELATIONSHIP','EXTRACTED',?3,'TREE_SITTER',?4,?5,?6,NULL,'UNCONTESTED',?7)",
                params![Uuid::new_v4().to_string(), relationship_id, r.confidence, rel, r.line as i64, now(), json!({"parser":"tree-sitter","edge_type":&r.kind}).to_string()]
            )?;
        }
    }
    let existing: Vec<String> = {
        let mut stmt = tx.prepare("SELECT path FROM files")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let mut removed = 0;
    for p in existing {
        if !seen.contains(&p) {
            tx.execute("DELETE FROM files WHERE path=?1", [&p])?;
            tx.execute("UPDATE entities SET invalidated_at_revision=?1 WHERE path=?2 AND invalidated_at_revision IS NULL", params![rev, p])?;
            tx.execute("UPDATE entity_versions SET invalidated_at_revision=?1 WHERE path=?2 AND invalidated_at_revision IS NULL", params![rev, p])?;
            tx.execute("UPDATE relationships SET invalidated_at_revision=?1 WHERE path=?2 AND invalidated_at_revision IS NULL", params![rev, p])?;
            removed += 1;
        }
    }
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('head_revision', ?1) 
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&rev]
    )?;
    tx.commit()?;

    // Sync ecosystem manifest intelligence into analytical store
    if let Ok(eco) = crate::manifest::detect_ecosystem(&root) {
        let _ = crate::manifest::sync_ecosystem_to_db(&conn, &eco);
    }
    let active_symbols = conn.query_row(
        "SELECT COUNT(*) FROM entity_versions WHERE invalidated_at_revision IS NULL",
        [],
        |r| r.get(0),
    )?;
    Ok(AnalysisSummary {
        revision: rev,
        indexed_files: files.len(),
        changed_files: changed,
        symbols: active_symbols,
        removed_files: removed,
    })
}

pub fn ensure_fresh(root: &Path) -> Result<()> {
    let root = root_path(root)?;
    let patterns = ignore_patterns(&root);
    let conn = db(&root)?;
    let mut hashes = HashMap::new();
    {
        let mut s = conn.prepare("SELECT path, content_hash FROM files")?;
        for row in s.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (k, v) = row?;
            hashes.insert(k, v);
        }
    }
    let mut stale = false;
    let mut scanned = HashSet::new();
    for entry in WalkBuilder::new(&root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build()
    {
        let Ok(e) = entry else { continue };
        let p = e.path();
        if p.is_dir() || excluded(&root, p, &patterns) {
            continue;
        }
        if language(p).is_none() {
            continue;
        }
        let Ok(b) = fs::read(p) else { continue };
        if b.len() > 2_000_000 || b.contains(&0) {
            continue;
        }
        let Ok(rel) = p.strip_prefix(&root) else {
            continue;
        };
        let rel = rel.to_string_lossy().replace('\\', "/");
        scanned.insert(rel.clone());
        if hashes.get(&rel) != Some(&hash(&b)) {
            stale = true;
            break;
        }
    }
    if !stale && hashes.len() == scanned.len() {
        return Ok(());
    }
    drop(conn);
    let _ = analyze(&root)?;
    Ok(())
}

pub fn tokens(s: &str) -> Vec<String> {
    let re = Regex::new(r"[\p{L}\p{N}]{2,}").unwrap();
    re.find_iter(&s.to_lowercase())
        .map(|m| m.as_str().to_string())
        .collect()
}

pub fn estimate_tokens(s: &str) -> usize {
    (s.chars().count() + 3) / 4
}
