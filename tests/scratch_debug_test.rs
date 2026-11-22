
use std::fs;
use engram::analyzer::analyze;
use engram::storage::db;
use engram::impact::compute_impact_cone;

#[test]
fn test_debug_missing_deps() {
    let root = std::env::temp_dir().join(format!("engram_debug_missing_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(root.join("src")).unwrap();

    fs::write(root.join("src/valid.ts"), "export class Valid {}\n").unwrap();
    fs::write(
        root.join("src/consumer.ts"),
        r#"import { Valid } from './valid';
import { NonExistent } from './does_not_exist';
import { ExternalMissing } from 'non-existent-npm-package';

export class Consumer {}
"#,
    )
    .unwrap();

    analyze(&root).unwrap();
    let conn = db(&root).unwrap();

    let cone = compute_impact_cone(&conn, &root, "src/consumer.ts", 2).unwrap();

    println!("All entities:");
    for e in cone.all_entities() {
        println!("  path: {:<30} is_ext: {:<6} class: {:?}", e.path, e.is_external, e.classification);
    }

    let _ = fs::remove_dir_all(&root);
}
