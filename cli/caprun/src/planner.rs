/// planner — deterministic, non-LLM intent-to-plan-node mapper (PLAN-02)
///
/// # Security invariants (PLAN-03 / I2)
///
/// This module holds ONLY opaque `ValueId` handles — it NEVER sees:
///  - A `ValueRecord` (literal + taint + provenance_chain)
///  - A raw byte slice or string from untrusted content
///  - A taint label
///
/// The function signature enforces this at compile time: the only value-typed
/// parameters are `intent: &CaprunIntent` (typed, user-trusted) and `ValueId`
/// handles. The broker-owned `ValueStore` keeps the literals and taint; the
/// planner references values by their opaque handles only.
///
/// # No I/O, no async, infallible
///
/// `plan_from_intent` is a pure function. It performs no I/O, no async
/// operations, and is infallible (`-> PlanNode`, not `-> Result<PlanNode>`).
/// It MUST NOT call `ValueStore::mint` or construct a `ValueRecord`.

use runtime_core::{
    intent::CaprunIntent,
    plan_node::{PlanArg, PlanNode, SinkId, ValueId},
};

/// Map a typed `CaprunIntent` to a single `PlanNode`.
///
/// The planner holds ONLY opaque `ValueId` handles — never the literal or taint.
/// Taint lives in the broker-owned `ValueStore`; the planner is not aware of it.
///
/// # Arguments
/// * `intent`           — the typed user intent (enum, never free-form text).
/// * `intent_value_id`  — the `UserTrusted` `ValueId` minted by `mint_from_intent`
///                        (opaque handle for the user-provided literal, e.g. recipient).
/// * `_file_value_ids`  — tainted handles from `mint_from_read` (available for future
///                        mixed-path demos; NOT used on the clean allow-path).
///
/// # Returns
///
/// A `PlanNode` with the appropriate sink and args. All args are opaque `ValueId`
/// handles; no literals or taint labels appear in the returned node.
///
/// # Security (PLAN-03)
///
/// The `..` in each match arm intentionally ignores struct fields (e.g. `recipient`)
/// inside the `CaprunIntent` variant. The literal already lives in the broker's
/// `ValueStore`, accessible only via the returned `ValueId` handle — the planner
/// never needs (and must never access) the literal directly.
#[allow(dead_code)] // Used by worker.rs via `mod planner;`; stub phase only.
pub fn plan_from_intent(
    intent: &CaprunIntent,
    intent_value_id: ValueId,
    _file_value_ids: &[ValueId],
) -> PlanNode {
    // RED: stub — returns wrong sink so tests fail, proving RED state.
    // GREEN will replace this with the correct implementation.
    let _ = intent_value_id;
    match intent {
        CaprunIntent::SendEmailSummary { .. } => PlanNode {
            sink: SinkId("STUB_NOT_IMPLEMENTED".into()),
            args: vec![],
        },
    }
}
