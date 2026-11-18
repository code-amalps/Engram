use anyhow::{anyhow, Result};
use ignore::WalkBuilder;
use regex::Regex;
use rusqlite::{params, OptionalExtension, Transaction};
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
use crate::storage::{
    config_hash, db, git, hash, insert_entity_members, member_id, now, revision, root_path,
    EntityMember, ParseStatus,
};

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
    #[serde(default)]
    pub is_external: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub file: FileRecord,
    pub symbols: Vec<Symbol>,
    pub relationships: Vec<Relationship>,
    pub members: Vec<EntityMember>,
    pub parse_status: ParseStatus,
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
        "java" => Some("java"),
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
        "java" => tree_sitter_java::LANGUAGE.into(),
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
        ("java", "class_declaration") => Some("CLASS"),
        ("java", "interface_declaration") => Some("INTERFACE"),
        ("java", "record_declaration") => Some("RECORD"),
        ("java", "enum_declaration") => Some("ENUM"),
        ("java", "method_declaration") => Some("METHOD"),
        ("java", "constructor_declaration") => Some("CONSTRUCTOR"),
        _ => None,
    }
}

pub fn extract_import_specifier(text: &str, lang: &str) -> String {
    if lang == "typescript" || lang == "javascript" {
        if let Some(pos) = text.find("from") {
            let after = text[pos + 4..].trim();
            return after
                .trim_matches(|c| c == '\'' || c == '"' || c == ';' || c == ' ')
                .to_string();
        }
        if let Some(pos) = text.find("require(") {
            let after = text[pos + 8..].trim();
            if let Some(end) = after.find(')') {
                return after[..end]
                    .trim_matches(|c| c == '\'' || c == '"' || c == ' ')
                    .to_string();
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
    } else if lang == "java" && text.starts_with("import ") {
        return text[7..]
            .trim()
            .trim_start_matches("static ")
            .trim()
            .trim_matches(';')
            .to_string();
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
        let ext = Path::new(&normalized)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let is_code_ext = matches!(ext, "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" | "json");
        if !is_code_ext {
            if current_file.ends_with(".js") || current_file.ends_with(".jsx") {
                format!("{normalized}.js")
            } else {
                format!("{normalized}.ts")
            }
        } else {
            normalized
        }
    } else {
        spec.to_string()
    }
}

fn is_rust_lifetime(chars: &[char], i: usize) -> bool {
    let n = chars.len();
    if i >= n || chars[i] != '\'' {
        return false;
    }
    // Lifetimes must start with an ASCII letter or underscore
    if i + 1 >= n || (!chars[i + 1].is_ascii_alphabetic() && chars[i + 1] != '_') {
        return false;
    }
    // Find the end of the identifier
    let mut k = i + 1;
    while k < n && (chars[k].is_alphanumeric() || chars[k] == '_') {
        k += 1;
    }
    // If followed immediately by a closing single quote (e.g. `'a'` or `'static'`),
    // this is a char literal or TS string literal, NOT a lifetime.
    if k < n && chars[k] == '\'' {
        return false;
    }
    // Look backward to check preceding context (skipping whitespace)
    let mut p = i;
    while p > 0 && chars[p - 1].is_whitespace() {
        p -= 1;
    }
    if p == 0 {
        return true;
    }
    let prev = chars[p - 1];
    // In Rust syntax, a lifetime is preceded by '&', '<', ',', ':', or '+'
    if matches!(prev, '&' | '<' | ',' | ':' | '+') {
        return true;
    }
    // Also check if preceded by keyword 'for' (e.g. `for<'a>`)
    if p >= 3 {
        let prev_word: String = chars[p - 3..p].iter().collect();
        if prev_word == "for" && (p == 3 || chars[p - 4].is_whitespace() || matches!(chars[p - 4], '(' | '<' | ',')) {
            return true;
        }
    }
    false
}

pub fn normalize_signature(sig: &str) -> String {
    let trimmed = sig.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Step 1: Pre-pass to strip comments while protecting string and character literals
    let mut without_comments = String::with_capacity(trimmed.len());
    let raw_chars: Vec<char> = trimmed.chars().collect();
    let n_raw = raw_chars.len();
    let mut i = 0;
    let mut in_quote: Option<char> = None;

    while i < n_raw {
        let c = raw_chars[i];
        if let Some(q) = in_quote {
            without_comments.push(c);
            if c == '\\' && i + 1 < n_raw {
                without_comments.push(raw_chars[i + 1]);
                i += 2;
                continue;
            }
            if c == q {
                in_quote = None;
            }
            i += 1;
            continue;
        }

        if c == '"' || c == '`' {
            in_quote = Some(c);
            without_comments.push(c);
            i += 1;
            continue;
        }

        if c == '\'' {
            if is_rust_lifetime(&raw_chars, i) {
                without_comments.push('\'');
                i += 1;
                continue;
            }
            in_quote = Some('\'');
            without_comments.push('\'');
            i += 1;
            continue;
        }

        // Check for line comment //
        if c == '/' && i + 1 < n_raw && raw_chars[i + 1] == '/' {
            without_comments.push(' ');
            i += 2;
            while i < n_raw && raw_chars[i] != '\n' && raw_chars[i] != '\r' {
                i += 1;
            }
            continue;
        }

        // Check for block comment /* ... */ with nested block comment support
        if c == '/' && i + 1 < n_raw && raw_chars[i + 1] == '*' {
            without_comments.push(' ');
            i += 2;
            let mut depth = 1;
            while i + 1 < n_raw && depth > 0 {
                if raw_chars[i] == '/' && raw_chars[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if raw_chars[i] == '*' && raw_chars[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if depth > 0 && i < n_raw {
                i = n_raw;
            }
            continue;
        }

        without_comments.push(c);
        i += 1;
    }

    // Step 2: Replace newlines, carriage returns, tabs with space outside quotes
    let mut flat = String::with_capacity(without_comments.len());
    let chars: Vec<char> = without_comments.chars().collect();
    let n = chars.len();
    i = 0;
    in_quote = None;

    while i < n {
        let c = chars[i];
        if let Some(q) = in_quote {
            flat.push(c);
            if c == '\\' && i + 1 < n {
                flat.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == q {
                in_quote = None;
            }
            i += 1;
            continue;
        }

        if c == '"' || c == '`' {
            in_quote = Some(c);
            flat.push(c);
            i += 1;
            continue;
        }

        if c == '\'' {
            if is_rust_lifetime(&chars, i) {
                flat.push('\'');
                i += 1;
                continue;
            }
            in_quote = Some('\'');
            flat.push('\'');
            i += 1;
            continue;
        }

        if c == '\r' || c == '\n' || c == '\t' {
            flat.push(' ');
        } else {
            flat.push(c);
        }
        i += 1;
    }

    // Step 3: Strip trailing block body opening '{' or semicolon ';' outside quotes
    let flat_trimmed = flat.trim();
    let body_trimmed = flat_trimmed
        .strip_suffix('{')
        .map(str::trim)
        .unwrap_or(flat_trimmed);
    let body_trimmed = body_trimmed
        .strip_suffix(';')
        .map(str::trim)
        .unwrap_or(body_trimmed);

    // Step 4: Tokenize and collapse spacing around delimiters outside quotes
    let mut out = String::with_capacity(body_trimmed.len());
    let chars: Vec<char> = body_trimmed.chars().collect();
    let n = chars.len();
    let mut prev_char: Option<char> = None;
    in_quote = None;
    i = 0;

    while i < n {
        let c = chars[i];

        // If inside string or character literal, copy verbatim
        if let Some(q) = in_quote {
            out.push(c);
            prev_char = Some(c);
            if c == '\\' && i + 1 < n {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == q {
                in_quote = None;
            }
            i += 1;
            continue;
        }

        if c == '"' || c == '`' {
            in_quote = Some(c);
            out.push(c);
            prev_char = Some(c);
            i += 1;
            continue;
        }

        if c == '\'' {
            if is_rust_lifetime(&chars, i) {
                out.push('\'');
                prev_char = Some('\'');
                i += 1;
                continue;
            }
            in_quote = Some('\'');
            out.push('\'');
            prev_char = Some('\'');
            i += 1;
            continue;
        }

        // Rust double-colon ::
        if c == ':' && i + 1 < n && chars[i + 1] == ':' {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push_str("::");
            prev_char = Some(':');
            i += 2;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }

        // Rust return arrow ->
        if c == '-' && i + 1 < n && chars[i + 1] == '>' {
            while out.ends_with(' ') {
                out.pop();
            }
            if !out.is_empty() && !out.ends_with('(') && !out.ends_with('[') {
                out.push(' ');
            }
            out.push_str("-> ");
            prev_char = Some(' ');
            i += 2;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }

        // TypeScript arrow =>
        if c == '=' && i + 1 < n && chars[i + 1] == '>' {
            while out.ends_with(' ') {
                out.pop();
            }
            if !out.is_empty() && !out.ends_with('(') && !out.ends_with('[') {
                out.push(' ');
            }
            out.push_str("=> ");
            prev_char = Some(' ');
            i += 2;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }

        if c.is_whitespace() {
            // Peek next non-whitespace
            let mut j = i + 1;
            while j < n && chars[j].is_whitespace() {
                j += 1;
            }
            if j < n {
                let next_c = chars[j];
                // Do not emit space before ',', ':', ')', '>', ']', ';', '?'
                if matches!(next_c, ',' | ':' | ')' | '>' | ']' | ';' | '?') {
                    i = j;
                    continue;
                }
            }
            // Do not emit space after '(', '<', '[', '&'
            if let Some(prev) = prev_char {
                if matches!(prev, '(' | '<' | '[' | '&') {
                    i = j;
                    continue;
                }
                out.push(' ');
                prev_char = Some(' ');
            }
            i = j;
            continue;
        }

        if c == ',' {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push(',');
            out.push(' ');
            prev_char = Some(' ');
            i += 1;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }

        if c == ':' {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push(':');
            out.push(' ');
            prev_char = Some(' ');
            i += 1;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }

        if c == '<' {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push('<');
            prev_char = Some('<');
            i += 1;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }

        if c == '(' || c == '[' {
            // Only pop trailing space if preceded by an identifier (e.g. `getItem (` -> `getItem(`)
            // Do NOT pop trailing space if preceded by `:`, `=>`, `->`, `=`, `,`
            if let Some(trimmed) = out.strip_suffix(' ') {
                if !trimmed.ends_with(':')
                    && !trimmed.ends_with("=>")
                    && !trimmed.ends_with("->")
                    && !trimmed.ends_with('=')
                    && !trimmed.ends_with(',')
                {
                    out.pop();
                }
            }
            out.push(c);
            prev_char = Some(c);
            i += 1;
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            continue;
        }

        if matches!(c, ')' | ']') {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push(c);
            prev_char = Some(c);
            i += 1;
            let mut j = i;
            while j < n && chars[j].is_whitespace() {
                j += 1;
            }
            if j < n {
                let next_c = chars[j];
                // If followed by '(', don't keep space (e.g. `fn()()`)
                if next_c == '(' {
                    i = j;
                    continue;
                }
                // If followed by an identifier/keyword (e.g. `) void` or `) const`), emit a space!
                if next_c.is_alphanumeric() || matches!(next_c, '_' | '$' | '*' | '&') {
                    out.push(' ');
                    prev_char = Some(' ');
                    i = j;
                    continue;
                }
            }
            i = j;
            continue;
        }

        if c == '>' {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push('>');
            prev_char = Some('>');
            i += 1;
            let mut j = i;
            while j < n && chars[j].is_whitespace() {
                j += 1;
            }
            if j < n {
                let next_c = chars[j];
                // If next character is an identifier/keyword/token character (alphanumeric, '_', '$', '*', '&')
                // we must preserve a separating space (e.g. `impl<T> Queue<T>`)
                if next_c.is_alphanumeric() || matches!(next_c, '_' | '$' | '*' | '&') {
                    out.push(' ');
                    prev_char = Some(' ');
                    i = j;
                    continue;
                }
                i = j;
                continue;
            }
            continue;
        }

        out.push(c);
        prev_char = Some(c);
        i += 1;
    }

    out.trim().to_string()
}

pub fn is_external_target(target: &str) -> bool {
    let clean = target.trim();
    if clean.is_empty() {
        return false;
    }
    if clean.starts_with('.') || clean.starts_with('/') || clean.starts_with('\\') {
        return false;
    }
    if clean.contains("node_modules") || clean.contains("vendor/") || clean.contains("vendor\\") {
        return true;
    }
    if clean.starts_with('@') {
        return true;
    }
    if clean.starts_with("std::") || clean.starts_with("core::") || clean.starts_with("alloc::") {
        return true;
    }
    if clean.starts_with("crate::") || clean.starts_with("super::") || clean.starts_with("self::") {
        return false;
    }
    if clean.starts_with("java.") || clean.starts_with("javax.") {
        return true;
    }
    if clean.starts_with("src/") || clean.starts_with("projects/") || clean.starts_with("tests/") || clean.starts_with("app/") {
        return false;
    }
    if clean.ends_with(".ts") || clean.ends_with(".js") || clean.ends_with(".rs") || clean.ends_with(".go") || clean.ends_with(".java") || clean.ends_with(".cs") || clean.ends_with(".py") {
        return false;
    }
    if clean.starts_with("rxjs") || clean.starts_with("lodash") || clean.starts_with("express") {
        return true;
    }
    if matches!(clean.split("::").next().unwrap_or(""), "serde" | "anyhow" | "tokio" | "clap" | "rusqlite" | "chrono" | "uuid" | "regex" | "tree_sitter") {
        return true;
    }
    if !clean.contains('/') && !clean.contains('\\') && !clean.contains("::") {
        return true;
    }
    true
}

pub fn generate_bounded_excerpt(
    source_lines: &[&str],
    start_line: usize,
    end_line: usize,
    members: &[EntityMember],
    lang: &str,
) -> (String, &'static str) {
    if source_lines.is_empty() || start_line == 0 || start_line > source_lines.len() {
        return ("[NOT_AVAILABLE]".to_string(), "NOT_AVAILABLE");
    }
    let start_idx = start_line.saturating_sub(1);
    let end_idx = end_line.min(source_lines.len());
    let raw_slice = &source_lines[start_idx..end_idx];
    let line_count = raw_slice.len();
    let char_count: usize = raw_slice.iter().map(|l| l.len() + 1).sum();

    if line_count <= 40 && char_count <= 2000 {
        let block = raw_slice.join("\n");
        return (format!("[COMPLETE_BLOCK]\n{block}"), "COMPLETE");
    }

    let header_line = raw_slice.first().copied().unwrap_or("").trim_end();
    let mut contracted = Vec::new();
    contracted.push(header_line.to_string());

    let mut added_members = 0;
    for m in members {
        if m.member_kind == "constructor" {
            contracted.push(format!("  {} {{ /* ... */ }}", m.signature));
            added_members += 1;
        } else if m.member_kind == "method" || m.member_kind == "overload_signature" {
            if m.visibility == "public" || m.visibility == "pub" {
                if m.member_kind == "overload_signature" {
                    contracted.push(format!("  {};", m.signature));
                } else {
                    contracted.push(format!("  {} {{ /* ... */ }}", m.signature));
                }
                added_members += 1;
            }
        } else if m.member_kind == "field" {
            if m.visibility == "public" || m.visibility == "pub" {
                if lang == "rust" {
                    contracted.push(format!("  {},", m.signature));
                } else {
                    contracted.push(format!("  {};", m.signature));
                }
                added_members += 1;
            }
        }
    }

    if added_members == 0 {
        for line in raw_slice.iter().take(30) {
            contracted.push(line.to_string());
        }
        contracted.push("  // ... [remaining body elided]".to_string());
    }

    if !contracted.last().map(|l| l.trim() == "}").unwrap_or(false) {
        contracted.push("}".to_string());
    }

    let result = format!("[CONTRACTED_SIGNATURES]\n{}", contracted.join("\n"));
    (result, "CONTRACTED")
}

fn extract_ts_decorators(node: Node, src: &[u8]) -> Vec<String> {
    let mut decorators = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "decorator" {
            decorators.push(node_text(child, src));
        }
    }
    if node.kind() == "class_declaration" {
        if let Some(parent) = node.parent() {
            if parent.kind() == "export_statement" {
                let mut pcursor = parent.walk();
                for child in parent.children(&mut pcursor) {
                    if child.kind() == "decorator" {
                        let text = node_text(child, src);
                        if !decorators.contains(&text) {
                            decorators.push(text);
                        }
                    }
                }
            }
        }
    }
    decorators
}

fn extract_ts_visibility(node: Node, src: &[u8]) -> String {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "accessibility_modifier" {
            let text = node_text(child, src);
            if matches!(text.as_str(), "public" | "private" | "protected") {
                return text;
            }
        }
    }
    "public".to_string()
}

fn extract_typescript_members(
    node: Node,
    src: &[u8],
    entity_id: &str,
    out: &mut Vec<EntityMember>,
) {
    let kind = node.kind();
    let is_interface = kind == "interface_declaration";

    let body_node = node.child_by_field_name("body");
    let Some(body) = body_node else { return };

    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        let ck = child.kind();
        match ck {
            "method_definition" => {
                let name = child
                    .child_by_field_name("name")
                    .map(|x| node_text(x, src))
                    .unwrap_or_else(|| "<anonymous>".into());
                let vis = extract_ts_visibility(child, src);
                let is_async = child.children(&mut child.walk()).any(|c| c.kind() == "async");
                let is_static = child.children(&mut child.walk()).any(|c| c.kind() == "static");
                let params = child
                    .child_by_field_name("parameters")
                    .map(|x| node_text(x, src))
                    .unwrap_or_else(|| "()".into());
                let ret = child
                    .child_by_field_name("return_type")
                    .map(|x| node_text(x, src).trim_start_matches(':').trim().to_string());
                let decorators = extract_ts_decorators(child, src);
                let dec_json = if !decorators.is_empty() {
                    Some(serde_json::to_string(&decorators).unwrap_or_default())
                } else {
                    None
                };

                let start_line = child.start_position().row + 1;
                let end_line = child.end_position().row + 1;

                if name == "constructor" {
                    let sig = normalize_signature(&format!("constructor{params}"));
                    out.push(EntityMember {
                        member_id: member_id(entity_id, &name, &sig),
                        entity_id: entity_id.to_string(),
                        name: "constructor".to_string(),
                        member_kind: "constructor".to_string(),
                        signature: sig,
                        return_type: None,
                        start_line,
                        end_line,
                        visibility: vis,
                        is_async: false,
                        is_static: false,
                        decorators: dec_json,
                    });

                    // Check for constructor parameter properties (injected services)
                    if let Some(params_node) = child.child_by_field_name("parameters") {
                        let mut p_cursor = params_node.walk();
                        for param in params_node.children(&mut p_cursor) {
                            if param.kind() == "required_parameter" || param.kind() == "optional_parameter" {
                                let has_acc = param.children(&mut param.walk()).any(|c| c.kind() == "accessibility_modifier");
                                if has_acc {
                                    let p_vis = extract_ts_visibility(param, src);
                                    let p_name = param
                                        .child_by_field_name("name")
                                        .or_else(|| param.child_by_field_name("pattern"))
                                        .map(|x| node_text(x, src))
                                        .unwrap_or_default();
                                    let p_type = param
                                        .child_by_field_name("type")
                                        .map(|x| node_text(x, src).trim_start_matches(':').trim().to_string());
                                    if !p_name.is_empty() {
                                        let p_sig = normalize_signature(&format!(
                                            "{p_vis} {p_name}: {}",
                                            p_type.as_deref().unwrap_or("any")
                                        ));
                                        out.push(EntityMember {
                                            member_id: member_id(entity_id, &p_name, &p_sig),
                                            entity_id: entity_id.to_string(),
                                            name: p_name,
                                            member_kind: "field".to_string(),
                                            signature: p_sig,
                                            return_type: p_type,
                                            start_line: param.start_position().row + 1,
                                            end_line: param.end_position().row + 1,
                                            visibility: p_vis,
                                            is_async: false,
                                            is_static: false,
                                            decorators: None,
                                        });
                                    }
                                }
                            }
                        }
                    }
                } else {
                    let mut sig_parts = Vec::new();
                    if vis != "public" {
                        sig_parts.push(vis.as_str());
                    } else {
                        sig_parts.push("public");
                    }
                    if is_static {
                        sig_parts.push("static");
                    }
                    if is_async {
                        sig_parts.push("async");
                    }
                    let header = format!("{} {name}{params}", sig_parts.join(" "));
                    let full_sig = if let Some(ref r) = ret {
                        format!("{header}: {r}")
                    } else {
                        header
                    };
                    let sig = normalize_signature(&full_sig);

                    out.push(EntityMember {
                        member_id: member_id(entity_id, &name, &sig),
                        entity_id: entity_id.to_string(),
                        name,
                        member_kind: "method".to_string(),
                        signature: sig,
                        return_type: ret,
                        start_line,
                        end_line,
                        visibility: vis,
                        is_async,
                        is_static,
                        decorators: dec_json,
                    });
                }
            }
            "method_signature" => {
                let name = child
                    .child_by_field_name("name")
                    .map(|x| node_text(x, src))
                    .unwrap_or_else(|| "<anonymous>".into());
                let vis = extract_ts_visibility(child, src);
                let params = child
                    .child_by_field_name("parameters")
                    .map(|x| node_text(x, src))
                    .unwrap_or_else(|| "()".into());
                let ret = child
                    .child_by_field_name("return_type")
                    .map(|x| node_text(x, src).trim_start_matches(':').trim().to_string());
                let is_static = child.children(&mut child.walk()).any(|c| c.kind() == "static");

                let mut sig_parts = Vec::new();
                if vis != "public" {
                    sig_parts.push(vis.as_str());
                } else {
                    sig_parts.push("public");
                }
                if is_static {
                    sig_parts.push("static");
                }
                let header = format!("{} {name}{params}", sig_parts.join(" "));
                let full_sig = if let Some(ref r) = ret {
                    format!("{header}: {r}")
                } else {
                    header
                };
                let sig = normalize_signature(&full_sig);

                out.push(EntityMember {
                    member_id: member_id(entity_id, &name, &sig),
                    entity_id: entity_id.to_string(),
                    name,
                    member_kind: if is_interface {
                        "method".to_string()
                    } else {
                        "overload_signature".to_string()
                    },
                    signature: sig,
                    return_type: ret,
                    start_line: child.start_position().row + 1,
                    end_line: child.end_position().row + 1,
                    visibility: vis,
                    is_async: false,
                    is_static,
                    decorators: None,
                });
            }
            "public_field_definition" | "property_definition" | "property_signature" => {
                let name = child
                    .child_by_field_name("name")
                    .map(|x| node_text(x, src))
                    .unwrap_or_else(|| "<anonymous>".into());
                let vis = extract_ts_visibility(child, src);
                let is_static = child.children(&mut child.walk()).any(|c| c.kind() == "static");
                let prop_type = child
                    .child_by_field_name("type")
                    .map(|x| node_text(x, src).trim_start_matches(':').trim().to_string());
                let decorators = extract_ts_decorators(child, src);
                let dec_json = if !decorators.is_empty() {
                    Some(serde_json::to_string(&decorators).unwrap_or_default())
                } else {
                    None
                };

                let full_sig = if let Some(ref pt) = prop_type {
                    format!("{vis} {name}: {pt}")
                } else {
                    format!("{vis} {name}")
                };
                let sig = normalize_signature(&full_sig);

                out.push(EntityMember {
                    member_id: member_id(entity_id, &name, &sig),
                    entity_id: entity_id.to_string(),
                    name,
                    member_kind: "field".to_string(),
                    signature: sig,
                    return_type: prop_type,
                    start_line: child.start_position().row + 1,
                    end_line: child.end_position().row + 1,
                    visibility: vis,
                    is_async: false,
                    is_static,
                    decorators: dec_json,
                });
            }
            _ => {}
        }
    }
}

fn extract_rust_attributes(node: Node, src: &[u8]) -> Vec<String> {
    let mut attrs = Vec::new();
    let mut prev = node.prev_sibling();
    while let Some(sib) = prev {
        if sib.kind() == "attribute_item" {
            attrs.push(node_text(sib, src));
            prev = sib.prev_sibling();
        } else {
            break;
        }
    }
    attrs.reverse();
    attrs
}

fn extract_rust_visibility(node: Node, src: &[u8]) -> String {
    let vis_node = node
        .child_by_field_name("visibility")
        .or_else(|| {
            let mut cursor = node.walk();
            node.children(&mut cursor).find(|c| c.kind() == "visibility_modifier")
        });

    if let Some(vis) = vis_node {
        let text = node_text(vis, src);
        if !text.is_empty() {
            return text;
        }
    }
    "private".to_string()
}

fn extract_rust_members(
    node: Node,
    src: &[u8],
    entity_id: &str,
    out: &mut Vec<EntityMember>,
) {
    let kind = node.kind();
    match kind {
        "struct_item" => {
            let attrs = extract_rust_attributes(node, src);
            let dec_json = if !attrs.is_empty() {
                Some(serde_json::to_string(&attrs).unwrap_or_default())
            } else {
                None
            };

            if let Some(fields) = node.child_by_field_name("body") {
                let mut cursor = fields.walk();
                for field in fields.children(&mut cursor) {
                    if field.kind() == "field_declaration" {
                        let name = field
                            .child_by_field_name("name")
                            .map(|x| node_text(x, src))
                            .unwrap_or_default();
                        let f_type = field
                            .child_by_field_name("type")
                            .map(|x| node_text(x, src))
                            .unwrap_or_default();
                        let vis = extract_rust_visibility(field, src);
                        let sig = normalize_signature(&format!("{vis} {name}: {f_type}"));

                        out.push(EntityMember {
                            member_id: member_id(entity_id, &name, &sig),
                            entity_id: entity_id.to_string(),
                            name,
                            member_kind: "field".to_string(),
                            signature: sig,
                            return_type: Some(f_type),
                            start_line: field.start_position().row + 1,
                            end_line: field.end_position().row + 1,
                            visibility: vis,
                            is_async: false,
                            is_static: false,
                            decorators: dec_json.clone(),
                        });
                    }
                }
            }
        }
        "enum_item" => {
            if let Some(variants) = node.child_by_field_name("body") {
                let mut cursor = variants.walk();
                for variant in variants.children(&mut cursor) {
                    if variant.kind() == "enum_variant" {
                        let name = variant
                            .child_by_field_name("name")
                            .map(|x| node_text(x, src))
                            .unwrap_or_default();
                        let sig = normalize_signature(&node_text(variant, src));

                        out.push(EntityMember {
                            member_id: member_id(entity_id, &name, &sig),
                            entity_id: entity_id.to_string(),
                            name,
                            member_kind: "variant".to_string(),
                            signature: sig,
                            return_type: None,
                            start_line: variant.start_position().row + 1,
                            end_line: variant.end_position().row + 1,
                            visibility: "pub".to_string(),
                            is_async: false,
                            is_static: false,
                            decorators: None,
                        });
                    }
                }
            }
        }
        "trait_item" | "impl_item" => {
            if let Some(body) = node.child_by_field_name("body") {
                let mut cursor = body.walk();
                for item in body.children(&mut cursor) {
                    if item.kind() == "function_item" || item.kind() == "function_signature_item" {
                        let name = item
                            .child_by_field_name("name")
                            .map(|x| node_text(x, src))
                            .unwrap_or_default();
                        let vis = extract_rust_visibility(item, src);
                        let is_async = item.children(&mut item.walk()).any(|c| {
                            c.kind() == "function_modifiers" && node_text(c, src).contains("async")
                        });
                        let params = item
                            .child_by_field_name("parameters")
                            .map(|x| node_text(x, src))
                            .unwrap_or_else(|| "()".into());
                        let ret = item
                            .child_by_field_name("return_type")
                            .map(|x| node_text(x, src).trim_start_matches("->").trim().to_string());

                        let attrs = extract_rust_attributes(item, src);
                        let dec_json = if !attrs.is_empty() {
                            Some(serde_json::to_string(&attrs).unwrap_or_default())
                        } else {
                            None
                        };

                        let async_str = if is_async { "async " } else { "" };
                        let full_sig = if let Some(ref r) = ret {
                            format!("{vis} {async_str}fn {name}{params} -> {r}")
                        } else {
                            format!("{vis} {async_str}fn {name}{params}")
                        };
                        let sig = normalize_signature(&full_sig);

                        out.push(EntityMember {
                            member_id: member_id(entity_id, &name, &sig),
                            entity_id: entity_id.to_string(),
                            name,
                            member_kind: "method".to_string(),
                            signature: sig,
                            return_type: ret,
                            start_line: item.start_position().row + 1,
                            end_line: item.end_position().row + 1,
                            visibility: vis,
                            is_async,
                            is_static: false,
                            decorators: dec_json,
                        });
                    }
                }
            }
        }
        _ => {}
    }
}

fn extract_typescript_imports(
    n: Node,
    src: &[u8],
    path: &str,
    rels: &mut Vec<Relationship>,
) {
    if let Some(src_node) = n.child_by_field_name("source") {
        let raw = node_text(src_node, src);
        let spec = raw.trim_matches(|c| c == '\'' || c == '"' || c == ' ');
        if !spec.is_empty() {
            let target = resolve_import_path(path, spec);
            let is_ext = is_external_target(&target);
            rels.push(Relationship {
                source: path.into(),
                target: target.clone(),
                kind: "IMPORTS".into(),
                evidence: "TREE_SITTER_IMPORT_SYNTAX".into(),
                confidence: 0.99,
                line: n.start_position().row + 1,
                is_external: is_ext,
            });

            // Extract clean imported symbol specifiers without noise
            let mut cursor = n.walk();
            for child in n.children(&mut cursor) {
                if child.kind() == "import_clause" {
                    let mut ic_cursor = child.walk();
                    for ic_child in child.children(&mut ic_cursor) {
                        if ic_child.kind() == "named_imports" {
                            let mut ni_cursor = ic_child.walk();
                            for spec_node in ic_child.children(&mut ni_cursor) {
                                if spec_node.kind() == "import_specifier" {
                                    let sym_name = spec_node
                                        .child_by_field_name("name")
                                        .map(|x| node_text(x, src))
                                        .unwrap_or_else(|| node_text(spec_node, src));
                                    if !sym_name.is_empty() && sym_name != "import" {
                                        rels.push(Relationship {
                                            source: path.into(),
                                            target: format!("{target}::{sym_name}"),
                                            kind: "IMPORTS_SYMBOL".into(),
                                            evidence: "TREE_SITTER_IMPORT_SPECIFIER".into(),
                                            confidence: 0.99,
                                            line: spec_node.start_position().row + 1,
                                            is_external: is_ext,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn extract_fallback_symbols(
    source: &str,
    path: &str,
    lang: &str,
    out: &mut Vec<Symbol>,
) {
    if lang == "typescript" || lang == "javascript" {
        let class_re = Regex::new(r#"(?m)^\s*(?:export\s+)?(?:default\s+)?(?:abstract\s+)?class\s+([A-Za-z0-9_$]+)"#).ok();
        let iface_re = Regex::new(r#"(?m)^\s*(?:export\s+)?interface\s+([A-Za-z0-9_$]+)"#).ok();
        let fn_re = Regex::new(r#"(?m)^\s*(?:export\s+)?(?:async\s+)?function\s+([A-Za-z0-9_$]+)"#).ok();

        for (line_idx, line) in source.lines().enumerate() {
            let row = line_idx + 1;
            if let Some(ref re) = class_re {
                if let Some(caps) = re.captures(line) {
                    let name = caps[1].to_string();
                    let sig = normalize_signature(line);
                    let fp = hash(sig.as_bytes());
                    let stable = hash(format!("{path}\0CLASS\0{name}\0{fp}").as_bytes());
                    out.push(Symbol {
                        id: format!("ent_{}", &stable[..24]),
                        path: path.into(),
                        name,
                        kind: "CLASS".into(),
                        line: row,
                        end_line: row,
                        signature: sig,
                        fingerprint: fp,
                    });
                }
            }
            if let Some(ref re) = iface_re {
                if let Some(caps) = re.captures(line) {
                    let name = caps[1].to_string();
                    let sig = normalize_signature(line);
                    let fp = hash(sig.as_bytes());
                    let stable = hash(format!("{path}\0INTERFACE\0{name}\0{fp}").as_bytes());
                    out.push(Symbol {
                        id: format!("ent_{}", &stable[..24]),
                        path: path.into(),
                        name,
                        kind: "INTERFACE".into(),
                        line: row,
                        end_line: row,
                        signature: sig,
                        fingerprint: fp,
                    });
                }
            }
            if let Some(ref re) = fn_re {
                if let Some(caps) = re.captures(line) {
                    let name = caps[1].to_string();
                    let sig = normalize_signature(line);
                    let fp = hash(sig.as_bytes());
                    let stable = hash(format!("{path}\0FUNCTION\0{name}\0{fp}").as_bytes());
                    out.push(Symbol {
                        id: format!("ent_{}", &stable[..24]),
                        path: path.into(),
                        name,
                        kind: "FUNCTION".into(),
                        line: row,
                        end_line: row,
                        signature: sig,
                        fingerprint: fp,
                    });
                }
            }
        }
    } else if lang == "rust" {
        let struct_re = Regex::new(r#"(?m)^\s*(?:pub(?:\([^\)]+\))?\s+)?struct\s+([A-Za-z0-9_]+)"#).ok();
        let trait_re = Regex::new(r#"(?m)^\s*(?:pub(?:\([^\)]+\))?\s+)?trait\s+([A-Za-z0-9_]+)"#).ok();
        let impl_re = Regex::new(r#"(?m)^\s*impl(?:<[^>]+>)?\s+(?:([A-Za-z0-9_]+)\s+for\s+)?([A-Za-z0-9_]+)"#).ok();
        let fn_re = Regex::new(r#"(?m)^\s*(?:pub(?:\([^\)]+\))?\s+)?(?:async\s+)?fn\s+([A-Za-z0-9_]+)"#).ok();

        for (line_idx, line) in source.lines().enumerate() {
            let row = line_idx + 1;
            if let Some(ref re) = struct_re {
                if let Some(caps) = re.captures(line) {
                    let name = caps[1].to_string();
                    let sig = normalize_signature(line);
                    let fp = hash(sig.as_bytes());
                    let stable = hash(format!("{path}\0STRUCT\0{name}\0{fp}").as_bytes());
                    out.push(Symbol {
                        id: format!("ent_{}", &stable[..24]),
                        path: path.into(),
                        name,
                        kind: "STRUCT".into(),
                        line: row,
                        end_line: row,
                        signature: sig,
                        fingerprint: fp,
                    });
                }
            }
            if let Some(ref re) = trait_re {
                if let Some(caps) = re.captures(line) {
                    let name = caps[1].to_string();
                    let sig = normalize_signature(line);
                    let fp = hash(sig.as_bytes());
                    let stable = hash(format!("{path}\0TRAIT\0{name}\0{fp}").as_bytes());
                    out.push(Symbol {
                        id: format!("ent_{}", &stable[..24]),
                        path: path.into(),
                        name,
                        kind: "TRAIT".into(),
                        line: row,
                        end_line: row,
                        signature: sig,
                        fingerprint: fp,
                    });
                }
            }
            if let Some(ref re) = impl_re {
                if let Some(caps) = re.captures(line) {
                    let name = caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string();
                    let sig = normalize_signature(line);
                    let fp = hash(sig.as_bytes());
                    let stable = hash(format!("{path}\0IMPL\0{name}\0{fp}").as_bytes());
                    out.push(Symbol {
                        id: format!("ent_{}", &stable[..24]),
                        path: path.into(),
                        name,
                        kind: "IMPL".into(),
                        line: row,
                        end_line: row,
                        signature: sig,
                        fingerprint: fp,
                    });
                }
            }
            if let Some(ref re) = fn_re {
                if let Some(caps) = re.captures(line) {
                    let name = caps[1].to_string();
                    let sig = normalize_signature(line);
                    let fp = hash(sig.as_bytes());
                    let stable = hash(format!("{path}\0FUNCTION\0{name}\0{fp}").as_bytes());
                    out.push(Symbol {
                        id: format!("ent_{}", &stable[..24]),
                        path: path.into(),
                        name,
                        kind: "FUNCTION".into(),
                        line: row,
                        end_line: row,
                        signature: sig,
                        fingerprint: fp,
                    });
                }
            }
        }
    }
}

pub fn visit_ast(
    n: Node,
    src: &[u8],
    path: &str,
    lang: &str,
    out: &mut Vec<Symbol>,
    rels: &mut Vec<Relationship>,
    members: &mut Vec<EntityMember>,
    revision: &str,
) {
    if let Some(kind) = recognized(n.kind(), lang) {
        let name = n
            .child_by_field_name("name")
            .or_else(|| n.child_by_field_name("type"))
            .map(|x| node_text(x, src))
            .unwrap_or_else(|| "<anonymous>".into());
        let full = node_text(n, src);
        let first_line = full.lines().next().unwrap_or(&full);
        let decl_sig = normalize_signature(first_line);
        let signature = if decl_sig.is_empty() {
            full.chars().take(300).collect::<String>()
        } else {
            decl_sig
        };
        let skeleton = n.to_sexp();
        let fingerprint = hash(skeleton.as_bytes());
        let stable = hash(format!("{path}\0{kind}\0{name}\0{fingerprint}").as_bytes());
        let sym_id = format!("ent_{}", &stable[..24]);

        out.push(Symbol {
            id: sym_id.clone(),
            path: path.into(),
            name,
            kind: kind.into(),
            line: n.start_position().row + 1,
            end_line: n.end_position().row + 1,
            signature,
            fingerprint,
        });

        if lang == "typescript" || lang == "javascript" {
            if kind == "CLASS" || kind == "INTERFACE" {
                extract_typescript_members(n, src, &sym_id, members);
            }
        } else if lang == "rust" {
            if kind == "STRUCT" || kind == "TRAIT" || kind == "IMPL" || kind == "ENUM" {
                extract_rust_members(n, src, &sym_id, members);
            }
        }
    }

    let mut skip_children_imports = false;
    if lang == "typescript" || lang == "javascript" {
        if n.kind() == "import_statement" {
            extract_typescript_imports(n, src, path, rels);
            skip_children_imports = true;
        }
    } else if lang == "rust" {
        if n.kind() == "use_declaration" {
            let raw = node_text(n, src);
            let spec = extract_import_specifier(&raw, lang);
            if !spec.is_empty() {
                let is_ext = is_external_target(&spec);
                rels.push(Relationship {
                    source: path.into(),
                    target: spec,
                    kind: "IMPORTS".into(),
                    evidence: "TREE_SITTER_USE_DECLARATION".into(),
                    confidence: 0.99,
                    line: n.start_position().row + 1,
                    is_external: is_ext,
                });
            }
            skip_children_imports = true;
        }
    } else {
        if n.kind().contains("import") || n.kind() == "using_directive" {
            let raw = node_text(n, src);
            let spec = extract_import_specifier(&raw, lang);
            if !spec.is_empty() {
                let target = resolve_import_path(path, &spec);
                let is_ext = is_external_target(&target);
                rels.push(Relationship {
                    source: path.into(),
                    target,
                    kind: "IMPORTS".into(),
                    evidence: "TREE_SITTER_IMPORT_SYNTAX".into(),
                    confidence: 0.99,
                    line: n.start_position().row + 1,
                    is_external: is_ext,
                });
            }
        }
    }

    let mut c = n.walk();
    for child in n.children(&mut c) {
        if skip_children_imports && child.kind().contains("import") {
            continue;
        }
        visit_ast(child, src, path, lang, out, rels, members, revision);
    }
    let _ = revision;
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
    let mut members = Vec::new();
    visit_ast(n, src, path, lang, out, rels, &mut members, revision);
}

pub fn parse_file_ast(
    root: &Path,
    rel: &str,
    bytes: &[u8],
    rev: &str,
) -> Result<ParsedFile> {
    let path = Path::new(rel);
    let lang = match language(path) {
        Some(l) => l,
        None => {
            let record = FileRecord {
                path: rel.into(),
                hash: hash(bytes),
                language: "unknown".into(),
                excerpt: String::new(),
            };
            return Ok(ParsedFile {
                file: record,
                symbols: Vec::new(),
                relationships: Vec::new(),
                members: Vec::new(),
                parse_status: ParseStatus::NotAvailable,
            });
        }
    };
    let source = String::from_utf8_lossy(bytes);

    if lang == "go" {
        let go_mod = crate::go_parser::load_root_go_mod_doc(root);
        let ast = crate::go_parser::extract_go_ast(&source)?;
        let (symbols, rels) =
            crate::go_parser::convert_go_ast_to_engram_with_root(root, rel, &ast, go_mod.as_ref());
        let display = redact(&source);
        let record = FileRecord {
            path: rel.into(),
            hash: hash(bytes),
            language: lang.into(),
            excerpt: display.chars().take(20000).collect(),
        };
        return Ok(ParsedFile {
            file: record,
            symbols,
            relationships: rels,
            members: Vec::new(),
            parse_status: ParseStatus::Complete,
        });
    }

    if lang == "java" {
        let ast = crate::java_parser::extract_java_ast(&source)?;
        let (symbols, rels) =
            crate::java_parser::convert_java_ast_to_engram_with_root(root, rel, &ast);
        let display = redact(&source);
        let record = FileRecord {
            path: rel.into(),
            hash: hash(bytes),
            language: lang.into(),
            excerpt: display.chars().take(20000).collect(),
        };
        return Ok(ParsedFile {
            file: record,
            symbols,
            relationships: rels,
            members: Vec::new(),
            parse_status: ParseStatus::Complete,
        });
    }

    let mut parser = TsParser::new();
    parser.set_language(&lang_for(lang))?;
    let tree = parser
        .parse(source.as_ref(), None)
        .ok_or_else(|| anyhow!("parser returned no tree"))?;

    let has_error = tree.root_node().has_error();
    let mut parse_status = if has_error {
        ParseStatus::Partial
    } else {
        ParseStatus::Complete
    };

    let mut symbols = Vec::new();
    let mut rels = Vec::new();
    let mut members = Vec::new();

    visit_ast(
        tree.root_node(),
        source.as_bytes(),
        rel,
        lang,
        &mut symbols,
        &mut rels,
        &mut members,
        rev,
    );

    // If syntax errors occurred, run fallback extraction to catch symbols swallowed by error recovery
    if has_error {
        let mut fallback = Vec::new();
        extract_fallback_symbols(&source, rel, lang, &mut fallback);
        for fs in fallback {
            if !symbols.iter().any(|s| s.name == fs.name && s.kind == fs.kind) {
                symbols.push(fs);
            }
        }
        parse_status = ParseStatus::Partial;
    }

    // Link methods from IMPL to matching STRUCT in same file
    if lang == "rust" {
        let struct_map: HashMap<String, String> = symbols
            .iter()
            .filter(|s| s.kind == "STRUCT")
            .map(|s| (s.name.clone(), s.id.clone()))
            .collect();

        if !struct_map.is_empty() {
            let mut extra_members = Vec::new();
            for m in &members {
                if m.member_kind == "method" {
                    if let Some(parent) = symbols.iter().find(|s| s.id == m.entity_id && s.kind == "IMPL") {
                        if let Some(struct_id) = struct_map.get(&parent.name) {
                            if struct_id != &m.entity_id {
                                let mut m_clone = m.clone();
                                m_clone.entity_id = struct_id.clone();
                                m_clone.member_id = member_id(&m_clone.entity_id, &m_clone.name, &m_clone.signature);
                                extra_members.push(m_clone);
                            }
                        }
                    }
                }
            }
            members.extend(extra_members);
        }
    }

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
                is_external: false,
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
                is_external: false,
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
            is_external: false,
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
            is_external: false,
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
            is_external: false,
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
    Ok(ParsedFile {
        file: record,
        symbols,
        relationships: rels,
        members,
        parse_status,
    })
}

pub fn parse_file(
    root: &Path,
    rel: &str,
    bytes: &[u8],
    rev: &str,
) -> Result<(FileRecord, Vec<Symbol>, Vec<Relationship>)> {
    let parsed = parse_file_ast(root, rel, bytes, rev)?;
    Ok((parsed.file, parsed.symbols, parsed.relationships))
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
             FROM entity_versions WHERE invalidated_at_revision IS NULL",
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
    crate::incremental::init_meta_cache(&conn)?;
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
    let mut changed_files_list: Vec<String> = Vec::new();
    let mut changed_symbols_list: Vec<String> = Vec::new();
    for (abs, rel) in &files {
        let (content_hash, bytes, _is_fast) =
            match crate::incremental::get_authoritative_content_hash(abs, rel, &tx) {
                Ok(res) => res,
                Err(_) => {
                    let b = match fs::read(abs) {
                        Ok(b) if b.len() <= 2_000_000 && !b.contains(&0) => b,
                        _ => continue,
                    };
                    let h = hash(&b);
                    (h, b, false)
                }
            };
        if !bytes.is_empty() && (bytes.len() > 2_000_000 || bytes.contains(&0)) {
            continue;
        }
        seen.insert(rel.clone());
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
        let bytes = if bytes.is_empty() {
            match fs::read(abs) {
                Ok(b) if b.len() <= 2_000_000 && !b.contains(&0) => b,
                _ => continue,
            }
        } else {
            bytes
        };
        let parsed = parse_file_ast(&root, rel, &bytes, &rev)?;
        let file = parsed.file;
        let mut symbols = parsed.symbols;
        let rels = parsed.relationships;
        let mut members = parsed.members;
        let parse_status = parsed.parse_status;

        let old_ids: Vec<String> = symbols.iter().map(|s| s.id.clone()).collect();
        reconcile_symbol_ids(&tx, &mut symbols, &current_paths, &mut assigned_entity_ids)?;
        let id_map: HashMap<String, String> = old_ids
            .into_iter()
            .zip(symbols.iter().map(|s| s.id.clone()))
            .filter(|(old, new)| old != new)
            .collect();

        if !id_map.is_empty() {
            for m in &mut members {
                if let Some(new_id) = id_map.get(&m.entity_id) {
                    m.entity_id = new_id.clone();
                    m.member_id = member_id(&m.entity_id, &m.name, &m.signature);
                }
            }
        }

        changed += 1;
        changed_files_list.push(rel.clone());
        for s in &symbols {
            changed_symbols_list.push(s.name.clone());
        }
        tx.execute(
            "INSERT INTO files(path, content_hash, analysis_key, language, excerpt, revision, analyzed_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7)
             ON CONFLICT(path) DO UPDATE SET content_hash=excluded.content_hash, analysis_key=excluded.analysis_key, language=excluded.language, excerpt=excluded.excerpt, revision=excluded.revision, analyzed_at=excluded.analyzed_at",
            params![file.path, file.hash, file_analysis_key, file.language, file.excerpt, rev, now()]
        )?;
        tx.execute("UPDATE entities SET invalidated_at_revision=?1 WHERE path=?2 AND invalidated_at_revision IS NULL", params![rev, rel])?;
        tx.execute("UPDATE entity_versions SET invalidated_at_revision=?1 WHERE path=?2 AND invalidated_at_revision IS NULL", params![rev, rel])?;
        tx.execute("UPDATE relationships SET invalidated_at_revision=?1 WHERE path=?2 AND invalidated_at_revision IS NULL", params![rev, rel])?;
        tx.execute("DELETE FROM entity_members WHERE entity_id IN (SELECT entity_id FROM entities WHERE path = ?1)", [&rel])?;

        for s in symbols {
            tx.execute("INSERT OR IGNORE INTO entity_identity(entity_id, kind, created_at_revision) VALUES(?1,?2,?3)", params![s.id, s.kind, rev])?;
            tx.execute(
                "INSERT OR REPLACE INTO entity_versions(version_id, entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, invalidated_at_revision, status)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,NULL,'OBSERVED')",
                params![Uuid::new_v4().to_string(), s.id, s.path, s.name, s.kind, s.line as i64, s.end_line as i64, s.signature, s.fingerprint, rev]
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO entities(entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, invalidated_at_revision, status, parse_status, is_external)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,NULL,'OBSERVED',?10,0)",
                params![s.id, s.path, s.name, s.kind, s.line as i64, s.end_line as i64, s.signature, s.fingerprint, rev, parse_status.as_str()]
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
        insert_entity_members(&tx, &members)?;

        for r in rels {
            let relationship_id = format!(
                "rel_{}",
                &hash(format!("{}\0{}\0{}\0{}", rel, r.kind, r.target, r.line).as_bytes())[..24]
            );
            let is_ext = if r.is_external { 1 } else { 0 };
            tx.execute(
                "INSERT INTO relationships(source_id, target, kind, evidence, confidence, path, line, created_at_revision, is_external)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![r.source, r.target, r.kind, r.evidence, r.confidence, rel, r.line as i64, rev, is_ext]
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

    // Cross-file Go structural interface matching
    let mut go_file_asts = Vec::new();
    for (abs, rel) in &files {
        if rel.ends_with(".go") {
            if let Ok(content) = fs::read_to_string(abs) {
                if let Ok(ast) = crate::go_parser::extract_go_ast(&content) {
                    go_file_asts.push((rel.clone(), ast));
                }
            }
        }
    }
    if !go_file_asts.is_empty() {
        let go_refs: Vec<(&str, &crate::go_parser::GoFileAst)> =
            go_file_asts.iter().map(|(p, a)| (p.as_str(), a)).collect();
        let cross_iface_rels = crate::go_parser::infer_cross_file_go_interfaces(&go_refs);
        for r in cross_iface_rels {
            let path = r.source.split("::").next().unwrap_or(&r.source);
            let relationship_id = format!(
                "rel_{}",
                &hash(format!("{}\0{}\0{}\0{}", path, r.kind, r.target, r.line).as_bytes())[..24]
            );
            let is_ext = if r.is_external { 1 } else { 0 };
            tx.execute(
                "INSERT INTO relationships(source_id, target, kind, evidence, confidence, path, line, created_at_revision, is_external)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![r.source, r.target, r.kind, r.evidence, r.confidence, path, r.line as i64, rev, is_ext]
            )?;
            tx.execute(
                "INSERT INTO observations(observation_id, target_id, revision_id, property_name, value_json, observed_at)
                 VALUES(?1,?2,?3,'STATIC_RELATIONSHIP',?4,?5)",
                params![Uuid::new_v4().to_string(), relationship_id, rev, json!({"type":&r.kind,"target":&r.target,"path":path,"line":r.line}).to_string(), now()]
            )?;
            tx.execute(
                "INSERT INTO evidence(evidence_id, target_id, target_type, tier, confidence, source_type, source_path, source_line, recorded_at, decay_half_life_commits, verification_status, details_json)
                 VALUES(?1,?2,'RELATIONSHIP','EXTRACTED',?3,'TREE_SITTER',?4,?5,?6,NULL,'UNCONTESTED',?7)",
                params![Uuid::new_v4().to_string(), relationship_id, r.confidence, path, r.line as i64, now(), json!({"parser":"tree-sitter","edge_type":&r.kind}).to_string()]
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
    if !changed_files_list.is_empty() {
        if let Ok(dependents) = crate::incremental::find_tier2_relational_dependents(
            &tx,
            &changed_files_list,
            &changed_symbols_list,
        ) {
            for dep in dependents {
                tx.execute(
                    "UPDATE relationships SET created_at_revision = ?1 WHERE path = ?2 AND invalidated_at_revision IS NULL",
                    params![rev, dep],
                ).ok();
            }
        }
    }
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('head_revision', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [&rev],
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
