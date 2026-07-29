/// proto_claims — serde round-trip and fail-closed tests for the worker→broker
/// claims protocol (WorkerClaim, ReportClaims, ClaimsReceived).
///
/// These tests are cross-platform (no abstract-UDS, no Linux-only syscalls) and
/// run on both macOS and Linux.
///
/// ASM-03: the confined worker emits typed claims (not raw bytes); the broker
/// fails closed on unknown claim kinds. These tests prove both halves of that
/// contract.

/// Fixed, non-secret test MAC key (v1.6 Phase 28, HARDEN-02) — shared by
/// every `dispatch_request` call in this file (both the standalone Test 2d
/// and `DispatchHarness::dispatch`).
const TEST_KEY: &[u8] = b"proto-claims-rs-integration-test-key";

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
            subject: "Q3 summary".to_string(),
            body: "See attached.".to_string(),
        },
        primary_file_derived: false,
    };
    let json = serde_json::to_value(&req).expect("serialize ProvideIntent request");
    let recovered: BrokerRequest =
        serde_json::from_value(json).expect("deserialize ProvideIntent request");
    assert_eq!(req, recovered);
}

/// Test 2c: BrokerResponse::IntentAccepted containing a freshly-minted ValueId
/// round-trips through serde_json to an equal value.
///
/// Phase 15 (15-04, finding #6): IntentAccepted gained additive
/// `subject_value_id`/`body_value_id` fields — exercises the `Some` case for
/// both (the `SendEmailSummary` shape) so the additive fields' serde shape is
/// proven, not just `value_id`.
#[test]
fn intent_accepted_response_round_trips() {
    use brokerd::proto::BrokerResponse;
    use runtime_core::plan_node::ValueId;

    let value_id = ValueId::new();
    let resp = BrokerResponse::IntentAccepted {
        value_id: value_id.clone(),
        subject_value_id: Some(ValueId::new()),
        body_value_id: Some(ValueId::new()),
        named_handles: vec![],
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
    // v1.6 Phase 27 (X-04/F3): dispatch_request now takes the shared
    // Arc<Mutex<SessionStatus>> shape — a fresh test-local cell here.
    let session_status = Arc::new(Mutex::new(runtime_core::SessionStatus::Active));
    // ProvideIntent never exercises RequestFd; any valid dir anchors the root.
    let ws_root = Arc::new(
        adapter_fs::workspace::WorkspaceRoot::open(std::env::temp_dir().as_path())
            .expect("open ws root"),
    );
    // Trusted-inode placeholder (HARDEN-01, review Fix 2) — this test
    // never drives RequestFd, so the fstat identity compare is never reached.
    let trusted_inode: Option<(u64, u64)> = None;

    let (mut server_end, mut client_end) =
        tokio::net::UnixStream::pair().expect("UnixStream::pair");

    // Phase 16 (BLOCKER-1 guard a): dispatch_request gained two new
    // per-connection `&mut bool` ordering-guard params; this test drives a
    // single ProvideIntent and never RequestFd, so fresh `false` locals are correct.
    let mut intent_provided = false;
    let mut fd_requested = false;
    let mut fd_request_count: u32 = 0;

    // Send ProvideIntent through the real dispatch arm.
    dispatch_request(
        BrokerRequest::ProvideIntent {
            intent: CaprunIntent::SendEmailSummary {
                recipient: "boss@company.com".to_string(),
                subject: "Q3 summary".to_string(),
                body: "See attached.".to_string(),
            },
            primary_file_derived: false,
        },
        &mut server_end,
        &conn,
        TEST_KEY,
        session_id,
        &mut last_event_id,
        &mut last_event_hash,
        &mut store,
        &ws_root,
        &session_status,
        &runtime_core::SessionPolicy::allow_all(),
        trusted_inode,
        &mut intent_provided,
        &mut fd_requested,
        &mut fd_request_count,
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
        BrokerResponse::IntentAccepted {
            value_id,
            subject_value_id,
            body_value_id,
            named_handles,
        } => {
            // Phase 15 (15-04, finding #6): SendEmailSummary mints THREE
            // DISTINCT UserTrusted handles — subject/body must be present and
            // distinct from the recipient handle (never degenerately equal).
            let subject_value_id = subject_value_id.expect("subject_value_id must be Some for SendEmailSummary");
            let body_value_id = body_value_id.expect("body_value_id must be Some for SendEmailSummary");
            assert_ne!(subject_value_id, value_id, "subject handle must be DISTINCT from the recipient handle");
            assert_ne!(body_value_id, value_id, "body handle must be DISTINCT from the recipient handle");
            assert_ne!(subject_value_id, body_value_id, "subject and body handles must be DISTINCT from each other");
            // Phase 49: email keeps three-slot shape — named_handles empty.
            assert!(
                named_handles.is_empty(),
                "SendEmailSummary named_handles must be empty (coding-only field)"
            );
            let subject_record = store
                .resolve(&subject_value_id)
                .expect("subject_value_id must resolve in the per-connection store");
            assert_eq!(subject_record.literal, "Q3 summary");
            let body_record = store
                .resolve(&body_value_id)
                .expect("body_value_id must resolve in the per-connection store");
            assert_eq!(body_record.literal, "See attached.");

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

    // Causal chain must have advanced. Phase 15 (15-04, finding #6): a
    // SendEmailSummary ProvideIntent now appends THREE intent_received events
    // (recipient, subject, body) via three sequential mint_from_intent calls —
    // `find_event_by_type`'s LIMIT-1 would return only the FIRST (recipient's),
    // not the causal chain head, so resolve `last_event_id` directly by id
    // instead and assert it IS an intent_received event.
    let locked = conn.lock().unwrap();
    let evt = brokerd::audit::find_event_by_id(&locked, &session_id.to_string(), &last_event_id.to_string())
        .expect("find_event_by_id")
        .expect("last_event_id must resolve to a real event in the audit DAG");
    assert_eq!(
        evt.event_type, "intent_received",
        "causal chain must advance to an intent_received event (the LAST of the three \
         sequential mints — recipient, subject, body)"
    );

    // Sanity: exactly three intent_received events exist (recipient, subject, body).
    let intent_received_count: i64 = locked
        .query_row(
            "SELECT COUNT(*) FROM events WHERE session_id = ?1 AND event_type = 'intent_received'",
            [&session_id.to_string()],
            |row| row.get(0),
        )
        .expect("query intent_received count");
    assert_eq!(
        intent_received_count, 3,
        "SendEmailSummary must mint THREE intent_received events (recipient, subject, body)"
    );
}

/// Phase 49 / CODE-02: IntentAccepted with non-empty named_handles round-trips
/// through serde_json (additive field shape for coding multi-mint).
#[test]
fn intent_accepted_named_handles_round_trips() {
    use brokerd::proto::BrokerResponse;
    use runtime_core::plan_node::ValueId;

    let write_path = ValueId::new();
    let write_contents = ValueId::new();
    let resp = BrokerResponse::IntentAccepted {
        value_id: write_path.clone(),
        subject_value_id: None,
        body_value_id: None,
        named_handles: vec![
            ("write_path".into(), write_path),
            ("write_contents".into(), write_contents),
        ],
    };
    let json = serde_json::to_string(&resp).expect("serialize IntentAccepted named_handles");
    let recovered: BrokerResponse =
        serde_json::from_str(&json).expect("deserialize IntentAccepted named_handles");
    assert_eq!(resp, recovered);
}

/// Phase 49 / CODE-02: ProvideIntent SafeCodingWorkflow (primary_file_derived
/// false) returns N distinct named UserTrusted handles; each resolves in the
/// per-connection ValueStore; path/contents carry origin_role "path"; no two
/// bag keys share the same ValueId.
#[tokio::test]
async fn provide_intent_safe_coding_multi_mint_distinct_named_handles() {
    use brokerd::audit::open_audit_db;
    use brokerd::proto::{BrokerRequest, BrokerResponse};
    use brokerd::server::dispatch_request;
    use executor::value_store::ValueStore;
    use runtime_core::intent::CaprunIntent;
    use runtime_core::plan_node::TaintLabel;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
    let session_id = Uuid::new_v4();
    let mut store = ValueStore::default();
    let mut last_event_id = Uuid::new_v4();
    let mut last_event_hash = "genesis-hash".to_string();
    let session_status = Arc::new(Mutex::new(runtime_core::SessionStatus::Active));
    let ws_root = Arc::new(
        adapter_fs::workspace::WorkspaceRoot::open(std::env::temp_dir().as_path())
            .expect("open ws root"),
    );
    let trusted_inode: Option<(u64, u64)> = None;
    let (mut server_end, mut client_end) =
        tokio::net::UnixStream::pair().expect("UnixStream::pair");
    let mut intent_provided = false;
    let mut fd_requested = false;
    let mut fd_request_count: u32 = 0;

    let coding = CaprunIntent::SafeCodingWorkflow {
        path: "src/lib.rs".into(),
        contents: "fn main() {}".into(),
        test_command: "sh".into(),
        test_args_json: r#"["-c","cargo test"]"#.into(),
        commit_message: "feat: coding".into(),
        remote: "origin".into(),
        refspec: "HEAD:refs/heads/feature".into(),
        owner: "acme".into(),
        repo: "caprun".into(),
        base: "main".into(),
        head: "feature".into(),
        pr_title: "PR title".into(),
        pr_body: "PR body".into(),
    };

    dispatch_request(
        BrokerRequest::ProvideIntent {
            intent: coding,
            primary_file_derived: false,
        },
        &mut server_end,
        &conn,
        TEST_KEY,
        session_id,
        &mut last_event_id,
        &mut last_event_hash,
        &mut store,
        &ws_root,
        &session_status,
        &runtime_core::SessionPolicy::allow_all(),
        trusted_inode,
        &mut intent_provided,
        &mut fd_requested,
        &mut fd_request_count,
    )
    .await
    .expect("dispatch ProvideIntent SafeCodingWorkflow must succeed");

    use tokio::io::AsyncReadExt;
    let mut len_buf = [0u8; 4];
    client_end.read_exact(&mut len_buf).await.expect("read len");
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; msg_len];
    client_end.read_exact(&mut body).await.expect("read body");
    let response: BrokerResponse = serde_json::from_slice(&body).expect("deserialize response");

    let expected_keys: [&str; 13] = [
        "write_path",
        "write_contents",
        "test_command",
        "test_args",
        "commit_message",
        "push_remote",
        "push_refspec",
        "pr_owner",
        "pr_repo",
        "pr_base",
        "pr_head",
        "pr_title",
        "pr_body",
    ];

    match response {
        BrokerResponse::IntentAccepted {
            value_id,
            subject_value_id,
            body_value_id,
            named_handles,
        } => {
            assert!(
                subject_value_id.is_none() && body_value_id.is_none(),
                "coding uses named_handles, not email three-slot subject/body"
            );
            assert_eq!(
                named_handles.len(),
                expected_keys.len(),
                "SafeCodingWorkflow must mint one handle per bag key"
            );

            let mut seen_ids = HashSet::new();
            let mut by_key = std::collections::HashMap::new();
            for (key, vid) in &named_handles {
                assert!(
                    seen_ids.insert(vid.clone()),
                    "bag key {key} must have a DISTINCT ValueId (T-49-04)"
                );
                by_key.insert(key.as_str(), vid.clone());
            }
            for k in expected_keys {
                assert!(
                    by_key.contains_key(k),
                    "named_handles must include bag key {k}"
                );
            }

            // Primary value_id is the write_path handle.
            assert_eq!(
                &value_id,
                by_key.get("write_path").expect("write_path present"),
                "primary value_id must be the write_path handle"
            );

            // Every handle resolves UserTrusted with no untrusted labels.
            for (key, vid) in &named_handles {
                let record = store.resolve(vid).unwrap_or_else(|| {
                    panic!("named handle {key} must resolve in per-connection store")
                });
                assert!(
                    record.taint.contains(&TaintLabel::UserTrusted),
                    "named handle {key} must carry UserTrusted"
                );
                assert!(
                    !record.taint.iter().any(|t| t.is_untrusted()),
                    "named handle {key} must not carry untrusted labels"
                );
            }

            // path/contents origin_role usable for file.write Step 1c role gate.
            let path_rec = store
                .resolve(by_key.get("write_path").unwrap())
                .expect("write_path resolve");
            assert_eq!(
                path_rec.origin_role.as_deref(),
                Some("path"),
                "write_path origin_role must be Some(\"path\")"
            );
            assert_eq!(path_rec.literal, "src/lib.rs");
            let contents_rec = store
                .resolve(by_key.get("write_contents").unwrap())
                .expect("write_contents resolve");
            assert_eq!(
                contents_rec.origin_role.as_deref(),
                Some("path"),
                "write_contents origin_role must be Some(\"path\")"
            );
            assert_eq!(contents_rec.literal, "fn main() {}");
        }
        other => panic!(
            "expected IntentAccepted for SafeCodingWorkflow, got {:?}",
            other
        ),
    }

    // 13 sequential mint_from_intent calls → 13 intent_received events.
    let locked = conn.lock().unwrap();
    let intent_received_count: i64 = locked
        .query_row(
            "SELECT COUNT(*) FROM events WHERE session_id = ?1 AND event_type = 'intent_received'",
            [&session_id.to_string()],
            |row| row.get(0),
        )
        .expect("query intent_received count");
    assert_eq!(
        intent_received_count, 13,
        "SafeCodingWorkflow must mint 13 distinct intent_received events"
    );
    assert!(
        intent_provided,
        "intent_provided must be true after successful multi-mint"
    );
}

/// Test 2e: `WorkerClaim::DocFragment` round-trips through serde_json to an
/// equal value when wrapped in a `ReportClaims` request (additive variant,
/// Phase 15 15-03).
#[test]
fn doc_fragment_claim_round_trips_via_report_claims() {
    use brokerd::proto::{BrokerRequest, WorkerClaim};

    let req = BrokerRequest::ReportClaims {
        claims: vec![WorkerClaim::DocFragment("accounts".to_string())],
    };
    let json = serde_json::to_value(&req).expect("serialize ReportClaims(DocFragment) request");
    let recovered: BrokerRequest =
        serde_json::from_value(json).expect("deserialize ReportClaims(DocFragment) request");
    assert_eq!(req, recovered);
}

/// Test 2f: `BrokerRequest::ReportDerivedClaim` round-trips through serde_json
/// to an equal value (additive variant, Phase 15 15-03).
#[test]
fn report_derived_claim_request_round_trips() {
    use brokerd::proto::{BrokerRequest, TransformKind};
    use runtime_core::plan_node::ValueId;

    let req = BrokerRequest::ReportDerivedClaim {
        transformed_literal: "accounts@ev1l.com".to_string(),
        transform: TransformKind::Concat,
        input_value_ids: vec![ValueId::new(), ValueId::new()],
    };
    let json = serde_json::to_value(&req).expect("serialize ReportDerivedClaim request");
    let recovered: BrokerRequest =
        serde_json::from_value(json).expect("deserialize ReportDerivedClaim request");
    assert_eq!(req, recovered);
}

/// Test 2g: `BrokerResponse::DerivedClaimReceived` round-trips through
/// serde_json to an equal value (additive variant, Phase 15 15-03).
#[test]
fn derived_claim_received_response_round_trips() {
    use brokerd::proto::BrokerResponse;
    use runtime_core::plan_node::ValueId;

    let resp = BrokerResponse::DerivedClaimReceived {
        value_id: ValueId::new(),
    };
    let json = serde_json::to_string(&resp).expect("serialize DerivedClaimReceived response");
    let recovered: BrokerResponse =
        serde_json::from_str(&json).expect("deserialize DerivedClaimReceived response");
    assert_eq!(resp, recovered);
}

// -----------------------------------------------------------------------
// Live-dispatch tests (Phase 15 15-03, Task 2): these exercise
// `brokerd::server::dispatch_request` directly -- the SAME function the real
// broker calls per incoming IPC message -- rather than calling
// `quarantine::mint_from_derivation`/`mint_from_read` standalone. This is the
// distinction findings #1c/#3/MAJOR-1 require: a hand-built record walking a
// unit test proves nothing about what the LIVE WIRE actually does for a real
// worker message.
// -----------------------------------------------------------------------

/// Shared harness: fresh in-memory audit DB, per-connection ValueStore,
/// causal-chain locals, and a connected `UnixStream::pair` -- mirrors the
/// setup `provide_intent_dispatch_returns_intent_accepted_with_resolvable_handle`
/// (Test 2d, above) already established, factored out so the new tests below
/// don't repeat it five times.
struct DispatchHarness {
    conn: std::sync::Arc<std::sync::Mutex<rusqlite::Connection>>,
    session_id: uuid::Uuid,
    store: executor::value_store::ValueStore,
    last_event_id: uuid::Uuid,
    last_event_hash: String,
    // v1.6 Phase 27 (X-04/F3): dispatch_request now takes the shared
    // Arc<Mutex<SessionStatus>> shape — a fresh test-local cell here.
    session_status: std::sync::Arc<std::sync::Mutex<runtime_core::SessionStatus>>,
    // Trusted-inode placeholder (HARDEN-01, review Fix 2) — this harness
    // never drives RequestFd, so the fstat identity compare is never reached.
    trusted_inode: Option<(u64, u64)>,
    // Phase 16 (BLOCKER-1 guard a): threaded across every `.dispatch()` call
    // on this harness instance, exactly like `session_status` — a test that
    // drives multiple requests through the SAME harness sees the guard
    // persist across them, mirroring a real connection's per-connection state.
    intent_provided: bool,
    fd_requested: bool,
    // Phase 33 (FS-01, T-33-06/T-33-07): threaded across every `.dispatch()`
    // call on this harness instance, exactly like `fd_requested` above.
    fd_request_count: u32,
    ws_root: std::sync::Arc<adapter_fs::workspace::WorkspaceRoot>,
    server_end: tokio::net::UnixStream,
    client_end: tokio::net::UnixStream,
}

impl DispatchHarness {
    fn new() -> Self {
        use brokerd::audit::open_audit_db;
        use executor::value_store::ValueStore;
        use std::sync::{Arc, Mutex};
        use uuid::Uuid;

        let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
        let session_id = Uuid::new_v4();
        let store = ValueStore::default();
        let last_event_id = Uuid::new_v4();
        let last_event_hash = "genesis-hash".to_string();
        let session_status = Arc::new(Mutex::new(runtime_core::SessionStatus::Active));
        let trusted_inode: Option<(u64, u64)> = None;
        let ws_root = Arc::new(
            adapter_fs::workspace::WorkspaceRoot::open(std::env::temp_dir().as_path())
                .expect("open ws root"),
        );
        let (server_end, client_end) =
            tokio::net::UnixStream::pair().expect("UnixStream::pair");

        DispatchHarness {
            conn,
            session_id,
            store,
            last_event_id,
            last_event_hash,
            session_status,
            trusted_inode,
            intent_provided: false,
            fd_requested: false,
            fd_request_count: 0,
            ws_root,
            server_end,
            client_end,
        }
    }

    /// Dispatch one request through the REAL `dispatch_request` fn and read
    /// back the framed `BrokerResponse` from the client end of the pair.
    async fn dispatch(
        &mut self,
        request: brokerd::proto::BrokerRequest,
    ) -> anyhow::Result<brokerd::proto::BrokerResponse> {
        use brokerd::server::dispatch_request;
        use tokio::io::AsyncReadExt;

        dispatch_request(
            request,
            &mut self.server_end,
            &self.conn,
            TEST_KEY,
            self.session_id,
            &mut self.last_event_id,
            &mut self.last_event_hash,
            &mut self.store,
            &self.ws_root,
            &self.session_status,
            &runtime_core::SessionPolicy::allow_all(),
            self.trusted_inode,
            &mut self.intent_provided,
            &mut self.fd_requested,
            &mut self.fd_request_count,
        )
        .await?;

        let mut len_buf = [0u8; 4];
        self.client_end.read_exact(&mut len_buf).await?;
        let msg_len = u32::from_le_bytes(len_buf) as usize;
        let mut body = vec![0u8; msg_len];
        self.client_end.read_exact(&mut body).await?;
        Ok(serde_json::from_slice(&body)?)
    }
}

/// Test A (resolve-then-mint, provenance threading): a `ReportClaims` batch
/// of two `DocFragment`s, followed by a `ReportDerivedClaim` over the two
/// returned `value_ids`, yields a derived `value_id` whose resolved record's
/// `provenance_chain` contains BOTH inputs' own file_read-rooted chains --
/// proving the derivation genuinely threads provenance on the LIVE dispatch
/// path (not a fresh transform-local root).
#[tokio::test]
async fn report_derived_claim_dispatch_threads_provenance_from_resolved_inputs() {
    use brokerd::proto::{BrokerRequest, BrokerResponse, TransformKind, WorkerClaim};

    let mut h = DispatchHarness::new();

    let resp = h
        .dispatch(BrokerRequest::ReportClaims {
            claims: vec![
                WorkerClaim::DocFragment("accounts".to_string()),
                WorkerClaim::DocFragment("ev1l.com".to_string()),
            ],
        })
        .await
        .expect("dispatch ReportClaims must succeed");
    let value_ids = match resp {
        BrokerResponse::ClaimsReceived { value_ids } => value_ids,
        other => panic!("expected ClaimsReceived, got {:?}", other),
    };
    assert_eq!(value_ids.len(), 2);

    let record0 = h
        .store
        .resolve(&value_ids[0])
        .expect("fragment 0 must resolve")
        .clone();
    let record1 = h
        .store
        .resolve(&value_ids[1])
        .expect("fragment 1 must resolve")
        .clone();

    let resp = h
        .dispatch(BrokerRequest::ReportDerivedClaim {
            transformed_literal: "accounts@ev1l.com".to_string(),
            transform: TransformKind::Concat,
            input_value_ids: value_ids.clone(),
        })
        .await
        .expect("dispatch ReportDerivedClaim must succeed");
    let derived_id = match resp {
        BrokerResponse::DerivedClaimReceived { value_id } => value_id,
        other => panic!("expected DerivedClaimReceived, got {:?}", other),
    };

    let derived = h
        .store
        .resolve(&derived_id)
        .expect("derived value_id must resolve in the per-connection store");
    assert_eq!(derived.literal, "accounts@ev1l.com");
    for id in &record0.provenance_chain {
        assert!(
            derived.provenance_chain.contains(id),
            "derived provenance_chain must thread input 0's own read-rooted chain"
        );
    }
    for id in &record1.provenance_chain {
        assert!(
            derived.provenance_chain.contains(id),
            "derived provenance_chain must thread input 1's own read-rooted chain"
        );
    }
}

/// Test B (finding #1c, live-wire): a `WorkerClaim::DocFragment` carrying an
/// already-assembled recipient (containing `'@'`) is REJECTED by the LIVE
/// `ReportClaims` dispatch arm -- `Error` response, mints nothing, no
/// `ClaimsReceived` is ever sent. Proves the laundering rejection fires on
/// the real dispatch path, not merely inside a hand-built `mint_from_read`
/// unit test.
#[tokio::test]
async fn report_claims_dispatch_rejects_assembled_recipient_as_doc_fragment() {
    use brokerd::proto::{BrokerRequest, BrokerResponse, WorkerClaim};

    let mut h = DispatchHarness::new();
    let initial_event_id = h.last_event_id;
    let initial_event_hash = h.last_event_hash.clone();

    let resp = h
        .dispatch(BrokerRequest::ReportClaims {
            claims: vec![WorkerClaim::DocFragment("accounts@ev1l.com".to_string())],
        })
        .await
        .expect("dispatch must complete (response is Error, not a transport failure)");

    match resp {
        BrokerResponse::Error { .. } => {}
        other => panic!(
            "expected Error response rejecting the assembled recipient, got {:?}",
            other
        ),
    }
    // No chain-head advance on the fail-closed path.
    assert_eq!(h.last_event_id, initial_event_id);
    assert_eq!(h.last_event_hash, initial_event_hash);
}

/// Test C (finding #3, live-wire, input ORDER): a `ReportDerivedClaim` whose
/// `input_value_ids` places a `UserTrusted` intent-rooted handle FIRST and a
/// `doc_fragment` file_read-rooted handle second (an untrusted union whose
/// `provenance_chain[0]` does not resolve to a `file_read` event) is REJECTED
/// on the LIVE dispatch path -- `Error` response, mints nothing, no
/// chain-head advance.
#[tokio::test]
async fn report_derived_claim_dispatch_rejects_non_file_read_root_at_index_0() {
    use brokerd::proto::{BrokerRequest, BrokerResponse, TransformKind, WorkerClaim};
    use runtime_core::intent::CaprunIntent;

    let mut h = DispatchHarness::new();

    let resp = h
        .dispatch(BrokerRequest::ProvideIntent {
            intent: CaprunIntent::SendEmailSummary {
                recipient: "boss@company.com".to_string(),
                subject: "Q3 summary".to_string(),
                body: "See attached.".to_string(),
            },
            primary_file_derived: false,
        })
        .await
        .expect("dispatch ProvideIntent must succeed");
    let intent_value_id = match resp {
        BrokerResponse::IntentAccepted { value_id, .. } => value_id,
        other => panic!("expected IntentAccepted, got {:?}", other),
    };

    let resp = h
        .dispatch(BrokerRequest::ReportClaims {
            claims: vec![WorkerClaim::DocFragment("ev1l.com".to_string())],
        })
        .await
        .expect("dispatch ReportClaims must succeed");
    let fragment_value_id = match resp {
        BrokerResponse::ClaimsReceived { value_ids } => value_ids[0].clone(),
        other => panic!("expected ClaimsReceived, got {:?}", other),
    };

    let before_event_id = h.last_event_id;
    let before_event_hash = h.last_event_hash.clone();

    // UserTrusted handle FIRST, file_read handle second -- the attacker
    // picking input ORDER to try to smuggle an intent_received root at [0].
    let resp = h
        .dispatch(BrokerRequest::ReportDerivedClaim {
            transformed_literal: "boss@company.comev1l.com".to_string(),
            transform: TransformKind::Concat,
            input_value_ids: vec![intent_value_id, fragment_value_id],
        })
        .await
        .expect("dispatch must complete (response is Error, not a transport failure)");

    match resp {
        BrokerResponse::Error { .. } => {}
        other => panic!(
            "expected Error response rejecting the non-file_read-rooted union, got {:?}",
            other
        ),
    }
    assert_eq!(h.last_event_id, before_event_id, "no chain-head advance on rejection");
    assert_eq!(h.last_event_hash, before_event_hash, "no chain-head advance on rejection");
}

/// Test D (MAJOR-1, live-wire concat-mismatch): a `ReportDerivedClaim` over
/// two resolvable `doc_fragment` inputs whose literals join to
/// `"accounts@ev1l.com"`, but whose claimed `transformed_literal` is
/// `"attacker@evil.com"` (not the join), is REJECTED on the LIVE dispatch
/// path -- `Error` response, mints nothing, no chain-head advance. Proves the
/// byte-verify guard fires on the real wire, not merely in a unit test.
#[tokio::test]
async fn report_derived_claim_dispatch_rejects_concat_byte_mismatch() {
    use brokerd::proto::{BrokerRequest, BrokerResponse, TransformKind, WorkerClaim};

    let mut h = DispatchHarness::new();

    let resp = h
        .dispatch(BrokerRequest::ReportClaims {
            claims: vec![
                WorkerClaim::DocFragment("accounts".to_string()),
                WorkerClaim::DocFragment("ev1l.com".to_string()),
            ],
        })
        .await
        .expect("dispatch ReportClaims must succeed");
    let value_ids = match resp {
        BrokerResponse::ClaimsReceived { value_ids } => value_ids,
        other => panic!("expected ClaimsReceived, got {:?}", other),
    };

    let before_event_id = h.last_event_id;
    let before_event_hash = h.last_event_hash.clone();

    let resp = h
        .dispatch(BrokerRequest::ReportDerivedClaim {
            transformed_literal: "attacker@evil.com".to_string(),
            transform: TransformKind::Concat,
            input_value_ids: value_ids,
        })
        .await
        .expect("dispatch must complete (response is Error, not a transport failure)");

    match resp {
        BrokerResponse::Error { .. } => {}
        other => panic!(
            "expected Error response rejecting the concat byte-verify mismatch, got {:?}",
            other
        ),
    }
    assert_eq!(h.last_event_id, before_event_id, "no chain-head advance on rejection");
    assert_eq!(h.last_event_hash, before_event_hash, "no chain-head advance on rejection");
}

/// Test E: a `ReportDerivedClaim` naming an `input_value_id` that does not
/// resolve in this connection's `ValueStore` (dangling/forged/cross-connection
/// handle) is REJECTED -- `Error` response, mints nothing.
#[tokio::test]
async fn report_derived_claim_dispatch_rejects_unresolvable_input() {
    use brokerd::proto::{BrokerRequest, BrokerResponse, TransformKind};
    use runtime_core::plan_node::ValueId;

    let mut h = DispatchHarness::new();
    let before_event_id = h.last_event_id;

    let resp = h
        .dispatch(BrokerRequest::ReportDerivedClaim {
            transformed_literal: "whatever@example.com".to_string(),
            transform: TransformKind::Concat,
            input_value_ids: vec![ValueId::new()],
        })
        .await
        .expect("dispatch must complete (response is Error, not a transport failure)");

    match resp {
        BrokerResponse::Error { .. } => {}
        other => panic!(
            "expected Error response rejecting the unresolvable input handle, got {:?}",
            other
        ),
    }
    assert_eq!(h.last_event_id, before_event_id, "no chain-head advance on rejection");
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
