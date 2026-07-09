//! email_smtp_acceptance — SMTP-03 + SMTP-05 acceptance tests (Linux-only, Mailpit-backed)
//!
//! These tests prove, against a REAL local capture SMTP (Mailpit), that:
//!   - SMTP-03: a confirmed `email.send` effect results in exactly one message
//!     captured by Mailpit, addressed to the intended recipient — sent by the
//!     broker/confirm process (`confirmation.rs::confirm()`), never a stub.
//!   - SMTP-05: a tainted body carrying a CR/LF-then-`Bcc:` injection sequence
//!     does NOT smuggle a recipient into the captured envelope — verified by
//!     reading Mailpit's HTTP API `To`/`Cc`/`Bcc` fields, not merely that the
//!     send returned Ok (D-22 — "defends by construction" must be VERIFIED).
//!
//! Requires a running Mailpit sidecar (`scripts/mailpit-verify.sh`) — Linux
//! kernel confinement is irrelevant here (this drives the confirm-path
//! process directly, in-process, exactly as a real `caprun confirm` would),
//! but the whole harness is `#[cfg(target_os = "linux")]`-gated to match this
//! project's established "Mailpit-backed tests only run under the Colima
//! Linux recipe" convention (CLAUDE.md) — `cargo test -p brokerd` on macOS
//! shows these as 0 passed, expected, not a gap.
//!
//! # Mailpit HTTP API field path (empirically confirmed, Task 1 / 13-04-SUMMARY.md)
//!
//! `GET /api/v1/message/{ID}` (the DETAIL endpoint, NOT the LIST endpoint —
//! the LIST endpoint returns `null` for absent Cc/Bcc, the DETAIL endpoint
//! always returns an array) returns `To`/`Cc`/`Bcc` as arrays of
//! `{"Name": "...", "Address": "..."}`. This is the field path both tests
//! assert against.

#[cfg(target_os = "linux")]
use adapter_fs::workspace::WorkspaceRoot;
#[cfg(target_os = "linux")]
use brokerd::audit::{append_event, insert_blocked_literal, open_audit_db};
#[cfg(target_os = "linux")]
use brokerd::confirmation::{
    combined_digest, confirm, insert_pending_confirmation, ConfirmOutcome, PendingConfirmation,
    PendingConfirmationState, ResolvedArg,
};
#[cfg(target_os = "linux")]
use runtime_core::executor_decision::SinkBlockedAnchor;
#[cfg(target_os = "linux")]
use runtime_core::plan_node::{SinkId, TaintLabel, ValueId};
#[cfg(target_os = "linux")]
use runtime_core::Event;
#[cfg(target_os = "linux")]
use sha2::{Digest, Sha256};
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::net::TcpStream;
#[cfg(target_os = "linux")]
use std::time::Duration;
#[cfg(target_os = "linux")]
use uuid::Uuid;

/// Read the Mailpit host — the SAME env var the broker/confirm process reads
/// for the SMTP connection itself (`CAPRUN_SMTP_HOST`, D-04 endpoint
/// sourcing). Defaults to `127.0.0.1` for a locally-running Mailpit instance
/// outside the sidecar's Docker network.
#[cfg(target_os = "linux")]
fn mailpit_host() -> String {
    std::env::var("CAPRUN_SMTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string())
}

/// Mailpit's HTTP API port is FIXED at 8025 by Mailpit's own convention —
/// distinct from the SMTP port read via `CAPRUN_SMTP_PORT` (1025).
#[cfg(target_os = "linux")]
const MAILPIT_HTTP_PORT: u16 = 8025;

/// Serializes the two Mailpit-backed acceptance tests in this file against
/// each other. Both tests share ONE external Mailpit inbox (a real SMTP
/// server, not an in-process fixture) — `cargo test`'s default
/// multi-threaded runner would otherwise let them race on the same inbox
/// (one test's `wait_for_message_count` could observe the OTHER test's
/// message). Each test acquires this lock for its entire body, mirroring
/// `email_smtp.rs::SMTP_ENV_LOCK`'s rationale for a different shared
/// process-global resource.
#[cfg(target_os = "linux")]
static MAILPIT_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Minimal dependency-light raw HTTP/1.1 client (no new HTTP crate
/// dependency introduced — matches this phase's "keep it dependency-light"
/// instruction). Sends `Connection: close` so Mailpit closes the socket
/// after replying, letting a simple read-to-EOF loop work without needing to
/// parse `Content-Length` or chunked-encoding framing (empirically verified
/// against a live Mailpit instance during Task 1: it honors `Connection: close`).
#[cfg(target_os = "linux")]
fn http_request(method: &str, host: &str, port: u16, path: &str) -> String {
    let mut stream = TcpStream::connect((host, port))
        .unwrap_or_else(|e| panic!("failed to connect to Mailpit HTTP API at {host}:{port}: {e}"));
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let request =
        format!("{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).unwrap();
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .unwrap_or_else(|e| panic!("failed reading Mailpit HTTP API response: {e}"));
    let text = String::from_utf8_lossy(&raw).into_owned();
    text.split_once("\r\n\r\n")
        .map(|(_, b)| b.to_string())
        .unwrap_or_else(|| panic!("malformed HTTP response from Mailpit (no header/body separator): {text}"))
}

/// GET a path from Mailpit's HTTP API, parsed as JSON.
#[cfg(target_os = "linux")]
fn http_get_json(host: &str, port: u16, path: &str) -> serde_json::Value {
    let body = http_request("GET", host, port, path);
    serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("failed to parse Mailpit JSON response: {e}\nbody: {body}"))
}

/// Delete ALL messages from Mailpit's inbox (`DELETE /api/v1/messages`) —
/// called at the start of each test (under `MAILPIT_TEST_LOCK`) so each test
/// starts from a known-empty inbox regardless of what a prior test run left
/// behind (Mailpit's inbox otherwise persists across test runs against the
/// same sidecar container).
#[cfg(target_os = "linux")]
fn clear_mailpit_inbox(host: &str) {
    let _ = http_request("DELETE", host, MAILPIT_HTTP_PORT, "/api/v1/messages");
}

/// Poll Mailpit's LIST endpoint until at least `expected_count` messages have
/// arrived, returning their `ID`s. A real SMTP send completes asynchronously
/// relative to this HTTP poll — bound the wait so a genuine failure doesn't
/// hang forever.
#[cfg(target_os = "linux")]
fn wait_for_message_count(host: &str, expected_count: usize) -> Vec<String> {
    for _ in 0..50 {
        let list = http_get_json(host, MAILPIT_HTTP_PORT, "/api/v1/messages");
        let messages = list["messages"].as_array().cloned().unwrap_or_default();
        if messages.len() >= expected_count {
            return messages
                .iter()
                .filter_map(|m| m["ID"].as_str().map(String::from))
                .collect();
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    panic!("timed out waiting for {expected_count} message(s) to appear in Mailpit");
}

/// Fetch a single message's DETAIL via Mailpit's HTTP API — the endpoint
/// whose `To`/`Cc`/`Bcc` fields are always arrays (see module doc comment).
#[cfg(target_os = "linux")]
fn fetch_message_detail(host: &str, id: &str) -> serde_json::Value {
    http_get_json(host, MAILPIT_HTTP_PORT, &format!("/api/v1/message/{id}"))
}

/// Extract every `Address` string from a `To`/`Cc`/`Bcc` array field (empty
/// vec if the field is missing, null, or not an array).
#[cfg(target_os = "linux")]
fn addresses(detail: &serde_json::Value, field: &str) -> Vec<String> {
    detail[field]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| entry["Address"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Seed a Pending `email.send` block (mirroring
/// `confirmation::tests::seed_pending_email_send_block` — duplicated here,
/// not made `pub(crate)`, because this integration-test binary has no access
/// to `brokerd`'s private test module) and drive it through `confirm()` —
/// the SAME entry point `caprun confirm <effect_id>` uses, proving the send
/// comes from the broker/confirm process, never a test-only bypass.
#[cfg(target_os = "linux")]
fn seed_and_confirm_email_send(
    conn: &mut rusqlite::Connection,
    to: &str,
    subject: &str,
    body: &str,
) -> ConfirmOutcome {
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
    // CONFIRM-03 (Round-6): computed once over the FULL resolved_args set,
    // threaded into BOTH the sink_blocked Event and the PendingConfirmation
    // below — mirrors server.rs's Block-time write.
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

    let mut root_dir = std::env::temp_dir();
    root_dir.push(format!("caprun_email_smtp_acceptance_{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root_dir).unwrap();
    let ws = WorkspaceRoot::open(&root_dir).unwrap();

    let outcome = confirm(conn, &effect_id.to_string(), &ws).expect("confirm");
    std::fs::remove_dir_all(&root_dir).ok();
    outcome
}

/// (SMTP-03) A confirmed `email.send` effect (clean recipient/subject/body)
/// results in exactly one message captured by Mailpit, addressed to the
/// intended recipient — sent by the broker/confirm process.
#[cfg(target_os = "linux")]
#[test]
fn smtp_03_confirmed_send_captured_by_mailpit() {
    let _guard = MAILPIT_TEST_LOCK.lock().unwrap();
    let host = mailpit_host();
    clear_mailpit_inbox(&host);
    let mut conn = open_audit_db(":memory:").expect("open_audit_db");

    let outcome =
        seed_and_confirm_email_send(&mut conn, "recipient@example.com", "hello", "hi there");
    assert_eq!(
        outcome,
        ConfirmOutcome::Released,
        "a confirmed clean email.send must Release (real send succeeded)"
    );

    let ids = wait_for_message_count(&host, 1);
    assert_eq!(
        ids.len(),
        1,
        "expected exactly one captured message addressed to the intended recipient"
    );

    let detail = fetch_message_detail(&host, &ids[0]);
    let to = addresses(&detail, "To");
    assert_eq!(
        to,
        vec!["recipient@example.com".to_string()],
        "captured message must be addressed to the intended recipient"
    );
}

/// (SMTP-05) A tainted `body` literal carrying a CR/LF-then-`Bcc:` injection
/// sequence does NOT smuggle a recipient into the captured envelope —
/// Mailpit's parsed `To`/`Cc`/`Bcc` show ONLY the intended recipient, no
/// attacker address (D-22 — "defends by construction" must be VERIFIED, not
/// assumed from lettre's reputation). The attack fails because the body is
/// written into the message strictly AFTER the RFC 5322 header/body
/// separator — the `to` recipient, in contrast, IS parsed via `Address` and
/// would fail closed if itself CRLF-bearing (see `email_smtp.rs`'s own
/// `build_message_rejects_crlf_in_to_fail_closed` unit test for that
/// boundary) — this fixture targets the OTHER half of D-22: a clean
/// recipient with a CRLF-injected BODY.
#[cfg(target_os = "linux")]
#[test]
fn smtp_05_crlf_body_cannot_smuggle_recipient() {
    let _guard = MAILPIT_TEST_LOCK.lock().unwrap();
    let host = mailpit_host();
    clear_mailpit_inbox(&host);
    let mut conn = open_audit_db(":memory:").expect("open_audit_db");

    let malicious_body = "hi there\r\nBcc: attacker@evil.com";
    let outcome =
        seed_and_confirm_email_send(&mut conn, "victim@example.com", "hello", malicious_body);
    assert_eq!(
        outcome,
        ConfirmOutcome::Released,
        "a clean recipient with a CRLF-bearing BODY must still Release — only \
         the recipient literal is parsed via Address, never the body"
    );

    let ids = wait_for_message_count(&host, 1);
    assert_eq!(ids.len(), 1, "expected exactly one captured message");

    let detail = fetch_message_detail(&host, &ids[0]);
    let to = addresses(&detail, "To");
    let cc = addresses(&detail, "Cc");
    let bcc = addresses(&detail, "Bcc");

    assert_eq!(
        to,
        vec!["victim@example.com".to_string()],
        "To must contain ONLY the intended recipient"
    );
    assert!(cc.is_empty(), "Cc must be empty — no smuggled recipient");
    assert!(
        !bcc.contains(&"attacker@evil.com".to_string()),
        "Bcc must NOT contain the smuggled attacker address: {bcc:?}"
    );
    assert!(
        bcc.is_empty(),
        "Bcc must be empty — the CR/LF-then-Bcc: sequence in the body must \
         NEVER become a real recipient at Mailpit: {bcc:?}"
    );
}
