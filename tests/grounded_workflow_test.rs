use std::fs;
use std::path::PathBuf;
use std::process::Command;

use engram::flywheel::{preflight_task, postflight_check, PreflightOptions, PostflightOptions};
use engram::contract::get_active_contract;
use engram::recovery::{recover_contract, RecoveryActionType, RecoveryStatus};
use engram::memory::{record_provisional_decision, load_durable_memories, MemoryLifecycleState};

fn setup_disposable_multifile_fixture() -> PathBuf {
    let ws = std::env::temp_dir().join(format!("engram_grounded_fixture_{}", uuid::Uuid::new_v4()));
    let src_domain = ws.join("src/domain");
    let src_services = ws.join("src/services");
    fs::create_dir_all(&src_domain).unwrap();
    fs::create_dir_all(&src_services).unwrap();

    fs::write(
        ws.join("package.json"),
        r#"{ "name": "order-system", "version": "1.0.0" }"#,
    )
    .unwrap();

    fs::write(
        src_domain.join("order.ts"),
        r#"
export interface OrderItem {
  id: string;
  price: number;
  quantity: number;
}

export class Order {
  constructor(public id: string, public items: OrderItem[]) {}

  calculateSubtotal(): number {
    return this.items.reduce((sum, item) => sum + (item.price * item.quantity), 0);
  }
}
"#,
    )
    .unwrap();

    fs::write(
        src_domain.join("payment.ts"),
        r#"
export enum PaymentStatus {
  PENDING = "PENDING",
  COMPLETED = "COMPLETED",
  FAILED = "FAILED",
}

export class PaymentGateway {
  charge(amount: number): PaymentStatus {
    return amount > 0 ? PaymentStatus.COMPLETED : PaymentStatus.FAILED;
  }
}
"#,
    )
    .unwrap();

    fs::write(
        src_services.join("checkout.ts"),
        r#"
import { Order } from "../domain/order";
import { PaymentGateway } from "../domain/payment";

export class CheckoutService {
  constructor(private gateway: PaymentGateway) {}

  process(order: Order): boolean {
    const total = order.calculateSubtotal();
    return this.gateway.charge(total) === "COMPLETED";
  }
}
"#,
    )
    .unwrap();

    // Initialize git repository
    let git_init = Command::new("git").arg("init").current_dir(&ws).status().unwrap();
    assert!(git_init.success());

    let _ = Command::new("git").args(["config", "user.name", "Test"]).current_dir(&ws).status();
    let _ = Command::new("git").args(["config", "user.email", "test@test.com"]).current_dir(&ws).status();
    let _ = Command::new("git").args(["add", "."]).current_dir(&ws).status();
    let _ = Command::new("git").args(["commit", "-m", "Initial commit"]).current_dir(&ws).status();

    // Initialize Engram and analyze repository
    let bin = env!("CARGO_BIN_EXE_engram");
    let init_res = Command::new(bin).arg("-C").arg(&ws).arg("init").status().unwrap();
    assert!(init_res.success());

    let analyze_res = Command::new(bin).arg("-C").arg(&ws).arg("analyze").status().unwrap();
    assert!(analyze_res.success());

    ws
}

#[test]
fn test_grounded_workflow_demonstration() {
    let ws = setup_disposable_multifile_fixture();

    // =========================================================================
    // Step 1 & 2: Preflight Context Compilation & Change Contract Locking
    // =========================================================================
    let preflight_opts = PreflightOptions {
        task: "Add percentage discount coupon calculation to Order".to_string(),
        token_budget: 1500,
        forbidden_boundaries: Some(vec!["src/domain/payment.ts".to_string()]),
    };

    let preflight_res = preflight_task(&ws, &preflight_opts).expect("preflight must succeed");
    assert!(preflight_res.context_files > 0, "Context files must be compiled");
    assert!(preflight_res.context_tokens <= 1500, "Must be within budget limit");

    // Lock Change Contract
    let contract_id = preflight_res.contract_id;
    let active_contract = get_active_contract(&ws).expect("must query active contract")
        .expect("active contract must exist");
    assert_eq!(active_contract.contract_id, contract_id);
    assert_eq!(active_contract.task_goal, "Add percentage discount coupon calculation to Order");

    // =========================================================================
    // Step 3: Valid In-Boundary File Mutation
    // =========================================================================
    let order_path = ws.join("src/domain/order.ts");
    let valid_mutation = r#"
export interface OrderItem {
  id: string;
  price: number;
  quantity: number;
}

export class Order {
  constructor(public id: string, public items: OrderItem[], public discountPercent: number = 0) {}

  calculateSubtotal(): number {
    return this.items.reduce((sum, item) => sum + (item.price * item.quantity), 0);
  }

  calculateTotal(): number {
    const subtotal = this.calculateSubtotal();
    const discount = subtotal * (this.discountPercent / 100);
    return Math.max(0, subtotal - discount);
  }
}
"#;
    fs::write(&order_path, valid_mutation).unwrap();

    // =========================================================================
    // Step 4: Postflight Validation of In-Boundary Mutation
    // =========================================================================
    let postflight_opts = PostflightOptions {
        contract_id: Some(contract_id.clone()),
        strict: false,
        staged: false,
        base_commit: None,
        ci_mode: false,
    };

    let postflight_res = postflight_check(&ws, &postflight_opts).expect("postflight check must execute");
    assert_eq!(postflight_res.contract_id, contract_id);
    assert_eq!(postflight_res.verdict, engram::flywheel::PostflightVerdict::Pass);
    assert_eq!(postflight_res.boundary_violation_count, 0);

    // =========================================================================
    // Step 5: Deliberate Out-of-Boundary Mutation Attempt
    // =========================================================================
    let payment_path = ws.join("src/domain/payment.ts");
    let forbidden_mutation = r#"
export enum PaymentStatus {
  PENDING = "PENDING",
  COMPLETED = "COMPLETED",
  FAILED = "FAILED",
}

export class PaymentGateway {
  charge(amount: number): PaymentStatus {
    // Deliberate out-of-boundary bypass modifying payment fee logic
    return PaymentStatus.COMPLETED;
  }
}
"#;
    fs::write(&payment_path, forbidden_mutation).unwrap();

    let postflight_violation_opts = PostflightOptions {
        contract_id: Some(contract_id.clone()),
        strict: true,
        staged: false,
        base_commit: None,
        ci_mode: false,
    };

    let violation_res = postflight_check(&ws, &postflight_violation_opts).expect("postflight must return violation report");
    assert_eq!(violation_res.verdict, engram::flywheel::PostflightVerdict::BlockedBoundaryViolation);
    assert!(violation_res.boundary_violation_count > 0, "Must detect boundary violation");
    let violation_summary = format!("{:?}", violation_res);
    assert!(
        violation_summary.contains("payment.ts"),
        "Violation must explicitly identify the out-of-boundary file"
    );

    // =========================================================================
    // Step 6: Non-Destructive Recovery Response & Postflight Clearance
    // =========================================================================
    let recovery_plan = recover_contract(&ws, &contract_id).expect("recovery formulation must succeed");
    assert_eq!(recovery_plan.contract_id, contract_id);
    assert_eq!(recovery_plan.attempt_count, 1, "Must be attempt 1");
    assert_eq!(recovery_plan.max_attempts, 2, "Max attempts must be 2");
    assert_eq!(recovery_plan.status, RecoveryStatus::InProgress, "Recovery status must be InProgress");
    assert!(
        !recovery_plan.plan.actions.is_empty(),
        "Recovery recipe must provide non-destructive repair actions"
    );

    // 6a. Prove that the recovery plan targets the forbidden payment.ts for reversion
    assert!(
        recovery_plan.plan.actions.iter().any(|a| {
            a.action_type == RecoveryActionType::RevertMutation && a.target.contains("payment.ts")
        }),
        "Recovery plan must include RevertMutation for the forbidden payment.ts"
    );

    // 6b. Prove that the recovery plan preserves the authorized order.ts change
    assert!(
        recovery_plan.plan.actions.iter().any(|a| {
            a.action_type == RecoveryActionType::PreserveRequestedChange && a.target.contains("order.ts")
        }),
        "Recovery plan must include PreserveRequestedChange for the authorized order.ts"
    );

    // 6c. Confirm valid order.ts modification remains intact on disk
    let order_after_recovery = fs::read_to_string(&order_path).unwrap();
    assert!(
        order_after_recovery.contains("calculateTotal"),
        "Authorized order.ts edit must remain intact on disk after recovery planning"
    );
    assert!(
        order_after_recovery.contains("discountPercent"),
        "Authorized discount logic in order.ts must be preserved"
    );

    // 6d. Confirm forbidden payment.ts was NOT autonomously modified on disk by recovery planning
    // (Engram proposes recovery actions rather than silently editing source code)
    let payment_after_recovery = fs::read_to_string(&payment_path).unwrap();
    assert_eq!(
        payment_after_recovery, forbidden_mutation,
        "Recovery planning must be strictly non-destructive on source code"
    );

    // 6e. Simulate applying the recipe (revert payment.ts to baseline git state)
    let git_checkout = Command::new("git")
        .args(["checkout", "src/domain/payment.ts"])
        .current_dir(&ws)
        .status()
        .unwrap();
    assert!(git_checkout.success(), "Git checkout must succeed in reverting payment.ts");

    // 6f. Postflight re-check: must pass cleanly and clear the contract
    let postflight_cleared = postflight_check(&ws, &postflight_opts)
        .expect("postflight after repair must succeed");
    assert_eq!(
        postflight_cleared.verdict,
        engram::flywheel::PostflightVerdict::Pass,
        "Postflight must pass after reverting forbidden file"
    );
    assert_eq!(postflight_cleared.boundary_violation_count, 0);

    // 6g. Confirm order.ts still contains the valid discount feature after postflight pass
    let order_final = fs::read_to_string(&order_path).unwrap();
    assert!(order_final.contains("calculateTotal"));

    // =========================================================================
    // Step 7: Memory Audit with Enforced Provisional Governance
    // =========================================================================
    let decision = record_provisional_decision(
        &ws,
        "Order Discount Ceiling Policy",
        "Discounts cannot exceed 50% without managerial review",
        "Prevent margin erosion on bulk items",
        vec!["order".to_string()],
        vec!["src/domain/order.ts".to_string()],
        None,
        "ai_agent_claude",
    )
    .expect("recording provisional decision must succeed");

    assert_eq!(decision.lifecycle_state, MemoryLifecycleState::Provisional);
    assert_eq!(decision.accepted_by, "ai_agent_claude");
    assert!(decision.ratified_at.is_none());
    assert!(decision.ratified_by.is_none());
    assert!(decision.tags.contains(&"provisional".to_string()));
    assert!(decision.tags.contains(&"mcp".to_string()));

    // Verify storage persistence
    let doc = load_durable_memories(&ws).expect("must load durable memories");
    let persisted = doc.memories.iter().find(|m| m.id == decision.id).expect("memory must be saved");
    assert_eq!(persisted.lifecycle_state, MemoryLifecycleState::Provisional);
    assert!(persisted.ratified_at.is_none(), "Provisional decision must not be ratified");

    // Cleanup disposable fixture
    let _ = fs::remove_dir_all(&ws);
}
