/// effect.rs — The 3-class Effect enum
///
/// Effect has exactly three top-level variants (CON-effect-classes, DEC-layer-roles).
/// Grow the ontology by adding variants to sub-enums, never by adding new top-level classes.

/// Observe-class effects: read-only, no mutation.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ObserveEffect {
    ReadWorkspaceFile { path: String },
    ListWorkspace { path: String },
    RunTests { command: String },
    SummarizeArtifact { artifact_id: uuid::Uuid },
}

/// Reversible mutation effects: can be undone (e.g. workspace writes, patches).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ReversibleEffect {
    WriteArtifact { name: String, content_hash: String },
    ApplyPatch { patch_hash: String },
    EditWorkspaceFile { path: String, patch_hash: String },
}

/// Irreversible/external effects: cannot be undone (e.g. sends, pushes, deploys).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum IrreversibleEffect {
    SendEmail { draft_hash: String, to: Vec<String> },
    GitPush { remote: String, branch: String },
    DeployService { service: String, environment: String },
}

/// The 3-class Effect enum at the planner surface.
///
/// Exactly three variants — Observe, MutateReversible, CommitIrreversible.
/// This shape is locked (CON-effect-classes). Do not add a fourth top-level variant.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Effect {
    Observe(ObserveEffect),
    MutateReversible(ReversibleEffect),
    CommitIrreversible(IrreversibleEffect),
}
