/// audit — SQLite hash-linked audit DAG
///
/// Schema: two tables (sessions, events) with a KEYED HMAC-SHA256 chain over
/// the events table (v1.6 Phase 28, HARDEN-02). Each event row stores its own
/// MAC and its parent's MAC, forming an AUTHENTICATED, tamper-evident
/// append-only chain — authenticity now depends on the broker-owned secret
/// MAC key (`cli/caprun/src/key.rs::load_or_create_key`), not merely on
/// possession of the SQLite file. A bare `events`-table writer without the
/// key cannot recompute a valid MAC for a forged/edited row, even if they
/// rewrite every descendant row to be internally self-consistent under the
/// PUBLIC (unkeyed) algorithm — see `verify_chain`'s doc comment and
/// `self_consistent_forgery_without_key_is_rejected` below.
///
/// The MAC input is built by `mac_frame` (domain-separated + length-framed,
/// see its doc comment) over the SAME field set the prior unkeyed SHA-256
/// chain used, in the SAME order:
///   parent_hash (empty string if None) || id || session_id || event_type ||
///   payload || taint
/// — domain tag `b"caprun.audit.event.v1"`. `mac_frame` is a SHARED helper
/// (not event-specific) so Plan 04's chain-anchor MAC
/// (`b"caprun.audit.anchor.v1"`) and Plan 05's `pending_confirmations`
/// whole-row MAC (`b"caprun.audit.pending-confirmation.v1"`) reuse the exact
/// same domain-separation + length-framing discipline over their own field
/// sets — never a bare per-field `mac.update(field.as_bytes())`
/// concatenation, which is ambiguous (("ab","c") and ("a","bc") collide) and
/// would permit cross-record-type MAC replay if the same broker key is ever
/// reused across record types without a domain tag.
///
/// Append-only invariant: brokerd issues NO UPDATE or DELETE on the `events`
/// table. Only the test suite uses raw SQL to simulate tampering.
///
/// See 03-RESEARCH.md Pattern 5 for the original schema DDL, and
/// 28-RESEARCH.md / DESIGN-security-hardening.md for the keyed-chain design.

use anyhow::Result;
use hmac::{Hmac, Mac};
use runtime_core::Event;
use sha2::Sha256;

/// HMAC-SHA256 type alias — the keyed MAC primitive for the audit chain
/// (v1.6 Phase 28, HARDEN-02). `Hmac::new_from_slice` accepts a key of any
/// length (RFC 2104), though `cli/caprun/src/key.rs::load_or_create_key`
/// always produces exactly 32 bytes.
type HmacSha256 = Hmac<Sha256>;

/// Domain-separated, length-framed MAC input builder — SHARED across every
/// record type this broker-key MAC scheme covers (events here; the Plan 04
/// chain anchor and Plan 05 `pending_confirmations` whole-row MAC reuse this
/// SAME helper with their OWN domain tags).
///
/// Without domain separation + length framing, a bare concatenation of
/// fields is AMBIGUOUS: `("ab", "c")` and `("a", "bc")` hash identically
/// under plain `mac.update(field)` calls, and reusing one broker key across
/// multiple record types (events, chain anchors, pending_confirmations)
/// would let a MAC computed for one record type be replayed as a valid MAC
/// for a differently-shaped record. This function closes both holes:
///   1. `domain` is mixed in FIRST — a fixed, record-type-specific tag (e.g.
///      `b"caprun.audit.event.v1"`) that a differently-typed record never
///      shares, so a MAC computed for one domain can never verify under
///      another.
///   2. Each field in `fields` is preceded by its own 8-byte little-endian
///      length prefix before its bytes — this makes the boundary between
///      adjacent fields unambiguous, closing the `("ab","c")`/`("a","bc")`
///      collision.
pub(crate) fn mac_frame(mac: &mut HmacSha256, domain: &[u8], fields: &[&[u8]]) {
    mac.update(domain);
    for field in fields {
        mac.update(&(field.len() as u64).to_le_bytes());
        mac.update(field);
    }
}

/// Domain tag for the events-table hash-chain MAC (this file). Plan 04's
/// chain anchor and Plan 05's `pending_confirmations` whole-row MAC MUST use
/// their OWN distinct domain tags (`b"caprun.audit.anchor.v1"` /
/// `b"caprun.audit.pending-confirmation.v1"` respectively) — never this one —
/// when they reuse `mac_frame` above.
const EVENT_MAC_DOMAIN: &[u8] = b"caprun.audit.event.v1";

/// Domain tag for the `chain_anchor` row's MAC (v1.6 Phase 28 Plan 04,
/// HARDEN-02 D-04) — reserved in Plan 03's doc comments above, consumed
/// here. MUST stay distinct from `EVENT_MAC_DOMAIN` (and Plan 05's
/// `b"caprun.audit.pending-confirmation.v1"`) so a MAC computed for one
/// record type can never verify — or be replayed — as another's, per
/// `mac_frame`'s domain-separation discipline.
const ANCHOR_MAC_DOMAIN: &[u8] = b"caprun.audit.anchor.v1";

/// SQLite schema — STRICT tables enforce column type constraints.
///
/// `sessions`: broker-created session rows (id, intent, status, created_at).
/// `events`: append-only audit chain (id, parent_id, hash, parent_hash, …).
pub const SCHEMA_DDL: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id         TEXT PRIMARY KEY,
    intent_id  TEXT NOT NULL,
    status     TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS events (
    id          TEXT PRIMARY KEY,
    parent_id   TEXT,
    session_id  TEXT NOT NULL,
    event_type  TEXT NOT NULL,
    actor       TEXT NOT NULL,
    payload     TEXT NOT NULL,
    taint       TEXT NOT NULL,
    parent_hash TEXT,
    hash        TEXT NOT NULL
) STRICT;

-- Redactable side table for blocked-literal data at rest. The raw literal from a
-- `sink_blocked` event lives HERE, keyed by (event_id, arg) — NEVER in the hashed
-- `events.payload` (only its SHA-256 digest is in the anchor/chain). A `sink_blocked`
-- event carries a PLURAL anchors collection (Phase 14, D-14 Collect-then-Block), so
-- one event can have MULTIPLE blocked-literal rows, one per blocked arg — `arg` is
-- part of the primary key precisely so every blocked arg's literal is persisted, not
-- just the first (a plan node cannot have two args of the same name — validate_schema
-- already guarantees `arg` is unique within one event's blocked set). Redaction is a
-- single `DELETE FROM blocked_literals WHERE event_id = ?`, which removes ALL rows for
-- that event at once (the whole block resolves confirm/deny together): the digests
-- remain in the tamper-evident chain as proof content existed, but the literals are
-- gone. This resolves the tamper-evidence-vs-redactability conflict of storing the
-- literal in the chain.
CREATE TABLE IF NOT EXISTS blocked_literals (
    event_id TEXT NOT NULL,
    arg      TEXT NOT NULL,
    literal  TEXT NOT NULL,
    PRIMARY KEY (event_id, arg)
) STRICT;

-- Redactable/mutable side table for the durable confirm/deny checkpoint, keyed by
-- `effect_id` (the same identity as `SinkBlockedAnchor.effect_id`) — playing the
-- same role `blocked_literals` plays for the raw literal: state that must survive
-- past the blocking process's exit, kept OUT of the hashed `events.payload` column
-- so it stays mutable/redactable without breaking `verify_chain`. `caprun confirm`/
-- `caprun deny` are ALWAYS separate, later OS processes from the one that created
-- the block (DESIGN-confirmation-release.md, section: The Problem Being Solved); this
-- table is the ONLY thing that survives to resume from. `blocked_event_id` is the id
-- of the anchoring `sink_blocked` event (never an `events`-table column) — used later
-- to chain the confirm/deny event's `parent_id` and to gate on blocked_literals
-- redaction. `resolved_args` is a JSON-serialized `Vec<ResolvedArg>` blob, mirroring
-- the `events.payload` convention of serializing the whole struct to one TEXT column —
-- never a normalized child table (RESEARCH Open Question 2).
--
-- `blocked_arg_names`/`combined_digest` (Phase 16, CONFIRM-03, DESIGN-confirm-
-- binding.md Round-6) are additive: `blocked_arg_names` is a JSON-serialized
-- `Vec<String>` (DISPLAY-MARKING metadata only), and `combined_digest` is the
-- SHA-256 digest mirrored from the hashed `sink_blocked` Event payload. A
-- pre-existing DB predating these columns is widened idempotently by
-- `migrate_pending_confirmations_schema` below — `CREATE TABLE IF NOT EXISTS`
-- only ever fires on a FRESH database.
--
-- `mac` (v1.6 Phase 28 Plan 05, HARDEN-02 / X-02) is the WHOLE-ROW broker-key
-- MAC — computed/verified in `confirmation.rs` (domain tag
-- caprun.audit.pending-confirmation.v1, distinct from the events/anchor
-- domains) over every column above (effect_id, session_id, blocked_event_id,
-- sink, resolved_args, blocked_arg_names, combined_digest,
-- workspace_root_path, state), recomputed atomically with `state` by
-- `transition_state`'s own `UPDATE`. Closes the DB-writer flip-back/delete
-- gap X-02 pins uniformly for `pending_confirmations` (the SAME table
-- `confirm()`/`deny()` — the ONLY thing that survives a process restart —
-- resume from). A legacy pre-Plan-05 row's DEFAULT `''` MAC fails
-- `verify_pending_confirmation_mac`'s `hex::decode`/`verify_slice` closed by
-- construction — no special-casing needed.
--
-- `frozen_new_oid` (v1.9 Phase 44 Plan 04, GIT-02/03, WG-7) is the git.push
-- human-confirmed new-oid, frozen at insert time and covered by the whole-row
-- `mac` below — the anti-TOCTOU payload freeze (DESIGN-v1.9-egress-policy §1.6).
-- EMPTY (`''`) for every non-git.push sink (they have no frozen payload oid); a
-- pre-Phase-44 DB is widened idempotently by `migrate_pending_confirmations_
-- schema` below.
CREATE TABLE IF NOT EXISTS pending_confirmations (
    effect_id           TEXT PRIMARY KEY,
    session_id          TEXT NOT NULL,
    blocked_event_id    TEXT NOT NULL,
    sink                TEXT NOT NULL,
    resolved_args       TEXT NOT NULL,
    blocked_arg_names   TEXT NOT NULL,
    combined_digest     TEXT NOT NULL,
    workspace_root_path TEXT NOT NULL,
    state               TEXT NOT NULL,
    mac                 TEXT NOT NULL DEFAULT '',
    frozen_new_oid      TEXT NOT NULL DEFAULT ''
) STRICT;

-- Anchored/monotonic head (v1.6 Phase 28 Plan 04, HARDEN-02 D-04): a single
-- MAC'd row per session recording the CURRENT chain head (head_event_id,
-- head_hash) and the ACTUAL persisted event_count. Upserted ATOMICALLY
-- inside `append_event`, under the SAME already-held connection lock as the
-- events INSERT (never a separate, skippable call site — see
-- append_event's doc comment / 28-RESEARCH.md Pitfall 4: a panic/early
-- return between two separate calls would reopen the truncation gap this
-- table exists to close). `verify_chain` loads this row, verifies its own
-- MAC, and asserts the recomputed walk's final (id, hash) and total event
-- count match it — this is what turns tail-truncation (DELETE the last N
-- events) from invisible into detected: an attacker with bare `events`-
-- table write access cannot re-MAC this row without the broker key. A
-- session with events but NO chain_anchor row (e.g. a legacy pre-Phase-28
-- database) fails closed in verify_chain (untrusted until re-anchored)
-- rather than silently passing.
CREATE TABLE IF NOT EXISTS chain_anchor (
    session_id     TEXT PRIMARY KEY,
    head_event_id  TEXT NOT NULL,
    head_hash      TEXT NOT NULL,
    event_count    INTEGER NOT NULL,
    mac            TEXT NOT NULL
) STRICT;

-- Replay-safe CAS row for the trusted (never-blocked) Allowed-dispatch sink
-- path (v1.6 Phase 29, HARDEN-03). `idempotency_key` is the PRIMARY KEY — its
-- value is `plan_node_idempotency_key(sink, args)` below, a CONTENT-derived
-- digest over `sink.0` + sorted `(arg_name, value_id)` pairs, NEVER
-- `effect_id` (D-08/§c: `effect_id` is minted fresh via `Uuid::new_v4()` on
-- EVERY dispatch call, including every replay, so an `effect_id`-keyed CAS
-- would never fire — it is proven unsound, not merely a style choice). A
-- replayed `SubmitPlanNode` carrying the IDENTICAL resolved plan-node content
-- computes the SAME key and hits the PRIMARY KEY constraint on `INSERT`,
-- which is how the dispatch site (plan 29-02, `server.rs`) detects and
-- suppresses the duplicate send BEFORE any SMTP socket opens. `effect_id`/
-- `session_id`/`sent_at` are carried for audit legibility only (not part of
-- the CAS identity). Deliberately NOT backfilled for a legacy (pre-Phase-29)
-- database — see `migrate_sent_plan_nodes_schema`'s doc comment.
CREATE TABLE IF NOT EXISTS sent_plan_nodes (
    idempotency_key TEXT PRIMARY KEY,
    effect_id       TEXT NOT NULL,
    session_id      TEXT NOT NULL,
    sent_at         TEXT NOT NULL
) STRICT;

-- Session-scoped capability record for github.pr's bearer token (v1.8 Phase
-- 38, GITHUB-02, DESIGN-git-github-http-sinks.md §4.3, FORK 3). A bearer
-- token's authority far exceeds one PR (push / merge / cross-repo read) and
-- it is a broker-held secret, so authorizing the CREDENTIAL is a DISTINCT
-- human action from the per-effect I2 confirm of a single PR's (sink, args).
-- Mirrors the v1.4 `ConnectionRole` capability precedent: a fixed,
-- fail-closed, default-deny capability decision. Its lifetime is the Session
-- — keyed by `session_id` (PRIMARY KEY), so a grant on one session NEVER
-- leaks to another and it does NOT persist across sessions. Written ONLY by
-- the human `caprun grant` action, NEVER by the worker (the worker has no DB
-- access — same custody boundary as confirm/deny). Absent a live row here,
-- `has_github_grant` returns false and the github.pr sink Denies (§8) — a
-- bare per-effect confirm can NEVER create a PR. `grant_type` pins WHICH
-- capability ('github.pr'); `granted_at` is rfc3339 for audit legibility.
CREATE TABLE IF NOT EXISTS session_grants (
    session_id TEXT PRIMARY KEY,
    grant_type TEXT NOT NULL,
    granted_at TEXT NOT NULL
) STRICT;

-- Content-derived, at-most-once CAS row for the github.pr sink (v1.8 Phase
-- 38, GITHUB-04, DESIGN-git-github-http-sinks.md §4.5 / §10) — the replay
-- defense, mirroring HARDEN-03's `sent_plan_nodes` exactly. `idempotency_key`
-- is the PRIMARY KEY — its value is `github_pr_content_key(...)` below, a
-- CONTENT-derived digest over the RESOLVED literals (owner, repo, base, head,
-- title, body) via the vetted `combined_digest` primitive. It is NEVER a
-- `PlanNode` field and NEVER `effect_id` (D-08 / §c: `effect_id` is minted
-- fresh via `Uuid::new_v4()` on EVERY resubmit, so an `effect_id`-keyed CAS
-- catches ZERO replays — proven unsound, not a style choice). A replayed
-- github.pr carrying the IDENTICAL resolved content computes the SAME key and
-- hits the PRIMARY KEY constraint on `INSERT`, which is how the dispatch site
-- (Plans 38-04/38-05) detects and suppresses the duplicate BEFORE any GitHub
-- API socket opens. `effect_id`/`session_id`/`created_at` are carried for
-- audit legibility only (NOT part of the CAS identity). There is NO
-- clear-key-on-failure path (the crash-window MAJOR-5 accepted residual, §11:
-- a lost PR is the at-most-once fail-closed tradeoff — never a second PR).
-- Deliberately NOT backfilled for a legacy DB — see
-- `migrate_created_prs_schema`.
CREATE TABLE IF NOT EXISTS created_prs (
    idempotency_key TEXT PRIMARY KEY,
    effect_id       TEXT NOT NULL,
    session_id      TEXT NOT NULL,
    created_at      TEXT NOT NULL
) STRICT;
";

/// Idempotently widen a pre-existing `pending_confirmations` table with the
/// two Phase-16 CONFIRM-03 columns (`blocked_arg_names`, `combined_digest`).
///
/// `CREATE TABLE IF NOT EXISTS` in `SCHEMA_DDL` above only ever creates the
/// table on a FRESH database — a pre-existing `audit.db` (created before this
/// change) keeps its original 7-column shape forever unless explicitly
/// migrated here. Without this, the widened `insert_pending_confirmation`
/// would fail with "no such column" the moment `caprun confirm` reuses a
/// run's original DB (`main.rs:321`).
///
/// Gated on a `PRAGMA table_info(pending_confirmations)` column-presence
/// check — never a blind `ALTER TABLE ... ADD COLUMN`, which errors on a
/// column that already exists. This function runs on EVERY `open_audit_db`
/// call, so it MUST be safe to re-run against an already-migrated (or
/// freshly-created, already-widened) DB — verified by
/// `pending_confirmations_migration_is_idempotent` below.
///
/// A legacy row that predates this migration gets the DEFAULT values
/// (`''`/`'[]'`) — `confirm()`'s recompute-and-compare (Plan 16-02) will fail
/// its digest compare closed for such a row, which is the correct, accepted
/// behavior for data that predates the binding.
fn migrate_pending_confirmations_schema(conn: &rusqlite::Connection) -> Result<()> {
    let mut existing_columns: Vec<String> = Vec::new();
    {
        let mut stmt = conn.prepare("PRAGMA table_info(pending_confirmations)")?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            // PRAGMA table_info columns: cid, name, type, notnull, dflt_value, pk.
            let name: String = row.get(1)?;
            existing_columns.push(name);
        }
    }

    if !existing_columns.iter().any(|c| c == "blocked_arg_names") {
        conn.execute(
            "ALTER TABLE pending_confirmations ADD COLUMN blocked_arg_names TEXT NOT NULL DEFAULT '[]'",
            [],
        )?;
    }
    if !existing_columns.iter().any(|c| c == "combined_digest") {
        conn.execute(
            "ALTER TABLE pending_confirmations ADD COLUMN combined_digest TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    // v1.6 Phase 28 Plan 05 (HARDEN-02 / X-02): widen a pre-Plan-05 DB with
    // the whole-row MAC column. The DEFAULT `''` value fails
    // `verify_pending_confirmation_mac`'s `hex::decode`/`Mac::verify_slice`
    // closed for any legacy row — untrusted-until-re-confirmed, same "fail
    // closed on a migrated legacy row" discipline `chain_anchor`'s migration
    // pin uses (28-RESEARCH.md "Migration (pinned)").
    if !existing_columns.iter().any(|c| c == "mac") {
        conn.execute(
            "ALTER TABLE pending_confirmations ADD COLUMN mac TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    // v1.9 Phase 44 Plan 04 (GIT-02/03, WG-7): widen a pre-Phase-44 DB with the
    // git.push frozen-new-oid column. The DEFAULT `''` is correct for every
    // legacy non-git.push row (they carry no frozen payload oid). A legacy row's
    // stored whole-row `mac` was computed over the PRE-Phase-44 field frame (no
    // `frozen_new_oid` element); `build_pending_confirmation_mac` now frames
    // `frozen_new_oid` as an additional (last) length-framed element, so a
    // legacy row's MAC no longer verifies and it fails closed
    // (untrusted-until-re-confirmed) — the SAME accepted migration discipline
    // the `mac`/`combined_digest` widenings above use (a pending confirmation
    // in flight across a broker upgrade must be re-issued, never silently
    // trusted).
    if !existing_columns.iter().any(|c| c == "frozen_new_oid") {
        conn.execute(
            "ALTER TABLE pending_confirmations ADD COLUMN frozen_new_oid TEXT NOT NULL DEFAULT ''",
            [],
        )?;
    }
    Ok(())
}

/// Idempotently verify the `chain_anchor` table exists (v1.6 Phase 28 Plan
/// 04, HARDEN-02 D-04) — the presence-check half of a migration for a
/// pre-existing (pre-Phase-28) database.
///
/// Unlike `migrate_pending_confirmations_schema` above (which WIDENS an
/// existing table with new columns via a `PRAGMA table_info`-gated `ALTER
/// TABLE`), `chain_anchor` is a WHOLE NEW table with all five columns
/// present from its very first introduction — there is no column-widening
/// case to handle. The single DDL statement in `SCHEMA_DDL` above (run via
/// `execute_batch` on every `open_audit_db` call, BEFORE this function) is
/// already idempotent and already creates the table on a legacy database
/// missing it entirely — so this function's own job is the defensive,
/// explicit `sqlite_master`-gated presence check mirroring
/// `migrate_pending_confirmations_schema`'s idiom (verify before trusting,
/// never assume): it fails loudly (never silently) if the table is somehow
/// still absent after `SCHEMA_DDL` ran, rather than duplicating the DDL
/// statement a second time in this file.
///
/// Deliberately does NOT backfill `chain_anchor` rows for a legacy
/// database's EXISTING sessions — a pre-Phase-28 session's events predate
/// MAC authentication entirely, and per DESIGN-security-hardening.md's
/// "Migration (pinned)" rule, such a session must remain untrusted until
/// explicitly re-anchored. That fail-closed behavior is enforced by
/// `verify_chain`'s absent-anchor check, not backfilled here.
fn migrate_chain_anchor_schema(conn: &rusqlite::Connection) -> Result<()> {
    let table_exists: bool = match conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'chain_anchor'",
        [],
        |_row| Ok(()),
    ) {
        Ok(()) => true,
        Err(rusqlite::Error::QueryReturnedNoRows) => false,
        Err(e) => return Err(anyhow::Error::from(e)),
    };

    if !table_exists {
        return Err(anyhow::anyhow!(
            "chain_anchor table missing after SCHEMA_DDL ran — open_audit_db invariant violated"
        ));
    }
    Ok(())
}

/// Idempotently verify the `sent_plan_nodes` table exists (v1.6 Phase 29,
/// HARDEN-03) — the presence-check half of a migration for a pre-existing
/// (pre-Phase-29) database.
///
/// Mirrors `migrate_chain_anchor_schema` above verbatim: `sent_plan_nodes` is
/// a WHOLE NEW table with all four columns present from its very first
/// introduction — there is no column-widening case to handle. The single DDL
/// statement in `SCHEMA_DDL` above (run via `execute_batch` on every
/// `open_audit_db` call, BEFORE this function) is already idempotent and
/// already creates the table on a legacy database missing it entirely — so
/// this function's own job is the defensive, explicit `sqlite_master`-gated
/// presence check: it fails loudly (never silently) if the table is somehow
/// still absent after `SCHEMA_DDL` ran, rather than duplicating the DDL
/// statement a second time in this file.
///
/// Deliberately does NOT backfill `sent_plan_nodes` rows for a legacy
/// database's past Allowed-dispatch sends — a pre-Phase-29 send was never
/// CAS-protected in the first place (this is forward-looking hardening, not
/// retroactive), exactly analogous to `chain_anchor`'s own "no backfill" rule
/// above: a legacy replayed plan node was never protected anyway (§c), and
/// an idempotent migration that only ADDS a table cannot corrupt an existing
/// chain.
fn migrate_sent_plan_nodes_schema(conn: &rusqlite::Connection) -> Result<()> {
    let table_exists: bool = match conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'sent_plan_nodes'",
        [],
        |_row| Ok(()),
    ) {
        Ok(()) => true,
        Err(rusqlite::Error::QueryReturnedNoRows) => false,
        Err(e) => return Err(anyhow::Error::from(e)),
    };

    if !table_exists {
        return Err(anyhow::anyhow!(
            "sent_plan_nodes table missing after SCHEMA_DDL ran — open_audit_db invariant violated"
        ));
    }
    Ok(())
}

/// Idempotently verify the `session_grants` table exists (v1.8 Phase 38,
/// GITHUB-02) — the presence-check half of a migration for a pre-existing
/// (pre-Phase-38) database.
///
/// Mirrors `migrate_sent_plan_nodes_schema` / `migrate_chain_anchor_schema`
/// verbatim: `session_grants` is a WHOLE NEW table with all three columns
/// present from its very first introduction — there is no column-widening
/// case. `SCHEMA_DDL`'s `CREATE TABLE IF NOT EXISTS` (run via `execute_batch`
/// on every `open_audit_db` call, BEFORE this function) already creates it on
/// a legacy DB missing it; this function's job is the defensive, explicit
/// `sqlite_master`-gated presence check — fail loudly (never silently) if the
/// table is somehow still absent after `SCHEMA_DDL` ran.
///
/// Deliberately does NOT backfill grants for a legacy database — a grant is a
/// live, session-scoped human authorization; a pre-Phase-38 session was never
/// grant-gated and cannot be retroactively "granted" (forward-looking
/// capability, exactly like `sent_plan_nodes`'s no-backfill rule). Absence of
/// a row is the correct fail-closed default (`has_github_grant` → false).
fn migrate_session_grants_schema(conn: &rusqlite::Connection) -> Result<()> {
    let table_exists: bool = match conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'session_grants'",
        [],
        |_row| Ok(()),
    ) {
        Ok(()) => true,
        Err(rusqlite::Error::QueryReturnedNoRows) => false,
        Err(e) => return Err(anyhow::Error::from(e)),
    };

    if !table_exists {
        return Err(anyhow::anyhow!(
            "session_grants table missing after SCHEMA_DDL ran — open_audit_db invariant violated"
        ));
    }
    Ok(())
}

/// Idempotently verify the `created_prs` table exists (v1.8 Phase 38,
/// GITHUB-04) — the presence-check half of a migration for a pre-existing
/// (pre-Phase-38) database.
///
/// Mirrors `migrate_sent_plan_nodes_schema` verbatim: `created_prs` is a
/// WHOLE NEW table with all four columns present from its very first
/// introduction — there is no column-widening case. `SCHEMA_DDL`'s `CREATE
/// TABLE IF NOT EXISTS` (run BEFORE this function) already creates it on a
/// legacy DB missing it; this function's job is the defensive, explicit
/// `sqlite_master`-gated presence check — fail loudly (never silently) if the
/// table is somehow still absent after `SCHEMA_DDL` ran.
///
/// Deliberately does NOT backfill `created_prs` rows for a legacy database's
/// past sends — a pre-Phase-38 github.pr was never CAS-protected (this is
/// forward-looking hardening, not retroactive), exactly analogous to
/// `sent_plan_nodes`'s own "no backfill" rule.
fn migrate_created_prs_schema(conn: &rusqlite::Connection) -> Result<()> {
    let table_exists: bool = match conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'created_prs'",
        [],
        |_row| Ok(()),
    ) {
        Ok(()) => true,
        Err(rusqlite::Error::QueryReturnedNoRows) => false,
        Err(e) => return Err(anyhow::Error::from(e)),
    };

    if !table_exists {
        return Err(anyhow::anyhow!(
            "created_prs table missing after SCHEMA_DDL ran — open_audit_db invariant violated"
        ));
    }
    Ok(())
}

/// Record a durable, session-scoped github.pr auth-grant (v1.8 Phase 38,
/// GITHUB-02, DESIGN §4.3) AND append an OPAQUE `github_grant_authorized`
/// audit event chained on the session's current chain head.
///
/// This is the WRITE half of the NEW capability model; `has_github_grant`
/// below is the READ/gate half both dispatch paths (Plans 38-04/38-05)
/// consult. Called ONLY from the human `caprun grant` action
/// (`cli/caprun/src/main.rs`), NEVER from the worker.
///
/// Two writes under the caller's connection:
/// 1. `INSERT OR IGNORE` the `session_grants` row (`grant_type` "github.pr",
///    `granted_at` rfc3339). `INSERT OR IGNORE` makes a repeated grant on the
///    SAME session idempotent — no error, no duplicate capability (the
///    PRIMARY KEY on `session_id` collapses it).
/// 2. ONLY when the row was freshly inserted (`rows_affected == 1`), append an
///    OPAQUE `github_grant_authorized` Event (actor `grant:github.pr:{sid}`,
///    empty taint) chained on the session's current chain head via
///    `append_event`. Gating the event on the fresh insert keeps a replayed
///    grant a genuine no-op (never grows the chain with duplicate authorize
///    events), mirroring the CAS "suppress on replay" discipline.
///
/// The event payload carries NO token/secret — only the fact that the grant
/// was authorized (the token itself is handled in Plan 38-03, never persisted
/// into any audit payload here).
pub fn record_github_grant(
    conn: &rusqlite::Connection,
    key: &[u8],
    session_id: &str,
) -> Result<()> {
    let rows_affected = conn.execute(
        "INSERT OR IGNORE INTO session_grants (session_id, grant_type, granted_at) \
         VALUES (?1, ?2, ?3)",
        rusqlite::params![session_id, "github.pr", chrono::Utc::now().to_rfc3339()],
    )?;

    // Replay-suppression: only emit the authorize event on a FRESH grant.
    if rows_affected == 1 {
        let session_uuid = uuid::Uuid::parse_str(session_id)?;
        let (parent_id, parent_hash) = match current_chain_head(conn, session_id)? {
            Some((id, hash)) => (Some(id), Some(hash)),
            None => (None, None),
        };
        let event = Event::new(
            uuid::Uuid::new_v4(),
            parent_id,
            session_uuid,
            format!("grant:github.pr:{session_id}"),
            "github_grant_authorized".to_string(),
            chrono::Utc::now(),
            vec![],
        );
        append_event(conn, key, &event, parent_hash.as_deref())?;
    }
    Ok(())
}

/// Fail-closed existence check for a session's github.pr auth-grant (v1.8
/// Phase 38, GITHUB-02) — the SINGLE gate both dispatch paths (Plans
/// 38-04/38-05) consult before using the bearer token.
///
/// Returns `true` iff a `session_grants` row exists for `session_id`
/// (session-scoped: a grant on session A does NOT make this true for session
/// B). Returns `false` on a fresh DB, an ungranted session, OR any query
/// error — fail-closed by construction (`query_row`'s `QueryReturnedNoRows`
/// and every other error map to `false`, never `true`). Absent a `true`
/// here, the github.pr sink Denies (§8) — a bare confirm cannot create a PR.
pub fn has_github_grant(conn: &rusqlite::Connection, session_id: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM session_grants WHERE session_id = ?1",
        rusqlite::params![session_id],
        |_row| Ok(()),
    )
    .is_ok()
}

/// Content-derived idempotency key for a github.pr duplicate-suppression CAS
/// row (v1.8 Phase 38, GITHUB-04, DESIGN §4.5).
///
/// Built by feeding the SIX resolved `(name, literal)` pairs
/// (owner/repo/base/head/title/body) into the vetted
/// `crate::confirmation::combined_digest` primitive — REUSED deliberately, NOT
/// re-implemented and NOT a plain concatenation. Unlike
/// `plan_node_idempotency_key` (which hashes fixed-width UUID `value_id`s and
/// schema-fixed arg names, so plain concat is safe there), these six values
/// are ATTACKER-INFLUENCEABLE variable-length literals (`title`/`body`
/// especially): plain concatenation would be partition-blind
/// (`owner="ab",repo="c"` colliding with `owner="a",repo="bc"`).
/// `combined_digest`'s per-field fixed-width inner-hash discipline
/// (`sha256(name)‖sha256(literal)` at 64-hex width, then an outer hash) is
/// exactly the defense against that collision — so the key is DERIVED from
/// resolved content, is order-stable, and changes if ANY one field changes.
///
/// The key is NEVER a `PlanNode` field and NEVER `effect_id`-keyed (§4.5).
pub fn github_pr_content_key(
    owner: &str,
    repo: &str,
    base: &str,
    head: &str,
    title: &str,
    body: &str,
) -> String {
    let pairs = [
        ("owner", owner),
        ("repo", repo),
        ("base", base),
        ("head", head),
        ("title", title),
        ("body", body),
    ];
    crate::confirmation::combined_digest(&pairs)
}

/// At-most-once CAS reservation for a github.pr, committed BEFORE the GitHub
/// API call (v1.8 Phase 38, GITHUB-04) — the SINGLE INSERT-before-effect CAS
/// both dispatch sites (Plans 38-04/38-05) call, mirroring HARDEN-03's
/// `email.send` `sent_plan_nodes` `INSERT OR IGNORE ... rows_affected`
/// pattern.
///
/// `idempotency_key` MUST be a `github_pr_content_key` (content-derived).
/// Performs `INSERT OR IGNORE INTO created_prs ...` and returns
/// `rows_affected == 1`:
/// - `true`  == FRESH: this dispatch owns the effect and may open the API
///   socket.
/// - `false` == REPLAY: an identical-content submission already reserved this
///   key; the PRIMARY KEY collision is suppressed and the caller MUST NOT make
///   a second API call (at-most-once per PR content).
///
/// There is NO clear-key-on-failure path — a reserved-but-failed PR stays
/// reserved (the accepted crash-window tradeoff §11: a lost PR, never a
/// second PR).
pub fn reserve_created_pr(
    conn: &rusqlite::Connection,
    idempotency_key: &str,
    effect_id: &str,
    session_id: &str,
) -> Result<bool> {
    let rows_affected = conn.execute(
        "INSERT OR IGNORE INTO created_prs (idempotency_key, effect_id, session_id, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![
            idempotency_key,
            effect_id,
            session_id,
            chrono::Utc::now().to_rfc3339()
        ],
    )?;
    Ok(rows_affected == 1)
}

/// Content-derived idempotency key for the trusted (Allowed) sink-dispatch
/// CAS row (v1.6 Phase 29, HARDEN-03).
///
/// Computed as `SHA256( sink.0 || sorted( (arg_name, value_id) pairs ) )` —
/// content-derived from the RESOLVED plan-node handles the broker already
/// owns, NEVER from `effect_id` (D-08/§c: `effect_id` is minted fresh via
/// `Uuid::new_v4()` on every dispatch call, including every replay, so an
/// `effect_id`-keyed CAS would catch zero replays — proven unsound, not a
/// style choice).
///
/// Args are sorted by `name` before hashing (mirroring
/// `confirmation::combined_digest`'s `sort_by(|a,b| a.0.cmp(b.0))`
/// discipline) — this is the load-bearing step that buys order-invariance:
/// `plan_node_idempotency_key(sink, [a, b])` ==
/// `plan_node_idempotency_key(sink, [b, a])`.
///
/// Implementation-choice pin: this uses the SIMPLER direct-concatenation
/// shape, NOT `combined_digest`'s fixed-width-inner-hash-per-field
/// discipline. `combined_digest` hardens against a partition-blindness
/// collision (`("ab","c")` vs `("a","bc")`) because it hashes
/// ATTACKER-INFLUENCEABLE literal strings of arbitrary length. This function
/// hashes `arg.value_id.0.to_string()`, which is ALWAYS a fixed-width
/// 36-char hyphenated UUID string, and `arg.name`, which is schema-fixed
/// (`sink_schema.rs`'s `required`/`allowed` sets) — neither field has
/// variable-length attacker-controlled content, so there is no
/// partition-blindness collision risk here (29-RESEARCH.md Assumption A2).
/// The pre-hash sort is kept — it is what buys order-invariance.
///
/// # Scope caveat (D-08, stated unsoftened)
/// This key is `value_id`-SCOPED (per-plan-node), NOT resolved-literal-
/// scoped: it deliberately does NOT catch a worker that mints a NEW
/// `value_id` resolving to the identical literal (e.g. re-resolving
/// `mint_from_intent`/`mint_from_derivation` for "the same" recipient) — an
/// attacker who can mint fresh `ValueId`s can still cause N distinct sends.
/// This is out of v1.6 scope (tracked as a future effects-budget
/// obligation), not a gap in this function's own correctness.
pub(crate) fn plan_node_idempotency_key(
    sink: &runtime_core::SinkId,
    args: &[runtime_core::PlanArg],
) -> String {
    use sha2::Digest as _;
    let mut sorted: Vec<&runtime_core::PlanArg> = args.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    let mut hasher = Sha256::new();
    hasher.update(sink.0.as_bytes());
    for arg in sorted {
        hasher.update(arg.name.as_bytes());
        hasher.update(arg.value_id.0.to_string().as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// Open (or create) the audit database at `path` and run schema DDL.
///
/// Uses rusqlite with the `bundled` feature (no system SQLite dep).
/// WAL mode is enabled for concurrent read access.
///
/// Pass `":memory:"` for an in-process, ephemeral DB suitable for tests.
///
/// # Arguments
/// * `path` — filesystem path for the SQLite file, or `":memory:"`.
pub fn open_audit_db(path: &str) -> Result<rusqlite::Connection> {
    let conn = rusqlite::Connection::open(path)?;
    conn.execute_batch(SCHEMA_DDL)?;
    migrate_pending_confirmations_schema(&conn)?;
    migrate_chain_anchor_schema(&conn)?;
    migrate_sent_plan_nodes_schema(&conn)?;
    migrate_session_grants_schema(&conn)?;
    migrate_created_prs_schema(&conn)?;
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    Ok(conn)
}

/// Persist the raw blocked literal for ONE blocked arg of a `sink_blocked` event
/// into the redactable `blocked_literals` side table, keyed by (event id, arg).
///
/// The literal lives ONLY here — never in `events.payload` (the hashed anchor
/// carries only its SHA-256 digest). This keeps the raw literal (attacker content
/// / PII) out of the tamper-evident chain so it can later be redacted without
/// breaking `verify_chain`. A plural (Phase 14) block calls this ONCE PER anchor
/// in the event's `anchors` collection — every blocked arg's literal is persisted,
/// not just the first. Caller should invoke this under the same broker-owned
/// connection lock as the `append_event` that wrote the `sink_blocked` row.
pub fn insert_blocked_literal(
    conn: &rusqlite::Connection,
    event_id: &str,
    arg: &str,
    literal: &str,
) -> Result<()> {
    conn.execute(
        "INSERT INTO blocked_literals (event_id, arg, literal) VALUES (?1, ?2, ?3)",
        rusqlite::params![event_id, arg, literal],
    )?;
    Ok(())
}

/// Fetch a raw literal for a blocked event, or `None` if none was ever stored or
/// all have been redacted. Verify tamper-evidence by comparing `sha256(literal)`
/// to the matching anchor's `literal_sha256`.
///
/// This is a presence/redaction-gate check (`confirmation::confirm`'s Step 3):
/// for a single-blocked-arg event (today's only exercised shape) this returns
/// THE literal. For a genuinely-plural block it returns an arbitrary one of the
/// surviving rows — sufficient for the presence check (redaction removes every
/// row for the event at once, so "any row survives" is a well-defined, atomic
/// property), but NOT a stable per-arg accessor; a dedicated per-arg query is
/// Phase 16 scope alongside the rest of multi-arg narration (CONFIRM-04).
pub fn get_blocked_literal(conn: &rusqlite::Connection, event_id: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT literal FROM blocked_literals WHERE event_id = ?1")?;
    let mut rows = stmt.query(rusqlite::params![event_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get(0)?)),
        None => Ok(None),
    }
}

/// Redact the raw literal for a blocked event: delete its `blocked_literals` row.
///
/// This does NOT touch the `events` chain — the anchor's `literal_sha256` digest
/// remains, so `verify_chain` still passes and the audit record still proves that
/// content of that digest was blocked. Returns the number of rows removed (0 if
/// already absent). Redaction is intentionally idempotent.
pub fn redact_blocked_literal(conn: &rusqlite::Connection, event_id: &str) -> Result<usize> {
    let removed = conn.execute(
        "DELETE FROM blocked_literals WHERE event_id = ?1",
        rusqlite::params![event_id],
    )?;
    Ok(removed)
}

/// Fetch the stored `hash` for a known event id, or `None` if no event with that
/// id exists.
///
/// Lets a later confirm/deny process fetch the anchoring `sink_blocked` event's
/// hash so it can append its confirm/deny event with the correct `parent_hash`
/// (keeping `verify_chain` linear), without needing the full deserialized `Event`.
pub fn event_hash_by_id(conn: &rusqlite::Connection, event_id: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare("SELECT hash FROM events WHERE id = ?1")?;
    let mut rows = stmt.query(rusqlite::params![event_id])?;
    match rows.next()? {
        Some(row) => Ok(Some(row.get(0)?)),
        None => Ok(None),
    }
}

/// Compute the keyed HMAC-SHA256 MAC for an audit event row (v1.6 Phase 28,
/// HARDEN-02).
///
/// Input is `mac_frame`'s domain-separated, length-framed encoding (domain
/// tag `EVENT_MAC_DOMAIN`) over the ordered field set:
///   parent_hash (empty string if None), id, session_id, event_type,
///   payload, taint
/// — the SAME field set and order the prior unkeyed SHA-256 chain used.
///
/// `key` is the FIRST parameter (must_haves truth). Two different keys over
/// IDENTICAL fields produce DIFFERENT digests (Success Criterion 2) — an
/// events-table writer without the key cannot recompute a valid MAC.
///
/// Returns hex-encoded lowercase HMAC-SHA256 digest.
pub fn compute_event_hash(
    key: &[u8],
    parent_hash: Option<&str>,
    id: &str,
    session_id: &str,
    event_type: &str,
    payload: &str,
    taint: &str,
) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key)
        .expect("HMAC-SHA256 accepts a key of any length");
    mac_frame(
        &mut mac,
        EVENT_MAC_DOMAIN,
        &[
            parent_hash.unwrap_or("").as_bytes(),
            id.as_bytes(),
            session_id.as_bytes(),
            event_type.as_bytes(),
            payload.as_bytes(),
            taint.as_bytes(),
        ],
    );
    hex::encode(mac.finalize().into_bytes())
}

/// Constant-time keyed verification of an event row's MAC (v1.6 Phase 28,
/// HARDEN-02) — reconstructs the SAME `mac_frame` input `compute_event_hash`
/// builds, then compares against `expected_hex` via `Mac::verify_slice`
/// (constant-time; NEVER a `==`/`!=` hex-string compare, which would leak
/// timing information about how many leading bytes matched).
///
/// Returns `false` (never panics, never `Result`) on:
///   - a hex-decode failure of `expected_hex` (malformed/corrupt stored MAC),
///   - a MAC mismatch under `key` (tampering, wrong key, or an unkeyed/
///     legacy row that predates this scheme).
///
/// Fail-closed by construction: every error path returns `false`, never
/// `true`.
pub fn verify_event_hash(
    key: &[u8],
    expected_hex: &str,
    parent_hash: Option<&str>,
    id: &str,
    session_id: &str,
    event_type: &str,
    payload: &str,
    taint: &str,
) -> bool {
    let expected_bytes = match hex::decode(expected_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let mut mac = match <HmacSha256 as Mac>::new_from_slice(key) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac_frame(
        &mut mac,
        EVENT_MAC_DOMAIN,
        &[
            parent_hash.unwrap_or("").as_bytes(),
            id.as_bytes(),
            session_id.as_bytes(),
            event_type.as_bytes(),
            payload.as_bytes(),
            taint.as_bytes(),
        ],
    );
    mac.verify_slice(&expected_bytes).is_ok()
}

/// Compute a `chain_anchor` row's MAC (v1.6 Phase 28 Plan 04, HARDEN-02
/// D-04) — domain-separated (`ANCHOR_MAC_DOMAIN`, distinct from
/// `EVENT_MAC_DOMAIN`) via the shared `mac_frame` helper, over
/// `session_id`, `head_event_id`, `head_hash`, and `event_count` (encoded
/// as its decimal string; `mac_frame`'s length-framing already makes each
/// field's boundary unambiguous, so no fixed-width integer encoding is
/// needed). Binding `event_count` INTO the MAC — not just the head hash —
/// is what lets `verify_chain` detect a tail-truncation replay: a MAC'd
/// head hash alone does not prove the chain wasn't shortened and re-
/// anchored at an earlier legitimate point.
fn compute_anchor_mac(
    key: &[u8],
    session_id: &str,
    head_event_id: &str,
    head_hash: &str,
    event_count: i64,
) -> String {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key)
        .expect("HMAC-SHA256 accepts a key of any length");
    mac_frame(
        &mut mac,
        ANCHOR_MAC_DOMAIN,
        &[
            session_id.as_bytes(),
            head_event_id.as_bytes(),
            head_hash.as_bytes(),
            event_count.to_string().as_bytes(),
        ],
    );
    hex::encode(mac.finalize().into_bytes())
}

/// Constant-time verification of a `chain_anchor` row's MAC — mirrors
/// `verify_event_hash`'s fail-closed contract (never panics, always
/// `Mac::verify_slice`, never a `==`/`!=` hex-string compare).
fn verify_anchor_mac(
    key: &[u8],
    expected_hex: &str,
    session_id: &str,
    head_event_id: &str,
    head_hash: &str,
    event_count: i64,
) -> bool {
    let expected_bytes = match hex::decode(expected_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let mut mac = match <HmacSha256 as Mac>::new_from_slice(key) {
        Ok(m) => m,
        Err(_) => return false,
    };
    mac_frame(
        &mut mac,
        ANCHOR_MAC_DOMAIN,
        &[
            session_id.as_bytes(),
            head_event_id.as_bytes(),
            head_hash.as_bytes(),
            event_count.to_string().as_bytes(),
        ],
    );
    mac.verify_slice(&expected_bytes).is_ok()
}

/// Append an event to the audit DAG and return its SHA-256 hash.
///
/// Reuses `runtime_core::Event` directly — no duplicate type definition.
/// The full event is serialized to JSON for the `payload` column; `event.taint`
/// is serialized separately for the `taint` column.
///
/// # Arguments
/// * `conn` — open rusqlite connection (broker-owned; never shared with workers).
/// * `key` — the broker-owned MAC key (v1.6 Phase 28, HARDEN-02) — the SAME
///   key `cli/caprun/src/key.rs::load_or_create_key` returns, threaded
///   through every production call site (never regenerated per-call).
/// * `event` — the event to persist.
/// * `parent_hash` — hash of the parent event row (`None` for session-root events).
///
/// # Append-only invariant
/// This function only ever issues `INSERT`. No UPDATE or DELETE on `events`.
pub fn append_event(
    conn: &rusqlite::Connection,
    key: &[u8],
    event: &Event,
    parent_hash: Option<&str>,
) -> Result<String> {
    // Defect B guard (DESIGN §4 rule 7): a `sink_blocked` event with no anchors is
    // a security-meaningless bare marker. Reject it here so it is NON-PERSISTABLE
    // through the TCB — not merely never-triggered. Plural (Phase 14): the guard
    // now checks the whole collection is non-empty, not a singular `Option`.
    if event.event_type == "sink_blocked" && event.anchors.is_empty() {
        return Err(anyhow::anyhow!(
            "sink_blocked event requires at least one anchor (Defect B guard)"
        ));
    }
    let payload = serde_json::to_string(event)?;
    let taint_str = serde_json::to_string(&event.taint)?;
    let hash = compute_event_hash(
        key,
        parent_hash,
        &event.id.to_string(),
        &event.session_id.to_string(),
        &event.event_type,
        &payload,
        &taint_str,
    );
    conn.execute(
        "INSERT INTO events \
         (id, parent_id, session_id, event_type, actor, payload, taint, parent_hash, hash) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![
            event.id.to_string(),
            event.parent_id.map(|id| id.to_string()),
            event.session_id.to_string(),
            &event.event_type,
            &event.actor,
            &payload,
            &taint_str,
            parent_hash,
            &hash,
        ],
    )?;

    // Atomically upsert the MAC'd `chain_anchor` row for this session, under
    // the SAME already-held `conn` lock as the events INSERT above (v1.6
    // Phase 28 Plan 04, HARDEN-02 D-04) — every one of the 19 production
    // `append_event` call sites inherits this for free; NO second call site
    // is ever added anywhere else (28-RESEARCH.md Pitfall 4: a separate,
    // caller-invoked step could be skipped by a panic/early-return between
    // the two calls, reopening the truncation gap this table exists to
    // close — mirrors `quarantine.rs::mint_from_read`'s two-write same-lock
    // atomicity discipline).
    let session_id_str = event.session_id.to_string();
    // Read back the ACTUAL persisted count — never assume `+ 1` arithmetic
    // (28-RESEARCH.md Anti-Pattern: the MAC must cover a CONFIRMED value).
    let event_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM events WHERE session_id = ?1",
        rusqlite::params![session_id_str],
        |row| row.get(0),
    )?;
    let event_id_str = event.id.to_string();
    let anchor_mac = compute_anchor_mac(key, &session_id_str, &event_id_str, &hash, event_count);
    conn.execute(
        "INSERT INTO chain_anchor (session_id, head_event_id, head_hash, event_count, mac) \
         VALUES (?1, ?2, ?3, ?4, ?5) \
         ON CONFLICT(session_id) DO UPDATE SET \
             head_event_id = excluded.head_event_id, \
             head_hash = excluded.head_hash, \
             event_count = excluded.event_count, \
             mac = excluded.mac",
        rusqlite::params![session_id_str, event_id_str, &hash, event_count, anchor_mac],
    )?;

    Ok(hash)
}

/// Return all Events recorded for `session_id`, ordered by insertion (rowid).
///
/// Each row's `payload` column contains the full serialized `Event` (written by
/// `append_event`). Deserializes each payload back into a `runtime_core::Event`
/// so the returned Events carry their original `taint` labels intact.
///
/// # Arguments
/// * `conn` — open rusqlite connection.
/// * `session_id` — the UUID of the session to query (as a string).
pub fn query_events_by_session(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Vec<Event>> {
    let mut stmt = conn.prepare(
        "SELECT payload FROM events WHERE session_id = ?1 ORDER BY rowid",
    )?;
    let events = stmt
        .query_map(rusqlite::params![session_id], |row| {
            let payload: String = row.get(0)?;
            Ok(payload)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    events
        .into_iter()
        .map(|p| serde_json::from_str::<Event>(&p).map_err(anyhow::Error::from))
        .collect()
}

/// Return every `pending_confirmations` row still awaiting a human decision for
/// `session_id`, as `(effect_id, sink)` pairs ordered by insertion (rowid).
///
/// # READ-ONLY POSTURE (WG-5, Matt #2)
///
/// A pure read: it SELECTs `effect_id, sink` from `pending_confirmations` WHERE
/// `session_id = ?1 AND state = 'pending'` and NOTHING else — mints no
/// ValueRecord, appends no audit event, performs no MAC transition, opens no
/// sink. Mirrors the read-only posture of `query_events_by_session` above. It
/// exists so the `caprun run` parent can surface each blocked `effect_id` + its
/// `sink` + an actionable `caprun review/confirm/deny <effect_id> <db>` pointer
/// after an I2 Block, closing the operator loop without touching state.
///
/// The `AND state = 'pending'` guard mirrors `find_pending_confirmation`'s own
/// filter: a row already confirmed/denied (terminal) is excluded, so the
/// operator is only ever pointed at effects that still have a live decision.
///
/// # Arguments
/// * `conn` — open rusqlite connection.
/// * `session_id` — the UUID of the session to query (as a string).
pub fn list_pending_confirmations_for_session(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Vec<(uuid::Uuid, String)>> {
    let mut stmt = conn.prepare(
        "SELECT effect_id, sink FROM pending_confirmations \
         WHERE session_id = ?1 AND state = 'pending' \
         ORDER BY rowid",
    )?;
    let rows = stmt
        .query_map(rusqlite::params![session_id], |row| {
            let effect_id: String = row.get(0)?;
            let sink: String = row.get(1)?;
            Ok((effect_id, sink))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    // Parse each effect_id string into a Uuid outside the row closure (rusqlite's
    // FromSql for Uuid is feature-gated; a manual parse keeps the query dependency-
    // free and fails closed on a malformed id rather than silently dropping it).
    rows.into_iter()
        .map(|(effect_id, sink)| {
            let id = uuid::Uuid::parse_str(&effect_id)
                .map_err(|e| anyhow::anyhow!("pending_confirmations.effect_id not a UUID: {e}"))?;
            Ok((id, sink))
        })
        .collect()
}

/// Locate the first Event of `event_type` within `session_id`.
///
/// Deserializes the `payload` column so the returned Event's `taint` reflects
/// what was originally persisted — required by the §9 held-out test to assert
/// the taint chain anchor exists in the DAG.
///
/// Returns `None` if no matching event exists for the session.
///
/// # Arguments
/// * `conn` — open rusqlite connection.
/// * `session_id` — the UUID of the session to search (as a string).
/// * `event_type` — the event_type string to match (e.g., `"file_read"`, `"email_send_stub"`).
pub fn find_event_by_type(
    conn: &rusqlite::Connection,
    session_id: &str,
    event_type: &str,
) -> Result<Option<Event>> {
    let result: std::result::Result<String, rusqlite::Error> = conn.query_row(
        "SELECT payload FROM events \
         WHERE session_id = ?1 AND event_type = ?2 \
         ORDER BY rowid \
         LIMIT 1",
        rusqlite::params![session_id, event_type],
        |row| row.get(0),
    );
    match result {
        Ok(payload) => {
            let event = serde_json::from_str::<Event>(&payload)?;
            Ok(Some(event))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::Error::from(e)),
    }
}

/// Locate a SPECIFIC Event by its exact `id` within `session_id`.
///
/// Unlike `find_event_by_type` (first-of-type only, `LIMIT 1`), this resolves
/// exactly ONE event by its unique primary key (`id`), scoped additionally to
/// `session_id` for defense in depth (a cross-session id must never resolve).
/// This is the accessor the EXTRACT-02 per-anchor audit walk needs: once a
/// session has ≥2 events of the same `event_type` (true as soon as multi-field
/// extraction produces ≥2 `file_read` events), `find_event_by_type` can no
/// longer disambiguate WHICH one a given `provenance_chain[i]` refers to
/// (15-RESEARCH.md Pitfall 3).
///
/// Returns `None` if no event with that id exists, or if it exists but under a
/// DIFFERENT session_id (session-scoped, never leaks cross-session events).
///
/// # Arguments
/// * `conn` — open rusqlite connection.
/// * `session_id` — the UUID of the session to search (as a string).
/// * `id` — the exact event id to resolve (as a string).
pub fn find_event_by_id(
    conn: &rusqlite::Connection,
    session_id: &str,
    id: &str,
) -> Result<Option<Event>> {
    let result: std::result::Result<String, rusqlite::Error> = conn.query_row(
        "SELECT payload FROM events \
         WHERE id = ?1 AND session_id = ?2",
        rusqlite::params![id, session_id],
        |row| row.get(0),
    );
    match result {
        Ok(payload) => {
            let event = serde_json::from_str::<Event>(&payload)?;
            Ok(Some(event))
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(anyhow::Error::from(e)),
    }
}

/// Return the session's LEAF event — the event that is no other event's
/// `parent_id` within this `session_id` — as `(id, hash)`, or `None` if the
/// session has no events at all.
///
/// On a LINEAR chain (the only shape `append_event`'s single-caller-at-a-time
/// discipline is meant to produce) this is exactly one row: the last event
/// appended. Callers that continue the session's causal chain (`confirm()`,
/// `deny()` — Plan 16-02, MAJOR-7) MUST parent their next appended event on
/// THIS head, never on a mid-chain id like `blocked_event_id` — parenting on
/// a mid-chain id makes that id have two children (a FORK), which
/// `verify_chain`'s single-linear-chain recursive-CTE walk cannot traverse
/// past (empirically discovered in `quarantine.rs`'s `mint_from_read`
/// `chain_head_id`/`chain_head_hash` threading, mirrored here).
///
/// If the chain is ALREADY forked (more than one leaf), this query can return
/// more than one row; `ORDER BY rowid DESC LIMIT 1` picks the most-recently
/// -appended one so the function still returns a single answer, but a forked
/// chain has already failed `verify_chain` — callers MUST gate on
/// `verify_chain` FIRST (Plan 16-02's `confirm()` does exactly this) so this
/// function is only ever consulted against a chain already known to be
/// linear.
pub fn current_chain_head(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> Result<Option<(uuid::Uuid, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, hash FROM events \
         WHERE session_id = ?1 AND id NOT IN ( \
             SELECT parent_id FROM events \
             WHERE session_id = ?1 AND parent_id IS NOT NULL \
         ) \
         ORDER BY rowid DESC LIMIT 1",
    )?;
    let mut rows = stmt.query(rusqlite::params![session_id])?;
    match rows.next()? {
        Some(row) => {
            let id: String = row.get(0)?;
            let hash: String = row.get(1)?;
            Ok(Some((uuid::Uuid::parse_str(&id)?, hash)))
        }
        None => Ok(None),
    }
}

/// Walk the audit chain for `session_id` and verify every keyed MAC link,
/// then cross-check the MAC'd `chain_anchor` row (v1.6 Phase 28, HARDEN-02;
/// Plan 04, D-04 — the anchor cross-check is what turns tail-truncation
/// from invisible into detected).
///
/// Uses a recursive CTE to traverse from the root event (parent_id IS NULL)
/// through each subsequent event linked by `parent_id`. For each row:
///
/// 1. Asserts that `stored.parent_hash` matches the hash of the previous event.
/// 2. Recomputes the keyed MAC via `verify_event_hash` (constant-time —
///    `Mac::verify_slice`, never a `==`/`!=` hex-string compare) from stored
///    fields under `key`.
///
/// After the walk completes, the NEW (Plan 04) anchor cross-check runs:
///
/// 3. Loads the session's `chain_anchor` row. **Absent → `false`** (a
///    session with events but NO anchor row — e.g. a legacy pre-Phase-28
///    database — is untrusted until re-anchored; DESIGN's "Migration
///    (pinned)" rule, fail-closed by construction, never silently trusted).
/// 4. Verifies the anchor row's OWN MAC (constant-time, `verify_anchor_mac`)
///    under `key`. Mismatch → `false`.
/// 5. Asserts the walk's final `(id, hash)` and total row COUNT equal the
///    anchor's `head_event_id`/`head_hash`/`event_count`. Mismatch →
///    `false` — this is what catches a tail-truncation (`DELETE` the last N
///    events via raw SQL, bypassing `append_event` so the anchor is left
///    stale): the walk now terminates at a SHORTER true leaf that no longer
///    matches the anchor an attacker without the broker key cannot re-MAC.
///
/// Returns `false` if:
/// - Any MAC mismatch is detected (tampering, wrong key, or an unkeyed/
///   legacy row from before this scheme — fail-closed, not silently passed).
/// - Any `parent_hash` link is broken.
/// - No events exist for the session (empty chain is considered unverified).
/// - The session's `chain_anchor` row is absent, MAC-invalid, or its
///   head/count no longer matches the recomputed walk (tail-truncation).
///
/// Authenticity now depends on `key` (the broker-owned secret MAC key): a
/// bare `events`-table writer without `key` cannot forge a self-consistent
/// chain that verifies here, even if every descendant row is rewritten to be
/// internally consistent under the PUBLIC (unkeyed) algorithm (Success
/// Criterion 1) — see `self_consistent_forgery_without_key_is_rejected`.
/// Calling with the WRONG key on an otherwise-untampered chain also returns
/// `false` (Success Criterion 2) — see `verify_chain_is_key_dependent`. A
/// genuinely untampered, normally-appended chain still verifies `true` — no
/// false positive — since `append_event` atomically upserts a matching
/// anchor with every append (Plan 04 Task 1).
pub fn verify_chain(conn: &rusqlite::Connection, session_id: &str, key: &[u8]) -> bool {
    let result: Result<bool> = (|| {
        let mut stmt = conn.prepare(
            "WITH RECURSIVE chain(
                 id, session_id, event_type, payload, taint, parent_hash, hash, depth
             ) AS (
                 -- Root: event with no causal predecessor
                 SELECT id, session_id, event_type, payload, taint, parent_hash, hash, 0
                 FROM events
                 WHERE session_id = ?1 AND parent_id IS NULL
               UNION ALL
                 -- Recursive step: find the next event in the chain
                 SELECT e.id, e.session_id, e.event_type, e.payload, e.taint,
                        e.parent_hash, e.hash, c.depth + 1
                 FROM events e
                 JOIN chain c ON e.parent_id = c.id
                 WHERE e.session_id = ?1
             )
             SELECT id, session_id, event_type, payload, taint, parent_hash, hash
             FROM chain
             ORDER BY depth",
        )?;

        let mut rows = stmt.query(rusqlite::params![session_id])?;
        let mut prev_hash: Option<String> = None;
        let mut found_any = false;
        let mut last_id: Option<String> = None;
        let mut walked_count: i64 = 0;

        while let Some(row) = rows.next()? {
            found_any = true;
            walked_count += 1;
            let id: String = row.get(0)?;
            let sid: String = row.get(1)?;
            let event_type: String = row.get(2)?;
            let payload: String = row.get(3)?;
            let taint: String = row.get(4)?;
            let stored_parent_hash: Option<String> = row.get(5)?;
            let stored_hash: String = row.get(6)?;

            // Check parent_hash linkage: must equal the previous event's hash
            if stored_parent_hash != prev_hash {
                return Ok(false);
            }

            // Constant-time keyed MAC verify (never a `==`/`!=` hex compare).
            if !verify_event_hash(
                key,
                &stored_hash,
                stored_parent_hash.as_deref(),
                &id,
                &sid,
                &event_type,
                &payload,
                &taint,
            ) {
                return Ok(false);
            }

            last_id = Some(id);
            prev_hash = Some(stored_hash);
        }

        if !found_any {
            return Ok(false);
        }

        // Anchor cross-check (Plan 04, D-04): load the session's
        // chain_anchor row — absent means untrusted/legacy, fail closed.
        let anchor_row: Option<(String, String, i64, String)> = match conn.query_row(
            "SELECT head_event_id, head_hash, event_count, mac \
             FROM chain_anchor WHERE session_id = ?1",
            rusqlite::params![session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ) {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(anyhow::Error::from(e)),
        };

        let (anchor_head_event_id, anchor_head_hash, anchor_event_count, anchor_mac) =
            match anchor_row {
                Some(v) => v,
                None => return Ok(false), // fail-closed: no anchor => untrusted
            };

        // Constant-time keyed MAC verify of the anchor row itself.
        if !verify_anchor_mac(
            key,
            &anchor_mac,
            session_id,
            &anchor_head_event_id,
            &anchor_head_hash,
            anchor_event_count,
        ) {
            return Ok(false);
        }

        // Orphan/extra-row guard (HARDEN-02): the anchored count must also equal
        // the LIVE row count for this session. The recursive walk above only
        // counts events reachable from the single NULL-parent root; an actor
        // with `events`-table write access could INSERT an unreferenced row
        // (a non-NULL `parent_id` pointing nowhere) that the walk skips. Such a
        // row carries a forged MAC (the attacker lacks the key) and never enters
        // a confirm/deny decision path, but an *authenticated* chain must not
        // silently carry unreferenced rows — fail closed if the table holds more
        // events for this session than the chain walk reached.
        let live_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM events WHERE session_id = ?1",
            rusqlite::params![session_id],
            |row| row.get(0),
        )?;
        if live_count != walked_count {
            return Ok(false);
        }

        // The walk's final (id, hash) and total row count must equal the
        // anchor's — a stale anchor (tail-truncation via raw SQL bypassing
        // append_event) fails here.
        let final_id = last_id.expect("found_any guarantees at least one row was walked");
        let final_hash = prev_hash.expect("found_any guarantees at least one row was walked");
        if final_id != anchor_head_event_id
            || final_hash != anchor_head_hash
            || walked_count != anchor_event_count
        {
            return Ok(false);
        }

        Ok(true)
    })();

    result.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use runtime_core::TaintLabel;
    use uuid::Uuid;

    /// Fixed, non-secret test MAC key — these unit tests exercise chain
    /// mechanics (append/query/verify), not key custody (that's
    /// `cli/caprun/src/key.rs`'s job) — a stable byte string is sufficient
    /// and keeps every test in this module using ONE consistent key unless a
    /// test explicitly wants a DIFFERENT one (key-dependence tests below).
    const TEST_KEY: &[u8] = b"audit-rs-unit-test-key-not-secret-32b";

    /// Build a minimal file_read Event with taint labels [ExternalUntrusted, EmailRaw].
    fn make_file_read_event(session_id: Uuid) -> Event {
        Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "worker".to_string(),
            "file_read".to_string(),
            Utc::now(),
            vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw],
        )
    }

    #[test]
    fn find_event_by_type_returns_event_with_taint_intact() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let event = make_file_read_event(session_id);
        let event_id = event.id;

        append_event(&conn, TEST_KEY, &event, None).expect("append_event");

        let found = find_event_by_type(&conn, &session_id.to_string(), "file_read")
            .expect("find_event_by_type")
            .expect("event should be present");

        assert_eq!(found.id, event_id);
        assert_eq!(
            found.taint,
            vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw],
            "taint must survive the payload round-trip"
        );
    }

    #[test]
    fn find_event_by_type_returns_none_for_missing_type() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let event = make_file_read_event(session_id);

        append_event(&conn, TEST_KEY, &event, None).expect("append_event");

        let found = find_event_by_type(&conn, &session_id.to_string(), "email_send_stub")
            .expect("find_event_by_type");

        assert!(found.is_none(), "no email_send_stub event should be found");
    }

    /// On a linear chain, `current_chain_head` returns the LAST-appended
    /// event's (id, hash) — never an earlier event, even though the root
    /// event still exists in the same `events` table (Plan 16-02, MAJOR-7).
    #[test]
    fn current_chain_head_returns_last_appended_event_on_linear_chain() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();

        let root = make_file_read_event(session_id);
        let root_hash = append_event(&conn, TEST_KEY, &root, None).expect("append root");

        let child = Event::new(
            Uuid::new_v4(),
            Some(root.id),
            session_id,
            "worker".to_string(),
            "sink_ok".to_string(),
            Utc::now(),
            vec![],
        );
        let child_hash = append_event(&conn, TEST_KEY, &child, Some(&root_hash)).expect("append child");

        let head = current_chain_head(&conn, &session_id.to_string())
            .expect("current_chain_head")
            .expect("a linear chain must have exactly one leaf");
        assert_eq!(
            head,
            (child.id, child_hash),
            "current_chain_head must return the LAST-appended event, not the root"
        );
    }

    /// A session with no events at all returns `None` (no leaf to report).
    #[test]
    fn current_chain_head_returns_none_for_session_with_no_events() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();

        let head = current_chain_head(&conn, &session_id.to_string()).expect("current_chain_head");
        assert!(head.is_none());
    }

    /// find_event_by_id disambiguates among TWO events of the SAME event_type
    /// in one session — resolving EACH to its own distinct event, which
    /// `find_event_by_type` (LIMIT 1) cannot do (15-RESEARCH.md Pitfall 3).
    #[test]
    fn find_event_by_id_disambiguates_two_same_type_events() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();

        let event_a = make_file_read_event(session_id);
        let event_b = make_file_read_event(session_id);
        assert_ne!(event_a.id, event_b.id, "sanity: distinct event ids");

        append_event(&conn, TEST_KEY, &event_a, None).expect("append event_a");
        append_event(&conn, TEST_KEY, &event_b, None).expect("append event_b");

        let found_a = find_event_by_id(&conn, &session_id.to_string(), &event_a.id.to_string())
            .expect("find_event_by_id a")
            .expect("event_a must resolve");
        let found_b = find_event_by_id(&conn, &session_id.to_string(), &event_b.id.to_string())
            .expect("find_event_by_id b")
            .expect("event_b must resolve");

        assert_eq!(found_a.id, event_a.id, "must resolve event_a to its OWN id");
        assert_eq!(found_b.id, event_b.id, "must resolve event_b to its OWN id");
        assert_ne!(
            found_a.id, found_b.id,
            "the two same-type events must resolve to DISTINCT events"
        );
    }

    /// A uuid never appended to the DAG resolves to `None` — the anti-staple
    /// negative-space case (a fabricated root never resolves).
    #[test]
    fn find_event_by_id_returns_none_for_never_appended_uuid() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let event = make_file_read_event(session_id);
        append_event(&conn, TEST_KEY, &event, None).expect("append_event");

        let fabricated_id = Uuid::new_v4();
        let found = find_event_by_id(&conn, &session_id.to_string(), &fabricated_id.to_string())
            .expect("find_event_by_id");
        assert!(
            found.is_none(),
            "a uuid never appended to the DAG must resolve to None"
        );
    }

    /// A real event id from a DIFFERENT session must resolve to `None`
    /// (session-scoped lookup — never leaks cross-session events).
    #[test]
    fn find_event_by_id_returns_none_for_cross_session_id() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_a = Uuid::new_v4();
        let session_b = Uuid::new_v4();
        let event = make_file_read_event(session_a);
        append_event(&conn, TEST_KEY, &event, None).expect("append_event");

        let found = find_event_by_id(&conn, &session_b.to_string(), &event.id.to_string())
            .expect("find_event_by_id");
        assert!(
            found.is_none(),
            "an event id from a DIFFERENT session must resolve to None (session-scoped)"
        );
    }

    #[test]
    fn event_hash_by_id_returns_hash_for_known_event() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let event = make_file_read_event(session_id);
        let event_id = event.id.to_string();

        let hash = append_event(&conn, TEST_KEY, &event, None).expect("append_event");

        let found = event_hash_by_id(&conn, &event_id).expect("event_hash_by_id");
        assert_eq!(found, Some(hash));
    }

    #[test]
    fn event_hash_by_id_returns_none_for_unknown_id() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");

        let found = event_hash_by_id(&conn, &Uuid::new_v4().to_string())
            .expect("event_hash_by_id");
        assert!(found.is_none());
    }

    #[test]
    fn pending_confirmations_insert_and_duplicate_effect_id_fails() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let effect_id = Uuid::new_v4().to_string();
        let session_id = Uuid::new_v4().to_string();
        let blocked_event_id = Uuid::new_v4().to_string();

        conn.execute(
            "INSERT INTO pending_confirmations \
             (effect_id, session_id, blocked_event_id, sink, resolved_args, \
              blocked_arg_names, combined_digest, workspace_root_path, state) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                effect_id,
                session_id,
                blocked_event_id,
                "file.create",
                "[]",
                "[]",
                "deadbeef",
                "/workspace",
                "pending",
            ],
        )
        .expect("first insert should succeed");

        let duplicate = conn.execute(
            "INSERT INTO pending_confirmations \
             (effect_id, session_id, blocked_event_id, sink, resolved_args, \
              blocked_arg_names, combined_digest, workspace_root_path, state) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                effect_id,
                session_id,
                blocked_event_id,
                "file.create",
                "[]",
                "[]",
                "deadbeef",
                "/workspace",
                "pending",
            ],
        );

        assert!(
            duplicate.is_err(),
            "duplicate effect_id insert must fail the PRIMARY KEY constraint"
        );
    }

    /// Insert a `pending_confirmations` row directly (test helper) — mirrors the
    /// 9-column shape the sibling tests use.
    fn insert_pending_row(
        conn: &rusqlite::Connection,
        effect_id: &str,
        session_id: &str,
        sink: &str,
        state: &str,
    ) {
        conn.execute(
            "INSERT INTO pending_confirmations \
             (effect_id, session_id, blocked_event_id, sink, resolved_args, \
              blocked_arg_names, combined_digest, workspace_root_path, state) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                effect_id,
                session_id,
                Uuid::new_v4().to_string(),
                sink,
                "[]",
                "[]",
                "deadbeef",
                "/workspace",
                state,
            ],
        )
        .expect("insert pending_confirmations row");
    }

    /// `list_pending_confirmations_for_session` returns exactly the `pending`
    /// rows for the queried session, as `(effect_id, sink)` pairs in rowid order
    /// — and excludes terminal (confirmed/denied) rows AND other sessions' rows.
    /// This is the read-only surface the `caprun run` post-Block operator-loop
    /// pointer is built from (WG-5).
    #[test]
    fn list_pending_confirmations_returns_only_pending_rows_for_session() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let other_session = Uuid::new_v4();

        let e1 = Uuid::new_v4();
        let e2 = Uuid::new_v4();
        let e_terminal = Uuid::new_v4();
        let e_other = Uuid::new_v4();

        // Two pending rows for our session (distinct sinks), inserted in order.
        insert_pending_row(&conn, &e1.to_string(), &session_id.to_string(), "email.send", "pending");
        insert_pending_row(&conn, &e2.to_string(), &session_id.to_string(), "file.create", "pending");
        // A terminal (confirmed) row for the SAME session — must be excluded.
        insert_pending_row(&conn, &e_terminal.to_string(), &session_id.to_string(), "email.send", "confirmed");
        // A pending row for a DIFFERENT session — must be excluded.
        insert_pending_row(&conn, &e_other.to_string(), &other_session.to_string(), "email.send", "pending");

        let rows = list_pending_confirmations_for_session(&conn, &session_id.to_string())
            .expect("list_pending_confirmations_for_session");

        assert_eq!(
            rows,
            vec![(e1, "email.send".to_string()), (e2, "file.create".to_string())],
            "only the two pending rows for this session, in rowid order, as (effect_id, sink)"
        );

        // A session with no pending rows returns an empty vec (never an error).
        let empty = list_pending_confirmations_for_session(&conn, &Uuid::new_v4().to_string())
            .expect("query for an unknown session must succeed with no rows");
        assert!(empty.is_empty(), "a session with no pending rows yields an empty vec");
    }

    /// (Task 2 migration test) A DB whose `pending_confirmations` table
    /// PREDATES the Phase-16 `blocked_arg_names`/`combined_digest` columns
    /// (the OLD 7-column shape) is widened idempotently by
    /// `migrate_pending_confirmations_schema` — reopening it a second time
    /// does not error, and a widened 9-column INSERT then succeeds where it
    /// would otherwise fail with "no such column."
    #[test]
    fn pending_confirmations_migration_widens_legacy_schema_idempotently() {
        // Build a temp-file DB (":memory:" can't be reopened across
        // connections) with the OLD 7-column `pending_confirmations` shape —
        // bypassing `open_audit_db`/`SCHEMA_DDL` entirely to simulate a
        // pre-Phase-16 database on disk.
        let mut path = std::env::temp_dir();
        path.push(format!("caprun_legacy_pending_confirmations_{}.db", Uuid::new_v4()));

        {
            let legacy_conn = rusqlite::Connection::open(&path).expect("open legacy db");
            legacy_conn
                .execute_batch(
                    "CREATE TABLE pending_confirmations (
                        effect_id           TEXT PRIMARY KEY,
                        session_id          TEXT NOT NULL,
                        blocked_event_id    TEXT NOT NULL,
                        sink                TEXT NOT NULL,
                        resolved_args       TEXT NOT NULL,
                        workspace_root_path TEXT NOT NULL,
                        state               TEXT NOT NULL
                    ) STRICT;",
                )
                .expect("create legacy 7-column table");
        }

        // First open: `open_audit_db` runs `SCHEMA_DDL` (a no-op — the table
        // already exists) then the migration, which MUST widen the legacy
        // table in place.
        let conn = open_audit_db(path.to_str().unwrap()).expect("open_audit_db (first, migrating)");

        let widened = conn.execute(
            "INSERT INTO pending_confirmations \
             (effect_id, session_id, blocked_event_id, sink, resolved_args, \
              blocked_arg_names, combined_digest, workspace_root_path, state) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                "file.create",
                "[]",
                "[]",
                "deadbeef",
                "/workspace",
                "pending",
            ],
        );
        assert!(
            widened.is_ok(),
            "a widened 9-column INSERT must succeed after migrating a legacy \
             7-column pending_confirmations table: {widened:?}"
        );
        drop(conn);

        // Second open (a fresh connection, simulating `caprun confirm` reusing
        // the same DB file later): the migration must be idempotent — no
        // "duplicate column name" error, and the widened INSERT still works.
        let conn2 = open_audit_db(path.to_str().unwrap()).expect("open_audit_db (second, already-migrated)");
        let widened_again = conn2.execute(
            "INSERT INTO pending_confirmations \
             (effect_id, session_id, blocked_event_id, sink, resolved_args, \
              blocked_arg_names, combined_digest, workspace_root_path, state) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                "file.create",
                "[]",
                "[]",
                "deadbeef",
                "/workspace",
                "pending",
            ],
        );
        assert!(
            widened_again.is_ok(),
            "re-opening an already-migrated DB must not error, and a second \
             widened INSERT must still succeed: {widened_again:?}"
        );

        drop(conn2);
        std::fs::remove_file(&path).ok();
    }

    /// (Task 1, HARDEN-03) `sent_plan_nodes` is a WHOLE NEW table, unlike
    /// `pending_confirmations`'s column-widening case above — the migration
    /// idempotency question here is simpler: does re-running `open_audit_db`
    /// on an already-migrated DB (with the table already present) error?
    /// It must not — mirrors the `chain_anchor` presence-check discipline.
    #[test]
    fn sent_plan_nodes_migration_is_idempotent() {
        let mut path = std::env::temp_dir();
        path.push(format!("caprun_sent_plan_nodes_{}.db", Uuid::new_v4()));

        // First open: `open_audit_db` runs `SCHEMA_DDL` (creates the table
        // fresh) then `migrate_sent_plan_nodes_schema`'s presence check.
        let conn = open_audit_db(path.to_str().unwrap()).expect("open_audit_db (first)");

        let inserted = conn.execute(
            "INSERT INTO sent_plan_nodes (idempotency_key, effect_id, session_id, sent_at) \
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                "deadbeef-idempotency-key",
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                Utc::now().to_rfc3339(),
            ],
        );
        assert!(
            inserted.is_ok(),
            "sent_plan_nodes must accept an insert on the first-created table: {inserted:?}"
        );
        drop(conn);

        // Second open (fresh connection, simulating a broker restart against
        // the same DB file): re-running `open_audit_db` on an
        // already-migrated DB must not error, and the prior row must still
        // be present (no data loss / no table recreation).
        let conn2 =
            open_audit_db(path.to_str().unwrap()).expect("open_audit_db (second, already-migrated) must not error");

        let count: i64 = conn2
            .query_row("SELECT COUNT(*) FROM sent_plan_nodes", [], |row| row.get(0))
            .expect("count sent_plan_nodes rows");
        assert_eq!(
            count, 1,
            "the row inserted before the second open must survive re-migration"
        );

        drop(conn2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn query_events_by_session_returns_all_events() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let event = make_file_read_event(session_id);
        let event_id = event.id;

        append_event(&conn, TEST_KEY, &event, None).expect("append_event");

        let events = query_events_by_session(&conn, &session_id.to_string())
            .expect("query_events_by_session");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, event_id);
        assert_eq!(
            events[0].taint,
            vec![TaintLabel::ExternalUntrusted, TaintLabel::EmailRaw]
        );
    }

    // ── HARDEN-02 Task 2: forgery-without-key rejected + key-dependence ────

    /// Recompute an event row's hash the way a bare `events`-table writer
    /// WITHOUT the broker key would: the PUBLIC, unkeyed SHA-256 algorithm
    /// this file used before the v1.6 Phase 28 HMAC upgrade (plain
    /// concatenation, no domain separation, no length framing, no secret
    /// key). An attacker who only has read/write access to the SQLite file
    /// — never the key — can reproduce THIS, and only this.
    fn unkeyed_sha256_hash(
        parent_hash: Option<&str>,
        id: &str,
        session_id: &str,
        event_type: &str,
        payload: &str,
        taint: &str,
    ) -> String {
        use sha2::Digest as _;
        let mut hasher = Sha256::new();
        hasher.update(parent_hash.unwrap_or(""));
        hasher.update(id);
        hasher.update(session_id);
        hasher.update(event_type);
        hasher.update(payload);
        hasher.update(taint);
        hex::encode(hasher.finalize())
    }

    /// Success Criterion 1 (HARDEN-02): a bare `events`-table writer who does
    /// NOT know the broker key rewrites one event row's payload AND
    /// recomputes every descendant's hash/parent_hash to be internally
    /// SELF-CONSISTENT — but only under the PUBLIC unkeyed algorithm, since
    /// that is all an attacker without the key can reproduce. This is
    /// specifically NOT a call to the keyed `compute_event_hash(&key, ...)`
    /// (that would require the very secret this test simulates the attacker
    /// lacking) — the forgery is rejected because `verify_chain` recomputes
    /// with the REAL keyed MAC, which the attacker's unkeyed forgery can
    /// never match.
    #[test]
    fn self_consistent_forgery_without_key_is_rejected() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let real_key: &[u8] = b"the-real-broker-secret-key-attacker-lacks";

        // Build a genuine 2-event keyed chain: root -> child.
        let root = make_file_read_event(session_id);
        let root_hash = append_event(&conn, real_key, &root, None).expect("append root");
        let child = Event::new(
            Uuid::new_v4(),
            Some(root.id),
            session_id,
            "worker".to_string(),
            "sink_ok".to_string(),
            Utc::now(),
            vec![],
        );
        let _child_hash =
            append_event(&conn, real_key, &child, Some(&root_hash)).expect("append child");

        // Sanity: the genuine chain verifies true under the real key BEFORE
        // any tamper.
        assert!(
            verify_chain(&conn, &session_id.to_string(), real_key),
            "sanity: the untampered chain must verify true under the real key"
        );

        // ATTACK: rewrite the root's payload (e.g. forging a different
        // event), then — WITHOUT the key — recompute a fully self-consistent
        // chain under the PUBLIC unkeyed algorithm: root's new hash, and
        // child's new hash (its parent_hash field now points at root's new,
        // unkeyed-recomputed hash).
        let tampered_payload = r#"{"tampered":"attacker-controlled content"}"#;
        let root_taint: String = conn
            .query_row(
                "SELECT taint FROM events WHERE id = ?1",
                rusqlite::params![root.id.to_string()],
                |row| row.get(0),
            )
            .expect("query root taint");
        let forged_root_hash = unkeyed_sha256_hash(
            None,
            &root.id.to_string(),
            &session_id.to_string(),
            "file_read",
            tampered_payload,
            &root_taint,
        );
        conn.execute(
            "UPDATE events SET payload = ?1, hash = ?2 WHERE id = ?3",
            rusqlite::params![tampered_payload, forged_root_hash, root.id.to_string()],
        )
        .expect("tamper root payload+hash");

        let child_payload: String = conn
            .query_row(
                "SELECT payload FROM events WHERE id = ?1",
                rusqlite::params![child.id.to_string()],
                |row| row.get(0),
            )
            .expect("query child payload");
        let forged_child_hash = unkeyed_sha256_hash(
            Some(&forged_root_hash),
            &child.id.to_string(),
            &session_id.to_string(),
            "sink_ok",
            &child_payload,
            "[]",
        );
        conn.execute(
            "UPDATE events SET parent_hash = ?1, hash = ?2 WHERE id = ?3",
            rusqlite::params![forged_root_hash, forged_child_hash, child.id.to_string()],
        )
        .expect("tamper child parent_hash+hash");

        // THE TEETH: the forged chain is internally self-consistent under
        // the unkeyed algorithm the attacker CAN reproduce, but
        // `verify_chain` recomputes with the REAL keyed MAC — which the
        // attacker, lacking the key, could never have produced.
        assert!(
            !verify_chain(&conn, &session_id.to_string(), real_key),
            "a self-consistent forgery built WITHOUT the broker key must be rejected \
             by the keyed verify_chain (Success Criterion 1)"
        );
    }

    /// Success Criterion 2 (HARDEN-02): the chain's authenticity depends on
    /// the secret key. Two different keys over IDENTICAL fields produce
    /// DIFFERENT MACs, and `verify_chain` called with the WRONG key on an
    /// otherwise-UNTAMPERED chain returns `false`.
    #[test]
    fn verify_chain_is_key_dependent() {
        let key_a: &[u8] = b"key-a-0123456789abcdef0123456789ab";
        let key_b: &[u8] = b"key-b-fedcba9876543210fedcba987654";
        assert_ne!(key_a, key_b, "sanity: the two test keys must differ");

        // Key-dependence of compute_event_hash itself: identical fields,
        // different keys, different digests.
        let digest_a = compute_event_hash(
            key_a,
            None,
            "id-1",
            "session-1",
            "file_read",
            "payload",
            "taint",
        );
        let digest_b = compute_event_hash(
            key_b,
            None,
            "id-1",
            "session-1",
            "file_read",
            "payload",
            "taint",
        );
        assert_ne!(
            digest_a, digest_b,
            "compute_event_hash must produce different digests under different keys \
             for identical fields"
        );

        // verify_chain called with the WRONG key on an untampered chain
        // returns false — never a false positive.
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let event = make_file_read_event(session_id);
        append_event(&conn, key_a, &event, None).expect("append under key_a");

        assert!(
            verify_chain(&conn, &session_id.to_string(), key_a),
            "sanity: the chain must verify true under the SAME key it was appended with"
        );
        assert!(
            !verify_chain(&conn, &session_id.to_string(), key_b),
            "verify_chain under the WRONG key on an untampered chain must return false \
             (Success Criterion 2)"
        );
    }

    // ── HARDEN-02 Plan 04: chain_anchor tail-truncation + fail-closed ──────

    /// D-04's whole point: tail-truncation was PREVIOUSLY invisible to
    /// `verify_chain` (the recursive-CTE walk simply terminates at the
    /// shorter true leaf and `found_any` still passed). The MAC'd
    /// `chain_anchor` row closes this: after a raw-SQL `DELETE` removes the
    /// tail event (bypassing `append_event` entirely, so the anchor is left
    /// STALE — still pointing at the deleted event), the walk's recomputed
    /// leaf/count no longer match the anchor, and an attacker without the
    /// broker key cannot re-MAC the anchor to match the shortened chain.
    #[test]
    fn tail_truncation_detected_via_anchor_mismatch() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();

        let root = make_file_read_event(session_id);
        let root_hash = append_event(&conn, TEST_KEY, &root, None).expect("append root");
        let child = Event::new(
            Uuid::new_v4(),
            Some(root.id),
            session_id,
            "worker".to_string(),
            "sink_ok".to_string(),
            Utc::now(),
            vec![],
        );
        append_event(&conn, TEST_KEY, &child, Some(&root_hash)).expect("append child");

        // Sanity (also the "untampered chain still verifies true, no false
        // positive" assertion): the genuine 2-event anchored chain verifies
        // true BEFORE any tamper.
        assert!(
            verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "sanity: an untampered, normally-appended anchored chain must verify \
             true (no false positive)"
        );

        // ATTACK: delete the tail (child) event via raw SQL, bypassing
        // append_event entirely — events shrinks to just the root, but the
        // chain_anchor row is left STALE (still records the deleted child
        // as head_event_id/head_hash with event_count=2).
        conn.execute(
            "DELETE FROM events WHERE id = ?1",
            rusqlite::params![child.id.to_string()],
        )
        .expect("raw-SQL tail delete (simulating a bare events-table writer)");

        assert!(
            !verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "tail-truncation (raw DELETE bypassing append_event, leaving a stale \
             anchor) must now be DETECTED — verify_chain must return false (D-04)"
        );
    }

    /// Orphan-row guard (HARDEN-02): an actor with `events`-table write access
    /// could INSERT an unreferenced row (a non-NULL `parent_id` pointing at no
    /// existing event) that the recursive chain walk — rooted at `parent_id IS
    /// NULL` and following `parent_id` links — never reaches. Its MAC is forged
    /// (the attacker lacks the key) and it never enters a confirm/deny decision
    /// path, but an authenticated chain must not silently carry unreferenced
    /// rows. The live-count guard in `verify_chain` fails closed when the table
    /// holds more events for the session than the walk reached.
    #[test]
    fn orphan_event_injection_detected_via_live_count() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();

        let root = make_file_read_event(session_id);
        let root_hash = append_event(&conn, TEST_KEY, &root, None).expect("append root");
        let child = Event::new(
            Uuid::new_v4(),
            Some(root.id),
            session_id,
            "worker".to_string(),
            "sink_ok".to_string(),
            Utc::now(),
            vec![],
        );
        append_event(&conn, TEST_KEY, &child, Some(&root_hash)).expect("append child");

        assert!(
            verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "sanity: untampered anchored chain verifies true before the orphan insert"
        );

        // ATTACK: INSERT an unreferenced ("orphan") event via raw SQL — same
        // session, but `parent_id` points at a fresh, nonexistent UUID, so the
        // recursive walk never reaches it. Bypasses append_event, so the
        // chain_anchor row is NOT updated: walked_count stays 2 (matching the
        // stale anchor), but the live row count is now 3.
        conn.execute(
            "INSERT INTO events \
             (id, parent_id, session_id, event_type, actor, payload, taint, parent_hash, hash) \
             VALUES (?1, ?2, ?3, 'orphan', 'attacker', '{}', '[]', ?4, 'orphan-forged-hash')",
            rusqlite::params![
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(), // nonexistent parent — never walked
                session_id.to_string(),
                "orphan-forged-parent-hash",
            ],
        )
        .expect("raw-SQL orphan insert (simulating a bare events-table writer)");

        assert!(
            !verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "orphan-row injection (unreferenced events-table INSERT bypassing \
             append_event) must be DETECTED — verify_chain must return false via \
             the live-count guard"
        );
    }

    /// Migration pin (DESIGN-security-hardening.md "Migration (pinned)"): a
    /// session with events but NO `chain_anchor` row — e.g. a legacy
    /// pre-Phase-28 database, or an anchor row deleted alongside the tail
    /// (T-28-10) — must fail closed: untrusted until re-anchored, never
    /// silently trusted as authenticated.
    #[test]
    fn legacy_db_without_anchor_fails_closed() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();

        let event = make_file_read_event(session_id);
        append_event(&conn, TEST_KEY, &event, None).expect("append_event");

        // Sanity: append_event's atomic upsert means the anchor DOES exist
        // right after a normal append, and the chain verifies true.
        assert!(
            verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "sanity: a freshly-appended chain has its anchor and verifies true"
        );

        // Simulate a legacy pre-Phase-28 DB (or a deleted anchor row,
        // T-28-10): remove the chain_anchor row for this session — the
        // events themselves are untouched.
        conn.execute(
            "DELETE FROM chain_anchor WHERE session_id = ?1",
            rusqlite::params![session_id.to_string()],
        )
        .expect("remove anchor row to simulate a legacy un-anchored session");

        assert!(
            !verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "a session with events but NO chain_anchor row must fail closed \
             (untrusted until re-anchored) — never silently pass, never panic"
        );
    }

    // ── Task 2 (HARDEN-03): plan_node_idempotency_key ──────────────────────

    fn test_sink(s: &str) -> runtime_core::SinkId {
        runtime_core::SinkId(s.to_string())
    }

    fn test_arg(name: &str, value_id: uuid::Uuid) -> runtime_core::PlanArg {
        runtime_core::PlanArg {
            name: name.to_string(),
            value_id: runtime_core::ValueId(value_id),
        }
    }

    /// Order-invariance: swapping the input arg order must not change the
    /// key — this is what the pre-hash sort buys.
    #[test]
    fn idempotency_key_is_order_invariant() {
        let sink = test_sink("email.send");
        let v_to = Uuid::new_v4();
        let v_body = Uuid::new_v4();
        let arg_a = test_arg("to", v_to);
        let arg_b = test_arg("body", v_body);

        let forward = plan_node_idempotency_key(&sink, &[arg_a.clone(), arg_b.clone()]);
        let reversed = plan_node_idempotency_key(&sink, &[arg_b, arg_a]);

        assert_eq!(
            forward, reversed,
            "swapping arg order must not change the idempotency key"
        );
    }

    /// Sink-scoping: identical args, different `sink.0`, must produce a
    /// DIFFERENT key.
    #[test]
    fn idempotency_key_is_sink_scoped() {
        let value_id = Uuid::new_v4();
        let args = [test_arg("to", value_id)];

        let key_email = plan_node_idempotency_key(&test_sink("email.send"), &args);
        let key_other = plan_node_idempotency_key(&test_sink("file.create"), &args);

        assert_ne!(
            key_email, key_other,
            "the same args under a different sink must produce a different key"
        );
    }

    /// Value-distinguishing: same sink + arg name, different `value_id`,
    /// must produce a DIFFERENT key.
    #[test]
    fn idempotency_key_distinguishes_value_id() {
        let sink = test_sink("email.send");
        let key_a = plan_node_idempotency_key(&sink, &[test_arg("to", Uuid::new_v4())]);
        let key_b = plan_node_idempotency_key(&sink, &[test_arg("to", Uuid::new_v4())]);

        assert_ne!(
            key_a, key_b,
            "the same sink + arg name with a DIFFERENT value_id must produce a different key"
        );
    }

    /// Determinism: identical inputs (same sink, same args, same order)
    /// produce the identical key across repeated calls.
    #[test]
    fn idempotency_key_is_deterministic() {
        let sink = test_sink("email.send");
        let v_to = Uuid::new_v4();
        let v_body = Uuid::new_v4();
        let args = [test_arg("to", v_to), test_arg("body", v_body)];

        let key_first = plan_node_idempotency_key(&sink, &args);
        let key_second = plan_node_idempotency_key(&sink, &args);

        assert_eq!(
            key_first, key_second,
            "identical inputs must produce the identical key across calls"
        );
    }

    // ── GITHUB-02 (Task 1): session auth-grant capability ───────────────────

    /// has_github_grant is false on a fresh DB (fail-closed default — absent a
    /// grant the github.pr sink Denies, §8).
    #[test]
    fn github_grant_false_on_fresh_db() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        assert!(
            !has_github_grant(&conn, &session_id.to_string()),
            "a fresh DB with no grant row must report has_github_grant == false"
        );
    }

    /// record_github_grant inserts the capability AND appends an OPAQUE
    /// `github_grant_authorized` event; afterward has_github_grant == true and
    /// the event carries NO secret in its payload/taint.
    #[test]
    fn github_grant_recorded_makes_has_true_and_emits_opaque_event() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();

        assert!(!has_github_grant(&conn, &session_id.to_string()));
        record_github_grant(&conn, TEST_KEY, &session_id.to_string())
            .expect("record_github_grant");
        assert!(
            has_github_grant(&conn, &session_id.to_string()),
            "after record_github_grant, has_github_grant must be true"
        );

        // The grant event exists, is opaque (empty taint, no token), and its
        // actor names the grant capability + session.
        let event = find_event_by_type(
            &conn,
            &session_id.to_string(),
            "github_grant_authorized",
        )
        .expect("find_event_by_type")
        .expect("a github_grant_authorized event must exist");
        assert!(
            event.taint.is_empty(),
            "the grant event must be opaque — empty taint set"
        );
        assert_eq!(
            event.actor,
            format!("grant:github.pr:{session_id}"),
            "the grant event actor must name the grant capability and session"
        );
        // The genuine, normally-appended chain still verifies true (the grant
        // event chained correctly on the session head + anchor upserted).
        assert!(
            verify_chain(&conn, &session_id.to_string(), TEST_KEY),
            "the grant event must leave a verifiable, anchored chain"
        );
    }

    /// Session-scoped: a grant on session A must NOT leak to session B (the
    /// capability is keyed by session_id — the load-bearing GITHUB-02 claim).
    #[test]
    fn github_grant_is_session_scoped() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_a = Uuid::new_v4();
        let session_b = Uuid::new_v4();

        record_github_grant(&conn, TEST_KEY, &session_a.to_string())
            .expect("record grant for A");

        assert!(
            has_github_grant(&conn, &session_a.to_string()),
            "session A must have the grant"
        );
        assert!(
            !has_github_grant(&conn, &session_b.to_string()),
            "a grant on session A must NOT leak to session B (session-scoped)"
        );
    }

    /// record_github_grant is idempotent: a second grant for the SAME session
    /// does not error and does not duplicate the capability row (INSERT OR
    /// IGNORE via the session_id PRIMARY KEY), and no duplicate authorize
    /// event is appended on the replay.
    #[test]
    fn github_grant_record_is_idempotent() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();

        record_github_grant(&conn, TEST_KEY, &session_id.to_string())
            .expect("first grant");
        record_github_grant(&conn, TEST_KEY, &session_id.to_string())
            .expect("second grant must not error (idempotent)");

        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM session_grants WHERE session_id = ?1",
                rusqlite::params![session_id.to_string()],
                |row| row.get(0),
            )
            .expect("count session_grants rows");
        assert_eq!(
            row_count, 1,
            "a repeated grant must not duplicate the capability row"
        );

        // The replay must NOT append a second authorize event.
        let event_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM events \
                 WHERE session_id = ?1 AND event_type = 'github_grant_authorized'",
                rusqlite::params![session_id.to_string()],
                |row| row.get(0),
            )
            .expect("count grant events");
        assert_eq!(
            event_count, 1,
            "a replayed grant must not append a duplicate github_grant_authorized event"
        );
    }

    /// The `session_grants` migration is idempotent and re-runnable across a
    /// broker restart against the same DB file (mirrors
    /// `sent_plan_nodes_migration_is_idempotent`).
    #[test]
    fn github_grant_migration_is_idempotent() {
        let mut path = std::env::temp_dir();
        path.push(format!("caprun_session_grants_{}.db", Uuid::new_v4()));

        let conn = open_audit_db(path.to_str().unwrap()).expect("open_audit_db (first)");
        let session_id = Uuid::new_v4();
        record_github_grant(&conn, TEST_KEY, &session_id.to_string())
            .expect("record grant against freshly-created table");
        drop(conn);

        // Re-open (simulating a broker restart) — the migration must not error
        // and the grant row must survive (no table recreation / data loss).
        let conn2 = open_audit_db(path.to_str().unwrap())
            .expect("open_audit_db (second, already-migrated) must not error");
        assert!(
            has_github_grant(&conn2, &session_id.to_string()),
            "the grant recorded before the second open must survive re-migration"
        );

        drop(conn2);
        std::fs::remove_file(&path).ok();
    }

    // ── GITHUB-04 (Task 2): duplicate-PR CAS ─────────────────────────────────

    /// github_pr_content_key is deterministic: identical
    /// (owner,repo,base,head,title,body) literals produce the IDENTICAL key
    /// across calls.
    #[test]
    fn created_pr_content_key_is_deterministic() {
        let k1 = github_pr_content_key("octo", "hello", "main", "feature", "Add X", "body text");
        let k2 = github_pr_content_key("octo", "hello", "main", "feature", "Add X", "body text");
        assert_eq!(
            k1, k2,
            "identical PR content must produce the identical content key"
        );
    }

    /// Changing ANY one of the six fields changes the key (field-change
    /// sensitivity — a replay is only suppressed for byte-identical content).
    #[test]
    fn created_pr_content_key_changes_when_any_field_changes() {
        let base = github_pr_content_key("octo", "hello", "main", "feature", "Add X", "body");
        let variants = [
            github_pr_content_key("OCTO", "hello", "main", "feature", "Add X", "body"),
            github_pr_content_key("octo", "HELLO", "main", "feature", "Add X", "body"),
            github_pr_content_key("octo", "hello", "MAIN", "feature", "Add X", "body"),
            github_pr_content_key("octo", "hello", "main", "FEATURE", "Add X", "body"),
            github_pr_content_key("octo", "hello", "main", "feature", "Add Y", "body"),
            github_pr_content_key("octo", "hello", "main", "feature", "Add X", "BODY"),
        ];
        for (i, v) in variants.iter().enumerate() {
            assert_ne!(
                &base, v,
                "changing field #{i} must change the content key"
            );
        }
    }

    /// Partition-blindness resistance: `owner="ab",repo="c"` must NOT collide
    /// with `owner="a",repo="bc"` — this holds ONLY because the key is built
    /// on `combined_digest` (per-field fixed-width inner hash), NOT a plain
    /// concatenation.
    #[test]
    fn created_pr_content_key_resists_partition_blindness() {
        let a = github_pr_content_key("ab", "c", "main", "feature", "t", "b");
        let b = github_pr_content_key("a", "bc", "main", "feature", "t", "b");
        assert_ne!(
            a, b,
            "a boundary shift between owner and repo must change the key \
             (combined_digest defends partition-blindness)"
        );
    }

    /// reserve_created_pr returns true on the first insert of a key and false
    /// on a replay of the SAME key (PRIMARY KEY violation suppressed via
    /// INSERT OR IGNORE) — at-most-once per PR content.
    #[test]
    fn created_pr_reserve_fresh_then_suppressed() {
        let conn = open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4().to_string();
        let key = github_pr_content_key("octo", "hello", "main", "feature", "Add X", "body");

        // First reservation (a fresh effect_id) — this dispatch owns the PR.
        let fresh = reserve_created_pr(&conn, &key, &Uuid::new_v4().to_string(), &session_id)
            .expect("first reserve");
        assert!(fresh, "the first reservation of a content key must be fresh (true)");

        // Replay: identical content key, DIFFERENT (fresh) effect_id — the CAS
        // must still suppress it (proving it is content-keyed, not
        // effect_id-keyed).
        let replay = reserve_created_pr(&conn, &key, &Uuid::new_v4().to_string(), &session_id)
            .expect("replay reserve");
        assert!(
            !replay,
            "a replay of the same content key must be suppressed (false) even \
             under a fresh effect_id"
        );

        // Exactly one row persisted for the key.
        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM created_prs WHERE idempotency_key = ?1",
                rusqlite::params![key],
                |row| row.get(0),
            )
            .expect("count created_prs rows");
        assert_eq!(row_count, 1, "at most one CAS row per content key");
    }

    /// The `created_prs` migration is idempotent across a broker restart
    /// against the same DB file (mirrors `sent_plan_nodes_migration_is_idempotent`).
    #[test]
    fn created_pr_migration_is_idempotent() {
        let mut path = std::env::temp_dir();
        path.push(format!("caprun_created_prs_{}.db", Uuid::new_v4()));

        let conn = open_audit_db(path.to_str().unwrap()).expect("open_audit_db (first)");
        let key = github_pr_content_key("octo", "hello", "main", "feature", "Add X", "body");
        assert!(
            reserve_created_pr(&conn, &key, &Uuid::new_v4().to_string(), &Uuid::new_v4().to_string())
                .expect("reserve against freshly-created table"),
            "first reserve on the fresh table must be true"
        );
        drop(conn);

        // Re-open — migration must not error and the reserved row must survive,
        // so a second reserve of the same key is suppressed (false).
        let conn2 = open_audit_db(path.to_str().unwrap())
            .expect("open_audit_db (second, already-migrated) must not error");
        let replay = reserve_created_pr(
            &conn2,
            &key,
            &Uuid::new_v4().to_string(),
            &Uuid::new_v4().to_string(),
        )
        .expect("reserve after reopen");
        assert!(
            !replay,
            "the row reserved before the second open must survive re-migration \
             (replay suppressed)"
        );

        drop(conn2);
        std::fs::remove_file(&path).ok();
    }
}
