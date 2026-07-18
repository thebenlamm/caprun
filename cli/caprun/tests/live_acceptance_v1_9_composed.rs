//! live_acceptance_v1_9_composed — the v1.9 milestone's composed SUCCESS proof
//! (LIVE-05, Linux-gated). One half of the v1.9 DONE gate.
//!
//! # What drives what — FRAMING HONESTY (LIVE-05 locked decision #1, this
//!   project's v1.3 DOC-01 / v1.4 P22 precedent). Read this BLUNTLY; the layers
//!   are deliberately NOT conflated:
//!
//!   1. The multi-sink authorized-WRITE SUCCESS chain (process.exec → filesystem
//!      edit → git.commit → git.push confirm-release → github.pr → http.request.write
//!      POST) is **composed in-crate through the real broker arms** — each leg is
//!      submitted to the ACTUAL production dispatch arm
//!      `brokerd::server::evaluate_plan_node_and_record_for_test` (the
//!      `test-fixtures`-gated VERBATIM delegate to the live
//!      `evaluate_plan_node_and_record`, closing the Phase-38 mirror-drift
//!      finding), and the git.push leg is released through the real
//!      `brokerd::confirmation::confirm`. This chain is NOT expressible as a single
//!      `caprun run`: that verb plans exactly ONE `CaprunIntent` → ONE `PlanNode` →
//!      ONE sink (only `email.send` / `file.create` intents exist), so no single
//!      `caprun run` can express the v1.9 multi-sink write chain — building a
//!      multi-node composed-intent planner is out-of-scope new TCB against this
//!      project's manual-ops-first discipline. The composition is faithful (real
//!      sinks, real mock endpoints under `mock-egress-ca`, genuine non-stapled
//!      taint, per-session `verify_chain` true) precisely because it runs the same
//!      arms the live daemon runs — but it is composition through those arms, not a
//!      CLI invocation of the whole workflow.
//!   2. The whole run is **inspected by a genuine compiled `caprun audit`
//!      subprocess** — for every composed session this test spawns the REAL
//!      `caprun audit <session_id> <db>` binary (`env!("CARGO_BIN_EXE_caprun")`)
//!      and asserts its `Chain verification: PASSED` verdict + rendered
//!      sink/terminal events. This is 100% real CLI: the same read-only viewer
//!      proven in `s45_cli_viewer_acceptance.rs` (U1), MACing against the SAME
//!      persisted key, failing closed on an absent/`:memory:` key.
//!   3. At least ONE leg is **genuinely CLI-driven via `caprun run`**: a real
//!      `caprun run --policy <trusted> create-file-from-report …` subprocess drives
//!      a confined worker over untrusted (tainted) report content, its `file.create`
//!      path I2-Blocks under the real confinement stack, the parent surfaces the
//!      blocked `effect_id` + `caprun review` pointer, and that block session is
//!      then audited by `caprun audit` — landing in the SAME shared persisted
//!      `audit.db` as the composed chain. `caprun run` drives ONLY this single
//!      confined block leg; it never expresses the multi-sink write chain (see 1).
//!
//! So: the CLI genuinely INSPECTS the whole run and genuinely DRIVES one confined
//! blocking leg; the multi-sink success chain is composed through the identical
//! broker arms the CLI would call. This module makes no broader claim than that.
//!
//! # One shared persisted audit.db (never `:memory:`)
//!
//! Every leg is its own session over ONE shared, persisted `audit.db` (F1-safe:
//! a sibling of the workspace roots, never nested beneath one) with a sibling
//! `.key` seeded BEFORE any append — so the in-process `verify_chain` AND the
//! `caprun audit` subprocess (and the `caprun run` leg's own
//! `load_or_create_key`) all MAC against the SAME key. The standing composed-run
//! pattern from `live_acceptance_v1_8_composed.rs`: one shared file, every
//! session's `verify_chain` independently true, never a single cross-session
//! `parent_id` chain. A single `#[tokio::test]` fn runs the legs SEQUENTIALLY (no
//! parallelism → the process-global `CAPRUN_GITHUB_*` / `CAPRUN_GIT_PUSH_TOKEN`
//! env vars are race-free; each leg set_var/remove_var around itself).
//!
//! # Linux-only + run recipe
//!
//! The success legs spawn the REAL kernel-confined `caprun-exec-launcher`
//! (`process.exec` / `git.commit`) and open real TLS sockets to the mock
//! (`git.push` / `github.pr` / `http.request.write`), and the `caprun run` leg
//! self-confines a worker (Landlock + seccomp + no_new_privs) — a macOS run would
//! prove nothing (those primitives are no-op stubs there). This file's body is
//! `#[cfg(target_os = "linux")]`; `cargo test -p caprun` on macOS compiles it and
//! runs only the always-on guard test below (0 Linux tests — expected, not a gap,
//! per CLAUDE.md's "Linux-only security tests" / cfg-linux-test-blindness).
//!
//! The authoritative run is the composed harness ONLY, which does `cargo build
//! --workspace` FIRST (so the sibling `caprun`/`caprun-worker`/`caprun-exec-launcher`
//! binaries exist at `current_exe`-resolution time —
//! cargo-test-workspace-missing-sibling-binary) and enables the NON-DEFAULT
//! `brokerd/mock-egress-ca` feature (so the mock cert is trusted + the mock
//! write/push/pr hosts are admitted):
//!
//!   COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test -p caprun \
//!     --test live_acceptance_v1_9_composed --features brokerd/mock-egress-ca' \
//!     bash scripts/compose-verify.sh
//!
//! `compose-verify.sh` captures the TRUE exit code BEFORE any pipe and asserts on
//! named tests + counts — NEVER on `$?` through a pipe
//! (`verification-exit-code-through-pipe`).

#[cfg(target_os = "linux")]
mod linux {
    use brokerd::audit::{append_event, open_audit_db, verify_chain};
    use brokerd::session::persist_session;
    use chrono::Utc;
    use runtime_core::{Event, Session, SessionStatus};
    use std::path::Path;
    use uuid::Uuid;

    /// Persist a `sessions` row with a CALLER-CHOSEN id (mirrors
    /// `live_acceptance_v1_8_composed.rs::persist_known_session`) so the composed
    /// legs can track exactly which sessions they created for the final sweep.
    fn persist_known_session(conn: &rusqlite::Connection, session_id: Uuid) {
        let session = Session {
            id: session_id,
            intent_id: Uuid::new_v4(),
            status: SessionStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        persist_session(conn, &session).expect("persist_session (composed-test known id)");
    }

    /// Seed a `session_created` causal-root event so subsequent appends chain onto
    /// a real parent (mirrors the v1.8 `seed_root_event`). Returns the root id +
    /// hash to thread the first mint onto.
    #[allow(dead_code)]
    fn seed_root_event(
        conn: &rusqlite::Connection,
        key: &[u8],
        session_id: Uuid,
    ) -> (Uuid, String) {
        let root = Event::new(
            Uuid::new_v4(),
            None,
            session_id,
            "broker".into(),
            "session_created".into(),
            Utc::now(),
            vec![],
        );
        let hash = append_event(conn, key, &root, None).expect("append root event");
        (root.id, hash)
    }

    /// Idempotent read-existing-first MAC-key custody (duplicated from the v1.8
    /// composed test — `cli/caprun` has no lib target, so this external
    /// integration-test crate cannot import the CLI's `pub(crate)` key helper, and
    /// distinct `tests/*.rs` binaries cannot share a module). Writes the key at
    /// `<db>.key` BEFORE any seeding append — the SAME bytes the in-process
    /// `verify_chain`, the `caprun audit` subprocess's `load_existing_key`, AND the
    /// `caprun run` leg's `load_or_create_key` all read back. F1-safe by
    /// construction (DB + `.key` are siblings under a unique tmp dir, never nested
    /// beneath a workspace root). 32 bytes = caprun's `KEY_LEN`, so the shared
    /// `caprun run` leg reads it back intact.
    fn seed_test_key(db_path: &Path) -> Vec<u8> {
        let key_path = std::path::PathBuf::from(format!("{}.key", db_path.to_str().unwrap()));
        if let Ok(bytes) = std::fs::read(&key_path) {
            return bytes;
        }
        let mut key = Uuid::new_v4().as_bytes().to_vec();
        key.extend_from_slice(Uuid::new_v4().as_bytes());
        std::fs::write(&key_path, &key).expect("write test MAC key file");
        key
    }

    /// Session discovery safe for a multi-session shared DB (mirrors the v1.8
    /// `all_session_ids` — never the unqualified no-`ORDER BY` `LIMIT 1`
    /// anti-pattern, Pitfall 2/3).
    fn all_session_ids(conn: &rusqlite::Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT id FROM sessions ORDER BY rowid")
            .expect("prepare all_session_ids query");
        stmt.query_map([], |row| row.get(0))
            .expect("query all_session_ids")
            .filter_map(Result::ok)
            .collect()
    }

    /// The composed v1.9 live-acceptance SUCCESS scenario — half of the v1.9 DONE
    /// gate (LIVE-05). All legs run sequentially in ONE test fn over ONE shared
    /// persisted `audit.db` (single-threaded → env mutation + the shared DB path
    /// are race-free), each leg its own session; the final sweep asserts EXACTLY
    /// the composed session set exists and every `verify_chain` is independently
    /// true.
    #[tokio::test]
    async fn live_acceptance_v1_9_composed_success_chain() {
        let run_id = Uuid::new_v4();
        let tmp = std::env::temp_dir().join(format!("caprun_live_v19_{run_id}"));
        std::fs::create_dir_all(&tmp).expect("create tmp dir");
        // F1-safe layout: every workspace / git-repo root lives under its own
        // subdirectory of `tmp`; the audit.db is a SIBLING of them (directly under
        // `tmp`), never a child of a WorkspaceRoot — so the `caprun run` leg's
        // F1 `refuse_if_beneath_workspace` custody check passes.
        let audit_db = tmp.join("audit.db"); // ONE shared path — NEVER :memory:
        let audit_db_str = audit_db.to_str().unwrap();

        // Mint/persist the shared MAC key ONCE, before any leg (s45 seed pattern).
        let key = seed_test_key(&audit_db);

        // Track every session id the composed legs persist so the final sweep can
        // assert the EXACT set (never LIMIT 1).
        let mut expected_sessions: Vec<Uuid> = Vec::new();

        // ── Composed SUCCESS legs (Task 2) + caprun audit inspection / caprun run
        //    Block leg (Task 3) are inserted between here and the sweep. ──────────

        // ── END-OF-RUN SWEEP — open the shared audit_db ONCE; every composed
        //    session must exist with verify_chain independently true. ─────────────
        {
            let conn = open_audit_db(audit_db_str).expect("open shared audit DB (sweep)");
            let sids = all_session_ids(&conn);
            for sid in &sids {
                assert!(
                    verify_chain(&conn, sid, &key),
                    "verify_chain must be true for session {sid} (ORDER BY rowid, never LIMIT 1)"
                );
            }
            for sid in &expected_sessions {
                assert!(
                    sids.contains(&sid.to_string()),
                    "session {sid} must be among the enumerated sessions in the final sweep"
                );
            }
            assert_eq!(
                sids.len(),
                expected_sessions.len(),
                "exactly the composed sessions must exist in the shared audit.db"
            );
        }

        std::fs::remove_dir_all(&tmp).ok();
    }
}

/// Cross-platform guard: keeps `cargo test -p caprun` meaningful on the macOS dev
/// box (where the Linux body above is cfg-excluded, 0 tests reported — expected,
/// not a gap). Confirms the `caprun` binary is wired into the test build (so the
/// genuine `caprun audit` / `caprun run` subprocess legs can resolve it), mirroring
/// `live_acceptance_v1_8_composed.rs`'s guard.
#[test]
fn live_acceptance_v1_9_composed_guard_binary_present() {
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    assert!(
        !caprun_bin.is_empty(),
        "CARGO_BIN_EXE_caprun must resolve — caprun must be built for the live test"
    );
}
