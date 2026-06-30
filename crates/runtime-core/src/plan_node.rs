/// plan_node.rs — PlanNode, PlanArg, ValueId, ValueNode, SinkId, TaintLabel, Provenance
///
/// DEC-architectural-lock-plan-nodes: The broker effect path takes plan nodes
/// from day one. The handle model (DESIGN-plan-executor §ValueRecord & ValueId
/// Handle Model) is the spine of I2 soundness: the planner references values by
/// opaque `ValueId` (via `PlanArg`) and NEVER authors literal or taint. The
/// broker-owned `ValueRecord` (in value_record.rs) carries literal + taint +
/// provenance_chain. This closes taint-stripping (T-04-02): an injected planner
/// holds only an opaque handle and has nothing to strip.

/// Labels indicating the trust/taint level of a value's provenance chain.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum TaintLabel {
    UserTrusted,
    LocalWorkspace,
    ExternalUntrusted,
    EmailRaw,
    PdfRaw,
    LlmGenerated,
    WorkerExtracted,
}

/// Opaque handle for a value. The planner holds ONLY this — never the literal
/// or taint. The broker resolves a `ValueId` to a `ValueRecord` it owns.
///
/// Deriving `Hash` + `Eq` lets a `ValueId` key a `HashMap` in the broker's
/// value store. `ValueId::new()` mints a fresh v4 UUID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ValueId(pub uuid::Uuid);

impl ValueId {
    /// Mint a fresh opaque value handle backed by a random v4 UUID.
    pub fn new() -> Self {
        ValueId(uuid::Uuid::new_v4())
    }
}

impl Default for ValueId {
    fn default() -> Self {
        Self::new()
    }
}

/// Where a value came from in the audit DAG.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Provenance {
    pub source_event_id: Option<uuid::Uuid>,
    pub source_artifact_id: Option<uuid::Uuid>,
    pub description: String,
    /// Ordered derivation edges from the originating read Event. For the v0
    /// linear DAG this has length 1 — `provenance_chain[0]` is the file_read
    /// Event id the §9 held-out test asserts against.
    pub provenance_chain: Vec<uuid::Uuid>,
}

/// A concrete value with its provenance and accumulated taint labels.
///
/// LEGACY value type. This is NOT the sink-argument path: the sink-argument path
/// is `PlanArg` resolving to a broker-owned `ValueRecord`. `ValueNode` is retained
/// for its existing serde contract and is no longer routed into `PlanNode.args`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValueNode {
    /// The concrete literal value (string, number, bool, object, etc.)
    pub literal: serde_json::Value,
    /// Where this value came from
    pub provenance: Provenance,
    /// Taint labels accumulated on this value through the DAG
    pub taint: Vec<TaintLabel>,
}

/// A planner-facing sink argument. Binds a name to an opaque `ValueId`.
///
/// Carries NO literal and NO taint field — that is the whole point of the handle
/// model (T-04-02 mitigation). An injected planner that fabricates a `PlanArg`
/// still cannot strip taint or forge a literal: those live in the broker-owned
/// `ValueRecord` keyed by `value_id`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanArg {
    pub name: String,
    pub value_id: ValueId,
}

/// Opaque identifier for a sink (e.g. "email.send", "git.push").
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SinkId(pub String);

/// A plan node: a sink to call with opaque-handle arguments.
///
/// This is the only authorized effect path. No raw effect-to-sink path may exist.
/// `args` are `PlanArg` handles — the planner never carries literals or taint here.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanNode {
    pub sink: SinkId,
    pub args: Vec<PlanArg>,
}
