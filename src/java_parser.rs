//! Java / JVM Language Parser: Tree-sitter AST extraction,
//! Canonical symbol naming (e.g. `InvoiceService.createInvoice`),
//! Class, interface, enum, record, method, constructor, and annotation extraction,
//! Maven `pom.xml` and Gradle `build.gradle` dependency parsing,
//! Interface implementation matching, and test convention linking.

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tree_sitter::{Node, Parser as TsParser};

use crate::analyzer::{node_text, Relationship, Symbol};
use crate::storage::hash;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaImport {
    pub path: String,
    pub is_static: bool,
    pub is_wildcard: bool,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaType {
    pub name: String,
    pub kind: String, // "CLASS", "INTERFACE", "RECORD", "ENUM", "ANNOTATION"
    pub superclass: Option<String>,
    pub interfaces: Vec<String>,
    pub annotations: Vec<String>,
    pub line: usize,
    pub end_line: usize,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaMethod {
    pub name: String,        // Canonical name: "InvoiceService.createInvoice"
    pub class_name: String,  // "InvoiceService"
    pub method_name: String, // "createInvoice"
    pub return_type: Option<String>,
    pub params: Vec<String>,
    pub annotations: Vec<String>,
    pub is_constructor: bool,
    pub is_test: bool,
    pub line: usize,
    pub end_line: usize,
    pub signature: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaFileAst {
    pub package: Option<String>,
    pub imports: Vec<JavaImport>,
    pub types: Vec<JavaType>,
    pub methods: Vec<JavaMethod>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MavenDependency {
    pub group_id: String,
    pub artifact_id: String,
    pub version: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PomDocument {
    pub group_id: Option<String>,
    pub artifact_id: Option<String>,
    pub version: Option<String>,
    pub dependencies: Vec<MavenDependency>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GradleDependency {
    pub configuration: String, // "implementation", "testImplementation", etc.
    pub notation: String,      // "org.springframework.boot:spring-boot-starter:3.2.0"
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GradleDocument {
    pub group: Option<String>,
    pub version: Option<String>,
    pub plugins: Vec<String>,
    pub dependencies: Vec<GradleDependency>,
}

/// Extract Java AST from source text using Tree-sitter.
pub fn extract_java_ast(source: &str) -> Result<JavaFileAst> {
    let mut parser = TsParser::new();
    let lang = tree_sitter_java::LANGUAGE.into();
    parser.set_language(&lang)?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("tree-sitter-java returned no tree"))?;

    let root = tree.root_node();
    let src = source.as_bytes();
    let mut ast = JavaFileAst::default();

    visit_java_root(root, src, &mut ast);
    Ok(ast)
}

fn visit_java_root(root: Node, src: &[u8], ast: &mut JavaFileAst) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "package_declaration" => {
                let text = node_text(child, src);
                let pkg = text
                    .trim_start_matches("package")
                    .trim()
                    .trim_end_matches(';')
                    .trim();
                if !pkg.is_empty() {
                    ast.package = Some(pkg.to_string());
                }
            }
            "import_declaration" => {
                let line = child.start_position().row + 1;
                let text = node_text(child, src);
                let is_static = text.contains("static ");
                let clean = text
                    .trim_start_matches("import")
                    .trim()
                    .trim_start_matches("static")
                    .trim()
                    .trim_end_matches(';')
                    .trim();
                let is_wildcard = clean.ends_with(".*");
                let path = clean.trim_end_matches(".*").to_string();
                if !path.is_empty() {
                    ast.imports.push(JavaImport {
                        path,
                        is_static,
                        is_wildcard,
                        line,
                    });
                }
            }
            "class_declaration"
            | "interface_declaration"
            | "record_declaration"
            | "enum_declaration"
            | "annotation_type_declaration" => {
                extract_type_decl(child, src, ast, None);
            }
            _ => {}
        }
    }
}

fn extract_type_decl(node: Node, src: &[u8], ast: &mut JavaFileAst, enclosing_type: Option<&str>) {
    let kind = match node.kind() {
        "class_declaration" => "CLASS",
        "interface_declaration" => "INTERFACE",
        "record_declaration" => "RECORD",
        "enum_declaration" => "ENUM",
        "annotation_type_declaration" => "ANNOTATION",
        _ => return,
    };

    let raw_name = node
        .child_by_field_name("name")
        .map(|n| node_text(n, src))
        .unwrap_or_default();
    if raw_name.is_empty() {
        return;
    }

    let type_name = match enclosing_type {
        Some(parent) => format!("{parent}.{raw_name}"),
        None => raw_name,
    };

    let mut superclass = None;
    let mut interfaces = Vec::new();
    let mut annotations = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "modifiers" => {
                let mut mod_cursor = child.walk();
                for m_child in child.children(&mut mod_cursor) {
                    if m_child.kind() == "marker_annotation" || m_child.kind() == "annotation" {
                        let ann = node_text(m_child, src);
                        annotations.push(ann);
                    }
                }
            }
            "superclass" => {
                let mut sc_cursor = child.walk();
                for sc_child in child.children(&mut sc_cursor) {
                    if sc_child.kind() == "type_identifier" {
                        superclass = Some(node_text(sc_child, src));
                        break;
                    }
                }
                if superclass.is_none() {
                    let raw = node_text(child, src);
                    let clean = raw.trim_start_matches("extends").trim().to_string();
                    if !clean.is_empty() {
                        superclass = Some(clean);
                    }
                }
            }
            "super_interfaces" => {
                let mut if_cursor = child.walk();
                for if_child in child.children(&mut if_cursor) {
                    if if_child.kind() == "type_list" {
                        let mut tl_cursor = if_child.walk();
                        for tl_child in if_child.children(&mut tl_cursor) {
                            if tl_child.kind() == "type_identifier" {
                                interfaces.push(node_text(tl_child, src));
                            }
                        }
                    } else if if_child.kind() == "type_identifier" {
                        interfaces.push(node_text(if_child, src));
                    }
                }
                if interfaces.is_empty() {
                    let raw = node_text(child, src);
                    let clean = raw.trim_start_matches("implements").trim();
                    for part in clean.split(',') {
                        let p = part.trim();
                        if !p.is_empty() {
                            interfaces.push(p.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let full_text = node_text(node, src);
    let signature = full_text.lines().next().unwrap_or(&full_text).to_string();

    ast.types.push(JavaType {
        name: type_name.clone(),
        kind: kind.to_string(),
        superclass,
        interfaces,
        annotations,
        line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
    });

    // Extract body members (methods, constructors, nested types)
    let body_node = node.child_by_field_name("body").or_else(|| {
        let mut c = node.walk();
        node.children(&mut c).find(|n| {
            n.kind() == "class_body"
                || n.kind() == "interface_body"
                || n.kind() == "enum_body"
                || n.kind() == "annotation_type_body"
        })
    });

    if let Some(body) = body_node {
        let mut b_cursor = body.walk();
        for b_child in body.children(&mut b_cursor) {
            match b_child.kind() {
                "method_declaration" => {
                    if let Some(m) = extract_method(b_child, src, &type_name, false) {
                        ast.methods.push(m);
                    }
                }
                "constructor_declaration" => {
                    if let Some(m) = extract_method(b_child, src, &type_name, true) {
                        ast.methods.push(m);
                    }
                }
                "class_declaration"
                | "interface_declaration"
                | "record_declaration"
                | "enum_declaration" => {
                    extract_type_decl(b_child, src, ast, Some(&type_name));
                }
                _ => {}
            }
        }
    }
}

fn extract_method(
    node: Node,
    src: &[u8],
    class_name: &str,
    is_constructor: bool,
) -> Option<JavaMethod> {
    let raw_name = node
        .child_by_field_name("name")
        .map(|n| node_text(n, src))
        .unwrap_or_default();
    if raw_name.is_empty() {
        return None;
    }

    let canonical_name = format!("{class_name}.{raw_name}");

    let mut return_type = None;
    if !is_constructor {
        if let Some(t_node) = node.child_by_field_name("type") {
            return_type = Some(node_text(t_node, src));
        }
    }

    let mut params = Vec::new();
    if let Some(params_node) = node.child_by_field_name("parameters") {
        let mut p_cursor = params_node.walk();
        for p_child in params_node.children(&mut p_cursor) {
            if p_child.kind() == "formal_parameter" || p_child.kind() == "spread_parameter" {
                if let Some(t) = p_child.child_by_field_name("type") {
                    params.push(node_text(t, src));
                } else {
                    let raw = node_text(p_child, src);
                    let parts: Vec<&str> = raw.split_whitespace().collect();
                    if let Some(first) = parts.first() {
                        params.push(first.to_string());
                    }
                }
            }
        }
    }

    let mut annotations = Vec::new();
    let mut is_test = false;

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "modifiers" {
            let mut mod_cursor = child.walk();
            for m_child in child.children(&mut mod_cursor) {
                if m_child.kind() == "marker_annotation" || m_child.kind() == "annotation" {
                    let ann = node_text(m_child, src);
                    if ann.contains("Test") {
                        is_test = true;
                    }
                    annotations.push(ann);
                }
            }
        }
    }

    if raw_name.starts_with("test") {
        is_test = true;
    }

    let full_text = node_text(node, src);
    let signature = full_text.lines().next().unwrap_or(&full_text).to_string();

    Some(JavaMethod {
        name: canonical_name,
        class_name: class_name.to_string(),
        method_name: raw_name,
        return_type,
        params,
        annotations,
        is_constructor,
        is_test,
        line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
    })
}

/// Parse Maven `pom.xml` dependencies and metadata.
pub fn parse_pom_xml(contents: &str) -> PomDocument {
    let mut doc = PomDocument::default();
    let mut in_dependencies = false;
    let mut current_dep: Option<MavenDependency> = None;

    for line in contents.lines() {
        let trimmed = line.trim();

        if trimmed == "<dependencies>" {
            in_dependencies = true;
            continue;
        }
        if trimmed == "</dependencies>" {
            in_dependencies = false;
            continue;
        }

        if in_dependencies {
            if trimmed == "<dependency>" {
                current_dep = Some(MavenDependency::default());
                continue;
            }
            if trimmed == "</dependency>" {
                if let Some(dep) = current_dep.take() {
                    if !dep.group_id.is_empty() && !dep.artifact_id.is_empty() {
                        doc.dependencies.push(dep);
                    }
                }
                continue;
            }

            if let Some(ref mut dep) = current_dep {
                if let Some(val) = extract_tag_value(trimmed, "groupId") {
                    dep.group_id = val;
                } else if let Some(val) = extract_tag_value(trimmed, "artifactId") {
                    dep.artifact_id = val;
                } else if let Some(val) = extract_tag_value(trimmed, "version") {
                    dep.version = Some(val);
                } else if let Some(val) = extract_tag_value(trimmed, "scope") {
                    dep.scope = Some(val);
                }
            }
        } else {
            // Root project metadata
            if doc.group_id.is_none() {
                if let Some(val) = extract_tag_value(trimmed, "groupId") {
                    doc.group_id = Some(val);
                }
            }
            if doc.artifact_id.is_none() {
                if let Some(val) = extract_tag_value(trimmed, "artifactId") {
                    doc.artifact_id = Some(val);
                }
            }
            if doc.version.is_none() {
                if let Some(val) = extract_tag_value(trimmed, "version") {
                    doc.version = Some(val);
                }
            }
        }
    }

    doc
}

fn extract_tag_value(line: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    if let (Some(start), Some(end)) = (line.find(&open), line.find(&close)) {
        let val = &line[start + open.len()..end];
        return Some(val.trim().to_string());
    }
    None
}

/// Parse Gradle `build.gradle` dependencies and plugins.
pub fn parse_build_gradle(contents: &str) -> GradleDocument {
    let mut doc = GradleDocument::default();
    let mut in_dependencies = false;

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }

        if trimmed == "dependencies {" {
            in_dependencies = true;
            continue;
        }
        if in_dependencies && trimmed == "}" {
            in_dependencies = false;
            continue;
        }

        if in_dependencies {
            let configs = [
                "implementation",
                "testImplementation",
                "api",
                "compileOnly",
                "runtimeOnly",
                "annotationProcessor",
            ];
            for cfg in &configs {
                if trimmed.starts_with(cfg) {
                    let rest = trimmed[cfg.len()..].trim();
                    let notation = rest
                        .trim_matches(|c| c == '(' || c == ')' || c == '\'' || c == '"' || c == ';')
                        .trim();
                    if !notation.is_empty() {
                        doc.dependencies.push(GradleDependency {
                            configuration: cfg.to_string(),
                            notation: notation.to_string(),
                        });
                    }
                    break;
                }
            }
        } else {
            if trimmed.starts_with("group =") || trimmed.starts_with("group=") {
                let val = trimmed
                    .split('=')
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .trim_matches(|c| c == '\'' || c == '"');
                doc.group = Some(val.to_string());
            } else if trimmed.starts_with("version =") || trimmed.starts_with("version=") {
                let val = trimmed
                    .split('=')
                    .nth(1)
                    .unwrap_or("")
                    .trim()
                    .trim_matches(|c| c == '\'' || c == '"');
                doc.version = Some(val.to_string());
            } else if trimmed.starts_with("id '") || trimmed.starts_with("id \"") {
                let id = trimmed[3..]
                    .trim()
                    .trim_matches(|c| c == '\'' || c == '"' || c == ';');
                doc.plugins.push(id.to_string());
            }
        }
    }

    doc
}

/// Convert parsed Java AST into Engram Symbols and Relationships.
pub fn convert_java_ast_to_engram_with_root(
    root: &Path,
    rel_path: &str,
    ast: &JavaFileAst,
) -> (Vec<Symbol>, Vec<Relationship>) {
    let mut symbols = Vec::new();
    let mut rels = Vec::new();

    // 1. Package symbol
    if let Some(ref pkg) = ast.package {
        let stable = hash(format!("{rel_path}\0MODULE\0{}\0", pkg).as_bytes());
        symbols.push(Symbol {
            id: format!("ent_{}", &stable[..24]),
            path: rel_path.to_string(),
            name: pkg.clone(),
            kind: "MODULE".to_string(),
            line: 1,
            end_line: 1,
            signature: format!("package {pkg}"),
            fingerprint: hash(pkg.as_bytes()),
        });
    }

    // 2. Types (Classes, Interfaces, Records, Enums, Annotations)
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

        // Interface implementation relationships
        for iface in &t.interfaces {
            rels.push(Relationship {
                source: t.name.clone(),
                target: iface.clone(),
                kind: "IMPLEMENTS".to_string(),
                evidence: "JAVA_IMPLEMENTS_DECLARATION".to_string(),
                confidence: 0.99,
                line: t.line,
            });
        }

        // Superclass inheritance relationships
        if let Some(ref sc) = t.superclass {
            rels.push(Relationship {
                source: t.name.clone(),
                target: sc.clone(),
                kind: "EXTENDS".to_string(),
                evidence: "JAVA_EXTENDS_DECLARATION".to_string(),
                confidence: 0.99,
                line: t.line,
            });
        }
    }

    // 3. Methods & Constructors
    for m in &ast.methods {
        let kind = if m.is_constructor {
            "CONSTRUCTOR"
        } else {
            "METHOD"
        };
        let stable = hash(format!("{rel_path}\0{}\0{}\0", kind, m.name).as_bytes());
        symbols.push(Symbol {
            id: format!("ent_{}", &stable[..24]),
            path: rel_path.to_string(),
            name: m.name.clone(),
            kind: kind.to_string(),
            line: m.line,
            end_line: m.end_line,
            signature: m.signature.clone(),
            fingerprint: hash(m.signature.as_bytes()),
        });
    }

    // 4. Imports -> Relationships
    for imp in &ast.imports {
        let target_file = resolve_java_import_to_file(root, rel_path, &imp.path);
        let target = target_file.unwrap_or_else(|| imp.path.clone());
        let (evidence, confidence) = if target.ends_with(".java") {
            ("LOCAL_JAVA_SOURCE", 0.99)
        } else if imp.path.starts_with("java.") || imp.path.starts_with("javax.") {
            ("JAVA_STDLIB", 0.95)
        } else {
            ("EXTERNAL_DEPENDENCY", 0.90)
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

    // 5. Test to Target Association via naming convention
    let filename = Path::new(rel_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if filename.ends_with("Test.java") || filename.ends_with("Tests.java") {
        let stem = filename
            .trim_end_matches("Test.java")
            .trim_end_matches("Tests.java");
        let target_name = format!("{stem}.java");

        // Try to resolve in the same package or in main source tree
        if rel_path.contains("src/test/java/") {
            let main_path = rel_path.replace("src/test/java/", "src/main/java/");
            let main_target = main_path.replace(filename, &target_name);
            rels.push(Relationship {
                source: rel_path.to_string(),
                target: main_target,
                kind: "TESTS".to_string(),
                evidence: "MAVEN_GRADLE_TEST_CONVENTION".to_string(),
                confidence: 0.95,
                line: 1,
            });
        } else {
            rels.push(Relationship {
                source: rel_path.to_string(),
                target: target_name,
                kind: "TESTS".to_string(),
                evidence: "NAMING_CONVENTION".to_string(),
                confidence: 0.90,
                line: 1,
            });
        }
    }

    (symbols, rels)
}

/// Resolve a Java import specifier (e.g. `com.engram.billing.Invoice`) to a local file path if present.
pub fn resolve_java_import_to_file(
    root: &Path,
    _current_rel: &str,
    import_path: &str,
) -> Option<String> {
    let rel_file_path = format!("{}.java", import_path.replace('.', "/"));

    // Standard Maven / Gradle candidate paths
    let candidates = [
        format!("src/main/java/{rel_file_path}"),
        format!("src/test/java/{rel_file_path}"),
        format!("src/{rel_file_path}"),
        rel_file_path.clone(),
    ];

    for c in &candidates {
        if root.join(c).exists() {
            return Some(crate::analyzer::normalize_rel_path(Path::new(c)));
        }
    }

    // Look for file with the class name in the repository
    let class_name = import_path.split('.').last()?;
    let target_filename = format!("{class_name}.java");

    if let Ok(entries) = std::fs::read_dir(root) {
        let mut dirs_to_visit = Vec::new();
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                dirs_to_visit.push(entry.path());
            }
        }
        while let Some(dir) = dirs_to_visit.pop() {
            if let Ok(sub) = std::fs::read_dir(&dir) {
                for entry in sub.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if name != "target" && name != "build" && name != ".git" {
                            dirs_to_visit.push(p);
                        }
                    } else if p.is_file() {
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            if name == target_filename {
                                if let Ok(rel) = p.strip_prefix(root) {
                                    return Some(crate::analyzer::normalize_rel_path(rel));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

/// Normalize Java symbol name (e.g. `InvoiceService.createInvoice()` -> `InvoiceService.createInvoice`).
pub fn normalize_java_symbol_name(symbol: &str) -> String {
    let clean = symbol.split('(').next().unwrap_or(symbol).trim();
    clean.to_string()
}

/// Matches expected Change Contract symbol against actual mutated symbol.
pub fn matches_java_contract_symbol(expected: &str, actual: &str) -> bool {
    let norm_exp = normalize_java_symbol_name(expected);
    let norm_act = normalize_java_symbol_name(actual);

    if norm_exp == norm_act {
        return true;
    }

    // Match short name if expected is fully qualified or vice versa
    if norm_exp.ends_with(&format!(".{norm_act}")) || norm_act.ends_with(&format!(".{norm_exp}")) {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_java_records_and_classes() {
        let src = r#"
package com.engram.billing;

import java.time.Instant;
import java.util.Optional;

public record Invoice(
    String id,
    String customerId,
    double amount,
    String status,
    Instant issuedAt
) {
    public boolean isPaid() {
        return "PAID".equalsIgnoreCase(status);
    }
}
"#;
        let ast = extract_java_ast(src).unwrap();
        assert_eq!(ast.package.as_deref(), Some("com.engram.billing"));
        assert_eq!(ast.imports.len(), 2);
        assert_eq!(ast.types.len(), 1);
        assert_eq!(ast.types[0].name, "Invoice");
        assert_eq!(ast.types[0].kind, "RECORD");
        assert_eq!(ast.methods.len(), 1);
        assert_eq!(ast.methods[0].name, "Invoice.isPaid");
        assert_eq!(ast.methods[0].method_name, "isPaid");
    }

    #[test]
    fn test_extract_java_interfaces_and_services() {
        let src = r#"
package com.engram.billing;

import java.util.Optional;
import java.util.List;

public interface InvoiceRepository {
    Optional<Invoice> findById(String id);
    void save(Invoice invoice);
    List<Invoice> findByCustomerId(String customerId);
}
"#;
        let ast = extract_java_ast(src).unwrap();
        assert_eq!(ast.types.len(), 1);
        assert_eq!(ast.types[0].name, "InvoiceRepository");
        assert_eq!(ast.types[0].kind, "INTERFACE");
        assert_eq!(ast.methods.len(), 3);
        assert_eq!(ast.methods[0].name, "InvoiceRepository.findById");
        assert_eq!(ast.methods[1].name, "InvoiceRepository.save");
        assert_eq!(ast.methods[2].name, "InvoiceRepository.findByCustomerId");
    }

    #[test]
    fn test_parse_pom_xml_dependencies() {
        let pom = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.engram.billing</groupId>
    <artifactId>billing-service</artifactId>
    <version>1.0.0</version>
    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter</artifactId>
            <version>3.2.0</version>
        </dependency>
        <dependency>
            <groupId>org.junit.jupiter</groupId>
            <artifactId>junit-jupiter-api</artifactId>
            <version>5.10.1</version>
            <scope>test</scope>
        </dependency>
    </dependencies>
</project>"#;

        let doc = parse_pom_xml(pom);
        assert_eq!(doc.group_id.as_deref(), Some("com.engram.billing"));
        assert_eq!(doc.artifact_id.as_deref(), Some("billing-service"));
        assert_eq!(doc.version.as_deref(), Some("1.0.0"));
        assert_eq!(doc.dependencies.len(), 2);
        assert_eq!(doc.dependencies[0].group_id, "org.springframework.boot");
        assert_eq!(doc.dependencies[0].artifact_id, "spring-boot-starter");
        assert_eq!(doc.dependencies[1].scope.as_deref(), Some("test"));
    }

    #[test]
    fn test_parse_build_gradle_dependencies() {
        let gradle = r#"
plugins {
    id 'java'
}

group = 'com.engram.billing'
version = '1.0.0'

dependencies {
    implementation 'org.springframework.boot:spring-boot-starter:3.2.0'
    testImplementation 'org.junit.jupiter:junit-jupiter-api:5.10.1'
}
"#;
        let doc = parse_build_gradle(gradle);
        assert_eq!(doc.group.as_deref(), Some("com.engram.billing"));
        assert_eq!(doc.version.as_deref(), Some("1.0.0"));
        assert_eq!(doc.plugins, vec!["java"]);
        assert_eq!(doc.dependencies.len(), 2);
        assert_eq!(doc.dependencies[0].configuration, "implementation");
        assert_eq!(
            doc.dependencies[0].notation,
            "org.springframework.boot:spring-boot-starter:3.2.0"
        );
    }

    #[test]
    fn test_java_symbol_matching() {
        assert!(matches_java_contract_symbol(
            "InvoiceService.createInvoice",
            "InvoiceService.createInvoice"
        ));
        assert!(matches_java_contract_symbol(
            "createInvoice",
            "InvoiceService.createInvoice"
        ));
        assert!(matches_java_contract_symbol(
            "InvoiceService.createInvoice()",
            "InvoiceService.createInvoice"
        ));
    }
}
