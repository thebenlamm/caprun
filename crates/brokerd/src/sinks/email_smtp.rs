/// sinks/email_smtp — the real broker-mediated SMTP adapter (SMTP-01..SMTP-05).
///
/// This module is the ONLY code path in the whole TCB that performs an actual
/// SMTP call (D-03). It is broker-resident, NEVER confined-worker-resident,
/// and is invoked ONLY from the confirm-path process AFTER a human has
/// confirmed a Blocked plan node's tainted routing args
/// (`crate::confirmation::confirm()`). Plan 02 wires
/// `invoke_email_smtp_from_resolved` into `confirm()`'s atomic CAS +
/// `email_send_attempted` transaction; this module does not know about that
/// transaction and never touches `pending_confirmations` itself.
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

/// Build the outgoing wire message.
///
/// RED-phase stub (Task 1, TDD): does NOT yet parse the `to` literal through
/// `lettre::Address` — hardcodes a stub recipient instead, so the CRLF
/// fail-closed test below fails, proving the test actually exercises the
/// eventual real behavior before it is implemented.
fn build_message(resolved_args: &[ResolvedArg]) -> Result<Message> {
    let subject = resolved_literal(resolved_args, "subject")
        .ok_or_else(|| anyhow::anyhow!("email_smtp: build_message missing required `subject` arg"))?;
    let body = resolved_literal(resolved_args, "body")
        .ok_or_else(|| anyhow::anyhow!("email_smtp: build_message missing required `body` arg"))?;

    let from: Address = smtp_from()
        .parse()
        .context("email_smtp: smtp_from() config value failed Address parse")?;
    // TODO(RED, Task 1 GREEN phase): parse the real `to` literal via
    // lettre::Address FIRST — this stub ignores it entirely.
    let to = Mailbox::new(None, Address::new_dangerous("stub", "localhost"));

    Message::builder()
        .from(Mailbox::new(None, from))
        .to(to)
        .subject(subject)
        .body(Body::new(body.to_string()))
        .context("email_smtp: build_message body construction failed (fail-closed)")
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_core::plan_node::{TaintLabel, ValueId};

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
}
