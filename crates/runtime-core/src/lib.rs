// runtime-core: pure domain types — no I/O, no async, no network
// All external effects are mediated through PlanNode/PlanArg (DEC-architectural-lock-plan-nodes).
// The planner holds only opaque ValueId handles; the broker owns ValueRecord (literal+taint+chain).

pub mod artifact;
pub mod effect;
pub mod event;
pub mod executor_decision;
pub mod intent;
pub mod plan_node;
pub mod session;
pub mod value_record;

// Re-export all public domain types so downstream crates import from runtime-core,
// never from submodules directly.
pub use artifact::{Artifact, ArtifactRef};
pub use effect::{Effect, IrreversibleEffect, ObserveEffect, ReversibleEffect};
pub use event::Event;
pub use executor_decision::ExecutorDecision;
pub use intent::{CaprunIntent, Intent, IntentStatus};
pub use plan_node::{PlanArg, PlanNode, Provenance, SinkId, TaintLabel, ValueId, ValueNode};
pub use session::{Session, SessionStatus};
pub use value_record::ValueRecord;
