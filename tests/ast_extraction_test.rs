use engram::analyzer::{
    generate_bounded_excerpt, is_external_target, normalize_signature, parse_file_ast,
};
use engram::storage::{self, EntityMember, ParseStatus, get_entity_members};
use rusqlite::Connection;
use std::{
    fs,
    path::{Path, PathBuf},
};

struct TempTestDir {
    path: PathBuf,
}

impl TempTestDir {
    fn new(prefix: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "engram_ast_test_{}_{}",
            prefix,
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).expect("failed to create temp test dir");
        Self { path: dir }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn test_01_typescript_angular_member_extraction() {
    let temp = TempTestDir::new("ts_angular");
    let ts_code = r#"
import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { LocalStorageService } from './local-storage.service';

@Injectable({
  providedIn: 'root'
})
export class AuthService {
  private token: string | null = null;

  constructor(private http: HttpClient, private storage: LocalStorageService) {}

  public login(credentials: any): Promise<boolean>;
  public login(user: string, pass: string): Promise<boolean>;
  public login(arg1: any, arg2?: string): Promise<boolean> {
    return Promise.resolve(true);
  }

  public logout(): void {
    this.token = null;
  }
}

export interface UserSession {
  userId: string;
  getActive(): boolean;
}
"#;

    let parsed = parse_file_ast(
        temp.path(),
        "src/auth.service.ts",
        ts_code.as_bytes(),
        "rev1",
    )
    .expect("parsing failed");

    assert_eq!(parsed.parse_status, ParseStatus::Complete);
    assert!(!parsed.symbols.is_empty(), "expected symbols");

    let auth_class = parsed
        .symbols
        .iter()
        .find(|s| s.name == "AuthService" && s.kind == "CLASS")
        .expect("AuthService class symbol not found");

    let auth_members: Vec<&EntityMember> = parsed
        .members
        .iter()
        .filter(|m| m.entity_id == auth_class.id)
        .collect();

    // Verify constructor
    let ctor = auth_members
        .iter()
        .find(|m| m.member_kind == "constructor")
        .expect("constructor not found");
    assert!(ctor.signature.contains("constructor"));

    // Verify constructor injected fields: http and storage
    let http_field = auth_members
        .iter()
        .find(|m| m.name == "http" && m.member_kind == "field")
        .expect("injected field 'http' not found");
    assert_eq!(http_field.visibility, "private");
    assert_eq!(http_field.return_type.as_deref(), Some("HttpClient"));

    let storage_field = auth_members
        .iter()
        .find(|m| m.name == "storage" && m.member_kind == "field")
        .expect("injected field 'storage' not found");
    assert_eq!(storage_field.visibility, "private");
    assert_eq!(
        storage_field.return_type.as_deref(),
        Some("LocalStorageService")
    );

    // Verify token field
    let token_field = auth_members
        .iter()
        .find(|m| m.name == "token" && m.member_kind == "field")
        .expect("field 'token' not found");
    assert_eq!(token_field.visibility, "private");

    // Verify overloads
    let overloads: Vec<_> = auth_members
        .iter()
        .filter(|m| m.name == "login" && m.member_kind == "overload_signature")
        .collect();
    assert_eq!(
        overloads.len(),
        2,
        "expected 2 overload signatures for login"
    );

    // Verify implementing method
    let login_method = auth_members
        .iter()
        .find(|m| m.name == "login" && m.member_kind == "method")
        .expect("implementing method login not found");
    assert_eq!(login_method.visibility, "public");

    // Verify logout method
    let logout_method = auth_members
        .iter()
        .find(|m| m.name == "logout" && m.member_kind == "method")
        .expect("method logout not found");
    assert_eq!(logout_method.visibility, "public");

    // Verify interface UserSession
    let session_iface = parsed
        .symbols
        .iter()
        .find(|s| s.name == "UserSession" && s.kind == "INTERFACE")
        .expect("UserSession interface symbol not found");

    let iface_members: Vec<&EntityMember> = parsed
        .members
        .iter()
        .filter(|m| m.entity_id == session_iface.id)
        .collect();

    let uid_field = iface_members
        .iter()
        .find(|m| m.name == "userId")
        .expect("userId member not found");
    assert_eq!(uid_field.member_kind, "field");

    let active_method = iface_members
        .iter()
        .find(|m| m.name == "getActive")
        .expect("getActive member not found");
    assert_eq!(active_method.member_kind, "method");
}

#[test]
fn test_02_rust_ast_member_extraction() {
    let temp = TempTestDir::new("rust_ast");
    let rust_code = r#"
use std::collections::VecDeque;

#[derive(Debug, Clone, Default)]
pub struct WorkQueue<T: Send> {
    pub tasks: VecDeque<T>,
    capacity: usize,
}

pub trait Queue<T> {
    fn push(&mut self, item: T);
    fn pop(&mut self) -> Option<T>;
}

impl<T: Send> Queue<T> for WorkQueue<T> {
    pub fn push(&mut self, item: T) {
        self.tasks.push_back(item);
    }

    pub fn pop(&mut self) -> Option<T> {
        self.tasks.pop_front()
    }
}

impl<T: Send> WorkQueue<T> {
    pub fn new(cap: usize) -> Self {
        Self {
            tasks: VecDeque::new(),
            capacity: cap,
        }
    }

    pub async fn drain(&mut self) -> Vec<T> {
        Vec::new()
    }
}
"#;

    let parsed = parse_file_ast(temp.path(), "src/queue.rs", rust_code.as_bytes(), "rev1")
        .expect("parsing failed");

    assert_eq!(parsed.parse_status, ParseStatus::Complete);

    let struct_sym = parsed
        .symbols
        .iter()
        .find(|s| s.name == "WorkQueue" && s.kind == "STRUCT")
        .expect("WorkQueue struct symbol not found");

    let struct_members: Vec<&EntityMember> = parsed
        .members
        .iter()
        .filter(|m| m.entity_id == struct_sym.id)
        .collect();

    // Verify struct fields
    let tasks_field = struct_members
        .iter()
        .find(|m| m.name == "tasks")
        .expect("field 'tasks' not found");
    assert_eq!(tasks_field.visibility, "pub");
    assert_eq!(tasks_field.member_kind, "field");

    let cap_field = struct_members
        .iter()
        .find(|m| m.name == "capacity")
        .expect("field 'capacity' not found");
    assert_eq!(cap_field.visibility, "private");

    // Verify struct outer attributes
    assert!(
        tasks_field
            .decorators
            .as_ref()
            .unwrap_or(&String::new())
            .contains("derive")
    );

    // Verify methods linked to struct or impl
    let new_method = parsed
        .members
        .iter()
        .find(|m| m.name == "new" && m.member_kind == "method")
        .expect("method 'new' not found");
    assert_eq!(new_method.visibility, "pub");

    let drain_method = parsed
        .members
        .iter()
        .find(|m| m.name == "drain" && m.member_kind == "method")
        .expect("method 'drain' not found");
    assert!(drain_method.is_async, "expected drain method to be async");

    // Verify Trait members
    let trait_sym = parsed
        .symbols
        .iter()
        .find(|s| s.name == "Queue" && s.kind == "TRAIT")
        .expect("Queue trait symbol not found");

    let trait_members: Vec<&EntityMember> = parsed
        .members
        .iter()
        .filter(|m| m.entity_id == trait_sym.id)
        .collect();
    assert!(trait_members.iter().any(|m| m.name == "push"));
    assert!(trait_members.iter().any(|m| m.name == "pop"));
}

#[test]
fn test_03_signature_normalization_and_id_determinism() {
    let sig1 = "  public   getItem < T > ( key :  string ,  options ?: any ) : T | null { ";
    let norm1 = normalize_signature(sig1);
    assert_eq!(
        norm1,
        "public getItem<T>(key: string, options?: any): T | null"
    );

    // Idempotency: f(f(x)) == f(x)
    assert_eq!(normalize_signature(&norm1), norm1);

    // Rust path with double-colon
    let rust_sig = "pub  fn  new ( cap :  usize ) -> std :: collections :: VecDeque < T > {";
    let norm_rust = normalize_signature(rust_sig);
    assert_eq!(
        norm_rust,
        "pub fn new(cap: usize) -> std::collections::VecDeque<T>"
    );
    assert_eq!(normalize_signature(&norm_rust), norm_rust);

    // Deterministic member ID
    let entity_id = "ent_test_entity_123456";
    let id1 = storage::member_id(entity_id, "getItem", &norm1);
    let id2 = storage::member_id(entity_id, "getItem", &normalize_signature(sig1));
    assert_eq!(
        id1, id2,
        "member_id must be deterministic regardless of whitespace"
    );
    assert_eq!(id1.len(), 16, "member_id must be 16 chars");
}

#[test]
fn test_04_bounded_excerpt_generation() {
    // 1. Small block (<= 40 lines): should return COMPLETE
    let small_lines: Vec<&str> = (1..=25).map(|_| "  const x = 1;").collect();
    let members = Vec::new();
    let (excerpt_small, state_small) =
        generate_bounded_excerpt(&small_lines, 1, 25, &members, "typescript");
    assert_eq!(state_small, "COMPLETE");
    assert!(excerpt_small.starts_with("[COMPLETE_BLOCK]"));

    // 2. Large block (> 40 lines): should return CONTRACTED
    let mut large_lines: Vec<String> = Vec::new();
    large_lines.push("export class LargeService {".to_string());
    for i in 1..=60 {
        large_lines.push(format!("  // Line {}", i));
    }
    large_lines.push("}".to_string());

    let line_refs: Vec<&str> = large_lines.iter().map(String::as_str).collect();
    let sample_members = vec![
        EntityMember::new(
            "ent_1",
            "constructor",
            "constructor",
            "constructor(api: ApiService)",
            2,
            5,
        ),
        EntityMember {
            member_id: "m1".into(),
            entity_id: "ent_1".into(),
            name: "fetchData".into(),
            member_kind: "method".into(),
            signature: "public fetchData(): Promise<Data>".into(),
            return_type: Some("Promise<Data>".into()),
            start_line: 10,
            end_line: 25,
            visibility: "public".into(),
            is_async: false,
            is_static: false,
            decorators: None,
        },
    ];

    let (excerpt_large, state_large) =
        generate_bounded_excerpt(&line_refs, 1, 62, &sample_members, "typescript");
    assert_eq!(state_large, "CONTRACTED");
    assert!(excerpt_large.starts_with("[CONTRACTED_SIGNATURES]"));
    assert!(excerpt_large.contains("constructor(api: ApiService) { /* ... */ }"));
    assert!(excerpt_large.contains("public fetchData(): Promise<Data> { /* ... */ }"));
    assert!(excerpt_large.ends_with('}'));
}

#[test]
fn test_05_partial_parsing_recovery_malformed_ts() {
    let temp = TempTestDir::new("malformed_ts");
    let broken_ts = r#"
export class ResilientService {
  public validMethod(): string {
    return "healthy";
  }

  // Syntax corruption
  broken @@ invalid %% syntax ?? {{

  public anotherValidMethod(): number {
    return 42;
  }
}
"#;

    let parsed = parse_file_ast(temp.path(), "src/broken.ts", broken_ts.as_bytes(), "rev1")
        .expect("parser must never panic or fail on malformed files");

    // Must report Partial status
    assert_eq!(parsed.parse_status, ParseStatus::Partial);

    // Must extract the class symbol and valid surrounding methods
    let class_sym = parsed
        .symbols
        .iter()
        .find(|s| s.name == "ResilientService")
        .expect("ResilientService class must be extracted despite partial syntax errors");

    let members: Vec<&EntityMember> = parsed
        .members
        .iter()
        .filter(|m| m.entity_id == class_sym.id)
        .collect();

    assert!(
        members.iter().any(|m| m.name == "validMethod"),
        "validMethod must be recovered"
    );
}

#[test]
fn test_06_partial_parsing_recovery_malformed_rust() {
    let temp = TempTestDir::new("malformed_rust");
    let broken_rust = r#"
pub struct HealthyStruct {
    pub count: usize,
}

// Corrupted syntax in file
fn broken_syntax( { #[invalid !! ]

pub trait HealthyTrait {
    fn check(&self) -> bool;
}
"#;

    let parsed = parse_file_ast(temp.path(), "src/broken.rs", broken_rust.as_bytes(), "rev1")
        .expect("parser must not fail");

    assert_eq!(parsed.parse_status, ParseStatus::Partial);

    assert!(
        parsed.symbols.iter().any(|s| s.name == "HealthyStruct"),
        "HealthyStruct must be extracted"
    );
    assert!(
        parsed.symbols.iter().any(|s| s.name == "HealthyTrait"),
        "HealthyTrait must be extracted"
    );
}

#[test]
fn test_07_clean_import_extraction_no_syntax_noise() {
    let temp = TempTestDir::new("clean_imports");
    let ts_code = r#"
import { Injectable } from '@angular/core';
import { tap, map, catchError } from 'rxjs/operators';
import { LocalStorageService } from './local-storage.service';
import { AppState } from '../state/app.state';

export class DummyService {}
"#;

    let parsed = parse_file_ast(
        temp.path(),
        "src/dummy.service.ts",
        ts_code.as_bytes(),
        "rev1",
    )
    .expect("parsing failed");

    for rel in &parsed.relationships {
        // Assert no raw syntax noise leaked into targets
        assert!(
            !rel.target.contains("{ tap"),
            "target leaked syntax: {}",
            rel.target
        );
        assert!(
            !rel.target.contains("{ Injectable"),
            "target leaked syntax: {}",
            rel.target
        );
        assert!(
            !rel.target.contains("import "),
            "target leaked syntax: {}",
            rel.target
        );
    }

    // Verify first-party vs third-party is_external classification
    assert!(is_external_target("@angular/core"));
    assert!(is_external_target("rxjs/operators"));
    assert!(!is_external_target("./local-storage.service.ts"));

    let angular_import = parsed
        .relationships
        .iter()
        .find(|r| r.target == "@angular/core" && r.kind == "IMPORTS")
        .expect("@angular/core import missing");
    assert!(
        angular_import.is_external,
        "@angular/core must be classified as external"
    );

    let rxjs_import = parsed
        .relationships
        .iter()
        .find(|r| r.target == "rxjs/operators" && r.kind == "IMPORTS")
        .expect("rxjs/operators import missing");
    assert!(
        rxjs_import.is_external,
        "rxjs/operators must be classified as external"
    );

    let local_storage_import = parsed
        .relationships
        .iter()
        .find(|r| r.target.ends_with("local-storage.service.ts") && r.kind == "IMPORTS")
        .expect("local-storage.service.ts import missing");
    assert!(
        !local_storage_import.is_external,
        "local-storage.service must be classified as internal"
    );

    // Verify clean symbol import specifier
    let sym_import = parsed
        .relationships
        .iter()
        .find(|r| {
            r.target
                .ends_with("local-storage.service.ts::LocalStorageService")
        })
        .expect("clean symbol import specifier missing");
    assert_eq!(sym_import.kind, "IMPORTS_SYMBOL");
    assert!(!sym_import.is_external);
}

#[test]
fn test_08_database_persistence_and_entity_members() {
    let temp = TempTestDir::new("db_persistence");
    let db_path = temp.path().join("derived_symbols.db");
    let mut conn = Connection::open(&db_path).expect("open db");
    storage::init_db(&mut conn).expect("init db");

    let ts_code = r#"
export class OrderService {
  private id: string = "order_1";

  constructor(private repo: any) {}

  public processOrder(orderId: string): boolean {
    return true;
  }
}
"#;

    let parsed = parse_file_ast(
        temp.path(),
        "src/order.service.ts",
        ts_code.as_bytes(),
        "rev1",
    )
    .expect("parsing failed");

    // Insert into DB
    let tx = conn.transaction().expect("tx");
    for s in &parsed.symbols {
        tx.execute(
            "INSERT OR REPLACE INTO entities (entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, status, parse_status, is_external)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'OBSERVED', ?10, 0)",
            rusqlite::params![s.id, s.path, s.name, s.kind, s.line as i64, s.end_line as i64, s.signature, s.fingerprint, "rev1", parsed.parse_status.as_str()],
        ).expect("insert entity");
    }

    let inserted_members =
        storage::insert_entity_members(&tx, &parsed.members).expect("insert members");
    assert!(
        inserted_members >= 3,
        "expected at least 3 members (field, constructor, method)"
    );
    tx.commit().expect("commit");

    // Query entity members using storage API
    let order_class = parsed
        .symbols
        .iter()
        .find(|s| s.name == "OrderService")
        .unwrap();
    let stored_members = get_entity_members(&conn, &order_class.id).expect("get_entity_members");
    assert_eq!(stored_members.len(), parsed.members.len());

    assert!(
        stored_members
            .iter()
            .any(|m| m.name == "processOrder" && m.member_kind == "method")
    );
    assert!(
        stored_members
            .iter()
            .any(|m| m.name == "id" && m.member_kind == "field")
    );
    assert!(
        stored_members
            .iter()
            .any(|m| m.name == "constructor" && m.member_kind == "constructor")
    );
}

#[test]
fn test_09_incremental_indexing_with_entity_members() {
    let temp = TempTestDir::new("inc_members");
    let db_path = temp.path().join("derived_symbols.db");
    let mut conn = Connection::open(&db_path).expect("open db");
    storage::init_db(&mut conn).expect("init db");

    let v1_code = r#"
export class ServiceA {
  public oldMethod(): void {}
}
"#;
    let v2_code = r#"
export class ServiceA {
  public newMethod(): void {}
  public secondMethod(): number { return 1; }
}
"#;

    // Step 1: Parse and insert v1
    let parsed_v1 =
        parse_file_ast(temp.path(), "src/service_a.ts", v1_code.as_bytes(), "rev1").unwrap();
    let sym_v1 = parsed_v1
        .symbols
        .iter()
        .find(|s| s.name == "ServiceA")
        .unwrap();
    {
        let tx = conn.transaction().unwrap();
        tx.execute(
            "INSERT OR REPLACE INTO entities (entity_id, path, name, kind, start_line, end_line, signature, ast_fingerprint, created_at_revision, status, parse_status, is_external)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'OBSERVED', ?10, 0)",
            rusqlite::params![sym_v1.id, sym_v1.path, sym_v1.name, sym_v1.kind, sym_v1.line as i64, sym_v1.end_line as i64, sym_v1.signature, sym_v1.fingerprint, "rev1", "COMPLETE"],
        ).unwrap();
        storage::insert_entity_members(&tx, &parsed_v1.members).unwrap();
        tx.commit().unwrap();
    }

    let members_v1 = get_entity_members(&conn, &sym_v1.id).unwrap();
    assert_eq!(members_v1.len(), 1);
    assert_eq!(members_v1[0].name, "oldMethod");

    // Step 2: Incremental update to v2 - purge stale members and insert new
    let parsed_v2 =
        parse_file_ast(temp.path(), "src/service_a.ts", v2_code.as_bytes(), "rev2").unwrap();
    let mut members_v2 = parsed_v2.members;
    // Map entity id to existing
    for m in &mut members_v2 {
        m.entity_id = sym_v1.id.clone();
        m.member_id = storage::member_id(&m.entity_id, &m.name, &m.signature);
    }

    {
        let tx = conn.transaction().unwrap();
        tx.execute(
            "DELETE FROM entity_members WHERE entity_id = ?1",
            [&sym_v1.id],
        )
        .unwrap();
        storage::insert_entity_members(&tx, &members_v2).unwrap();
        tx.commit().unwrap();
    }

    let members_v2_stored = get_entity_members(&conn, &sym_v1.id).unwrap();
    assert_eq!(members_v2_stored.len(), 2);
    assert!(
        !members_v2_stored.iter().any(|m| m.name == "oldMethod"),
        "oldMethod must be purged"
    );
    assert!(
        members_v2_stored.iter().any(|m| m.name == "newMethod"),
        "newMethod must be present"
    );
    assert!(
        members_v2_stored.iter().any(|m| m.name == "secondMethod"),
        "secondMethod must be present"
    );
}

#[test]
fn test_10_unsupported_language_resilience() {
    let temp = TempTestDir::new("unsupported_lang");
    let random_content = b"SOME RAW DATA OR TEXT CONTENT";
    let parsed = parse_file_ast(temp.path(), "unknown_file.xyz", random_content, "rev1")
        .expect("must not panic on unknown language");

    assert_eq!(parsed.parse_status, ParseStatus::NotAvailable);
    assert!(parsed.symbols.is_empty());
    assert!(parsed.members.is_empty());
}

#[test]
fn test_11_signature_normalization_edge_cases() {
    // 1. String literal containing punctuation, spaces, delimiters, and arrows
    let str_lit_sig = r#"fn log(msg: &str = "hello, world : ) -> ") -> bool"#;
    let norm_str = normalize_signature(str_lit_sig);
    assert_eq!(
        norm_str,
        r#"fn log(msg: &str = "hello, world : ) -> ") -> bool"#
    );
    assert_eq!(
        normalize_signature(&norm_str),
        norm_str,
        "String literal normalization must be idempotent"
    );

    // 2. Character literals containing delimiters
    let char_lit_sig = r#"fn find(delim: char = ',') -> Option<usize>"#;
    let norm_char = normalize_signature(char_lit_sig);
    assert_eq!(norm_char, r#"fn find(delim: char = ',') -> Option<usize>"#);
    assert_eq!(
        normalize_signature(&norm_char),
        norm_char,
        "Character literal normalization must be idempotent"
    );

    // 3. Nested generic types
    let nested_sig = "  pub fn get_map < K , V > ( ) -> Map < String , Vec < Option < T > > > { ";
    let norm_nested = normalize_signature(nested_sig);
    assert_eq!(
        norm_nested,
        "pub fn get_map<K, V>() -> Map<String, Vec<Option<T>>>"
    );
    assert_eq!(normalize_signature(&norm_nested), norm_nested);

    // 4. Rust double-colon paths
    let path_sig = "pub fn create() -> std :: collections :: HashMap < String , Vec < u8 > >";
    let norm_path = normalize_signature(path_sig);
    assert_eq!(
        norm_path,
        "pub fn create() -> std::collections::HashMap<String, Vec<u8>>"
    );
    assert_eq!(normalize_signature(&norm_path), norm_path);

    // 5. TypeScript arrow functions and return types
    let ts_arrow_sig = "handler : ( cb : ( x : number ) => boolean ) => void";
    let norm_arrow = normalize_signature(ts_arrow_sig);
    assert_eq!(norm_arrow, "handler: (cb: (x: number) => boolean) => void");
    assert_eq!(normalize_signature(&norm_arrow), norm_arrow);

    // 6. Generic declaration followed by identifier (e.g. impl<T> Queue<T>)
    let impl_sig = "impl < T : Send >   WorkQueue < T >  { ";
    let norm_impl = normalize_signature(impl_sig);
    assert_eq!(norm_impl, "impl<T: Send> WorkQueue<T>");
    assert_eq!(normalize_signature(&norm_impl), norm_impl);

    // 7. Method overloads with distinct parameter types must not normalize to identical signatures
    let overload_a = normalize_signature("query(key: string): string;");
    let overload_b = normalize_signature("query(keys: string[]): string[];");
    assert_eq!(overload_a, "query(key: string): string");
    assert_eq!(overload_b, "query(keys: string[]): string[]");
    assert_ne!(
        overload_a, overload_b,
        "Semantically distinct overload signatures must not collapse"
    );

    // 8. Inline block and line comments removal
    let comment_sig = "fn calculate( /* factor */ x: f64, // base rate\n y: f64 ) -> f64 {";
    let norm_comment = normalize_signature(comment_sig);
    assert_eq!(norm_comment, "fn calculate(x: f64, y: f64) -> f64");
    assert_eq!(normalize_signature(&norm_comment), norm_comment);

    // 9. Comments inside string literals must NOT be stripped
    let comment_in_str = r#"fn url(u: &str = "http://example.com/*test*/") -> bool"#;
    let norm_comment_in_str = normalize_signature(comment_in_str);
    assert_eq!(
        norm_comment_in_str,
        r#"fn url(u: &str = "http://example.com/*test*/") -> bool"#
    );

    // 10. Rust lifetimes (must NOT be treated as opening character literals)
    let sig_lifetime_1 = "fn get < 'a > ( value : & 'a str ) -> & 'a str {";
    let norm_lt1 = normalize_signature(sig_lifetime_1);
    assert_eq!(norm_lt1, "fn get<'a>(value: &'a str) -> &'a str");
    assert_eq!(
        normalize_signature(&norm_lt1),
        norm_lt1,
        "Lifetime signature must be idempotent"
    );

    let sig_lifetime_2 =
        "fn transform < 'a , 'b > ( first : & 'a str , second : & 'b str ) -> & 'a str";
    let norm_lt2 = normalize_signature(sig_lifetime_2);
    assert_eq!(
        norm_lt2,
        "fn transform<'a, 'b>(first: &'a str, second: &'b str) -> &'a str"
    );
    assert_eq!(normalize_signature(&norm_lt2), norm_lt2);

    let sig_lifetime_static = "fn process < T : 'static > ( value : T )";
    let norm_static = normalize_signature(sig_lifetime_static);
    assert_eq!(norm_static, "fn process<T: 'static>(value: T)");
    assert_eq!(normalize_signature(&norm_static), norm_static);

    let sig_lifetime_char = "fn example ( value : char ) -> & 'static str";
    let norm_char_static = normalize_signature(sig_lifetime_char);
    assert_eq!(norm_char_static, "fn example(value: char) -> &'static str");
    assert_eq!(normalize_signature(&norm_char_static), norm_char_static);

    // 11. Lifetime semantic distinction in member IDs
    let id_a = storage::member_id("ent_svc", "get", "fn get<'a>(value: &'a str) -> &'a str");
    let id_b = storage::member_id("ent_svc", "get", "fn get<'b>(value: &'b str) -> &'b str");
    assert_ne!(
        id_a, id_b,
        "Distinct lifetime signatures must produce distinct member IDs"
    );

    // Formatting differences produce identical member IDs
    let id_fmt = storage::member_id(
        "ent_svc",
        "get",
        &normalize_signature("  fn   get < 'a > (  value :  & 'a  str  )  ->   & 'a  str  { "),
    );
    assert_eq!(
        id_a, id_fmt,
        "Formatting-only differences must produce identical member IDs"
    );

    // 12. Comment boundary preservation
    let comma_comment = "fn calculate(a: i32 /* comment */ , b: i32) -> i32";
    let norm_comma = normalize_signature(comma_comment);
    assert_eq!(norm_comma, "fn calculate(a: i32, b: i32) -> i32");

    let token_comment = "fn process/* comment */_value()";
    let norm_token = normalize_signature(token_comment);
    assert_eq!(
        norm_token, "fn process _value()",
        "Comment removal must not merge distinct lexical tokens"
    );

    let generic_comment = "fn example() -> Vec/* comment */<String>";
    let norm_gen = normalize_signature(generic_comment);
    assert_eq!(norm_gen, "fn example() -> Vec<String>");

    // 13. Nested block comments
    let nested_comment = "fn nested(/* outer /* inner */ still comment */ x: i32) -> i32";
    let norm_nested_comm = normalize_signature(nested_comment);
    assert_eq!(norm_nested_comm, "fn nested(x: i32) -> i32");
}

#[test]
fn test_12_java_ast_extraction_and_dependency_classification() {
    let temp = TempTestDir::new("java_deps");

    // Set up workspace directory structure:
    // src/main/java/com/example/model/User.java
    // src/main/java/com/example/service/UserService.java
    // src/test/java/com/example/service/UserServiceTest.java
    let model_dir = temp.path().join("src/main/java/com/example/model");
    let service_dir = temp.path().join("src/main/java/com/example/service");
    let test_dir = temp.path().join("src/test/java/com/example/service");

    fs::create_dir_all(&model_dir).unwrap();
    fs::create_dir_all(&service_dir).unwrap();
    fs::create_dir_all(&test_dir).unwrap();

    let user_code = r#"
package com.example.model;

public class User {
    private String id;
    public String getId() { return id; }
}
"#;
    fs::write(model_dir.join("User.java"), user_code).unwrap();

    let service_code = r#"
package com.example.service;

import com.example.model.User;
import java.time.Instant;
import java.util.List;
import org.springframework.stereotype.Service;

public interface UserServiceInterface {
    User getUser(String id);
}

@Service
public class UserService implements UserServiceInterface {
    public User getUser(String id) {
        return new User();
    }
}
"#;
    fs::write(service_dir.join("UserService.java"), service_code).unwrap();

    let test_code = r#"
package com.example.service;

public class UserServiceTest {
    public void testGetUser() {}
}
"#;
    fs::write(test_dir.join("UserServiceTest.java"), test_code).unwrap();

    // Parse UserService.java
    let parsed_service = parse_file_ast(
        temp.path(),
        "src/main/java/com/example/service/UserService.java",
        service_code.as_bytes(),
        "rev1",
    )
    .expect("Java service parsing failed");

    assert_eq!(parsed_service.parse_status, ParseStatus::Complete);
    assert!(!parsed_service.symbols.is_empty());

    // 1. Verify workspace-local Java dependency is classified as internal (is_external: false)
    let local_import = parsed_service
        .relationships
        .iter()
        .find(|r| r.target.ends_with("User.java") && r.kind == "IMPORTS")
        .expect("Local User.java import relationship missing");
    assert!(
        !local_import.is_external,
        "Local Java source must not be classified as external"
    );
    assert_eq!(local_import.evidence, "LOCAL_JAVA_SOURCE");

    // 2. Verify standard library imports are classified as external (is_external: true)
    let stdlib_import = parsed_service
        .relationships
        .iter()
        .find(|r| r.target == "java.time.Instant" && r.kind == "IMPORTS")
        .expect("java.time.Instant import missing");
    assert!(
        stdlib_import.is_external,
        "Java stdlib import must be classified as external"
    );
    assert_eq!(stdlib_import.evidence, "JAVA_STDLIB");

    let util_import = parsed_service
        .relationships
        .iter()
        .find(|r| r.target == "java.util.List" && r.kind == "IMPORTS")
        .expect("java.util.List import missing");
    assert!(
        util_import.is_external,
        "java.util.List import must be classified as external"
    );

    // 3. Verify external package import is classified as external (is_external: true)
    let spring_import = parsed_service
        .relationships
        .iter()
        .find(|r| r.target == "org.springframework.stereotype.Service" && r.kind == "IMPORTS")
        .expect("Spring Service import missing");
    assert!(
        spring_import.is_external,
        "Third-party dependency must be classified as external"
    );
    assert_eq!(spring_import.evidence, "EXTERNAL_DEPENDENCY");

    // 4. Verify interface implementation relationship
    let iface_rel = parsed_service
        .relationships
        .iter()
        .find(|r| r.target == "UserServiceInterface" && r.kind == "IMPLEMENTS")
        .expect("implements relationship missing");
    assert!(
        !iface_rel.is_external,
        "Interface implementation must be internal"
    );

    // 5. Parse UserServiceTest.java and verify test-to-target convention
    let parsed_test = parse_file_ast(
        temp.path(),
        "src/test/java/com/example/service/UserServiceTest.java",
        test_code.as_bytes(),
        "rev1",
    )
    .expect("Java test parsing failed");

    let test_rel = parsed_test
        .relationships
        .iter()
        .find(|r| r.kind == "TESTS")
        .expect("Test convention relationship missing");
    assert_eq!(
        test_rel.target,
        "src/main/java/com/example/service/UserService.java"
    );
    assert!(!test_rel.is_external);
    assert_eq!(test_rel.evidence, "MAVEN_GRADLE_TEST_CONVENTION");
}

#[test]
fn test_13_deleted_file_member_cleanup_and_reindexing() {
    let temp = TempTestDir::new("del_members");
    let root = temp.path();

    // Initialize git repository in temp test dir so incremental_analyze functions in Tier 1
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(root)
        .output()
        .expect("git init failed");
    std::process::Command::new("git")
        .args(["config", "user.name", "Engram Auditor"])
        .current_dir(root)
        .output()
        .unwrap();
    std::process::Command::new("git")
        .args(["config", "user.email", "auditor@engram.local"])
        .current_dir(root)
        .output()
        .unwrap();

    let src_dir = root.join("src");
    fs::create_dir_all(&src_dir).unwrap();

    let worker_path = src_dir.join("worker.ts");
    let worker_v1 = r#"
export class WorkerService {
  public compute(val: number): number {
    return val * 2;
  }
  public status(): string {
    return "ready";
  }
}
"#;
    fs::write(&worker_path, worker_v1).unwrap();

    // Step 1: Initial cold analysis
    let cold = engram::incremental::incremental_analyze(root).expect("Cold analysis failed");
    assert!(
        cold.tier1_reparsed_files
            .contains(&"src/worker.ts".to_string())
    );

    let conn = storage::db(root).unwrap();
    let worker_ent_id: String = conn
        .query_row(
            "SELECT entity_id FROM entities WHERE path = 'src/worker.ts' AND invalidated_at_revision IS NULL",
            [],
            |r| r.get(0),
        )
        .expect("Worker entity must exist");

    let initial_members = get_entity_members(&conn, &worker_ent_id).unwrap();
    assert_eq!(initial_members.len(), 2, "Expected 2 initial members");
    assert!(initial_members.iter().any(|m| m.name == "compute"));
    assert!(initial_members.iter().any(|m| m.name == "status"));

    // Step 2: Delete worker.ts from disk
    fs::remove_file(&worker_path).unwrap();

    // Step 3: Run incremental analysis detecting file deletion
    let incr_del = engram::incremental::incremental_analyze(root)
        .expect("Incremental analysis after delete failed");
    assert!(
        incr_del
            .tier1_deleted_files
            .contains(&"src/worker.ts".to_string())
    );

    // Verify entity was invalidated
    let invalidated_rev: Option<String> = conn
        .query_row(
            "SELECT invalidated_at_revision FROM entities WHERE path = 'src/worker.ts'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        invalidated_rev.is_some(),
        "Entity must have invalidated_at_revision set"
    );

    // Verify entity members were transactionally deleted and are no longer visible
    let members_after_del = get_entity_members(&conn, &worker_ent_id).unwrap();
    assert_eq!(
        members_after_del.len(),
        0,
        "Stale members must be completely purged from entity_members on file deletion"
    );

    // Step 4: Re-create worker.ts with a new method and incremental re-indexing
    let worker_v2 = r#"
export class WorkerService {
  public rebornMethod(): boolean {
    return true;
  }
}
"#;
    fs::write(&worker_path, worker_v2).unwrap();

    let incr_reborn = engram::incremental::incremental_analyze(root)
        .expect("Incremental analysis after recreate failed");
    assert!(
        incr_reborn
            .tier1_reparsed_files
            .contains(&"src/worker.ts".to_string())
    );

    let new_ent_id: String = conn
        .query_row(
            "SELECT entity_id FROM entities WHERE path = 'src/worker.ts' AND invalidated_at_revision IS NULL",
            [],
            |r| r.get(0),
        )
        .expect("Active worker entity must exist");

    let fresh_members = get_entity_members(&conn, &new_ent_id).unwrap();
    assert_eq!(fresh_members.len(), 1, "Expected exactly 1 new member");
    assert_eq!(fresh_members[0].name, "rebornMethod");
    assert!(
        !fresh_members.iter().any(|m| m.name == "compute"),
        "compute must not be present"
    );
    assert!(
        !fresh_members.iter().any(|m| m.name == "status"),
        "status must not be present"
    );
}
