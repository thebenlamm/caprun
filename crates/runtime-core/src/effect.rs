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
    // WG-4 (Phase 44, DESIGN-v1.9-egress-policy §1.3): the second field is
    // `refspec`, NOT `branch`. The `git.push` sink's args are `{remote, refspec}`
    // (crates/executor/src/sink_schema.rs) and DESIGN §1.3 captures a `refspec`
    // from TRUSTED intent — naming this field `branch` would leave the effect
    // ontology keyed on a divergent name a refactor could desync from the sink
    // schema + sensitivity tables (T-44-04). One identical `refspec` name across
    // all three surfaces.
    GitPush { remote: String, refspec: String },
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
