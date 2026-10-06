use std::fs;
use std::path::PathBuf;

fn setup_test_repo() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("engram_integration_{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).unwrap();

    // Create a mini git repo or project structure
    fs::create_dir_all(dir.join("src")).unwrap();
    fs::write(
        dir.join("src/orders.py"),
        "class OrderService:\n    def export_csv(self):\n        pass\n",
    )
    .unwrap();
    fs::write(
        dir.join("src/index.ts"),
        "export interface IOrderService { exportCsv(): void; }\n",
    )
    .unwrap();
    fs::write(
        dir.join("src/OrderController.cs"),
        "public class OrderController { public void GetOrders() {} }\n",
    )
    .unwrap();

    dir
}

#[test]
fn test_end_to_end_lifecycle() {
    let repo = setup_test_repo();

    // 1. Test CLI initialization
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("-C")
        .arg(&repo)
        .arg("init")
        .output()
        .expect("failed to execute engram init");
    assert!(output.status.success());
    assert!(repo.join(".engram/constitution.yaml").exists());
    assert!(repo.join(".engram/config.yaml").exists());
    assert!(repo.join(".engramignore").exists());

    // 2. Test Repository Analysis
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("--json")
        .arg("-C")
        .arg(&repo)
        .arg("analyze")
        .output()
        .expect("failed to execute engram analyze");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"indexed_files\": 3"));
    assert!(stdout.contains("\"symbols\": 5"));

    // 3. Test Context Compilation
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("--json")
        .arg("-C")
        .arg(&repo)
        .arg("context")
        .arg("export csv orders")
        .arg("--token-budget")
        .arg("1000")
        .output()
        .expect("failed to execute engram context");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("OrderService"));

    // 4. Test Task Analysis & Contract
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("-C")
        .arg(&repo)
        .arg("analyze-task")
        .arg("export csv orders")
        .output()
        .expect("failed to execute analyze-task");
    assert!(output.status.success());
    let res: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let task_id = res["task_id"].as_str().unwrap();

    // 5. Generate Change Contract
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("-C")
        .arg(&repo)
        .arg("contract")
        .arg(task_id)
        .output()
        .expect("failed to execute contract");
    assert!(output.status.success());
    let contract: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(contract["task_id"], task_id);

    // 6. Test Decision & Memory Update
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("-C")
        .arg(&repo)
        .arg("decision")
        .arg(task_id)
        .arg("accepted")
        .arg("Clean implementation")
        .output()
        .expect("failed to execute decision");
    if !output.status.success() {
        eprintln!("Decision stderr: {}", String::from_utf8_lossy(&output.stderr));
        eprintln!("Decision stdout: {}", String::from_utf8_lossy(&output.stdout));
    }
    assert!(output.status.success());

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_engram"))
        .arg("-C")
        .arg(&repo)
        .arg("update-memory")
        .arg(task_id)
        .output()
        .expect("failed to execute update-memory");
    assert!(output.status.success());
    let mem: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(mem["promoted"], true);

    // Cleanup
    let _ = fs::remove_dir_all(&repo);
}
