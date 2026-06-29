/// brokerd — broker effect API
///
/// The only authorised entry point for effects is `submit_plan_node`.
/// There is no raw effect-to-sink path anywhere in this crate.
/// All arguments are carried through PlanNode/ValueNode from runtime-core
/// (DEC-architectural-lock-plan-nodes).

/// Submit a plan node for execution.
///
/// Phase 1 stub: always returns `ExecutorDecision::NotImplemented`.
/// Future phases (Phase 4) will evaluate taint labels in ValueNode arguments
/// and return Allowed / BlockedPendingConfirmation / Denied.
///
/// # No raw effect-to-sink path
/// brokerd in Phase 1 contains ONLY this function. There is no convenience
/// executor helper and no type representing a raw effect-to-sink path.
pub fn submit_plan_node(
    _session_id: uuid::Uuid,
    _plan: runtime_core::PlanNode,
) -> runtime_core::ExecutorDecision {
    // RED: intentionally wrong — test will fail asserting NotImplemented
    runtime_core::ExecutorDecision::Allowed
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;
    use runtime_core::{ExecutorDecision, PlanNode, SinkId};

    #[test]
    fn submit_plan_node_returns_not_implemented() {
        let session_id = Uuid::new_v4();
        let plan = PlanNode {
            sink: SinkId("test.sink".into()),
            args: vec![],
        };
        let result = submit_plan_node(session_id, plan);
        assert_eq!(result, ExecutorDecision::NotImplemented);
    }
}
