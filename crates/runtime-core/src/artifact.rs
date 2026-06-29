/// artifact.rs — Artifact and ArtifactRef
///
/// An Artifact is a named, hash-identified immutable object produced or consumed
/// during a Session. ArtifactRef is a lightweight reference (id + name) suitable
/// for embedding in other domain types without carrying the full content hash.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// An immutable artifact produced or consumed during a Session.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Artifact {
    pub id: Uuid,
    pub name: String,
    /// MIME type or logical type (e.g. "text/plain", "patch", "test-report").
    pub artifact_type: String,
    /// SHA-256 content hash in "sha256:<hex>" format.
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
}

/// A lightweight reference to an Artifact (id + name, no content hash).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArtifactRef {
    pub id: Uuid,
    pub name: String,
}
