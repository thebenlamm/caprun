/// sinks/email_smtp — the real broker-mediated SMTP adapter (SMTP-01..SMTP-05).
///
/// This module is the ONLY code path in the whole TCB that performs an actual
/// SMTP call (D-03). It is broker-resident, NEVER confined-worker-resident.
///
/// # Sanctioned callers (Phase 16 amendment — BOTH, not one)
///
/// `invoke_email_smtp_from_resolved` has exactly TWO sanctioned callers:
///   1. `crate::confirmation::confirm()`'s `email.send` special case, AFTER
///      its atomic CAS (`pending -> confirmed`) + durable
///      `email_send_attempted` transaction commits (Plan 16-02/SEND-01) — the
///      Blocked-then-human-confirmed path.
///   2. `server.rs`'s `SubmitPlanNode` Allowed-decision dispatch, AFTER its
///      own durable, opaque `email_send_attempted` append succeeds (Plan
///      16-04, CONTROL-01) — the trusted, never-blocked path. This caller has
///      NO CAS/`PendingConfirmation` (there is nothing to confirm on an
///      Allowed decision); see the REPLAY RESIDUAL RISK note at that call
///      site.
///
/// SHARED PRECONDITION (both callers): a durable, opaque `email_send_attempted`
/// event MUST be appended (parent-chained onto the caller's own current head)
/// BEFORE this function is ever called — so a crash/power-loss between the
/// attempt and delivery always leaves an audit record naming `email.send`,
/// regardless of which caller reached this function.
///
/// # Wire-message construction — CRLF/header-injection defense (SMTP-05, D-07/D-22)
///
/// Every recipient literal (`to`/`cc`/`bcc`) is parsed through `lettre::Address`'s
/// typed parser FIRST — a CR or LF byte anywhere in the literal makes
/// `Address::from_str` return `Err` (fail-closed), never reaching a
/// `Message::builder()` setter. Only after every recipient parses successfully
/// are the already-valid `Mailbox` values fed into the builder. Headers are
/// constructed EXCLUSIVELY through this typed builder — never `format!()`,
/// never `lettre`'s raw pre-encoded-header constructor (see
/// `scripts/check-email-smtp-construction.sh`, the structural grep gate
/// proving this file never uses that forbidden token).
///
/// # Endpoint sourcing (D-04 restated)
///
/// `smtp_host()`/`smtp_port()` read ONLY trusted local process env
/// (`CAPRUN_SMTP_HOST`/`CAPRUN_SMTP_PORT`), defaulting to `127.0.0.1:1025`
/// (Mailpit's conventional local target). NEVER read from the audit DB, a
/// plan node, a `ValueNode`, or `PendingConfirmation` — any block-time-writable
/// field — since the combined digest (Phase 16) binds only blocked-arg
/// literals, not the endpoint; sourcing it from writable state would let a
/// tamperer redirect a confirmed send to an uncovered destination.
///
/// `smtp_from()` is sourced the SAME way (trusted local env,
/// `CAPRUN_SMTP_FROM`, defaulting to `caprun@localhost`) — `lettre` requires a
/// `From` header on every `Message` (`MessageBuilder::build` returns
/// `Err(MissingFrom)` otherwise), and the sink schema this phase inherits
/// (`crates/executor/src/sink_schema.rs`) has no `from` arg. The sender
/// address is therefore broker-owned trusted config, never a resolved
/// literal — it MUST NOT be sourced from `resolved_args`.
///
/// # Opaque payloads only (T-13-02)
///
/// Both `email_send_succeeded` and `email_send_failed` events carry NO
/// resolved literal and NO raw SMTP response text in their hashed payload —
/// only `effect_id` (in the `actor` field, mirroring `file_create.rs`'s
/// `sink:file.create:<effect_id>` convention) and a static `event_type`
/// marker. Raw SMTP error text is routed to this codebase's `eprintln!`
/// logging convention (see `server.rs`) — never the hash chain.
use anyhow::{Context, Result};
use chrono::Utc;
use lettre::message::{Body, Mailbox};
use lettre::{Address, Message, SmtpTransport, Transport};
use runtime_core::Event;
use uuid::Uuid;

use crate::audit::append_event;
use crate::confirmation::ResolvedArg;

/// Serializes any test (in THIS module or a sibling module, e.g.
/// `confirmation::tests`) that mutates the process-global `CAPRUN_SMTP_*` env
/// vars — `cargo test`'s default multi-threaded runner would otherwise let two
/// such tests race on the same process-wide environment, since `mod tests` in
/// `email_smtp.rs` and `mod tests` in `confirmation.rs` are compiled into the
/// SAME `brokerd` lib test binary. `pub(crate)` so `confirmation::tests` can
/// take the same lock rather than defining its own (which would not actually
/// serialize anything).
#[cfg(test)]
pub(crate) static SMTP_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Read the trusted local SMTP host config (D-04 endpoint sourcing).
///
/// NEVER reads from the audit DB, a plan node, a `ValueNode`, or
/// `PendingConfirmation` — only trusted local process env, defaulting to
/// Mailpit's conventional loopback host.
fn smtp_host() -> String {
    std::env::var("CAPRUN_SMTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string())
}

/// Read the trusted local SMTP port config (D-04 endpoint sourcing).
///
/// Falls back to Mailpit's default SMTP port (1025) if unset or unparsable.
fn smtp_port() -> u16 {
    std::env::var("CAPRUN_SMTP_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(1025)
}

/// Read the trusted local "From" sender address (D-04 endpoint sourcing —
/// see module doc comment: `lettre` requires a From header, and the sink
/// schema has no `from` arg, so this is broker-owned trusted config, never a
/// resolved literal).
fn smtp_from() -> String {
    std::env::var("CAPRUN_SMTP_FROM").unwrap_or_else(|_| "caprun@localhost".to_string())
}

/// Look up a named literal directly from a frozen `ResolvedArg` snapshot.
/// Mirrors `file_create.rs`'s `resolved_literal` helper.
fn resolved_literal<'a>(resolved_args: &'a [ResolvedArg], name: &str) -> Option<&'a str> {
    resolved_args
        .iter()
        .find(|a| a.name == name)
        .map(|a| a.literal.as_str())
}

/// Parse a recipient literal into a `Mailbox` via lettre's typed `Address`
/// parser FIRST — the fail-closed CRLF boundary (SMTP-05/D-07). A CR or LF
/// byte anywhere in `literal` makes this return `Err`, never reaching any
/// `Message::builder()` setter (`Address`'s allow-list grammar rejects bytes
/// 10/13 in any branch — see `planning-docs/DESIGN-content-adapter-mediation.md`
/// "Wire-Message Construction").
fn parse_recipient(literal: &str) -> Result<Mailbox> {
    let address: Address = literal
        .parse()
        .with_context(|| "email_smtp: recipient literal failed Address parse (fail-closed)")?;
    Ok(Mailbox::from(address))
}

/// Build the outgoing wire message EXCLUSIVELY through lettre's typed
/// `Message::builder()` setters (SMTP-05, D-07/D-22).
///
/// Every recipient literal is parsed via `Address` FIRST (fail-closed on
/// CR/LF, `parse_recipient` above); only AFTER every recipient parses
/// successfully are the already-valid `Mailbox` values fed to
/// `.to()/.cc()/.bcc()` (infallible chaining). `.subject()` is also
/// infallible. `.body()` is the second and only other fallible call — its
/// `Err` propagates as a fail-closed abort, same as a recipient parse `Err`.
/// Never `.unwrap()`, never a `format!()`-built header, never lettre's raw
/// pre-encoded-header constructor.
///
/// `to`/`subject`/`body` are required (schema-mandatory); `cc`/`bcc` are
/// schema-optional and simply omitted from the builder chain if absent.
fn build_message(resolved_args: &[ResolvedArg]) -> Result<Message> {
    let to_literal = resolved_literal(resolved_args, "to")
        .ok_or_else(|| anyhow::anyhow!("email_smtp: build_message missing required `to` arg"))?;
    let subject = resolved_literal(resolved_args, "subject")
        .ok_or_else(|| anyhow::anyhow!("email_smtp: build_message missing required `subject` arg"))?;
    let body = resolved_literal(resolved_args, "body")
        .ok_or_else(|| anyhow::anyhow!("email_smtp: build_message missing required `body` arg"))?;

    // Parse every recipient literal FIRST — fail-closed at Address parse
    // time, before any builder call (SMTP-05). cc/bcc are schema-optional;
    // absence is fine (None), a present-but-invalid literal still fails
    // closed via `?`.
    let to_mbox = parse_recipient(to_literal)?;
    let cc_mbox = resolved_literal(resolved_args, "cc")
        .map(parse_recipient)
        .transpose()?;
    let bcc_mbox = resolved_literal(resolved_args, "bcc")
        .map(parse_recipient)
        .transpose()?;

    // The sender address is broker-owned trusted config (D-04 endpoint
    // sourcing, see module doc comment) — never a resolved literal. lettre
    // requires a From header on every Message (MissingFrom otherwise).
    let from: Address = smtp_from()
        .parse()
        .context("email_smtp: smtp_from() config value failed Address parse")?;

    let mut builder = Message::builder().from(Mailbox::from(from)).to(to_mbox);
    if let Some(cc) = cc_mbox {
        builder = builder.cc(cc);
    }
    if let Some(bcc) = bcc_mbox {
        builder = builder.bcc(bcc);
    }
    builder = builder.subject(subject);

    builder
        .body(Body::new(body.to_string()))
        .context("email_smtp: build_message body construction failed (fail-closed)")
}

/// Invoke the real `email.send` SMTP sink from a FROZEN `ResolvedArg`
/// snapshot (mirrors `file_create.rs::invoke_file_create_from_resolved`'s
/// frozen-snapshot shape). This is the ONLY code path in the whole TCB that
/// performs an SMTP call (D-03) — never called from the confined worker.
/// It has exactly TWO sanctioned callers (see the module doc comment above,
/// "Sanctioned callers" — Phase 16 amendment): (1) `confirm()`'s `email.send`
/// special case, AFTER its CAS + `email_send_attempted` transaction commits;
/// (2) `server.rs`'s Allowed-decision plan-node dispatch, AFTER its own
/// durable `email_send_attempted` append succeeds. BOTH share the SAME
/// precondition: a durable, opaque `email_send_attempted` MUST be appended
/// (parent-chained) before this function is called.
///
/// # Arguments
/// * `conn`          — open rusqlite connection (broker-owned; used for the
///   succeeded/failed append AFTER the caller's own precondition append
///   commits).
/// * `session_id`    — the Session the plan node belonged to.
/// * `effect_id`     — the SAME `effect_id` as the caller's plan-node
///   evaluation (the original block's anchor for caller 1; the broker-minted
///   effect identity for caller 2).
/// * `resolved_args` — the frozen `ResolvedArg` snapshot (from
///   `PendingConfirmation` for caller 1; resolved live from the
///   per-connection `ValueStore` for caller 2).
/// * `parent_id`     — causal predecessor event id (the caller's own
///   `email_send_attempted` event).
/// * `parent_hash`   — hash of that predecessor row (chain anchor).
///
/// # Returns
/// `(event_id, hash)` of the appended `email_send_succeeded` event on
/// success.
///
/// # Errors
/// On a `build_message` fail-closed abort or an SMTP transport error, an
/// `email_send_failed` event (OPAQUE payload — see module doc comment) is
/// durably appended FIRST, then the original error is propagated (no retry,
/// never swallowed, never `.unwrap()`/panic).
#[allow(clippy::too_many_arguments)]
pub fn invoke_email_smtp_from_resolved(
    conn: &rusqlite::Connection,
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    resolved_args: &[ResolvedArg],
    parent_id: Uuid,
    parent_hash: &str,
) -> Result<(Uuid, String)> {
    // build_message's fail-closed Err (missing/CRLF-bearing recipient, bad
    // body encoding) is treated identically to a transport Err below — both
    // are audited-abort paths, never a silent drop.
    let message = match build_message(resolved_args) {
        Ok(m) => m,
        Err(e) => {
            return record_send_failed(conn, key, session_id, effect_id, parent_id, parent_hash, e)
        }
    };

    let transport = SmtpTransport::builder_dangerous(smtp_host())
        .port(smtp_port())
        .build();

    match transport.send(&message) {
        Ok(_response) => {
            // Opaque payload: only effect_id (in `actor`) and a static
            // event_type marker — never a resolved literal, never the raw
            // SMTP response (T-13-02).
            let event = Event::new(
                Uuid::new_v4(),
                Some(parent_id),
                session_id,
                format!("sink:email.send:{effect_id}"),
                "email_send_succeeded".into(),
                Utc::now(),
                vec![],
            );
            let hash = append_event(conn, key, &event, Some(parent_hash))
                .context("append email_send_succeeded")?;
            Ok((event.id, hash))
        }
        Err(e) => record_send_failed(conn, key, session_id, effect_id, parent_id, parent_hash, e),
    }
}

/// Shared fail-closed audited-abort path for BOTH a `build_message` error and
/// an SMTP transport error: route the raw error text to `logger.error()`
/// (this codebase's `eprintln!("[brokerd] ...")` convention — the ONLY place
/// raw SMTP response text or a CRLF-bearing literal's parse error may
/// appear), append an OPAQUE-payload `email_send_failed` event, then
/// propagate a distinct non-swallowed `Err`. Never `.unwrap()`/panic, never a
/// silent drop, never `Ok(ConfirmedButSinkFailed)`-style swallowing.
fn record_send_failed<E: std::fmt::Display>(
    conn: &rusqlite::Connection,
    key: &[u8],
    session_id: Uuid,
    effect_id: Uuid,
    parent_id: Uuid,
    parent_hash: &str,
    err: E,
) -> Result<(Uuid, String)> {
    eprintln!("[brokerd] email.send failed (effect_id={effect_id}): {err}");
    let event = Event::new(
        Uuid::new_v4(),
        Some(parent_id),
        session_id,
        format!("sink:email.send:{effect_id}"),
        "email_send_failed".into(),
        Utc::now(),
        vec![],
    );
    append_event(conn, key, &event, Some(parent_hash)).context("append email_send_failed")?;
    Err(anyhow::anyhow!("email.send SMTP send failed: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{find_event_by_type, open_audit_db};
    use runtime_core::plan_node::{TaintLabel, ValueId};

    /// Fixed, non-secret test MAC key (mirrors `audit.rs`'s `TEST_KEY`).
    const TEST_KEY: &[u8] = b"email-smtp-rs-unit-test-key-not-secret";

    fn arg(name: &str, literal: &str) -> ResolvedArg {
        ResolvedArg {
            name: name.to_string(),
            value_id: ValueId::new(),
            literal: literal.to_string(),
            taint: vec![TaintLabel::UserTrusted],
            provenance_chain: vec![],
        }
    }

    #[test]
    fn build_message_ok_for_clean_single_recipient() {
        let args = vec![
            arg("to", "recipient@example.com"),
            arg("subject", "hello"),
            arg("body", "hi there"),
        ];
        let msg = build_message(&args);
        assert!(
            msg.is_ok(),
            "clean single recipient must build Ok: {:?}",
            msg.err()
        );
    }

    #[test]
    fn build_message_rejects_crlf_in_to_fail_closed() {
        let args = vec![
            arg("to", "victim@example.com\r\nBcc: attacker@evil.com"),
            arg("subject", "hello"),
            arg("body", "hi there"),
        ];
        let msg = build_message(&args);
        assert!(
            msg.is_err(),
            "CRLF-bearing `to` literal must fail closed at parse time, never build a Message"
        );
    }

    #[test]
    fn build_message_tolerates_absent_cc_bcc() {
        let args = vec![
            arg("to", "recipient@example.com"),
            arg("subject", "hello"),
            arg("body", "hi there"),
        ];
        let msg = build_message(&args);
        assert!(msg.is_ok(), "absent cc/bcc must not error: {:?}", msg.err());
    }

    // ── invoke_email_smtp_from_resolved (Task 2) ──

    /// A transport failure (closed/unbound port) must propagate Err (never
    /// swallowed) AND durably append an opaque-payload `email_send_failed`
    /// event — never an `email_send_succeeded` event.
    #[test]
    fn invoke_email_smtp_from_resolved_transport_failure_records_email_send_failed() {
        let _guard = SMTP_ENV_LOCK.lock().unwrap();

        // Bind an ephemeral port then immediately drop the listener: nothing
        // is listening on it for the rest of this test, so a connect attempt
        // is refused (ECONNREFUSED) almost immediately — no long timeout.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);

        std::env::set_var("CAPRUN_SMTP_HOST", "127.0.0.1");
        std::env::set_var("CAPRUN_SMTP_PORT", port.to_string());

        let conn = open_audit_db(":memory:").unwrap();
        let session_id = Uuid::new_v4();
        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            Utc::now(),
            vec![],
        );
        let root_hash = append_event(&conn, TEST_KEY, &root, None).unwrap();

        let effect_id = Uuid::new_v4();
        let args = vec![
            arg("to", "recipient@example.com"),
            arg("subject", "hello"),
            arg("body", "hi there"),
        ];

        let result = invoke_email_smtp_from_resolved(
            &conn,
            TEST_KEY,
            session_id,
            effect_id,
            &args,
            root.id,
            &root_hash,
        );

        std::env::remove_var("CAPRUN_SMTP_HOST");
        std::env::remove_var("CAPRUN_SMTP_PORT");

        assert!(
            result.is_err(),
            "a closed-port transport failure must propagate Err, never be swallowed"
        );

        let failed = find_event_by_type(&conn, &session_id.to_string(), "email_send_failed")
            .unwrap()
            .expect("email_send_failed event must be durably appended");
        assert_eq!(failed.actor, format!("sink:email.send:{effect_id}"));
        assert_eq!(failed.parent_id, Some(root.id));

        assert!(
            find_event_by_type(&conn, &session_id.to_string(), "email_send_succeeded")
                .unwrap()
                .is_none(),
            "no email_send_succeeded event on the failure path"
        );
    }
}
