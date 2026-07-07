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
use crate::quarantine::{mint_from_intent, mint_from_read, Claim};
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
) -> anyhow::Result<()> {
    match request {
        BrokerRequest::CreateSession { intent_id } => {
            // Session-independent: mints its own fresh session (parent_id None);
            // does NOT thread the connection chain. This in-broker IPC path is
            // test-only (the live cli/caprun path calls create_session directly,
            // not through this arm) and always seeds SeedProvenance::TrustedArg.
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
                };
                let (_read_event_id, _read_hash, value_id, demoted_event_id, demoted_hash) = {
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
                    )?
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
            // A block persists the durable anchor via the broker-owned
            // Event::sink_blocked constructor (sets Event.taint = anchor.taint,
            // anchor = Some). The causal parent stays the chain head — NOT
            // read_event_id (two graphs are never equated, DESIGN §0/§4 rule 3).
            // On a block, capture the LIVE literal so it can be written to the
            // redactable `blocked_literals` side table (keyed by the sink_blocked
            // event id). The literal is NEVER put in the hashed event payload — the
            // anchor carries only its digest (`literal_sha256`).
            let (audit_event, blocked_literal) = match &decision {
                runtime_core::ExecutorDecision::BlockedPendingConfirmation { anchor, literal } => (
                    Event::sink_blocked(
                        Uuid::new_v4(),
                        Some(*last_event_id), // causal head — never read_event_id
                        session_id,
                        Utc::now(),
                        anchor.clone(),
                    ),
                    Some(literal.clone()),
                ),
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
                    None,
                ),
            };
            let event_type = audit_event.event_type.clone();
            let new_hash = {
                let locked = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                let hash =
                    append_event(&locked, &audit_event, Some(last_event_hash)).map_err(|e| {
                        eprintln!("[brokerd] {event_type} audit append FAILED (fail-closed): {e}");
                        anyhow::anyhow!("audit append failed: {e}")
                    })?;
                // Write the raw literal to the redactable side table under the SAME
                // lock as the event append (fail-closed: a failure here aborts the
                // block before any response is sent).
                if let Some(literal) = &blocked_literal {
                    crate::audit::insert_blocked_literal(
                        &locked,
                        &audit_event.id.to_string(),
                        literal,
                    )
                    .map_err(|e| {
                        eprintln!("[brokerd] blocked-literal side-table write FAILED (fail-closed): {e}");
                        anyhow::anyhow!("blocked-literal write failed: {e}")
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

        BrokerRequest::ProvideIntent { intent } => {
            // Extract the user-provided literal from the typed intent.
            // Exhaustive match: adding a new CaprunIntent variant causes a compile error here,
            // forcing the dispatcher to be updated (no silent unhandled variants).
            let literal = match &intent {
                CaprunIntent::SendEmailSummary { recipient } => recipient.clone(),
                CaprunIntent::CreateFileFromReport { path } => path.clone(),
            };

            // Mint inside the per-connection ValueStore (Pitfall 1: minting outside
            // handle_connection would put the ValueId in an unreachable store → Denied).
            // mint_from_intent is the ONLY caller site of mint_from_intent (T-06-05).
            let (intent_event_id, intent_hash, value_id) = {
                let locked = conn
                    .lock()
                    .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                // Causal parent = the connection chain head (DESIGN §0): intent_received
                // is parent-linked onto session_created, keeping the parent_id chain unbroken.
                mint_from_intent(
                    &locked,
                    value_store,
                    session_id,
                    literal,
                    Some(*last_event_id),
                    Some(last_event_hash),
                )?
            };

            // Advance the causal chain AFTER successful mint (same pattern as ReportClaims arm).
            *last_event_id = intent_event_id;
            *last_event_hash = intent_hash;

            send_response(stream, &BrokerResponse::IntentAccepted { value_id }).await?;
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
