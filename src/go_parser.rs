//! Go Language Parser: Tree-sitter AST extraction, canonical receiver-normalized symbols,
//! go.mod local module resolution, and test conventions.

use anyhow::{Result, anyhow};
use std::collections::HashSet;
use std::path::Path;
use tree_sitter::{Node, Parser as TsParser};

use crate::analyzer::{Relationship, Symbol, node_text};
use crate::storage::hash;

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
    pub line: usize,
    pub end_line: usize,
    pub signature: String,
}

#[derive(Debug, Clone, Default)]
pub struct GoFileAst {
    pub package: Option<GoPackage>,
    pub imports: Vec<GoImport>,
    pub types: Vec<GoType>,
    pub functions: Vec<GoFunction>,
    pub methods: Vec<GoMethod>,
}

/// Parse `module <path>` from a go.mod file.
pub fn parse_go_mod(contents: &str) -> Option<String> {
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.starts_with("module ") {
            let mod_part = trimmed["module ".len()..].trim();
            let clean = mod_part.split("//").next().unwrap_or(mod_part).trim();
            let clean = clean.trim_matches(|c| c == '"' || c == '\'');
            if !clean.is_empty() {
                return Some(clean.to_string());
            }
        }
    }
    None
}

/// Locate and read `go.mod` in root directory if present.
pub fn load_root_go_mod(root: &Path) -> Option<String> {
    let mod_path = root.join("go.mod");
    if mod_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&mod_path) {
            return parse_go_mod(&content);
        }
    }
    None
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
        trimmed.trim_start_matches(|c| c == '*' || c == '&').trim().to_string()
    }
}

/// Checks whether an expected contract symbol matches an actual symbol name.
pub fn matches_go_contract_symbol(expected: &str, actual: &str) -> bool {
    let norm_exp = normalize_go_symbol_name(expected);
    let norm_act = normalize_go_symbol_name(actual);
    norm_exp == norm_act
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
            extract_types(node, src, &mut ast.types);
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

fn extract_types(node: Node, src: &[u8], out: &mut Vec<GoType>) {
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
                out.push(GoType {
                    name,
                    kind: kind.to_string(),
                    line: child.start_position().row + 1,
                    end_line: child.end_position().row + 1,
                    signature,
                });
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

    // Search parameter_declaration inside parameter_list
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
        // Fallback: extract from raw text between parentheses
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

    let canonical_name = format!("{receiver_type}.{method_name}");
    let full = node_text(node, src);
    let signature = full.lines().next().unwrap_or(&full).to_string();

    Some(GoMethod {
        name: canonical_name,
        receiver_type,
        is_pointer_receiver,
        method_name,
        line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
    })
}

/// Convert parsed Go AST into Engram Symbols and Relationships for storage.
pub fn convert_go_ast_to_engram(
    rel_path: &str,
    ast: &GoFileAst,
    go_module: Option<&str>,
    workspace_files: Option<&HashSet<String>>,
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

    // 4. Methods (canonical receiver name)
    for m in &ast.methods {
        let stable = hash(format!("{rel_path}\0METHOD\0{}\0", m.name).as_bytes());
        symbols.push(Symbol {
            id: format!("ent_{}", &stable[..24]),
            path: rel_path.to_string(),
            name: m.name.clone(), // "OrderService.ExportCSV"
            kind: "METHOD".to_string(),
            line: m.line,
            end_line: m.end_line,
            signature: m.signature.clone(),
            fingerprint: hash(m.signature.as_bytes()),
        });
    }

    // 5. Imports -> Relationships
    for imp in &ast.imports {
        let targets = resolve_go_import_targets(rel_path, &imp.path, go_module, workspace_files);
        for target in targets {
            rels.push(Relationship {
                source: rel_path.to_string(),
                target,
                kind: "IMPORTS".to_string(),
                evidence: "TREE_SITTER_IMPORT_SYNTAX".to_string(),
                confidence: 0.99,
                line: imp.line,
            });
        }
    }

    // 6. Test file conventions (*_test.go -> target *.go)
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

/// Helper that inspects root disk filesystem to resolve package directory imports to workspace files.
pub fn convert_go_ast_to_engram_with_root(
    root: &Path,
    rel_path: &str,
    ast: &GoFileAst,
    go_module: Option<&str>,
) -> (Vec<Symbol>, Vec<Relationship>) {
    let mut workspace_files = HashSet::new();
    for imp in &ast.imports {
        let clean_path = if let Some(module) = go_module {
            if imp.path.starts_with(&format!("{module}/")) {
                Some(&imp.path[module.len() + 1..])
            } else if imp.path == module {
                Some(".")
            } else {
                None
            }
        } else {
            None
        };

        if let Some(sub) = clean_path {
            let target_dir = root.join(sub);
            if target_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&target_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() && p.extension().map(|e| e == "go").unwrap_or(false) {
                            if let Ok(rel_target) = p.strip_prefix(root) {
                                let norm = crate::analyzer::normalize_rel_path(rel_target);
                                workspace_files.insert(norm);
                            }
                        }
                    }
                }
            }
        } else if imp.path.starts_with('.') {
            let parent = Path::new(rel_path).parent().unwrap_or(Path::new(""));
            let joined = parent.join(&imp.path);
            let target_dir = root.join(&joined);
            if target_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&target_dir) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() && p.extension().map(|e| e == "go").unwrap_or(false) {
                            if let Ok(rel_target) = p.strip_prefix(root) {
                                let norm = crate::analyzer::normalize_rel_path(rel_target);
                                workspace_files.insert(norm);
                            }
                        }
                    }
                }
            }
        }
    }

    convert_go_ast_to_engram(
        rel_path,
        ast,
        go_module,
        if workspace_files.is_empty() {
            None
        } else {
            Some(&workspace_files)
        },
    )
}

/// Resolve Go import specifiers:
/// - Relative imports ("./foo", "../bar") resolved to normalized relative path.
/// - Local module imports ("github.com/acme/orders/internal/payment") mapped via go.mod module declaration.
/// - Resolves to existing workspace files within the package if available, enabling 1-hop graph grounding.
pub fn resolve_go_import_targets(
    current_file: &str,
    import_path: &str,
    go_module: Option<&str>,
    workspace_files: Option<&HashSet<String>>,
) -> Vec<String> {
    let mut results = Vec::new();

    // 1. Relative import
    if import_path.starts_with('.') {
        let parent = Path::new(current_file).parent().unwrap_or(Path::new(""));
        let joined = parent.join(import_path);
        let normalized = crate::analyzer::normalize_rel_path(&joined);

        if let Some(files) = workspace_files {
            let mut found = false;
            for f in files {
                if (f.starts_with(&format!("{normalized}/")) || f == &normalized) && f.ends_with(".go") {
                    results.push(f.clone());
                    found = true;
                }
            }
            if !found {
                results.push(normalized);
            }
        } else {
            results.push(normalized);
        }
        return results;
    }

    // 2. Local module resolution using go.mod
    if let Some(module) = go_module {
        if import_path == module {
            if let Some(files) = workspace_files {
                for f in files {
                    if !f.contains('/') && f.ends_with(".go") {
                        results.push(f.clone());
                    }
                }
            }
            if results.is_empty() {
                results.push(".".to_string());
            }
            return results;
        } else if import_path.starts_with(&format!("{module}/")) {
            let sub_path = &import_path[module.len() + 1..];
            if let Some(files) = workspace_files {
                let mut found = false;
                for f in files {
                    if (f.starts_with(&format!("{sub_path}/")) || f == sub_path) && f.ends_with(".go") {
                        results.push(f.clone());
                        found = true;
                    }
                }
                if !found {
                    results.push(sub_path.to_string());
                }
            } else {
                results.push(sub_path.to_string());
            }
            return results;
        }
    }

    // 3. Third-party or stdlib import
    results.push(import_path.to_string());
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_go_mod_module_declaration() {
        let content = r#"
module github.com/acme/orders

go 1.22

require (
    github.com/google/uuid v1.6.0
)
"#;
        assert_eq!(
            parse_go_mod(content),
            Some("github.com/acme/orders".to_string())
        );
    }

    #[test]
    fn test_extract_go_ast_complete() {
        let source = r#"
package orders

import (
    "io"
    "github.com/acme/orders/internal/payment"
)

type OrderService struct {
    client *payment.Client
}

type Exporter interface {
    Export(w io.Writer) error
}

func (s *OrderService) ExportCSV(w io.Writer) error {
    return nil
}

func (s OrderService) ValueMethod() {}

func HelperFunc() string {
    return "ok"
}
"#;

        let ast = extract_go_ast(source).unwrap();
        assert_eq!(ast.package.as_ref().map(|p| p.name.as_str()), Some("orders"));

        assert_eq!(ast.imports.len(), 2);
        assert_eq!(ast.imports[0].path, "io");
        assert_eq!(ast.imports[1].path, "github.com/acme/orders/internal/payment");

        assert_eq!(ast.types.len(), 2);
        assert_eq!(ast.types[0].name, "OrderService");
        assert_eq!(ast.types[0].kind, "STRUCT");
        assert_eq!(ast.types[1].name, "Exporter");
        assert_eq!(ast.types[1].kind, "INTERFACE");

        assert_eq!(ast.functions.len(), 1);
        assert_eq!(ast.functions[0].name, "HelperFunc");

        assert_eq!(ast.methods.len(), 2);
        // Pointer receiver method
        assert_eq!(ast.methods[0].name, "OrderService.ExportCSV");
        assert_eq!(ast.methods[0].receiver_type, "OrderService");
        assert_eq!(ast.methods[0].method_name, "ExportCSV");
        assert!(ast.methods[0].is_pointer_receiver);

        // Value receiver method
        assert_eq!(ast.methods[1].name, "OrderService.ValueMethod");
        assert_eq!(ast.methods[1].receiver_type, "OrderService");
        assert!(!ast.methods[1].is_pointer_receiver);
    }

    #[test]
    fn test_receiver_normalization_and_symbol_matching() {
        assert_eq!(
            normalize_go_symbol_name("*OrderService.ExportCSV"),
            "OrderService.ExportCSV"
        );
        assert_eq!(
            normalize_go_symbol_name("OrderService.ExportCSV"),
            "OrderService.ExportCSV"
        );

        // MATCH
        assert!(matches_go_contract_symbol(
            "OrderService.ExportCSV",
            "OrderService.ExportCSV"
        ));
        assert!(matches_go_contract_symbol(
            "*OrderService.ExportCSV",
            "OrderService.ExportCSV"
        ));

        // NO MATCH
        assert!(!matches_go_contract_symbol(
            "OrderService.ExportCSV",
            "OrderService.Delete"
        ));
    }

    #[test]
    fn test_local_import_resolution_with_go_mod() {
        let mut workspace = HashSet::new();
        workspace.insert("internal/payment/client.go".to_string());
        workspace.insert("internal/payment/types.go".to_string());

        let module = Some("github.com/acme/orders");
        let targets = resolve_go_import_targets(
            "internal/orders/service.go",
            "github.com/acme/orders/internal/payment",
            module,
            Some(&workspace),
        );

        assert_eq!(targets.len(), 2);
        assert!(targets.contains(&"internal/payment/client.go".to_string()));
        assert!(targets.contains(&"internal/payment/types.go".to_string()));
    }
}
