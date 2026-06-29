/// plan_node.rs — PlanNode, ValueNode, SinkId, TaintLabel, Provenance
///
/// DEC-architectural-lock-plan-nodes: The broker effect path takes plan nodes
/// from day one. ValueNode carries literal + provenance + taint so the executor
/// can enforce I2 (value-injection defense). All three fields are present from
/// this first commit — removing taint would be a breaking change.

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

/// Where a value came from in the audit DAG.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Provenance {
    pub source_event_id: Option<uuid::Uuid>,
    pub source_artifact_id: Option<uuid::Uuid>,
    pub description: String,
}

/// A concrete value with its provenance and accumulated taint labels.
///
/// All three fields — literal, provenance, taint — are required from the first
/// commit. This is the architectural lock (DEC-architectural-lock-plan-nodes +
/// Success Criterion 3). Removing taint would defeat Phase 4 I2 enforcement.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ValueNode {
    /// The concrete literal value (string, number, bool, object, etc.)
    pub literal: serde_json::Value,
    /// Where this value came from
    pub provenance: Provenance,
    /// Taint labels accumulated on this value through the DAG
    pub taint: Vec<TaintLabel>,
}

/// Opaque identifier for a sink (e.g. "email.send", "git.push").
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SinkId(pub String);

/// A plan node: a sink to call with typed, provenanced, tainted arguments.
///
/// This is the only authorized effect path. Raw EffectRequest→sink is forbidden.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanNode {
    pub sink: SinkId,
    pub args: Vec<ValueNode>,
}
