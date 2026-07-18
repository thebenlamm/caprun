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

pub mod policy_gate;
pub mod sink_schema;
pub mod sink_sensitivity;
pub mod value_store;

use runtime_core::{
    plan_node::PlanNode, BlockedArg, DenyReason, ExecutorDecision, SessionPolicy, SessionStatus,
    SinkBlockedAnchor,
};
use sha2::{Digest, Sha256};
use sink_sensitivity::EffectClass;
use uuid::Uuid;
use value_store::ValueStore;

/// Evaluate a single PlanNode against the broker-owned value store and the
/// hardcoded sink sensitivity map, returning an `ExecutorDecision`.
///
/// # Decision rule (DESIGN-plan-executor §Executor Decision Logic; Phase 14
/// `planning-docs/DESIGN-content-adapter-mediation.md` "Collect-then-Block (D-14)")
///
/// For each `PlanArg { name, value_id }` in `plan_node.args`:
///   1. Resolve `value_id` from `value_store`. If `None` → `Denied` (dangling handle).
///   1a. Empty-taint guard → `Denied`.
///   1b. Empty-provenance guard → `Denied`.
///   2. If `name` is routing-sensitive OR content-sensitive for `plan_node.sink`
///      AND `record.taint` carries any explicitly-untrusted label
///      (`TaintLabel::is_untrusted()`) → collect a `BlockedArg` populated
///      verbatim from the record. `UserTrusted`/`LocalWorkspace`-only
///      provenance does NOT block (HARD-02).
///
/// The loop does NOT return on the first collected `BlockedArg` — it scans
/// EVERY arg on the plan node first (Collect-then-Block, D-14), so a plan node
/// carrying both a tainted routing-sensitive arg (e.g. `to`) and a tainted
/// content-sensitive arg (e.g. `body`) surfaces BOTH in one
/// `BlockedPendingConfirmation { anchors }` — never first-match-wins. Only
/// after the loop completes with an EMPTY collection does Step 0.5 run, then
/// `Allowed`.
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
    session_status: &SessionStatus,
    policy: &SessionPolicy,
) -> ExecutorDecision {
    // Step 0: arg-schema gate (HARD-01/HARD-05). Runs FIRST — before any handle
    // resolve, taint, or sensitivity check. An unknown sink or malformed arg set
    // (unknown/duplicate/missing arg) fails closed here, so no unvalidated plan
    // node ever reaches the resolve/sensitivity loop. Extends the single
    // DenyReason taxonomy (no second error type).
    if let Err(reason) = sink_schema::validate_schema(plan_node) {
        return ExecutorDecision::Denied { reason };
    }

    // Step 0.25 (POLICY-01/POLICY-02, DESIGN-v1.9-egress-policy §5.1/§5.2): the
    // pre-I2 narrowing gate. Runs AFTER the Step-0 schema gate (so an unknown
    // sink still Denies with `UnknownSink`, never `PolicyDeny`) and BEFORE the
    // collect-then-Block I2 loop below. This is LOAD-BEARING placement +
    // direction: the gate is DENY-ONLY (it returns only `Err(PolicyDeny)`; a
    // PERMIT is `Ok(())`, which falls THROUGH to the UNMODIFIED I2 loop). There
    // is NO Allow-and-skip-I2 path — a policy PERMIT never short-circuits the I2
    // loop or the Step-0.5 CommitIrreversible class gate, so no policy, however
    // permissive, can weaken an I2 Block (POLICY-02, LOCKED; T-42-07). Policy is
    // ADDITIVE and deny-only; the I2 sensitivity map + loop stay HARDCODED and
    // untouched (DESIGN §5.2, §7).
    if let Err(reason) = policy_gate::policy_gate(policy, plan_node, value_store) {
        return ExecutorDecision::Denied { reason };
    }

    // Collect-then-Block (Phase 14, D-14): accumulate EVERY sensitive+tainted arg
    // across the whole plan node before returning any Block decision. Never
    // return from inside this loop on the first match — a plan node with both a
    // tainted `to` (routing-sensitive) and a tainted `body` (content-sensitive)
    // MUST surface both, not just the first one scanned (closes the
    // B1-reincarnation risk, `DESIGN-content-adapter-mediation.md` "Precedence").
    let mut blocked: Vec<BlockedArg> = Vec::new();

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

        // Step 1a: Empty-taint guard. Runs BEFORE the sensitivity check so
        // an all-trusted-looking record with empty taint cannot fail open (an empty
        // taint iterator is never untrusted → would slip past the block). Defense in
        // depth: mint already rejects empty taint, but the executor must not depend
        // on that alone (DESIGN §3 ordering: resolve → empty-taint → empty-provenance
        // → sensitivity check).
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

        // Step 1c: role check (NEW, T2, DESIGN-slot-type-binding.md §6/§7).
        // A per-arg fail-closed structural guard — same tier as 1/1a/1b, fires
        // BEFORE this arg is considered for sensitivity collection, and RETURNS
        // immediately on a mismatch (never joins the Steps 2/3 collect-then-Block
        // `blocked` vec). A role mismatch is a structural type error, not a
        // confirmable judgment call — it Denies, never Blocks.
        //
        // expected_role returns Option<&[&str]>, matched explicitly here:
        //   None            => this slot is unconstrained (e.g. email.send's
        //                       `attachment`) — a documented, intentional
        //                       scope-out (DESIGN §7 item 3), NOT fail-open.
        //                       (file.create's `contents` became role-checked
        //                       in HARDEN-05; it is no longer a None example.)
        //                       Fall through unchanged.
        //   Some(list)      => this slot IS role-checked. The value passes iff
        //                       `record.origin_role` is `Some(s)` AND `list`
        //                       contains `s`. A `None` role at a role-checked
        //                       slot fails closed (DESIGN §7 item 1), exactly
        //                       like a role not present in `list` (item 2).
        // Never collapse the None/Some(&[]) states via an unwrap-with-empty-
        // default here — that would break the fail-closed contract (DESIGN
        // §7, Pitfall 2).
        if let Some(expected) = sink_sensitivity::expected_role(&plan_node.sink, &arg.name) {
            let role_ok = match record.origin_role.as_deref() {
                Some(role) => expected.contains(&role),
                None => false,
            };
            if !role_ok {
                return ExecutorDecision::Denied {
                    reason: DenyReason::SlotTypeMismatch {
                        sink: plan_node.sink.0.clone(),
                        arg: arg.name.clone(),
                        expected: expected.iter().map(|s| s.to_string()).collect(),
                        found: record.origin_role.clone(),
                    },
                };
            }
        }

        // Step 2/3 (unified, Phase 14 D-14): sensitivity check. If this arg either
        // routes the effect (e.g., email.send "to") OR carries content-sensitive
        // payload (e.g., email.send "body"/"subject") AND the resolved record
        // carries any UNTRUSTED label, collect it as a blocked arg.
        // UserTrusted/LocalWorkspace-only provenance does NOT block (HARD-02).
        // The payload is copied verbatim from the broker-owned record — NOT synthesized.
        let sensitive = sink_sensitivity::is_routing_sensitive(&plan_node.sink, &arg.name)
            || sink_sensitivity::is_content_sensitive(&plan_node.sink, &arg.name);
        if sensitive && record.taint.iter().any(|t| t.is_untrusted()) {
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
            blocked.push(BlockedArg {
                anchor,
                literal: record.literal.clone(),
            });
        }
    }

    // Only after the per-arg loop has fully scanned every arg: if anything was
    // collected, return ONE combined Block covering the whole set (D-14/D-17).
    if !blocked.is_empty() {
        return ExecutorDecision::BlockedPendingConfirmation { anchors: blocked };
    }

    // Step 0.5 (DESIGN-session-trust-state.md §8, TAINT-02/03): the draft-only
    // CommitIrreversible class deny. Runs ONLY here — after the per-arg loop
    // (Steps 1/1a/1b/2) has completed with NO Block collected — and BEFORE the
    // final `Allowed` return. This placement is load-bearing (round-1 blocker B1,
    // and its Phase-14 D-15 restatement): the per-arg I2 Block always takes
    // precedence over this I1/I0 class-level deny. If any arg Blocked above,
    // this point is never reached.
    //
    // Exhaustive match over all six `SessionStatus` variants, no wildcard arm
    // (§10) — a future variant is a compile error here, not a silent fail-open.
    match *session_status {
        SessionStatus::Draft => {
            if sink_sensitivity::sink_effect_class(&plan_node.sink) == EffectClass::CommitIrreversible {
                return ExecutorDecision::Denied {
                    reason: DenyReason::DraftOnlySessionDeniesCommitIrreversible {
                        sink: plan_node.sink.clone(),
                    },
                };
            }
            // Draft + non-CommitIrreversible (Observe/MutateReversible): fall
            // through to Allowed (TAINT-03).
        }
        SessionStatus::Active => {
            // No deny from this gate; fall through to Allowed.
        }
        SessionStatus::WaitingApproval
        | SessionStatus::Done
        | SessionStatus::Failed
        | SessionStatus::RolledBack => {
            // Phase 16 (BLOCKER-1 guard b, DESIGN-session-trust-state.md): these
            // lifecycle states are terminal or paused — a CommitIrreversible
            // sink must NEVER fall through to Allowed here. A non-terminal
            // Allowed-dispatch (e.g. a naively-added email.send branch) could
            // otherwise be reached via a stale/replayed submission against a
            // session that has already moved past Active. Non-CommitIrreversible
            // sinks (Observe/MutateReversible) are unaffected — this gate is
            // scoped to the CommitIrreversible class only, same as the Draft arm.
            if sink_sensitivity::sink_effect_class(&plan_node.sink) == EffectClass::CommitIrreversible {
                return ExecutorDecision::Denied {
                    reason: DenyReason::NonLiveSessionDeniesCommitIrreversible {
                        sink: plan_node.sink.clone(),
                    },
                };
            }
            // Non-CommitIrreversible sinks in these lifecycle states: no deny
            // from THIS gate; fall through to Allowed.
        }
    }

    ExecutorDecision::Allowed
}
