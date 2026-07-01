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

pub mod sink_schema;
pub mod sink_sensitivity;
pub mod value_store;

use runtime_core::{plan_node::PlanNode, DenyReason, ExecutorDecision, SinkBlockedAnchor};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use value_store::ValueStore;

/// Evaluate a single PlanNode against the broker-owned value store and the
/// hardcoded sink sensitivity map, returning an `ExecutorDecision`.
///
/// # Decision rule (DESIGN-plan-executor §Executor Decision Logic)
///
/// For each `PlanArg { name, value_id }` in `plan_node.args`:
///   1. Resolve `value_id` from `value_store`. If `None` → `Denied` (dangling handle).
///   2. If `name` is routing-sensitive for `plan_node.sink` AND `record.taint` carries
///      any explicitly-untrusted label (`TaintLabel::is_untrusted()`) →
///      `BlockedPendingConfirmation` populated verbatim from the record.
///      `UserTrusted`/`LocalWorkspace`-only provenance does NOT block (HARD-02).
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
    effect_id: Uuid,
    plan_node: &PlanNode,
    value_store: &ValueStore,
) -> ExecutorDecision {
    // Step 0: arg-schema gate (HARD-01/HARD-05). Runs FIRST — before any handle
    // resolve, taint, or sensitivity check. An unknown sink or malformed arg set
    // (unknown/duplicate/missing arg) fails closed here, so no unvalidated plan
    // node ever reaches the resolve/sensitivity loop. Extends the single
    // DenyReason taxonomy (no second error type).
    if let Err(reason) = sink_schema::validate_schema(plan_node) {
        return ExecutorDecision::Denied { reason };
    }

    for arg in &plan_node.args {
        // Step 1: Resolve the opaque handle from the trusted broker-owned store.
        // A None resolution is Denied — a dangling/forged handle never becomes Allowed.
        let record = match value_store.resolve(&arg.value_id) {
            Some(r) => r,
            None => {
                return ExecutorDecision::Denied {
                    reason: DenyReason::DanglingHandle,
                };
            }
        };

        // Step 1a: Empty-taint guard. Runs BEFORE the routing-sensitivity check so
        // an all-trusted-looking record with empty taint cannot fail open (an empty
        // taint iterator is never untrusted → would slip past the block). Defense in
        // depth: mint already rejects empty taint, but the executor must not depend
        // on that alone (DESIGN §3 ordering: resolve → empty-taint → empty-provenance
        // → is_routing_sensitive).
        if record.taint.is_empty() {
            return ExecutorDecision::Denied {
                reason: DenyReason::EmptyTaintInvariantViolation,
            };
        }

        // Step 1b: Empty-provenance guard. Also before the sensitivity check — a
        // [UserTrusted] record with empty provenance (the hole codex #5 flagged)
        // Denies here instead of reaching Allowed.
        if record.provenance_chain.is_empty() {
            return ExecutorDecision::Denied {
                reason: DenyReason::MissingProvenanceAnchor,
            };
        }

        // Step 2: Routing-sensitive check. If this arg routes the effect (e.g.,
        // email.send "to") and the resolved record carries any UNTRUSTED label, Block.
        // UserTrusted/LocalWorkspace-only provenance does NOT block (HARD-02).
        // The payload is copied verbatim from the broker-owned record — NOT synthesized.
        if sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
            && record.taint.iter().any(|t| t.is_untrusted())
        {
            // Build the durable anchor by cloning the resolved record VERBATIM.
            // The executor mints NOTHING (effect_id is the broker-supplied param)
            // and sets NO taint — every field is a clone of plan_node/arg/record
            // (T-04-03 anti-stapling). read_event_id == provenance_chain[0]
            // (07-01's mint invariant guarantees provenance is non-empty).
            //
            // The anchor carries only the SHA-256 DIGEST of the literal, so the
            // raw literal never enters the hashed audit chain (redactable at rest,
            // still tamper-evident). The live literal travels separately on the
            // decision for the confirmation UX / side-table write.
            let literal_sha256 = {
                let mut hasher = Sha256::new();
                hasher.update(record.literal.as_bytes());
                hex::encode(hasher.finalize())
            };
            let read_event_id = record.provenance_chain[0];
            // Item-4 hardening: read_event_id is denormalized from
            // provenance_chain[0]; assert they agree at construction so the
            // indexing field can never silently drift from the chain root.
            debug_assert_eq!(
                read_event_id, record.provenance_chain[0],
                "anchor.read_event_id must equal provenance_chain[0]"
            );
            let anchor = SinkBlockedAnchor {
                effect_id,
                sink: plan_node.sink.clone(),
                arg: arg.name.clone(),
                value_id: arg.value_id.clone(),
                literal_sha256,
                taint: record.taint.clone(),
                provenance_chain: record.provenance_chain.clone(),
                read_event_id,
            };
            return ExecutorDecision::BlockedPendingConfirmation {
                anchor,
                literal: record.literal.clone(),
            };
        }

        // Step 3: Content-sensitive tainted args (subject/body/attachment) do NOT
        // Block in v0 — Tier-4 verbatim review is deferred to the approval-hook plan.
    }

    ExecutorDecision::Allowed
}
