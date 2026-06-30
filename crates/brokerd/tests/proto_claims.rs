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
