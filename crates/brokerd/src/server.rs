/// server — tokio async UDS IPC server (the unified, session-scoped reference monitor)
///
/// ─────────────────────────────────────────────────────────────────────────────
/// VERIFIED abstract-namespace UDS pattern — confirmed by reading tokio-1.52.3
/// source (src/net/unix/listener.rs + stream.rs) 2026-06-29, and by
/// crates/brokerd/tests/uds_abstract_spike.rs (bind → accept → round-trip)
/// ─────────────────────────────────────────────────────────────────────────────
///
/// APPROACH A — PRIMARY (verified simpler in tokio 1.52.3):
///   tokio 1.52.3 handles abstract paths natively in UnixListener::bind and
///   UnixStream::connect. Simply pass the path with a leading NUL byte:
///
///   let listener = tokio::net::UnixListener::bind("\0/agentos/<session_id>")?;
///
/// KEY PROPERTY: Abstract sockets bypass Landlock filesystem restrictions.
/// After Landlock deny-all-filesystem is applied in the worker, it can still
/// connect to the broker's abstract socket.
///
/// IPC framing: 4-byte LE length prefix + JSON body (serde_json).
/// Max message size: 64 KiB (ASVS V5 input validation / T-03-08 DoS mitigate).
///
/// ─────────────────────────────────────────────────────────────────────────────
/// UNIFIED DISPATCH (Phase 5, Plan 02)
/// ─────────────────────────────────────────────────────────────────────────────
/// This is THE single broker dispatch path — there is no second loop in the CLI.
/// It is the live, session-scoped, fail-closed reference monitor:
///   * RequestFd    — broker opens the workspace file (ambient fs) and passes the
///                    fd via SCM_RIGHTS; appends a causal `fd_granted` event.
///   * ReportClaims — mints a genuinely-tainted ValueRecord per typed claim via
///                    `quarantine::mint_from_read` (the SOLE taint-mint site), so
///                    `provenance_chain[0]` anchors to the real file_read event.
///   * SubmitPlanNode — evaluates I2 via `executor::submit_plan_node` against the
///                    CONNECTION-established session_id (never a message-supplied
///                    one), durably appends `sink_blocked`/`plan_node_evaluated`
///                    BEFORE returning the decision (fail-closed, ACC-02).
///
/// Per-connection isolation (HARD-03): each accepted connection owns a fresh
/// `ValueStore::default()`, so a ValueId minted in one connection resolves to
/// None (→ Denied) in another. There is no shared cross-session store.

use crate::audit::append_event;
use crate::proto::{BrokerRequest, BrokerResponse, WorkerClaim};
use crate::quarantine::{mint_from_derivation, mint_from_intent, mint_from_read, Claim};
use runtime_core::intent::CaprunIntent;
use crate::session::{create_session, persist_session};
use adapter_fs::pass_fd;
use anyhow::Context;
use chrono::Utc;
use executor::value_store::ValueStore;
use runtime_core::{Event, SeedProvenance, SessionStatus};
use std::os::unix::io::AsRawFd;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

/// Maximum IPC message size (64 KiB).
///
/// Any message claiming a length beyond this limit is rejected before allocation
/// (T-03-08 DoS mitigate: guard before vec allocation, not after).
const MAX_MSG_SIZE: usize = 64 * 1024;

/// Start the broker IPC server on an abstract-namespace UDS socket.
///
/// Binds `\0/agentos/{session_id}` using tokio's native abstract-path support
/// (Approach A — verified in uds_abstract_spike.rs). Accepts connections in a
/// loop and spawns a per-connection task. Each connection owns its own
/// `ValueStore` and threads the causal chain forward from the supplied initial
/// session_created event state.
///
/// # Arguments
/// * `session_id`            — string form, used as the socket name suffix.
/// * `conn`                  — shared, mutex-protected SQLite connection.
/// * `session_id_uuid`       — broker-authoritative session identity (HARD-03).
/// * `initial_last_event_id` — id of the `session_created` event minted upstream.
/// * `initial_last_event_hash` — hash of that event row (chain anchor).
/// * `initial_session_status` — the session's status at creation time (from
///   `create_session`'s result), seeded into every connection's per-connection
///   `session_status` local (threaded exactly like `initial_last_event_id`/
///   `initial_last_event_hash`, RESEARCH Pitfall 2).
///
/// The `value_store` is NO LONGER a parameter — it is created fresh per
/// connection inside `handle_connection` to enforce session-scoped resolution.
///
/// Returns when the accept loop encounters an unrecoverable error.
pub async fn run_broker_server(
    session_id: &str,
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id_uuid: Uuid,
    initial_last_event_id: Uuid,
    initial_last_event_hash: String,
    initial_session_status: SessionStatus,
    workspace_root: Arc<adapter_fs::workspace::WorkspaceRoot>,
) -> anyhow::Result<()> {
    // Approach A: tokio detects the leading NUL and calls from_abstract_name internally.
    let sock_path = format!("\0/agentos/{session_id}");
    let listener = tokio::net::UnixListener::bind(&sock_path)?;

    loop {
        let (stream, _addr) = listener.accept().await?;
        let conn_clone = conn.clone();
        // Workspace-root capability — cloned per connection exactly like `conn`.
        let workspace_root_clone = workspace_root.clone();
        // Pass connection state by value — each connection owns its own chain state.
        let initial_hash = initial_last_event_hash.clone();
        let initial_status = initial_session_status.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(
                stream,
                conn_clone,
                session_id_uuid,
                initial_last_event_id,
                initial_hash,
                initial_status,
                workspace_root_clone,
            )
            .await
            {
                eprintln!("[brokerd] connection error: {e}");
            }
        });
    }
}

/// Handle one IPC connection: loop reading framed requests and dispatching them.
///
/// Framing: 4-byte LE length prefix + JSON body.
/// Guard: length > 64 KiB → reject with Error response, never allocate.
///
/// Owns a per-connection `ValueStore` (HARD-03: session-scoped resolution) and
/// threads `last_event_id` / `last_event_hash` / `session_status` across every
/// message so each appended event chains causally onto the previous one and
/// the session's trust state is resolved from broker-owned in-memory state,
/// never re-derived from IPC (RESEARCH Pitfall 2).
async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,
    mut last_event_id: Uuid,
    mut last_event_hash: String,
    initial_session_status: SessionStatus,
    workspace_root: Arc<adapter_fs::workspace::WorkspaceRoot>,
) -> anyhow::Result<()> {
    // Per-connection ValueStore — scoped to this session ONLY (HARD-03 fix).
    let mut value_store = ValueStore::default();
    // Per-connection session_status local — seeded from create_session's
    // result, updated in place after mint_from_read demotes (never re-queried
    // from the DB per call, DESIGN §4, RESEARCH Pitfall 2).
    let mut session_status = initial_session_status;
    // Phase 16 (BLOCKER-1 guard a): per-connection, broker-enforced ordering
    // state — ProvideIntent is accepted EXACTLY ONCE and ONLY BEFORE any
    // RequestFd on this connection. Initialized false per connection, exactly
    // like `session_status` above (never re-derived from IPC).
    let mut intent_provided = false;
    let mut fd_requested = false;

    loop {
        // Read 4-byte LE length prefix; clean EOF ends the connection loop.
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e.into()),
        }
        let msg_len = u32::from_le_bytes(len_buf) as usize;

        // T-03-08: guard before allocation — reject oversized messages.
        if msg_len > MAX_MSG_SIZE {
            send_response(
                &mut stream,
                &BrokerResponse::Error {
                    message: "message too large".into(),
                },
            )
            .await?;
            break;
        }

        let mut body = vec![0u8; msg_len];
        stream.read_exact(&mut body).await?;

        // Deserialize — serde errors → generic Error response, never panic (T-03-08/09).
        let request = match serde_json::from_slice::<BrokerRequest>(&body) {
            Ok(req) => req,
            Err(e) => {
                eprintln!("[brokerd] deserialize error: {e}");
                send_response(
                    &mut stream,
                    &BrokerResponse::Error {
                        message: "invalid request".into(),
                    },
                )
                .await?;
                break;
            }
        };

        dispatch_request(
            request,
            &mut stream,
            &conn,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut value_store,
            &workspace_root,
            &mut session_status,
            &mut intent_provided,
            &mut fd_requested,
        )
        .await?;
    }
    Ok(())
}

/// Dispatch a single `BrokerRequest`, writing its response to `stream`.
///
/// Extracted from the accept loop (RESEARCH Pitfall 6) so individual arms are
/// unit-testable against a `UnixStream::pair()` without binding a real socket.
///
/// `last_event_id` / `last_event_hash` are advanced (after a successful append)
/// by every arm that records an event, preserving the causal chain.
/// `session_status` is the broker-owned, per-connection trust-state local —
/// advanced to `Draft` after `ReportClaims` demotes, and passed by reference
/// (never re-derived from IPC/PlanNode) into `executor::submit_plan_node`
/// (DESIGN-session-trust-state.md §4/§11 condition 0).
///
/// `intent_provided` / `fd_requested` (Phase 16, BLOCKER-1 guard a) are the
/// broker-enforced, per-connection ordering state for `ProvideIntent`: it is
/// accepted EXACTLY ONCE and ONLY BEFORE any `RequestFd` on this connection.
/// Both start `false`, are threaded by `&mut` exactly like `session_status`,
/// and are set at the RequestFd/ProvideIntent arms below — never reset,
/// never re-derived from IPC.
#[allow(clippy::too_many_arguments)]
pub async fn dispatch_request(
    request: BrokerRequest,
    stream: &mut tokio::net::UnixStream,
    conn: &Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,
    last_event_id: &mut Uuid,
    last_event_hash: &mut String,
    value_store: &mut ValueStore,
    workspace_root: &Arc<adapter_fs::workspace::WorkspaceRoot>,
    session_status: &mut SessionStatus,
    intent_provided: &mut bool,
    fd_requested: &mut bool,
) -> anyhow::Result<()> {
    match request {
        BrokerRequest::CreateSession { intent_id } => {
            // Phase 16 (BLOCKER-1 guard c): this in-broker IPC arm forces a
            // fresh session Active via SeedProvenance::TrustedArg — a forced-
            // Active mint reachable over IPC with NO once-only/ordering guard
            // would, combined with the reach-to-a-send path BLOCKER-1
            // documents, let ANY IPC caller obtain a trusted-seeded session.
            // It is test-only (the live cli/caprun path calls create_session
            // directly, never through this arm) and is now gated behind a
            // fail-closed RUNTIME opt-in env flag — NOT `cfg(test)`, which is
            // unset when brokerd compiles as a dependency of an integration-
            // test binary (the Round-3 mechanism this replaces was Linux-
            // invisible and would panic uds_ipc.rs's own tests under
            // mailpit-verify.sh). Exactly the string "1" enables the arm
            // (never `.is_ok()` — F3 hardening: an EMPTY inherited
            // CAPRUN_ENABLE_IPC_CREATE_SESSION="" must NOT enable it). Default
            // (unset/empty/anything else) = Error, mint nothing. The confined
            // worker cannot set the broker PROCESS's own environment
            // (separate process), so no production IPC caller can obtain a
            // forced-Active session this way.
            if !matches!(
                std::env::var("CAPRUN_ENABLE_IPC_CREATE_SESSION").as_deref(),
                Ok("1")
            ) {
                let _ = intent_id;
                send_response(
                    stream,
                    &BrokerResponse::Error {
                        message: "CreateSession over IPC is disabled; set \
                                  CAPRUN_ENABLE_IPC_CREATE_SESSION=1 to enable \
                                  (test-only path)"
                            .into(),
                    },
                )
                .await?;
                return Ok(());
            }

            // Session-independent: mints its own fresh session (parent_id None);
            // does NOT thread the connection chain.
            let seed_provenance = SeedProvenance::TrustedArg;
            let session = create_session(intent_id, seed_provenance.clone());
            let new_session_id = session.id;

            // ORIGIN-01: record the seed-provenance determination in the
            // session_created Event. `Event` carries no free-form metadata
            // field (its serialized form IS the audit `payload` column), so
            // the provenance tag rides in `actor` — still part of the hashed
            // payload — as an explicit, exhaustively-matched tag (never a
            // silent default).
            let actor = match seed_provenance {
                SeedProvenance::TrustedArg => "broker:seed_provenance=trusted_arg",
                SeedProvenance::FileDerived => "broker:seed_provenance=file_derived",
            };
            let event = Event::new(
                Uuid::new_v4(),
                None,
                new_session_id,
                actor.into(),
                "session_created".into(),
                Utc::now(),
                vec![],
            );

            let result: anyhow::Result<()> = match conn.lock() {
                Ok(locked) => persist_session(&locked, &session)
                    .and_then(|_| append_event(&locked, &event, None).map(|_| ())),
                Err(e) => Err(anyhow::anyhow!("mutex poisoned: {e}")),
            };

            match result {
                Ok(_) => {
                    send_response(
                        stream,
                        &BrokerResponse::SessionCreated {
                            session_id: new_session_id,
                        },
                    )
                    .await?;
                }
                Err(e) => {
                    eprintln!("[brokerd] CreateSession error: {e}");
                    send_response(
                        stream,
                        &BrokerResponse::Error {
                            message: "internal error".into(),
                        },
                    )
                    .await?;
                }
            }
        }

        BrokerRequest::RequestFd { path } => {
            // Phase 16 (BLOCKER-1 guard a): mark fd_requested at entry, BEFORE
            // any other work — this is what makes a subsequent ProvideIntent
            // rejected fail-closed, regardless of whether this RequestFd itself
            // ultimately succeeds or errors below.
            *fd_requested = true;

            // HARD-04: resolve the worker-supplied path BENEATH the workspace
            // dirfd anchor via a single openat2(RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS)
            // syscall. Absolute paths, `..` traversal, and symlink escapes are
            // rejected at kernel resolution time (fail-closed) — the broker no
            // longer opens a worker-controlled path via ambient std::fs::File::open.
            let file = workspace_root
                .read_within(&path)
                .with_context(|| format!("broker: read_within {path}"))?;
            let file_fd = file.as_raw_fd();

            // Append fd_granted Event — causal parent is the prior event (not None).
            let fd_event_id = Uuid::new_v4();
            let fd_event = Event::new(
                fd_event_id,
                Some(*last_event_id),
                session_id,
                "broker".into(),
                "fd_granted".into(),
                Utc::now(),
                vec![],
            );
            let fd_hash = {
                let locked = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                append_event(&locked, &fd_event, Some(last_event_hash))
                    .context("append fd_granted")?
            };

            // pass_fd is blocking (sendmsg) — MUST use spawn_blocking (RESEARCH Pitfall 2).
            let sock_fd = stream.as_raw_fd();
            tokio::task::spawn_blocking(move || {
                let result = pass_fd(sock_fd, file_fd).map_err(|e| anyhow::anyhow!("pass_fd: {e}"));
                drop(file); // close broker's copy after sendmsg completes
                result
            })
            .await
            .context("spawn_blocking pass_fd")??;

            send_response(stream, &BrokerResponse::FdGranted).await?;

            // Advance the chain ONLY after the append + fd-pass succeeded.
            *last_event_id = fd_event_id;
            *last_event_hash = fd_hash;
        }

        BrokerRequest::ReportClaims { claims } => {
            // Mint one genuinely-tainted ValueRecord per typed claim.
            // mint_from_read is the SOLE taint-mint site — NEVER call ValueStore::mint
            // directly on the live path (taint stapling fails §9, T-04-03/T-05-05).
            let mut value_ids = Vec::new();
            for claim in claims {
                // Map the wire variant to a broker-side quarantine Claim. The
                // broker independently assigns the claim_type; the worker cannot
                // launder trust — mint_from_read taints purely by claim_type
                // (email → EmailRaw, relative_path → PathRaw). Exhaustive match:
                // any future WorkerClaim variant is a compile-forced arm here (and
                // an unknown wire `kind` already fails closed at deserialize).
                let quarantine_claim = match claim {
                    WorkerClaim::EmailAddress(addr) => Claim {
                        claim_type: "email_address".into(),
                        value: addr,
                    },
                    WorkerClaim::RelativePath(path) => Claim {
                        claim_type: "relative_path".into(),
                        value: path,
                    },
                    // Phase 15 (15-03): a raw doc fragment (one half of a
                    // Reply-To:/Domain: pair) — mint_from_read enforces
                    // looks_like_doc_fragment below, so an assembled ('@'-
                    // containing) recipient is rejected, not silently
                    // accepted as a fresh single-element chain (finding #1a).
                    WorkerClaim::DocFragment(fragment) => Claim {
                        claim_type: "doc_fragment".into(),
                        value: fragment,
                    },
                };
                let mint_result = {
                    let locked = conn
                        .lock()
                        .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                    mint_from_read(
                        &locked,
                        value_store,
                        session_id,
                        &quarantine_claim,
                        // Causal parent = the connection chain head (DESIGN §0), so
                        // file_read is parent-linked (fd_granted → file_read) and the
                        // audit DAG is ONE unbroken parent_id chain (ACC-05 / verify_chain).
                        Some(*last_event_id),
                        Some(last_event_hash),
                    )
                };
                // finding #1c: mint_from_read now enforces looks_like_doc_fragment
                // for the doc_fragment claim_type, so it CAN return Err on a live,
                // attacker-controlled message (an assembled recipient sent as a
                // fresh DocFragment). Surface that Err as a Denied/error response
                // on the wire — NEVER unwrap/propagate it as a connection-killing
                // internal error. Mints nothing for this claim; no ClaimsReceived
                // is sent for this batch (fail-closed, no partial success report).
                let (_read_event_id, _read_hash, value_id, demoted_event_id, demoted_hash) =
                    match mint_result {
                        Ok(tuple) => tuple,
                        Err(e) => {
                            eprintln!("[brokerd] ReportClaims mint_from_read error: {e}");
                            send_response(
                                stream,
                                &BrokerResponse::Error {
                                    message: "ReportClaims rejected (fail-closed)".into(),
                                },
                            )
                            .await?;
                            return Ok(());
                        }
                    };
                // Advance the chain head to `session_demoted` — the LAST event
                // mint_from_read appended — NOT to the file_read event. Using
                // file_read's own id/hash here would make the NEXT appended
                // event a SIBLING of session_demoted (both children of
                // file_read), forking the causal DAG and breaking
                // audit::verify_chain's single-linear-chain walk.
                *last_event_id = demoted_event_id;
                *last_event_hash = demoted_hash;
                // mint_from_read already demoted the session's DB row to Draft
                // (TAINT-01, atomically with the file_read append above). Update
                // the in-memory per-connection local to match — unconditionally,
                // since Draft is one-way/idempotent (mirrors how *last_event_id/
                // *last_event_hash are updated after each append).
                *session_status = SessionStatus::Draft;
                value_ids.push(value_id);
            }
            send_response(stream, &BrokerResponse::ClaimsReceived { value_ids }).await?;
        }

        BrokerRequest::SubmitPlanNode { plan_node } => {
            // The broker mints the effect identity (HARD-06) and passes it into the
            // executor — the executor never mints a Uuid (DESIGN §4 rule 2).
            let effect_id = Uuid::new_v4();
            // session_id comes from the connection — NEVER from the IPC message (HARD-03).
            // session_status likewise — broker-owned, per-connection state, NEVER
            // read from plan_node/IPC (DESIGN §4/§11 condition 0).
            let decision = executor::submit_plan_node(
                session_id,
                effect_id,
                &plan_node,
                value_store,
                &*session_status,
            );

            // Durably record the decision BEFORE returning any response (ACC-02).
            // Fail-closed: an append error propagates with `?`, so the block is
            // NEVER reported to the worker as having succeeded.
            //
            // On a block, resolve the FULL arg set from the still-live per-
            // connection ValueStore into a Vec<ResolvedArg> snapshot FIRST —
            // this is the ONLY moment those literals are recoverable (the
            // ValueStore does not survive process exit;
            // DESIGN-confirmation-release.md "The Problem Being Solved"). A
            // handle that fails to resolve here is a broker-internal invariant
            // violation (validate_schema already guaranteed presence) — fail
            // closed, never persist a partial snapshot.
            //
            // The CONFIRM-03 combined digest (Phase 16, DESIGN-confirm-binding.md
            // Round-6 amendment) MUST be computed over this SAME full set —
            // blocked AND trusted args together, never a subset — so it is
            // computed HERE, once, and threaded into BOTH the sink_blocked
            // Event and the PendingConfirmation below (no second computation,
            // no new lock).
            let block_snapshot: Option<(
                Vec<crate::confirmation::ResolvedArg>,
                String,
                Vec<String>,
            )> = if let runtime_core::ExecutorDecision::BlockedPendingConfirmation { anchors } =
                &decision
            {
                let mut resolved_args = Vec::with_capacity(plan_node.args.len());
                for arg in &plan_node.args {
                    let record = value_store.resolve(&arg.value_id).ok_or_else(|| {
                        anyhow::anyhow!(
                            "block-time snapshot: arg `{}` handle did not resolve",
                            arg.name
                        )
                    })?;
                    resolved_args.push(crate::confirmation::ResolvedArg {
                        name: arg.name.clone(),
                        value_id: arg.value_id.clone(),
                        literal: record.literal.clone(),
                        taint: record.taint.clone(),
                        provenance_chain: record.provenance_chain.clone(),
                    });
                }
                // CONFIRM-03 (Round-6): the digest covers EVERY current
                // resolved_args element (blocked AND trusted together) — the
                // primitive sorts by byte-wise ascending arg_name and asserts
                // name-uniqueness internally, so no manual sort/filter is
                // needed here.
                let pairs: Vec<(&str, &str)> = resolved_args
                    .iter()
                    .map(|a| (a.name.as_str(), a.literal.as_str()))
                    .collect();
                let digest = crate::confirmation::combined_digest(&pairs);
                // blocked_arg_names is DISPLAY-MARKING metadata ONLY (which of
                // the full set Plan 16-02 narrates as BLOCKED vs TRUSTED) — the
                // ordered `anchors` collection IS the ordered blocked subset
                // from the executor's collect-then-Block loop; never re-filter
                // resolved_args for this, and it does NOT gate the digest's
                // domain.
                let blocked_arg_names: Vec<String> =
                    anchors.iter().map(|b| b.anchor.arg.clone()).collect();
                Some((resolved_args, digest, blocked_arg_names))
            } else {
                None
            };

            // A block persists ALL durable anchors via the broker-owned
            // Event::sink_blocked constructor (sets Event.taint by merging every
            // anchor's taint, anchors = the full collection). The causal parent
            // stays the chain head — NOT read_event_id (two graphs are never
            // equated, DESIGN §0/§4 rule 3). On a block, capture EVERY blocked
            // arg's LIVE literal (name + literal) so each can be written to the
            // redactable `blocked_literals` side table (keyed by the sink_blocked
            // event id AND arg name — Phase 14 plural). The literal is NEVER put
            // in the hashed event payload — the anchor carries only its digest
            // (`literal_sha256`). `combined_digest`/`blocked_arg_names` (Phase 16)
            // ride inside this SAME hashed Event payload, computed once above.
            let (audit_event, blocked_literals) = match &decision {
                runtime_core::ExecutorDecision::BlockedPendingConfirmation { anchors } => {
                    let (_, digest, blocked_arg_names) = block_snapshot.as_ref().expect(
                        "block_snapshot is Some for every BlockedPendingConfirmation decision",
                    );
                    (
                        Event::sink_blocked(
                            Uuid::new_v4(),
                            Some(*last_event_id), // causal head — never read_event_id
                            session_id,
                            Utc::now(),
                            anchors.iter().map(|b| b.anchor.clone()).collect(),
                            Some(digest.clone()),
                            blocked_arg_names.clone(),
                        ),
                        anchors
                            .iter()
                            .map(|b| (b.anchor.arg.clone(), b.literal.clone()))
                            .collect::<Vec<(String, String)>>(),
                    )
                }
                _ => (
                    Event::new(
                        Uuid::new_v4(),
                        Some(*last_event_id), // causal parent preserved — not None
                        session_id,
                        "executor".into(),
                        "plan_node_evaluated".into(),
                        Utc::now(),
                        vec![],
                    ),
                    Vec::new(),
                ),
            };
            let event_type = audit_event.event_type.clone();

            let pending_confirmation = if let runtime_core::ExecutorDecision::BlockedPendingConfirmation {
                anchors,
            } = &decision
            {
                let (resolved_args, combined_digest, blocked_arg_names) = block_snapshot.expect(
                    "block_snapshot is Some for every BlockedPendingConfirmation decision",
                );
                Some(crate::confirmation::PendingConfirmation {
                    // Every element shares one effect_id — one Block, one
                    // effect_id, N blocked args (Phase 14 plural).
                    effect_id: anchors[0].anchor.effect_id,
                    session_id,
                    blocked_event_id: audit_event.id,
                    sink: plan_node.sink.clone(),
                    resolved_args,
                    blocked_arg_names,
                    combined_digest,
                    workspace_root_path: workspace_root.root_path().to_string_lossy().into_owned(),
                    state: crate::confirmation::PendingConfirmationState::Pending,
                })
            } else {
                None
            };
            let new_hash = {
                let locked = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                let hash =
                    append_event(&locked, &audit_event, Some(last_event_hash)).map_err(|e| {
                        eprintln!("[brokerd] {event_type} audit append FAILED (fail-closed): {e}");
                        anyhow::anyhow!("audit append failed: {e}")
                    })?;
                // Write EVERY blocked arg's raw literal to the redactable side
                // table under the SAME lock as the event append (fail-closed: a
                // failure here aborts the block before any response is sent).
                // Iterates the full blocked_literals collection — Phase 14 plural
                // — so no blocked value is dropped before the human confirm/deny
                // decision (T-14-06).
                for (arg, literal) in &blocked_literals {
                    crate::audit::insert_blocked_literal(
                        &locked,
                        &audit_event.id.to_string(),
                        arg,
                        literal,
                    )
                    .map_err(|e| {
                        eprintln!("[brokerd] blocked-literal side-table write FAILED (fail-closed): {e}");
                        anyhow::anyhow!("blocked-literal write failed: {e}")
                    })?;
                }
                // The pending_confirmations checkpoint commits under the SAME
                // lock as the sink_blocked event append + blocked-literal write
                // — they succeed or fail together (T-10-02 / DESIGN Persistence
                // contract): a sink_blocked event can never exist without its
                // checkpoint, and no orphaned checkpoint without an anchoring
                // block.
                if let Some(pc) = &pending_confirmation {
                    crate::confirmation::insert_pending_confirmation(&locked, pc).map_err(|e| {
                        eprintln!("[brokerd] pending_confirmations insert FAILED (fail-closed): {e}");
                        anyhow::anyhow!("pending_confirmations insert failed: {e}")
                    })?;
                }
                hash
            };
            *last_event_id = audit_event.id;
            *last_event_hash = new_hash;

            // 07-04b: on an Allowed `file.create` decision, invoke the live sink.
            // The authorizing `plan_node_evaluated` event is already durably
            // appended above, so the effect + its audit record follow it
            // (two-phase ordering: authorize, then effect). The sink event chains
            // onto the just-advanced (plan_node_evaluated) head. A sink error
            // propagates with `?` after a durable `sink_execution_failed` record —
            // no automatic retry (T-07-45). Non-file.create Allowed decisions keep
            // today's behavior (no sink invocation in v0).
            if matches!(decision, runtime_core::ExecutorDecision::Allowed)
                && plan_node.sink.0 == "file.create"
            {
                let (sink_event_id, sink_hash) = {
                    let locked = conn
                        .lock()
                        .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                    crate::sinks::file_create::invoke_file_create(
                        &locked,
                        value_store,
                        session_id,
                        effect_id,
                        &plan_node,
                        workspace_root,
                        *last_event_id,
                        last_event_hash,
                    )?
                };
                *last_event_id = sink_event_id;
                *last_event_hash = sink_hash;
            }

            // Only send the decision AFTER the durable append (and any sink
            // invocation) succeeded.
            send_response(stream, &BrokerResponse::PlanNodeDecision { decision }).await?;
        }

        BrokerRequest::ReportDerivedClaim {
            transformed_literal,
            transform,
            input_value_ids,
        } => {
            // Resolve every input handle against THIS connection's
            // broker-owned ValueStore, failing closed (Pitfall 1) if ANY is
            // missing (dangling/forged/cross-connection handle) — mint
            // NOTHING rather than mint from a partially-resolved set.
            //
            // Clone each resolved &ValueRecord into an OWNED ValueRecord
            // immediately: mint_from_derivation's `inputs: &[&ValueRecord]`
            // param and its `store: &mut ValueStore` param would otherwise
            // borrow-conflict if `inputs` referenced `value_store` directly
            // (mint_from_derivation's own doc comment: "the caller resolves
            // ValueIds to records ... before calling; the broker never
            // re-resolves them from store here").
            let mut resolved: Vec<runtime_core::value_record::ValueRecord> =
                Vec::with_capacity(input_value_ids.len());
            let mut unresolved = false;
            for input_id in &input_value_ids {
                match value_store.resolve(input_id) {
                    Some(record) => resolved.push(record.clone()),
                    None => {
                        unresolved = true;
                        break;
                    }
                }
            }
            if unresolved {
                send_response(
                    stream,
                    &BrokerResponse::Error {
                        message: "ReportDerivedClaim rejected: an input_value_id did not \
                                  resolve in this connection's ValueStore (fail-closed)"
                            .into(),
                    },
                )
                .await?;
                return Ok(());
            }
            let input_refs: Vec<&runtime_core::value_record::ValueRecord> =
                resolved.iter().collect();

            // mint_from_derivation is the SOLE new mint call this arm makes
            // (never ValueStore::mint directly, sole-taint-mint-site
            // discipline). It performs its own byte-verify + every-element
            // file_read-root guards internally; this arm does NOT parse,
            // extract, or re-apply the transform itself — the worker already
            // applied it (EXTRACT-01: extraction/transform stays worker-side).
            let mint_result = {
                let locked = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                mint_from_derivation(
                    &locked,
                    value_store,
                    session_id,
                    transformed_literal,
                    &input_refs,
                    transform.as_mint_tag(),
                    // Causal parent = the connection chain head (DESIGN §0),
                    // mirroring ReportClaims/SubmitPlanNode's threading.
                    Some(*last_event_id),
                    Some(last_event_hash),
                )
            };

            match mint_result {
                Ok((derivation_event_id, derivation_hash, value_id)) => {
                    // Advance the causal chain ONLY on the Ok path — mirrors
                    // the ReportClaims arm's chain-head-advance discipline
                    // exactly. session_status is NOT touched here (this arm
                    // is not an I1 trust-flip site — mint_from_read already
                    // demoted the session when the raw fragments were read).
                    *last_event_id = derivation_event_id;
                    *last_event_hash = derivation_hash;
                    send_response(stream, &BrokerResponse::DerivedClaimReceived { value_id })
                        .await?;
                }
                Err(e) => {
                    // Surface ALL of mint_from_derivation's fail-closed
                    // guards (zero inputs; ANY-element non-file_read-rooted
                    // untrusted union, finding #3 + MEDIUM R1/R2; concat
                    // byte-verify mismatch, MAJOR-1) as Denied — mint
                    // nothing, advance NO chain head (the broker never
                    // partially commits a derivation).
                    eprintln!("[brokerd] ReportDerivedClaim mint_from_derivation error: {e}");
                    send_response(
                        stream,
                        &BrokerResponse::Error {
                            message: "ReportDerivedClaim rejected (fail-closed)".into(),
                        },
                    )
                    .await?;
                }
            }
        }

        BrokerRequest::ProvideIntent { intent } => {
            // Phase 16 (BLOCKER-1 guard a): ProvideIntent is accepted EXACTLY
            // ONCE and ONLY BEFORE any RequestFd on this connection —
            // broker-enforced, not assumed from an honest worker's startup
            // ordering. A second ProvideIntent, or one arriving after ANY
            // RequestFd, is rejected fail-closed: mint NOTHING, no chain-head
            // advance. This closes the attack path where RequestFd releases
            // raw untrusted bytes (never demoting session_status) and the
            // worker then calls ProvideIntent to mint an ARBITRARY
            // attacker-chosen literal as fully-trusted UserTrusted.
            if *intent_provided || *fd_requested {
                send_response(
                    stream,
                    &BrokerResponse::Error {
                        message: "ProvideIntent rejected: must arrive exactly once, \
                                  before any RequestFd (fail-closed)"
                            .into(),
                    },
                )
                .await?;
                return Ok(());
            }

            // Extract the user-provided literal(s) from the typed intent.
            // Exhaustive match: adding a new CaprunIntent variant causes a compile error here,
            // forcing the dispatcher to be updated (no silent unhandled variants).
            //
            // Phase 15 (15-04, finding #6): `SendEmailSummary` carries THREE
            // trusted literals (recipient/subject/body); `CreateFileFromReport`
            // still carries only `path`. `subject_literal`/`body_literal` are
            // `None` for the latter (minimal additive shape — it mints only
            // ONE handle, unchanged from pre-15-04 behavior).
            let (primary_literal, subject_literal, body_literal): (String, Option<String>, Option<String>) =
                match &intent {
                    CaprunIntent::SendEmailSummary { recipient, subject, body } => {
                        (recipient.clone(), Some(subject.clone()), Some(body.clone()))
                    }
                    CaprunIntent::CreateFileFromReport { path } => (path.clone(), None, None),
                };

            // Mint inside the per-connection ValueStore (Pitfall 1: minting outside
            // handle_connection would put the ValueId in an unreachable store → Denied).
            // mint_from_intent is the ONLY caller site of mint_from_intent (T-06-05).
            //
            // THREE sequential mint_from_intent calls when subject/body are
            // present, threading last_event_id/last_event_hash across all of
            // them so the causal chain stays ONE linear chain (never a fork) —
            // each call's returned event becomes the next call's parent.
            let (value_id, subject_value_id, body_value_id) = {
                let locked = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;

                // Causal parent = the connection chain head (DESIGN §0): intent_received
                // is parent-linked onto session_created, keeping the parent_id chain unbroken.
                let (recipient_event_id, recipient_hash, recipient_value_id) = mint_from_intent(
                    &locked,
                    value_store,
                    session_id,
                    primary_literal,
                    Some(*last_event_id),
                    Some(last_event_hash),
                )?;
                *last_event_id = recipient_event_id;
                *last_event_hash = recipient_hash;

                let subject_value_id = match subject_literal {
                    Some(subject) => {
                        let (event_id, hash, vid) = mint_from_intent(
                            &locked,
                            value_store,
                            session_id,
                            subject,
                            Some(*last_event_id),
                            Some(last_event_hash),
                        )?;
                        *last_event_id = event_id;
                        *last_event_hash = hash;
                        Some(vid)
                    }
                    None => None,
                };

                let body_value_id = match body_literal {
                    Some(body) => {
                        let (event_id, hash, vid) = mint_from_intent(
                            &locked,
                            value_store,
                            session_id,
                            body,
                            Some(*last_event_id),
                            Some(last_event_hash),
                        )?;
                        *last_event_id = event_id;
                        *last_event_hash = hash;
                        Some(vid)
                    }
                    None => None,
                };

                (recipient_value_id, subject_value_id, body_value_id)
            };

            // Guard (a): mark accepted ONLY after the mint(s) succeeded — a
            // failed ProvideIntent (propagated via `?` above) never marks the
            // connection as having consumed its one-shot allowance.
            *intent_provided = true;

            send_response(
                stream,
                &BrokerResponse::IntentAccepted {
                    value_id,
                    subject_value_id,
                    body_value_id,
                },
            )
            .await?;
        }

        BrokerRequest::ReportRead { .. } => {
            // Deprecated: the live taint path is ReportClaims (typed extracts).
            // The variant remains in proto for wire compatibility but is no longer
            // a live broker path — direct callers to ReportClaims.
            send_response(
                stream,
                &BrokerResponse::Error {
                    message: "ReportRead is deprecated — use ReportClaims".into(),
                },
            )
            .await?;
        }
    }
    Ok(())
}

/// Write a framed BrokerResponse to the stream (4-byte LE length + JSON body).
async fn send_response(
    stream: &mut tokio::net::UnixStream,
    response: &BrokerResponse,
) -> anyhow::Result<()> {
    let body = serde_json::to_vec(response)?;
    let len = (body.len() as u32).to_le_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&body).await?;
    Ok(())
}

/// Phase 16 (BLOCKER-1 guard a) — ProvideIntent once-and-before-RequestFd unit
/// tests over `UnixStream::pair` (mirrors the existing dispatch_request arm
/// tests in `crates/brokerd/tests/proto_claims.rs`, but as `--lib` tests since
/// they drive `dispatch_request` in-crate and assert purely on the new
/// per-connection ordering-guard locals).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::open_audit_db;
    use runtime_core::intent::CaprunIntent;
    use std::os::unix::io::{AsRawFd, FromRawFd};

    /// Read one framed `BrokerResponse` from the client end of a
    /// `UnixStream::pair` (mirrors `send_response`'s wire framing).
    async fn read_framed(stream: &mut tokio::net::UnixStream) -> BrokerResponse {
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.expect("read length");
        let msg_len = u32::from_le_bytes(len_buf) as usize;
        let mut body = vec![0u8; msg_len];
        stream.read_exact(&mut body).await.expect("read body");
        serde_json::from_slice(&body).expect("deserialize response")
    }

    fn sample_intent() -> CaprunIntent {
        CaprunIntent::SendEmailSummary {
            recipient: "boss@company.com".to_string(),
            subject: "Q3 summary".to_string(),
            body: "See attached.".to_string(),
        }
    }

    /// A fresh in-memory audit DB + per-connection state, mirroring
    /// `proto_claims.rs::DispatchHarness`'s setup shape.
    fn harness() -> (
        Arc<Mutex<rusqlite::Connection>>,
        Uuid,
        ValueStore,
        Uuid,
        String,
        SessionStatus,
        Arc<adapter_fs::workspace::WorkspaceRoot>,
    ) {
        let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
        let session_id = Uuid::new_v4();
        let store = ValueStore::default();
        let last_event_id = Uuid::new_v4();
        let last_event_hash = "genesis-hash".to_string();
        let session_status = SessionStatus::Active;
        let ws_root = Arc::new(
            adapter_fs::workspace::WorkspaceRoot::open(std::env::temp_dir().as_path())
                .expect("open ws root"),
        );
        (
            conn,
            session_id,
            store,
            last_event_id,
            last_event_hash,
            session_status,
            ws_root,
        )
    }

    /// Guard (a), positive case: the first ProvideIntent before any RequestFd
    /// still succeeds — the happy path is unchanged.
    #[tokio::test]
    async fn provide_intent_before_any_request_fd_succeeds() {
        let (conn, session_id, mut store, mut last_event_id, mut last_event_hash, mut session_status, ws_root) =
            harness();
        let mut intent_provided = false;
        let mut fd_requested = false;
        let (mut server_end, mut client_end) =
            tokio::net::UnixStream::pair().expect("UnixStream::pair");

        dispatch_request(
            BrokerRequest::ProvideIntent { intent: sample_intent() },
            &mut server_end,
            &conn,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut store,
            &ws_root,
            &mut session_status,
            &mut intent_provided,
            &mut fd_requested,
        )
        .await
        .expect("ProvideIntent before any RequestFd must succeed");

        let resp = read_framed(&mut client_end).await;
        assert!(
            matches!(resp, BrokerResponse::IntentAccepted { .. }),
            "expected IntentAccepted, got {resp:?}"
        );
        assert!(intent_provided, "intent_provided must be set true after success");
        assert!(!fd_requested, "fd_requested must remain false — RequestFd never ran");
    }

    /// Guard (a), negative case 1: a SECOND ProvideIntent on the same
    /// connection is rejected fail-closed (Error, mints nothing) — accepted
    /// EXACTLY ONCE.
    #[tokio::test]
    async fn second_provide_intent_is_rejected() {
        let (conn, session_id, mut store, mut last_event_id, mut last_event_hash, mut session_status, ws_root) =
            harness();
        let mut intent_provided = false;
        let mut fd_requested = false;
        let (mut server_end, mut client_end) =
            tokio::net::UnixStream::pair().expect("UnixStream::pair");

        dispatch_request(
            BrokerRequest::ProvideIntent { intent: sample_intent() },
            &mut server_end,
            &conn,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut store,
            &ws_root,
            &mut session_status,
            &mut intent_provided,
            &mut fd_requested,
        )
        .await
        .expect("first ProvideIntent must succeed");
        let _ = read_framed(&mut client_end).await;

        let last_event_id_before_second = last_event_id;
        let last_event_hash_before_second = last_event_hash.clone();

        dispatch_request(
            BrokerRequest::ProvideIntent { intent: sample_intent() },
            &mut server_end,
            &conn,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut store,
            &ws_root,
            &mut session_status,
            &mut intent_provided,
            &mut fd_requested,
        )
        .await
        .expect("dispatch must complete (Error response, not a transport failure)");

        let resp = read_framed(&mut client_end).await;
        assert!(
            matches!(resp, BrokerResponse::Error { .. }),
            "a second ProvideIntent must be rejected fail-closed, got {resp:?}"
        );
        assert_eq!(
            last_event_id, last_event_id_before_second,
            "no chain-head advance on the rejected second ProvideIntent (mints nothing)"
        );
        assert_eq!(last_event_hash, last_event_hash_before_second);
    }

    /// Guard (a), negative case 2: a ProvideIntent arriving AFTER any
    /// RequestFd on the same connection is rejected fail-closed, even though
    /// no ProvideIntent has ever succeeded on this connection.
    #[tokio::test]
    async fn provide_intent_after_request_fd_is_rejected() {
        let (conn, session_id, mut store, mut last_event_id, mut last_event_hash, mut session_status, _ws_root) =
            harness();
        // A real workspace dir + file so RequestFd's read_within succeeds
        // (the arm sets fd_requested at ENTRY regardless, but a real target
        // keeps this test representative of a genuine RequestFd call).
        let mut ws_dir = std::env::temp_dir();
        ws_dir.push(format!("caprun_guard_a_rfd_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&ws_dir).expect("create ws dir");
        std::fs::write(ws_dir.join("workspace.txt"), b"hello").expect("write ws file");
        let ws_root = Arc::new(
            adapter_fs::workspace::WorkspaceRoot::open(&ws_dir).expect("open ws root"),
        );

        let mut intent_provided = false;
        let mut fd_requested = false;
        let (mut server_end, mut client_end) =
            tokio::net::UnixStream::pair().expect("UnixStream::pair");

        dispatch_request(
            BrokerRequest::RequestFd { path: "workspace.txt".to_string() },
            &mut server_end,
            &conn,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut store,
            &ws_root,
            &mut session_status,
            &mut intent_provided,
            &mut fd_requested,
        )
        .await
        .expect("RequestFd must succeed");
        // The RequestFd arm sends the fd's 1-byte SCM_RIGHTS sendmsg payload
        // BEFORE the FdGranted JSON response (mirrors cli/caprun/src/worker.rs's
        // real client-side ordering) — drain that 1 byte via recv_fd FIRST, or
        // read_framed's length-prefix read would be corrupted by it and hang
        // waiting for bytes that will never arrive.
        let received_fd = adapter_fs::recv_fd(client_end.as_raw_fd())
            .expect("recv_fd must consume the RequestFd arm's SCM_RIGHTS payload");
        drop(unsafe { std::fs::File::from_raw_fd(received_fd) });
        let _ = read_framed(&mut client_end).await;
        assert!(fd_requested, "fd_requested must be set true after RequestFd");

        let last_event_id_before_intent = last_event_id;
        let last_event_hash_before_intent = last_event_hash.clone();

        dispatch_request(
            BrokerRequest::ProvideIntent { intent: sample_intent() },
            &mut server_end,
            &conn,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut store,
            &ws_root,
            &mut session_status,
            &mut intent_provided,
            &mut fd_requested,
        )
        .await
        .expect("dispatch must complete (Error response, not a transport failure)");

        let resp = read_framed(&mut client_end).await;
        assert!(
            matches!(resp, BrokerResponse::Error { .. }),
            "ProvideIntent arriving after RequestFd must be rejected fail-closed, got {resp:?}"
        );
        assert!(!intent_provided, "intent_provided must remain false — the mint never ran");
        assert_eq!(
            last_event_id, last_event_id_before_intent,
            "no chain-head advance on the rejected post-RequestFd ProvideIntent"
        );
        assert_eq!(last_event_hash, last_event_hash_before_intent);

        std::fs::remove_dir_all(&ws_dir).ok();
    }
}
