/// value_record.rs — ValueRecord, the broker-owned resolution of a ValueId.
///
/// The handle model (DESIGN-plan-executor §ValueRecord & ValueId Handle Model)
/// splits authority: the planner holds only an opaque `ValueId` (via `PlanArg`),
/// while the broker owns the `ValueRecord` that carries the literal, the taint
/// labels, and the provenance_chain. The planner NEVER constructs a `ValueRecord`
/// — only the broker mints one when a worker reads or derives a value. This is
/// what makes taint-stripping structurally impossible at the planner boundary
/// (T-04-02): there is no taint field on the planner side to strip.

use crate::plan_node::{TaintLabel, ValueId};

/// A broker-owned value: literal + taint + ordered provenance chain, keyed by an
/// opaque `ValueId`.
///
/// `provenance_chain[0]` MUST equal the originating file_read Event id — this is
/// the field the §9 held-out test asserts against to prove the taint chain is
/// genuinely propagated (not stapled at the sink). Broker-owned: the planner
/// never constructs this type.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValueRecord {
    /// The opaque handle the planner references this value by.
    pub id: ValueId,
    /// The concrete literal value this handle resolves to.
    pub literal: String,
    /// Taint labels accumulated through the derivation DAG.
    pub taint: Vec<TaintLabel>,
    /// Ordered derivation edges from the originating read Event;
    /// `provenance_chain[0]` MUST equal the file_read Event id.
    pub provenance_chain: Vec<uuid::Uuid>,
    /// Semantic origin-role tag (T2, DESIGN-slot-type-binding.md §1) — additive,
    /// orthogonal to `taint`. `None` is a state distinct from every valid role
    /// tag; the Wave-2 fail-closed default keys off exactly that bit.
    /// `#[serde(default)]` (DESIGN F6) so a record serialized before this field
    /// existed still deserializes as `None`.
    #[serde(default)]
    pub origin_role: Option<String>,
}
