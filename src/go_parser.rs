//! Go Language Parser: Tree-sitter AST extraction, canonical receiver-normalized symbols,
//! go.mod full directive resolution (require, replace, exclude, toolchain),
//! dependency classification (LOCAL, VENDORED, REPLACED_LOCAL, REPLACED_EXTERNAL, EXTERNAL, UNRESOLVED),
//! two-stage structural interface satisfaction, and test conventions.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::{Node, Parser as TsParser};

use crate::analyzer::{node_text, Relationship, Symbol};
use crate::storage::hash;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PackageOrigin {
    Local,
    Vendored,
    ReplacedLocal,
    ReplacedExternal,
    External,
    Unresolved,
}

impl PackageOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Local => "LOCAL",
            Self::Vendored => "VENDORED",
            Self::ReplacedLocal => "REPLACED_LOCAL",
            Self::ReplacedExternal => "REPLACED_EXTERNAL",
            Self::External => "EXTERNAL",
            Self::Unresolved => "UNRESOLVED",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GoModDocument {
    pub module: Option<String>,
    pub go_version: Option<String>,
    pub toolchain: Option<String>,
    pub requires: Vec<GoRequire>,
    pub replaces: Vec<GoReplace>,
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoRequire {
    pub path: String,
    pub version: String,
    pub indirect: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoReplace {
    pub old_path: String,
    pub old_version: Option<String>,
    pub new_path: String,
    pub new_version: Option<String>,
    pub is_local_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoPackage {
    pub name: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoImport {
    pub path: String,
    pub alias: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoType {
    pub name: String,
    pub kind: String, // "STRUCT", "INTERFACE", "TYPE"
    pub line: usize,
    pub end_line: usize,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoMethodSignature {
    pub name: String,
    pub params: Vec<String>,
    pub results: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoInterfaceDef {
    pub name: String,
    pub methods: Vec<GoMethodSignature>,
    pub line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoFunction {
    pub name: String,
    pub line: usize,
    pub end_line: usize,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoMethod {
    pub name: String,              // canonical name: "OrderService.ExportCSV"
    pub receiver_type: String,     // "OrderService" (normalized, no '*')
    pub is_pointer_receiver: bool, // true if (*OrderService)
    pub method_name: String,       // "ExportCSV"
    pub signature_def: GoMethodSignature,
    pub line: usize,
    pub end_line: usize,
    pub signature: String,
}

#[derive(Debug, Clone, Default)]
pub struct GoFileAst {
    pub package: Option<GoPackage>,
    pub imports: Vec<GoImport>,
    pub types: Vec<GoType>,
    pub interfaces: Vec<GoInterfaceDef>,
    pub functions: Vec<GoFunction>,
    pub methods: Vec<GoMethod>,
}

/// Parse full `go.mod` document including module, require, replace, exclude, and toolchain.
pub fn parse_full_go_mod(contents: &str) -> GoModDocument {
    let mut doc = GoModDocument::default();
    let mut current_block: Option<&str> = None;

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        if line == ")" {
            current_block = None;
            continue;
        }

        if let Some(block) = current_block {
            let clean_line = line.split("//").next().unwrap_or(line).trim();
            if clean_line.is_empty() {
                continue;
            }
            match block {
                "require" => {
                    let is_indirect = raw_line.contains("// indirect");
                    if let Some(req) = parse_require_line(clean_line, is_indirect) {
                        doc.requires.push(req);
                    }
                }
                "replace" => {
                    if let Some(rep) = parse_replace_line(clean_line) {
                        doc.replaces.push(rep);
                    }
                }
                "exclude" => {
                    doc.excludes.push(clean_line.to_string());
                }
                _ => {}
            }
            continue;
        }

        // Check for block starts: require (, replace (, exclude (
        if line.starts_with("require (") || line == "require (" {
            current_block = Some("require");
            continue;
        }
        if line.starts_with("replace (") || line == "replace (" {
            current_block = Some("replace");
            continue;
        }
        if line.starts_with("exclude (") || line == "exclude (" {
            current_block = Some("exclude");
            continue;
        }

        // Single-line directives
        if line.starts_with("module ") {
            let mod_part = line["module ".len()..].trim();
            let clean = mod_part
                .split("//")
                .next()
                .unwrap_or(mod_part)
                .trim()
                .trim_matches(|c| c == '"' || c == '\'');
            if !clean.is_empty() {
                doc.module = Some(clean.to_string());
            }
        } else if line.starts_with("go ") {
            let ver = line["go ".len()..]
                .trim()
                .split("//")
                .next()
                .unwrap_or("")
                .trim();
            doc.go_version = Some(ver.to_string());
        } else if line.starts_with("toolchain ") {
            let tc = line["toolchain ".len()..]
                .trim()
                .split("//")
                .next()
                .unwrap_or("")
                .trim();
            doc.toolchain = Some(tc.to_string());
        } else if line.starts_with("require ") {
            let req_part = line["require ".len()..].trim();
            let is_indirect = raw_line.contains("// indirect");
            let clean = req_part.split("//").next().unwrap_or(req_part).trim();
            if let Some(req) = parse_require_line(clean, is_indirect) {
                doc.requires.push(req);
            }
        } else if line.starts_with("replace ") {
            let rep_part = line["replace ".len()..].trim();
            let clean = rep_part.split("//").next().unwrap_or(rep_part).trim();
            if let Some(rep) = parse_replace_line(clean) {
                doc.replaces.push(rep);
            }
        } else if line.starts_with("exclude ") {
            let exc_part = line["exclude ".len()..].trim();
            let clean = exc_part.split("//").next().unwrap_or(exc_part).trim();
            doc.excludes.push(clean.to_string());
        }
    }

    doc
}

fn parse_require_line(line: &str, indirect: bool) -> Option<GoRequire> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() >= 2 {
        Some(GoRequire {
            path: parts[0].trim_matches('"').to_string(),
            version: parts[1].to_string(),
            indirect,
        })
    } else {
        None
    }
}

fn parse_replace_line(line: &str) -> Option<GoReplace> {
    // Syntax: old_path [old_version] => new_path [new_version]
    let arrow_parts: Vec<&str> = line.split("=>").collect();
    if arrow_parts.len() != 2 {
        return None;
    }

    let left = arrow_parts[0].trim();
    let right = arrow_parts[1].trim();

    let left_parts: Vec<&str> = left.split_whitespace().collect();
    let right_parts: Vec<&str> = right.split_whitespace().collect();

    if left_parts.is_empty() || right_parts.is_empty() {
        return None;
    }

    let old_path = left_parts[0].trim_matches('"').to_string();
    let old_version = if left_parts.len() > 1 {
        Some(left_parts[1].to_string())
    } else {
        None
    };

    let new_path = right_parts[0].trim_matches('"').to_string();
    let new_version = if right_parts.len() > 1 {
        Some(right_parts[1].to_string())
    } else {
        None
    };

    let is_local_dir =
        new_path.starts_with('.') || new_path.starts_with('/') || new_path.contains('\\');

    Some(GoReplace {
        old_path,
        old_version,
        new_path,
        new_version,
        is_local_dir,
    })
}

/// Backwards-compatible parser: extracts module path from a go.mod file.
pub fn parse_go_mod(contents: &str) -> Option<String> {
    parse_full_go_mod(contents).module
}

/// Locate and read `go.mod` in root directory returning full `GoModDocument`.
pub fn load_root_go_mod_doc(root: &Path) -> Option<GoModDocument> {
    let mod_path = root.join("go.mod");
    if mod_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&mod_path) {
            return Some(parse_full_go_mod(&content));
        }
    }
    None
}

/// Locate and read `go.mod` in root directory if present.
pub fn load_root_go_mod(root: &Path) -> Option<String> {
    load_root_go_mod_doc(root).and_then(|doc| doc.module)
}

/// Normalize Go symbol names by stripping pointer prefixes (e.g. "*OrderService.ExportCSV" -> "OrderService.ExportCSV").
pub fn normalize_go_symbol_name(sym: &str) -> String {
    let trimmed = sym.trim();
    if let Some(pos) = trimmed.find('.') {
        let receiver = &trimmed[..pos];
        let method = &trimmed[pos + 1..];
        let norm_recv = receiver.trim_start_matches(|c| c == '*' || c == '&').trim();
        format!("{norm_recv}.{method}")
    } else {
        trimmed
            .trim_start_matches(|c| c == '*' || c == '&')
            .trim()
            .to_string()
    }
}

/// Checks whether an expected contract symbol matches an actual symbol name.
pub fn matches_go_contract_symbol(expected: &str, actual: &str) -> bool {
    let norm_exp = normalize_go_symbol_name(expected);
    let norm_act = normalize_go_symbol_name(actual);
    norm_exp == norm_act
}

/// Check if a package import is part of the Go standard library.
pub fn is_go_stdlib(import_path: &str) -> bool {
    // Go stdlib packages do not have a domain dot in the first component
    let first_part = import_path.split('/').next().unwrap_or(import_path);
    !first_part.contains('.')
}

/// Classify and resolve dependency import path with strict Go module semantics.
/// Invariant 1: AMstraLift/Engram must never report an external dependency as local merely because a matching-looking directory exists.
/// Invariant 2: An unresolved dependency must remain UNRESOLVED and must never silently degrade into an inferred relationship.
pub fn resolve_go_dependency_origin(
    root: &Path,
    current_file: &str,
    import_path: &str,
    go_mod: Option<&GoModDocument>,
) -> (PackageOrigin, Vec<String>) {
    // 1. Relative import (e.g. "./payment", "../common")
    if import_path.starts_with('.') {
        let parent = Path::new(current_file).parent().unwrap_or(Path::new(""));
        let joined = parent.join(import_path);
        let normalized = crate::analyzer::normalize_rel_path(&joined);

        let target_dir = root.join(&normalized);
        let mut target_files = Vec::new();
        if target_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&target_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() && p.extension().map(|e| e == "go").unwrap_or(false) {
                        if let Ok(rel) = p.strip_prefix(root) {
                            target_files.push(crate::analyzer::normalize_rel_path(rel));
                        }
                    }
                }
            }
        }
        if target_files.is_empty() {
            target_files.push(normalized);
        }
        return (PackageOrigin::Local, target_files);
    }

    // 2. Vendored check: vendor/<import_path>
    let vendor_dir = root.join("vendor").join(import_path);
    if vendor_dir.is_dir() {
        let mut target_files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&vendor_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().map(|e| e == "go").unwrap_or(false) {
                    if let Ok(rel) = p.strip_prefix(root) {
                        target_files.push(crate::analyzer::normalize_rel_path(rel));
                    }
                }
            }
        }
        if target_files.is_empty() {
            target_files.push(format!("vendor/{import_path}"));
        }
        return (PackageOrigin::Vendored, target_files);
    }

    // 3. Replace directive check
    if let Some(doc) = go_mod {
        for rep in &doc.replaces {
            if rep.old_path == import_path || import_path.starts_with(&format!("{}/", rep.old_path))
            {
                let sub_suffix = if import_path == rep.old_path {
                    ""
                } else {
                    &import_path[rep.old_path.len()..]
                };

                if rep.is_local_dir {
                    let combined = format!("{}{}", rep.new_path, sub_suffix);
                    let norm = crate::analyzer::normalize_rel_path(Path::new(&combined));
                    let local_target = root.join(&norm);
                    let mut target_files = Vec::new();
                    if local_target.is_dir() {
                        if let Ok(entries) = std::fs::read_dir(&local_target) {
                            for entry in entries.flatten() {
                                let p = entry.path();
                                if p.is_file() && p.extension().map(|e| e == "go").unwrap_or(false)
                                {
                                    if let Ok(rel) = p.strip_prefix(root) {
                                        target_files.push(crate::analyzer::normalize_rel_path(rel));
                                    }
                                }
                            }
                        }
                    }
                    if target_files.is_empty() {
                        target_files.push(norm);
                    }
                    return (PackageOrigin::ReplacedLocal, target_files);
                } else {
                    let rep_external = format!("{}{}", rep.new_path, sub_suffix);
                    return (PackageOrigin::ReplacedExternal, vec![rep_external]);
                }
            }
        }
    }

    // 4. Current / local module resolution
    // INVARIANT: Must strictly start with declared module path to be LOCAL!
    if let Some(doc) = go_mod {
        if let Some(ref mod_path) = doc.module {
            if import_path == mod_path || import_path.starts_with(&format!("{mod_path}/")) {
                let sub_path = if import_path == mod_path {
                    ".".to_string()
                } else {
                    import_path[mod_path.len() + 1..].to_string()
                };

                let target_dir = root.join(&sub_path);
                let mut target_files = Vec::new();
                if target_dir.is_dir() {
                    if let Ok(entries) = std::fs::read_dir(&target_dir) {
                        for entry in entries.flatten() {
                            let p = entry.path();
                            if p.is_file() && p.extension().map(|e| e == "go").unwrap_or(false) {
                                if let Ok(rel) = p.strip_prefix(root) {
                                    target_files.push(crate::analyzer::normalize_rel_path(rel));
                                }
                            }
                        }
                    }
                }
                if target_files.is_empty() {
                    target_files.push(sub_path);
                }
                return (PackageOrigin::Local, target_files);
            }
        }
    }

    // 5. External module or standard library
    if is_go_stdlib(import_path) {
        return (PackageOrigin::External, vec![import_path.to_string()]);
    }

    if let Some(doc) = go_mod {
        if doc
            .requires
            .iter()
            .any(|r| r.path == import_path || import_path.starts_with(&format!("{}/", r.path)))
        {
            return (PackageOrigin::External, vec![import_path.to_string()]);
        }
    }

    // 6. Explicitly UNRESOLVED (never degraded silently)
    (PackageOrigin::Unresolved, vec![import_path.to_string()])
}

/// Two-stage algorithm to verify if a struct's method set satisfies an interface:
/// 1. Method name candidate lookup
/// 2. Full signature verification (parameter and result type comparison)
pub fn check_struct_implements_interface(
    struct_methods: &[&GoMethod],
    interface: &GoInterfaceDef,
) -> bool {
    if interface.methods.is_empty() {
        return false; // Empty interface{} is ignored
    }

    for iface_method in &interface.methods {
        // Stage 1: Candidate lookup by method name
        let candidate = struct_methods
            .iter()
            .find(|m| m.method_name == iface_method.name);

        let candidate = match candidate {
            Some(c) => c,
            None => return false,
        };

        // Stage 2: Full signature comparison
        if candidate.signature_def.params != iface_method.params
            || candidate.signature_def.results != iface_method.results
        {
            return false;
        }
    }

    true
}

/// Extract Go AST from source text using Tree-sitter.
pub fn extract_go_ast(source: &str) -> Result<GoFileAst> {
    let mut parser = TsParser::new();
    let lang = tree_sitter_go::LANGUAGE.into();
    parser.set_language(&lang)?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("tree-sitter-go returned no tree"))?;

    let root = tree.root_node();
    let src = source.as_bytes();
    let mut ast = GoFileAst::default();

    visit_go_node(root, src, &mut ast);
    Ok(ast)
}

fn visit_go_node(node: Node, src: &[u8], ast: &mut GoFileAst) {
    match node.kind() {
        "package_clause" => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "package_identifier" {
                    let name = node_text(child, src);
                    ast.package = Some(GoPackage {
                        name,
                        line: node.start_position().row + 1,
                    });
                    break;
                }
            }
        }
        "import_declaration" => {
            extract_imports(node, src, &mut ast.imports);
        }
        "type_declaration" => {
            extract_types(node, src, &mut ast.types, &mut ast.interfaces);
        }
        "function_declaration" => {
            if let Some(func) = extract_function(node, src) {
                ast.functions.push(func);
            }
        }
        "method_declaration" => {
            if let Some(method) = extract_method(node, src) {
                ast.methods.push(method);
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                visit_go_node(child, src, ast);
            }
        }
    }
}

fn extract_imports(node: Node, src: &[u8], out: &mut Vec<GoImport>) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "import_spec" {
            let mut alias = None;
            let mut path = String::new();
            let mut spec_cursor = child.walk();
            for part in child.children(&mut spec_cursor) {
                if part.kind() == "package_identifier" {
                    alias = Some(node_text(part, src));
                } else if part.kind() == "interpreted_string_literal" {
                    let raw = node_text(part, src);
                    path = raw.trim_matches('"').to_string();
                }
            }
            if !path.is_empty() {
                out.push(GoImport {
                    path,
                    alias,
                    line: child.start_position().row + 1,
                });
            }
        } else if child.kind() == "import_spec_list" {
            extract_imports(child, src, out);
        }
    }
}

fn extract_parameter_types(param_node: Node, src: &[u8]) -> Vec<String> {
    let mut types = Vec::new();
    let mut cursor = param_node.walk();
    for child in param_node.children(&mut cursor) {
        if child.kind() == "parameter_declaration" {
            // Count names if multiple (e.g. `a, b string`)
            let mut name_count = 0;
            let mut type_str = String::new();

            let mut decl_cursor = child.walk();
            for decl_child in child.children(&mut decl_cursor) {
                if decl_child.kind() == "identifier" {
                    name_count += 1;
                } else if decl_child.kind() != "," && !decl_child.kind().is_empty() {
                    type_str = node_text(decl_child, src);
                }
            }

            if type_str.is_empty() {
                type_str = node_text(child, src);
            }

            let repetitions = name_count.max(1);
            for _ in 0..repetitions {
                types.push(type_str.clone());
            }
        }
    }
    types
}

fn extract_result_types(result_node: Option<Node>, src: &[u8]) -> Vec<String> {
    let r = match result_node {
        Some(node) => node,
        None => return Vec::new(),
    };

    if r.kind() == "parameter_list" {
        extract_parameter_types(r, src)
    } else {
        vec![node_text(r, src)]
    }
}

fn extract_types(
    node: Node,
    src: &[u8],
    types_out: &mut Vec<GoType>,
    ifaces_out: &mut Vec<GoInterfaceDef>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_spec" {
            let name = child
                .child_by_field_name("name")
                .map(|n| node_text(n, src))
                .unwrap_or_default();

            let type_node = child.child_by_field_name("type");
            let kind = match type_node.map(|t| t.kind()) {
                Some("struct_type") => "STRUCT",
                Some("interface_type") => "INTERFACE",
                _ => "TYPE",
            };

            let full = node_text(child, src);
            let signature = full.lines().next().unwrap_or(&full).to_string();

            if !name.is_empty() {
                types_out.push(GoType {
                    name: name.clone(),
                    kind: kind.to_string(),
                    line: child.start_position().row + 1,
                    end_line: child.end_position().row + 1,
                    signature,
                });

                if kind == "INTERFACE" {
                    if let Some(tn) = type_node {
                        let mut iface_methods = Vec::new();
                        let mut iface_cursor = tn.walk();
                        for elem in tn.children(&mut iface_cursor) {
                            if elem.kind() == "method_elem" || elem.kind() == "method_spec" {
                                if let Some(m_name_node) = elem.child_by_field_name("name") {
                                    let m_name = node_text(m_name_node, src);
                                    let params = elem
                                        .child_by_field_name("parameters")
                                        .map(|p| extract_parameter_types(p, src))
                                        .unwrap_or_default();
                                    let results = extract_result_types(
                                        elem.child_by_field_name("result"),
                                        src,
                                    );

                                    iface_methods.push(GoMethodSignature {
                                        name: m_name,
                                        params,
                                        results,
                                    });
                                }
                            }
                        }
                        ifaces_out.push(GoInterfaceDef {
                            name,
                            methods: iface_methods,
                            line: child.start_position().row + 1,
                            end_line: child.end_position().row + 1,
                        });
                    }
                }
            }
        }
    }
}

fn extract_function(node: Node, src: &[u8]) -> Option<GoFunction> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, src);
    let full = node_text(node, src);
    let signature = full.lines().next().unwrap_or(&full).to_string();

    Some(GoFunction {
        name,
        line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
    })
}

fn extract_method(node: Node, src: &[u8]) -> Option<GoMethod> {
    let name_node = node.child_by_field_name("name")?;
    let method_name = node_text(name_node, src);

    let receiver_node = node.child_by_field_name("receiver")?;
    let mut receiver_type = String::new();
    let mut is_pointer_receiver = false;

    let mut cursor = receiver_node.walk();
    for child in receiver_node.children(&mut cursor) {
        if child.kind() == "parameter_declaration" {
            if let Some(type_node) = child.child_by_field_name("type") {
                if type_node.kind() == "pointer_type" {
                    is_pointer_receiver = true;
                    let mut pt_cursor = type_node.walk();
                    for pt_child in type_node.children(&mut pt_cursor) {
                        if pt_child.kind() == "type_identifier" {
                            receiver_type = node_text(pt_child, src);
                            break;
                        }
                    }
                } else if type_node.kind() == "type_identifier" {
                    receiver_type = node_text(type_node, src);
                }
            }
        }
    }

    if receiver_type.is_empty() {
        let raw_recv = node_text(receiver_node, src);
        let trimmed = raw_recv.trim_matches(|c| c == '(' || c == ')').trim();
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if let Some(t) = parts.last() {
            if t.starts_with('*') {
                is_pointer_receiver = true;
                receiver_type = t.trim_start_matches('*').to_string();
            } else {
                receiver_type = t.to_string();
            }
        }
    }

    let params = node
        .child_by_field_name("parameters")
        .map(|p| extract_parameter_types(p, src))
        .unwrap_or_default();
    let results = extract_result_types(node.child_by_field_name("result"), src);

    let canonical_name = format!("{receiver_type}.{method_name}");
    let full = node_text(node, src);
    let signature = full.lines().next().unwrap_or(&full).to_string();

    Some(GoMethod {
        name: canonical_name,
        receiver_type,
        is_pointer_receiver,
        method_name: method_name.clone(),
        signature_def: GoMethodSignature {
            name: method_name,
            params,
            results,
        },
        line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
    })
}

/// Convert parsed Go AST into Engram Symbols and Relationships.
/// Performs dependency resolution with origin classification,
/// two-stage interface implementation matching, and test conventions.
pub fn convert_go_ast_to_engram_with_root(
    root: &Path,
    rel_path: &str,
    ast: &GoFileAst,
    go_mod_doc: Option<&GoModDocument>,
) -> (Vec<Symbol>, Vec<Relationship>) {
    let mut symbols = Vec::new();
    let mut rels = Vec::new();

    // 1. Package symbol
    if let Some(ref pkg) = ast.package {
        let stable = hash(format!("{rel_path}\0MODULE\0{}\0", pkg.name).as_bytes());
        symbols.push(Symbol {
            id: format!("ent_{}", &stable[..24]),
            path: rel_path.to_string(),
            name: pkg.name.clone(),
            kind: "MODULE".to_string(),
            line: pkg.line,
            end_line: pkg.line,
            signature: format!("package {}", pkg.name),
            fingerprint: hash(pkg.name.as_bytes()),
        });
    }

    // 2. Struct and Interface types
    for t in &ast.types {
        let stable = hash(format!("{rel_path}\0{}\0{}\0", t.kind, t.name).as_bytes());
        symbols.push(Symbol {
            id: format!("ent_{}", &stable[..24]),
            path: rel_path.to_string(),
            name: t.name.clone(),
            kind: t.kind.clone(),
            line: t.line,
            end_line: t.end_line,
            signature: t.signature.clone(),
            fingerprint: hash(t.signature.as_bytes()),
        });
    }

    // 3. Free functions
    for f in &ast.functions {
        let stable = hash(format!("{rel_path}\0FUNCTION\0{}\0", f.name).as_bytes());
        symbols.push(Symbol {
            id: format!("ent_{}", &stable[..24]),
            path: rel_path.to_string(),
            name: f.name.clone(),
            kind: "FUNCTION".to_string(),
            line: f.line,
            end_line: f.end_line,
            signature: f.signature.clone(),
            fingerprint: hash(f.signature.as_bytes()),
        });
    }

    // 4. Methods
    for m in &ast.methods {
        let stable = hash(format!("{rel_path}\0METHOD\0{}\0", m.name).as_bytes());
        symbols.push(Symbol {
            id: format!("ent_{}", &stable[..24]),
            path: rel_path.to_string(),
            name: m.name.clone(),
            kind: "METHOD".to_string(),
            line: m.line,
            end_line: m.end_line,
            signature: m.signature.clone(),
            fingerprint: hash(m.signature.as_bytes()),
        });
    }

    // 5. Imports -> Relationships with explicit origin and evidence tiers
    for imp in &ast.imports {
        let (origin, targets) = resolve_go_dependency_origin(root, rel_path, &imp.path, go_mod_doc);
        for target in targets {
            let (evidence, confidence) = match origin {
                PackageOrigin::Unresolved => ("UNRESOLVED", 0.0),
                PackageOrigin::Vendored => ("VENDORED", 0.99),
                PackageOrigin::ReplacedLocal => ("REPLACED_LOCAL", 0.99),
                PackageOrigin::ReplacedExternal => ("REPLACED_EXTERNAL", 0.95),
                PackageOrigin::Local => ("LOCAL", 0.99),
                PackageOrigin::External => ("EXTERNAL", 0.95),
            };

            rels.push(Relationship {
                source: rel_path.to_string(),
                target,
                kind: "IMPORTS".to_string(),
                evidence: evidence.to_string(),
                confidence,
                line: imp.line,
            });
        }
    }

    // 6. Two-stage Interface Implementation matching (`IMPLEMENTS`)
    // Group methods by receiver struct
    let mut struct_names: HashSet<&str> = HashSet::new();
    for t in &ast.types {
        if t.kind == "STRUCT" {
            struct_names.insert(&t.name);
        }
    }

    for struct_name in struct_names {
        let methods_for_struct: Vec<&GoMethod> = ast
            .methods
            .iter()
            .filter(|m| m.receiver_type == struct_name)
            .collect();

        for iface in &ast.interfaces {
            if check_struct_implements_interface(&methods_for_struct, iface) {
                rels.push(Relationship {
                    source: format!("{rel_path}::{struct_name}"),
                    target: format!("{rel_path}::{}", iface.name),
                    kind: "IMPLEMENTS".to_string(),
                    evidence: "EXTRACTED".to_string(),
                    confidence: 0.95,
                    line: iface.line,
                });
            }
        }
    }

    // 7. Test file conventions (*_test.go -> target *.go)
    if rel_path.ends_with("_test.go") {
        if let Some(target) = rel_path.strip_suffix("_test.go").map(|p| format!("{p}.go")) {
            rels.push(Relationship {
                source: rel_path.to_string(),
                target,
                kind: "TESTS".to_string(),
                evidence: "NAMING_CONVENTION".to_string(),
                confidence: 0.95,
                line: 1,
            });
        }
    }

    (symbols, rels)
}

/// Infer cross-file structural interface satisfaction (`IMPLEMENTS`) across multiple Go files.
pub fn infer_cross_file_go_interfaces(files: &[(&str, &GoFileAst)]) -> Vec<Relationship> {
    let mut rels = Vec::new();

    for (path_iface, ast_iface) in files {
        for iface in &ast_iface.interfaces {
            for (path_struct, ast_struct) in files {
                if path_iface == path_struct {
                    continue; // Intra-file already handled in convert_go_ast_to_engram_with_root
                }

                let mut struct_names: HashSet<&str> = HashSet::new();
                for t in &ast_struct.types {
                    if t.kind == "STRUCT" {
                        struct_names.insert(&t.name);
                    }
                }

                for struct_name in struct_names {
                    let methods_for_struct: Vec<&GoMethod> = ast_struct
                        .methods
                        .iter()
                        .filter(|m| m.receiver_type == struct_name)
                        .collect();

                    if check_struct_implements_interface(&methods_for_struct, iface) {
                        let line = ast_struct
                            .types
                            .iter()
                            .find(|t| t.name == struct_name)
                            .map(|t| t.line)
                            .unwrap_or(1);

                        rels.push(Relationship {
                            source: format!("{path_struct}::{struct_name}"),
                            target: format!("{path_iface}::{}", iface.name),
                            kind: "IMPLEMENTS".to_string(),
                            evidence: "EXTRACTED".to_string(),
                            confidence: 0.95,
                            line,
                        });
                    }
                }
            }
        }
    }

    rels
}

/// Backwards-compatible import target resolver.
pub fn resolve_go_import_targets(
    current_file: &str,
    import_path: &str,
    go_module: Option<&str>,
    workspace_files: Option<&HashSet<String>>,
) -> Vec<String> {
    let dummy_root = Path::new(".");
    let doc = go_module.map(|m| GoModDocument {
        module: Some(m.to_string()),
        ..Default::default()
    });

    let (origin, mut targets) =
        resolve_go_dependency_origin(dummy_root, current_file, import_path, doc.as_ref());

    if let Some(ws) = workspace_files {
        if origin == PackageOrigin::Local {
            let mut matched = Vec::new();
            for t in &targets {
                for f in ws {
                    if (f.starts_with(&format!("{t}/")) || f == t) && f.ends_with(".go") {
                        matched.push(f.clone());
                    }
                }
            }
            if !matched.is_empty() {
                targets = matched;
            }
        }
    }

    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_go_mod_directives() {
        let content = r#"
module github.com/acme/shop

go 1.22
toolchain go1.22.5

require (
    github.com/google/uuid v1.6.0
    github.com/gin-gonic/gin v1.9.1 // indirect
)

replace (
    github.com/acme/common => ./local/common
    golang.org/x/crypto => github.com/golang/crypto v0.0.0-20201221181555-eec23a3978ad
)

exclude (
    github.com/bad/pkg v1.2.0
)
"#;

        let doc = parse_full_go_mod(content);
        assert_eq!(doc.module, Some("github.com/acme/shop".to_string()));
        assert_eq!(doc.go_version, Some("1.22".to_string()));
        assert_eq!(doc.toolchain, Some("go1.22.5".to_string()));

        assert_eq!(doc.requires.len(), 2);
        assert_eq!(doc.requires[0].path, "github.com/google/uuid");
        assert_eq!(doc.requires[0].version, "v1.6.0");
        assert!(!doc.requires[0].indirect);
        assert!(doc.requires[1].indirect);

        assert_eq!(doc.replaces.len(), 2);
        assert_eq!(doc.replaces[0].old_path, "github.com/acme/common");
        assert_eq!(doc.replaces[0].new_path, "./local/common");
        assert!(doc.replaces[0].is_local_dir);

        assert_eq!(doc.replaces[1].old_path, "golang.org/x/crypto");
        assert_eq!(doc.replaces[1].new_path, "github.com/golang/crypto");
        assert!(!doc.replaces[1].is_local_dir);

        assert_eq!(doc.excludes.len(), 1);
        assert!(doc.excludes[0].contains("github.com/bad/pkg"));
    }

    #[test]
    fn test_dependency_origin_classification() {
        let temp_dir =
            std::env::temp_dir().join(format!("engram_test_dep_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(temp_dir.join("vendor/github.com/vendored/pkg")).unwrap();
        std::fs::create_dir_all(temp_dir.join("internal/payment")).unwrap();
        std::fs::create_dir_all(temp_dir.join("local/common")).unwrap();

        let doc = GoModDocument {
            module: Some("github.com/acme/shop".to_string()),
            requires: vec![GoRequire {
                path: "github.com/google/uuid".to_string(),
                version: "v1.6.0".to_string(),
                indirect: false,
            }],
            replaces: vec![
                GoReplace {
                    old_path: "github.com/acme/common".to_string(),
                    old_version: None,
                    new_path: "./local/common".to_string(),
                    new_version: None,
                    is_local_dir: true,
                },
                GoReplace {
                    old_path: "golang.org/x/crypto".to_string(),
                    old_version: None,
                    new_path: "github.com/golang/crypto".to_string(),
                    new_version: Some("v0.1.0".to_string()),
                    is_local_dir: false,
                },
            ],
            ..Default::default()
        };

        // 1. Vendored
        let (origin1, _) = resolve_go_dependency_origin(
            &temp_dir,
            "service.go",
            "github.com/vendored/pkg",
            Some(&doc),
        );
        assert_eq!(origin1, PackageOrigin::Vendored);

        // 2. Replaced Local
        let (origin2, _) = resolve_go_dependency_origin(
            &temp_dir,
            "service.go",
            "github.com/acme/common",
            Some(&doc),
        );
        assert_eq!(origin2, PackageOrigin::ReplacedLocal);

        // 3. Replaced External
        let (origin3, _) = resolve_go_dependency_origin(
            &temp_dir,
            "service.go",
            "golang.org/x/crypto",
            Some(&doc),
        );
        assert_eq!(origin3, PackageOrigin::ReplacedExternal);

        // 4. Local Module
        let (origin4, _) = resolve_go_dependency_origin(
            &temp_dir,
            "service.go",
            "github.com/acme/shop/internal/payment",
            Some(&doc),
        );
        assert_eq!(origin4, PackageOrigin::Local);

        // 5. External (Required or Stdlib)
        let (origin5a, _) = resolve_go_dependency_origin(
            &temp_dir,
            "service.go",
            "github.com/google/uuid",
            Some(&doc),
        );
        assert_eq!(origin5a, PackageOrigin::External);
        let (origin5b, _) = resolve_go_dependency_origin(&temp_dir, "service.go", "io", Some(&doc));
        assert_eq!(origin5b, PackageOrigin::External);

        // 6. Explicitly UNRESOLVED (missing/unknown)
        let (origin6, _) = resolve_go_dependency_origin(
            &temp_dir,
            "service.go",
            "github.com/unknown/ghost",
            Some(&doc),
        );
        assert_eq!(origin6, PackageOrigin::Unresolved);

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_two_stage_interface_matching() {
        let source_match = r#"
package orders

type Exporter interface {
    ExportCSV(path string) error
}

type OrderService struct {}

func (s *OrderService) ExportCSV(path string) error {
    return nil
}
"#;
        let ast1 = extract_go_ast(source_match).unwrap();
        assert_eq!(ast1.interfaces.len(), 1);
        let iface = &ast1.interfaces[0];
        let methods: Vec<&GoMethod> = ast1.methods.iter().collect();

        // MATCH
        assert!(check_struct_implements_interface(&methods, iface));

        // Signature MISMATCH (different param type)
        let source_mismatch = r#"
package orders

type Exporter interface {
    ExportCSV(path string) error
}

type OrderService struct {}

func (s *OrderService) ExportCSV(count int) error {
    return nil
}
"#;
        let ast2 = extract_go_ast(source_mismatch).unwrap();
        let iface2 = &ast2.interfaces[0];
        let methods2: Vec<&GoMethod> = ast2.methods.iter().collect();

        // Must NOT implement despite matching method name!
        assert!(!check_struct_implements_interface(&methods2, iface2));
    }
}
