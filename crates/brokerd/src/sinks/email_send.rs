/// sinks/email_send — mediated email.send sink stub
///
/// This stub records an `email_send_stub` Event in the append-only audit DAG
/// and NEVER sends email. It is the post-confirmation dispatch target for the
/// email.send sink; the executor always evaluates the plan node first and blocks
/// when tainted values are present.
///
/// Security note (T-04-05): every would-be invocation leaves a tamper-evident
/// trace in the DAG. No raw literals (recipient address) are written to the
/// payload field — only opaque metadata (sink name, session).
///
/// Post-v0 extension point: replace this stub with the live SMTP/API path once
/// the FAMP confirmation wire and human-approval UX are production-hardened.

use anyhow::Result;
use chrono::Utc;
use runtime_core::{Event, PlanNode};
use uuid::Uuid;

use crate::audit;

/// Record an `email_send_stub` Event in the audit DAG and return its hash.
///
/// Performs NO network or SMTP action. The only effect is appending the audit
/// event so the §9 chain assertion can verify the invocation was recorded.
///
/// # Arguments
/// * `conn`        — open rusqlite connection (broker-owned).
/// * `session_id`  — the active broker session.
/// * `plan_node`   — the plan node describing the email.send call (opaque handles only).
/// * `parent_hash` — hash of the causal predecessor Event (`None` for first event).
///
/// # No raw literals in payload
/// PlanNode.args carry only opaque ValueId handles — no literal recipient
/// address — so the audit row does not repeat the hostile literal.
/// The §9 held-out test never reaches this function because the executor
/// blocks first; the stub is exercised by unit tests calling it directly.
pub fn invoke_email_send_stub(
    conn: &rusqlite::Connection,
    session_id: Uuid,
    plan_node: &PlanNode,
    parent_hash: Option<&str>,
) -> Result<String> {
    let _ = plan_node; // Opaque handles only — not embedded in the event payload.
    let event = Event {
        id: Uuid::new_v4(),
        parent_id: None,
        session_id,
        actor: "sink-stub:email.send".to_string(),
        event_type: "email_send_stub".to_string(),
        timestamp: Utc::now(),
        taint: vec![], // Stub carries no taint — taint lives on the blocked ValueRecord.
    };
    audit::append_event(conn, &event, parent_hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit;
    use runtime_core::{PlanNode, SinkId};

    #[test]
    fn invoke_email_send_stub_records_email_send_stub_event() {
        let conn = audit::open_audit_db(":memory:").expect("open_audit_db");
        let session_id = Uuid::new_v4();
        let plan_node = PlanNode {
            sink: SinkId("email.send".to_string()),
            args: vec![],
        };

        let hash = invoke_email_send_stub(&conn, session_id, &plan_node, None)
            .expect("invoke_email_send_stub");

        assert!(!hash.is_empty(), "hash must be non-empty");

        let found =
            audit::find_event_by_type(&conn, &session_id.to_string(), "email_send_stub")
                .expect("find_event_by_type")
                .expect("email_send_stub event must be present");

        assert_eq!(found.actor, "sink-stub:email.send");
        assert_eq!(found.event_type, "email_send_stub");
        assert!(found.taint.is_empty(), "stub event carries no taint");
    }
}
