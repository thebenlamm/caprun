//! harden04_featureless_create_session — D-10 behavioral negative gate
//! (v1.6 HARDEN-04, `DESIGN-security-hardening.md` §d/§j).
//!
//! Proves that a FEATURELESS (default) build of `brokerd` denies
//! `CreateSession` over the real broker UDS even with the legacy
//! `CAPRUN_ENABLE_IPC_CREATE_SESSION=1` opt-in flag SET — because the mint
//! arm is physically absent from this build, not merely runtime-denied
//! (contrast with `crates/brokerd/tests/uds_ipc.rs`'s
//! `create_session_over_ipc_denied_by_default_when_flag_unset`, which proves
//! the runtime-gate default-deny with the flag UNSET on a test/test-fixtures
//! build — that arm is present there, just not opted into).
//!
//! ## Why this lives in `cli/caprun`, not `crates/brokerd`
//!
//! `crates/brokerd/Cargo.toml`'s own `[dev-dependencies]` self dev-dependency
//! (27-02 Task 1) enables `test-fixtures` for brokerd's OWN test targets
//! (unit tests + `tests/uds_ipc.rs`) — that is precisely how those tests
//! reach the cfg-gated mint arm. Any test living inside `crates/brokerd`
//! would inescapably link a test-fixtures-enabled `brokerd`, which cannot
//! prove featureless absence.
//!
//! `cli/caprun/Cargo.toml`'s dependency on `brokerd` (`[dependencies]`,
//! no `features = [...]`) requests brokerd's DEFAULT feature set only.
//! `test-fixtures` is not a default feature (27-02 Task 1's `[features]`
//! block declares it opt-in only), so building caprun's own targets pulls in
//! a plain, non-test-fixtures `brokerd` lib — UNLESS Cargo's feature
//! resolver unifies features workspace-wide because SOME other build unit in
//! the same invocation also needs brokerd's `test-fixtures` (this happens
//! under a bare `cargo test --workspace`, because that invocation also
//! builds `crates/brokerd`'s own test targets, which requests
//! `test-fixtures` via its self dev-dependency).
//!
//! EMPIRICALLY VERIFIED invocation that keeps brokerd genuinely featureless
//! in this test's build graph — scoped to the `caprun` package only, so
//! brokerd's own test targets (and its self dev-dep) are never part of the
//! build plan:
//!
//! ```text
//! cargo test -p caprun --test harden04_featureless_create_session
//! ```
//!
//! This test asserts its own precondition directly rather than trusting the
//! invocation alone: it inspects the actual `CreateSession` response before
//! deciding whether the D-10 negative assertion applies.
//!
//! EMPIRICALLY CONFIRMED (via a genuine Linux run under Colima/Docker,
//! `scripts/mailpit-verify.sh`, during this plan's own execution): a bare
//! `cargo test --workspace` DOES re-unify `test-fixtures` onto brokerd
//! graph-wide, because that single invocation also legitimately builds
//! `crates/brokerd`'s own test targets (which need the feature via its self
//! dev-dependency) alongside caprun's. Under `--workspace`, this test's
//! build of brokerd is therefore NOT featureless, and `CreateSession`
//! actually mints a session (`SessionCreated`) even with the flag set.
//!
//! The skip/hard-fail branch is keyed on `brokerd::TEST_FIXTURES_ACTIVE` —
//! a const reflecting whether THIS build graph actually compiled brokerd
//! with `test-fixtures` — NOT on the response variant. Keying on the
//! response itself would mean a genuine D-10 regression (the mint arm
//! leaking into a build that should be featureless) produces exactly the
//! `SessionCreated` response this test exists to catch, and that response
//! would then be misread as "this graph must not be featureless" and
//! silently skipped, giving D-10 zero ongoing protection. Because the
//! const reflects the actual compiled feature set rather than the
//! response, it cannot be fooled by the regression it is meant to detect:
//! when `TEST_FIXTURES_ACTIVE` is true (ambient unification, e.g. under
//! `--workspace`), the skip is legitimate and non-failing so `cargo test
//! --workspace --no-fail-fast` stays green; when it is false (the genuinely
//! featureless SCOPED `-p caprun` invocation), the hard assertion below
//! ALWAYS runs and a genuine regression ALWAYS fails loudly — it can never
//! be routed into the skip branch.
//!
//! Abstract-namespace UDS (what `run_broker_server` binds) is a Linux kernel
//! extension — unavailable on macOS — so, mirroring `uds_ipc.rs`'s own
//! convention, the live body is `#[cfg(target_os = "linux")]`. Run via the
//! project's standing Mailpit-aware Linux recipe (CLAUDE.md):
//!
//! ```text
//! MAILPIT_VERIFY_CMD='cargo test -p caprun --test harden04_featureless_create_session' \
//!   bash scripts/mailpit-verify.sh
//! ```

#[cfg(target_os = "linux")]
mod linux_tests {
    use brokerd::audit::open_audit_db;
    use brokerd::proto::{BrokerRequest, BrokerResponse};
    use brokerd::server::run_broker_server;
    use rusqlite::Connection;
    use runtime_core::SessionStatus;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use uuid::Uuid;

    /// Serializes this test against any other test in the `caprun` package
    /// that also mutates `CAPRUN_ENABLE_IPC_CREATE_SESSION` — mirrors
    /// `uds_ipc.rs::CREATE_SESSION_ENV_LOCK`'s rationale. No other test file
    /// in `cli/caprun/tests/` currently touches this var, but the lock costs
    /// nothing and keeps the precedent consistent.
    static CREATE_SESSION_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Send a framed `BrokerRequest` and receive a `BrokerResponse`.
    /// Identical wire framing to `uds_ipc.rs::round_trip` (4-byte LE length
    /// prefix, then JSON body) — same protocol, different crate.
    async fn round_trip(
        stream: &mut tokio::net::UnixStream,
        req: &BrokerRequest,
    ) -> BrokerResponse {
        let body = serde_json::to_vec(req).expect("serialize request");
        let len = (body.len() as u32).to_le_bytes();
        stream.write_all(&len).await.expect("write length");
        stream.write_all(&body).await.expect("write body");

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.expect("read length");
        let msg_len = u32::from_le_bytes(len_buf) as usize;
        let mut resp_body = vec![0u8; msg_len];
        stream.read_exact(&mut resp_body).await.expect("read body");
        serde_json::from_slice(&resp_body).expect("deserialize response")
    }

    /// D-10: a featureless (default) broker build denies `CreateSession`
    /// over the real abstract-namespace UDS even with
    /// `CAPRUN_ENABLE_IPC_CREATE_SESSION=1` SET — proving the mint arm is
    /// physically absent, not merely runtime-denied.
    #[tokio::test]
    async fn featureless_create_session_denied_even_with_flag_set() {
        let _env_guard = CREATE_SESSION_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());

        // Deliberately the OPPOSITE of uds_ipc.rs's negative test: there the
        // flag is UNSET to prove the runtime gate. Here the flag is SET to
        // "1" — the exact value that, on a test/test-fixtures build, WOULD
        // mint a session (see uds_ipc.rs::server_accept). If this build were
        // NOT genuinely featureless, this test would incorrectly observe
        // `SessionCreated` here and the assertion below would fail loudly.
        std::env::set_var("CAPRUN_ENABLE_IPC_CREATE_SESSION", "1");

        let conn: Arc<Mutex<Connection>> =
            Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));

        let pid = std::process::id();
        let server_session_id = format!("harden04-featureless-{pid}");
        let sock_path = format!("\0/agentos/{server_session_id}");

        let conn_clone = conn.clone();
        let session_id_clone = server_session_id.clone();
        // CreateSession never exercises RequestFd; any valid dir anchors the root.
        let ws_root = std::sync::Arc::new(
            adapter_fs::workspace::WorkspaceRoot::open(std::env::temp_dir().as_path())
                .expect("open ws root"),
        );
        let server_handle = tokio::spawn(async move {
            let _ = run_broker_server(
                &session_id_clone,
                conn_clone,
                Uuid::new_v4(),
                Uuid::new_v4(),
                String::new(),
                SessionStatus::Active,
                ws_root,
                std::env::temp_dir().join("__harden04_featureless_no_trusted_path__"),
                std::sync::Arc::new([0u8; 32]), // HARDEN-02 broker MAC key (test)
            )
            .await;
        });

        tokio::task::yield_now().await;

        let mut stream = tokio::net::UnixStream::connect(&sock_path)
            .await
            .expect("connect to broker abstract socket");

        let intent_id = Uuid::new_v4();
        let resp = round_trip(&mut stream, &BrokerRequest::CreateSession { intent_id }).await;

        // Keyed on whether THIS build graph actually compiled brokerd with
        // `test-fixtures` (brokerd::TEST_FIXTURES_ACTIVE), NOT on the
        // response variant. Keying on the response itself (the prior
        // implementation) meant a genuine D-10 regression — the mint arm
        // leaking into a build that should be featureless — would produce
        // exactly the `SessionCreated` response this test exists to catch,
        // and that response would then be (incorrectly) treated as "this
        // graph must not be featureless" and silently skipped. The const
        // reflects the actual compiled feature set, so it cannot be fooled
        // by the very regression it is meant to detect.
        if brokerd::TEST_FIXTURES_ACTIVE {
            // EMPIRICALLY CONFIRMED during this plan's own verification run:
            // a bare `cargo test --workspace` DOES re-unify `test-fixtures`
            // onto this build graph, because that single invocation also
            // builds brokerd's OWN test targets (which need the feature via
            // its self dev-dependency), and Cargo's feature resolver unifies
            // the winning feature set for a package across the whole
            // invocation. Under THAT invocation this graph legitimately has
            // the mint arm compiled in, so `SessionCreated` is expected here
            // — a featureless-only assertion is not meaningful on this
            // build. This is a real, EXPECTED consequence of ambient
            // unification, not a D-10 regression; converting it into an
            // explicit, loud, non-failing skip keeps `cargo test --workspace
            // --no-fail-fast` green for a reason unrelated to D-10. The
            // scoped invocation is what actually proves D-10 — run it
            // directly (or via the project's Linux Mailpit recipe) for the
            // real signal:
            //
            //   cargo test -p caprun --test harden04_featureless_create_session
            eprintln!(
                "harden04_featureless_create_session: SKIPPING the D-10 \
                 negative assertion -- brokerd::TEST_FIXTURES_ACTIVE is true, \
                 meaning THIS build graph genuinely compiled brokerd with \
                 test-fixtures (most likely because this ran under a \
                 workspace-wide invocation such as `cargo test --workspace` \
                 rather than the scoped `cargo test -p caprun --test \
                 harden04_featureless_create_session`). This is expected \
                 under ambient Cargo feature unification and is NOT a D-10 \
                 regression. Re-run scoped to get the real D-10 proof."
            );
            server_handle.abort();
            return;
        }

        // The teeth of D-10: this graph is genuinely featureless
        // (brokerd::TEST_FIXTURES_ACTIVE == false), so the response MUST be
        // the fail-closed Error, DESPITE the flag being set to exactly the
        // value that opts in on a test-fixtures build. If the mint arm ever
        // leaks into a featureless build (the `#[cfg]` gate breaks), `resp`
        // here would be `SessionCreated` and this assertion fails loudly —
        // unlike the prior response-keyed implementation, this branch is
        // ALWAYS reached and ALWAYS evaluated when the build is genuinely
        // featureless, so a genuine regression can never be routed into the
        // skip branch above.
        assert!(
            matches!(resp, BrokerResponse::Error { .. }),
            "D-10 VIOLATION: expected a featureless (default) broker build to \
             deny CreateSession even with CAPRUN_ENABLE_IPC_CREATE_SESSION=1 \
             set, but got {:?}.",
            resp
        );

        // Zero session rows minted — the fail-closed Error path never
        // touches the audit DB.
        let session_count: i64 = conn
            .lock()
            .expect("mutex poisoned")
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .expect("query sessions");
        assert_eq!(
            session_count, 0,
            "D-10 VIOLATION: a session row was minted despite the featureless \
             deny path — no session should ever be created here"
        );

        server_handle.abort();
    }
}
