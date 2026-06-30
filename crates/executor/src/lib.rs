/// executor — deterministic non-LLM I2 executor.
///
/// DEC-security-invariants: I2 is enforced by this crate, hardcoded in Rust TCB.
/// The decision function (`submit_plan_node`) is a pure function over a trusted
/// broker-owned value store and the hardcoded sink sensitivity map. No LLM in the
/// enforcement path.
///
/// Anti-stapling invariant (T-04-03): the executor reads taint ONLY through
/// `value_store.resolve()`. It NEVER mints a ValueRecord and NEVER sets a taint
/// field. The sole taint writer in the crate is `ValueStore::mint`.

pub mod sink_sensitivity;
pub mod value_store;

use runtime_core::{plan_node::PlanNode, ExecutorDecision};
use uuid::Uuid;
use value_store::ValueStore;

/// Evaluate a single PlanNode against the broker-owned value store and the
/// hardcoded sink sensitivity map, returning an `ExecutorDecision`.
///
/// # Decision rule (DESIGN-plan-executor §Executor Decision Logic)
///
/// For each `PlanArg { name, value_id }` in `plan_node.args`:
///   1. Resolve `value_id` from `value_store`. If `None` → `Denied` (dangling handle).
///   2. If `name` is routing-sensitive for `plan_node.sink` AND `record.taint` is
///      non-empty → `BlockedPendingConfirmation` populated verbatim from the record.
///   3. (Content-sensitive tainted args do not Block in v0 — marked for Tier-4
///      verbatim review, not yet surfaced.)
///
/// After all args pass: `Allowed`.
///
/// # Anti-stapling (T-04-03)
///
/// This function MUST NOT call `ValueStore::mint` and MUST NOT construct a
/// `ValueRecord`. The negative-grep acceptance criterion asserts:
///   grep -v '^[[:space:]]*//' crates/executor/src/lib.rs | grep -c 'ValueStore::mint'   → 0
///   grep -v '^[[:space:]]*//' crates/executor/src/lib.rs | grep -c 'ValueRecord {'      → 0
pub fn submit_plan_node(
    _session_id: Uuid,
    plan_node: &PlanNode,
    value_store: &ValueStore,
) -> ExecutorDecision {
    for arg in &plan_node.args {
        // Step 1: Resolve the opaque handle from the trusted broker-owned store.
        // A None resolution is Denied — a dangling/forged handle never becomes Allowed.
        let record = match value_store.resolve(&arg.value_id) {
            Some(r) => r,
            None => {
                return ExecutorDecision::Denied {
                    reason: format!(
                        "unresolvable handle for arg '{}': ValueId not in store",
                        arg.name
                    ),
                };
            }
        };

        // Step 2: Routing-sensitive check. If this arg routes the effect (e.g.,
        // email.send "to") and the resolved record carries ANY taint, Block.
        // The payload is copied verbatim from the broker-owned record — NOT synthesized.
        if sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
            && !record.taint.is_empty()
        {
            return ExecutorDecision::BlockedPendingConfirmation {
                literal_value: record.literal.clone(),
                sink: plan_node.sink.0.clone(),
                arg_name: arg.name.clone(),
                taint: record.taint.clone(),
                provenance_chain: record.provenance_chain.clone(),
            };
        }

        // Step 3: Content-sensitive tainted args (subject/body/attachment) do NOT
        // Block in v0 — Tier-4 verbatim review is deferred to the approval-hook plan.
    }

    ExecutorDecision::Allowed
}
