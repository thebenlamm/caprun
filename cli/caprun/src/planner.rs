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
///
/// # Routed by CALL-SITE CONVENTION, not provenance (Phase 15 finding #7)
///
/// `to`/`path` and `body` are placed by whichever handle the CALLER (the
/// confined worker) hands in via `derived_recipient`/`body` — the planner
/// structurally CANNOT see provenance or taint (PLAN-03), so it never makes a
/// "which handle is tainted" decision itself; it only places named handles by
/// convention. The executor (not the planner) makes the trust decision.

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
///                        for the intent's primary trusted literal (email
///                        recipient / file.create path).
/// * `derived_recipient` — routed by CALL-SITE CONVENTION (finding #7), not
///                        provenance: for `SendEmailSummary`, `Some` iff the
///                        confined worker derived a genuine multi-fragment
///                        recipient (finding #8's resolved fork); for
///                        `CreateFileFromReport`, `Some` iff a tainted
///                        workspace-derived path claim exists. `None` in
///                        either case falls back to `intent_value_id`.
/// * `body`             — routed by CALL-SITE CONVENTION: `Some` iff the
///                        worker extracted a tainted body fragment (email
///                        only); `None` falls back to `trusted_body_handle`.
///                        Unused by `CreateFileFromReport`.
/// * `trusted_subject_handle` — the UserTrusted handle for `email.send/subject`
///                        (Phase 15 finding #6 — always a DISTINCT handle from
///                        `intent_value_id`, never the literal). Unused by
///                        `CreateFileFromReport`.
/// * `trusted_body_handle` — the UserTrusted fallback handle for
///                        `email.send/body` when `body` is `None` (finding
///                        #6 — distinct from `intent_value_id`/
///                        `trusted_subject_handle`). Unused by
///                        `CreateFileFromReport`.
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
    derived_recipient: Option<ValueId>,
    body: Option<ValueId>,
    trusted_subject_handle: ValueId,
    trusted_body_handle: ValueId,
) -> PlanNode {
    match intent {
        CaprunIntent::SendEmailSummary { .. } => {
            // `to`: the derived (doc-sourced) recipient handle when present
            // (hostile path — routing-sensitive, tainted → Block downstream),
            // else the UserTrusted intent handle (clean path → Allowed).
            let to = derived_recipient.unwrap_or_else(|| intent_value_id.clone());
            // `body`: the derived (doc-sourced) tainted body handle when
            // present (content-sensitive, tainted → Block downstream), else
            // the UserTrusted trusted-body handle (clean path → Allowed).
            let body_value_id = body.unwrap_or(trusted_body_handle);
            PlanNode {
                sink: SinkId("email.send".into()),
                args: vec![
                    PlanArg { name: "to".into(), value_id: to },
                    // `subject` is ALWAYS the UserTrusted handle — Phase 15
                    // (EXTRACT-01) introduces no doc-derived subject
                    // extraction; a genuinely distinct handle from
                    // `intent_value_id`/`trusted_body_handle` (finding #6),
                    // so a clean send is not degenerately
                    // to==subject==body==recipient.
                    PlanArg { name: "subject".into(), value_id: trusted_subject_handle },
                    PlanArg { name: "body".into(), value_id: body_value_id },
                ],
            }
        }
        CaprunIntent::CreateFileFromReport { .. } => {
            // Route `path` (routing-sensitive) by handle CALL-SITE CONVENTION,
            // making both §9 paths reachable for 07-05:
            //   * hostile — if the workspace read yielded a tainted
            //     RelativePath claim, the caller passes it as
            //     `derived_recipient` (ExternalUntrusted, PathRaw) → the
            //     executor sees an untrusted routing arg → Block.
            //   * clean   — otherwise `derived_recipient` is None → route the
            //     UserTrusted intent path → Allowed → the broker invokes the
            //     live file.create sink.
            // The planner only chooses a handle; it never sees the literal or
            // taint (PLAN-03) — the broker-owned ValueStore holds those, and
            // the executor (not the planner) makes the trust decision.
            let path_value_id = derived_recipient.unwrap_or_else(|| intent_value_id.clone());
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
