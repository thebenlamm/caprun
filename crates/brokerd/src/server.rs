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
// v1.6 HARDEN-04: `create_session`/`persist_session`/`SeedProvenance` are used
// only by `create_session_arm`'s `#[cfg(any(test, feature = "test-fixtures"))]`
// mint sibling — gated identically so a default/featureless build (where that
// sibling does not exist) has no unused-import warning.
#[cfg(any(test, feature = "test-fixtures"))]
use crate::session::{create_session, persist_session};
use adapter_fs::pass_fd;
use anyhow::Context;
use chrono::Utc;
use executor::value_store::ValueStore;
#[cfg(any(test, feature = "test-fixtures"))]
use runtime_core::SeedProvenance;
use runtime_core::{Event, SessionStatus};
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

/// Maximum IPC message size (64 KiB).
///
/// Any message claiming a length beyond this limit is rejected before allocation
/// (T-03-08 DoS mitigate: guard before vec allocation, not after).
const MAX_MSG_SIZE: usize = 64 * 1024;

/// A connection's fixed capability set (Phase 20, PLANNER-02/04).
///
/// Decided ONCE at connection establishment — the accept loop's
/// classification of the first vs. every subsequent connection
/// (`run_broker_server`) — and threaded immutably through `handle_connection`
/// for the life of that connection. NEVER re-derived per-message, mirroring
/// the `session_status`/guard(a) discipline's own "resolved once, trusted-
/// path-only" principle (`DESIGN-session-trust-coherence.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionRole {
    /// The session's single worker connection — Phase 19's one-way
    /// occupancy-latch slot. Full capabilities; `permits` always returns
    /// `true`, so the worker path is byte-identical to pre-Phase-20 behavior.
    Worker,
    /// A capability-restricted second connection (Phase 20's forward-looking
    /// planner seam, PLANNER-02/04). Holds NO mint verb
    /// (`ProvideIntent`/`ReportClaims`/`ReportDerivedClaim`/`CreateSession`)
    /// and no raw-bytes verb (`RequestFd`/`ReportRead`) — `permits` returns
    /// `true` ONLY for `SubmitPlanNode`.
    Planner,
}

impl ConnectionRole {
    /// Pure, fail-closed, default-deny capability decision: does this role
    /// permit dispatching `req`?
    ///
    /// `Worker` permits every verb (no behavior change to the worker path —
    /// DESIGN §2's one-way latch is untouched).
    ///
    /// `Planner` permits ONLY `SubmitPlanNode`. Every other verb is denied by
    /// an EXPLICIT named arm — never a catch-all `_ => false` — mirroring
    /// `BrokerRequest`'s own exhaustive-match discipline so a future verb
    /// addition forces this match to be revisited rather than silently
    /// falling through to a default. This includes denying a mid-stream
    /// `DeclarePlannerRole` re-handshake (role is decided once, at
    /// establishment, never re-derived per message — T-20-02) and denying
    /// `CreateSession` even though it is already gated fail-closed behind
    /// guard-(c)'s `CAPRUN_ENABLE_IPC_CREATE_SESSION` opt-in
    /// (`DESIGN-session-trust-coherence.md` §3: the capability set's
    /// default-deny reasoning must not implicitly assume guard-(c) is the
    /// only thing standing between a planner connection and that arm).
    pub fn permits(&self, req: &BrokerRequest) -> bool {
        match self {
            ConnectionRole::Worker => true,
            ConnectionRole::Planner => match req {
                BrokerRequest::SubmitPlanNode { .. } => true,
                BrokerRequest::ProvideIntent { .. } => false,
                BrokerRequest::ReportClaims { .. } => false,
                BrokerRequest::ReportDerivedClaim { .. } => false,
                BrokerRequest::CreateSession { .. } => false,
                BrokerRequest::RequestFd { .. } => false,
                BrokerRequest::ReportRead { .. } => false,
                BrokerRequest::DeclarePlannerRole => false,
            },
        }
    }
}

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
///   `create_session`'s result). Seeds the ONE shared, monotonic
///   `Arc<Mutex<SessionStatus>>` cell constructed inside this function (v1.6
///   Phase 27, HARDEN-01/X-04 fold, `DESIGN-security-hardening.md` §a/§f) —
///   every connection clones the ARC HANDLE, never a fresh owned snapshot.
/// * `trusted_workspace_path` — the CLI-designated `<workspace-file>` (v1.6
///   Phase 27, HARDEN-01, `DESIGN-security-hardening.md` §a). Resolved to a
///   `(dev, ino)` pair EXACTLY ONCE, HERE, at function entry (review Fix 2:
///   the locked design text pins this identity as "resolved once … at
///   broker startup") — the resulting `trusted_inode: Option<(u64, u64)>`
///   is the value actually threaded through `handle_connection` /
///   `classify_second_connection` / `dispatch_request` for the `RequestFd`
///   arm's `fstat` inode-identity compare; the raw `PathBuf` itself is never
///   passed below this point, so no per-grant `std::fs::metadata` call can
///   ever re-resolve (and potentially re-target) "trusted" mid-session.
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
    trusted_workspace_path: PathBuf,
    // v1.6 Phase 28 (HARDEN-02): the broker-owned MAC key, threaded as a
    // sibling of `conn` — the accepted fallback (28-03-PLAN.md Step C) to
    // bundling the key onto the connection handle itself, since `conn`'s
    // locked guard is consumed as a bare `&rusqlite::Connection` by MANY
    // non-audit call sites (session/pending_confirmations/blocked_literals
    // helpers) that would otherwise all need restructuring. Cloned per
    // connection exactly like `conn` (cheap — an `Arc` pointer clone).
    key: Arc<[u8; 32]>,
) -> anyhow::Result<()> {
    // Approach A: tokio detects the leading NUL and calls from_abstract_name internally.
    let sock_path = format!("\0/agentos/{session_id}");
    let listener = tokio::net::UnixListener::bind(&sock_path)?;

    // v1.6 Phase 27 (HARDEN-01/X-04 fold, DESIGN-security-hardening.md §a/§f,
    // DESIGN-GATE-RECORD-v1.6.md finding F3): session_status becomes a
    // SINGLE shared, monotonic `Arc<Mutex<SessionStatus>>`, constructed
    // EXACTLY ONCE here from the owned `initial_session_status` param —
    // mirroring how `planner_slot_occupied: Arc<AtomicBool>` is constructed
    // inside this function below (never passed in pre-wrapped from
    // `main.rs`). Every connection below clones the ARC HANDLE (a cheap
    // pointer clone) and `dispatch_request` re-reads it under lock at the
    // top of every call — a Worker-connection demotion becomes immediately
    // visible to a Planner connection accepted afterward (closes the X-04
    // staleness gap). F3 pins this cell as construct-once and monotonic
    // (`Active` -> `Draft` only): no connection-setup path below may EVER
    // write `Active` back into it.
    let session_status = Arc::new(Mutex::new(initial_session_status));

    // v1.6 Phase 27 review Fix 2 (Finding 2, design-pin deviation): the
    // CLI-designated `<workspace-file>` identity is resolved ONCE, HERE, at
    // `run_broker_server` entry — mirroring the `session_status` cell's own
    // construct-once precedent immediately above. `DESIGN-security-hardening.md`
    // §a (locked) pins this identity as "resolved once … at broker startup";
    // the PRIOR implementation instead called `std::fs::metadata(trusted_path)`
    // on EVERY `RequestFd` (an ambient, ONGOING, symlink-following
    // resolution), so a post-startup swap/symlink at `trusted_path` could
    // redefine "trusted" mid-session. `trusted_inode` is `Copy` and threaded
    // by value from here down through `handle_connection` /
    // `classify_second_connection` / `dispatch_request` — no call site below
    // ever calls `std::fs::metadata` on the path again. `None` means the
    // file was unresolvable at startup, which fails closed: every RequestFd
    // grant demotes, because no grant can ever match an unresolvable trusted
    // identity.
    let trusted_inode: Option<(u64, u64)> = std::fs::metadata(&trusted_workspace_path)
        .ok()
        .map(|m| (m.dev(), m.ino()));

    // Cross-connection trust-coherence fix (v1.4 Phase 19, TRUST-01/TRUST-02):
    // a ONE-WAY, session-lifetime occupancy latch for the WORKER slot. Set
    // exactly once, the first time a connection is accepted, and NEVER
    // cleared for the life of this `run_broker_server` invocation — no
    // release-on-disconnect, no Drop guard, no reconnect path. A confined
    // worker legitimately holds exactly one connection to its session for the
    // whole run (DESIGN-session-trust-coherence.md §2); a 2nd WORKER-style
    // connection (whether overlapping or arriving after the 1st has cleanly
    // disconnected) would otherwise get pristine per-connection guard(a)
    // locals (`intent_provided`/`fd_requested` below) and a stale
    // `session_status`, letting a confined worker mint an attacker-controlled
    // `UserTrusted` literal. A loop-local `bool` is sufficient here — the
    // accept loop's OWN classification of the first connection is
    // single-threaded (DESIGN §2 line 52); only the SEPARATE planner-slot
    // latch below needs cross-task sharing, because Task 3 classifies every
    // subsequent connection in its own spawned task (T-20-08). Do NOT reset
    // this to `false` anywhere.
    let mut worker_slot_occupied = false;

    // Phase 20 (PLANNER-02/04) accept-loop extension, DESIGN §3: a SECOND,
    // ONE-WAY latch for the single capability-restricted planner connection
    // this session may admit. Unlike `worker_slot_occupied`, this MUST be a
    // shared, atomically-claimable flag: every connection accepted after the
    // worker slot is classified in its OWN spawned task (`classify_second_connection`
    // below) so a stalled/slow connection can never block the accept loop
    // (T-20-08) — two such tasks could otherwise race to admit two "single"
    // planner connections. `compare_exchange` makes the claim atomic. Like
    // the worker slot, it is NEVER cleared for the life of this invocation —
    // no release-on-disconnect, no reconnect.
    let planner_slot_occupied = Arc::new(std::sync::atomic::AtomicBool::new(false));

    loop {
        let (stream, _addr) = listener.accept().await?;

        if !worker_slot_occupied {
            // The FIRST accepted connection takes the worker slot exactly as
            // Phase 19 did — no pre-read of any frame on this path, so the
            // worker path stays byte-identical to Phase 19 (this is what
            // keeps the Phase 19 regression tests and uds_ipc/e2e green).
            worker_slot_occupied = true;

            let conn_clone = conn.clone();
            // Workspace-root capability — cloned per connection exactly like `conn`.
            let workspace_root_clone = workspace_root.clone();
            // Pass connection state by value — each connection owns its own chain state.
            let initial_hash = initial_last_event_hash.clone();
            // v1.6 Phase 27 (X-04/F3): clone the ARC HANDLE (cheap pointer
            // clone) — this is a READ of the shared cell, never a fresh
            // owned re-seed. Never write `Active` into the cell here.
            let session_status_clone = session_status.clone();
            // v1.6 Phase 28 (HARDEN-02): cloned per connection exactly like `conn`.
            let key_clone = key.clone();
            tokio::spawn(async move {
                // ConnectionRole::Worker — full capabilities, `permits`
                // always `true` (Phase 20, PLANNER-02/04).
                if let Err(e) = handle_connection(
                    stream,
                    conn_clone,
                    session_id_uuid,
                    initial_last_event_id,
                    initial_hash,
                    session_status_clone,
                    workspace_root_clone,
                    trusted_inode,
                    ConnectionRole::Worker,
                    key_clone,
                )
                .await
                {
                    eprintln!("[brokerd] connection error: {e}");
                }
            });
            continue;
        }

        // Every SUBSEQUENT accepted connection (Phase 20, PLANNER-02/04,
        // DESIGN §3): classify it in a spawned task with a bounded first-
        // frame read timeout, so a stalled connection can never block the
        // accept loop from servicing later connections (T-20-08).
        let conn_clone = conn.clone();
        let workspace_root_clone = workspace_root.clone();
        let initial_hash = initial_last_event_hash.clone();
        // v1.6 Phase 27 (X-04/F3): SAME shared cell, cloned as a handle —
        // a Worker-connection demotion committed before this Planner
        // connection is accepted is observed immediately via this handle.
        let session_status_clone = session_status.clone();
        let planner_slot_clone = planner_slot_occupied.clone();
        // v1.6 Phase 28 (HARDEN-02): cloned per connection exactly like `conn`.
        let key_clone = key.clone();
        tokio::spawn(async move {
            classify_second_connection(
                stream,
                conn_clone,
                session_id_uuid,
                initial_last_event_id,
                initial_hash,
                session_status_clone,
                workspace_root_clone,
                trusted_inode,
                planner_slot_clone,
                key_clone,
            )
            .await;
        });
    }
}

/// Bounded time to wait for a subsequent connection's FIRST framed message
/// during accept-loop classification (T-20-08). Classification runs in its
/// own spawned task (never inline in the accept loop), so a connection that
/// never sends anything cannot block the accept loop from servicing later
/// connections — it simply times out and is rejected like any other
/// non-`DeclarePlannerRole` first frame.
const CLASSIFY_FIRST_FRAME_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Classify and, if appropriate, service a connection accepted AFTER the
/// worker slot is already occupied (Phase 20, PLANNER-02/04,
/// `DESIGN-session-trust-coherence.md` §3). Reads the connection's FIRST
/// framed message with a bounded timeout:
///
///   * `BrokerRequest::DeclarePlannerRole` AND the planner slot is still free
///     — atomically claim the ONE-WAY planner-slot latch (never released for
///     the life of this `run_broker_server` invocation, mirroring the worker
///     slot), send an `Ack` for the handshake, and run `handle_connection`
///     with `ConnectionRole::Planner`. The `DeclarePlannerRole` message
///     itself is consumed HERE — it is never replayed into
///     `dispatch_request`.
///   * anything else (a different first frame, a timeout, or the planner
///     slot already claimed) — send the SAME "a connection is already
///     established for this session; only one connection per session is
///     permitted" rejection Phase 19 uses, then drop. `handle_connection` is
///     NEVER constructed for a rejected connection, so no per-connection
///     trust state (`ValueStore`/`session_status`/`intent_provided`/
///     `fd_requested`) is ever seeded for it. A plain 2nd worker-style
///     connection (which sends `ProvideIntent`/`RequestFd`/etc. as its first
///     frame, not the handshake) is therefore rejected EXACTLY as Phase 19
///     rejects it.
#[allow(clippy::too_many_arguments)]
async fn classify_second_connection(
    mut stream: tokio::net::UnixStream,
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,
    last_event_id: Uuid,
    last_event_hash: String,
    session_status: Arc<Mutex<SessionStatus>>,
    workspace_root: Arc<adapter_fs::workspace::WorkspaceRoot>,
    trusted_inode: Option<(u64, u64)>,
    planner_slot_occupied: Arc<std::sync::atomic::AtomicBool>,
    key: Arc<[u8; 32]>,
) {
    let first_frame = tokio::time::timeout(
        CLASSIFY_FIRST_FRAME_TIMEOUT,
        read_one_frame(&mut stream),
    )
    .await;

    let is_planner_declare = matches!(
        first_frame,
        Ok(Ok(Some(BrokerRequest::DeclarePlannerRole)))
    );

    if is_planner_declare
        && planner_slot_occupied
            .compare_exchange(
                false,
                true,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            )
            .is_ok()
    {
        // Ack the handshake, then hand off to the normal per-connection
        // request loop with restricted capabilities. The DeclarePlannerRole
        // message that got us here is NOT re-dispatched.
        if send_response(&mut stream, &BrokerResponse::Ack).await.is_err() {
            return;
        }
        if let Err(e) = handle_connection(
            stream,
            conn,
            session_id,
            last_event_id,
            last_event_hash,
            session_status,
            workspace_root,
            trusted_inode,
            ConnectionRole::Planner,
            key,
        )
        .await
        {
            eprintln!("[brokerd] planner connection error: {e}");
        }
        return;
    }

    // Every other case: reject exactly as Phase 19's occupied-slot rejection
    // — the caller still gets a diagnosable framed response, not a bare RST.
    let _ = send_response(
        &mut stream,
        &BrokerResponse::Error {
            message: "a connection is already established for this session; only one connection per session is permitted".into(),
        },
    )
    .await;
}

/// Read one framed `BrokerRequest` (4-byte LE length prefix + JSON body) from
/// `stream`, or `Ok(None)` on a clean EOF before any bytes arrive. Mirrors
/// `handle_connection`'s own per-message framing (length prefix, T-03-08
/// oversized-message guard, serde deserialize), factored out so
/// `classify_second_connection` (Task 3) can read exactly one frame without
/// duplicating that logic inline. NOT used by `handle_connection` itself —
/// the worker path's own loop is left untouched so it stays byte-identical
/// to Phase 19.
async fn read_one_frame(
    stream: &mut tokio::net::UnixStream,
) -> anyhow::Result<Option<BrokerRequest>> {
    let mut len_buf = [0u8; 4];
    match stream.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e.into()),
    }
    let msg_len = u32::from_le_bytes(len_buf) as usize;
    if msg_len > MAX_MSG_SIZE {
        return Err(anyhow::anyhow!("message too large"));
    }
    let mut body = vec![0u8; msg_len];
    stream.read_exact(&mut body).await?;
    let request = serde_json::from_slice::<BrokerRequest>(&body)?;
    Ok(Some(request))
}

/// Handle one IPC connection: loop reading framed requests and dispatching them.
///
/// Framing: 4-byte LE length prefix + JSON body.
/// Guard: length > 64 KiB → reject with Error response, never allocate.
///
/// Owns a per-connection `ValueStore` (HARD-03: session-scoped resolution) and
/// threads `last_event_id` / `last_event_hash` across every message so each
/// appended event chains causally onto the previous one. `session_status`
/// (v1.6 Phase 27, X-04/F3 fold) is the SHARED, monotonic
/// `Arc<Mutex<SessionStatus>>` handle constructed once in `run_broker_server`
/// — this connection holds a cheap Arc clone, never a fresh owned snapshot,
/// and every `dispatch_request` call re-reads it under lock so a demotion
/// committed on ANY connection is immediately visible here.
///
/// `role` (Phase 20, PLANNER-02/04) is this connection's FIXED capability set,
/// decided once by the accept loop's classification (`run_broker_server`) —
/// `ConnectionRole::Worker` for the session's first connection,
/// `ConnectionRole::Planner` for the one additional connection admitted after
/// a `DeclarePlannerRole` handshake (Task 3). BEFORE every `dispatch_request`
/// call, this loop checks `role.permits(&request)`: for `Worker` this is
/// always `true` (the worker path is byte-identical to pre-Phase-20
/// behavior); for `Planner` a non-permitted verb is rejected fail-closed
/// HERE, without ever calling `dispatch_request` and without seeding or
/// advancing any mint/trust state.
#[allow(clippy::too_many_arguments)]
async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    conn: Arc<Mutex<rusqlite::Connection>>,
    session_id: Uuid,
    mut last_event_id: Uuid,
    mut last_event_hash: String,
    session_status: Arc<Mutex<SessionStatus>>,
    workspace_root: Arc<adapter_fs::workspace::WorkspaceRoot>,
    trusted_inode: Option<(u64, u64)>,
    role: ConnectionRole,
    key: Arc<[u8; 32]>,
) -> anyhow::Result<()> {
    // Per-connection ValueStore — scoped to this session ONLY (HARD-03 fix).
    let mut value_store = ValueStore::default();
    // Phase 16 (BLOCKER-1 guard a): per-connection, broker-enforced ordering
    // state — ProvideIntent is accepted EXACTLY ONCE and ONLY BEFORE any
    // RequestFd on this connection. Initialized false per connection (never
    // re-derived from IPC).
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

        // Phase 20 (PLANNER-02/04) pre-dispatch capability gate: fail-closed,
        // default-deny, decided once (via `role`, fixed at connection
        // establishment) and checked on EVERY message — never re-derived
        // per-message. For `ConnectionRole::Worker`, `permits` is always
        // `true`, so this is a no-op and the worker path reaches
        // `dispatch_request` exactly as before Phase 20.
        if !role.permits(&request) {
            send_response(
                &mut stream,
                &BrokerResponse::Error {
                    message: "verb not permitted for this connection's capability set"
                        .into(),
                },
            )
            .await?;
            continue;
        }

        // Phase 20 (PLANNER-04, `DESIGN-session-trust-coherence.md` §7): a
        // `ConnectionRole::Planner` connection's `SubmitPlanNode` is handled
        // HERE, entirely bypassing `dispatch_request`'s
        // `BrokerResponse::PlanNodeDecision` full-decision arm, so a planner
        // connection can NEVER receive `anchors`/`literal_sha256`/`literal`.
        // The `role.permits` gate immediately above guarantees `request` is
        // `SubmitPlanNode` here (Planner's ONLY permitted verb) — this branch
        // reuses `evaluate_plan_node_and_record`, the SAME executor-
        // evaluation-and-durable-recording entry point the worker's
        // `dispatch_request` arm calls below, then projects the decision to
        // the reduced `blocked` boolean itself. `Allowed` -> `false`; every
        // non-`Allowed` outcome (`BlockedPendingConfirmation`, `Denied`,
        // `NotImplemented`) -> `true`.
        if role == ConnectionRole::Planner {
            let BrokerRequest::SubmitPlanNode { plan_node } = &request else {
                unreachable!(
                    "ConnectionRole::Planner.permits() admits only SubmitPlanNode"
                );
            };
            // v1.6 Phase 27 (X-04/F3): re-read the shared cell HERE, at the
            // exact point Step 0.5's I0 deny is evaluated — never a stale
            // snapshot cached at connection setup. This is what lets a
            // Planner connection accepted AFTER a Worker-connection
            // demotion observe `Draft` instead of a stale `Active`.
            let status_snapshot: SessionStatus = session_status
                .lock()
                .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?
                .clone();
            let decision = evaluate_plan_node_and_record(
                plan_node,
                &conn,
                &key[..],
                session_id,
                &mut value_store,
                &workspace_root,
                &status_snapshot,
                &mut last_event_id,
                &mut last_event_hash,
            )
            .await?;
            let blocked = !matches!(decision, runtime_core::ExecutorDecision::Allowed);
            send_response(
                &mut stream,
                &BrokerResponse::PlanNodeDecisionReduced { blocked },
            )
            .await?;
            continue;
        }

        // THE PRODUCTION dispatch_request call site (v1.6 Phase 27): passes
        // the SHARED Arc<Mutex<SessionStatus>> handle this connection holds
        // — NOT a fresh clone, NOT a re-seeded owned value — plus the
        // trusted identity this connection received, FROZEN once at
        // `run_broker_server` entry (Fix 2). Either substitution would
        // silently reintroduce a stale-snapshot bug this plan closes.
        dispatch_request(
            request,
            &mut stream,
            &conn,
            &key[..],
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut value_store,
            &workspace_root,
            &session_status,
            trusted_inode,
            &mut intent_provided,
            &mut fd_requested,
        )
        .await?;
    }
    Ok(())
}

/// Evaluate a `PlanNode` against the executor and durably record the
/// resulting decision — the ENTIRE audit-and-effect side of `SubmitPlanNode`
/// (block-time snapshot, `sink_blocked`/`plan_node_evaluated` event append,
/// blocked-literal side-table write, pending-confirmation checkpoint, and any
/// Allowed-decision sink invocation for `file.create`/`email.send`).
///
/// Phase 20 (PLANNER-04, `DESIGN-session-trust-coherence.md` §7): shared
/// VERBATIM by BOTH the worker's full-decision response path
/// (`dispatch_request`'s `SubmitPlanNode` arm) and the planner-role reduced-
/// response path (`handle_connection`'s pre-dispatch planner branch) — this
/// function performs the SAME evaluation and the SAME durable recording
/// either way; it sends NO response itself and knows nothing about
/// `ConnectionRole`. Each caller projects the returned `ExecutorDecision`
/// into its OWN wire shape (`BrokerResponse::PlanNodeDecision` for the
/// worker, `BrokerResponse::PlanNodeDecisionReduced` for the planner) — the
/// reduction is a caller-side projection of an identical evaluation, never a
/// different one.
#[allow(clippy::too_many_arguments)]
async fn evaluate_plan_node_and_record(
    plan_node: &runtime_core::PlanNode,
    conn: &Arc<Mutex<rusqlite::Connection>>,
    key: &[u8],
    session_id: Uuid,
    value_store: &mut ValueStore,
    workspace_root: &Arc<adapter_fs::workspace::WorkspaceRoot>,
    session_status: &SessionStatus,
    last_event_id: &mut Uuid,
    last_event_hash: &mut String,
) -> anyhow::Result<runtime_core::ExecutorDecision> {
    // The broker mints the effect identity (HARD-06) and passes it into the
    // executor — the executor never mints a Uuid (DESIGN §4 rule 2).
    let effect_id = Uuid::new_v4();
    // session_id comes from the connection — NEVER from the IPC message (HARD-03).
    // session_status likewise — broker-owned state, re-read at the top of
    // dispatch_request / at the point of the Planner branch's Step 0.5 check
    // (v1.6 Phase 27, X-04/F3) — NEVER read from plan_node/IPC (DESIGN §4/§11
    // condition 0). This function only READS it (never mutates), so the
    // caller-side snapshot is passed by shared reference.
    let decision = executor::submit_plan_node(
        session_id,
        effect_id,
        plan_node,
        value_store,
        session_status,
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
            // Placeholder — insert_pending_confirmation (below) computes and
            // stores the REAL whole-row MAC under `key`, ignoring this value
            // (v1.6 Phase 28 Plan 05, HARDEN-02 / X-02).
            mac: String::new(),
        })
    } else {
        None
    };
    let new_hash = {
        let locked = conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
        let hash =
            append_event(&locked, key, &audit_event, Some(last_event_hash)).map_err(|e| {
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
            crate::confirmation::insert_pending_confirmation(&locked, key, pc).map_err(|e| {
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
                key,
                value_store,
                session_id,
                effect_id,
                plan_node,
                workspace_root,
                *last_event_id,
                last_event_hash,
            )?
        };
        *last_event_id = sink_event_id;
        *last_event_hash = sink_hash;
    }

    // Phase 16 (CONTROL-01): on an Allowed `email.send` decision,
    // mirror the file.create Allowed-dispatch above — same locking,
    // head-advance, and two-phase (authorize, then effect) ordering.
    // The executor already ran the I2/CONTENT/session-state predicate
    // and did NOT block, so this fires ONLY on a trusted, never-
    // blocked send; no PendingConfirmation/combined_digest is ever
    // created on this path (that shape is exclusive to the Block
    // path above).
    if matches!(decision, runtime_core::ExecutorDecision::Allowed)
        && plan_node.sink.0 == "email.send"
    {
        // Resolve the FULL arg set from the live per-connection
        // ValueStore into a Vec<ResolvedArg> — the SAME resolve-loop
        // shape as the block-time snapshot above — fail closed if any
        // handle does not resolve (a broker-internal invariant
        // violation; validate_schema already guaranteed presence).
        let mut resolved_args = Vec::with_capacity(plan_node.args.len());
        for arg in &plan_node.args {
            let record = value_store.resolve(&arg.value_id).ok_or_else(|| {
                anyhow::anyhow!(
                    "email.send Allowed-dispatch: arg `{}` handle did not resolve",
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

        let locked = conn
            .lock()
            .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;

        // MAJOR-4: append a durable, OPAQUE email_send_attempted event
        // BEFORE the SMTP socket ever opens — parent-chained onto the
        // just-advanced plan_node_evaluated head, under the SAME lock.
        // A crash/power-loss between attempt and delivery still leaves
        // an audit record naming email.send (mirrors confirm()'s
        // attempt-ledger shape in confirmation.rs, minus the CAS —
        // there is no PendingConfirmation to CAS on this never-
        // blocked path).
        let attempted_event = Event::new(
            Uuid::new_v4(),
            Some(*last_event_id),
            session_id,
            format!("sink:email.send:{effect_id}"),
            "email_send_attempted".into(),
            Utc::now(),
            vec![],
        );
        let attempted_event_id = attempted_event.id;
        let attempted_hash = append_event(&locked, key, &attempted_event, Some(last_event_hash))
            .map_err(|e| {
                eprintln!(
                    "[brokerd] email_send_attempted audit append FAILED (fail-closed): {e}"
                );
                anyhow::anyhow!("email_send_attempted append failed: {e}")
            })?;
        *last_event_id = attempted_event_id;
        *last_event_hash = attempted_hash.clone();

        // AFTER the durable attempt append succeeded — only now does
        // an SMTP connection ever open. REPLAY RESIDUAL RISK (named,
        // not silent): this Allowed path has no CAS/PendingConfirmation
        // — a replayed SubmitPlanNode mints a fresh effect_id and would
        // send again (N submissions => N emails). Accepted for v1.3
        // (the durable per-attempt ledger above makes each send
        // auditable); v2 obligation tracked in .planning/todos/pending.
        let (sink_event_id, sink_hash) =
            crate::sinks::email_smtp::invoke_email_smtp_from_resolved(
                &locked,
                key,
                session_id,
                effect_id,
                &resolved_args,
                attempted_event_id,
                &attempted_hash,
            )?;
        *last_event_id = sink_event_id;
        *last_event_hash = sink_hash;
    }

    Ok(decision)
}

/// The `CreateSession`-over-IPC disabled-path `Error` response (v1.6
/// HARDEN-04). Shared verbatim by both `create_session_arm` siblings below so
/// the wire behavior on this negative path is IDENTICAL whether the mint arm
/// is present-but-runtime-disabled (a `test`/`test-fixtures` build with the
/// env flag unset) or physically absent (a default/featureless build) — only
/// the arm's REACHABILITY changes, never its response shape.
async fn create_session_arm_disabled_response(
    stream: &mut tokio::net::UnixStream,
) -> anyhow::Result<()> {
    send_response(
        stream,
        &BrokerResponse::Error {
            message: "CreateSession over IPC is disabled; set \
                      CAPRUN_ENABLE_IPC_CREATE_SESSION=1 to enable \
                      (test-only path)"
                .into(),
        },
    )
    .await
}

/// The `CreateSession`-IPC forced-`Active` mint arm (v1.6 HARDEN-04).
///
/// Phase 16 (BLOCKER-1 guard c): this in-broker IPC arm forces a fresh
/// session `Active` via `SeedProvenance::TrustedArg` — a forced-Active mint
/// reachable over IPC with NO once-only/ordering guard would, combined with
/// the reach-to-a-send path BLOCKER-1 documents, let ANY IPC caller obtain a
/// trusted-seeded session. It is test-only (the live `cli/caprun` path calls
/// `create_session` directly, never through this arm).
///
/// v1.6 HARDEN-04 hardens this further: the mint body below is now gated
/// `#[cfg(any(test, feature = "test-fixtures"))]` so it is PHYSICALLY ABSENT
/// from a default/featureless build (SC3) — not merely runtime-gated. The
/// runtime env-flag check (exactly the string `"1"`, never `.is_ok()` — F3
/// hardening: an EMPTY inherited `CAPRUN_ENABLE_IPC_CREATE_SESSION=""` must
/// NOT enable it) is RETAINED inside this test-only-compiled arm as
/// deliberate defense-in-depth and to preserve the exact semantics of the 3
/// existing `uds_ipc.rs` tests. The featureless sibling immediately below
/// returns the identical `Error` unconditionally, with no env read at all —
/// no runtime flag can ever re-enable this arm in a default build (D-10).
#[cfg(any(test, feature = "test-fixtures"))]
async fn create_session_arm(
    stream: &mut tokio::net::UnixStream,
    conn: &Arc<Mutex<rusqlite::Connection>>,
    key: &[u8],
    intent_id: Uuid,
) -> anyhow::Result<()> {
    // SC3 build-evidence marker: a stable, greppable string literal that
    // exists ONLY in this test-fixtures-gated sibling. `std::hint::black_box`
    // keeps it live in the compiled artifact (no dead-code elimination) with
    // zero runtime side effect — grepping a compiled binary for this literal
    // demonstrates whether the mint arm is physically present (built
    // `--features test-fixtures`) or absent (a default build).
    let _sc3_marker = std::hint::black_box("HARDEN04_MINT_ARM_PRESENT_v1_6");

    if !matches!(
        std::env::var("CAPRUN_ENABLE_IPC_CREATE_SESSION").as_deref(),
        Ok("1")
    ) {
        let _ = intent_id;
        return create_session_arm_disabled_response(stream).await;
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
            .and_then(|_| append_event(&locked, key, &event, None).map(|_| ())),
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
            .await
        }
        Err(e) => {
            eprintln!("[brokerd] CreateSession error: {e}");
            send_response(
                stream,
                &BrokerResponse::Error {
                    message: "internal error".into(),
                },
            )
            .await
        }
    }
}

/// The featureless (shipped default) sibling of `create_session_arm` (v1.6
/// HARDEN-04, SC3/D-10): the forced-Active mint body above is PHYSICALLY
/// ABSENT from this build configuration — this function contains NO
/// `std::env::var` read at all. It returns the SAME `Error` response the
/// disabled-path above returns, unconditionally: no runtime flag (not even
/// `CAPRUN_ENABLE_IPC_CREATE_SESSION=1`) can ever re-enable the mint in a
/// default/featureless build, because the mint code does not exist in the
/// compiled artifact.
#[cfg(not(any(test, feature = "test-fixtures")))]
async fn create_session_arm(
    stream: &mut tokio::net::UnixStream,
    _conn: &Arc<Mutex<rusqlite::Connection>>,
    _key: &[u8],
    _intent_id: Uuid,
) -> anyhow::Result<()> {
    create_session_arm_disabled_response(stream).await
}

/// Dispatch a single `BrokerRequest`, writing its response to `stream`.
///
/// Extracted from the accept loop (RESEARCH Pitfall 6) so individual arms are
/// unit-testable against a `UnixStream::pair()` without binding a real socket.
///
/// `last_event_id` / `last_event_hash` are advanced (after a successful append)
/// by every arm that records an event, preserving the causal chain.
/// `session_status` (v1.6 Phase 27, X-04/F3 fold, `DESIGN-security-hardening.md`
/// §a/§f) is the SHARED, monotonic `Arc<Mutex<SessionStatus>>` cell constructed
/// once in `run_broker_server` — re-read under lock at the TOP of this
/// function (never trusted as a stale per-connection snapshot), and any
/// arm that demotes (`ReportClaims`, and the `RequestFd` fstat-mismatch
/// demotion) writes `Draft` back through the SAME lock — monotonic
/// `Active -> Draft` only, never re-seeded `Active`. `trusted_inode` is the
/// CLI-designated `<workspace-file>`'s `(dev, ino)` identity, FROZEN ONCE at
/// `run_broker_server` entry (v1.6 Phase 27 review Fix 2, design-pin
/// deviation fix) — the `RequestFd` arm's fstat compare matches the
/// currently-granted file's inode against this frozen pair, NEVER by
/// re-resolving `trusted_path` via `std::fs::metadata` per grant. `None`
/// means the file was unresolvable at startup and fails closed (every grant
/// demotes).
///
/// `intent_provided` / `fd_requested` (Phase 16, BLOCKER-1 guard a) are the
/// broker-enforced, per-connection ordering state for `ProvideIntent`: it is
/// accepted EXACTLY ONCE and ONLY BEFORE any `RequestFd` on this connection.
/// Both start `false` and are set at the RequestFd/ProvideIntent arms below
/// — never reset, never re-derived from IPC.
#[allow(clippy::too_many_arguments)]
pub async fn dispatch_request(
    request: BrokerRequest,
    stream: &mut tokio::net::UnixStream,
    conn: &Arc<Mutex<rusqlite::Connection>>,
    key: &[u8],
    session_id: Uuid,
    last_event_id: &mut Uuid,
    last_event_hash: &mut String,
    value_store: &mut ValueStore,
    workspace_root: &Arc<adapter_fs::workspace::WorkspaceRoot>,
    session_status: &Arc<Mutex<SessionStatus>>,
    trusted_inode: Option<(u64, u64)>,
    intent_provided: &mut bool,
    fd_requested: &mut bool,
) -> anyhow::Result<()> {
    // v1.6 Phase 27 (X-04/F3): re-read the shared, monotonic cell HERE, at
    // the top of every dispatch_request call — never trust a value cached
    // at connection-setup time. Every existing READ of session_status below
    // now reads this freshly-locked snapshot; every existing WRITE below
    // locks the shared cell and commits a monotonic Active->Draft write
    // (never re-seeding Active).
    let mut current_status: SessionStatus = session_status
        .lock()
        .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?
        .clone();

    match request {
        BrokerRequest::CreateSession { intent_id } => {
            create_session_arm(stream, conn, key, intent_id).await?;
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
                append_event(&locked, key, &fd_event, Some(last_event_hash))
                    .context("append fd_granted")?
            };

            // HARDEN-01 (v1.6 Phase 27, DESIGN-security-hardening.md §a,
            // corrected ordering per DESIGN-GATE-RECORD-v1.6.md finding F2;
            // identity FROZEN per review Fix 2): a broker-derived fstat
            // (st_dev, st_ino) inode-identity compare — NEVER a path-string
            // compare (permissive normalization risk: `./x` vs `x` could be
            // treated as trusted) — decides whether this fd-grant stays
            // Active. Computed on the SAME already-open `file` about to be
            // passed to the worker (no re-open, no TOCTOU window) against
            // `trusted_inode`, the CLI-designated `<workspace-file>`'s
            // identity resolved ONCE at `run_broker_server` entry — NEVER
            // by re-resolving the path via `std::fs::metadata` on every
            // grant (that would be an ambient, ongoing, symlink-following
            // resolution letting a post-startup swap/symlink redefine
            // "trusted" mid-session, a deviation from the locked "resolved
            // once … at broker startup" design text). Fail-closed default:
            // any mismatch, any `metadata()` error on the granted file, or
            // an unresolvable `trusted_inode` (`None`), demotes.
            let is_trusted_labeled = match trusted_inode {
                Some((d, i)) => file
                    .metadata()
                    .map(|m| m.dev() == d && m.ino() == i)
                    .unwrap_or(false),
                None => false,
            };

            // Corrected ordering (F2, locked): open -> fstat compare ->
            // commit demotion if untrusted -> THEN pass_fd. The demotion
            // still commits before the fd is released to the worker (D-01),
            // using the fd `read_within` already opened above.
            let mut chain_head_id = fd_event_id;
            let mut chain_head_hash = fd_hash.clone();
            if !is_trusted_labeled {
                // Second broker-side I1 demotion site (D-02 amendment,
                // `quarantine.rs`'s `mint_from_read` is the first) — a
                // genuinely broker-derived act (never worker-asserted),
                // exactly like `fd_requested` being flipped at entry above.
                //
                // v1.6 Phase 27 review Fix 3 (Finding 3, MINOR — single-lock
                // atomicity): the `update_session_status` UPDATE and the
                // `session_demoted` `append_event` now run under ONE
                // `conn.lock()` acquisition, mirroring the SAME atomic
                // pattern `mint_from_read` (quarantine.rs) already uses —
                // §a pins "never a second, separately-locked step." The
                // PRIOR implementation locked `conn` once for the UPDATE and
                // AGAIN, separately, for the append; a panic/failure between
                // those two acquisitions could leave `status = Draft`
                // persisted with no causal `session_demoted` Event (an
                // audit-DAG gap). Direction was already fail-closed (the fd
                // is never passed on that path), but the atomicity pin was
                // violated.
                let demoted_event_id = Uuid::new_v4();
                let demoted_event = Event::new(
                    demoted_event_id,
                    Some(fd_event_id),
                    session_id,
                    "broker".into(),
                    // Reuses the EXACT literal event_type "session_demoted"
                    // mint_from_read already uses (quarantine.rs) so
                    // verify_chain/audit tooling that filters on that token
                    // keeps working.
                    "session_demoted".into(),
                    Utc::now(),
                    vec![],
                );
                let demoted_hash = {
                    let locked = conn
                        .lock()
                        .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                    // (a) Mutable read-model update: the SAME
                    // `update_session_status` helper `mint_from_read` already
                    // uses (no new mint/demotion call site for
                    // check-invariants.sh Gate 3 purposes).
                    crate::session::update_session_status(&locked, session_id, &SessionStatus::Draft)
                        .context("update_session_status (RequestFd fd-grant demotion)")?;
                    // (c) Append-only ledger entry: a session_demoted Event
                    // parented on the fd_granted id just appended above — a
                    // GENUINE causal edge (fd_granted -> session_demoted),
                    // never a stapled tag. SAME `locked` guard as the UPDATE
                    // immediately above — one acquisition, one atomic unit.
                    append_event(&locked, key, &demoted_event, Some(&fd_hash))
                        .context("append session_demoted (RequestFd fd-grant demotion)")?
                };
                // (b) SHARED in-memory cell: a DIFFERENT mutex
                // (`session_status`, not `conn`) — lock + monotonic write
                // (X-04/F3), never a re-seeded Active. Occurs within the
                // same logical demotion block as (a)/(c) above, immediately
                // after the durable pair commits.
                {
                    let mut locked_status = session_status
                        .lock()
                        .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                    *locked_status = SessionStatus::Draft;
                }
                // Not re-read later in THIS call (RequestFd never calls
                // evaluate_plan_node_and_record) — kept for symmetry with
                // the shared-cell write above and any future same-call reader.
                #[allow(unused_assignments)]
                {
                    current_status = SessionStatus::Draft;
                }

                chain_head_id = demoted_event_id;
                chain_head_hash = demoted_hash;
            }

            // pass_fd is blocking (sendmsg) — MUST use spawn_blocking (RESEARCH Pitfall 2).
            // Demotion (if any) already committed above — pass_fd never
            // blocks the read; it only removes the session's authority for
            // irreversible effects (I1). The benign clean path (trusted
            // workspace_rel, inode match) skips the demote branch entirely
            // and stays Active (SC2 — CONTROL-01 must still send).
            let sock_fd = stream.as_raw_fd();
            tokio::task::spawn_blocking(move || {
                let result = pass_fd(sock_fd, file_fd).map_err(|e| anyhow::anyhow!("pass_fd: {e}"));
                drop(file); // close broker's copy after sendmsg completes
                result
            })
            .await
            .context("spawn_blocking pass_fd")??;

            send_response(stream, &BrokerResponse::FdGranted).await?;

            // Advance the chain ONLY after the append + fd-pass succeeded —
            // to fd_granted's id/hash on the trusted path, or to the
            // fd-grant demotion's session_demoted id/hash on the untrusted
            // path (never a sibling fork of fd_granted, mirroring
            // ReportClaims's identical chain-head-advance discipline).
            *last_event_id = chain_head_id;
            *last_event_hash = chain_head_hash;
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
                        key,
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
                // the SHARED in-memory cell to match — unconditionally, since
                // Draft is one-way/idempotent (v1.6 Phase 27, X-04/F3: lock +
                // monotonic write, never a re-seeded Active).
                {
                    let mut locked = session_status
                        .lock()
                        .map_err(|e| anyhow::anyhow!("mutex poisoned: {e}"))?;
                    *locked = SessionStatus::Draft;
                }
                // Not re-read later in THIS call (ReportClaims never calls
                // evaluate_plan_node_and_record) — kept for symmetry with the
                // shared-cell write above and any future same-call reader.
                #[allow(unused_assignments)]
                {
                    current_status = SessionStatus::Draft;
                }
                value_ids.push(value_id);
            }
            send_response(stream, &BrokerResponse::ClaimsReceived { value_ids }).await?;
        }

        BrokerRequest::SubmitPlanNode { plan_node } => {
            // Full worker-path response (Phase 20, DESIGN §7 residual #3):
            // this arm is reachable ONLY on a `ConnectionRole::Worker`
            // connection — the planner-role path is intercepted in
            // `handle_connection`'s pre-dispatch planner branch, BEFORE
            // `dispatch_request` is ever called, and never reaches this arm.
            // `evaluate_plan_node_and_record` is the SAME evaluation-and-
            // durable-recording entry point both paths share; only the
            // RESPONSE shape differs by caller.
            let decision = evaluate_plan_node_and_record(
                &plan_node,
                conn,
                key,
                session_id,
                value_store,
                workspace_root,
                &current_status,
                last_event_id,
                last_event_hash,
            )
            .await?;

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
                    key,
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
            // `primary_role` (T2, DESIGN-slot-type-binding.md §2, Round-1 F3): role is
            // selected INSIDE this intent-variant match, in the SAME arm that produces
            // `primary_literal` — never hardcoded at the shared mint_from_intent call
            // below, which is reached by BOTH variants (recipient for SendEmailSummary,
            // path for CreateFileFromReport). Hardcoding "recipient" there would mistag
            // every file.create path.
            let (primary_literal, primary_role, subject_literal, body_literal): (
                String,
                &'static str,
                Option<String>,
                Option<String>,
            ) = match &intent {
                CaprunIntent::SendEmailSummary { recipient, subject, body } => (
                    recipient.clone(),
                    "recipient",
                    Some(subject.clone()),
                    Some(body.clone()),
                ),
                CaprunIntent::CreateFileFromReport { path } => (path.clone(), "path", None, None),
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
                    key,
                    value_store,
                    session_id,
                    primary_literal,
                    Some(*last_event_id),
                    Some(last_event_hash),
                    Some(primary_role.to_string()),
                )?;
                *last_event_id = recipient_event_id;
                *last_event_hash = recipient_hash;

                let subject_value_id = match subject_literal {
                    Some(subject) => {
                        let (event_id, hash, vid) = mint_from_intent(
                            &locked,
                            key,
                            value_store,
                            session_id,
                            subject,
                            Some(*last_event_id),
                            Some(last_event_hash),
                            Some("subject".to_string()),
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
                            key,
                            value_store,
                            session_id,
                            body,
                            Some(*last_event_id),
                            Some(last_event_hash),
                            Some("body".to_string()),
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

        BrokerRequest::DeclarePlannerRole => {
            // Phase 20 (PLANNER-02/04): this variant is only meaningful as a
            // SUBSEQUENT connection's FIRST framed message, and is consumed
            // directly by the accept-loop classification in
            // `run_broker_server` — a newly-classified planner connection
            // never replays it into this dispatch loop. If it ever arrives
            // HERE, the connection's role is already fixed (established,
            // one way or another); role is decided once, never re-derived
            // per message (T-20-02). Reject fail-closed rather than silently
            // no-op, so a stray or malicious mid-stream re-declaration never
            // appears to succeed. Mints nothing, advances no chain head.
            send_response(
                stream,
                &BrokerResponse::Error {
                    message: "DeclarePlannerRole is only valid as a connection's first \
                              framed message; role is decided once at establishment"
                        .into(),
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

    /// Fixed, non-secret test MAC key (mirrors `audit.rs`'s `TEST_KEY`).
    const TEST_KEY: &[u8] = b"server-rs-unit-test-key-not-secret-32by";

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
    ///
    /// `session_status` is a fresh, test-local `Arc<Mutex<SessionStatus>>`
    /// cell (v1.6 Phase 27, X-04/F3 fold) — NOT the production shared cell,
    /// but the same shape `dispatch_request` now requires. `trusted_inode`
    /// is `None` — a placeholder (none of these guard-(a)-ordering tests
    /// exercise the HARDEN-01 fstat identity compare — that is Task 4's own
    /// harness / `crates/brokerd/tests/harden01_session_integrity.rs`).
    fn harness() -> (
        Arc<Mutex<rusqlite::Connection>>,
        Uuid,
        ValueStore,
        Uuid,
        String,
        Arc<Mutex<SessionStatus>>,
        Arc<adapter_fs::workspace::WorkspaceRoot>,
        Option<(u64, u64)>,
    ) {
        let conn = Arc::new(Mutex::new(open_audit_db(":memory:").expect("open_audit_db")));
        let session_id = Uuid::new_v4();
        let store = ValueStore::default();
        let last_event_id = Uuid::new_v4();
        let last_event_hash = "genesis-hash".to_string();
        let session_status = Arc::new(Mutex::new(SessionStatus::Active));
        let ws_root = Arc::new(
            adapter_fs::workspace::WorkspaceRoot::open(std::env::temp_dir().as_path())
                .expect("open ws root"),
        );
        let trusted_inode: Option<(u64, u64)> = None;
        (
            conn,
            session_id,
            store,
            last_event_id,
            last_event_hash,
            session_status,
            ws_root,
            trusted_inode,
        )
    }

    /// Guard (a), positive case: the first ProvideIntent before any RequestFd
    /// still succeeds — the happy path is unchanged.
    #[tokio::test]
    async fn provide_intent_before_any_request_fd_succeeds() {
        let (conn, session_id, mut store, mut last_event_id, mut last_event_hash, session_status, ws_root, trusted_inode) =
            harness();
        let mut intent_provided = false;
        let mut fd_requested = false;
        let (mut server_end, mut client_end) =
            tokio::net::UnixStream::pair().expect("UnixStream::pair");

        dispatch_request(
            BrokerRequest::ProvideIntent { intent: sample_intent() },
            &mut server_end,
            &conn,
            TEST_KEY,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut store,
            &ws_root,
            &session_status,
            trusted_inode,
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
        let (conn, session_id, mut store, mut last_event_id, mut last_event_hash, session_status, ws_root, trusted_inode) =
            harness();
        let mut intent_provided = false;
        let mut fd_requested = false;
        let (mut server_end, mut client_end) =
            tokio::net::UnixStream::pair().expect("UnixStream::pair");

        dispatch_request(
            BrokerRequest::ProvideIntent { intent: sample_intent() },
            &mut server_end,
            &conn,
            TEST_KEY,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut store,
            &ws_root,
            &session_status,
            trusted_inode,
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
            TEST_KEY,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut store,
            &ws_root,
            &session_status,
            trusted_inode,
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
        let (conn, session_id, mut store, mut last_event_id, mut last_event_hash, session_status, _ws_root, _trusted_inode) =
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
        // Trusted inode == the SAME file being RequestFd'd (fstat'd here,
        // mirroring the frozen-at-startup pattern review Fix 2 requires),
        // so this guard-(a) ordering test's assertions (unrelated to
        // HARDEN-01) are unaffected by the fstat identity compare.
        let trusted_path = ws_dir.join("workspace.txt");
        let trusted_inode: Option<(u64, u64)> = std::fs::metadata(&trusted_path)
            .ok()
            .map(|m| (m.dev(), m.ino()));

        let mut intent_provided = false;
        let mut fd_requested = false;
        let (mut server_end, mut client_end) =
            tokio::net::UnixStream::pair().expect("UnixStream::pair");

        dispatch_request(
            BrokerRequest::RequestFd { path: "workspace.txt".to_string() },
            &mut server_end,
            &conn,
            TEST_KEY,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut store,
            &ws_root,
            &session_status,
            trusted_inode,
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
            TEST_KEY,
            session_id,
            &mut last_event_id,
            &mut last_event_hash,
            &mut store,
            &ws_root,
            &session_status,
            trusted_inode,
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
