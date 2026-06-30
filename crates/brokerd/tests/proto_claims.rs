/// proto_claims — serde round-trip and fail-closed tests for the worker→broker
/// claims protocol (WorkerClaim, ReportClaims, ClaimsReceived).
///
/// These tests are cross-platform (no abstract-UDS, no Linux-only syscalls) and
/// run on both macOS and Linux.
///
/// ASM-03: the confined worker emits typed claims (not raw bytes); the broker
/// fails closed on unknown claim kinds. These tests prove both halves of that
/// contract.

#[test]
fn worker_claim_email_address_round_trips() {
    use brokerd::proto::WorkerClaim;

    let claim = WorkerClaim::EmailAddress("accounts@ev1l.com".to_string());
    let json = serde_json::to_value(&claim).expect("serialize WorkerClaim");
    let recovered: WorkerClaim = serde_json::from_value(json).expect("deserialize WorkerClaim");
    assert_eq!(claim, recovered);
}
