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
use hmac::{Hmac, Mac};
use rusqlite::params;
use runtime_core::plan_node::TaintLabel;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// HMAC-SHA256 type for `pending_confirmations`'s whole-row MAC (v1.6 Phase
/// 28 Plan 05, HARDEN-02 / X-02) — a local alias mirroring `audit.rs`'s
/// `HmacSha256`, since that alias is private to `audit.rs`; the underlying
/// `Hmac<Sha256>` type is fully public, so this is structurally the SAME
/// primitive, not a divergent one.
type PendingConfHmac = Hmac<Sha256>;

/// Domain tag for `pending_confirmations`'s whole-row MAC — reserved in
/// Plan 03's `audit.rs` doc comments, consumed here. MUST stay distinct from
/// `audit.rs`'s `EVENT_MAC_DOMAIN`/`ANCHOR_MAC_DOMAIN` so a MAC computed for
/// one record type can never verify — or be replayed — as another's, per
/// `audit::mac_frame`'s domain-separation discipline.
const PENDING_CONFIRMATION_MAC_DOMAIN: &[u8] = b"caprun.audit.pending-confirmation.v1";

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
    /// The git.push human-confirmed frozen new-oid (v1.9 Phase 44 Plan 04,
    /// GIT-02/03, WG-7, DESIGN-v1.9-egress-policy §1.6) — the anti-TOCTOU
    /// PAYLOAD freeze: the specific commit oid the human authorized, snapshotted
    /// at insert time and covered by the whole-row `mac` below (a tampered oid
    /// fails verification). The Step-7 dispatch hands it to
    /// `invoke_git_push_from_resolved`, which refuses if the live rev-parse
    /// diverges. EMPTY (`String::new()`) for every non-git.push sink — they
    /// carry no frozen payload oid. Frozen at Block/gate time, never re-derived.
    pub frozen_new_oid: String,
    /// `Pending | Confirmed | Denied`. MUST start `Pending` at persistence time.
    pub state: PendingConfirmationState,
    /// The row's WHOLE-ROW broker-key MAC (v1.6 Phase 28 Plan 05, HARDEN-02 /
    /// X-02), hex-encoded — persisted alongside `state`, recomputed
    /// atomically with every `transition_state` call. `find_pending_
    /// confirmation` populates this from the `mac` column; a freshly
    /// constructed row (not yet inserted) may leave this as an empty
    /// placeholder — `insert_pending_confirmation` computes and stores the
    /// REAL value independently of whatever this field holds at call time.
    pub mac: String,
}

/// Build (but do not finalize) the keyed MAC over a `pending_confirmations`
/// row's WHOLE-ROW field set UNDER A GIVEN `state` (v1.6 Phase 28 Plan 05,
/// HARDEN-02 / X-02) — the `state` parameter is explicit and separate from
/// `pc.state` because `transition_state` needs to compute the MAC for the
/// NEW state, not the state the row still holds at read time.
///
/// DECISION (pinned, stated explicitly per this plan's SUMMARY): MACs the
/// WHOLE row — effect_id, session_id, blocked_event_id, sink, resolved_args,
/// blocked_arg_names, combined_digest, workspace_root_path, state — not only
/// `state`+`combined_digest` as the DESIGN doc's literal text names
/// (28-RESEARCH.md Assumption A3 / Open Question 2): `resolved_args`/
/// `blocked_arg_names`/`workspace_root_path` are equally forgeable by a bare
/// `pending_confirmations`-table writer and equally load-bearing for
/// `confirm()`'s Step 4.5b recompute-and-compare.
///
/// Uses the SHARED `audit::mac_frame` helper (domain-separated + length-
/// framed — never a bare per-field concatenation) with the reserved
/// `PENDING_CONFIRMATION_MAC_DOMAIN` tag, distinct from `audit.rs`'s event
/// and anchor domains.
fn build_pending_confirmation_mac(
    key: &[u8],
    pc: &PendingConfirmation,
    state: PendingConfirmationState,
) -> Result<PendingConfHmac> {
    let resolved_args_json = serde_json::to_string(&pc.resolved_args)?;
    let blocked_arg_names_json = serde_json::to_string(&pc.blocked_arg_names)?;
    let effect_id = pc.effect_id.to_string();
    let session_id = pc.session_id.to_string();
    let blocked_event_id = pc.blocked_event_id.to_string();
    let mut mac = <PendingConfHmac as Mac>::new_from_slice(key)
        .expect("HMAC-SHA256 accepts a key of any length");
    crate::audit::mac_frame(
        &mut mac,
        PENDING_CONFIRMATION_MAC_DOMAIN,
        &[
            effect_id.as_bytes(),
            session_id.as_bytes(),
            blocked_event_id.as_bytes(),
            pc.sink.0.as_bytes(),
            resolved_args_json.as_bytes(),
            blocked_arg_names_json.as_bytes(),
            pc.combined_digest.as_bytes(),
            pc.workspace_root_path.as_bytes(),
            state.as_str().as_bytes(),
            // v1.9 Phase 44 Plan 04 (GIT-02/03, WG-7): the git.push frozen
            // new-oid rides the SAME whole-row MAC integrity boundary as every
            // other snapshot field — appended as the LAST length-framed element
            // so the pre-Phase-44 element ordering is unchanged. A tampered oid
            // (or a forged git.push pending row) fails `verify_pending_
            // confirmation_mac`.
            pc.frozen_new_oid.as_bytes(),
        ],
    );
    Ok(mac)
}

/// Compute a `pending_confirmations` row's whole-row MAC (hex-encoded) under
/// `state` — the write-side half of `build_pending_confirmation_mac`, used by
/// `insert_pending_confirmation` and `transition_state` to STORE the MAC.
fn compute_pending_confirmation_mac_for_state(
    key: &[u8],
    pc: &PendingConfirmation,
    state: PendingConfirmationState,
) -> Result<String> {
    let mac = build_pending_confirmation_mac(key, pc, state)?;
    Ok(hex::encode(mac.finalize().into_bytes()))
}

/// Constant-time whole-row MAC verification for a `pending_confirmations`
/// row (v1.6 Phase 28 Plan 05, HARDEN-02 / X-02) — mirrors
/// `audit::verify_event_hash`'s / `verify_anchor_mac`'s fail-closed contract:
/// never panics, always `Mac::verify_slice` (never a plain string compare on
/// the MAC value, which would leak timing information about a secret-
/// dependent comparison).
///
/// Verifies `pc` EXACTLY as currently fetched — including `pc.mac` (the
/// persisted value) and `pc.state` (the persisted state at read time).
/// Called by `confirm()`/`deny()` immediately after `find_pending_
/// confirmation` returns, BEFORE the terminal-state branch reads `pc.state`
/// for any decision (28-RESEARCH.md Pitfall 5) — a raw-SQL flip-back that
/// `transition_state`'s own `WHERE state = 'pending'` guard would not catch
/// is detected here and fails closed.
///
/// Returns `false` (never panics) on a hex-decode failure of `pc.mac`
/// (malformed/corrupt/legacy-default-empty MAC) or a MAC mismatch under
/// `key` (tampering, wrong key, or an unMAC'd legacy row).
pub fn verify_pending_confirmation_mac(key: &[u8], pc: &PendingConfirmation) -> bool {
    let expected_bytes = match hex::decode(&pc.mac) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let mac = match build_pending_confirmation_mac(key, pc, pc.state) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac.verify_slice(&expected_bytes).is_ok()
}

/// Persist a new `PendingConfirmation` row.
///
/// One `INSERT` binding all ten columns (nine data columns plus the v1.6
/// Phase 28 Plan 05 whole-row `mac`). Serializes `resolved_args` and
/// `blocked_arg_names` with `serde_json::to_string`, `sink` as `pc.sink.0`,
/// uuids via `.to_string()`, state via `as_str()`. The MAC is computed HERE,
/// under `key`, over `pc`'s fields AND `pc.state` — `pc.mac` (whatever the
/// caller happened to set it to, if anything) is IGNORED; this function is
/// the sole source of truth for a freshly-inserted row's MAC. Caller should
/// invoke this under the same broker-owned connection lock as the
/// `append_event` that wrote the anchoring `sink_blocked` row (the two
/// writes MUST succeed or fail together).
pub fn insert_pending_confirmation(
    conn: &rusqlite::Connection,
    key: &[u8],
    pc: &PendingConfirmation,
) -> Result<()> {
    let resolved_args_json = serde_json::to_string(&pc.resolved_args)?;
    let blocked_arg_names_json = serde_json::to_string(&pc.blocked_arg_names)?;
    let mac = compute_pending_confirmation_mac_for_state(key, pc, pc.state)?;
    conn.execute(
        "INSERT INTO pending_confirmations \
         (effect_id, session_id, blocked_event_id, sink, resolved_args, \
          blocked_arg_names, combined_digest, workspace_root_path, state, mac, \
          frozen_new_oid) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
            &mac,
            &pc.frozen_new_oid,
        ],
    )?;
    Ok(())
}

/// Fetch a `PendingConfirmation` row by its `effect_id` (indexed PRIMARY KEY
/// lookup), or `None` if no row was ever persisted for that id — the fail-closed
/// case for an untrusted/forged CLI-supplied `effect_id` (T-10-07).
///
/// Populates `PendingConfirmation.mac` from the persisted `mac` column but
/// does NOT itself verify it — callers (`confirm()`/`deny()`) call
/// `verify_pending_confirmation_mac` immediately after this returns, BEFORE
/// reading `pc.state` for any decision (Pitfall 5).
pub fn find_pending_confirmation(
    conn: &rusqlite::Connection,
    effect_id: &str,
) -> Result<Option<PendingConfirmation>> {
    let mut stmt = conn.prepare(
        "SELECT effect_id, session_id, blocked_event_id, sink, resolved_args, \
                blocked_arg_names, combined_digest, workspace_root_path, state, mac, \
                frozen_new_oid \
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
            let mac: String = row.get(9)?;
            let frozen_new_oid: String = row.get(10)?;

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
                frozen_new_oid,
                state: PendingConfirmationState::from_str(&state)?,
                mac,
            }))
        }
        None => Ok(None),
    }
}

/// Transition a `pending_confirmations` row's `state`, returning the number of
/// affected rows.
///
/// A single `UPDATE ... SET state = ?1, mac = ?2 WHERE effect_id = ?3 AND
/// state = 'pending'`. The `AND state = 'pending'` guard is the CONFIRM-03
/// fail-closed terminal check IN THE SQL: a row already `confirmed`/`denied`
/// matches zero rows, so a re-transition is refused atomically with no
/// read-then-write race. Callers treat a `0` return as "already terminal /
/// refused".
///
/// `mac` is recomputed for `new_state` (v1.6 Phase 28 Plan 05, HARDEN-02 /
/// X-02) and rewritten in the SAME `UPDATE` — never a separate statement —
/// so `state` and `mac` can never observably desync. `pc` MUST be the row as
/// freshly fetched by the caller (its OTHER fields — resolved_args,
/// blocked_arg_names, combined_digest, workspace_root_path — are assumed
/// unchanged since that fetch and are what the new MAC binds alongside
/// `new_state`).
pub fn transition_state(
    conn: &rusqlite::Connection,
    key: &[u8],
    pc: &PendingConfirmation,
    new_state: PendingConfirmationState,
) -> Result<usize> {
    let new_mac = compute_pending_confirmation_mac_for_state(key, pc, new_state)?;
    let affected = conn.execute(
        "UPDATE pending_confirmations SET state = ?1, mac = ?2 WHERE effect_id = ?3 AND state = 'pending'",
        params![new_state.as_str(), new_mac, pc.effect_id.to_string()],
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
    /// An integrity gate failed: the `pending_confirmations` row's own
    /// whole-row MAC did not verify (v1.6 Phase 28 Plan 05, checked FIRST,
    /// before any `pc.state` read), OR the audit chain was broken
    /// (`audit::verify_chain` returned `false`), OR — `confirm()` only — the
    /// FULL-set `combined_digest` recomputed from the frozen `resolved_args`
    /// snapshot did NOT match the hash-chained `sink_blocked` Event's stored
    /// `combined_digest` (or that Event/field was missing). Both `confirm()`
    /// and `deny()` refuse to proceed on any of these (BLOCKER-2, MAJOR-3,
    /// X-02).
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
    /// HONESTY (MAJOR-6, UPDATED v1.6 Phase 28 Plans 03-05): `audit::
    /// verify_chain` recomputes a KEYED HMAC-SHA256 MAC under the
    /// broker-owned secret key (`cli/caprun/src/key.rs::load_or_create_key`)
    /// AND cross-checks the MAC'd `chain_anchor` head/count (Plan 04) — an
    /// actor with bare `events`-table write access (no key) can no longer
    /// forge a self-consistent chain OR silently truncate the tail. `pending_
    /// confirmations` is now ALSO folded into the same broker-key MAC scheme
    /// (Plan 05, X-02 uniform ruling): a flip-back/delete on THAT table is
    /// detected too, in BOTH `confirm()` and `deny()`. See
    /// `.planning/todos/pending` for anything past this phase's scope.
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
        TaintLabel::ExecRaw => "exec.raw",
        TaintLabel::HttpRaw => "http.raw",
    }
}

/// Compact, display-only rendering of a Uuid's first hyphen-delimited segment
/// (mirrors `cli/caprun/src/main.rs`'s `&hash[..8]` audit-DAG print convention).
/// Never used for identity comparison — only for the human-facing block display.
fn short_evt(id: &Uuid) -> String {
    format!("evt_{}", &id.to_string()[..8])
}

/// Neutralize terminal control characters in an attacker-influenceable literal
/// BEFORE it is written to the confirm prompt (WG-8 / T-44-19, mirrors the U1 /
/// VIEW-01 viewer discipline): a tainted refspec / remote / filename could embed
/// ANSI escapes (ESC `0x1b`, the CSI sequence) or other C0/C1 control bytes to
/// SPOOF or HIDE audit lines in the human's terminal. Every `char::is_control()`
/// byte (C0 incl. ESC/CR/LF/TAB, the C1 range, and DEL) is replaced with a
/// visible `\xNN` / `\u{NNNN}` escape; ordinary printable text — including
/// non-ASCII UTF-8 — is preserved verbatim so the human still reads the real
/// value. Pure/deterministic; performs no I/O.
fn neutralize_control_chars(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_control() {
            let cp = c as u32;
            if cp <= 0xff {
                out.push_str(&format!("\\x{cp:02x}"));
            } else {
                out.push_str(&format!("\\u{{{cp:04x}}}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Render the git.push payload-provenance summary appended to the confirm prompt
/// (WG-8, DESIGN-v1.9-egress-policy §1.6, GIT-03). Empty string for any
/// non-git.push confirmation (so every other sink's block is UNAFFECTED).
///
/// The pushed commit RANGE is the LOCALLY-computed tip of the human-confirmed
/// `frozen_new_oid` — this renderer performs NO network read (the remote's
/// advertised old-oid is unknown until the dispatch-time info/refs GET, Plan
/// 44-03) and NO git read (it is a pure function over the frozen snapshot). The
/// per-arg provenance summary surfaces each routing arg (remote, refspec) — the
/// payload-destination values the human authorizes — flagging any whose content
/// derives from untrusted taint, with EVERY displayed literal control-char-
/// neutralized (T-44-19). Per §1.6 this SURFACES provenance for human judgment;
/// it does NOT itself Block, and it does NOT over-promise that the pushed delta
/// is byte-identical to this locally-computed range (the accepted residual).
fn render_git_push_payload_summary(pc: &PendingConfirmation) -> String {
    if pc.sink.0 != "git.push" {
        return String::new();
    }

    // Byte-wise-ascending arg order (the SAME canonical order the per-arg
    // narration + combined_digest use), so a human reading top-to-bottom sees a
    // stable ordering.
    let mut sorted_args: Vec<&ResolvedArg> = pc.resolved_args.iter().collect();
    sorted_args.sort_by(|a, b| a.name.cmp(&b.name));

    let mut per_arg = String::new();
    for arg in &sorted_args {
        let untrusted = arg.taint.iter().any(|t| t.is_untrusted());
        let flag = if untrusted { "[UNTRUSTED-DERIVED]" } else { "[trusted]" };
        let taint_str = arg
            .taint
            .iter()
            .map(taint_label_display)
            .collect::<Vec<_>>()
            .join(", ");
        per_arg.push_str(&format!(
            "  {name:<8} {flag}  value: \"{literal}\"  taint: [{taint_str}]\n",
            name = arg.name,
            literal = neutralize_control_chars(&arg.literal),
        ));
    }

    format!(
        "\n\
         Push payload provenance (git.push — WG-8 / DESIGN §1.6):\n\
         Pushed commit (frozen new-oid): {frozen}\n\
         {per_arg}\
         NOTE: the range shown is the LOCALLY-computed tip of the frozen new-oid.\n\
         The remote's advertised base is unknown until the dispatch-time info/refs\n\
         GET, so this SURFACES provenance for your judgment — it is NOT a promise\n\
         that the pushed delta is byte-identical to this exact range (DESIGN §1.6\n\
         accepted residual). Any arg marked untrusted-derived above carries\n\
         content derived from untrusted taint: confirming authorizes pushing to a\n\
         destination influenced by untrusted content. The pushed PACK is frozen to\n\
         this exact commit oid (anti-TOCTOU, WG-7).\n",
        frozen = neutralize_control_chars(&pc.frozen_new_oid),
    )
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

        // WG-8 / T-44-19 (git.push ONLY): a tainted remote/refspec literal could
        // embed ANSI/control bytes to spoof audit lines in the confirm prompt.
        // Control-char-neutralize the DISPLAYED literal for git.push, preserving
        // the byte-verbatim T-10-04 display for every other sink (their block is
        // unaffected). Neutralization escapes control bytes to visible `\xNN` —
        // it never truncates/elides, so the full value is still shown.
        let shown_literal = if pc.sink.0 == "git.push" {
            neutralize_control_chars(&arg.literal)
        } else {
            arg.literal.clone()
        };

        per_arg.push_str(&format!(
            "\n\
             Arg:                {name} [{marker}]\n\
             Literal value:      \"{literal}\"\n\
             Taint:              [{taint_str}]\n\
             Source:             {source_evt}  (session {session_id})\n\
             Provenance chain:   {chain_str}\n",
            name = arg.name,
            literal = shown_literal,
            session_id = pc.session_id,
        ));
    }

    // WG-8 (git.push only): append the payload-provenance summary — the frozen
    // commit oid + a control-char-neutralized per-routing-arg taint flag. Empty
    // for every other sink (their block is unaffected).
    let git_push_summary = render_git_push_payload_summary(pc);

    let effect_id = pc.effect_id;
    format!(
        "Effect blocked pending confirmation.\n\
         \n\
         Effect ID:         {effect_id}\n\
         Sink:               {sink}\n\
         {per_arg}\
         {git_push_summary}\n\
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
///
/// Deliberately unauthenticated (v1.6 Phase 28 Plan 05, MAJOR-8): `review`
/// never transitions state, appends an Event, or invokes a sink — it is a
/// pre-decision, non-authoritative display, so no MAC/chain-verify gate is
/// added here (plan's explicit scope: `confirm()`/`deny()` only).
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

/// Append a durable `confirm_digest_mismatch` integrity-alarm Event, parented
/// on the CURRENT CHAIN HEAD (never `blocked_event_id` — MAJOR-7): this is
/// what keeps a mismatch→retry sequence a LINEAR extension of the chain
/// rather than a fork of `blocked_event_id` (which would permanently break
/// `audit::verify_chain`'s single-linear-chain walk, exactly as
/// `quarantine.rs`'s `chain_head_id` doc comment describes empirically
/// discovering for `mint_from_read`).
///
/// `verb` (v1.6 Phase 28 Plan 05) is `"confirm"` or `"deny"` — used ONLY in
/// the event's `actor` field so the audit trail records which decision verb
/// tripped the alarm; both verbs share this SAME helper and event_type
/// (`confirm_digest_mismatch`) since it is one integrity-alarm mechanism,
/// not two.
fn append_digest_mismatch_event(
    conn: &rusqlite::Connection,
    key: &[u8],
    pc: &PendingConfirmation,
    effect_id: &str,
    head_id: Uuid,
    head_hash: &str,
    verb: &str,
) -> Result<()> {
    let mismatch_event = runtime_core::Event::new(
        Uuid::new_v4(),
        Some(head_id),
        pc.session_id,
        format!("{verb}:{effect_id}"),
        "confirm_digest_mismatch".into(),
        Utc::now(),
        vec![],
    );
    crate::audit::append_event(conn, key, &mismatch_event, Some(head_hash))?;
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
///
/// NOTE (Plan 34-02 / 34-03): the `"process.exec"` Step-7 arm below does NOT
/// mint the released output. Unlike the Allowed path (server.rs), which mints
/// into the live session `ValueStore` and returns the `ValueId` for downstream
/// plan nodes to consume, `confirm()` runs in a separate human-driven process
/// with no live `ValueStore` and no subsequent plan node in the same
/// invocation — a mint could only target a throwaway store, discarded
/// immediately (dead ceremony, removed in 34-03 adversarial review). The
/// genuine, non-stapled durable taint anchor is the `process_exited` Event
/// (`{ExternalUntrusted, ExecRaw}`) the sink appends, chained on
/// `confirm_granted`. `confirm()` still never constructs a re-decide
/// `ValueStore` and never calls `submit_plan_node`.
pub async fn confirm(
    conn: &mut rusqlite::Connection,
    key: &[u8],
    effect_id: &str,
    workspace_root: &adapter_fs::workspace::WorkspaceRoot,
) -> Result<ConfirmOutcome> {
    // Step 1: fresh, indexed lookup — fail closed on an unknown/forged id (T-10-03).
    let pc = match find_pending_confirmation(conn, effect_id)? {
        Some(pc) => pc,
        None => return Ok(ConfirmOutcome::UnknownEffect),
    };

    // Step 1.5 (v1.6 Phase 28 Plan 05, HARDEN-02 / X-02): pending_confirmations
    // whole-row MAC-verify IMMEDIATELY after fetch, BEFORE Step 2 reads
    // pc.state for any decision (Pitfall 5) — a raw-SQL flip-back (e.g.
    // Denied -> Pending) that transition_state's own WHERE-guard would not
    // catch is detected here and fails closed.
    if !verify_pending_confirmation_mac(key, &pc) {
        let (head_id, head_hash) = current_chain_head_or_bail(conn, pc.session_id)?;
        append_digest_mismatch_event(conn, key, &pc, effect_id, head_id, &head_hash, "confirm")?;
        return Ok(ConfirmOutcome::DigestMismatch);
    }

    // Step 2: terminal-state check, read from the persisted (now MAC-verified) row.
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
    // HONESTY (MAJOR-6, UPDATED v1.6 Phase 28 Plans 03-05): `verify_chain`
    // now recomputes a KEYED HMAC-SHA256 MAC under `key` (the broker-owned
    // secret) AND cross-checks the MAC'd `chain_anchor` head/count — a bare
    // `events`-table writer without the key can no longer forge a
    // self-consistent chain OR silently truncate the tail. `pending_
    // confirmations` itself is now ALSO folded into the same MAC scheme
    // (Step 1.5 above, this plan) — see `ConfirmOutcome::DigestMismatch`'s
    // doc comment for the full current scope.
    if !crate::audit::verify_chain(conn, &pc.session_id.to_string(), key) {
        let (head_id, head_hash) = current_chain_head_or_bail(conn, pc.session_id)?;
        append_digest_mismatch_event(conn, key, &pc, effect_id, head_id, &head_hash, "confirm")?;
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
        append_digest_mismatch_event(conn, key, &pc, effect_id, head_id, &head_hash, "confirm")?;
        return Ok(ConfirmOutcome::DigestMismatch);
    }

    // Step 4.75 (Phase 33 adversarial-review MAJOR-1 fix): entry guard —
    // refuse to proceed for any sink this dispatch cannot actually invoke,
    // BEFORE Step 5 appends confirm_granted or Step 6 transitions state.
    // Without this guard, `caprun confirm` on a blocked sink with no
    // dispatch arm would durably burn the one-shot confirmation
    // (`confirm_granted` appended, state -> Confirmed) with no write ever
    // performed and no terminal `sink_executed`/`sink_execution_failed`
    // event — an audit-DAG gap. This list MUST stay in sync with the sink
    // match arms in Step 7 below. Fail-closed-RECOVERABLE: the row is left
    // `Pending` (never transitioned), so a corrected `confirm()` call — once
    // the sink IS wired — can still succeed. `process.exec` confirm-release
    // dispatch is wired below (Plan 34-02, EXEC-05).
    // `github.pr` (v1.8 Phase 38, GITHUB-02/03, DESIGN §4.6/§9): a
    // confirm-releasable sink absent from this allow-list is denied at the
    // guard — the entry-guard extension is REQUIRED, not optional (§9 "a new
    // confirm-releasable sink that is NOT added here is denied at the guard").
    // `http.request.write` (v1.9 Phase 43, HTTP-W-01, DESIGN-v1.9-egress-policy
    // §2): a confirm-releasable WRITE egress — REQUIRED in this allow-list (a
    // confirm-releasable sink absent from it is denied at the guard), kept in
    // sync with its Step-7 dispatch arm below.
    // `git.push` (v1.9 Phase 44 Plan 04, GIT-02/03, DESIGN-v1.9-egress-policy
    // §1.6/§1.7): git.push is ALWAYS confirm-gated (there is NO auto-dispatch
    // Allowed arm) — a confirm-releasable sink that MUST be on this allow-list,
    // kept in sync with its Step-4.8d precheck + Step-7 dispatch arm below.
    match pc.sink.0.as_str() {
        "file.create" | "email.send" | "file.write" | "process.exec" | "github.pr"
        | "http.request.write" | "git.push" => {}
        other => {
            return Err(anyhow::anyhow!(
                "confirm: sink `{other}` has no confirm-release dispatch wired \
                 — refusing before confirm_granted/state transition (fail-closed, \
                 row remains Pending)"
            ));
        }
    }

    // Step 4.8 (34-03 adversarial-review MAJOR fix): `process.exec` has fallible
    // PRE-SPAWN steps (frozen `command` lookup, `args` JSON parse, launcher-path
    // resolution). Run them HERE — before Step 5 appends `confirm_granted` and
    // Step 6 CAS→Confirmed burn the one-shot confirmation. A failure returns
    // fail-closed-RECOVERABLE: the row stays `Pending` (the operator can `deny`
    // it, or re-`confirm` once a missing launcher is deployed), NEVER a burned
    // confirmation with a dangling `confirm_granted` and no terminal event (the
    // exact P33 MAJOR-1 audit-gap class, previously re-entered through the
    // sink's own `?` legs that propagated AFTER the burn). Uses the SAME
    // `prepare_process_exec` the sink calls in Step 7, so precheck and dispatch
    // can never validate differently.
    if pc.sink.0.as_str() == "process.exec" {
        crate::sinks::process_exec::prepare_process_exec(&pc.resolved_args).map_err(|e| {
            anyhow::anyhow!(
                "process.exec: pre-spawn validation failed before confirm_granted \
                 (fail-closed, row remains Pending): {e:#}"
            )
        })?;
    }

    // Step 4.8b (v1.8 Phase 38, GITHUB-02/03, DESIGN §4.3/§4.6/§9): `github.pr`
    // has TWO independent pre-effect gates that MUST both pass BEFORE Step 5
    // appends `confirm_granted` and Step 6 burns the one-shot — mirroring the
    // `process.exec` Step-4.8 precheck discipline (fail-closed-RECOVERABLE: the
    // row stays `Pending`, never a burned confirmation dangling without a
    // terminal `github_pr_*` event — the P33/P34 audit-gap class this phase
    // closes).
    //   (1) GRANT GATE (GITHUB-02): a bare human `confirm` is NOT sufficient to
    //       create a PR — the SECOND independent gate. Without a live
    //       session auth-grant the row stays Pending (fail-closed-RECOVERABLE:
    //       run `caprun grant <session>` then re-confirm).
    //   (2) PRECHECK: the SAME `prepare_github_pr` the Step-7 dispatch uses
    //       (present + non-empty + url/body constructible), so precheck and
    //       dispatch validate IDENTICALLY and cannot drift.
    // The content-derived idempotency key (GITHUB-04, §4.5) is ALSO derived
    // HERE, pre-burn: derivation is pure/read-only, so computing it before the
    // burn keeps the Step-7 CAS free of any post-burn fallible arg-lookup leg
    // (a missing arg here fails closed-RECOVERABLE, never a dangling burn). The
    // six args are guaranteed present + non-empty by the precheck immediately
    // above.
    let github_pr_content_key: Option<String> = if pc.sink.0.as_str() == "github.pr" {
        if !crate::audit::has_github_grant(conn, &pc.session_id.to_string()) {
            return Err(anyhow::anyhow!(
                "github.pr: no live session auth-grant (GITHUB-02) — a bare confirm \
                 cannot create a PR; refusing before confirm_granted/state transition \
                 (fail-closed-RECOVERABLE, row remains Pending; run `caprun grant` then \
                 re-confirm)"
            ));
        }
        crate::sinks::github_pr::prepare_github_pr(&pc.resolved_args).map_err(|e| {
            anyhow::anyhow!(
                "github.pr: pre-POST validation failed before confirm_granted \
                 (fail-closed, row remains Pending): {e:#}"
            )
        })?;
        let lit = |name: &str| -> Result<String> {
            pc.resolved_args
                .iter()
                .find(|a| a.name == name)
                .map(|a| a.literal.clone())
                .ok_or_else(|| {
                    anyhow::anyhow!("github.pr: missing `{name}` resolved arg (pre-burn)")
                })
        };
        Some(crate::audit::github_pr_content_key(
            &lit("owner")?,
            &lit("repo")?,
            &lit("base")?,
            &lit("head")?,
            &lit("title")?,
            &lit("body")?,
        ))
    } else {
        None
    };

    // Step 4.8c (v1.9 Phase 43, HTTP-W-01, DESIGN-v1.9-egress-policy §2):
    // `http.request.write` has a fallible pre-write validation (all three args
    // present + non-empty, the {POST,PUT} method-enum, and a constructible url).
    // Run it HERE — before Step 5 appends `confirm_granted` and Step 6 burns the
    // one-shot — using the SAME `prepare_http_write` the Step-7 dispatch calls,
    // so precheck and dispatch validate url/method/body IDENTICALLY and cannot
    // drift. A failure returns fail-closed-RECOVERABLE: the row stays `Pending`
    // (the operator can `deny` it, or re-`confirm` once corrected), NEVER a burned
    // confirmation with a dangling `confirm_granted` and no terminal event (the
    // P33/P34 confirm-release audit-gap class this phase's discipline guards).
    if pc.sink.0.as_str() == "http.request.write" {
        crate::sinks::http_write::prepare_http_write(&pc.resolved_args).map_err(|e| {
            anyhow::anyhow!(
                "http.request.write: pre-write validation failed before confirm_granted \
                 (fail-closed-RECOVERABLE, row remains Pending): {e:#}"
            )
        })?;
    }

    // Step 4.8d (v1.9 Phase 44 Plan 04, GIT-02/03, DESIGN-v1.9-egress-policy
    // §1.6/§1.7): `git.push` has a fallible socket-free pre-push validation
    // (remote present + git.push-allowlisted + url constructible + the
    // force/deletion refspec value-gate + a non-empty/shape-valid frozen_new_oid,
    // WG-7). Run it HERE — before Step 5 appends `confirm_granted` and Step 6
    // burns the one-shot — using the SAME `prepare_git_push` validators the
    // Step-7 transfer driver's `run_git_push` applies, so precheck and dispatch
    // cannot drift (P33/P34, no precheck/dispatch drift). A failure returns
    // fail-closed-RECOVERABLE: the row stays `Pending` (the operator can `deny`
    // it, or re-`confirm` once corrected), NEVER a burned confirmation with a
    // dangling `confirm_granted` and no terminal event (the P33/P34
    // confirm-release audit-gap class).
    if pc.sink.0.as_str() == "git.push" {
        crate::sinks::git_push::prepare_git_push(&pc.resolved_args, &pc.frozen_new_oid).map_err(
            |e| {
                anyhow::anyhow!(
                    "git.push: pre-push validation failed before confirm_granted \
                     (fail-closed-RECOVERABLE, row remains Pending): {e:#}"
                )
            },
        )?;
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
    let granted_hash = crate::audit::append_event(conn, key, &granted_event, Some(&head_hash))?;

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
        let affected = transition_state(conn, key, &pc, PendingConfirmationState::Confirmed)?;
        if affected == 0 {
            return Ok(ConfirmOutcome::AlreadyTerminal);
        }
    }

    // Step 7: dispatch to the frozen-snapshot sink re-invocation — NEVER
    // executor::submit_plan_node (CON-i2-non-bypassable, T-10-05).
    match pc.sink.0.as_str() {
        "file.create" => match crate::sinks::file_create::invoke_file_create_from_resolved(
            conn,
            key,
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
        // Phase 33 adversarial-review MAJOR-1 fix: mirrors the "file.create"
        // arm immediately above verbatim (same two-phase audit shape, same
        // ConfirmOutcome mapping) — a blocked `file.write` previously had NO
        // Step-7 dispatch arm here, so a human `confirm` would durably burn
        // the one-shot confirmation with no write and no terminal audit
        // event; the Step 4.75 entry guard now also prevents this
        // (defense-in-depth: this arm plus the guard).
        "file.write" => match crate::sinks::file_write::invoke_file_write_from_resolved(
            conn,
            key,
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
            let affected = transition_state(&tx, key, &pc, PendingConfirmationState::Confirmed)?;
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
                crate::audit::append_event(&tx, key, &attempted_event, Some(&granted_hash))?;
            tx.commit()?;

            // AFTER commit — the CAS + attempt are now durable together, or
            // neither is; only now does an SMTP connection ever open.
            match crate::sinks::email_smtp::invoke_email_smtp_from_resolved(
                conn,
                key,
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
        // Plan 34-02 (EXEC-05) + 34-03: process.exec confirm-release. The sink
        // is async (unlike the other three arms, which are sync internally), so
        // this arm — and only this arm — awaits. It does NOT mint the released
        // output: unlike the Allowed path (server.rs), which mints into the live
        // session `ValueStore` and RETURNS the `ValueId` for downstream plan
        // nodes to consume, `caprun confirm` is a separate human-driven process
        // with no live `ValueStore` and no subsequent plan node in the same
        // invocation — a mint here could only target a throwaway store,
        // discarded immediately (dead ceremony, removed in 34-03 adversarial
        // review). The genuine, non-stapled durable taint anchor already exists:
        // `invoke_process_exec_from_resolved` appends a `process_exited` Event
        // carrying `{ExternalUntrusted, ExecRaw}`, chained on `confirm_granted`
        // (D-03/D-04) — that is what a later read/consume derives provenance
        // from. Removing the mint also keeps Gate 3's mint-site allow-list
        // byte-identical (no confirmation.rs mint site) — the invariant that
        // mint sites live ONLY in quarantine.rs + server.rs is preserved, not
        // exempted with an inline marker.
        "process.exec" => {
            match crate::sinks::process_exec::invoke_process_exec_from_resolved(
                conn,
                key,
                pc.session_id,
                pc.effect_id,
                &pc.resolved_args,
                workspace_root,
                granted_event_id,
                &granted_hash,
            )
            .await
            {
                Ok((_sink_event_id, _hash, _combined_output)) => Ok(ConfirmOutcome::Released),
                // Pre-spawn OR spawn failed: the sink appended a durable
                // `process_spawn_failed` event chained on `granted_event_id`
                // FIRST (34-03 MAJOR fix — a burned confirmation is NEVER left
                // without a terminal audit event). The process may have
                // genuinely run (byte-cap trip, etc.) and is durably audited
                // either way; no retry (DESIGN Step 4a.5).
                Err(_) => Ok(ConfirmOutcome::ConfirmedButSinkFailed),
            }
        }
        // v1.8 Phase 38 (GITHUB-02/03/04, DESIGN §4.3/§4.5/§4.6/§9):
        // github.pr confirm-release. Like process.exec this arm is async and
        // awaits. TWO gates already passed pre-burn (Step 4.8b): the live
        // session auth-grant (GITHUB-02 — a bare confirm cannot create a PR)
        // and the `prepare_github_pr` precheck (url/body constructible), so a
        // dangling `confirm_granted` can never arise from a missing grant or a
        // malformed arg. Here we (a) reserve the content-derived duplicate-PR
        // CAS (GITHUB-04, §4.5) BEFORE the POST — mirroring the email.send
        // `sent_plan_nodes` suppress shape — and (b) dispatch on FRESH.
        //
        // EVERY post-burn leg folds into a terminal `github_pr_*` event
        // (terminal EVENT closing the burned one-shot — §9, the exact P33/P34
        // class): FRESH invoke lets the sink append its own opaque
        // github_pr_succeeded/github_pr_failed FIRST; a REPLAY (CAS not-fresh)
        // appends a distinct `github_pr_replay_suppressed` marker and makes NO
        // second POST; a CAS error itself appends `github_pr_failed`. NO
        // clear-key-on-failure path (§11 accepted at-most-once residual). This
        // arm does NOT mint (Gate 3's mint-site allow-list stays byte-identical
        // — no mint token in confirmation.rs).
        "github.pr" => {
            let content_key = github_pr_content_key
                .expect("github.pr content key is derived pre-burn in Step 4.8b");
            match crate::audit::reserve_created_pr(
                conn,
                &content_key,
                &pc.effect_id.to_string(),
                &pc.session_id.to_string(),
            ) {
                Ok(true) => {
                    // FRESH: this dispatch owns the effect and may POST.
                    // invoke_github_pr_from_resolved folds every failure
                    // (pre-POST OR transport) into an OPAQUE github_pr_failed
                    // terminal event FIRST, then propagates — never a dangling
                    // confirm_granted.
                    match crate::sinks::github_pr::invoke_github_pr_from_resolved(
                        conn,
                        key,
                        pc.session_id,
                        pc.effect_id,
                        &pc.resolved_args,
                        granted_event_id,
                        &granted_hash,
                    )
                    .await
                    {
                        Ok(_) => Ok(ConfirmOutcome::Released),
                        Err(_) => Ok(ConfirmOutcome::ConfirmedButSinkFailed),
                    }
                }
                Ok(false) => {
                    // REPLAY: an identical-content PR is already reserved.
                    // Suppress the second POST, but append a DISTINCT terminal
                    // marker so the burned one-shot is NEVER left dangling (§9).
                    let suppressed = runtime_core::Event::new(
                        Uuid::new_v4(),
                        Some(granted_event_id),
                        pc.session_id,
                        format!("sink:github.pr:{effect_id}"),
                        "github_pr_replay_suppressed".into(),
                        Utc::now(),
                        vec![],
                    );
                    crate::audit::append_event(conn, key, &suppressed, Some(&granted_hash))?;
                    eprintln!(
                        "[brokerd] github.pr replay suppressed (content key already \
                         reserved in created_prs): effect_id={}",
                        pc.effect_id
                    );
                    Ok(ConfirmOutcome::ConfirmedButSinkFailed)
                }
                Err(e) => {
                    // The CAS itself failed post-burn — fold into a terminal
                    // github_pr_failed event so the confirmation is never left
                    // dangling (§9/P33/P34); raw error goes only to eprintln.
                    eprintln!(
                        "[brokerd] github.pr CAS reservation failed (effect_id={}): {e}",
                        pc.effect_id
                    );
                    let failed = runtime_core::Event::new(
                        Uuid::new_v4(),
                        Some(granted_event_id),
                        pc.session_id,
                        format!("sink:github.pr:{effect_id}"),
                        "github_pr_failed".into(),
                        Utc::now(),
                        vec![],
                    );
                    crate::audit::append_event(conn, key, &failed, Some(&granted_hash))?;
                    Ok(ConfirmOutcome::ConfirmedButSinkFailed)
                }
            }
        }
        // 43-03 (HTTP-W-01, DESIGN-v1.9-egress-policy §2): http.request.write
        // confirm-release. Like process.exec/github.pr this arm is async and
        // awaits. The pre-burn Step-4.8c precheck (`prepare_http_write`) already
        // validated url/method/body, so a dangling `confirm_granted` can never
        // arise from a malformed arg. `invoke_http_write_from_resolved` folds
        // EVERY failure (pre-write OR transport) into an OPAQUE `http_write_failed`
        // terminal event FIRST, then propagates — never a burned confirmation
        // with no terminal event (§9/P33/P34). No CAS/dedup: DESIGN §2 defines
        // http.request.write as a single confirm-releasable write. This arm does
        // NOT mint (Gate 3's mint-site allow-list stays byte-identical — no mint
        // token in confirmation.rs).
        "http.request.write" => {
            match crate::sinks::http_write::invoke_http_write_from_resolved(
                conn,
                key,
                pc.session_id,
                pc.effect_id,
                &pc.resolved_args,
                granted_event_id,
                &granted_hash,
            )
            .await
            {
                Ok(_) => Ok(ConfirmOutcome::Released),
                // The sink already appended a durable opaque http_write_failed
                // terminal event; state stays Confirmed, no retry (DESIGN Step 4a.5).
                Err(_) => Ok(ConfirmOutcome::ConfirmedButSinkFailed),
            }
        }
        // v1.9 Phase 44 Plan 04 (GIT-02/03, DESIGN-v1.9-egress-policy
        // §1.5/§1.6/§1.7): git.push confirm-release — the ONLY transfer entry
        // point (there is NO auto-dispatch Allowed arm; even a clean/untainted
        // git.push is re-gated into a pending confirmation by server.rs). Like
        // process.exec/github.pr/http.request.write this arm is async and awaits.
        // The pre-burn Step-4.8d precheck (`prepare_git_push`) already validated
        // remote/refspec/frozen_new_oid, so a dangling `confirm_granted` can
        // never arise from a malformed arg. `invoke_git_push_from_resolved`
        // threads the human-confirmed `frozen_new_oid` into the WG-7 anti-TOCTOU
        // equality gate (a live rev-parse that diverges is refused BEFORE any
        // pack/POST) and folds EVERY failure (pre-transfer gate OR transport)
        // into an OPAQUE `git_push_failed` terminal event FIRST, then propagates
        // — never a burned confirmation with no terminal event (§1.7/P33/P34).
        // No CAS/dedup (the new-oid freeze + at-most-once one-shot ARE the
        // duplicate defense). This arm does NOT mint (Gate 3's mint-site
        // allow-list stays byte-identical — no mint token in confirmation.rs).
        "git.push" => {
            match crate::sinks::git_push::invoke_git_push_from_resolved(
                conn,
                key,
                pc.session_id,
                pc.effect_id,
                &pc.resolved_args,
                workspace_root,
                &pc.frozen_new_oid,
                granted_event_id,
                &granted_hash,
            )
            .await
            {
                Ok(_) => Ok(ConfirmOutcome::Released),
                // The sink already appended a durable opaque git_push_failed
                // terminal event; state stays Confirmed, no retry (DESIGN Step 4a.5).
                Err(_) => Ok(ConfirmOutcome::ConfirmedButSinkFailed),
            }
        }
        // Phase 33 adversarial-review MAJOR-1 fix: this arm IS reachable in
        // principle (the match is exhaustive over `&str`), but the Step 4.75
        // entry guard above already refuses any sink outside
        // {"file.create","email.send","file.write","process.exec","github.pr",
        //  "http.request.write","git.push"} BEFORE reaching here — so hitting this
        // arm means the guard's allow-list and this match's arms have drifted out
        // of sync (a broker-internal invariant violation, not a normal runtime path).
        other => Err(anyhow::anyhow!(
            "confirm: unreachable — sink `{other}` passed the Step 4.75 entry \
             guard but has no Step 7 dispatch arm (guard/match drift)"
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
///
/// v1.6 Phase 28 Plan 05 (HARDEN-02 / X-02): `deny` now gains the SAME
/// pending_confirmations MAC gate AND `verify_chain` gate `confirm()` has —
/// previously `deny` had NO integrity check at all (28-RESEARCH.md NEW
/// FINDING).
pub fn deny(conn: &rusqlite::Connection, key: &[u8], effect_id: &str) -> Result<ConfirmOutcome> {
    // Step 1: same fresh lookup as confirm.
    let pc = match find_pending_confirmation(conn, effect_id)? {
        Some(pc) => pc,
        None => return Ok(ConfirmOutcome::UnknownEffect),
    };

    // Step 1.5: pending_confirmations whole-row MAC-verify IMMEDIATELY after
    // fetch, BEFORE Step 2 reads pc.state (Pitfall 5) — mirrors confirm()'s
    // Step 1.5 exactly.
    if !verify_pending_confirmation_mac(key, &pc) {
        let (head_id, head_hash) = current_chain_head_or_bail(conn, pc.session_id)?;
        append_digest_mismatch_event(conn, key, &pc, effect_id, head_id, &head_hash, "deny")?;
        return Ok(ConfirmOutcome::DigestMismatch);
    }

    // Step 2: terminal-state check, read from the persisted (now MAC-verified) row.
    if pc.state != PendingConfirmationState::Pending {
        return Ok(ConfirmOutcome::AlreadyTerminal);
    }

    // Both verbs show the same evidence before acting (DESIGN CLI Contract).
    println!("{}", render_block_display(&pc));

    // Step 2.5 (NEW — X-02): the SAME chain-verify gate confirm()'s Step
    // 4.5a runs. Fail closed on a broken/forged events chain BEFORE
    // recording a durable deny.
    if !crate::audit::verify_chain(conn, &pc.session_id.to_string(), key) {
        let (head_id, head_hash) = current_chain_head_or_bail(conn, pc.session_id)?;
        append_digest_mismatch_event(conn, key, &pc, effect_id, head_id, &head_hash, "deny")?;
        return Ok(ConfirmOutcome::DigestMismatch);
    }

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
    crate::audit::append_event(conn, key, &denied_event, Some(&head_hash))?;

    let affected = transition_state(conn, key, &pc, PendingConfirmationState::Denied)?;
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

    /// Fixed, non-secret test MAC key (mirrors `audit.rs`'s `TEST_KEY`) — used
    /// consistently for both the seeding `append_event` calls AND the
    /// `confirm()`/`deny()` calls under test in this module, so the keyed
    /// `verify_chain` gate verifies true on an untampered chain.
    const TEST_KEY: &[u8] = b"confirmation-rs-unit-test-key-not-secret";

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
            frozen_new_oid: String::new(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
        }
    }

    #[test]
    fn insert_then_find_round_trips_all_fields() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let effect_id = Uuid::new_v4();
        let pc = make_pending_confirmation(effect_id);

        insert_pending_confirmation(&conn, TEST_KEY, &pc).expect("insert_pending_confirmation");

        let found = find_pending_confirmation(&conn, &effect_id.to_string())
            .expect("find_pending_confirmation")
            .expect("row should be present");

        assert_eq!(found.effect_id, pc.effect_id);
        assert_eq!(found.session_id, pc.session_id);
        assert_eq!(found.blocked_event_id, pc.blocked_event_id);
        assert_eq!(found.sink, pc.sink);
        assert_eq!(found.resolved_args, pc.resolved_args);
        assert_eq!(found.blocked_arg_names, pc.blocked_arg_names);
        assert_eq!(found.combined_digest, pc.combined_digest);
        assert_eq!(found.workspace_root_path, pc.workspace_root_path);
        assert_eq!(found.state, PendingConfirmationState::Pending);
        assert!(
            verify_pending_confirmation_mac(TEST_KEY, &found),
            "the persisted whole-row MAC must verify under the same key used to insert it"
        );
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
        insert_pending_confirmation(&conn, TEST_KEY, &pc).expect("insert_pending_confirmation");

        let confirmed = transition_state(&conn, TEST_KEY, &pc, PendingConfirmationState::Confirmed)
            .expect("transition_state to Confirmed");
        assert_eq!(confirmed, 1);

        let denied_after_confirmed =
            transition_state(&conn, TEST_KEY, &pc, PendingConfirmationState::Denied)
                .expect("transition_state to Denied after Confirmed");
        assert_eq!(denied_after_confirmed, 0);

        let found = find_pending_confirmation(&conn, &effect_id.to_string())
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
        insert_pending_confirmation(&conn, TEST_KEY, &pc).expect("insert_pending_confirmation");

        let denied = transition_state(&conn, TEST_KEY, &pc, PendingConfirmationState::Denied)
            .expect("transition_state to Denied");
        assert_eq!(denied, 1);

        let confirmed_after_denied =
            transition_state(&conn, TEST_KEY, &pc, PendingConfirmationState::Confirmed)
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
            frozen_new_oid: String::new(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
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
            frozen_new_oid: String::new(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
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
        let root_hash = append_event(conn, TEST_KEY, &root, None).unwrap();

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
        append_event(conn, TEST_KEY, &blocked_event, Some(&root_hash)).unwrap();
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
            frozen_new_oid: String::new(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
        };
        insert_pending_confirmation(conn, TEST_KEY, &pc).unwrap();

        (effect_id, session_id, blocked_event_id)
    }

    /// (a) confirm on a Pending file.create block releases exactly once: the
    /// file is created, a confirm_granted event exists chained onto the
    /// sink_blocked event, and the row transitions to Confirmed.
    #[tokio::test]
    async fn confirm_on_pending_file_create_releases_and_creates_file() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_ok_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        let outcome = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("confirm");
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
    #[tokio::test]
    async fn confirm_twice_returns_already_terminal_and_creates_no_new_file() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_twice_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, _session_id, _blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        let first = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("first confirm");
        assert_eq!(first, ConfirmOutcome::Released);

        let second = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("second confirm");
        assert_eq!(second, ConfirmOutcome::AlreadyTerminal);

        let entries: Vec<_> = std::fs::read_dir(&root).unwrap().collect();
        assert_eq!(
            entries.len(),
            1,
            "a second confirm must not create any additional file"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    // ── Phase 33 adversarial-review MAJOR-1 fix: file.write confirm-release
    // + entry-guard tests ─────────────────────────────────────────────────

    /// Seed a Pending file.write block: a causal-root event, a `sink_blocked`
    /// event carrying a genuine `SinkBlockedAnchor`, its `blocked_literals` row,
    /// and a matching `PendingConfirmation` — mirrors
    /// `seed_pending_file_create_block` exactly, substituting the sink and the
    /// pre-existing-target requirement (`write_within` is existing-file-only;
    /// the caller MUST create `path` under `workspace_root_path` before
    /// seeding).
    ///
    /// Returns `(effect_id, session_id, blocked_event_id)`.
    fn seed_pending_file_write_block(
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
        let root_hash = append_event(conn, TEST_KEY, &root, None).unwrap();

        let literal_sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(path.as_bytes());
            hex::encode(hasher.finalize())
        };
        let anchor = SinkBlockedAnchor {
            effect_id,
            sink: SinkId("file.write".into()),
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
        append_event(conn, TEST_KEY, &blocked_event, Some(&root_hash)).unwrap();
        insert_blocked_literal(conn, &blocked_event_id.to_string(), "path", path).unwrap();

        let pc = PendingConfirmation {
            effect_id,
            session_id,
            blocked_event_id,
            sink: SinkId("file.write".into()),
            resolved_args,
            blocked_arg_names,
            combined_digest: digest,
            workspace_root_path: workspace_root_path.to_string(),
            frozen_new_oid: String::new(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
        };
        insert_pending_confirmation(conn, TEST_KEY, &pc).unwrap();

        (effect_id, session_id, blocked_event_id)
    }

    /// confirm on a Pending file.write block releases exactly once: the
    /// pre-existing target's contents are overwritten, a confirm_granted event
    /// exists chained onto the sink_blocked event, a chained sink_executed
    /// event exists, `verify_chain` is true, and the row transitions to
    /// Confirmed (MAJOR-1 fix — previously this dispatch arm did not exist).
    #[tokio::test]
    async fn confirm_on_pending_file_write_releases_and_writes_file() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_fw_ok_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        // write_within is existing-file-only — pre-create the target.
        std::fs::write(root.join("existing.txt"), b"original").unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, blocked_event_id) = seed_pending_file_write_block(
            &conn,
            "existing.txt",
            "released by confirm",
            &root.to_string_lossy(),
        );

        let outcome = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("confirm");
        assert_eq!(outcome, ConfirmOutcome::Released);

        let on_disk = std::fs::read_to_string(root.join("existing.txt")).unwrap();
        assert_eq!(on_disk, "released by confirm");

        let granted = find_event_by_type(&conn, &session_id.to_string(), "confirm_granted")
            .unwrap()
            .expect("confirm_granted event must exist");
        assert_eq!(granted.actor, format!("confirm:{effect_id}"));
        assert_eq!(granted.parent_id, Some(blocked_event_id));

        let sink_executed = find_event_by_type(&conn, &session_id.to_string(), "sink_executed")
            .unwrap()
            .expect("sink_executed event must exist — the write was actually released");
        assert_eq!(sink_executed.actor, format!("sink:file.write:{effect_id}"));
        assert_eq!(sink_executed.parent_id, Some(granted.id));

        assert!(
            crate::audit::verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "the chain must remain unbroken after a file.write confirm-release"
        );

        let pc = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(pc.state, PendingConfirmationState::Confirmed);

        std::fs::remove_dir_all(&root).ok();
    }

    // ── git.push confirm-release wiring (v1.9 Phase 44 Plan 04, GIT-02/03) ──

    /// Seed a Pending git.push block: a `session_created` root, a `sink_blocked`
    /// event carrying the combined digest over `{remote, refspec}` + a
    /// blocked_literals row (so the confirm redaction/digest gates pass), and a
    /// `PendingConfirmation` carrying `frozen_new_oid` (WG-7). Mirrors
    /// `seed_pending_file_write_block`. `blocked_arg_names = ["remote"]` models
    /// the tainted-remote I2-Block path; the clean-Allowed path assembles an
    /// equivalent row (server.rs, Task 2) — both converge on this shape.
    fn seed_pending_git_push_block(
        conn: &rusqlite::Connection,
        remote: &str,
        refspec: &str,
        frozen_new_oid: &str,
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
        let root_hash = append_event(conn, TEST_KEY, &root, None).unwrap();

        let literal_sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(remote.as_bytes());
            hex::encode(hasher.finalize())
        };
        let anchor = SinkBlockedAnchor {
            effect_id,
            sink: SinkId("git.push".into()),
            arg: "remote".into(),
            value_id: ValueId::new(),
            literal_sha256,
            taint: vec![TaintLabel::ExternalUntrusted],
            provenance_chain: vec![read_event_id],
            read_event_id,
        };

        let resolved_args = vec![
            ResolvedArg {
                name: "remote".to_string(),
                value_id: ValueId::new(),
                literal: remote.to_string(),
                taint: vec![TaintLabel::ExternalUntrusted],
                provenance_chain: vec![read_event_id],
            },
            ResolvedArg {
                name: "refspec".to_string(),
                value_id: ValueId::new(),
                literal: refspec.to_string(),
                taint: vec![TaintLabel::UserTrusted],
                provenance_chain: vec![],
            },
        ];
        let digest = combined_digest(
            &resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        let blocked_arg_names = vec!["remote".to_string()];

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
        append_event(conn, TEST_KEY, &blocked_event, Some(&root_hash)).unwrap();
        insert_blocked_literal(conn, &blocked_event_id.to_string(), "remote", remote).unwrap();

        let pc = PendingConfirmation {
            effect_id,
            session_id,
            blocked_event_id,
            sink: SinkId("git.push".into()),
            resolved_args,
            blocked_arg_names,
            combined_digest: digest,
            workspace_root_path: workspace_root_path.to_string(),
            frozen_new_oid: frozen_new_oid.to_string(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
        };
        insert_pending_confirmation(conn, TEST_KEY, &pc).unwrap();

        (effect_id, session_id, blocked_event_id)
    }

    /// WG-7: the git.push `frozen_new_oid` rides the whole-row MAC — a tampered
    /// oid (or a forged git.push pending row) fails `verify_pending_confirmation_
    /// mac`. Host-portable (no git / no socket).
    #[test]
    fn git_push_frozen_new_oid_is_mac_covered() {
        const FROZEN: &str = "abcdef0123456789abcdef0123456789abcdef01";
        let conn = open_audit_db(":memory:").unwrap();
        let (effect_id, _session_id, _blocked_event_id) = seed_pending_git_push_block(
            &conn,
            "https://github-mock.caprun.test/owner/repo.git",
            "refs/heads/main:refs/heads/main",
            FROZEN,
            "/workspace",
        );

        // As persisted, the whole-row MAC verifies and the oid round-trips.
        let pc = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(pc.frozen_new_oid, FROZEN, "the frozen oid round-trips through persistence");
        assert!(
            verify_pending_confirmation_mac(TEST_KEY, &pc),
            "an untampered git.push pending row's whole-row MAC must verify"
        );

        // Tamper ONLY the frozen oid in the fetched row — the MAC must now fail
        // (the freeze is inside the integrity boundary, not a bare side field).
        let mut tampered = pc.clone();
        tampered.frozen_new_oid = "1111111111111111111111111111111111111111".to_string();
        assert!(
            !verify_pending_confirmation_mac(TEST_KEY, &tampered),
            "a tampered frozen_new_oid MUST fail the whole-row MAC (WG-7)"
        );
    }

    /// Step 4.75 guard + Step 4.8d precheck (P33/P34): git.push is ON the
    /// entry-guard allow-list (it reaches the precheck, never the guard-drift
    /// error), and a pre-push precheck FAILURE is fail-closed-RECOVERABLE — the
    /// row stays `Pending`, NO `confirm_granted` is appended, and NO terminal
    /// `git_push_*` event exists (never a burned one-shot with a dangling
    /// confirm_granted). Uses a FORCE refspec (leading `+`), which the
    /// `validate_git_refspec` value-gate refuses in BOTH the default build (where
    /// the empty push allowlist also fails it first) AND under `mock-egress-ca`
    /// (where the mock host IS allowlisted but the force refspec still fails) —
    /// so the precheck failure is deterministic regardless of feature. The
    /// precheck is socket-free (host-portable, no git / no network).
    #[tokio::test]
    async fn git_push_precheck_failure_leaves_row_pending_no_burn() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_gitpush_precheck_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, _blocked_event_id) = seed_pending_git_push_block(
            &conn,
            "https://github-mock.caprun.test/owner/repo.git",
            // A FORCE refspec (leading '+') is refused by validate_git_refspec in
            // the precheck — deterministic across default AND mock-egress-ca
            // builds (never dependent on the allowlist state).
            "+refs/heads/main:refs/heads/main",
            "abcdef0123456789abcdef0123456789abcdef01",
            &root.to_string_lossy(),
        );

        let result = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await;
        assert!(
            result.is_err(),
            "a git.push precheck failure must return Err (fail-closed-RECOVERABLE), \
             NOT a guard-drift panic and NOT a burned Released outcome: {result:?}"
        );

        // The row is untouched — still Pending, re-confirmable once corrected.
        let pc = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(
            pc.state,
            PendingConfirmationState::Pending,
            "a pre-burn precheck failure MUST leave the row Pending"
        );
        // No one-shot was burned and no terminal event was appended.
        let sid = session_id.to_string();
        assert!(
            find_event_by_type(&conn, &sid, "confirm_granted").unwrap().is_none(),
            "NO confirm_granted may be appended before a passing precheck"
        );
        assert!(
            find_event_by_type(&conn, &sid, "git_push_failed").unwrap().is_none(),
            "the transfer never ran — no git_push_failed terminal event"
        );
        assert!(
            find_event_by_type(&conn, &sid, "git_push_succeeded").unwrap().is_none(),
            "the transfer never ran — no git_push_succeeded terminal event"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// The clean confirm-release reaches Step-7 dispatch (needs the mock host on
    /// the push allowlist, so gated behind `mock-egress-ca` — the default build's
    /// empty allowlist fails the precheck, covered by the test above). The
    /// precheck PASSES, `confirm_granted` is appended, the state burns to
    /// `Confirmed` (released exactly once), and `invoke_git_push_from_resolved`
    /// folds the host-unreachable transfer into a terminal `git_push_failed`
    /// event FIRST (terminal EVENT before the terminal disposition — §1.7/P33/P34),
    /// yielding `ConfirmedButSinkFailed`. A second confirm is refused as
    /// `AlreadyTerminal` (single-shot).
    #[cfg(feature = "mock-egress-ca")]
    #[tokio::test]
    async fn git_push_confirm_releases_once_reaching_step7_dispatch() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_gitpush_step7_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, blocked_event_id) = seed_pending_git_push_block(
            &conn,
            "https://github-mock.caprun.test/owner/repo.git",
            "refs/heads/main:refs/heads/main",
            "abcdef0123456789abcdef0123456789abcdef01",
            &root.to_string_lossy(),
        );

        let outcome = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws)
            .await
            .expect("confirm must complete (not a transport-level Err)");
        // The transfer cannot succeed here (no live mock receive-pack), so the
        // one-shot burns to Confirmed and the sink reports failure — but it is
        // RELEASED (reached Step-7), never blocked at the precheck.
        assert_eq!(outcome, ConfirmOutcome::ConfirmedButSinkFailed);

        let sid = session_id.to_string();
        let granted = find_event_by_type(&conn, &sid, "confirm_granted")
            .unwrap()
            .expect("confirm_granted must exist — the precheck passed and Step-7 ran");
        assert_eq!(granted.parent_id, Some(blocked_event_id));
        assert!(
            find_event_by_type(&conn, &sid, "git_push_failed").unwrap().is_some(),
            "the transfer failure folds into a terminal git_push_failed event FIRST \
             (never a burned confirmation with no terminal event)"
        );
        assert!(
            crate::audit::verify_chain(&conn, &sid, TEST_KEY),
            "the chain stays unbroken across a git.push confirm-release"
        );

        let pc = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(pc.state, PendingConfirmationState::Confirmed);

        // Single-shot: a second confirm is refused (the one-shot was burned).
        let again = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws)
            .await
            .expect("second confirm completes");
        assert_eq!(again, ConfirmOutcome::AlreadyTerminal);

        std::fs::remove_dir_all(&root).ok();
    }

    // ── WG-8 git.push confirm-prompt payload-provenance renderer (Task 3) ──

    /// Build an in-memory git.push PendingConfirmation (no DB) for the pure
    /// render tests. `remote_taint` sets the remote arg's taint; the refspec is
    /// UserTrusted.
    fn git_push_pc_for_render(
        remote_literal: &str,
        remote_taint: Vec<TaintLabel>,
        frozen_new_oid: &str,
    ) -> PendingConfirmation {
        PendingConfirmation {
            effect_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            blocked_event_id: Uuid::new_v4(),
            sink: SinkId("git.push".into()),
            resolved_args: vec![
                ResolvedArg {
                    name: "remote".to_string(),
                    value_id: ValueId::new(),
                    literal: remote_literal.to_string(),
                    taint: remote_taint,
                    provenance_chain: vec![],
                },
                ResolvedArg {
                    name: "refspec".to_string(),
                    value_id: ValueId::new(),
                    literal: "refs/heads/main:refs/heads/main".to_string(),
                    taint: vec![TaintLabel::UserTrusted],
                    provenance_chain: vec![],
                },
            ],
            blocked_arg_names: vec!["remote".to_string(), "refspec".to_string()],
            combined_digest: "deadbeef".to_string(),
            workspace_root_path: "/unused-for-render".to_string(),
            frozen_new_oid: frozen_new_oid.to_string(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
        }
    }

    /// The git.push confirm block renders the frozen commit oid + the payload
    /// provenance summary, and flags an untrusted-derived routing arg
    /// [UNTRUSTED-DERIVED]. Pure — no network/git read (renders with no live
    /// remote and no workspace).
    #[test]
    fn git_push_render_flags_untrusted_arg_and_shows_frozen_oid() {
        const FROZEN: &str = "abcdef0123456789abcdef0123456789abcdef01";
        let pc = git_push_pc_for_render(
            "https://evil.example.com/x.git",
            vec![TaintLabel::ExternalUntrusted],
            FROZEN,
        );
        let out = render_block_display(&pc);
        assert!(out.contains("Push payload provenance (git.push"), "WG-8 section header present");
        assert!(out.contains(FROZEN), "the frozen new-oid (pushed commit) is surfaced");
        assert!(
            out.contains("[UNTRUSTED-DERIVED]"),
            "an untrusted-tainted routing arg must be flagged: {out}"
        );
        assert!(
            out.contains("accepted residual"),
            "the §1.6 no-byte-identity residual note is shown (no over-promise)"
        );
    }

    /// A tainted routing literal carrying embedded control characters (an ANSI
    /// ESC sequence) is control-char-NEUTRALIZED in the payload summary — the raw
    /// ESC byte never reaches the terminal (T-44-19 audit-line-spoofing defense).
    #[test]
    fn git_push_render_neutralizes_control_chars() {
        const FROZEN: &str = "abcdef0123456789abcdef0123456789abcdef01";
        // remote literal embeds ESC (0x1b) + a CSI colour code + a CR.
        let evil = "https://x.test/\u{1b}[31mHACKED\u{1b}[0m\r.git";
        let pc = git_push_pc_for_render(evil, vec![TaintLabel::ExternalUntrusted], FROZEN);
        let out = render_block_display(&pc);
        assert!(
            !out.contains('\u{1b}'),
            "the raw ESC byte MUST be neutralized before display (no ANSI injection)"
        );
        assert!(!out.contains('\r'), "the raw CR MUST be neutralized");
        assert!(out.contains("\\x1b"), "ESC is shown as a visible \\x1b escape: {out}");
    }

    /// A CLEAN (all-UserTrusted) git.push renders [trusted] and NO
    /// [UNTRUSTED-DERIVED] flag.
    #[test]
    fn git_push_render_clean_shows_no_untrusted_flag() {
        const FROZEN: &str = "1234567890123456789012345678901234567890";
        let pc = git_push_pc_for_render(
            "https://github-mock.caprun.test/owner/repo.git",
            vec![TaintLabel::UserTrusted],
            FROZEN,
        );
        let out = render_block_display(&pc);
        assert!(out.contains("[trusted]"), "a clean routing arg renders [trusted]");
        assert!(
            !out.contains("[UNTRUSTED-DERIVED]"),
            "a clean git.push must NOT flag any arg untrusted-derived: {out}"
        );
    }

    /// A NON-git.push confirm block is UNAFFECTED — no payload-provenance section.
    #[test]
    fn non_git_push_render_has_no_payload_summary() {
        let pc = make_pending_confirmation(Uuid::new_v4());
        let out = render_block_display(&pc);
        assert!(
            !out.contains("Push payload provenance"),
            "only git.push blocks carry the WG-8 payload summary"
        );
    }

    /// Entry-guard (Step 4.75): an un-dispatchable sink's blocked confirmation
    /// (a still-unwired sink name — `process.exec` was the example here
    /// through Phase 33, but Plan 34-02 wires its Step 7 dispatch arm, so
    /// this test now uses a deliberately fictitious, never-wired sink name to
    /// keep exercising the guard mechanism itself) must NOT be burned by
    /// `confirm()` — the row must remain `Pending`, no `confirm_granted`
    /// event may exist, and the state-transitioning function must return
    /// `Err` rather than silently succeeding or leaving an orphaned
    /// `confirm_granted`-with-no-terminal-event audit gap.
    #[tokio::test]
    async fn confirm_on_undispatchable_sink_does_not_burn_confirmation() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_guard_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let session_id = Uuid::new_v4();
        let effect_id = Uuid::new_v4();
        let read_event_id = Uuid::new_v4();

        let root_evt = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            chrono::Utc::now(),
            vec![],
        );
        let root_hash = append_event(&conn, TEST_KEY, &root_evt, None).unwrap();

        let literal_sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(b"rm -rf /");
            hex::encode(hasher.finalize())
        };
        let anchor = SinkBlockedAnchor {
            effect_id,
            sink: SinkId("sink.never-wired".into()),
            arg: "command".into(),
            value_id: ValueId::new(),
            literal_sha256,
            taint: vec![TaintLabel::ExecRaw],
            provenance_chain: vec![read_event_id],
            read_event_id,
        };
        let resolved_args = vec![ResolvedArg {
            name: "command".to_string(),
            value_id: ValueId::new(),
            literal: "rm -rf /".to_string(),
            taint: vec![TaintLabel::ExecRaw],
            provenance_chain: vec![read_event_id],
        }];
        let digest = combined_digest(
            &resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        let blocked_arg_names = vec!["command".to_string()];

        let blocked_event = Event::sink_blocked(
            Uuid::new_v4(),
            Some(root_evt.id),
            session_id,
            chrono::Utc::now(),
            vec![anchor],
            Some(digest.clone()),
            blocked_arg_names.clone(),
        );
        let blocked_event_id = blocked_event.id;
        append_event(&conn, TEST_KEY, &blocked_event, Some(&root_hash)).unwrap();
        insert_blocked_literal(&conn, &blocked_event_id.to_string(), "command", "rm -rf /").unwrap();

        let pc = PendingConfirmation {
            effect_id,
            session_id,
            blocked_event_id,
            sink: SinkId("sink.never-wired".into()),
            resolved_args,
            blocked_arg_names,
            combined_digest: digest,
            workspace_root_path: root.to_string_lossy().to_string(),
            frozen_new_oid: String::new(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
        };
        insert_pending_confirmation(&conn, TEST_KEY, &pc).unwrap();

        let result = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await;
        assert!(
            result.is_err(),
            "confirm on an un-dispatchable sink must fail closed, not silently succeed"
        );

        // The confirmation must NOT be burned: still Pending.
        let found = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .expect("row must still exist");
        assert_eq!(
            found.state,
            PendingConfirmationState::Pending,
            "the entry guard must refuse BEFORE Step 6's state transition — \
             an un-dispatchable sink must leave the row Pending, not Confirmed"
        );

        // No confirm_granted event may have been appended (Step 5 never ran).
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "confirm_granted")
                .unwrap()
                .is_none(),
            "the entry guard must refuse BEFORE Step 5 appends confirm_granted — \
             no audit-DAG gap (confirm_granted with no terminal sink event) may exist"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// 34-03 adversarial-review MAJOR regression: a `process.exec` confirmation
    /// whose FROZEN `args` literal is not valid JSON must fail closed at the
    /// Step-4.8 precheck — BEFORE Step 5 appends `confirm_granted` or Step 6
    /// burns the one-shot. Before the fix, the malformed `args` `?`-propagated
    /// out of `invoke_process_exec_from_resolved` AFTER the burn, leaving a
    /// dangling `confirm_granted` with no terminal event (the P33 MAJOR-1
    /// audit-gap class, worker-reachable because `validate_schema` checks arg
    /// NAMES only, never the literal's JSON shape). Runs on any platform: the
    /// precheck fails at the JSON parse, before any launcher resolution or
    /// spawn. Asserts fail-closed-RECOVERABLE: Err, row still Pending, no
    /// confirm_granted event.
    #[tokio::test]
    async fn confirm_on_process_exec_malformed_args_does_not_burn_confirmation() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_exec_badargs_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let session_id = Uuid::new_v4();
        let effect_id = Uuid::new_v4();
        let read_event_id = Uuid::new_v4();

        let root_evt = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            chrono::Utc::now(),
            vec![],
        );
        let root_hash = append_event(&conn, TEST_KEY, &root_evt, None).unwrap();

        let literal_sha256 = {
            let mut hasher = Sha256::new();
            hasher.update(b"echo");
            hex::encode(hasher.finalize())
        };
        let anchor = SinkBlockedAnchor {
            effect_id,
            sink: SinkId("process.exec".into()),
            arg: "command".into(),
            value_id: ValueId::new(),
            literal_sha256,
            taint: vec![TaintLabel::ExecRaw],
            provenance_chain: vec![read_event_id],
            read_event_id,
        };
        // Valid `command`, but `args` is NOT a JSON Vec<String> — the exact
        // worker-frozen malformed literal that the MAJOR made fatal post-burn.
        let resolved_args = vec![
            ResolvedArg {
                name: "command".to_string(),
                value_id: ValueId::new(),
                literal: "echo".to_string(),
                taint: vec![TaintLabel::ExecRaw],
                provenance_chain: vec![read_event_id],
            },
            ResolvedArg {
                name: "args".to_string(),
                value_id: ValueId::new(),
                literal: "not-json".to_string(),
                taint: vec![TaintLabel::ExecRaw],
                provenance_chain: vec![read_event_id],
            },
        ];
        let digest = combined_digest(
            &resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        let blocked_arg_names = vec!["command".to_string()];

        let blocked_event = Event::sink_blocked(
            Uuid::new_v4(),
            Some(root_evt.id),
            session_id,
            chrono::Utc::now(),
            vec![anchor],
            Some(digest.clone()),
            blocked_arg_names.clone(),
        );
        let blocked_event_id = blocked_event.id;
        append_event(&conn, TEST_KEY, &blocked_event, Some(&root_hash)).unwrap();
        insert_blocked_literal(&conn, &blocked_event_id.to_string(), "command", "echo").unwrap();

        let pc = PendingConfirmation {
            effect_id,
            session_id,
            blocked_event_id,
            sink: SinkId("process.exec".into()),
            resolved_args,
            blocked_arg_names,
            combined_digest: digest,
            workspace_root_path: root.to_string_lossy().to_string(),
            frozen_new_oid: String::new(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
        };
        insert_pending_confirmation(&conn, TEST_KEY, &pc).unwrap();

        let result = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await;
        assert!(
            result.is_err(),
            "confirm on a process.exec block with malformed frozen `args` must \
             fail closed at the Step-4.8 precheck, not silently succeed"
        );

        // The confirmation must NOT be burned: still Pending (recoverable).
        let found = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .expect("row must still exist");
        assert_eq!(
            found.state,
            PendingConfirmationState::Pending,
            "the Step-4.8 precheck must refuse BEFORE Step 6's state transition — \
             a pre-spawn failure must leave the row Pending, not Confirmed"
        );

        // No confirm_granted event may have been appended (Step 5 never ran) —
        // this is the audit-DAG gap the MAJOR fix closes.
        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "confirm_granted")
                .unwrap()
                .is_none(),
            "the precheck must refuse BEFORE Step 5 appends confirm_granted — no \
             dangling confirm_granted with no terminal process.exec event may exist"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    // ── v1.8 Phase 38: github.pr confirm-release (GITHUB-02/03, P33/P34) ──────

    /// Seed a Blocked `github.pr` PendingConfirmation whose `blocked_arg`
    /// (`title` or `body`) resolves from an untrusted-tainted value — the
    /// GITHUB-03 exfil vector. All SIX PR args are present + non-empty so the
    /// pre-burn `prepare_github_pr` precheck passes; only `blocked_arg` carries
    /// `ExternalUntrusted` taint. Returns `(effect_id, session_id,
    /// blocked_event_id)`.
    fn seed_pending_github_pr_block(
        conn: &rusqlite::Connection,
        blocked_arg: &str,
        tainted_literal: &str,
        root_path: &str,
    ) -> (Uuid, Uuid, Uuid) {
        let session_id = Uuid::new_v4();
        let effect_id = Uuid::new_v4();
        let read_event_id = Uuid::new_v4();

        let root_evt = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            chrono::Utc::now(),
            vec![],
        );
        let root_hash = append_event(conn, TEST_KEY, &root_evt, None).unwrap();

        let base_vals = [
            ("owner", "octocat"),
            ("repo", "hello-world"),
            ("base", "main"),
            ("head", "feature"),
            ("title", "My PR"),
            ("body", "PR description"),
        ];
        let resolved_args: Vec<ResolvedArg> = base_vals
            .iter()
            .map(|(name, dflt)| {
                let is_blocked = *name == blocked_arg;
                ResolvedArg {
                    name: name.to_string(),
                    value_id: ValueId::new(),
                    literal: if is_blocked {
                        tainted_literal.to_string()
                    } else {
                        dflt.to_string()
                    },
                    taint: if is_blocked {
                        vec![TaintLabel::ExternalUntrusted]
                    } else {
                        vec![TaintLabel::UserTrusted]
                    },
                    provenance_chain: if is_blocked {
                        vec![read_event_id]
                    } else {
                        vec![]
                    },
                }
            })
            .collect();

        let digest = combined_digest(
            &resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        let blocked_arg_names = vec![blocked_arg.to_string()];

        let literal_sha256 = {
            let mut h = Sha256::new();
            h.update(tainted_literal.as_bytes());
            hex::encode(h.finalize())
        };
        let anchor = SinkBlockedAnchor {
            effect_id,
            sink: SinkId("github.pr".into()),
            arg: blocked_arg.into(),
            value_id: ValueId::new(),
            literal_sha256,
            taint: vec![TaintLabel::ExternalUntrusted],
            provenance_chain: vec![read_event_id],
            read_event_id,
        };
        let blocked_event = Event::sink_blocked(
            Uuid::new_v4(),
            Some(root_evt.id),
            session_id,
            chrono::Utc::now(),
            vec![anchor],
            Some(digest.clone()),
            blocked_arg_names.clone(),
        );
        let blocked_event_id = blocked_event.id;
        append_event(conn, TEST_KEY, &blocked_event, Some(&root_hash)).unwrap();
        insert_blocked_literal(
            conn,
            &blocked_event_id.to_string(),
            blocked_arg,
            tainted_literal,
        )
        .unwrap();

        let pc = PendingConfirmation {
            effect_id,
            session_id,
            blocked_event_id,
            sink: SinkId("github.pr".into()),
            resolved_args,
            blocked_arg_names,
            combined_digest: digest,
            workspace_root_path: root_path.to_string(),
            frozen_new_oid: String::new(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
        };
        insert_pending_confirmation(conn, TEST_KEY, &pc).unwrap();
        (effect_id, session_id, blocked_event_id)
    }

    /// GITHUB-03: a `github.pr` whose `title` (and, separately, `body`) resolves
    /// from an untrusted-tainted value Blocks (BlockedPendingConfirmation), and
    /// `render_block_display` shows the tainted literal VERBATIM, marked
    /// `[BLOCKED]` — the human sees EXACTLY what would leave the boundary.
    #[test]
    fn github_pr_tainted_title_blocks_and_shows_verbatim() {
        let root = std::env::temp_dir();

        // (a) tainted TITLE — the marquee exfil vector.
        let conn = open_audit_db(":memory:").unwrap();
        let exfil_title = "Exfil AKIA-LEAKED-SECRET-0xDEADBEEF";
        let (effect_id, _session_id, _bid) =
            seed_pending_github_pr_block(&conn, "title", exfil_title, &root.to_string_lossy());
        let pc = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .expect("blocked github.pr row must exist");
        assert_eq!(
            pc.state,
            PendingConfirmationState::Pending,
            "a tainted-title github.pr must be Blocked pending confirmation"
        );
        let display = render_block_display(&pc);
        assert!(
            display.contains("Effect blocked pending confirmation"),
            "the block narration header must be shown"
        );
        assert!(
            display.contains(&format!("\"{exfil_title}\"")),
            "the tainted title literal must appear VERBATIM at confirm (GITHUB-03) — \
             the human sees exactly what would leave the boundary"
        );
        assert!(
            display.contains("title [BLOCKED]"),
            "the tainted title arg must be marked [BLOCKED]"
        );

        // (b) tainted BODY — same verbatim/blocked property on the other exfil arg.
        let conn2 = open_audit_db(":memory:").unwrap();
        let exfil_body = "Body exfil: -----BEGIN KEY----- leaked -----END KEY-----";
        let (eid2, _s2, _b2) =
            seed_pending_github_pr_block(&conn2, "body", exfil_body, &root.to_string_lossy());
        let pc2 = find_pending_confirmation(&conn2, &eid2.to_string())
            .unwrap()
            .expect("blocked github.pr (body) row must exist");
        let display2 = render_block_display(&pc2);
        assert!(
            display2.contains(&format!("\"{exfil_body}\"")),
            "the tainted body literal must appear VERBATIM at confirm (GITHUB-03)"
        );
        assert!(
            display2.contains("body [BLOCKED]"),
            "the tainted body arg must be marked [BLOCKED]"
        );
    }

    /// GITHUB-02: `confirm()` on a tainted-title github.pr block with NO live
    /// session auth-grant fails closed — a bare confirm CANNOT create a PR. The
    /// row stays Pending (fail-closed-RECOVERABLE), NO `confirm_granted` is
    /// appended (the grant gate refuses BEFORE Step 5), and NO github.pr
    /// dispatch/terminal event exists (no PR was attempted).
    #[tokio::test]
    async fn github_pr_confirm_without_grant_does_not_burn() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_ghpr_nogrant_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, _bid) = seed_pending_github_pr_block(
            &conn,
            "title",
            "Exfil no-grant path",
            &root.to_string_lossy(),
        );

        // NO record_github_grant call — the grant gate must refuse.
        let result = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await;
        assert!(
            result.is_err(),
            "confirm without a live github.pr grant must fail closed (GITHUB-02) — \
             a bare confirm cannot create a PR"
        );

        let found = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .expect("row must still exist");
        assert_eq!(
            found.state,
            PendingConfirmationState::Pending,
            "the missing-grant refusal must leave the row Pending (recoverable), \
             never a burned confirmation"
        );

        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "confirm_granted")
                .unwrap()
                .is_none(),
            "the grant gate must refuse BEFORE Step 5 appends confirm_granted — no \
             dangling confirm_granted may exist on the missing-grant path"
        );
        for terminal in [
            "github_pr_succeeded",
            "github_pr_failed",
            "github_pr_replay_suppressed",
        ] {
            assert!(
                find_event_by_type(&conn, &session_id.to_string(), terminal)
                    .unwrap()
                    .is_none(),
                "no {terminal} event may exist — a bare confirm attempts no PR"
            );
        }
        assert!(
            crate::audit::verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "the audit chain must verify throughout"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// GITHUB-02/03 + §9 (P33/P34): `confirm()` WITH a live grant proceeds to
    /// the Step-7 dispatch. `confirm_granted` IS appended AND is FOLLOWED by a
    /// terminal `github_pr_*` event (on macOS the live POST stubs Err, so the
    /// terminal event is `github_pr_failed` — that still proves the no-dangling
    /// property), the `created_prs` CAS row is reserved, and `verify_chain` is
    /// true. Mirrors the process.exec no-burn/terminal-event discipline.
    #[tokio::test]
    async fn github_pr_confirm_with_grant_proceeds_no_dangling() {
        let _guard = crate::sinks::github_pr::GITHUB_ENV_LOCK.lock().unwrap();
        std::env::set_var("CAPRUN_GITHUB_TOKEN", "ghp_test_token_for_confirm_dispatch");
        std::env::remove_var("CAPRUN_GITHUB_API_BASE");

        let mut root = std::env::temp_dir();
        root.push(format!("caprun_ghpr_grant_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, _bid) = seed_pending_github_pr_block(
            &conn,
            "title",
            "Exfil with-grant path",
            &root.to_string_lossy(),
        );

        // Record the live session auth-grant BEFORE confirm (GITHUB-02).
        crate::audit::record_github_grant(&conn, TEST_KEY, &session_id.to_string()).unwrap();

        let outcome = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await;

        std::env::remove_var("CAPRUN_GITHUB_TOKEN");

        // On the macOS POST stub the sink fails -> ConfirmedButSinkFailed (a
        // TERMINAL outcome). The load-bearing assertion is the no-dangling
        // property below, not the specific outcome.
        let outcome = outcome.expect("confirm must not error once grant + precheck pass");
        assert!(
            matches!(
                outcome,
                ConfirmOutcome::Released | ConfirmOutcome::ConfirmedButSinkFailed
            ),
            "with a grant + valid precheck confirm must dispatch (Released on a live \
             2xx, ConfirmedButSinkFailed on the macOS POST stub), got {outcome:?}"
        );

        // confirm_granted IS present (the one-shot was burned) ...
        let granted = find_event_by_type(&conn, &session_id.to_string(), "confirm_granted")
            .unwrap()
            .expect("confirm_granted must be appended once the grant + precheck pass");

        // ... AND is FOLLOWED by a terminal github.pr event (no dangling — §9).
        // On macOS that is github_pr_failed (POST stub); a live 2xx would be
        // github_pr_succeeded. Either closes the burned confirmation.
        let terminal = find_event_by_type(&conn, &session_id.to_string(), "github_pr_failed")
            .unwrap()
            .or_else(|| {
                find_event_by_type(&conn, &session_id.to_string(), "github_pr_succeeded")
                    .unwrap()
            })
            .expect(
                "a terminal github_pr_* event MUST follow confirm_granted — no dangling \
                 confirm_granted-without-terminal-event (§9 P33/P34)",
            );
        assert_eq!(
            terminal.parent_id,
            Some(granted.id),
            "the terminal github.pr event must chain directly on confirm_granted"
        );

        // The duplicate-PR CAS row was reserved (GITHUB-04) ...
        let cas_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM created_prs WHERE effect_id = ?1",
                rusqlite::params![effect_id.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cas_rows, 1, "the created_prs CAS row must be reserved");

        // ... and the chain verifies end-to-end.
        assert!(
            crate::audit::verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "verify_chain must be true throughout the confirm-release dispatch"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// §9 (P33/P34) REGRESSION — the recurring confirm-release audit-gap class
    /// this phase exists to close (mirrors
    /// `confirm_on_process_exec_malformed_args_does_not_burn_confirmation`): a
    /// `github.pr` confirm whose frozen `title` resolves EMPTY fails at the
    /// pre-burn `prepare_github_pr` precheck (present-but-empty is fail-closed)
    /// BEFORE Step 5 — even WITH a live grant. The row stays Pending and NO
    /// `confirm_granted` is appended (no dangling confirm state).
    #[tokio::test]
    async fn github_pr_confirm_malformed_precheck_does_not_burn() {
        let _guard = crate::sinks::github_pr::GITHUB_ENV_LOCK.lock().unwrap();
        std::env::set_var("CAPRUN_GITHUB_TOKEN", "ghp_test_token_malformed_precheck");
        std::env::remove_var("CAPRUN_GITHUB_API_BASE");

        let mut root = std::env::temp_dir();
        root.push(format!("caprun_ghpr_malformed_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        // A present-but-EMPTY title — passes validate_schema (name present) but
        // must fail prepare_github_pr's non-empty precheck before the burn.
        let (effect_id, session_id, _bid) =
            seed_pending_github_pr_block(&conn, "title", "   ", &root.to_string_lossy());

        // Even WITH a grant, the malformed precheck must refuse before the burn.
        crate::audit::record_github_grant(&conn, TEST_KEY, &session_id.to_string()).unwrap();

        let result = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await;

        std::env::remove_var("CAPRUN_GITHUB_TOKEN");

        assert!(
            result.is_err(),
            "confirm on a github.pr block with an empty frozen `title` must fail \
             closed at the pre-burn precheck, not silently succeed"
        );

        let found = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .expect("row must still exist");
        assert_eq!(
            found.state,
            PendingConfirmationState::Pending,
            "the precheck must refuse BEFORE Step 6's state transition — a malformed \
             arg must leave the row Pending (recoverable), not Confirmed"
        );

        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "confirm_granted")
                .unwrap()
                .is_none(),
            "the precheck must refuse BEFORE Step 5 appends confirm_granted — no \
             dangling confirm_granted with no terminal github.pr event may exist \
             (the P33/P34 audit-gap class)"
        );
        // And no CAS row / terminal event may have been created.
        for terminal in [
            "github_pr_succeeded",
            "github_pr_failed",
            "github_pr_replay_suppressed",
        ] {
            assert!(
                find_event_by_type(&conn, &session_id.to_string(), terminal)
                    .unwrap()
                    .is_none(),
                "no {terminal} event may exist — the precheck refused before dispatch"
            );
        }
        let cas_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM created_prs WHERE effect_id = ?1",
                rusqlite::params![effect_id.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            cas_rows, 0,
            "no created_prs CAS row may be reserved when the precheck refuses pre-burn"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    // ── v1.9 Phase 43: http.request.write confirm-release (HTTP-W-01, P33/P34) ──

    /// Seed a Blocked `http.request.write` PendingConfirmation whose `body`
    /// resolves from an untrusted-tainted value — the I2 exfil vector for the
    /// write egress. `url` + `method` are UserTrusted; only `body` carries
    /// `ExternalUntrusted` taint, so it Blocks under I2. All three args are
    /// present so the pre-burn `prepare_http_write` precheck passes for a
    /// non-empty body. Returns `(effect_id, session_id, blocked_event_id)`.
    fn seed_pending_http_write_block(
        conn: &rusqlite::Connection,
        body_literal: &str,
        root_path: &str,
    ) -> (Uuid, Uuid, Uuid) {
        let session_id = Uuid::new_v4();
        let effect_id = Uuid::new_v4();
        let read_event_id = Uuid::new_v4();

        let root_evt = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            chrono::Utc::now(),
            vec![],
        );
        let root_hash = append_event(conn, TEST_KEY, &root_evt, None).unwrap();

        let resolved_args = vec![
            ResolvedArg {
                name: "url".to_string(),
                value_id: ValueId::new(),
                literal: "https://write-mock.caprun.test/ingest".to_string(),
                taint: vec![TaintLabel::UserTrusted],
                provenance_chain: vec![],
            },
            ResolvedArg {
                name: "method".to_string(),
                value_id: ValueId::new(),
                literal: "POST".to_string(),
                taint: vec![TaintLabel::UserTrusted],
                provenance_chain: vec![],
            },
            ResolvedArg {
                name: "body".to_string(),
                value_id: ValueId::new(),
                literal: body_literal.to_string(),
                taint: vec![TaintLabel::ExternalUntrusted],
                provenance_chain: vec![read_event_id],
            },
        ];

        let digest = combined_digest(
            &resolved_args
                .iter()
                .map(|a| (a.name.as_str(), a.literal.as_str()))
                .collect::<Vec<_>>(),
        );
        let blocked_arg_names = vec!["body".to_string()];

        let literal_sha256 = {
            let mut h = Sha256::new();
            h.update(body_literal.as_bytes());
            hex::encode(h.finalize())
        };
        let anchor = SinkBlockedAnchor {
            effect_id,
            sink: SinkId("http.request.write".into()),
            arg: "body".into(),
            value_id: ValueId::new(),
            literal_sha256,
            taint: vec![TaintLabel::ExternalUntrusted],
            provenance_chain: vec![read_event_id],
            read_event_id,
        };
        let blocked_event = Event::sink_blocked(
            Uuid::new_v4(),
            Some(root_evt.id),
            session_id,
            chrono::Utc::now(),
            vec![anchor],
            Some(digest.clone()),
            blocked_arg_names.clone(),
        );
        let blocked_event_id = blocked_event.id;
        append_event(conn, TEST_KEY, &blocked_event, Some(&root_hash)).unwrap();
        insert_blocked_literal(conn, &blocked_event_id.to_string(), "body", body_literal).unwrap();

        let pc = PendingConfirmation {
            effect_id,
            session_id,
            blocked_event_id,
            sink: SinkId("http.request.write".into()),
            resolved_args,
            blocked_arg_names,
            combined_digest: digest,
            workspace_root_path: root_path.to_string(),
            frozen_new_oid: String::new(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
        };
        insert_pending_confirmation(conn, TEST_KEY, &pc).unwrap();
        (effect_id, session_id, blocked_event_id)
    }

    /// HTTP-W-01 + §9 (P33/P34): a tainted-body `http.request.write` Blocks, and
    /// `confirm()` releases it EXACTLY ONCE — `confirm_granted` IS appended AND
    /// is FOLLOWED by a terminal `http_write_*` event (on this host the live write
    /// stubs/gate-fails, so the terminal event is `http_write_failed` — that still
    /// proves the no-dangling property), and `verify_chain` is true. No auth-grant
    /// gate (unlike github.pr): a single confirm-releasable write (DESIGN §2).
    #[tokio::test]
    async fn confirm_on_pending_http_write_releases_exactly_once_no_dangling() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_httpw_confirm_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, _bid) = seed_pending_http_write_block(
            &conn,
            "exfil AKIA-LEAKED-SECRET-0xDEADBEEF",
            &root.to_string_lossy(),
        );

        let outcome = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await;

        // On the host write stub / empty WRITE_HOST_ALLOWLIST the sink fails ->
        // ConfirmedButSinkFailed (a TERMINAL outcome). The load-bearing assertion
        // is the no-dangling property below, not the specific outcome.
        let outcome = outcome.expect("confirm must not error once the precheck passes");
        assert!(
            matches!(
                outcome,
                ConfirmOutcome::Released | ConfirmOutcome::ConfirmedButSinkFailed
            ),
            "with a valid precheck confirm must dispatch (Released on a live 2xx, \
             ConfirmedButSinkFailed on the host write stub), got {outcome:?}"
        );

        // confirm_granted IS present (the one-shot was burned) ...
        let granted = find_event_by_type(&conn, &session_id.to_string(), "confirm_granted")
            .unwrap()
            .expect("confirm_granted must be appended once the precheck passes");

        // ... AND is FOLLOWED by a terminal http_write_* event (no dangling — §9).
        let terminal = find_event_by_type(&conn, &session_id.to_string(), "http_write_failed")
            .unwrap()
            .or_else(|| {
                find_event_by_type(&conn, &session_id.to_string(), "http_write_succeeded")
                    .unwrap()
            })
            .expect(
                "a terminal http_write_* event MUST follow confirm_granted — no dangling \
                 confirm_granted-without-terminal-event (§9 P33/P34)",
            );
        assert_eq!(
            terminal.parent_id,
            Some(granted.id),
            "the terminal http_write event must chain directly on confirm_granted"
        );

        // The row is burned Confirmed — a SECOND confirm releases nothing more.
        let second = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws)
            .await
            .expect("second confirm must not error");
        assert_eq!(
            second,
            ConfirmOutcome::AlreadyTerminal,
            "the write is confirm-releasable EXACTLY ONCE — a second confirm is a no-op"
        );

        // The chain verifies end-to-end.
        assert!(
            crate::audit::verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "verify_chain must be true throughout the confirm-release dispatch"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// §9 (P33/P34) REGRESSION — the recurring confirm-release audit-gap class
    /// (mirrors `github_pr_confirm_malformed_precheck_does_not_burn` /
    /// `confirm_on_process_exec_malformed_args_does_not_burn_confirmation`): an
    /// `http.request.write` confirm whose frozen `body` resolves EMPTY fails at
    /// the pre-burn `prepare_http_write` precheck (present-but-empty is
    /// fail-closed) BEFORE Step 5. The row stays Pending and NO `confirm_granted`
    /// is appended (no dangling confirm state, no burned one-shot).
    #[tokio::test]
    async fn confirm_on_http_write_malformed_precheck_does_not_burn() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_httpw_malformed_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        // A present-but-EMPTY body — passes validate_schema (name present) but
        // must fail prepare_http_write's non-empty precheck before the burn.
        let (effect_id, session_id, _bid) =
            seed_pending_http_write_block(&conn, "   ", &root.to_string_lossy());

        let result = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await;
        assert!(
            result.is_err(),
            "confirm on an http.request.write block with an empty frozen `body` must \
             fail closed at the pre-burn precheck, not silently succeed"
        );

        let found = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .expect("row must still exist");
        assert_eq!(
            found.state,
            PendingConfirmationState::Pending,
            "the Step-4.8c precheck must refuse BEFORE Step 6's state transition — a \
             malformed arg must leave the row Pending (recoverable), not Confirmed"
        );

        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "confirm_granted")
                .unwrap()
                .is_none(),
            "the precheck must refuse BEFORE Step 5 appends confirm_granted — no \
             dangling confirm_granted with no terminal http.request.write event may \
             exist (the P33/P34 audit-gap class)"
        );
        for terminal in ["http_write_succeeded", "http_write_failed"] {
            assert!(
                find_event_by_type(&conn, &session_id.to_string(), terminal)
                    .unwrap()
                    .is_none(),
                "no {terminal} event may exist — the precheck refused before dispatch"
            );
        }

        std::fs::remove_dir_all(&root).ok();
    }

    /// (c) deny on a fresh Pending block records a durable denial: a
    /// confirm_denied event exists, state is Denied, and a subsequent confirm
    /// refuses (durable deny, CONFIRM-03). The sink is never invoked.
    #[tokio::test]
    async fn deny_on_pending_block_is_durable() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_deny_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        let outcome = deny(&conn, TEST_KEY, &effect_id.to_string()).expect("deny");
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

        let later = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("confirm after deny");
        assert_eq!(later, ConfirmOutcome::AlreadyTerminal);
        assert!(
            !root.join("out.txt").exists(),
            "deny must permanently prevent the effect from ever running"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    // ── pending_confirmations whole-row MAC gate (Task 3, v1.6 Phase 28 Plan 05) ──

    /// A raw-SQL flip-back of a TERMINAL row's `state` back to `pending`
    /// (WITHOUT recomputing `mac`) is caught by the whole-row MAC check —
    /// distinct from Step 2's plain `!= Pending` guard, which would NOT
    /// catch this specific tamper (the flipped `state` column IS literally
    /// `"pending"` again). Real `deny()` first (so `transition_state`
    /// legitimately persists a Denied-state MAC), THEN the raw flip-back,
    /// THEN `confirm()` — proving the MAC (bound to the OLD `denied` state)
    /// rejects the row even though the SQL-level `state` column reads
    /// `pending`.
    #[tokio::test]
    async fn flip_back_denied_to_pending_caught_by_mac() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_flipback_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, _session_id, _blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        let denied = deny(&conn, TEST_KEY, &effect_id.to_string()).expect("deny");
        assert_eq!(denied, ConfirmOutcome::Denied);

        // Raw-SQL flip-back: state -> 'pending', mac left UNTOUCHED (still
        // bound to state="denied") — simulates a bare
        // pending_confirmations-table writer.
        conn.execute(
            "UPDATE pending_confirmations SET state = 'pending' WHERE effect_id = ?1",
            rusqlite::params![effect_id.to_string()],
        )
        .unwrap();

        // Sanity: the flip really did land at the SQL layer — a Step-2-only
        // guard (`pc.state != Pending`), with no MAC check, would NOT catch
        // this: the row genuinely reads back as Pending.
        let raw_state: String = conn
            .query_row(
                "SELECT state FROM pending_confirmations WHERE effect_id = ?1",
                rusqlite::params![effect_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(raw_state, "pending");

        let outcome = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("confirm");
        assert_eq!(
            outcome,
            ConfirmOutcome::DigestMismatch,
            "a raw-SQL flip-back to Pending (bypassing transition_state's mac \
             recompute) must be caught by the pending_confirmations whole-row \
             MAC check, not silently accepted"
        );
        assert!(
            !root.join("out.txt").exists(),
            "no sink must ever run after a MAC-invalid flip-back"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// `deny()`'s BRAND-NEW gate (it previously had none at all —
    /// 28-RESEARCH.md NEW FINDING) rejects a MAC-invalid `pending_
    /// confirmations` row: no state transition, no `confirm_denied` event.
    #[test]
    fn deny_fails_closed_on_tampered_state() {
        let conn = open_audit_db(":memory:").unwrap();
        let root = std::env::temp_dir().join(format!("caprun_deny_tamper_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();

        let (effect_id, session_id, _blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        // Tamper resolved_args via raw SQL (bypassing insert/transition_state's
        // mac recompute) — the SAME helper the existing confirm()-side tests
        // use, reused here for deny().
        mutate_resolved_arg_literal(&conn, effect_id, "contents", "attacker-injected");

        let outcome = deny(&conn, TEST_KEY, &effect_id.to_string()).expect("deny");
        assert_eq!(
            outcome,
            ConfirmOutcome::DigestMismatch,
            "deny() must now fail closed on a MAC-invalid pending_confirmations row"
        );

        let pc = find_pending_confirmation(&conn, &effect_id.to_string())
            .unwrap()
            .unwrap();
        assert_eq!(
            pc.state,
            PendingConfirmationState::Pending,
            "a MAC-rejected deny must never transition state"
        );
        assert_eq!(
            count_events_of_type(&conn, session_id, "confirm_denied"),
            0,
            "no confirm_denied event may be appended when deny()'s new gate rejects the row"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// (d) confirm on an effect_id whose blocked_literals row was redacted
    /// refuses to release and creates no file (T-10-09 fail-closed).
    #[tokio::test]
    async fn confirm_with_redacted_blocked_literal_refuses_to_release() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_redacted_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, _session_id, blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        redact_blocked_literal(&conn, &blocked_event_id.to_string()).unwrap();

        let outcome = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("confirm");
        assert_eq!(outcome, ConfirmOutcome::BlockedLiteralRedacted);
        assert!(
            !root.join("out.txt").exists(),
            "a redacted blocked literal must never be released"
        );

        std::fs::remove_dir_all(&root).ok();
    }

    /// (e) confirm/deny on an unknown effect_id return UnknownEffect (T-10-03).
    #[tokio::test]
    async fn confirm_and_deny_on_unknown_effect_id_return_unknown_effect() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_unknown_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let unknown = Uuid::new_v4().to_string();

        assert_eq!(
            confirm(&mut conn, TEST_KEY, &unknown, &ws).await.expect("confirm"),
            ConfirmOutcome::UnknownEffect
        );
        assert_eq!(
            deny(&conn, TEST_KEY, &unknown).expect("deny"),
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
    #[tokio::test]
    async fn confirm_fails_closed_with_digest_mismatch_when_trusted_arg_tampered() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_tamper_trusted_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, _blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        mutate_resolved_arg_literal(&conn, effect_id, "contents", "attacker-injected contents");

        let outcome = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("confirm");
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
    #[tokio::test]
    async fn confirm_fails_closed_with_digest_mismatch_when_arg_renamed_post_block() {
        let mut conn = open_audit_db(":memory:").unwrap();
        let root = std::env::temp_dir().join(format!("caprun_confirm_rename_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let (effect_id, session_id, _blocked_event_id) =
            seed_pending_email_send_block(&conn, "recipient@example.com", "hello", "body text");

        rename_resolved_arg(&conn, effect_id, "body", "cc");

        let outcome = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("confirm");
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
    #[tokio::test]
    async fn confirm_fails_closed_via_verify_chain_when_literal_and_event_digest_edited_self_consistently()
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
            !crate::audit::verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "sanity: the Event payload edit must break verify_chain"
        );

        let outcome = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("confirm");
        assert_eq!(outcome, ConfirmOutcome::DigestMismatch);
        assert!(!root.join("out.txt").exists());

        std::fs::remove_dir_all(&root).ok();
    }

    /// (d) MAJOR-7 no-fork: a DigestMismatch (row left Pending, an alarm not
    /// a deny) followed by a SECOND confirm attempt — this time with the
    /// tamper reverted, so it succeeds — leaves `audit::verify_chain` STILL
    /// TRUE throughout. The mismatch event chained onto the CURRENT HEAD
    /// (not `blocked_event_id`), so the retry never forks the DAG.
    #[tokio::test]
    async fn digest_mismatch_then_retry_does_not_fork_dag_verify_chain_stays_true() {
        let mut root = std::env::temp_dir();
        root.push(format!("caprun_confirm_mismatch_retry_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let ws = WorkspaceRoot::open(&root).unwrap();

        let mut conn = open_audit_db(":memory:").unwrap();
        let (effect_id, session_id, _blocked_event_id) =
            seed_pending_file_create_block(&conn, "out.txt", "hello", &root.to_string_lossy());

        mutate_resolved_arg_literal(&conn, effect_id, "contents", "attacker-injected");

        let first = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("first confirm");
        assert_eq!(first, ConfirmOutcome::DigestMismatch);
        assert!(
            crate::audit::verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "the chain must remain linear (verify_chain true) immediately after the mismatch append"
        );

        // The row is left Pending (an alarm, not a deny) — revert the tamper
        // and retry; the recompute now matches the ORIGINAL digest again.
        mutate_resolved_arg_literal(&conn, effect_id, "contents", "hello");
        let second = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("second confirm");
        assert_eq!(second, ConfirmOutcome::Released);
        assert!(
            crate::audit::verify_chain(&conn, &session_id.to_string(), TEST_KEY),
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
        let root_hash = append_event(conn, TEST_KEY, &root, None).unwrap();

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
        append_event(conn, TEST_KEY, &blocked_event, Some(&root_hash)).unwrap();
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
            frozen_new_oid: String::new(),
            state: PendingConfirmationState::Pending,
            mac: String::new(),
        };
        insert_pending_confirmation(conn, TEST_KEY, &pc).unwrap();

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
    #[tokio::test]
    async fn confirm_email_send_twice_records_exactly_one_attempted_event() {
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

        let first = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("first confirm");
        assert_eq!(
            first,
            ConfirmOutcome::Released,
            "first confirm of a Pending email.send block must Release (real send succeeded)"
        );

        let second = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("second confirm");
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
    #[tokio::test]
    async fn confirm_email_send_adapter_failure_yields_email_send_failed() {
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

        let outcome = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("confirm");
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
        let second = confirm(&mut conn, TEST_KEY, &effect_id.to_string(), &ws).await.expect("second confirm");
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
