use std::collections::HashSet;
use std::path::Path;
use tree_sitter::Node;

use crate::analyzer::{Relationship, Symbol, node_text, normalize_rel_path, resolve_import_path};

pub fn extract_call_target_v2(n: Node, src: &[u8], lang: &str) -> Option<String> {
    if crate::analyzer::is_inside_decorator_pub(n) {
        return None;
    }
    match lang {
        "typescript" | "javascript" => {
            if n.kind() == "call_expression" {
                if let Some(func) = n.child_by_field_name("function") {
                    if func.kind() == "identifier" {
                        let name = node_text(func, src);
                        if name == "require" || name == "import" {
                            return None;
                        }
                        return Some(name);
                    } else if func.kind() == "import" {
                        return None;
                    } else if func.kind() == "member_expression" {
                        let prop = func
                            .child_by_field_name("property")
                            .map(|p| node_text(p, src))
                            .unwrap_or_default();
                        if prop.is_empty() {
                            return None;
                        }
                        return Some(prop);
                    }
                }
            }
        }
        "python" => {
            if n.kind() == "call" {
                if let Some(func) = n.child_by_field_name("function") {
                    if func.kind() == "identifier" {
                        return Some(node_text(func, src));
                    } else if func.kind() == "attribute" {
                        let attr = func
                            .child_by_field_name("attribute")
                            .map(|a| node_text(a, src))
                            .unwrap_or_default();
                        if !attr.is_empty() {
                            return Some(attr);
                        }
                    }
                }
            }
        }
        "rust" => {
            if n.kind() == "call_expression" {
                if let Some(func) = n.child_by_field_name("function") {
                    if func.kind() == "identifier" || func.kind() == "scoped_identifier" {
                        return Some(node_text(func, src));
                    } else if func.kind() == "field_expression" {
                        let field = func
                            .child_by_field_name("field")
                            .map(|f| node_text(f, src))
                            .unwrap_or_default();
                        if field.is_empty() {
                            return None;
                        }
                        if let Some(val) = func.child_by_field_name("value") {
                            let val_txt = node_text(val, src);
                            if val_txt == "self" {
                                return Some(field);
                            }
                            if val.kind() == "identifier"
                                && !val_txt.is_empty()
                                && val_txt.len() <= 40
                            {
                                return Some(format!("{val_txt}.{field}"));
                            }
                        }
                        return Some(field);
                    } else if func.kind() == "generic_function" {
                        if let Some(gf) = func.child_by_field_name("function") {
                            return Some(node_text(gf, src));
                        }
                    }
                }
            } else if n.kind() == "method_call_expression" {
                if let Some(name_node) = n
                    .child_by_field_name("name")
                    .or_else(|| n.child_by_field_name("method"))
                {
                    let text = node_text(name_node, src);
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
                let mut cursor = n.walk();
                for child in n.children(&mut cursor) {
                    if child.kind() == "field_identifier" {
                        let text = node_text(child, src);
                        if !text.is_empty() {
                            return Some(text);
                        }
                    }
                }
            } else if n.kind() == "macro_invocation" {
                if let Some(mac) = n.child_by_field_name("macro") {
                    let txt = node_text(mac, src);
                    if !matches!(
                        txt.as_str(),
                        "println"
                            | "eprintln"
                            | "format"
                            | "vec"
                            | "assert"
                            | "assert_eq"
                            | "assert_ne"
                            | "debug_assert"
                    ) {
                        return Some(txt);
                    }
                }
            }
        }
        "java" => {
            if n.kind() == "method_invocation" {
                if let Some(name) = n.child_by_field_name("name") {
                    return Some(node_text(name, src));
                }
            }
        }
        "csharp" => {
            if n.kind() == "invocation_expression" {
                // tree-sitter-c-sharp 0.23.5 uses field "function" (older grammars used "expression")
                if let Some(expr) = n
                    .child_by_field_name("function")
                    .or_else(|| n.child_by_field_name("expression"))
                {
                    if expr.kind() == "identifier" {
                        return Some(node_text(expr, src));
                    } else if expr.kind() == "generic_name" {
                        return extract_simple_type_name(expr, src);
                    } else if expr.kind() == "member_access_expression" {
                        if let Some(name) = expr
                            .child_by_field_name("name")
                            .and_then(|nm| extract_simple_type_name(nm, src))
                        {
                            if !name.is_empty() {
                                return Some(name);
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    None
}

fn is_primitive_or_builtin(name: &str, lang: &str) -> bool {
    let clean = name.trim();
    if clean.is_empty() || clean.len() > 80 {
        return true;
    }
    match lang {
        "csharp" => matches!(
            clean,
            "int"
                | "uint"
                | "long"
                | "ulong"
                | "short"
                | "ushort"
                | "byte"
                | "sbyte"
                | "bool"
                | "string"
                | "char"
                | "float"
                | "double"
                | "decimal"
                | "void"
                | "object"
                | "var"
                | "dynamic"
                | "Task"
                | "ValueTask"
                | "List"
                | "IList"
                | "IEnumerable"
                | "ICollection"
                | "IReadOnlyList"
                | "Dictionary"
                | "IDictionary"
                | "HashSet"
                | "CancellationToken"
                | "Exception"
                | "Guid"
                | "DateTime"
                | "DateTimeOffset"
                | "TimeSpan"
        ),
        "typescript" | "javascript" => matches!(
            clean,
            "string"
                | "number"
                | "boolean"
                | "void"
                | "any"
                | "unknown"
                | "never"
                | "null"
                | "undefined"
                | "object"
                | "symbol"
                | "bigint"
                | "Promise"
                | "Record"
                | "Array"
                | "ReadonlyArray"
                | "Partial"
                | "Required"
                | "Readonly"
                | "Pick"
                | "Omit"
                | "Map"
                | "Set"
                | "Date"
                | "Error"
        ),
        "python" => matches!(
            clean,
            "str"
                | "int"
                | "float"
                | "bool"
                | "bytes"
                | "None"
                | "Any"
                | "object"
                | "list"
                | "dict"
                | "set"
                | "tuple"
                | "List"
                | "Dict"
                | "Set"
                | "Tuple"
                | "Optional"
                | "Union"
                | "Sequence"
                | "Mapping"
                | "Callable"
                | "Iterable"
                | "Iterator"
                | "Type"
                | "Final"
                | "Literal"
                | "ClassVar"
        ),
        "rust" => matches!(
            clean,
            "i8" | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "f32"
                | "f64"
                | "bool"
                | "char"
                | "str"
                | "Self"
                | "String"
                | "Vec"
                | "Option"
                | "Result"
                | "Box"
                | "Arc"
                | "Rc"
                | "HashMap"
                | "HashSet"
                | "BTreeMap"
                | "BTreeSet"
                | "VecDeque"
                | "Path"
                | "PathBuf"
                | "Send"
                | "Sync"
                | "Sized"
                | "Clone"
                | "Copy"
                | "Debug"
                | "Display"
                | "Default"
                | "PartialEq"
                | "Eq"
                | "PartialOrd"
                | "Ord"
                | "Hash"
                | "Serialize"
                | "Deserialize"
                | "Connection"
        ),
        _ => false,
    }
}

pub fn is_known_external_package(spec: &str, lang: &str) -> bool {
    let clean = spec.trim();
    if clean.is_empty() {
        return false;
    }
    match lang {
        "csharp" => {
            let root_ns = clean.split('.').next().unwrap_or("");
            matches!(
                root_ns,
                "System"
                    | "Microsoft"
                    | "Xunit"
                    | "NUnit"
                    | "Moq"
                    | "FluentAssertions"
                    | "Newtonsoft"
                    | "Serilog"
            )
        }
        "python" => {
            if clean.starts_with('.') {
                return false;
            }
            let root_pkg = clean.split('.').next().unwrap_or("");
            matches!(
                root_pkg,
                "os" | "sys"
                    | "re"
                    | "json"
                    | "math"
                    | "time"
                    | "datetime"
                    | "pathlib"
                    | "typing"
                    | "dataclasses"
                    | "collections"
                    | "itertools"
                    | "functools"
                    | "asyncio"
                    | "uuid"
                    | "hashlib"
                    | "sqlite3"
                    | "unittest"
                    | "pytest"
                    | "logging"
                    | "enum"
                    | "abc"
                    | "contextlib"
                    | "io"
                    | "copy"
                    | "shutil"
                    | "subprocess"
                    | "tempfile"
                    | "urllib"
                    | "http"
                    | "pydantic"
                    | "fastapi"
                    | "flask"
                    | "django"
                    | "sqlalchemy"
                    | "numpy"
                    | "pandas"
                    | "requests"
            )
        }
        "rust" => {
            if clean.starts_with("crate::")
                || clean.starts_with("super::")
                || clean.starts_with("self::")
                || clean.ends_with(".rs")
            {
                return false;
            }
            let root_crate = clean.split("::").next().unwrap_or("");
            matches!(
                root_crate,
                "std"
                    | "core"
                    | "alloc"
                    | "serde"
                    | "serde_json"
                    | "anyhow"
                    | "thiserror"
                    | "tokio"
                    | "clap"
                    | "rusqlite"
                    | "chrono"
                    | "uuid"
                    | "regex"
                    | "tree_sitter"
                    | "sha2"
                    | "ignore"
                    | "log"
                    | "tracing"
            )
        }
        "typescript" | "javascript" => {
            if clean.starts_with('.') || clean.starts_with('/') {
                return false;
            }
            crate::analyzer::is_external_target(clean)
        }
        _ => crate::analyzer::is_external_target(clean),
    }
}

fn extract_simple_type_name(n: Node, src: &[u8]) -> Option<String> {
    match n.kind() {
        "identifier" | "type_identifier" | "predefined_type" => {
            let txt = node_text(n, src).trim().to_string();
            if txt.is_empty() { None } else { Some(txt) }
        }
        "generic_name" => {
            // C#: first identifier child
            let mut c = n.walk();
            for ch in n.children(&mut c) {
                if ch.kind() == "identifier" {
                    return Some(node_text(ch, src));
                }
            }
            None
        }
        "generic_type" | "nested_type_identifier" | "scoped_type_identifier" | "qualified_name" => {
            if let Some(nm) = n.child_by_field_name("name") {
                return extract_simple_type_name(nm, src);
            }
            let txt = node_text(n, src);
            let before_lt = txt.split('<').next().unwrap_or(&txt).trim();
            let tail = before_lt
                .rsplit('.')
                .next()
                .unwrap_or(before_lt)
                .rsplit("::")
                .next()
                .unwrap_or(before_lt)
                .trim();
            if tail.is_empty() {
                None
            } else {
                Some(tail.to_string())
            }
        }
        "member_expression" | "attribute" => {
            if let Some(prop) = n
                .child_by_field_name("property")
                .or_else(|| n.child_by_field_name("attribute"))
                .or_else(|| n.child_by_field_name("name"))
            {
                return Some(node_text(prop, src));
            }
            None
        }
        "primary_constructor_base_type" => n
            .child_by_field_name("type")
            .and_then(|t| extract_simple_type_name(t, src)),
        _ => {
            let txt = node_text(n, src);
            let clean = txt.split('<').next().unwrap_or(&txt).trim();
            if clean.is_empty() || clean.contains('\n') || clean.contains(' ') || clean.len() > 64 {
                None
            } else {
                let tail = clean
                    .rsplit('.')
                    .next()
                    .unwrap_or(clean)
                    .rsplit("::")
                    .next()
                    .unwrap_or(clean)
                    .trim();
                if tail.is_empty() {
                    None
                } else {
                    Some(tail.to_string())
                }
            }
        }
    }
}

fn collect_type_names_recursive(n: Node, src: &[u8], lang: &str, out: &mut Vec<String>) {
    match n.kind() {
        "identifier" | "type_identifier" => {
            let txt = node_text(n, src).trim().to_string();
            if !is_primitive_or_builtin(&txt, lang) {
                out.push(txt);
            }
            return;
        }
        "generic_name" => {
            if let Some(base) = extract_simple_type_name(n, src) {
                if !is_primitive_or_builtin(&base, lang) {
                    out.push(base);
                }
            }
            if let Some(args) = n
                .children(&mut n.walk())
                .find(|c| c.kind() == "type_argument_list")
            {
                collect_type_names_recursive(args, src, lang, out);
            }
            return;
        }
        "generic_type" => {
            if let Some(base) = n
                .child_by_field_name("name")
                .and_then(|x| extract_simple_type_name(x, src))
            {
                if !is_primitive_or_builtin(&base, lang) {
                    out.push(base);
                }
            }
            if let Some(ta) = n.child_by_field_name("type_arguments") {
                collect_type_names_recursive(ta, src, lang, out);
            }
            return;
        }
        "scoped_type_identifier" | "nested_type_identifier" | "qualified_name" => {
            if let Some(base) = extract_simple_type_name(n, src) {
                if !is_primitive_or_builtin(&base, lang) {
                    out.push(base);
                }
            }
            return;
        }
        _ => {}
    }
    let mut c = n.walk();
    for ch in n.children(&mut c) {
        if !matches!(ch.kind(), "predefined_type" | "primitive_type") {
            collect_type_names_recursive(ch, src, lang, out);
        }
    }
}

fn push_type_refs(
    type_node: Node,
    src: &[u8],
    path: &str,
    lang: &str,
    evidence: &str,
    line: usize,
    rels: &mut Vec<Relationship>,
) {
    let mut names = Vec::new();
    collect_type_names_recursive(type_node, src, lang, &mut names);
    let mut seen = HashSet::new();
    for name in names {
        if seen.insert(name.clone()) {
            rels.push(Relationship {
                source: path.into(),
                target: name,
                kind: "REFERENCES".into(),
                evidence: evidence.into(),
                confidence: 0.90,
                line,
                is_external: false,
            });
        }
    }
}

/// Extract structural, import, re-export, inheritance, implementation, and type-reference
/// relationships for a single AST node. Returns `true` if `visit_ast` should skip descending
/// into nested import nodes inside this subtree.
pub fn extract_node_relationships(
    n: Node,
    src: &[u8],
    path: &str,
    lang: &str,
    rels: &mut Vec<Relationship>,
) -> bool {
    let line = n.start_position().row + 1;
    match lang {
        "csharp" => extract_csharp_node(n, src, path, line, rels),
        "typescript" | "javascript" => extract_ts_node(n, src, path, line, rels),
        "python" => extract_python_node(n, src, path, line, rels),
        "rust" => extract_rust_node(n, src, path, line, rels),
        _ => {
            if n.kind().contains("import") {
                let raw = node_text(n, src);
                let spec = crate::analyzer::extract_import_specifier(&raw, lang);
                if !spec.is_empty() {
                    let target = resolve_import_path(path, &spec);
                    let is_ext = crate::analyzer::is_external_target(&target);
                    rels.push(Relationship {
                        source: path.into(),
                        target,
                        kind: "IMPORTS".into(),
                        evidence: "TREE_SITTER_IMPORT_SYNTAX".into(),
                        confidence: 0.99,
                        line,
                        is_external: is_ext,
                    });
                }
            }
            false
        }
    }
}

fn extract_csharp_node(
    n: Node,
    src: &[u8],
    path: &str,
    line: usize,
    rels: &mut Vec<Relationship>,
) -> bool {
    match n.kind() {
        "namespace_declaration" | "file_scoped_namespace_declaration" => {
            if let Some(name_node) = n.child_by_field_name("name") {
                let ns = node_text(name_node, src).trim().to_string();
                if !ns.is_empty() {
                    rels.push(Relationship {
                        source: path.into(),
                        target: ns,
                        kind: "DECLARES_MODULE".into(),
                        evidence: "TREE_SITTER_NAMESPACE_DECL".into(),
                        confidence: 0.99,
                        line,
                        is_external: false,
                    });
                }
            }
        }
        "using_directive" => {
            let raw = node_text(n, src);
            let mut clean = raw.trim().trim_end_matches(';').trim();
            if let Some(eq_idx) = clean.find('=') {
                clean = clean[eq_idx + 1..].trim();
            } else {
                if let Some(rest) = clean.strip_prefix("global ") {
                    clean = rest.trim();
                }
                if let Some(rest) = clean.strip_prefix("using ") {
                    clean = rest.trim();
                }
                if let Some(rest) = clean.strip_prefix("static ") {
                    clean = rest.trim();
                }
            }
            if !clean.is_empty() && !clean.contains(' ') && !clean.contains('(') {
                let is_ext = is_known_external_package(clean, "csharp");
                rels.push(Relationship {
                    source: path.into(),
                    target: clean.to_string(),
                    kind: "IMPORTS".into(),
                    evidence: "TREE_SITTER_USING_DIRECTIVE".into(),
                    confidence: 0.99,
                    line,
                    is_external: is_ext,
                });
            }
            return true;
        }
        "class_declaration"
        | "struct_declaration"
        | "record_declaration"
        | "interface_declaration" => {
            let is_interface = n.kind() == "interface_declaration";
            let mut c = n.walk();
            for ch in n.children(&mut c) {
                if ch.kind() == "base_list" {
                    let mut bc = ch.walk();
                    for b in ch.named_children(&mut bc) {
                        if let Some(base_name) = extract_simple_type_name(b, src) {
                            if is_primitive_or_builtin(&base_name, "csharp") {
                                continue;
                            }
                            let looks_like_interface = base_name.starts_with('I')
                                && base_name
                                    .chars()
                                    .nth(1)
                                    .map(|c| c.is_ascii_uppercase())
                                    .unwrap_or(false);
                            let rel_kind = if is_interface || !looks_like_interface {
                                "INHERITS"
                            } else {
                                "IMPLEMENTS"
                            };
                            rels.push(Relationship {
                                source: path.into(),
                                target: base_name.clone(),
                                kind: rel_kind.into(),
                                evidence: "CSHARP_BASE_LIST".into(),
                                confidence: 0.95,
                                line: b.start_position().row + 1,
                                is_external: false,
                            });
                            // Also extract generic arguments in base list (e.g. IRepository<Order>)
                            let mut tc = b.walk();
                            for sub in b.children(&mut tc) {
                                if sub.kind() == "type_argument_list" {
                                    push_type_refs(
                                        sub,
                                        src,
                                        path,
                                        "csharp",
                                        "CSHARP_BASE_GENERIC_ARG",
                                        line,
                                        rels,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
        "object_creation_expression" => {
            if let Some(ty) = n.child_by_field_name("type") {
                push_type_refs(
                    ty,
                    src,
                    path,
                    "csharp",
                    "CSHARP_OBJECT_CREATION",
                    line,
                    rels,
                );
            }
        }
        "parameter" | "property_declaration" => {
            if let Some(ty) = n.child_by_field_name("type") {
                push_type_refs(ty, src, path, "csharp", "CSHARP_TYPE_REF", line, rels);
            }
        }
        "method_declaration" => {
            if let Some(ret) = n
                .child_by_field_name("returns")
                .or_else(|| n.child_by_field_name("type"))
            {
                push_type_refs(ret, src, path, "csharp", "CSHARP_RETURN_TYPE", line, rels);
            }
        }
        "field_declaration" | "local_declaration_statement" => {
            let mut c = n.walk();
            for ch in n.children(&mut c) {
                if ch.kind() == "variable_declaration" {
                    if let Some(ty) = ch.child_by_field_name("type") {
                        if node_text(ty, src) != "var" {
                            push_type_refs(ty, src, path, "csharp", "CSHARP_TYPE_REF", line, rels);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    false
}

fn extract_ts_node(
    n: Node,
    src: &[u8],
    path: &str,
    line: usize,
    rels: &mut Vec<Relationship>,
) -> bool {
    match n.kind() {
        "import_statement" => {
            if let Some(src_node) = n.child_by_field_name("source") {
                let raw = node_text(src_node, src);
                let spec = raw.trim_matches(|c| c == '\'' || c == '"' || c == ' ');
                if !spec.is_empty() {
                    let target = resolve_import_path(path, spec);
                    let is_ext = is_known_external_package(&target, "typescript");
                    rels.push(Relationship {
                        source: path.into(),
                        target: target.clone(),
                        kind: "IMPORTS".into(),
                        evidence: "TREE_SITTER_IMPORT_SYNTAX".into(),
                        confidence: 0.99,
                        line,
                        is_external: is_ext,
                    });

                    let mut cursor = n.walk();
                    for child in n.children(&mut cursor) {
                        if child.kind() == "import_clause" {
                            let mut ic_cursor = child.walk();
                            for ic_child in child.children(&mut ic_cursor) {
                                if ic_child.kind() == "identifier" {
                                    let def_sym = node_text(ic_child, src);
                                    if !def_sym.is_empty() && def_sym != "type" {
                                        rels.push(Relationship {
                                            source: path.into(),
                                            target: format!("{target}::{def_sym}"),
                                            kind: "IMPORTS_SYMBOL".into(),
                                            evidence: "TREE_SITTER_DEFAULT_IMPORT".into(),
                                            confidence: 0.96,
                                            line: ic_child.start_position().row + 1,
                                            is_external: is_ext,
                                        });
                                    }
                                } else if ic_child.kind() == "named_imports" {
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
            return true;
        }
        "export_statement" => {
            if let Some(src_node) = n.child_by_field_name("source") {
                let raw = node_text(src_node, src);
                let spec = raw.trim_matches(|c| c == '\'' || c == '"' || c == ' ');
                if !spec.is_empty() {
                    let target = resolve_import_path(path, spec);
                    let is_ext = is_known_external_package(&target, "typescript");
                    rels.push(Relationship {
                        source: path.into(),
                        target: target.clone(),
                        kind: "IMPORTS".into(),
                        evidence: "TREE_SITTER_REEXPORT".into(),
                        confidence: 0.99,
                        line,
                        is_external: is_ext,
                    });
                    let mut c = n.walk();
                    for ch in n.children(&mut c) {
                        if ch.kind() == "export_clause" {
                            let mut ec = ch.walk();
                            for es in ch.children(&mut ec) {
                                if es.kind() == "export_specifier" {
                                    let sym_name = es
                                        .child_by_field_name("name")
                                        .map(|x| node_text(x, src))
                                        .unwrap_or_else(|| node_text(es, src));
                                    if !sym_name.is_empty() {
                                        rels.push(Relationship {
                                            source: path.into(),
                                            target: format!("{target}::{sym_name}"),
                                            kind: "IMPORTS_SYMBOL".into(),
                                            evidence: "TREE_SITTER_REEXPORT_SYMBOL".into(),
                                            confidence: 0.99,
                                            line: es.start_position().row + 1,
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
        "call_expression" => {
            if let Some(func) = n.child_by_field_name("function") {
                let ftxt = node_text(func, src);
                if ftxt == "require" || ftxt == "import" {
                    if let Some(args) = n.child_by_field_name("arguments") {
                        let mut ac = args.walk();
                        if let Some(first_str) = args.named_children(&mut ac).next() {
                            if first_str.kind() == "string" || first_str.kind() == "template_string"
                            {
                                let raw = node_text(first_str, src);
                                let spec = raw.trim_matches(|c| {
                                    c == '\'' || c == '"' || c == '`' || c == ' '
                                });
                                if !spec.is_empty() && !spec.contains("${") {
                                    let target = resolve_import_path(path, spec);
                                    let is_ext = is_known_external_package(&target, "typescript");
                                    rels.push(Relationship {
                                        source: path.into(),
                                        target,
                                        kind: "IMPORTS".into(),
                                        evidence: "DYNAMIC_OR_CJS_IMPORT".into(),
                                        confidence: 0.95,
                                        line,
                                        is_external: is_ext,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        "class_declaration" => {
            let mut c = n.walk();
            for ch in n.children(&mut c) {
                if ch.kind() == "class_heritage" {
                    let mut hc = ch.walk();
                    for hchild in ch.named_children(&mut hc) {
                        if hchild.kind() == "extends_clause" {
                            if let Some(val) = hchild
                                .child_by_field_name("value")
                                .and_then(|v| extract_simple_type_name(v, src))
                            {
                                if !is_primitive_or_builtin(&val, "typescript") {
                                    rels.push(Relationship {
                                        source: path.into(),
                                        target: val,
                                        kind: "INHERITS".into(),
                                        evidence: "TS_CLASS_EXTENDS".into(),
                                        confidence: 0.98,
                                        line: hchild.start_position().row + 1,
                                        is_external: false,
                                    });
                                }
                            }
                        } else if hchild.kind() == "implements_clause" {
                            let mut ic = hchild.walk();
                            for iface in hchild.named_children(&mut ic) {
                                if let Some(iname) = extract_simple_type_name(iface, src) {
                                    if !is_primitive_or_builtin(&iname, "typescript") {
                                        rels.push(Relationship {
                                            source: path.into(),
                                            target: iname,
                                            kind: "IMPLEMENTS".into(),
                                            evidence: "TS_CLASS_IMPLEMENTS".into(),
                                            confidence: 0.98,
                                            line: iface.start_position().row + 1,
                                            is_external: false,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        "interface_declaration" => {
            let mut c = n.walk();
            for ch in n.children(&mut c) {
                if ch.kind() == "extends_type_clause" {
                    let mut ec = ch.walk();
                    for ext_ty in ch.named_children(&mut ec) {
                        if let Some(name) = extract_simple_type_name(ext_ty, src) {
                            if !is_primitive_or_builtin(&name, "typescript") {
                                rels.push(Relationship {
                                    source: path.into(),
                                    target: name,
                                    kind: "INHERITS".into(),
                                    evidence: "TS_INTERFACE_EXTENDS".into(),
                                    confidence: 0.98,
                                    line: ext_ty.start_position().row + 1,
                                    is_external: false,
                                });
                            }
                        }
                    }
                }
            }
        }
        "new_expression" => {
            if let Some(ctor) = n.child_by_field_name("constructor") {
                if let Some(name) = extract_simple_type_name(ctor, src) {
                    if !is_primitive_or_builtin(&name, "typescript") {
                        rels.push(Relationship {
                            source: path.into(),
                            target: name,
                            kind: "REFERENCES".into(),
                            evidence: "TS_NEW_EXPRESSION".into(),
                            confidence: 0.94,
                            line,
                            is_external: false,
                        });
                    }
                }
            }
        }
        "type_annotation" => {
            push_type_refs(n, src, path, "typescript", "TS_TYPE_REF", line, rels);
        }
        _ => {}
    }
    false
}

fn extract_python_node(
    n: Node,
    src: &[u8],
    path: &str,
    line: usize,
    rels: &mut Vec<Relationship>,
) -> bool {
    match n.kind() {
        "import_statement" => {
            let mut c = n.walk();
            for ch in n.named_children(&mut c) {
                let mod_name = if ch.kind() == "aliased_import" {
                    ch.child_by_field_name("name")
                        .map(|m| node_text(m, src))
                        .unwrap_or_default()
                } else if ch.kind() == "dotted_name" {
                    node_text(ch, src)
                } else {
                    String::new()
                };
                if !mod_name.is_empty() {
                    let is_ext = is_known_external_package(&mod_name, "python");
                    rels.push(Relationship {
                        source: path.into(),
                        target: mod_name,
                        kind: "IMPORTS".into(),
                        evidence: "PYTHON_IMPORT_STATEMENT".into(),
                        confidence: 0.99,
                        line,
                        is_external: is_ext,
                    });
                }
            }
            return true;
        }
        "import_from_statement" => {
            let mod_node = n.child_by_field_name("module_name");
            let mod_spec = mod_node
                .map(|m| node_text(m, src).trim().to_string())
                .unwrap_or_default();
            if !mod_spec.is_empty() {
                let is_rel = mod_spec.starts_with('.');
                let is_ext = if is_rel {
                    false
                } else {
                    is_known_external_package(&mod_spec, "python")
                };
                rels.push(Relationship {
                    source: path.into(),
                    target: mod_spec.clone(),
                    kind: "IMPORTS".into(),
                    evidence: "PYTHON_IMPORT_FROM".into(),
                    confidence: 0.99,
                    line,
                    is_external: is_ext,
                });

                let mut nc = n.walk();
                for name_node in n.children_by_field_name("name", &mut nc) {
                    let sym = if name_node.kind() == "aliased_import" {
                        name_node
                            .child_by_field_name("name")
                            .map(|x| node_text(x, src))
                            .unwrap_or_default()
                    } else {
                        node_text(name_node, src)
                    };
                    let sym_clean = sym.trim();
                    if !sym_clean.is_empty() {
                        rels.push(Relationship {
                            source: path.into(),
                            target: format!("{mod_spec}::{sym_clean}"),
                            kind: "IMPORTS_SYMBOL".into(),
                            evidence: "PYTHON_IMPORT_SYMBOL".into(),
                            confidence: 0.99,
                            line: name_node.start_position().row + 1,
                            is_external: is_ext,
                        });
                    }
                }
            }
            return true;
        }
        "class_definition" => {
            if let Some(supers) = n.child_by_field_name("superclasses") {
                let mut sc = supers.walk();
                for base in supers.named_children(&mut sc) {
                    if let Some(name) = extract_simple_type_name(base, src) {
                        if !is_primitive_or_builtin(&name, "python") {
                            rels.push(Relationship {
                                source: path.into(),
                                target: name,
                                kind: "INHERITS".into(),
                                evidence: "PYTHON_CLASS_BASE".into(),
                                confidence: 0.97,
                                line: base.start_position().row + 1,
                                is_external: false,
                            });
                        }
                    }
                }
            }
        }
        "type" => {
            push_type_refs(n, src, path, "python", "PYTHON_TYPE_ANNOTATION", line, rels);
        }
        _ => {}
    }
    false
}

fn flatten_rust_use_tree(n: Node, src: &[u8], prefix: &str, out: &mut Vec<(String, bool)>) {
    match n.kind() {
        "identifier" | "crate" | "self" | "super" => {
            let seg = node_text(n, src).trim().to_string();
            if seg.is_empty() {
                return;
            }
            if seg == "self" && !prefix.is_empty() {
                out.push((prefix.to_string(), false));
                return;
            }
            let combined = if prefix.is_empty() {
                seg
            } else {
                format!("{prefix}::{seg}")
            };
            out.push((combined, true));
        }
        "scoped_identifier" => {
            let txt = node_text(n, src).trim().to_string();
            if txt.is_empty() {
                return;
            }
            let combined = if prefix.is_empty() {
                txt
            } else {
                format!("{prefix}::{txt}")
            };
            out.push((combined, true));
        }
        "use_as_clause" => {
            if let Some(path_node) = n.child_by_field_name("path") {
                flatten_rust_use_tree(path_node, src, prefix, out);
            }
        }
        "use_wildcard" => {
            let mut c = n.walk();
            let mut sub_prefix = prefix.to_string();
            for ch in n.named_children(&mut c) {
                let mut inner = Vec::new();
                flatten_rust_use_tree(ch, src, prefix, &mut inner);
                if let Some((first, _)) = inner.into_iter().next() {
                    sub_prefix = first;
                    break;
                }
            }
            if !sub_prefix.is_empty() {
                out.push((format!("{sub_prefix}::*"), false));
            }
        }
        "scoped_use_list" => {
            let base = if let Some(path_node) = n.child_by_field_name("path") {
                let pt = node_text(path_node, src).trim().to_string();
                if prefix.is_empty() {
                    pt
                } else {
                    format!("{prefix}::{pt}")
                }
            } else {
                prefix.to_string()
            };
            if let Some(list) = n.child_by_field_name("list") {
                flatten_rust_use_tree(list, src, &base, out);
            }
        }
        "use_list" => {
            let mut c = n.walk();
            for ch in n.named_children(&mut c) {
                flatten_rust_use_tree(ch, src, prefix, out);
            }
        }
        _ => {
            let txt = node_text(n, src).trim().to_string();
            if !txt.is_empty() && !txt.contains('{') {
                let combined = if prefix.is_empty() {
                    txt
                } else {
                    format!("{prefix}::{txt}")
                };
                out.push((combined, true));
            }
        }
    }
}

fn extract_rust_node(
    n: Node,
    src: &[u8],
    path: &str,
    line: usize,
    rels: &mut Vec<Relationship>,
) -> bool {
    match n.kind() {
        "use_declaration" => {
            if let Some(arg) = n.child_by_field_name("argument") {
                let mut entries = Vec::new();
                flatten_rust_use_tree(arg, src, "", &mut entries);
                for (use_path, has_leaf) in entries {
                    let is_ext = is_known_external_package(&use_path, "rust");
                    rels.push(Relationship {
                        source: path.into(),
                        target: use_path.clone(),
                        kind: "IMPORTS".into(),
                        evidence: "TREE_SITTER_USE_DECLARATION".into(),
                        confidence: 0.99,
                        line,
                        is_external: is_ext,
                    });
                    if has_leaf && use_path.contains("::") {
                        rels.push(Relationship {
                            source: path.into(),
                            target: use_path,
                            kind: "IMPORTS_SYMBOL".into(),
                            evidence: "TREE_SITTER_USE_SYMBOL".into(),
                            confidence: 0.96,
                            line,
                            is_external: is_ext,
                        });
                    }
                }
            }
            return true;
        }
        "mod_item" => {
            if n.child_by_field_name("body").is_none() {
                if let Some(name_node) = n.child_by_field_name("name") {
                    let mod_name = node_text(name_node, src).trim().to_string();
                    if !mod_name.is_empty() {
                        rels.push(Relationship {
                            source: path.into(),
                            target: format!("mod::{mod_name}"),
                            kind: "IMPORTS".into(),
                            evidence: "RUST_MOD_DECLARATION".into(),
                            confidence: 0.99,
                            line,
                            is_external: false,
                        });
                    }
                }
            }
        }
        "impl_item" => {
            if let Some(tr) = n.child_by_field_name("trait") {
                if let Some(tr_name) = extract_simple_type_name(tr, src) {
                    if !is_primitive_or_builtin(&tr_name, "rust") {
                        rels.push(Relationship {
                            source: path.into(),
                            target: tr_name,
                            kind: "IMPLEMENTS".into(),
                            evidence: "RUST_IMPL_TRAIT".into(),
                            confidence: 0.98,
                            line,
                            is_external: false,
                        });
                    }
                }
            }
            if let Some(ty) = n.child_by_field_name("type") {
                push_type_refs(ty, src, path, "rust", "RUST_IMPL_TARGET", line, rels);
            }
        }
        "trait_item" => {
            if let Some(bounds) = n.child_by_field_name("bounds") {
                let mut bc = bounds.walk();
                for ch in bounds.named_children(&mut bc) {
                    if let Some(bname) = extract_simple_type_name(ch, src) {
                        if !is_primitive_or_builtin(&bname, "rust") {
                            rels.push(Relationship {
                                source: path.into(),
                                target: bname,
                                kind: "INHERITS".into(),
                                evidence: "RUST_TRAIT_BOUNDS".into(),
                                confidence: 0.96,
                                line: ch.start_position().row + 1,
                                is_external: false,
                            });
                        }
                    }
                }
            }
        }
        "field_declaration" | "parameter" => {
            if let Some(ty) = n.child_by_field_name("type") {
                push_type_refs(ty, src, path, "rust", "RUST_TYPE_REF", line, rels);
            }
        }
        "function_item" => {
            if let Some(ret) = n.child_by_field_name("return_type") {
                push_type_refs(ret, src, path, "rust", "RUST_RETURN_TYPE", line, rels);
            }
        }
        _ => {}
    }
    false
}

/// Resolve workspace-relative paths on disk using direct $O(1)$ file stat probes
/// (never walking the workspace) and normalize test-to-target hints cleanly without
/// brittle hardcoded project folder names.
pub fn enrich_paths_with_root(
    root: &Path,
    rel: &str,
    lang: &str,
    rels: &mut Vec<Relationship>,
    _symbols: &[Symbol],
) {
    let parent = Path::new(rel).parent().unwrap_or(Path::new(""));
    let mut extra_rels: Vec<Relationship> = Vec::new();

    for r in rels.iter_mut() {
        match (lang, r.kind.as_str()) {
            ("typescript" | "javascript", "IMPORTS" | "IMPORTS_SYMBOL") => {
                let (file_part, sym_suffix) = if r.kind == "IMPORTS_SYMBOL" {
                    if let Some((f, s)) = r.target.split_once("::") {
                        (f.to_string(), Some(s.to_string()))
                    } else {
                        (r.target.clone(), None)
                    }
                } else {
                    (r.target.clone(), None)
                };

                if !r.is_external {
                    if let Some(resolved) = probe_ts_path_on_disk(root, rel, &file_part) {
                        r.target = if let Some(sym) = sym_suffix {
                            format!("{resolved}::{sym}")
                        } else {
                            resolved
                        };
                        r.is_external = false;
                    }
                }
            }
            ("python", "IMPORTS") => {
                if let Some(resolved) = probe_python_module_on_disk(root, rel, &r.target) {
                    r.target = resolved;
                    r.is_external = false;
                }
            }
            ("python", "IMPORTS_SYMBOL") => {
                if let Some((mod_part, sym)) = r.target.split_once("::") {
                    if let Some(resolved) = probe_python_module_on_disk(root, rel, mod_part) {
                        r.target = format!("{resolved}::{sym}");
                        r.is_external = false;
                    } else if mod_part == "." {
                        if let Some(sub_res) =
                            probe_python_module_on_disk(root, rel, &format!(".{sym}"))
                        {
                            extra_rels.push(Relationship {
                                source: rel.into(),
                                target: sub_res,
                                kind: "IMPORTS".into(),
                                evidence: "PYTHON_RELATIVE_SUBMODULE_IMPORT".into(),
                                confidence: 0.99,
                                line: r.line,
                                is_external: false,
                            });
                        }
                    }
                }
            }
            ("rust", "IMPORTS") if r.evidence == "RUST_MOD_DECLARATION" => {
                if let Some(mod_name) = r.target.strip_prefix("mod::") {
                    if let Some(resolved) = probe_rust_mod_on_disk(root, rel, mod_name) {
                        r.target = resolved;
                        r.is_external = false;
                    }
                }
            }
            ("rust", "IMPORTS" | "IMPORTS_SYMBOL") if !r.is_external => {
                let raw = r.target.as_str();
                if raw.starts_with("crate::")
                    || raw.starts_with("super::")
                    || raw.starts_with("self::")
                {
                    if let Some(resolved_file) = probe_rust_use_path_on_disk(root, rel, raw) {
                        if r.kind == "IMPORTS" {
                            extra_rels.push(Relationship {
                                source: rel.into(),
                                target: resolved_file,
                                kind: "IMPORTS".into(),
                                evidence: "RUST_USE_MODULE_FILE".into(),
                                confidence: 0.95,
                                line: r.line,
                                is_external: false,
                            });
                        } else if let Some(sym) = raw.rsplit("::").next() {
                            if sym != "*" {
                                r.target = format!("{resolved_file}::{sym}");
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    rels.extend(extra_rels);

    // Language-specific test file to production target heuristics (disk-probed first,
    // plus resolved import targets when test imports production modules).
    let file_name = Path::new(rel)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let mut test_targets: Vec<(String, &'static str, f64)> = Vec::new();

    if lang == "typescript" || lang == "javascript" {
        for suffix in [
            ".spec.ts",
            ".test.ts",
            ".spec.tsx",
            ".test.tsx",
            ".spec.js",
            ".test.js",
        ] {
            if let Some(prefix) = rel.strip_suffix(suffix) {
                let ext = if suffix.ends_with(".tsx") {
                    ".tsx"
                } else if suffix.ends_with(".js") {
                    ".js"
                } else {
                    ".ts"
                };
                let direct = format!("{prefix}{ext}");
                if root.join(&direct).is_file() {
                    test_targets.push((direct, "NAMING_CONVENTION", 0.95));
                } else if let Some(src_cand) =
                    direct.strip_prefix("tests/").map(|s| format!("src/{s}"))
                {
                    if root.join(&src_cand).is_file() {
                        test_targets.push((src_cand, "NAMING_CONVENTION", 0.92));
                    } else {
                        test_targets.push((direct, "NAMING_CONVENTION", 0.85));
                    }
                } else {
                    test_targets.push((direct, "NAMING_CONVENTION", 0.90));
                }
                break;
            }
        }
    } else if lang == "rust" && rel.starts_with("tests/") && rel.ends_with(".rs") {
        let stem = rel
            .trim_start_matches("tests/")
            .trim_end_matches(".rs")
            .trim_end_matches("_test");
        let cand_file = format!("src/{stem}.rs");
        let cand_mod = format!("src/{stem}/mod.rs");
        if root.join(&cand_mod).is_file() && !root.join(&cand_file).is_file() {
            test_targets.push((cand_mod, "NAMING_CONVENTION", 0.95));
        } else if root.join(&cand_file).is_file() {
            test_targets.push((cand_file, "NAMING_CONVENTION", 0.95));
        } else {
            test_targets.push((format!("{stem}.rs"), "RUST_TEST_STEM", 0.75));
        }
    } else if lang == "python"
        && (rel.starts_with("tests/")
            || file_name.starts_with("test_")
            || file_name.ends_with("_test.py"))
        && rel.ends_with(".py")
    {
        let stem = file_name
            .trim_end_matches(".py")
            .trim_start_matches("test_")
            .trim_end_matches("_test");
        if !stem.is_empty() {
            let candidates = [
                normalize_rel_path(&parent.join(format!("{stem}.py"))),
                format!("src/{stem}.py"),
                format!("app/{stem}.py"),
            ];
            if let Some(hit) = candidates.into_iter().find(|c| root.join(c).is_file()) {
                test_targets.push((hit, "NAMING_CONVENTION", 0.95));
            } else {
                test_targets.push((format!("{stem}.py"), "PYTHON_TEST_STEM", 0.75));
            }
        }
    } else if lang == "csharp" && (rel.ends_with("Tests.cs") || rel.ends_with("Test.cs")) {
        let stem = file_name
            .trim_end_matches("Tests.cs")
            .trim_end_matches("Test.cs");
        if !stem.is_empty() {
            let candidates = [
                normalize_rel_path(&parent.join(format!("{stem}.cs"))),
                format!("src/{stem}.cs"),
            ];
            if let Some(hit) = candidates.into_iter().find(|c| root.join(c).is_file()) {
                test_targets.push((hit, "NAMING_CONVENTION", 0.95));
            } else {
                test_targets.push((format!("{stem}.cs"), "CSHARP_TEST_STEM", 0.75));
            }
        }
    }

    // Also add TESTS hints from non-external workspace file imports inside detected test files
    let is_test_file = !test_targets.is_empty()
        || rel.starts_with("tests/")
        || file_name.starts_with("test_")
        || file_name.ends_with("_test.py")
        || file_name.ends_with("Tests.cs")
        || file_name.ends_with("Test.cs");
    if is_test_file {
        let import_targets: Vec<String> = rels
            .iter()
            .filter(|r| {
                r.kind == "IMPORTS"
                    && !r.is_external
                    && (r.target.ends_with(".ts")
                        || r.target.ends_with(".tsx")
                        || r.target.ends_with(".js")
                        || r.target.ends_with(".py")
                        || r.target.ends_with(".rs")
                        || r.target.ends_with(".cs"))
            })
            .map(|r| r.target.clone())
            .collect();
        for imp in import_targets {
            if imp != rel && !test_targets.iter().any(|(t, _, _)| t == &imp) {
                test_targets.push((imp, "TEST_IMPORT_TARGET", 0.90));
            }
        }
    }

    for (target, evidence, confidence) in test_targets {
        rels.push(Relationship {
            source: rel.into(),
            target,
            kind: "TESTS".into(),
            evidence: evidence.into(),
            confidence,
            line: 1,
            is_external: false,
        });
    }
}

fn probe_ts_path_on_disk(root: &Path, current_file: &str, raw_candidate: &str) -> Option<String> {
    if !raw_candidate.contains('/') && !raw_candidate.starts_with('.') {
        return None;
    }
    let norm = if raw_candidate.starts_with('.') {
        let p = Path::new(current_file).parent().unwrap_or(Path::new(""));
        normalize_rel_path(&p.join(raw_candidate))
    } else {
        raw_candidate.replace('\\', "/")
    };

    if root.join(&norm).is_file() {
        return Some(norm);
    }
    let stem = norm
        .strip_suffix(".ts")
        .or_else(|| norm.strip_suffix(".js"))
        .or_else(|| norm.strip_suffix(".tsx"))
        .or_else(|| norm.strip_suffix(".jsx"))
        .unwrap_or(&norm);

    for ext in ["ts", "tsx", "d.ts", "js", "jsx", "mjs", "cjs"] {
        let cand = format!("{stem}.{ext}");
        if root.join(&cand).is_file() {
            return Some(cand);
        }
    }
    for idx in ["index.ts", "index.tsx", "index.js", "index.jsx"] {
        let cand = format!("{stem}/{idx}");
        if root.join(&cand).is_file() {
            return Some(cand);
        }
    }
    None
}

fn probe_python_module_on_disk(root: &Path, current_file: &str, mod_spec: &str) -> Option<String> {
    let clean = mod_spec.trim();
    if clean.is_empty() {
        return None;
    }
    if clean.starts_with('.') {
        let dots = clean.chars().take_while(|c| *c == '.').count();
        let rest = &clean[dots..];
        let mut dir = Path::new(current_file)
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        for _ in 1..dots {
            if !dir.pop() {
                break;
            }
        }
        let rel_base = if rest.is_empty() {
            normalize_rel_path(&dir)
        } else {
            let sub = rest.replace('.', "/");
            if dir.as_os_str().is_empty() {
                sub
            } else {
                normalize_rel_path(&dir.join(sub))
            }
        };
        if !rel_base.is_empty() {
            let file_cand = format!("{rel_base}.py");
            if root.join(&file_cand).is_file() {
                return Some(file_cand);
            }
            let init_cand = format!("{rel_base}/__init__.py");
            if root.join(&init_cand).is_file() {
                return Some(init_cand);
            }
        } else if root.join("__init__.py").is_file() {
            return Some("__init__.py".to_string());
        }
        return None;
    }

    let mod_slash = clean.replace('.', "/");
    let parts: Vec<&str> = clean.split('.').collect();
    for prefix in ["", "src/", "app/"] {
        let file_cand = format!("{prefix}{mod_slash}.py");
        if root.join(&file_cand).is_file() {
            return Some(file_cand);
        }
        let init_cand = format!("{prefix}{mod_slash}/__init__.py");
        if root.join(&init_cand).is_file() {
            return Some(init_cand);
        }
        // `from pkg.mod import Item` or `from pkg.sub.mod import func` -> drop last segment if file
        if parts.len() >= 2 {
            let parent_slash = parts[..parts.len() - 1].join("/");
            let parent_cand = format!("{prefix}{parent_slash}.py");
            if root.join(&parent_cand).is_file() {
                return Some(parent_cand);
            }
        }
    }
    None
}

fn probe_rust_mod_on_disk(root: &Path, current_file: &str, mod_name: &str) -> Option<String> {
    let parent = Path::new(current_file).parent().unwrap_or(Path::new(""));
    let file_name = Path::new(current_file)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");
    let stem = Path::new(current_file)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let mut candidates = Vec::new();
    if file_name == "lib.rs" || file_name == "main.rs" || file_name == "mod.rs" {
        candidates.push(normalize_rel_path(&parent.join(format!("{mod_name}.rs"))));
        candidates.push(normalize_rel_path(&parent.join(mod_name).join("mod.rs")));
    } else if !stem.is_empty() {
        candidates.push(normalize_rel_path(
            &parent.join(stem).join(format!("{mod_name}.rs")),
        ));
        candidates.push(normalize_rel_path(
            &parent.join(stem).join(mod_name).join("mod.rs"),
        ));
        candidates.push(normalize_rel_path(&parent.join(format!("{mod_name}.rs"))));
    }
    candidates.into_iter().find(|c| root.join(c).is_file())
}

fn probe_rust_use_path_on_disk(root: &Path, current_file: &str, use_path: &str) -> Option<String> {
    let segs: Vec<&str> = use_path.split("::").collect();
    if segs.len() < 2 {
        return None;
    }
    let (base_dir, mod_segs): (String, &[&str]) = if segs[0] == "crate" {
        // Infer crate root dir from current_file (either src/ or projects/foo/src/)
        let norm = current_file.replace('\\', "/");
        if let Some(idx) = norm.find("/src/") {
            (format!("{}/src", &norm[..idx]), &segs[1..])
        } else if norm.starts_with("src/") {
            ("src".to_string(), &segs[1..])
        } else {
            ("src".to_string(), &segs[1..])
        }
    } else if segs[0] == "self" {
        let p = Path::new(current_file).parent().unwrap_or(Path::new(""));
        (normalize_rel_path(p), &segs[1..])
    } else if segs[0] == "super" {
        let mut p = Path::new(current_file)
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        p.pop();
        (normalize_rel_path(&p), &segs[1..])
    } else {
        return None;
    };

    // Try progressively shorter prefixes to match `<dir>/<seg1>/<seg2>.rs` or `<dir>/<seg1>/mod.rs`
    for len in (1..=mod_segs.len()).rev() {
        let sub = mod_segs[..len]
            .iter()
            .filter(|s| **s != "*")
            .copied()
            .collect::<Vec<_>>()
            .join("/");
        if sub.is_empty() {
            continue;
        }
        let direct = if base_dir.is_empty() {
            format!("{sub}.rs")
        } else {
            format!("{base_dir}/{sub}.rs")
        };
        if root.join(&direct).is_file() {
            return Some(direct);
        }
        let mod_rs = if base_dir.is_empty() {
            format!("{sub}/mod.rs")
        } else {
            format!("{base_dir}/{sub}/mod.rs")
        };
        if root.join(&mod_rs).is_file() {
            return Some(mod_rs);
        }
    }
    None
}
