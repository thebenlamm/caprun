//! llm-planner — the literal-free wire contract between the worker-side
//! `LlmPlanner` proxy and the out-of-process LLM sidecar (PLANNER-03/04).
//!
//! This crate is pure: no network code, no `reqwest`, no `tokio`. The
//! sidecar (Phase 21 Plan 02) and the worker-side proxy (Phase 21 Plan 03)
//! both depend on it and exchange ONLY the types defined here — none of
//! which carries a resolved literal. The LLM can reference a value only by
//! its opaque `ValueId` handle; the literal itself never crosses this wire.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_red() {
        // RED: PlannerRequest/HandleLabel/PlannerResponse/ResponseArg do not
        // exist yet — this module will not compile until Task 1's action
        // step adds them.
        let _req: PlannerRequest = todo!();
        let _label: HandleLabel = todo!();
        let _resp: PlannerResponse = todo!();
        let _arg: ResponseArg = todo!();
    }
}
