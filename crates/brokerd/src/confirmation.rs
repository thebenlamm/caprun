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
use chrono::Utc;
use rusqlite::params;
use runtime_core::plan_node::TaintLabel;
use uuid::Uuid;

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

/// The outcome of a `confirm`/`deny` decision. The CLI (`cli/caprun/src/main.rs`)
/// maps each variant to a distinct exit code (DESIGN Exit-code contract) — no
/// stdout parsing required by a scripted caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmOutcome {
    /// Confirm succeeded; the sink was invoked from the frozen snapshot.
    Released,
    /// `confirm_granted` was appended and state transitioned to `Confirmed`,
    /// but the sink re-invocation itself failed (DESIGN Step 4a.5 — at-most-once,
    /// no retry; a durable `sink_invocation_failed` event was already appended by
    /// the sink adapter).
    ConfirmedButSinkFailed,
    /// Deny was recorded; the sink was never invoked.
    Denied,
    /// No `PendingConfirmation` row exists for this `effect_id` — fail closed
    /// (T-10-03: a forged/unknown effect_id can never be released).
    UnknownEffect,
    /// The row is already `Confirmed` or `Denied` — refuse to re-transition
    /// (CONFIRM-03, T-10-01).
    AlreadyTerminal,
    /// The `blocked_literals` row for this `effect_id` was redacted — refuse to
    /// release (T-10-09, Pitfall 4 fail-closed).
    BlockedLiteralRedacted,
}

/// Stable, dotted-lowercase rendering of a `TaintLabel` for the CLI display
/// (e.g. `external.untrusted`, `path.raw` — DESIGN "caprun confirm CLI Contract").
///
/// Explicit exhaustive match — mirrors `TaintLabel::is_untrusted`'s discipline
/// (Pitfall 5): a new variant added without an arm here is a compile error,
/// never a silent fallback.
fn taint_label_display(label: &TaintLabel) -> &'static str {
    match label {
        TaintLabel::UserTrusted => "user.trusted",
        TaintLabel::LocalWorkspace => "local.workspace",
        TaintLabel::ExternalUntrusted => "external.untrusted",
        TaintLabel::EmailRaw => "email.raw",
        TaintLabel::PdfRaw => "pdf.raw",
        TaintLabel::LlmGenerated => "llm.generated",
        TaintLabel::WorkerExtracted => "worker.extracted",
        TaintLabel::PathRaw => "path.raw",
    }
}

/// Compact, display-only rendering of a Uuid's first hyphen-delimited segment
/// (mirrors `cli/caprun/src/main.rs`'s `&hash[..8]` audit-DAG print convention).
/// Never used for identity comparison — only for the human-facing block display.
fn short_evt(id: &Uuid) -> String {
    format!("evt_{}", &id.to_string()[..8])
}

/// Render the exact terminal output for a Pending block (DESIGN
/// "caprun confirm CLI Contract"). Shown by BOTH `confirm` and `deny` before
/// acting, so a human sees the same evidence regardless of which verb they run.
///
/// Selects the display arg as the FIRST `resolved_args` entry carrying an
/// untrusted taint label (the routing-sensitive blocked arg, e.g. `path`). The
/// literal is shown byte-exact, in quotes, with NO truncation or
/// canonicalization (T-10-04 mitigation / DESIGN Accepted Residual Risk 1).
pub fn render_block_display(pc: &PendingConfirmation) -> String {
    let display_arg = pc
        .resolved_args
        .iter()
        .find(|a| a.taint.iter().any(TaintLabel::is_untrusted))
        .or_else(|| pc.resolved_args.first());

    let (arg_name, literal, taint, provenance_chain): (&str, &str, &[TaintLabel], &[Uuid]) =
        match display_arg {
            Some(a) => (
                a.name.as_str(),
                a.literal.as_str(),
                a.taint.as_slice(),
                a.provenance_chain.as_slice(),
            ),
            // Fail-safe only: a genuine I2 block always has at least one arg.
            None => ("(none)", "(none)", &[], &[]),
        };

    let taint_str = taint
        .iter()
        .map(taint_label_display)
        .collect::<Vec<_>>()
        .join(", ");

    let source_evt = provenance_chain
        .first()
        .map(short_evt)
        .unwrap_or_else(|| "(none)".to_string());

    let mut chain_str = provenance_chain
        .iter()
        .map(short_evt)
        .collect::<Vec<_>>()
        .join(" -> ");
    if !chain_str.is_empty() {
        chain_str.push_str(" -> ");
    }
    chain_str.push_str("(this arg)");

    let effect_id = pc.effect_id;
    format!(
        "Effect blocked pending confirmation.\n\
         \n\
         Effect ID:         {effect_id}\n\
         Sink:               {sink}\n\
         Arg:                {arg_name}\n\
         Literal value:      \"{literal}\"\n\
         Taint:              [{taint_str}]\n\
         Source:             file_read {source_evt}  (session {session_id})\n\
         Provenance chain:   {chain_str}\n\
         \n\
         This value came from untrusted content read during this session. Run\n\
         `caprun confirm {effect_id}` to release this EXACT value, or\n\
         `caprun deny {effect_id}` to block it permanently.",
        sink = pc.sink.0,
        session_id = pc.session_id,
    )
}

/// `caprun confirm <effect_id>` decision logic — Steps 1-4a of DESIGN
/// "Confirmation Decision Logic".
///
/// Re-reads `PendingConfirmation.state` from the persisted DB on EVERY
/// invocation — never a cache — because the process running `confirm` is
/// never the same OS process that created the block (CONFIRM-03 cross-process
/// durability guarantee). NEVER calls `executor::submit_plan_node`, constructs a
/// `ValueStore`, or reads/writes any allowlist/standing-policy structure
/// (CONFIRM-02, T-10-05, "Confirm MUST NOT Re-Invoke submit_plan_node").
pub fn confirm(
    conn: &mut rusqlite::Connection,
    effect_id: &str,
    workspace_root: &adapter_fs::workspace::WorkspaceRoot,
) -> Result<ConfirmOutcome> {
    // Step 1: fresh, indexed lookup — fail closed on an unknown/forged id (T-10-03).
    let pc = match find_pending_confirmation(conn, effect_id)? {
        Some(pc) => pc,
        None => return Ok(ConfirmOutcome::UnknownEffect),
    };

    // Step 2: terminal-state check, read from the persisted row (never a cache).
    if pc.state != PendingConfirmationState::Pending {
        return Ok(ConfirmOutcome::AlreadyTerminal);
    }

    // Step 3: redaction gate (Pitfall 4) — refuse to release if the anchoring
    // blocked_literals row was deleted, even though this PendingConfirmation
    // snapshot still holds its own copy of the literal (fail-closed per DESIGN
    // Persistence contract's redaction interplay, T-10-09).
    if crate::audit::get_blocked_literal(conn, &pc.blocked_event_id.to_string())?.is_none() {
        return Ok(ConfirmOutcome::BlockedLiteralRedacted);
    }

    // Step 4: display the verbatim literal + provenance (CONFIRM-01).
    println!("{}", render_block_display(&pc));

    // Step 5: append confirm_granted, anchored onto the sink_blocked event —
    // preserving one unbroken causal chain (CONFIRM-04).
    let block_hash = crate::audit::event_hash_by_id(conn, &pc.blocked_event_id.to_string())?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "internal invariant violation: no event hash for blocked_event_id {}",
                pc.blocked_event_id
            )
        })?;
    let granted_event = runtime_core::Event::new(
        Uuid::new_v4(),
        Some(pc.blocked_event_id),
        pc.session_id,
        format!("confirm:{effect_id}"),
        "confirm_granted".into(),
        Utc::now(),
        vec![],
    );
    let granted_event_id = granted_event.id;
    let granted_hash = crate::audit::append_event(conn, &granted_event, Some(&block_hash))?;

    // Step 6: at-most-once — the state transition is persisted BEFORE the sink
    // is invoked (DESIGN Step 4a.5). A `0` return means a raced re-transition
    // between Step 2 and here — refuse (CONFIRM-03), even though a confirm_granted
    // event was already appended per the DESIGN's specified ordering.
    let affected = transition_state(conn, effect_id, PendingConfirmationState::Confirmed)?;
    if affected == 0 {
        return Ok(ConfirmOutcome::AlreadyTerminal);
    }

    // Step 7: dispatch to the frozen-snapshot sink re-invocation — NEVER
    // executor::submit_plan_node (CON-i2-non-bypassable, T-10-05).
    match pc.sink.0.as_str() {
        "file.create" => match crate::sinks::file_create::invoke_file_create_from_resolved(
            conn,
            pc.session_id,
            pc.effect_id,
            &pc.resolved_args,
            workspace_root,
            granted_event_id,
            &granted_hash,
        ) {
            Ok(_) => Ok(ConfirmOutcome::Released),
            // The sink adapter already appended a durable sink_invocation_failed
            // event; state stays Confirmed, no retry (DESIGN Step 4a.5).
            Err(_) => Ok(ConfirmOutcome::ConfirmedButSinkFailed),
        },
        "email.send" => {
            // Mirror invoke_email_send_stub's no-op append — email.send has no
            // live effect in v1.2, so there is nothing that can fail here.
            let plan_node = runtime_core::PlanNode {
                sink: pc.sink.clone(),
                args: vec![],
            };
            crate::sinks::email_send::invoke_email_send_stub(
                conn,
                pc.session_id,
                &plan_node,
                Some(&granted_hash),
            )?;
            Ok(ConfirmOutcome::Released)
        }
        other => Err(anyhow::anyhow!(
            "confirm: unreachable sink `{other}` — not a registered v1.2 sink"
        )),
    }
}

/// `caprun deny <effect_id>` decision logic — Steps 1-3 + 4b of DESIGN
/// "Confirmation Decision Logic".
///
/// `deny` NEVER invokes any sink — the effect never proceeds (CONFIRM-03).
/// It does not need the redaction gate (it releases nothing), but MUST still
/// find the block and set the causal parent chain onto the sink_blocked event.
pub fn deny(conn: &rusqlite::Connection, effect_id: &str) -> Result<ConfirmOutcome> {
    // Steps 1-2: same fresh lookup + terminal-state check as confirm.
    let pc = match find_pending_confirmation(conn, effect_id)? {
        Some(pc) => pc,
        None => return Ok(ConfirmOutcome::UnknownEffect),
    };
    if pc.state != PendingConfirmationState::Pending {
        return Ok(ConfirmOutcome::AlreadyTerminal);
    }

    // Both verbs show the same evidence before acting (DESIGN CLI Contract).
    println!("{}", render_block_display(&pc));

    let block_hash = crate::audit::event_hash_by_id(conn, &pc.blocked_event_id.to_string())?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "internal invariant violation: no event hash for blocked_event_id {}",
                pc.blocked_event_id
            )
        })?;
    let denied_event = runtime_core::Event::new(
        Uuid::new_v4(),
        Some(pc.blocked_event_id),
        pc.session_id,
        format!("deny:{effect_id}"),
        "confirm_denied".into(),
        Utc::now(),
        vec![],
    );
    crate::audit::append_event(conn, &denied_event, Some(&block_hash))?;

    let affected = transition_state(conn, effect_id, PendingConfirmationState::Denied)?;
    if affected == 0 {
        return Ok(ConfirmOutcome::AlreadyTerminal);
    }

    // Terminal. No retry path. The sink is NEVER invoked on the deny path.
    Ok(ConfirmOutcome::Denied)
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

    // ── confirm/deny decision logic (Task 1) ──────────────────────────────

    use crate::audit::{
        append_event, find_event_by_type, insert_blocked_literal, query_events_by_session,
        redact_blocked_literal,
    };
    use adapter_fs::workspace::WorkspaceRoot;
    use runtime_core::executor_decision::SinkBlockedAnchor;
    use runtime_core::Event;
    use sha2::{Digest, Sha256};

    /// Seed a Pending file.create block: a causal-root event, a `sink_blocked`
    /// event carrying a genuine `SinkBlockedAnchor`, its `blocked_literals` row,
    /// and a matching `PendingConfirmation` — mirroring server.rs's
    /// `SubmitPlanNode` block-time write (minus the live `plan_node`/`ValueStore`,
    /// which do not exist in this unit-test context).
    ///
    /// Returns `(effect_id, session_id, blocked_event_id)`.
    fn seed_pending_file_create_block(
        conn: &rusqlite::Connection,
        path: &str,
        contents: &str,
        workspace_root_path: &str,
    ) -> (Uuid, Uuid, Uuid) {
        let session_id = Uuid::new_v4();
        let effect_id = Uuid::new_v4();
        let read_event_id = Uuid::new_v4();

        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            chrono::Utc::now(),
            vec![],
        );
        let root_hash = append_event(conn, &root, None).unwrap();

        let literal_sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(path.as_bytes());
            hex::encode(hasher.finalize())
        };
        let anchor = SinkBlockedAnchor {
            effect_id,
            sink: SinkId("file.create".into()),
            arg: "path".into(),
            value_id: ValueId::new(),
            literal_sha256,
            taint: vec![TaintLabel::PathRaw],
            provenance_chain: vec![read_event_id],
            read_event_id,
        };
        let blocked_event = Event::sink_blocked(
            Uuid::new_v4(),
            Some(root.id),
            session_id,
            chrono::Utc::now(),
            anchor,
        );
        let blocked_event_id = blocked_event.id;
        append_event(conn, &blocked_event, Some(&root_hash)).unwrap();
        insert_blocked_literal(conn, &blocked_event_id.to_string(), path).unwrap();

        let pc = PendingConfirmation {
            effect_id,
            session_id,
            blocked_event_id,
            sink: SinkId("file.create".into()),
            resolved_args: vec![
                ResolvedArg {
                    name: "path".to_string(),
                    value_id: ValueId::new(),
                    literal: path.to_string(),
                    taint: vec![TaintLabel::PathRaw],
                    provenance_chain: vec![read_event_id],
                },
                ResolvedArg {
                    name: "contents".to_string(),
                    value_id: ValueId::new(),
                    literal: contents.to_string(),
                    taint: vec![TaintLabel::UserTrusted],
                    provenance_chain: vec![],
                },
            ],
            workspace_root_path: workspace_root_path.to_string(),
            state: PendingConfirmationState::Pending,
        };
        insert_pending_confirmation(conn, &pc).unwrap();

        (effect_id, session_id, blocked_event_id)
    }

    /// (a) confirm on a Pending file.create block releases exactly once: the
    /// file is created, a confirm_granted event exists chained onto the
    /// sink_blocked event, and the row transitions to Confirmed.
    #[test]
    fn confirm_on_pending_file_create_releases_and_creates_file() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_ok_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        let outcome = confirm(&mut conn, &effect_id.to_string(), &ws).expect("confirm");
        assert_eq!(outcome, ConfirmOutcome::Released);

        let on_disk = std::fs::read_to_string(root.join("out.txt")).unwrap();
        assert_eq!(on_disk, "hello");

        let granted = find_event_by_type(&conn, &session_id.to_string(), "confirm_granted")
            .unwrap()
            .expect("confirm_granted event must exist");
        assert_eq!(granted.actor, format!("confirm:{effect_id}"));
        assert_eq!(granted.parent_id, Some(blocked_event_id));

        let pc = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(pc.state, PendingConfirmationState::Confirmed);

        std::fs::remove_dir_all(&root).ok();
    }

    /// (b) a second confirm on the same effect_id refuses (AlreadyTerminal) and
    /// creates no new file (CONFIRM-03).
    #[test]
    fn confirm_twice_returns_already_terminal_and_creates_no_new_file() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_twice_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, _session_id, _blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        let first = confirm(&mut conn, &effect_id.to_string(), &ws).expect("first confirm");
        assert_eq!(first, ConfirmOutcome::Released);

        let second = confirm(&mut conn, &effect_id.to_string(), &ws).expect("second confirm");
        assert_eq!(second, ConfirmOutcome::AlreadyTerminal);

        let entries: Vec<_> = std::fs::read_dir(&root).unwrap().collect();
        assert_eq!(
            entries.len(),
            1,
            "a second confirm must not create any additional file"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// (c) deny on a fresh Pending block records a durable denial: a
    /// confirm_denied event exists, state is Denied, and a subsequent confirm
    /// refuses (durable deny, CONFIRM-03). The sink is never invoked.
    #[test]
    fn deny_on_pending_block_is_durable() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_deny_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        let outcome = deny(&conn, &effect_id.to_string()).expect("deny");
        assert_eq!(outcome, ConfirmOutcome::Denied);

        let denied = find_event_by_type(&conn, &session_id.to_string(), "confirm_denied")
            .unwrap()
            .expect("confirm_denied event must exist");
        assert_eq!(denied.actor, format!("deny:{effect_id}"));
        assert_eq!(denied.parent_id, Some(blocked_event_id));

        let pc = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(pc.state, PendingConfirmationState::Denied);

        let later = confirm(&mut conn, &effect_id.to_string(), &ws).expect("confirm after deny");
        assert_eq!(later, ConfirmOutcome::AlreadyTerminal);
        assert!(
            !root.join("out.txt").exists(),
            "deny must permanently prevent the effect from ever running"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// (d) confirm on an effect_id whose blocked_literals row was redacted
    /// refuses to release and creates no file (T-10-09 fail-closed).
    #[test]
    fn confirm_with_redacted_blocked_literal_refuses_to_release() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_redacted_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, _session_id, blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        redact_blocked_literal(&conn, &blocked_event_id.to_string()).unwrap();

        let outcome = confirm(&mut conn, &effect_id.to_string(), &ws).expect("confirm");
        assert_eq!(outcome, ConfirmOutcome::BlockedLiteralRedacted);
        assert!(
            !root.join("out.txt").exists(),
            "a redacted blocked literal must never be released"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// (e) confirm/deny on an unknown effect_id return UnknownEffect (T-10-03).
    #[test]
    fn confirm_and_deny_on_unknown_effect_id_return_unknown_effect() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_unknown_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let unknown = Uuid::new_v4().to_string();

        assert_eq!(
            confirm(&mut conn, &unknown, &ws).expect("confirm"),
            ConfirmOutcome::UnknownEffect
        );
        assert_eq!(
            deny(&conn, &unknown).expect("deny"),
            ConfirmOutcome::UnknownEffect
        );

        std::fs::remove_dir_all(&root).ok();
    }

    // ── email.send atomic CAS + email_send_attempted (Task 2, SEND-01/SEND-02) ──

    /// Seed a Pending email.send block: a causal-root event, a `sink_blocked`
    /// event carrying a genuine `SinkBlockedAnchor` on the tainted `to` arg,
    /// its `blocked_literals` row, and a matching `PendingConfirmation` —
    /// mirrors `seed_pending_file_create_block` but for the `email.send` sink.
    /// `workspace_root_path` is set to a throwaway value: `confirm()`'s
    /// `email.send` arm never reads it (only `file.create` does).
    ///
    /// Returns `(effect_id, session_id, blocked_event_id)`.
    fn seed_pending_email_send_block(
        conn: &rusqlite::Connection,
        to: &str,
        subject: &str,
        body: &str,
    ) -> (Uuid, Uuid, Uuid) {
        let session_id = Uuid::new_v4();
        let effect_id = Uuid::new_v4();
        let read_event_id = Uuid::new_v4();

        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            chrono::Utc::now(),
            vec![],
        );
        let root_hash = append_event(conn, &root, None).unwrap();

        let literal_sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(to.as_bytes());
            hex::encode(hasher.finalize())
        };
        let anchor = SinkBlockedAnchor {
            effect_id,
            sink: SinkId("email.send".into()),
            arg: "to".into(),
            value_id: ValueId::new(),
            literal_sha256,
            taint: vec![TaintLabel::ExternalUntrusted],
            provenance_chain: vec![read_event_id],
            read_event_id,
        };
        let blocked_event = Event::sink_blocked(
            Uuid::new_v4(),
            Some(root.id),
            session_id,
            chrono::Utc::now(),
            anchor,
        );
        let blocked_event_id = blocked_event.id;
        append_event(conn, &blocked_event, Some(&root_hash)).unwrap();
        insert_blocked_literal(conn, &blocked_event_id.to_string(), to).unwrap();

        let pc = PendingConfirmation {
            effect_id,
            session_id,
            blocked_event_id,
            sink: SinkId("email.send".into()),
            resolved_args: vec![
                ResolvedArg {
                    name: "to".to_string(),
                    value_id: ValueId::new(),
                    literal: to.to_string(),
                    taint: vec![TaintLabel::ExternalUntrusted],
                    provenance_chain: vec![read_event_id],
                },
                ResolvedArg {
                    name: "subject".to_string(),
                    value_id: ValueId::new(),
                    literal: subject.to_string(),
                    taint: vec![TaintLabel::UserTrusted],
                    provenance_chain: vec![],
                },
                ResolvedArg {
                    name: "body".to_string(),
                    value_id: ValueId::new(),
                    literal: body.to_string(),
                    taint: vec![TaintLabel::UserTrusted],
                    provenance_chain: vec![],
                },
            ],
            workspace_root_path: "/unused-for-email-send".to_string(),
            state: PendingConfirmationState::Pending,
        };
        insert_pending_confirmation(conn, &pc).unwrap();

        (effect_id, session_id, blocked_event_id)
    }

    /// Minimal in-process fake SMTP server for the SEND-01 "first confirm
    /// really sends" test path — accepts exactly ONE connection and speaks
    /// just enough SMTP for `lettre::SmtpTransport::send` to complete
    /// successfully (banner, EHLO, MAIL FROM, RCPT TO, DATA, dot-terminated
    /// message body, QUIT), then closes. Runs on a background thread bound to
    /// an OS-assigned ephemeral port. Returns the port to point
    /// `CAPRUN_SMTP_HOST`/`CAPRUN_SMTP_PORT` at.
    fn spawn_fake_smtp_accept_server() -> u16 {
        use std::io::{BufRead, Write};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut writer = stream;
                let _ = writer.write_all(b"220 test.local ESMTP\r\n");
                let mut in_data = false;
                let mut line = String::new();
                loop {
                    line.clear();
                    let n = reader.read_line(&mut line).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    if in_data {
                        if line == ".\r\n" {
                            in_data = false;
                            let _ = writer.write_all(b"250 2.0.0 OK: queued\r\n");
                        }
                        continue;
                    }
                    let upper = line.to_ascii_uppercase();
                    if upper.starts_with("EHLO") {
                        let _ = writer.write_all(b"250 test.local\r\n");
                    } else if upper.starts_with("MAIL FROM") {
                        let _ = writer.write_all(b"250 2.1.0 OK\r\n");
                    } else if upper.starts_with("RCPT TO") {
                        let _ = writer.write_all(b"250 2.1.5 OK\r\n");
                    } else if upper.starts_with("DATA") {
                        let _ = writer.write_all(b"354 Start mail input\r\n");
                        in_data = true;
                    } else if upper.starts_with("QUIT") {
                        let _ = writer.write_all(b"221 2.0.0 Bye\r\n");
                        break;
                    } else {
                        let _ = writer.write_all(b"250 OK\r\n");
                    }
                }
            }
        });
        port
    }

    /// Count events of `event_type` for `session_id` (there is no dedicated
    /// count helper in `audit.rs`; `query_events_by_session` + filter is the
    /// simplest way to assert "exactly one" without adding new production API
    /// surface for a test-only need).
    fn count_events_of_type(conn: &rusqlite::Connection, session_id: Uuid, event_type: &str) -> usize {
        query_events_by_session(conn, &session_id.to_string())
            .unwrap()
            .into_iter()
            .filter(|e| e.event_type == event_type)
            .count()
    }

    /// (SEND-01) A first confirm of a Pending email.send block releases: the
    /// CAS + `email_send_attempted` append committed atomically, the adapter's
    /// real send succeeded (fake SMTP server), and `email_send_succeeded` was
    /// recorded. A SECOND confirm on the SAME effect_id refuses
    /// (AlreadyTerminal) and does NOT append a second `email_send_attempted` —
    /// exactly ONE exists in the audit DAG for this effect_id, proving the CAS
    /// + attempt-append atomicity closes the double-fire window.
    #[test]
    fn confirm_email_send_twice_records_exactly_one_attempted_event() {
        let _guard = crate::sinks::email_smtp::SMTP_ENV_LOCK.lock().unwrap();

        let port = spawn_fake_smtp_accept_server();
        std::env::set_var("CAPRUN_SMTP_HOST", "127.0.0.1");
        std::env::set_var("CAPRUN_SMTP_PORT", port.to_string());

        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_email_ok_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, _blocked_event_id) =
            seed_pending_email_send_block(&conn, "recipient@example.com", "hello", "hi there");

        let first = confirm(&mut conn, &effect_id.to_string(), &ws).expect("first confirm");
        assert_eq!(
            first,
            ConfirmOutcome::Released,
            "first confirm of a Pending email.send block must Release (real send succeeded)"
        );

        let second = confirm(&mut conn, &effect_id.to_string(), &ws).expect("second confirm");
        assert_eq!(
            second,
            ConfirmOutcome::AlreadyTerminal,
            "a re-issued confirm on the same effect_id must refuse, never re-send"
        );

        assert_eq!(
            count_events_of_type(&conn, session_id, "email_send_attempted"),
            1,
            "exactly ONE email_send_attempted event must exist regardless of how many confirms were issued (SEND-01)"
        );
        assert_eq!(
            count_events_of_type(&conn, session_id, "email_send_succeeded"),
            1,
            "exactly ONE email_send_succeeded event must exist (the second confirm never re-sent)"
        );

        std::env::remove_var("CAPRUN_SMTP_HOST");
        std::env::remove_var("CAPRUN_SMTP_PORT");
        std::fs::remove_dir_all(&root).ok();
    }

    /// (SEND-02) When the adapter's real send fails (closed/unbound port),
    /// `confirm()` returns the distinct `ConfirmOutcome::EmailSendFailed` —
    /// never the file.create-style `ConfirmedButSinkFailed` swallow-shape.
    /// The CAS + `email_send_attempted` append have ALREADY committed
    /// (atomically, before the socket was ever opened) — a durable
    /// `email_send_failed` event also exists, and NO `email_send_succeeded`
    /// event was ever appended. No auto-retry: this is a one-shot decision.
    #[test]
    fn confirm_email_send_adapter_failure_yields_email_send_failed() {
        let _guard = crate::sinks::email_smtp::SMTP_ENV_LOCK.lock().unwrap();

        // Bind an ephemeral port then immediately drop the listener — nothing
        // is listening on it for the rest of this test, so a connect attempt
        // is refused (ECONNREFUSED) almost immediately (mirrors
        // email_smtp.rs's own transport-failure test).
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        std::env::set_var("CAPRUN_SMTP_HOST", "127.0.0.1");
        std::env::set_var("CAPRUN_SMTP_PORT", port.to_string());

        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_email_fail_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, _blocked_event_id) =
            seed_pending_email_send_block(&conn, "recipient@example.com", "hello", "hi there");

        let outcome = confirm(&mut conn, &effect_id.to_string(), &ws).expect("confirm");
        assert_eq!(
            outcome,
            ConfirmOutcome::EmailSendFailed,
            "a closed-port send failure must surface as the distinct EmailSendFailed outcome, never ConfirmedButSinkFailed or a swallowed Ok"
        );

        assert_eq!(
            count_events_of_type(&conn, session_id, "email_send_attempted"),
            1,
            "the CAS + email_send_attempted transaction must have committed BEFORE the failed send attempt"
        );
        assert_eq!(
            count_events_of_type(&conn, session_id, "email_send_failed"),
            1,
            "a durable email_send_failed event must be appended on adapter failure"
        );
        assert_eq!(
            count_events_of_type(&conn, session_id, "email_send_succeeded"),
            0,
            "no email_send_succeeded event may exist on the failure path"
        );

        // A re-confirm must not retry the send — it is already terminal
        // (Confirmed), refusing per the CAS (no auto-retry, SEND-02).
        let second = confirm(&mut conn, &effect_id.to_string(), &ws).expect("second confirm");
        assert_eq!(second, ConfirmOutcome::AlreadyTerminal);
        assert_eq!(
            count_events_of_type(&conn, session_id, "email_send_attempted"),
            1,
            "a re-confirm after a send failure must NOT append a second email_send_attempted"
        );

        std::env::remove_var("CAPRUN_SMTP_HOST");
        std::env::remove_var("CAPRUN_SMTP_PORT");
        std::fs::remove_dir_all(&root).ok();
    }
}
