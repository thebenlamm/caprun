// runtime-core: pure domain types — no I/O, no async, no network
// All external effects are mediated through PlanNode/ValueNode (DEC-architectural-lock-plan-nodes)

pub mod effect;
pub mod executor_decision;
pub mod plan_node;

// Re-export domain types for downstream crates
pub use effect::{Effect, IrreversibleEffect, ObserveEffect, ReversibleEffect};
pub use executor_decision::ExecutorDecision;
pub use plan_node::{PlanNode, Provenance, SinkId, TaintLabel, ValueNode};
