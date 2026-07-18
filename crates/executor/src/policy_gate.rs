//! policy_gate.rs — the deny-only, pre-I2 narrowing gate (POLICY-01/POLICY-02).
//!
//! DESIGN-v1.9-egress-policy §5.1/§5.2. This is the executor's policy-evaluation
//! surface: it evaluates ONE `PlanNode` against the session's `SessionPolicy` and
//! returns `Result<(), DenyReason>` — `Ok(())` = PERMIT, `Err(PolicyDeny{..})` =
//! DENY.
//!
//! # The load-bearing structural pin (POLICY-02, LOCKED)
//!
//! This function has NO `Allowed` — or any permit-CARRYING — return. Its ONLY
//! non-error result is `Ok(())`, which means "policy does not object; hand the
//! call to the UNMODIFIED I2 collect-then-Block loop." Because policy evaluation
//! can only ADD a `Denied{PolicyDeny}` BEFORE I2 runs and can NEVER produce a
//! value that skips I2, no policy — however permissive — can weaken an I2 Block
//! (§5.2 LOCKED, T-42-07). `submit_plan_node` invokes this gate AFTER the Step-0
//! schema gate and BEFORE the collect-then-Block loop; a PERMIT falls through, so
//! I2 always runs on every policy-permitted call. There is deliberately no
//! `policy` branch anywhere that bypasses or short-circuits the I2 loop or the
//! Step-0.5 `CommitIrreversible` class gate.
//!
//! # Attribution (POLICY-01)
//!
//! A policy refusal is a `DenyReason::PolicyDeny` (`code()=="policy_deny"`) —
//! DISTINCT from an I2 `BlockedPendingConfirmation`, so a policy-deny ("this call
//! was never permitted") and an I2 Block ("this permitted call carried a tainted
//! value into a sensitive arg") stay independently attributable (§5.1, §6 row 12,
//! LIVE-06 leg 3).

use runtime_core::{plan_node::PlanNode, DenyReason, PolicyDenyKind, SessionPolicy};

use crate::value_store::ValueStore;

/// Evaluate `plan_node` against `policy` as a deny-only pre-I2 gate.
///
/// Returns `Ok(())` when the policy permits the call (the executor then runs the
/// UNMODIFIED I2 loop) or `Err(DenyReason::PolicyDeny{..})` when the policy
/// refuses the sink or a constrained arg literal.
///
/// Deny order (DESIGN §5.1):
///   1. Sink deny-by-default: a sink absent from the policy allowlist →
///      `PolicyDeny{ arg: None, constraint: "sink-not-allowed" }`.
///   2. Per-arg coarse allowlist: a constrained arg whose resolved literal
///      matches no allowlist prefix → `PolicyDeny{ arg: Some(name), constraint }`.
///
/// A dangling/forged handle is NOT this gate's concern — it is left for the I2
/// loop's Step 1 `DanglingHandle` deny so attribution stays with I2. An
/// unresolvable arg is therefore SKIPPED here, never converted into a
/// policy-deny.
///
/// The return type is `Result<(), DenyReason>` with NO `Ok`-carried decision —
/// the structural guarantee that a permit can only fall through to the
/// unmodified I2 loop, never skip it (POLICY-02).
pub fn policy_gate(
    policy: &SessionPolicy,
    plan_node: &PlanNode,
    value_store: &ValueStore,
) -> Result<(), DenyReason> {
    // Axis 1 — the deny-by-default sink gate (DESIGN §5.1). A sink not in the
    // policy allowlist is refused sink-scoped (`arg: None`) before any arg is
    // examined.
    if !policy.permits_sink(&plan_node.sink) {
        return Err(DenyReason::PolicyDeny {
            sink: plan_node.sink.0.clone(),
            arg: None,
            constraint: PolicyDenyKind::SinkNotAllowed.constraint_tag().to_string(),
        });
    }

    // Axis 2 — coarse per-arg allowlists. `policy.evaluate` returns `Ok(())` for
    // an unconstrained arg on an allowed sink, so evaluating EVERY arg is
    // correct: only a CONFIGURED arg constraint can produce a deny here.
    for arg in &plan_node.args {
        // Resolve the literal from the trusted broker-owned store. A `None`
        // resolution is deliberately SKIPPED — the I2 loop owns the
        // `DanglingHandle` attribution; this gate never turns a dangling handle
        // into a `PolicyDeny`.
        let literal = match value_store.resolve(&arg.value_id) {
            Some(record) => &record.literal,
            None => continue,
        };
        if let Err(kind) = policy.evaluate(&plan_node.sink, &arg.name, literal) {
            return Err(DenyReason::PolicyDeny {
                sink: plan_node.sink.0.clone(),
                arg: Some(arg.name.clone()),
                constraint: kind.constraint_tag().to_string(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_core::plan_node::{PlanArg, SinkId, TaintLabel, ValueId};
    use runtime_core::PlanNode;

    fn node(sink: &str, args: Vec<PlanArg>) -> PlanNode {
        PlanNode {
            sink: SinkId(sink.to_string()),
            args,
        }
    }

    /// A sink absent from the policy allowlist is refused sink-scoped
    /// (`arg: None`, `constraint == "sink-not-allowed"`) before any arg work.
    #[test]
    fn sink_not_in_allowlist_denies_sink_scoped() {
        let policy = SessionPolicy::default_fail_closed();
        let store = ValueStore::default();
        let plan = node("email.send", vec![]);
        let result = policy_gate(&policy, &plan, &store);
        assert_eq!(
            result,
            Err(DenyReason::PolicyDeny {
                sink: "email.send".to_string(),
                arg: None,
                constraint: "sink-not-allowed".to_string(),
            }),
            "a sink not in the allowlist must PolicyDeny sink-scoped"
        );
    }

    /// A permitted sink with an unconstrained, resolvable arg falls through with
    /// `Ok(())` — the gate adds NO behavior on a permit (POLICY-02 fall-through).
    #[test]
    fn permitted_sink_unconstrained_arg_falls_through() {
        let policy = SessionPolicy::allow_all();
        let mut store = ValueStore::default();
        let id = store
            .mint(
                "user@example.com".to_string(),
                vec![TaintLabel::UserTrusted],
                vec![uuid::Uuid::new_v4()],
                Some("recipient".to_string()),
            )
            .expect("mint");
        let plan = node(
            "email.send",
            vec![PlanArg {
                name: "to".to_string(),
                value_id: id,
            }],
        );
        assert_eq!(
            policy_gate(&policy, &plan, &store),
            Ok(()),
            "a permitted sink + unconstrained arg must fall through to I2"
        );
    }

    /// An unresolvable (dangling) handle is SKIPPED by the gate — it is the I2
    /// loop's `DanglingHandle` deny to raise, never a `PolicyDeny`.
    #[test]
    fn dangling_handle_is_not_a_policy_deny() {
        let policy = SessionPolicy::allow_all();
        let store = ValueStore::default();
        let plan = node(
            "email.send",
            vec![PlanArg {
                name: "to".to_string(),
                value_id: ValueId::new(), // never minted → resolves to None
            }],
        );
        assert_eq!(
            policy_gate(&policy, &plan, &store),
            Ok(()),
            "a dangling handle must be skipped (I2 owns DanglingHandle), not PolicyDenied"
        );
    }
}
