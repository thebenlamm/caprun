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
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// The single shared primitive computing CONFIRM-03's combined digest over a
/// full `(arg_name, literal)` set (`planning-docs/DESIGN-confirm-binding.md`,
/// "Combined-Digest Binding", Round-6 amendment).
///
/// Both the PRODUCER (`server.rs`'s Block-time write, Task 2 of this plan)
/// and the VERIFIER (`confirm()`, Plan 16-02) call this SAME function over
/// the SAME domain — every element of `resolved_args` (blocked AND trusted
/// together) — so they CANNOT diverge on ordering, uniqueness, or element
/// encoding. The primitive owns all three:
///
/// 1. **Uniqueness** — asserts arg-name uniqueness across `args` (fail-closed:
///    a duplicate name makes the canonical order undefined — DESIGN Round-6
///    MUST).
/// 2. **Canonical order** — sorts a COPY of `args` by BYTE-WISE ASCENDING
///    `arg_name` (Rust `str`'s `Ord`, which compares the underlying UTF-8
///    bytes — never a locale/collation compare), so callers may hand it the
///    set in ANY input order.
/// 3. **Element encoding** — for each pair, IN THAT CANONICAL ORDER, computes
///    the arg_name's fixed-width 64-hex SHA-256 digest AND the literal's
///    fixed-width 64-hex SHA-256 digest (the exact `literal_sha256` pattern
///    from `crates/executor/src/lib.rs`, applied to both name and literal),
///    then feeds both 64-hex strings — name-digest THEN literal-digest — into
///    ONE outer SHA-256 hasher. Returns `hex::encode` of the outer hasher's
///    finalize.
///
/// This is `sha256(name)‖sha256(literal)` per element, at FIXED 64-hex
/// width — NOT raw literals, NOT literal-only, NOT plain concatenation: the
/// fixed width removes the partition-blindness bypass (`to`/`body` boundary
/// shift, DESIGN finding #4) and binding the name removes the rename bypass
/// (DESIGN Round-6: renaming a bound arg without binding its name would leave
/// the sorted literal sequence unchanged).
///
/// # Panics
/// Panics (fail-closed) if `args` contains a duplicate `arg_name` — the
/// ordering is undefined otherwise, and a producer/verifier pair silently
/// picking different "winners" for the duplicate would be a correctness
/// defect, not a recoverable case.
pub fn combined_digest(args: &[(&str, &str)]) -> String {
    let mut sorted: Vec<(&str, &str)> = args.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    for pair in sorted.windows(2) {
        assert_ne!(
            pair[0].0,
            pair[1].0,
            "combined_digest: duplicate arg_name `{}` — arg-name uniqueness \
             MUST be asserted before hashing (DESIGN-confirm-binding.md Round-6)",
            pair[0].0
        );
    }

    let mut hasher = Sha256::new();
    for (name, literal) in &sorted {
        let name_digest = {
            let mut h = Sha256::new();
            h.update(name.as_bytes());
            hex::encode(h.finalize())
        };
        let literal_digest = {
            let mut h = Sha256::new();
            h.update(literal.as_bytes());
            hex::encode(h.finalize())
        };
        hasher.update(name_digest.as_bytes());
        hasher.update(literal_digest.as_bytes());
    }
    hex::encode(hasher.finalize())
}

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
    /// The ordered subset of `resolved_args` names that were BLOCKED (Phase 16,
    /// DESIGN-confirm-binding.md Round-6) — DISPLAY-MARKING metadata ONLY
    /// (which of the full set Plan 16-02's narration marks BLOCKED vs
    /// TRUSTED). Does NOT select `combined_digest`'s domain — that is every
    /// current `resolved_args` element, blocked and trusted together. Frozen
    /// at Block time, never re-derived.
    pub blocked_arg_names: Vec<String>,
    /// The CONFIRM-03 combined SHA-256 digest over EVERY current
    /// `resolved_args` element's name+literal (Round-6 amendment),
    /// byte-wise-ascending-name order — mirrors the value stored in the
    /// hashed `sink_blocked` Event payload (the tamper-evident source of
    /// truth). Frozen at Block time, never re-derived: recompute-and-compared
    /// (never overwritten) at confirm/send time (Plan 16-02).
    pub combined_digest: String,
    /// The workspace directory the confirm process must reopen to re-invoke the
    /// sink. The other plumbing field the design doc's illustrative struct omits
    /// (RESEARCH Open Question 1 / Assumption A2).
    pub workspace_root_path: String,
    /// `Pending | Confirmed | Denied`. MUST start `Pending` at persistence time.
    pub state: PendingConfirmationState,
}

/// Persist a new `PendingConfirmation` row.
///
/// One `INSERT` binding all nine columns. Serializes `resolved_args` and
/// `blocked_arg_names` with `serde_json::to_string`, `sink` as `pc.sink.0`,
/// uuids via `.to_string()`, state via `as_str()`. Caller should invoke this
/// under the same broker-owned connection lock as the `append_event` that
/// wrote the anchoring `sink_blocked` row (the two writes MUST succeed or
/// fail together).
pub fn insert_pending_confirmation(
    conn: &rusqlite::Connection,
    pc: &PendingConfirmation,
) -> Result<()> {
    let resolved_args_json = serde_json::to_string(&pc.resolved_args)?;
    let blocked_arg_names_json = serde_json::to_string(&pc.blocked_arg_names)?;
    conn.execute(
        "INSERT INTO pending_confirmations \
         (effect_id, session_id, blocked_event_id, sink, resolved_args, \
          blocked_arg_names, combined_digest, workspace_root_path, state) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            pc.effect_id.to_string(),
            pc.session_id.to_string(),
            pc.blocked_event_id.to_string(),
            &pc.sink.0,
            &resolved_args_json,
            &blocked_arg_names_json,
            &pc.combined_digest,
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
                blocked_arg_names, combined_digest, workspace_root_path, state \
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
            let blocked_arg_names_json: String = row.get(5)?;
            let combined_digest: String = row.get(6)?;
            let workspace_root_path: String = row.get(7)?;
            let state: String = row.get(8)?;

            let resolved_args: Vec<ResolvedArg> = serde_json::from_str(&resolved_args_json)?;
            let blocked_arg_names: Vec<String> = serde_json::from_str(&blocked_arg_names_json)?;

            Ok(Some(PendingConfirmation {
                effect_id: uuid::Uuid::parse_str(&effect_id)?,
                session_id: uuid::Uuid::parse_str(&session_id)?,
                blocked_event_id: uuid::Uuid::parse_str(&blocked_event_id)?,
                sink: runtime_core::plan_node::SinkId(sink),
                resolved_args,
                blocked_arg_names,
                combined_digest,
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
    /// The `email.send` CAS + `email_send_attempted` transaction committed
    /// (state is durably `Confirmed`), but the real SMTP send itself failed.
    /// Distinct from `ConfirmedButSinkFailed` (SEND-02): the v1.2
    /// `Err(_) => Ok(ConfirmedButSinkFailed)` swallow-shape is explicitly
    /// rejected for `email.send` — this variant, plus a durable
    /// `email_send_failed` event already appended by the adapter, is how the
    /// failure is surfaced without being swallowed. No auto-retry.
    EmailSendFailed,
    /// `caprun review <effect_id>` displayed the block narration for a KNOWN
    /// effect_id — WITHOUT transitioning `PendingConfirmationState`, appending
    /// any Event, or invoking any sink (MAJOR-8 pre-decision surface). Purely
    /// read-only: always safe to re-run, any number of times, on a Pending
    /// OR terminal row.
    Reviewed,
    /// The audit chain was broken (`audit::verify_chain` returned `false`),
    /// OR the FULL-set `combined_digest` recomputed from the frozen
    /// `resolved_args` snapshot did NOT match the hash-chained `sink_blocked`
    /// Event's stored `combined_digest` (or that Event/field was missing) —
    /// confirm refuses to release (BLOCKER-2, MAJOR-3).
    ///
    /// This is an INTEGRITY ALARM, not an operator deny: the row is
    /// intentionally LEFT `Pending` (retriable), never auto-transitioned to
    /// `Denied` — an actor who can TRIGGER a mismatch (e.g. by tampering a
    /// trusted arg's literal) must NOT thereby gain the power to
    /// force-terminate a confirmation a human might still legitimately retry.
    /// A durable `confirm_digest_mismatch` Event is appended, parented on the
    /// CURRENT CHAIN HEAD (never `blocked_event_id` — MAJOR-7, so a
    /// mismatch→retry sequence extends the chain linearly and never forks
    /// `verify_chain`). No sink is ever invoked on this path.
    ///
    /// HONESTY (MAJOR-6): `audit::verify_chain` recomputes hashes from the
    /// SAME SQLite store and nothing pins the chain head — an actor with
    /// `events` table write access could forge the chain end-to-end. Wiring
    /// it here detects single-store and non-recomputing multi-store
    /// tampering; it does NOT make tamper-evidence "fully closed." The
    /// chain-head-not-anchored residual risk is an Accepted Residual Risk
    /// (v2 obligation: keyed-MAC / external head-pin) — see
    /// `.planning/todos/pending`.
    DigestMismatch,
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
/// "caprun confirm CLI Contract" + "Block Narration for Every Arg", Round-6).
/// Shown by `confirm`, `deny`, AND the read-only `review` verb, so a human
/// sees the SAME evidence regardless of which of the three they run.
///
/// Narrates EVERY element of `pc.resolved_args` — blocked AND trusted
/// together — in BYTE-WISE ASCENDING `arg_name` order (Rust `str`'s `Ord`),
/// the SAME canonical order `combined_digest` binds, so a human manually
/// re-deriving the digest from this display needs no separate ordering
/// convention. Each arg is marked `[BLOCKED]` (its name is in
/// `pc.blocked_arg_names`) or `[trusted]`, and its literal is shown byte-exact,
/// in quotes, with NO truncation, elision, or canonicalization (DESIGN
/// "Verbatim Display — No Truncation" / T-10-04 mitigation / Accepted
/// Residual Risk 1) — this MUST widen to the full set because Round-6 widened
/// `combined_digest`'s domain to the full set too: a display that still
/// showed only the blocked subset would let a human confirm bytes (a trusted
/// arg) the display never showed them.
///
/// T-14-08 (superseded): this function's PRIOR shape was a single-arg display
/// selection guarded by a FAIL-CLOSED `assert!(blocked_count <= 1)` panic on
/// any genuinely-plural block (proven to fire in a committed regression test,
/// `render_block_display_panics_on_genuine_two_blocked_arg_block_t14_08`,
/// BEFORE this rewrite replaced it — the two-commit discipline). That
/// single-arg selection and its guard are GONE: every arg is now narrated,
/// so there is no "plural" case left to panic on.
pub fn render_block_display(pc: &PendingConfirmation) -> String {
    let mut sorted_args: Vec<&ResolvedArg> = pc.resolved_args.iter().collect();
    sorted_args.sort_by(|a, b| a.name.cmp(&b.name));

    let mut per_arg = String::new();
    for arg in &sorted_args {
        let marker = if pc.blocked_arg_names.iter().any(|n| n == &arg.name) {
            "BLOCKED"
        } else {
            "trusted"
        };

        let taint_str = arg
            .taint
            .iter()
            .map(taint_label_display)
            .collect::<Vec<_>>()
            .join(", ");

        let source_evt = arg
            .provenance_chain
            .first()
            .map(short_evt)
            .unwrap_or_else(|| "(none)".to_string());

        let mut chain_str = arg
            .provenance_chain
            .iter()
            .map(short_evt)
            .collect::<Vec<_>>()
            .join(" -> ");
        if !chain_str.is_empty() {
            chain_str.push_str(" -> ");
        }
        chain_str.push_str("(this arg)");

        per_arg.push_str(&format!(
            "\n\
             Arg:                {name} [{marker}]\n\
             Literal value:      \"{literal}\"\n\
             Taint:              [{taint_str}]\n\
             Source:             {source_evt}  (session {session_id})\n\
             Provenance chain:   {chain_str}\n",
            name = arg.name,
            literal = arg.literal,
            session_id = pc.session_id,
        ));
    }

    let effect_id = pc.effect_id;
    format!(
        "Effect blocked pending confirmation.\n\
         \n\
         Effect ID:         {effect_id}\n\
         Sink:               {sink}\n\
         {per_arg}\n\
         This session is Draft / untrusted-seeded (I0/I1): it was seeded from\n\
         untrusted content read during this session. Confirming authorizes an\n\
         IRREVERSIBLE EXTERNAL send of every literal shown above, EXACTLY as\n\
         shown, from that posture — this confirm IS the I0/I1 human gate.\n\
         \n\
         Run `caprun confirm {effect_id}` to release this EXACT set, or\n\
         `caprun deny {effect_id}` to block it permanently.",
        sink = pc.sink.0,
    )
}

/// `caprun review <effect_id>` — read-only pre-decision surface (MAJOR-8).
///
/// Prints the SAME narration `confirm`/`deny` would show, via the SAME
/// `render_block_display` call — WITHOUT transitioning
/// `PendingConfirmationState`, appending any Event, or invoking any sink /
/// `executor::submit_plan_node`. Exists because `render_block_display`'s only
/// two call sites were previously INSIDE `confirm()`/`deny()`, AFTER the
/// operator already typed the decision verb — meaning the human "confirmed"
/// bytes they had not yet read. `review` gives the human the verbatim
/// literals + provenance at a point where they can still decide either way.
/// Idempotent: running it any number of times never changes state.
pub fn review(conn: &rusqlite::Connection, effect_id: &str) -> Result<ConfirmOutcome> {
    let pc = match find_pending_confirmation(conn, effect_id)? {
        Some(pc) => pc,
        None => return Ok(ConfirmOutcome::UnknownEffect),
    };
    println!("{}", render_block_display(&pc));
    Ok(ConfirmOutcome::Reviewed)
}

/// Fetch `audit::current_chain_head` and fail closed (an internal invariant
/// violation, not a normal `ConfirmOutcome`) if the session has NO events at
/// all — a `PendingConfirmation` row always implies at least the anchoring
/// `sink_blocked` Event exists, so a `None` here means the DB is corrupt in a
/// way this module has no recovery for.
fn current_chain_head_or_bail(
    conn: &rusqlite::Connection,
    session_id: uuid::Uuid,
) -> Result<(Uuid, String)> {
    crate::audit::current_chain_head(conn, &session_id.to_string())?.ok_or_else(|| {
        anyhow::anyhow!(
            "internal invariant violation: no chain head for session {session_id} \
             (a PendingConfirmation row implies at least one event exists)"
        )
    })
}

/// Append a durable `confirm_digest_mismatch` Event, parented on the CURRENT
/// CHAIN HEAD (never `blocked_event_id` — MAJOR-7): this is what keeps a
/// mismatch→retry sequence a LINEAR extension of the chain rather than a
/// fork of `blocked_event_id` (which would permanently break
/// `audit::verify_chain`'s single-linear-chain walk, exactly as
/// `quarantine.rs`'s `chain_head_id` doc comment describes empirically
/// discovering for `mint_from_read`).
fn append_confirm_digest_mismatch_event(
    conn: &rusqlite::Connection,
    pc: &PendingConfirmation,
    effect_id: &str,
    head_id: Uuid,
    head_hash: &str,
) -> Result<()> {
    let mismatch_event = runtime_core::Event::new(
        Uuid::new_v4(),
        Some(head_id),
        pc.session_id,
        format!("confirm:{effect_id}"),
        "confirm_digest_mismatch".into(),
        Utc::now(),
        vec![],
    );
    crate::audit::append_event(conn, &mismatch_event, Some(head_hash))?;
    Ok(())
}

/// `caprun confirm <effect_id>` decision logic — Steps 1-4a of DESIGN
/// "Confirmation Decision Logic", extended (Plan 16-02) with a chain-verify +
/// FULL-set recompute-and-compare integrity gate BEFORE any send.
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

    // Step 4.5a: CHAIN-VERIFY FIRST (MAJOR-3) — fail closed on a broken chain
    // BEFORE trusting ANYTHING read back from the hash-chained `sink_blocked`
    // Event. `find_event_by_id` only DESERIALIZES; it does NOT check hashes.
    // HONESTY (MAJOR-6): `verify_chain` recomputes hashes from the SAME
    // SQLite store and nothing pins the chain head — this detects
    // single-store and non-recomputing multi-store tampering, it is NOT
    // authenticated/externally-anchored (see ConfirmOutcome::DigestMismatch's
    // doc comment for the full honest scope + Accepted Residual Risk).
    if !crate::audit::verify_chain(conn, &pc.session_id.to_string()) {
        let (head_id, head_hash) = current_chain_head_or_bail(conn, pc.session_id)?;
        append_confirm_digest_mismatch_event(conn, &pc, effect_id, head_id, &head_hash)?;
        return Ok(ConfirmOutcome::DigestMismatch);
    }

    // Step 4.5b: RECOMPUTE-AND-COMPARE over the FULL current resolved_args set
    // (BLOCKER-2) — NEVER the blocked subset. `pairs` is fed to the SAME
    // shared `combined_digest` primitive the producer (server.rs, Plan 16-01)
    // used: it owns the byte-wise-ascending-name sort, the name-uniqueness
    // assertion, and the sha256(name)‖sha256(literal) element binding, so
    // producer and verifier cannot diverge on ordering/uniqueness/encoding.
    // These pairs borrow directly from `pc.resolved_args` — the SAME frozen,
    // single in-memory snapshot that Step 7 below hands to the sink; there is
    // NO intervening DB read between this compare and the send (no TOCTOU
    // window).
    let pairs: Vec<(&str, &str)> = pc
        .resolved_args
        .iter()
        .map(|a| (a.name.as_str(), a.literal.as_str()))
        .collect();
    let recomputed_digest = combined_digest(&pairs);

    let authoritative_digest = crate::audit::find_event_by_id(
        conn,
        &pc.session_id.to_string(),
        &pc.blocked_event_id.to_string(),
    )?
    .and_then(|e| e.combined_digest);

    // Fail closed uniformly whether the sink_blocked Event is missing, its
    // combined_digest field is absent, or the recompute simply disagrees —
    // all three are the SAME integrity alarm from confirm()'s perspective.
    if authoritative_digest.as_deref() != Some(recomputed_digest.as_str()) {
        let (head_id, head_hash) = current_chain_head_or_bail(conn, pc.session_id)?;
        append_confirm_digest_mismatch_event(conn, &pc, effect_id, head_id, &head_hash)?;
        return Ok(ConfirmOutcome::DigestMismatch);
    }

    // Step 5: append confirm_granted, parented on the CURRENT CHAIN HEAD
    // (MAJOR-7) — NOT `pc.blocked_event_id` directly. In the single-shot case
    // (nothing appended since the Block) the head IS `blocked_event_id`, so
    // existing `parent_id == Some(blocked_event_id)` assertions still hold;
    // after a mismatch has appended a `confirm_digest_mismatch` event, the
    // head has advanced and this keeps the DAG linear rather than forking
    // `blocked_event_id`.
    let (head_id, head_hash) = current_chain_head_or_bail(conn, pc.session_id)?;
    let granted_event = runtime_core::Event::new(
        Uuid::new_v4(),
        Some(head_id),
        pc.session_id,
        format!("confirm:{effect_id}"),
        "confirm_granted".into(),
        Utc::now(),
        vec![],
    );
    let granted_event_id = granted_event.id;
    let granted_hash = crate::audit::append_event(conn, &granted_event, Some(&head_hash))?;

    // Step 6: at-most-once — the state transition is persisted BEFORE the sink
    // is invoked (DESIGN Step 4a.5). A `0` return means a raced re-transition
    // between Step 2 and here — refuse (CONFIRM-03), even though a confirm_granted
    // event was already appended per the DESIGN's specified ordering.
    //
    // `email.send` is special-cased below (Task 2, SEND-01): it owns its OWN
    // CAS inside an atomic transaction wrapping BOTH the CAS and the durable
    // `email_send_attempted` append, committed BEFORE any SMTP connection
    // opens. This generic, unconditional transition MUST be skipped for
    // `email.send` — otherwise the CAS would already be consumed here,
    // permanently breaking the "one atomic transaction" requirement below
    // (two separate autocommit statements, not one atomic unit).
    if pc.sink.0.as_str() != "email.send" {
        let affected = transition_state(conn, effect_id, PendingConfirmationState::Confirmed)?;
        if affected == 0 {
            return Ok(ConfirmOutcome::AlreadyTerminal);
        }
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
            // SEND-01: the CAS (`pending -> confirmed`) and the durable
            // `email_send_attempted` append MUST commit in ONE atomic SQLite
            // transaction, BEFORE any SMTP connection is opened (Pattern 2,
            // DESIGN "At-Most-Once Send + Durable Attempt Ledger"). A zero-row
            // CAS rolls back automatically when `tx` drops without `.commit()`
            // — no attempt event is appended, and the send is never attempted.
            let tx = conn.transaction()?;
            let affected = transition_state(&tx, effect_id, PendingConfirmationState::Confirmed)?;
            if affected == 0 {
                return Ok(ConfirmOutcome::AlreadyTerminal);
            }
            let attempted_event = runtime_core::Event::new(
                Uuid::new_v4(),
                Some(granted_event_id),
                pc.session_id,
                format!("sink:email.send:{effect_id}"),
                "email_send_attempted".into(),
                Utc::now(),
                vec![],
            );
            let attempted_event_id = attempted_event.id;
            let attempted_hash =
                crate::audit::append_event(&tx, &attempted_event, Some(&granted_hash))?;
            tx.commit()?;

            // AFTER commit — the CAS + attempt are now durable together, or
            // neither is; only now does an SMTP connection ever open.
            match crate::sinks::email_smtp::invoke_email_smtp_from_resolved(
                conn,
                pc.session_id,
                pc.effect_id,
                &pc.resolved_args,
                attempted_event_id,
                &attempted_hash,
            ) {
                Ok(_) => Ok(ConfirmOutcome::Released),
                // The adapter already appended a durable, opaque-payload
                // email_send_failed event and routed raw error context to
                // this codebase's eprintln! logging convention — never
                // swallow (unlike file.create's ConfirmedButSinkFailed
                // shape), never auto-retry (SEND-02).
                Err(_) => Ok(ConfirmOutcome::EmailSendFailed),
            }
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
/// find the block and set the causal parent chain onto the CURRENT CHAIN HEAD
/// (MAJOR-7 — never `blocked_event_id` directly; in the single-shot case the
/// head IS `blocked_event_id`, so this is behavior-preserving there).
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

    let (head_id, head_hash) = current_chain_head_or_bail(conn, pc.session_id)?;
    let denied_event = runtime_core::Event::new(
        Uuid::new_v4(),
        Some(head_id),
        pc.session_id,
        format!("deny:{effect_id}"),
        "confirm_denied".into(),
        Utc::now(),
        vec![],
    );
    crate::audit::append_event(conn, &denied_event, Some(&head_hash))?;

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

    // ── combined_digest primitive (Task 1, CONFIRM-03 / DESIGN Round-6) ─────

    /// Local test helper mirroring the exact `literal_sha256` pattern
    /// `combined_digest` uses internally, so the expected-formula test can
    /// independently recompute the SAME value without calling into the
    /// function under test.
    fn sha256_hex(s: &str) -> String {
        let mut h = Sha256::new();
        h.update(s.as_bytes());
        hex::encode(h.finalize())
    }

    /// `combined_digest(&[("to", "a")])` equals
    /// `SHA-256(sha256_hex("to") ‖ sha256_hex("a"))` hex-encoded — the exact
    /// single-element formula (DESIGN "Combined-Digest Binding").
    #[test]
    fn combined_digest_single_element_matches_expected_formula() {
        let expected = {
            let mut h = Sha256::new();
            h.update(sha256_hex("to").as_bytes());
            h.update(sha256_hex("a").as_bytes());
            hex::encode(h.finalize())
        };
        assert_eq!(combined_digest(&[("to", "a")]), expected);
    }

    /// Partition-binding (DESIGN finding #4): with the SAME arg names, a
    /// boundary-shift literal pair yields a DIFFERENT digest — plain
    /// concatenation would collapse these to the same hash.
    #[test]
    fn combined_digest_partition_binding_boundary_shift_differs() {
        let a = combined_digest(&[("to", "mallory@evil.co"), ("body", "m sent...")]);
        let b = combined_digest(&[("to", "mallory@evil.com"), ("body", " sent...")]);
        assert_ne!(
            a, b,
            "a to/body boundary shift MUST change the combined digest \
             (fixed-width per-element hashing removes partition-blindness)"
        );
    }

    /// Name-binding (DESIGN Round-6 rename bypass): renaming an arg changes
    /// the digest even with an IDENTICAL literal.
    #[test]
    fn combined_digest_name_binding_rename_differs() {
        let body_digest = combined_digest(&[("body", "x")]);
        let cc_digest = combined_digest(&[("cc", "x")]);
        assert_ne!(
            body_digest, cc_digest,
            "renaming a bound arg (body -> cc) with an identical literal MUST \
             change the combined digest (closes the Round-6 rename bypass)"
        );
    }

    /// Canonical order is INTRINSIC to the primitive: it sorts by byte-wise
    /// ascending arg_name internally, so INPUT order does not matter.
    #[test]
    fn combined_digest_input_order_invariant() {
        let forward = combined_digest(&[("to", "a@example.com"), ("body", "hello")]);
        let reversed = combined_digest(&[("body", "hello"), ("to", "a@example.com")]);
        assert_eq!(
            forward, reversed,
            "combined_digest must be invariant to input order — it sorts \
             internally by byte-wise ascending arg_name"
        );
    }

    /// Transposing which literal a name binds to DOES change the digest
    /// (distinct from mere input reordering above).
    #[test]
    fn combined_digest_transposed_literals_differs() {
        let original = combined_digest(&[("to", "a@example.com"), ("body", "hello")]);
        let transposed = combined_digest(&[("to", "hello"), ("body", "a@example.com")]);
        assert_ne!(
            original, transposed,
            "swapping which literal a name binds to MUST change the digest"
        );
    }

    /// Duplicate arg names are a FAIL-CLOSED error (uniqueness asserted
    /// before hashing) — `combined_digest` MUST NOT silently produce a
    /// digest over an ambiguously-ordered duplicate-name set.
    #[test]
    #[should_panic(expected = "duplicate arg_name")]
    fn combined_digest_duplicate_arg_name_panics() {
        combined_digest(&[("to", "a@example.com"), ("to", "b@example.com")]);
    }

    fn make_pending_confirmation(effect_id: Uuid) -> PendingConfirmation {
        let resolved_args = vec![
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
        ];
        let combined_digest = combined_digest(
            &resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        PendingConfirmation {
            effect_id,
            session_id: Uuid::new_v4(),
            blocked_event_id: Uuid::new_v4(),
            sink: SinkId("file.create".to_string()),
            resolved_args,
            blocked_arg_names: vec!["path".to_string()],
            combined_digest,
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

    // ── T-14-08 regression proof (Plan 16-02, Task 1, COMMIT 1) ─────────────
    //
    // Builds a GENUINE 2-blocked-arg email.send block (a tainted, routing-
    // sensitive `to` AND a tainted, content-sensitive `body`, per
    // `executor::sink_sensitivity`'s EMAIL_SEND_ROUTING_SENSITIVE /
    // EMAIL_SEND_CONTENT_SENSITIVE membership) — proving the CURRENT
    // `assert!(blocked_count <= 1)` plurality guard in `render_block_display`
    // actually panics against a real fixture, BEFORE COMMIT 2 replaces that
    // guard with full ALL-args narration (DESIGN "Block Narration for Every
    // Arg", coordinator's T-14-08 two-commit discipline).

    /// A genuine 2-blocked-arg email.send `PendingConfirmation`: `to` (routing-
    /// sensitive, untrusted) and `body` (content-sensitive, untrusted) are
    /// BOTH blocked; `subject` is trusted. `blocked_arg_names` is the ordered
    /// byte-wise-ascending subset `["body", "to"]` (Round-6 display-marking
    /// metadata), and `combined_digest` is computed via the SAME shared
    /// primitive over the FULL resolved_args set, matching a real Block-time
    /// write.
    fn make_two_blocked_email_send_pending_confirmation() -> PendingConfirmation {
        let resolved_args = vec![
            ResolvedArg {
                name: "to".to_string(),
                value_id: ValueId::new(),
                literal: "mallory@evil.example".to_string(),
                taint: vec![TaintLabel::ExternalUntrusted],
                provenance_chain: vec![Uuid::new_v4()],
            },
            ResolvedArg {
                name: "subject".to_string(),
                value_id: ValueId::new(),
                literal: "hello".to_string(),
                taint: vec![TaintLabel::UserTrusted],
                provenance_chain: vec![],
            },
            ResolvedArg {
                name: "body".to_string(),
                value_id: ValueId::new(),
                literal: "attacker-controlled body text".to_string(),
                taint: vec![TaintLabel::ExternalUntrusted],
                provenance_chain: vec![Uuid::new_v4(), Uuid::new_v4()],
            },
        ];
        let digest = combined_digest(
            &resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        PendingConfirmation {
            effect_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            blocked_event_id: Uuid::new_v4(),
            sink: SinkId("email.send".to_string()),
            resolved_args,
            blocked_arg_names: vec!["body".to_string(), "to".to_string()],
            combined_digest: digest,
            workspace_root_path: "/unused-for-render".to_string(),
            state: PendingConfirmationState::Pending,
        }
    }

    /// T-14-08 COMMIT 2: the plurality guard is GONE — the SAME genuine
    /// 2-blocked-arg fixture COMMIT 1 proved panics the un-modified guard now
    /// renders successfully, narrating ALL THREE args (to/subject/body), each
    /// marked BLOCKED or trusted, in byte-wise ascending arg_name order
    /// (body < subject < to) — the SAME canonical order `combined_digest`
    /// binds — plus the Draft/untrusted-seeded posture statement.
    #[test]
    fn render_block_display_narrates_all_args_marked_blocked_or_trusted() {
        let pc = make_two_blocked_email_send_pending_confirmation();
        let output = render_block_display(&pc);

        // Every arg name + its verbatim literal appears.
        assert!(output.contains("Arg:                body [BLOCKED]"), "{output}");
        assert!(output.contains("Arg:                subject [trusted]"), "{output}");
        assert!(output.contains("Arg:                to [BLOCKED]"), "{output}");
        assert!(output.contains("\"attacker-controlled body text\""), "{output}");
        assert!(output.contains("\"hello\""), "{output}");
        assert!(output.contains("\"mallory@evil.example\""), "{output}");

        // Byte-wise ascending arg_name order: body < subject < to.
        let body_pos = output.find("Arg:                body [BLOCKED]").unwrap();
        let subject_pos = output.find("Arg:                subject [trusted]").unwrap();
        let to_pos = output.find("Arg:                to [BLOCKED]").unwrap();
        assert!(
            body_pos < subject_pos && subject_pos < to_pos,
            "narration order must match combined_digest's byte-wise-ascending \
             arg_name order (body < subject < to); got:\n{output}"
        );

        // Draft/untrusted-seeded posture + irreversible-send statement (D-20).
        assert!(output.contains("Draft"), "{output}");
        assert!(output.to_lowercase().contains("untrusted-seeded"), "{output}");
        assert!(output.to_lowercase().contains("irreversible"), "{output}");

        // No hardcoded "file_read" source mislabel (SOURCE-LABEL finding).
        assert!(
            !output.contains("file_read"),
            "render_block_display must no longer hardcode the file_read source \
             label; got:\n{output}"
        );
    }

    /// The single-blocked-arg case (existing file.create seed) still renders
    /// correctly: its one blocked arg marked BLOCKED, its trusted `contents`
    /// arg marked trusted.
    #[test]
    fn render_block_display_single_blocked_arg_still_renders_correctly() {
        let pc = make_pending_confirmation(Uuid::new_v4());
        let output = render_block_display(&pc);

        assert!(output.contains("Arg:                path [BLOCKED]"), "{output}");
        assert!(output.contains("Arg:                contents [trusted]"), "{output}");
        assert!(output.contains("\"/workspace/out.txt\""), "{output}");
        assert!(output.contains("\"hello world\""), "{output}");
    }

    /// No truncation/elision of any literal, even a long body (DESIGN
    /// "Verbatim Display — No Truncation").
    #[test]
    fn render_block_display_does_not_truncate_a_long_literal() {
        let long_body = "x".repeat(5000);
        let resolved_args = vec![
            ResolvedArg {
                name: "to".to_string(),
                value_id: ValueId::new(),
                literal: "recipient@example.com".to_string(),
                taint: vec![TaintLabel::ExternalUntrusted],
                provenance_chain: vec![Uuid::new_v4()],
            },
            ResolvedArg {
                name: "body".to_string(),
                value_id: ValueId::new(),
                literal: long_body.clone(),
                taint: vec![TaintLabel::ExternalUntrusted],
                provenance_chain: vec![Uuid::new_v4()],
            },
        ];
        let digest = combined_digest(
            &resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        let pc = PendingConfirmation {
            effect_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            blocked_event_id: Uuid::new_v4(),
            sink: SinkId("email.send".to_string()),
            resolved_args,
            blocked_arg_names: vec!["body".to_string(), "to".to_string()],
            combined_digest: digest,
            workspace_root_path: "/unused".to_string(),
            state: PendingConfirmationState::Pending,
        };

        let output = render_block_display(&pc);
        assert!(
            output.contains(&format!("\"{long_body}\"")),
            "the full 5000-byte body literal must appear verbatim, with no \
             truncation or elision"
        );
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

        let resolved_args = vec![
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
        ];
        // CONFIRM-03 (Round-6): computed once over the FULL resolved_args
        // set, threaded into BOTH the sink_blocked Event and the
        // PendingConfirmation below — mirrors server.rs's Block-time write.
        let digest = combined_digest(
            &resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        let blocked_arg_names = vec!["path".to_string()];

        let blocked_event = Event::sink_blocked(
            Uuid::new_v4(),
            Some(root.id),
            session_id,
            chrono::Utc::now(),
            vec![anchor],
            Some(digest.clone()),
            blocked_arg_names.clone(),
        );
        let blocked_event_id = blocked_event.id;
        append_event(conn, &blocked_event, Some(&root_hash)).unwrap();
        insert_blocked_literal(conn, &blocked_event_id.to_string(), "path", path).unwrap();

        let pc = PendingConfirmation {
            effect_id,
            session_id,
            blocked_event_id,
            sink: SinkId("file.create".into()),
            resolved_args,
            blocked_arg_names,
            combined_digest: digest,
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

    // ── `caprun review` — read-only pre-decision surface (Task 1, MAJOR-8) ──

    /// `review` on a Pending block prints the narration, returns `Reviewed`,
    /// and — crucially — does NOT transition state or append any event.
    /// Running it twice leaves state Pending both times (idempotent, no side
    /// effects).
    #[test]
    fn review_prints_narration_without_mutating_state_or_appending_event() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_review_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();

        let conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, _blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        let before_events = query_events_by_session(&conn, &session_id.to_string())
            .unwrap()
            .len();

        let outcome = review(&conn, &effect_id.to_string()).expect("review");
        assert_eq!(outcome, ConfirmOutcome::Reviewed);

        let pc = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(
            pc.state,
            PendingConfirmationState::Pending,
            "review must never transition state"
        );

        let after_events = query_events_by_session(&conn, &session_id.to_string())
            .unwrap()
            .len();
        assert_eq!(
            before_events, after_events,
            "review must never append any event"
        );

        // Running it again: still Pending, still no new event, still Reviewed.
        let outcome2 = review(&conn, &effect_id.to_string()).expect("review again");
        assert_eq!(outcome2, ConfirmOutcome::Reviewed);
        let pc2 = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(pc2.state, PendingConfirmationState::Pending);
        let after_events2 = query_events_by_session(&conn, &session_id.to_string())
            .unwrap()
            .len();
        assert_eq!(before_events, after_events2);

        std::fs::remove_dir_all(&root).ok();
    }

    /// `review` on an unknown effect_id returns `UnknownEffect` (T-10-03), the
    /// same fail-closed contract `confirm`/`deny` already have.
    #[test]
    fn review_on_unknown_effect_id_returns_unknown_effect() {
        let conn = open_audit_db(":memory:").unwrap();
        let outcome = review(&conn, &Uuid::new_v4().to_string()).unwrap();
        assert_eq!(outcome, ConfirmOutcome::UnknownEffect);
    }

    // ── chain-verify + FULL-set recompute-and-compare gate (Task 2, BLOCKER-2/MAJOR-3/6/7) ──

    /// Directly mutate a `pending_confirmations` row's `resolved_args` JSON —
    /// simulating an actor with `pending_confirmations` write access tampering
    /// ONE arg's literal, WITHOUT updating `combined_digest` — the exact
    /// side-table write BLOCKER-2's recompute-and-compare gate must catch.
    fn mutate_resolved_arg_literal(
        conn: &rusqlite::Connection,
        effect_id: Uuid,
        arg_name: &str,
        new_literal: &str,
    ) {
        let mut pc = find_pending_confirmation(conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        for arg in pc.resolved_args.iter_mut() {
            if arg.name == arg_name {
                arg.literal = new_literal.to_string();
            }
        }
        let json = serde_json::to_string(&pc.resolved_args).unwrap();
        conn.execute(
            "UPDATE pending_confirmations SET resolved_args = ?1 WHERE effect_id = ?2",
            rusqlite::params![json, effect_id.to_string()],
        )
        .unwrap();
    }

    /// Directly RENAME a `resolved_args` element (same literal, different
    /// name) — the Round-6 name-binding proof: the digest binds
    /// sha256(arg_name)‖sha256(literal), so a rename changes the recompute
    /// even with an unchanged literal set.
    fn rename_resolved_arg(
        conn: &rusqlite::Connection,
        effect_id: Uuid,
        old_name: &str,
        new_name: &str,
    ) {
        let mut pc = find_pending_confirmation(conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        for arg in pc.resolved_args.iter_mut() {
            if arg.name == old_name {
                arg.name = new_name.to_string();
            }
        }
        let json = serde_json::to_string(&pc.resolved_args).unwrap();
        conn.execute(
            "UPDATE pending_confirmations SET resolved_args = ?1 WHERE effect_id = ?2",
            rusqlite::params![json, effect_id.to_string()],
        )
        .unwrap();
    }

    /// Directly patch the hash-chained `sink_blocked` Event's `payload`
    /// column to carry a NEW `combined_digest` — WITHOUT recomputing the
    /// row's `hash` column. This desyncs `payload` from `hash`: a bare
    /// deserialize-and-compare (no `verify_chain`) would see a
    /// self-consistent digest, but `verify_chain`'s hash recompute catches
    /// the desync (MAJOR-6's honestly-scoped "single-store tampering"
    /// detection).
    fn tamper_event_payload_digest_inconsistently(
        conn: &rusqlite::Connection,
        blocked_event_id: Uuid,
        new_digest: &str,
    ) {
        let payload: String = conn
            .query_row(
                "SELECT payload FROM events WHERE id = ?1",
                rusqlite::params![blocked_event_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        let mut event: runtime_core::Event = serde_json::from_str(&payload).unwrap();
        event.combined_digest = Some(new_digest.to_string());
        let new_payload = serde_json::to_string(&event).unwrap();
        conn.execute(
            "UPDATE events SET payload = ?1 WHERE id = ?2",
            rusqlite::params![new_payload, blocked_event_id.to_string()],
        )
        .unwrap();
    }

    /// (b) BLOCKER-2: a TAMPERED TRUSTED arg (`contents`, not the blocked
    /// `path`) is caught. Before Round-6 this class of tamper was invisible
    /// to a blocked-subset-only digest; confirm() must fail closed
    /// (DigestMismatch), invoke NO sink, and append a durable
    /// `confirm_digest_mismatch` event.
    #[test]
    fn confirm_fails_closed_with_digest_mismatch_when_trusted_arg_tampered() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_tamper_trusted_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, _blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        mutate_resolved_arg_literal(&conn, effect_id, "contents", "attacker-injected contents");

        let outcome = confirm(&mut conn, &effect_id.to_string(), &ws).expect("confirm");
        assert_eq!(outcome, ConfirmOutcome::DigestMismatch);
        assert!(
            !root.join("out.txt").exists(),
            "no sink must ever run when a trusted arg's literal was tampered (BLOCKER-2)"
        );
        assert_eq!(
            count_events_of_type(&conn, session_id, "confirm_digest_mismatch"),
            1,
            "a durable confirm_digest_mismatch event must exist"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// (b') Round-6 name-binding proof: renaming a resolved_arg post-Block
    /// (`body` -> `cc`) with an OTHERWISE UNCHANGED literal set is caught —
    /// the digest binds the name, not just the literal.
    #[test]
    fn confirm_fails_closed_with_digest_mismatch_when_arg_renamed_post_block() {
        let mut conn = open_audit_db(":memory:").unwrap();
        let root = std::env::temp_dir().join(format!("caprun_confirm_rename_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let (effect_id, session_id, _blocked_event_id) =
            seed_pending_email_send_block(&conn, "recipient@example.com", "hello", "body text");

        rename_resolved_arg(&conn, effect_id, "body", "cc");

        let outcome = confirm(&mut conn, &effect_id.to_string(), &ws).expect("confirm");
        assert_eq!(outcome, ConfirmOutcome::DigestMismatch);
        assert_eq!(
            count_events_of_type(&conn, session_id, "email_send_attempted"),
            0,
            "no sink dispatch may occur when an arg was renamed post-Block (Round-6 rename bypass)"
        );
        assert_eq!(
            count_events_of_type(&conn, session_id, "confirm_digest_mismatch"),
            1
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// (c) A self-consistent literal+Event-digest edit (both changed
    /// together, so a BARE compare would pass) is caught by `verify_chain`
    /// instead — proving the chain-verify step is doing REAL work, distinct
    /// from the plain digest-compare tests above.
    #[test]
    fn confirm_fails_closed_via_verify_chain_when_literal_and_event_digest_edited_self_consistently()
    {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_chain_tamper_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        // Mutate the trusted `contents` literal AND recompute a matching
        // digest, persisting the NEW digest into BOTH the PendingConfirmation
        // row AND the sink_blocked Event's payload — self-consistent from a
        // bare-compare view, but the Event payload edit desyncs `payload`
        // from the row's `hash` column.
        let mut pc = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        for arg in pc.resolved_args.iter_mut() {
            if arg.name == "contents" {
                arg.literal = "attacker contents".to_string();
            }
        }
        let new_digest = combined_digest(
            &pc.resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        let resolved_args_json = serde_json::to_string(&pc.resolved_args).unwrap();
        conn.execute(
            "UPDATE pending_confirmations SET resolved_args = ?1, combined_digest = ?2 \
             WHERE effect_id = ?3",
            rusqlite::params![resolved_args_json, new_digest, effect_id.to_string()],
        )
        .unwrap();
        tamper_event_payload_digest_inconsistently(&conn, blocked_event_id, &new_digest);

        // Sanity: this edit genuinely breaks verify_chain — proving this test
        // exercises the chain-hash path, not the plain-compare path.
        assert!(
            !crate::audit::verify_chain(&conn, &session_id.to_string()),
            "sanity: the Event payload edit must break verify_chain"
        );

        let outcome = confirm(&mut conn, &effect_id.to_string(), &ws).expect("confirm");
        assert_eq!(outcome, ConfirmOutcome::DigestMismatch);
        assert!(!root.join("out.txt").exists());

        std::fs::remove_dir_all(&root).ok();
    }

    /// (d) MAJOR-7 no-fork: a DigestMismatch (row left Pending, an alarm not
    /// a deny) followed by a SECOND confirm attempt — this time with the
    /// tamper reverted, so it succeeds — leaves `audit::verify_chain` STILL
    /// TRUE throughout. The mismatch event chained onto the CURRENT HEAD
    /// (not `blocked_event_id`), so the retry never forks the DAG.
    #[test]
    fn digest_mismatch_then_retry_does_not_fork_dag_verify_chain_stays_true() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_mismatch_retry_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, _blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        mutate_resolved_arg_literal(&conn, effect_id, "contents", "attacker-injected");

        let first = confirm(&mut conn, &effect_id.to_string(), &ws).expect("first confirm");
        assert_eq!(first, ConfirmOutcome::DigestMismatch);
        assert!(
            crate::audit::verify_chain(&conn, &session_id.to_string()),
            "the chain must remain linear (verify_chain true) immediately after the mismatch append"
        );

        // The row is left Pending (an alarm, not a deny) — revert the tamper
        // and retry; the recompute now matches the ORIGINAL digest again.
        mutate_resolved_arg_literal(&conn, effect_id, "contents", "hello");
        let second = confirm(&mut conn, &effect_id.to_string(), &ws).expect("second confirm");
        assert_eq!(second, ConfirmOutcome::Released);
        assert!(
            crate::audit::verify_chain(&conn, &session_id.to_string()),
            "verify_chain must STILL be true after mismatch -> retry (MAJOR-7 no-fork)"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    // (e) deny is unaffected: `deny_on_pending_block_is_durable` (above)
    // already asserts confirm_denied.parent_id == Some(blocked_event_id) in
    // the single-shot case, which current_chain_head_or_bail preserves
    // exactly (head == blocked_event_id when nothing has been appended since
    // the Block).

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

        let resolved_args = vec![
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
        ];
        // CONFIRM-03 (Round-6): computed once over the FULL resolved_args
        // set, threaded into BOTH the sink_blocked Event and the
        // PendingConfirmation below — mirrors server.rs's Block-time write.
        let digest = combined_digest(
            &resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        let blocked_arg_names = vec!["to".to_string()];

        let blocked_event = Event::sink_blocked(
            Uuid::new_v4(),
            Some(root.id),
            session_id,
            chrono::Utc::now(),
            vec![anchor],
            Some(digest.clone()),
            blocked_arg_names.clone(),
        );
        let blocked_event_id = blocked_event.id;
        append_event(conn, &blocked_event, Some(&root_hash)).unwrap();
        insert_blocked_literal(conn, &blocked_event_id.to_string(), "to", to).unwrap();

        let pc = PendingConfirmation {
            effect_id,
            session_id,
            blocked_event_id,
            sink: SinkId("email.send".into()),
            resolved_args,
            blocked_arg_names,
            combined_digest: digest,
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
