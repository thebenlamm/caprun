/// proto_claims — serde round-trip and fail-closed tests for the worker→broker
/// claims protocol (WorkerClaim, ReportClaims, ClaimsReceived).
///
/// These tests are cross-platform (no abstract-UDS, no Linux-only syscalls) and
/// run on both macOS and Linux.
///
/// ASM-03: the confined worker emits typed claims (not raw bytes); the broker
/// fails closed on unknown claim kinds. These tests prove both halves of that
/// contract.

/// Test 1: BrokerRequest::ReportClaims containing one WorkerClaim::EmailAddress
/// round-trips through serde_json to an equal value.
///
/// Proves the wire format for the worker→broker claims submission path.
#[test]
fn report_claims_request_round_trips() {
    use brokerd::proto::{BrokerRequest, WorkerClaim};

    let req = BrokerRequest::ReportClaims {
        claims: vec![WorkerClaim::EmailAddress("accounts@ev1l.com".to_string())],
    };
    let json = serde_json::to_value(&req).expect("serialize ReportClaims request");
    let recovered: BrokerRequest =
        serde_json::from_value(json).expect("deserialize ReportClaims request");
    assert_eq!(req, recovered);
}

/// Test 2: BrokerResponse::ClaimsReceived containing one freshly-minted ValueId
/// round-trips through serde_json to an equal value.
///
/// Proves the wire format for the broker→worker opaque handle response path.
#[test]
fn claims_received_response_round_trips() {
    use brokerd::proto::BrokerResponse;
    use runtime_core::plan_node::ValueId;

    let value_id = ValueId::new();
    let resp = BrokerResponse::ClaimsReceived {
        value_ids: vec![value_id],
    };
    let json = serde_json::to_string(&resp).expect("serialize ClaimsReceived response");
    let recovered: BrokerResponse =
        serde_json::from_str(&json).expect("deserialize ClaimsReceived response");
    assert_eq!(resp, recovered);
}

/// Test 2b: BrokerRequest::ProvideIntent containing a SendEmailSummary intent
/// round-trips through serde_json to an equal value.
#[test]
fn provide_intent_request_round_trips() {
    use brokerd::proto::BrokerRequest;
    use runtime_core::intent::CaprunIntent;

    let req = BrokerRequest::ProvideIntent {
        intent: CaprunIntent::SendEmailSummary {
            recipient: "boss@company.com".to_string(),
        },
    };
    let json = serde_json::to_value(&req).expect("serialize ProvideIntent request");
    let recovered: BrokerRequest =
        serde_json::from_value(json).expect("deserialize ProvideIntent request");
    assert_eq!(req, recovered);
}

/// Test 2c: BrokerResponse::IntentAccepted containing a freshly-minted ValueId
/// round-trips through serde_json to an equal value.
#[test]
fn intent_accepted_response_round_trips() {
    use brokerd::proto::BrokerResponse;
    use runtime_core::plan_node::ValueId;

    let value_id = ValueId::new();
    let resp = BrokerResponse::IntentAccepted {
        value_id: value_id.clone(),
    };
    let json = serde_json::to_string(&resp).expect("serialize IntentAccepted response");
    let recovered: BrokerResponse =
        serde_json::from_str(&json).expect("deserialize IntentAccepted response");
    assert_eq!(resp, recovered);
}

/// Test 2d: ProvideIntent dispatch through dispatch_request returns IntentAccepted
/// with a ValueId that resolves in the per-connection store.
///
/// Proves: the ProvideIntent arm calls mint_from_intent inside the per-connection
/// ValueStore and returns the correct response variant (T-06-05: Pitfall 1 — the
/// ValueId MUST be resolvable within the connection's store, not a dangling handle).
#[tokio::test]
async fn provide_intent_dispatch_returns_intent_accepted_with_resolvable_handle() {
    use brokerd::audit::open_audit_db;
    use brokerd::proto::{BrokerRequest, BrokerResponse};
    use brokerd::server::dispatch_request;
    use executor::value_store::ValueStore;
    use runtime_core::intent::CaprunIntent;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
    let session_id = Uuid::new_v4();
    let mut store = ValueStore::default();
    let mut last_event_id = Uuid::new_v4();
    let mut last_event_hash = "genesis-hash".to_string();
    let mut session_status = runtime_core::SessionStatus::Active;
    // ProvideIntent never exercises RequestFd; any valid dir anchors the root.
    let ws_root = Arc::new(
        adapter_fs::workspace::WorkspaceRoot::open(std::env::temp_dir().as_path())
            .expect("open ws root"),
    );

    let (mut server_end, mut client_end) =
        tokio::net::UnixStream::pair().expect("UnixStream::pair");

    // Send ProvideIntent through the real dispatch arm.
    dispatch_request(
        BrokerRequest::ProvideIntent {
            intent: CaprunIntent::SendEmailSummary {
                recipient: "boss@company.com".to_string(),
            },
        },
        &mut server_end,
        &conn,
        session_id,
        &mut last_event_id,
        &mut last_event_hash,
        &mut store,
        &ws_root,
        &mut session_status,
    )
    .await
    .expect("dispatch ProvideIntent must succeed");

    // Read the response from the client end.
    use tokio::io::AsyncReadExt;
    let mut len_buf = [0u8; 4];
    client_end.read_exact(&mut len_buf).await.expect("read len");
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    client_end.read_exact(&mut body).await.expect("read body");
    let response: BrokerResponse = serde_json::from_slice(&body).expect("deserialize response");

    // Response must be IntentAccepted with a store-resolvable ValueId (Pitfall 1).
    match response {
        BrokerResponse::IntentAccepted { value_id } => {
            let record = store.resolve(&value_id).expect(
                "ValueId from IntentAccepted must resolve in the per-connection store (Pitfall 1: \
                 if None here, mint happened in the wrong store scope)",
            );
            assert_eq!(
                record.literal, "boss@company.com",
                "minted record literal must equal the intent recipient"
            );
            // The record must carry UserTrusted (positive provenance — not empty, Pitfall 2).
            use runtime_core::plan_node::TaintLabel;
            assert!(
                record.taint.contains(&TaintLabel::UserTrusted),
                "minted record must carry UserTrusted taint"
            );
            // And NOT any untrusted label.
            assert!(
                !record.taint.iter().any(|t| t.is_untrusted()),
                "minted record must not carry any untrusted labels"
            );
        }
        other => panic!(
            "expected IntentAccepted response to ProvideIntent, got {:?}",
            other
        ),
    }

    // Causal chain must have advanced.
    let locked = conn.lock().unwrap();
    let evt = brokerd::audit::find_event_by_type(&locked, &session_id.to_string(), "intent_received")
        .expect("find_event_by_type")
        .expect("intent_received event must exist in audit DAG after ProvideIntent dispatch");
    assert_eq!(
        evt.id, last_event_id,
        "causal chain must advance to the intent_received event id"
    );
}

/// Test 3: Deserializing a WorkerClaim JSON document whose `kind` field is an
/// unrecognized name returns an Err (fail closed).
///
/// Proves ASM-03's fail-closed contract: the exhaustive WorkerClaim enum (no
/// wildcard / other-arm) prevents the broker from silently accepting future or
/// attacker-crafted claim types that have not been audited and added to the enum.
/// Unknown claim kinds fail closed — they are rejected at the IPC boundary.
#[test]
fn unknown_claim_kind_fails_closed() {
    // Construct a valid internally-tagged shape (`kind` + `value`) but with a
    // bogus tag value that will never be a real WorkerClaim variant.
    // DO NOT add any production code that would make this succeed.
    let unknown_kind_json = r#"{"kind":"TotallyUnknownKind","value":"some-value"}"#;
    let result: Result<brokerd::proto::WorkerClaim, _> =
        serde_json::from_str(unknown_kind_json);
    assert!(
        result.is_err(),
        "WorkerClaim deserialization of an unknown kind must fail closed (Err), \
         but it returned Ok — this proves an unknown claim kind was accepted, \
         violating ASM-03 fail-closed contract"
    );
}
