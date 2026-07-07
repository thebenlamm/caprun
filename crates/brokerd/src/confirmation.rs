/// confirmation.rs — the durable checkpoint substrate for the single-shot
/// confirmation loop.
///
/// `caprun confirm`/`caprun deny` are ALWAYS separate, later OS processes from the
/// one that created the block (DESIGN-confirmation-release.md "The Problem Being
/// Solved"). The in-memory `executor::ValueStore` that resolved the original
/// `PlanNode`'s `ValueId` handles is gone by the time a confirm/deny process runs,
/// so the full resolved-arg payload MUST be persisted at Block time and read back
/// from durable storage — never reconstructed or cached in memory.
///
/// This module owns three public record types (`PendingConfirmation`,
/// `ResolvedArg`, `PendingConfirmationState`) and three side-table accessors
/// (`insert_pending_confirmation`, `find_pending_confirmation`, `transition_state`)
/// over the `pending_confirmations` table added to `SCHEMA_DDL` in `audit.rs`. It
/// mirrors the exact accessor shape of `insert_blocked_literal`/
/// `get_blocked_literal` in that file.
///
/// No block-time wiring, no confirm/deny decision logic, and no CLI live here —
/// only the persisted-state layer everything later builds on.
use anyhow::Result;
use rusqlite::params;

/// One resolved sink argument, frozen at Block time.
///
/// Mirrors the design doc's illustrative `ResolvedArg` struct: one per original
/// `PlanArg`, carrying its dereferenced `ValueRecord`'s literal/taint/provenance
/// chain as they stood at the moment of the Block — never re-resolved later,
/// since the `ValueStore` that could re-resolve a `ValueId` does not survive
/// process exit.
///
/// This whole record serializes to/from the `resolved_args` JSON blob column; it
/// is a brokerd-internal record, NOT part of the hashed Event/anchor chain, so it
/// is safe to extend beyond the design doc's illustrative struct.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ResolvedArg {
    /// Matches the original `PlanArg.name`.
    pub name: String,
    /// The original `PlanArg.value_id`, kept for audit traceability.
    pub value_id: runtime_core::plan_node::ValueId,
    /// The dereferenced `ValueRecord`'s literal, frozen at Block time.
    pub literal: String,
    /// The dereferenced `ValueRecord`'s taint set, frozen at Block time.
    pub taint: Vec<runtime_core::plan_node::TaintLabel>,
    /// The dereferenced `ValueRecord`'s provenance chain, frozen at Block time.
    pub provenance_chain: Vec<uuid::Uuid>,
}

/// The one-way state machine for a pending confirmation.
///
/// `Pending -> Confirmed` or `Pending -> Denied`, exactly once. Never
/// `Confirmed -> Denied`, never `Denied -> Confirmed`, never re-entry into
/// `Pending`. The terminal check is enforced in SQL by `transition_state`'s
/// `AND state = 'pending'` guard (CONFIRM-03) — not by any in-memory check, since
/// the process making the transition is never the same OS process that created
/// the row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PendingConfirmationState {
    Pending,
    Confirmed,
    Denied,
}

impl PendingConfirmationState {
    /// Stable lowercase string for the `state` TEXT column.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            PendingConfirmationState::Pending => "pending",
            PendingConfirmationState::Confirmed => "confirmed",
            PendingConfirmationState::Denied => "denied",
        }
    }

    /// Parse a persisted `state` column value. Fails closed on any unrecognized
    /// string — an unknown persisted state is a hard error, never a silent
    /// default, since silently defaulting could resurrect a terminal row as
    /// `Pending`.
    pub(crate) fn from_str(s: &str) -> Result<Self> {
        match s {
            "pending" => Ok(PendingConfirmationState::Pending),
            "confirmed" => Ok(PendingConfirmationState::Confirmed),
            "denied" => Ok(PendingConfirmationState::Denied),
            other => Err(anyhow::anyhow!(
                "unknown PendingConfirmationState value: {other}"
            )),
        }
    }
}

/// The durable checkpoint for a blocked sink call, persisted so a later, separate
/// `caprun confirm`/`caprun deny` process can resume it.
///
/// A superset of `SinkBlockedAnchor` (runtime_core::executor_decision), never an
/// extension of it: the anchor rides inside the hashed `sink_blocked` Event
/// payload and has its own tamper-evidence contract. This record is a sibling,
/// persisted alongside (never inside) the anchor, in the `pending_confirmations`
/// side table.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PendingConfirmation {
    /// The SAME identifier as `SinkBlockedAnchor.effect_id` (CONFIRM-04's anchor
    /// key). Broker-minted, never client- or worker-supplied.
    pub effect_id: uuid::Uuid,
    /// The Session the blocked plan node belonged to.
    pub session_id: uuid::Uuid,
    /// The id of the anchoring `sink_blocked` Event. Plumbing the design doc's
    /// illustrative struct omits: needed so confirm/deny can set `parent_id` and
    /// run the blocked-literals redaction gate. A side-table addition, never an
    /// `events`-table column.
    pub blocked_event_id: uuid::Uuid,
    /// The blocked plan node's `SinkId`, copied from the original `PlanNode.sink`
    /// at Block time.
    pub sink: runtime_core::plan_node::SinkId,
    /// The FULL resolved arg set for the blocked sink call — one `ResolvedArg` per
    /// original `PlanArg`, not merely the one arg that triggered the Block.
    pub resolved_args: Vec<ResolvedArg>,
    /// The workspace directory the confirm process must reopen to re-invoke the
    /// sink. The other plumbing field the design doc's illustrative struct omits
    /// (RESEARCH Open Question 1 / Assumption A2).
    pub workspace_root_path: String,
    /// `Pending | Confirmed | Denied`. MUST start `Pending` at persistence time.
    pub state: PendingConfirmationState,
}

/// Persist a new `PendingConfirmation` row.
///
/// One `INSERT` binding all seven columns. Serializes `resolved_args` with
/// `serde_json::to_string`, `sink` as `pc.sink.0`, uuids via `.to_string()`,
/// state via `as_str()`. Caller should invoke this under the same broker-owned
/// connection lock as the `append_event` that wrote the anchoring `sink_blocked`
/// row (the two writes MUST succeed or fail together).
pub fn insert_pending_confirmation(
    conn: &rusqlite::Connection,
    pc: &PendingConfirmation,
) -> Result<()> {
    let resolved_args_json = serde_json::to_string(&pc.resolved_args)?;
    conn.execute(
        "INSERT INTO pending_confirmations \
         (effect_id, session_id, blocked_event_id, sink, resolved_args, \
          workspace_root_path, state) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            pc.effect_id.to_string(),
            pc.session_id.to_string(),
            pc.blocked_event_id.to_string(),
            &pc.sink.0,
            &resolved_args_json,
            &pc.workspace_root_path,
            pc.state.as_str(),
        ],
    )?;
    Ok(())
}

/// Fetch a `PendingConfirmation` row by its `effect_id` (indexed PRIMARY KEY
/// lookup), or `None` if no row was ever persisted for that id — the fail-closed
/// case for an untrusted/forged CLI-supplied `effect_id` (T-10-07).
pub fn find_pending_confirmation(
    conn: &rusqlite::Connection,
    effect_id: &str,
) -> Result<Option<PendingConfirmation>> {
    let mut stmt = conn.prepare(
        "SELECT effect_id, session_id, blocked_event_id, sink, resolved_args, \
                workspace_root_path, state \
         FROM pending_confirmations WHERE effect_id = ?1",
    )?;
    let mut rows = stmt.query(params![effect_id])?;
    match rows.next()? {
        Some(row) => {
            let effect_id: String = row.get(0)?;
            let session_id: String = row.get(1)?;
            let blocked_event_id: String = row.get(2)?;
            let sink: String = row.get(3)?;
            let resolved_args_json: String = row.get(4)?;
            let workspace_root_path: String = row.get(5)?;
            let state: String = row.get(6)?;

            let resolved_args: Vec<ResolvedArg> = serde_json::from_str(&resolved_args_json)?;

            Ok(Some(PendingConfirmation {
                effect_id: uuid::Uuid::parse_str(&effect_id)?,
                session_id: uuid::Uuid::parse_str(&session_id)?,
                blocked_event_id: uuid::Uuid::parse_str(&blocked_event_id)?,
                sink: runtime_core::plan_node::SinkId(sink),
                resolved_args,
                workspace_root_path,
                state: PendingConfirmationState::from_str(&state)?,
            }))
        }
        None => Ok(None),
    }
}

/// Transition a `pending_confirmations` row's `state`, returning the number of
/// affected rows.
///
/// A single `UPDATE ... WHERE effect_id = ?2 AND state = 'pending'`. The
/// `AND state = 'pending'` guard is the CONFIRM-03 fail-closed terminal check IN
/// THE SQL: a row already `confirmed`/`denied` matches zero rows, so a
/// re-transition is refused atomically with no read-then-write race. Callers
/// treat a `0` return as "already terminal / refused".
pub fn transition_state(
    conn: &rusqlite::Connection,
    effect_id: &str,
    new_state: PendingConfirmationState,
) -> Result<usize> {
    let affected = conn.execute(
        "UPDATE pending_confirmations SET state = ?1 WHERE effect_id = ?2 AND state = 'pending'",
        params![new_state.as_str(), effect_id],
    )?;
    Ok(affected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::open_audit_db;
    use runtime_core::plan_node::{SinkId, TaintLabel, ValueId};
    use uuid::Uuid;

    fn make_pending_confirmation(effect_id: Uuid) -> PendingConfirmation {
        PendingConfirmation {
            effect_id,
            session_id: Uuid::new_v4(),
            blocked_event_id: Uuid::new_v4(),
            sink: SinkId("file.create".to_string()),
            resolved_args: vec![
                ResolvedArg {
                    name: "path".to_string(),
                    value_id: ValueId::new(),
                    literal: "/workspace/out.txt".to_string(),
                    taint: vec![TaintLabel::PathRaw],
                    provenance_chain: vec![Uuid::new_v4()],
                },
                ResolvedArg {
                    name: "contents".to_string(),
                    value_id: ValueId::new(),
                    literal: "hello world".to_string(),
                    taint: vec![TaintLabel::UserTrusted],
                    provenance_chain: vec![Uuid::new_v4(), Uuid::new_v4()],
                },
            ],
            workspace_root_path: "/workspace".to_string(),
            state: PendingConfirmationState::Pending,
        }
    }

    #[test]
    fn insert_then_find_round_trips_all_fields() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let effect_id = Uuid::new_v4();
        let pc = make_pending_confirmation(effect_id);

        insert_pending_confirmation(&conn, &pc).expect("insert_pending_confirmation");

        let found = find_pending_confirmation(&conn, &effect_id.to_string())
            .expect("find_pending_confirmation")
            .expect("row should be present");

        assert_eq!(found, pc);
        assert_eq!(found.state, PendingConfirmationState::Pending);
    }

    #[test]
    fn find_unknown_effect_id_returns_none() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");

        let found = find_pending_confirmation(&conn, &Uuid::new_v4().to_string())
            .expect("find_pending_confirmation");

        assert!(found.is_none());
    }

    #[test]
    fn transition_pending_to_confirmed_then_denied_is_refused() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let effect_id = Uuid::new_v4();
        let pc = make_pending_confirmation(effect_id);
        insert_pending_confirmation(&conn, &pc).expect("insert_pending_confirmation");

        let effect_id_str = effect_id.to_string();

        let confirmed = transition_state(
            &conn,
            &effect_id_str,
            PendingConfirmationState::Confirmed,
        )
        .expect("transition_state to Confirmed");
        assert_eq!(confirmed, 1);

        let denied_after_confirmed = transition_state(
            &conn,
            &effect_id_str,
            PendingConfirmationState::Denied,
        )
        .expect("transition_state to Denied after Confirmed");
        assert_eq!(denied_after_confirmed, 0);

        let found = find_pending_confirmation(&conn, &effect_id_str)
            .expect("find_pending_confirmation")
            .expect("row should still be present");
        assert_eq!(
            found.state,
            PendingConfirmationState::Confirmed,
            "state must remain durably Confirmed after a refused re-transition"
        );
    }

    #[test]
    fn transition_pending_to_denied_then_confirmed_is_refused() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let effect_id = Uuid::new_v4();
        let pc = make_pending_confirmation(effect_id);
        insert_pending_confirmation(&conn, &pc).expect("insert_pending_confirmation");

        let effect_id_str = effect_id.to_string();

        let denied =
            transition_state(&conn, &effect_id_str, PendingConfirmationState::Denied)
                .expect("transition_state to Denied");
        assert_eq!(denied, 1);

        let confirmed_after_denied = transition_state(
            &conn,
            &effect_id_str,
            PendingConfirmationState::Confirmed,
        )
        .expect("transition_state to Confirmed after Denied");
        assert_eq!(
            confirmed_after_denied, 0,
            "durable-deny: a denied row must never transition to confirmed (CONFIRM-03)"
        );
    }

    #[test]
    fn pending_confirmation_state_from_str_rejects_unknown_string() {
        assert!(PendingConfirmationState::from_str("bogus").is_err());
    }
}
