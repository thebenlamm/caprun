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
pub mod sinks;
pub mod approval;

/// Submit a plan node for I2 evaluation against the broker-owned value store.
///
/// Delegates to `executor::submit_plan_node` — the deterministic, non-LLM I2
/// enforcer in the Rust TCB. No raw effect-to-sink path exists here or anywhere
/// in brokerd (DEC-architectural-lock-plan-nodes, CON-broker-api-shape).
///
/// # No raw effect-to-sink path
/// This function is the sole public effect entry point for brokerd. It takes a
/// PlanNode (opaque-handle args only) and returns an `ExecutorDecision`. There is
/// no path from a raw `EffectRequest` or literal value directly to a sink.
pub fn submit_plan_node(
    session_id: uuid::Uuid,
    plan: runtime_core::PlanNode,
    store: &executor::value_store::ValueStore,
) -> runtime_core::ExecutorDecision {
    executor::submit_plan_node(session_id, &plan, store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use executor::value_store::ValueStore;
    use runtime_core::{ExecutorDecision, PlanNode, SinkId};
    use uuid::Uuid;

    /// Delegated submit_plan_node returns Allowed for a PlanNode with no args
    /// (nothing to check → no tainted handle → Allowed).
    #[test]
    fn submit_plan_node_empty_args_returns_allowed() {
        let session_id = Uuid::new_v4();
        let plan = PlanNode {
            sink: SinkId("test.sink".into()),
            args: vec![],
        };
        let store = ValueStore::default();
        let result = submit_plan_node(session_id, plan, &store);
        assert_eq!(result, ExecutorDecision::Allowed);
    }
}
