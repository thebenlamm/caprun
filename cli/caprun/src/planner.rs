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
/// * `file_value_ids`   — tainted handles from `mint_from_read`. Unused by the
///                        email allow-path; `CreateFileFromReport` routes the FIRST
///                        such handle (when present) into `file.create/path` to
///                        drive the hostile-block path, else falls back to the
///                        trusted `intent_value_id` (clean allow-path).
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
pub fn plan_from_intent(
    intent: &CaprunIntent,
    intent_value_id: ValueId,
    file_value_ids: &[ValueId],
) -> PlanNode {
    match intent {
        CaprunIntent::SendEmailSummary { .. } => PlanNode {
            // The `..` intentionally ignores `recipient` — the literal already
            // lives in the broker's ValueStore, reachable only via `intent_value_id`.
            sink: SinkId("email.send".into()),
            args: vec![PlanArg {
                name: "to".into(),
                // Use the UserTrusted handle — not a file-derived tainted handle.
                // This is the clean allow-path: executor sees [UserTrusted] taint →
                // is_untrusted() = false → Allowed (HARD-02).
                value_id: intent_value_id,
            }],
        },
        CaprunIntent::CreateFileFromReport { .. } => {
            // Route `path` (routing-sensitive) by handle PROVENANCE, making both
            // §9 paths reachable for 07-05:
            //   * hostile — if the workspace read yielded a tainted RelativePath
            //     claim, route that (ExternalUntrusted, PathRaw) handle → the
            //     executor sees an untrusted routing arg → Block.
            //   * clean   — otherwise route the UserTrusted intent path → Allowed
            //     → the broker invokes the live file.create sink.
            // The planner only chooses a handle; it never sees the literal or taint
            // (PLAN-03) — the broker-owned ValueStore holds those, and the executor
            // (not the planner) makes the trust decision.
            let path_value_id = file_value_ids
                .first()
                .cloned()
                .unwrap_or_else(|| intent_value_id.clone());
            PlanNode {
                sink: SinkId("file.create".into()),
                args: vec![
                    PlanArg { name: "path".into(), value_id: path_value_id },
                    // `contents` is content-sensitive (WHAT is written), never
                    // routing-sensitive — a tainted value here does not block. Use
                    // the trusted intent handle so a value always resolves.
                    PlanArg { name: "contents".into(), value_id: intent_value_id },
                ],
            }
        }
    }
}
