/// brokerd — broker effect API and confinement-mediation substrate
///
/// Phase 1 contract: `submit_plan_node` is the only authorised entry point
/// for effects. There is no raw effect-to-sink path anywhere in this crate.
/// All arguments are carried through PlanNode/ValueNode from runtime-core
/// (DEC-architectural-lock-plan-nodes).
///
/// Phase 3 additions (Wave 0 stubs — Wave 2/3 implement):
///   - `proto`   — IPC message types (BrokerRequest, BrokerResponse)
///   - `server`  — tokio async UDS IPC server
///   - `session` — Session lifecycle (create, persist)
///   - `audit`   — SQLite hash-linked audit DAG

pub mod proto;
pub mod quarantine;
pub mod server;
pub mod session;
pub mod audit;
pub mod policy;
pub mod provenance_proof;
pub mod confirmation;
pub mod sinks;
pub mod approval;

/// Reflects whether THIS build graph actually compiled `brokerd` with the
/// `test-fixtures` feature enabled (true under `cargo test --workspace`
/// feature-unification; false under a scoped `cargo test -p caprun`).
/// D-10 tests (v1.6 HARDEN-04) key their skip/hard-fail branch on this
/// const rather than on the response variant itself, so a genuine
/// featureless-graph regression cannot be silently downgraded to a skip.
pub const TEST_FIXTURES_ACTIVE: bool = cfg!(feature = "test-fixtures");

/// Submit a plan node for I2 evaluation against the broker-owned value store.
///
/// Delegates to `executor::submit_plan_node` — the deterministic, non-LLM I2
/// enforcer in the Rust TCB. No raw effect-to-sink path exists here or anywhere
/// in brokerd (DEC-architectural-lock-plan-nodes, CON-broker-api-shape).
///
/// # No raw effect-to-sink path
/// This function is the sole public effect entry point for brokerd. It takes a
/// PlanNode (opaque-handle args only) and returns an `ExecutorDecision`. There is
/// no path from a raw `EffectRequest` or literal value directly to a sink. (planner-discipline-allow)
///
/// `session_status` is forwarded to `executor::submit_plan_node` unchanged
/// (RESEARCH Open Question 1: extend rather than delete this wrapper). Callers
/// preserving today's behavior pass `&SessionStatus::Active`.
///
/// `policy` (v1.9 Phase 42, POLICY-01/POLICY-02) is likewise forwarded unchanged
/// to `executor::submit_plan_node`, where the deny-only pre-I2 narrowing gate
/// evaluates it BEFORE the collect-then-Block I2 loop. Callers preserving today's
/// behavior pass `&SessionPolicy::allow_all()` (or `broker_default()`); the live
/// dispatch path constructs the session policy in `run_broker_server`.
pub fn submit_plan_node(
    session_id: uuid::Uuid,
    plan: runtime_core::PlanNode,
    store: &executor::value_store::ValueStore,
    session_status: &runtime_core::SessionStatus,
    policy: &runtime_core::SessionPolicy,
) -> runtime_core::ExecutorDecision {
    // The broker mints the effect identity (HARD-06, DESIGN §4 rule 2) — the
    // executor never mints a Uuid. The live dispatch path (server.rs) does the
    // same before appending the durable sink_blocked anchor.
    let effect_id = uuid::Uuid::new_v4();
    executor::submit_plan_node(session_id, effect_id, &plan, store, session_status, policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use executor::value_store::ValueStore;
    use runtime_core::{ExecutorDecision, PlanNode, SinkId};
    use uuid::Uuid;

    /// Delegated submit_plan_node returns Allowed for a registered sink with no
    /// args (nothing to check → no tainted handle → Allowed). Uses `email.send`
    /// (a KNOWN sink with no required args) — 07-04a's arg-schema gate now fails
    /// an UNKNOWN sink closed, so the delegation smoke test must target a
    /// registered sink to exercise the Allowed path.
    #[test]
    fn submit_plan_node_empty_args_returns_allowed() {
        use runtime_core::{SessionPolicy, SessionStatus};
        let session_id = Uuid::new_v4();
        let plan = PlanNode {
            sink: SinkId("email.send".into()),
            args: vec![],
        };
        let store = ValueStore::default();
        let result = submit_plan_node(
            session_id,
            plan,
            &store,
            &SessionStatus::Active,
            &SessionPolicy::allow_all(),
        );
        assert_eq!(result, ExecutorDecision::Allowed);
    }

    /// The arg-schema gate fails an UNKNOWN sink closed (07-04a, HARD-01/HARD-05):
    /// a sink absent from `KNOWN_SINKS` is denied BEFORE any resolve/sensitivity.
    #[test]
    fn submit_plan_node_unknown_sink_denied() {
        use runtime_core::{DenyReason, SessionPolicy, SessionStatus};
        let session_id = Uuid::new_v4();
        let plan = PlanNode {
            sink: SinkId("test.sink".into()),
            args: vec![],
        };
        let store = ValueStore::default();
        // The Step-0 schema gate fires BEFORE the policy gate, so an unknown
        // sink still Denies with UnknownSink (never PolicyDeny) even under the
        // permissive allow_all() policy.
        let result = submit_plan_node(
            session_id,
            plan,
            &store,
            &SessionStatus::Active,
            &SessionPolicy::allow_all(),
        );
        assert_eq!(
            result,
            ExecutorDecision::Denied {
                reason: DenyReason::UnknownSink("test.sink".to_string())
            }
        );
    }
}
