//! llm_planner_live_accept — Phase 21's live PLANNER-03 acceptance (Linux-gated)
//!
//! Proves, with REAL captured output (not asserted from code alone), that a
//! real OpenAI-backed `LlmPlanner` run (Plan 21-03's worker-side proxy talking
//! to Plan 21-02's `caprun-planner` sidecar over an abstract UDS) drives a
//! clean, trusted `send-email-summary` intent to an Allowed decision and an
//! actual SMTP delivery captured by Mailpit.
//!
//! This is the CLEAN-PATH-ONLY proof. The hostile/Block scenario (a tainted
//! doc routed through the LLM planner and I2-blocked) is explicitly OUT OF
//! SCOPE here — that is Phase 22's HARD GATE (GATE-01..04), which needs an
//! adversarial planner-prompt-injection harness this plan does not build.
//!
//! Fixture: reuses the exact `CLEAN_PATH_CONTENT` shape from
//! `live_acceptance_v1_3.rs` / `s9_live_block.rs` — a plain email address with
//! NO `Reply-To:`/`Domain:` marker fragments, so the confined worker's
//! extractor derives no tainted recipient. The `LlmPlanner` is offered three
//! handles (recipient/subject/body) and is expected to route the trusted
//! `intent_value_id` (the CLI-supplied clean recipient) to `email.send`'s
//! `to` argument — copying the offered handle ID verbatim, never a literal
//! (parse_planner_response's fail-closed validator rejects a literal / an
//! unoffered handle).
//!
//! Run (Linux, via the project's standing Mailpit-aware recipe — CLAUDE.md,
//! "From Phase 16 onward, ALL Linux verification goes through
//! scripts/mailpit-verify.sh"). `cargo build -p caprun-planner` runs FIRST so
//! the sidecar binary exists in target/debug alongside `caprun`/`caprun-worker`
//! (caprun's main.rs resolves it via `current_exe().parent().join("caprun-planner")`,
//! which a bare `cargo test -p caprun` build does not produce on its own):
//!
//!   MAILPIT_VERIFY_CMD='cargo build -p caprun-planner && cargo test -p caprun \
//!     --test llm_planner_live_accept llm_planner_clean_allow_delivers' \
//!     bash scripts/mailpit-verify.sh
//!
//! Requires a real `OPENAI_API_KEY` in the HOST env (forwarded by
//! mailpit-verify.sh's Task-1 change into the rust:1 container). If the key
//! is absent, the live test SKIPS cleanly (prints a notice and returns) rather
//! than hard-failing a keyless CI run — matching this plan's `<done>` and
//! `<action>` requirements. The delivered-live path (this file, run WITH a
//! key) is the real proof; a keyless skip proves nothing on its own.

#[cfg(target_os = "linux")]
const CLEAN_PATH_CONTENT: &[u8] =
    b"Q3 financial summary prepared. Route distribution to reports@internal.example as instructed.";

// ─────────────────────────────────────────────────────────────────────────────
// A minimal, std-only Mailpit HTTP client — reproduced verbatim from
// `live_acceptance_v1_3.rs`'s `mod mailpit_client` (deliberate small
// duplication: distinct test binaries cannot share a module in this
// workspace). Do NOT re-derive the chunked-decode handling — copied as-is,
// including its own empirically-discovered gotcha comments.
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(target_os = "linux")]
mod mailpit_client {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;

    /// Mailpit's HTTP API port is FIXED at 8025 by Mailpit's own convention —
    /// distinct from the SMTP port (CAPRUN_SMTP_PORT, 1025).
    const HTTP_PORT: u16 = 8025;

    /// The Mailpit host — the SAME env var the broker/adapter reads for the
    /// SMTP connection itself (CAPRUN_SMTP_HOST). scripts/mailpit-verify.sh
    /// sets this to the sidecar's resolved container IP, never the literal
    /// "mailpit".
    pub fn host() -> String {
        std::env::var("CAPRUN_SMTP_HOST").unwrap_or_else(|_| "127.0.0.1".to_string())
    }

    fn http_request(method: &str, host: &str, port: u16, path: &str) -> String {
        let mut stream = TcpStream::connect((host, port)).unwrap_or_else(|e| {
            panic!("failed to connect to Mailpit HTTP API at {host}:{port}: {e}")
        });
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let request =
            format!("{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).unwrap();
        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .unwrap_or_else(|e| panic!("failed reading Mailpit HTTP API response: {e}"));

        let sep_pos = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .unwrap_or_else(|| {
                panic!(
                    "malformed HTTP response from Mailpit (no header/body separator): {}",
                    String::from_utf8_lossy(&raw)
                )
            });
        let headers = String::from_utf8_lossy(&raw[..sep_pos]).to_lowercase();
        let body_bytes = &raw[sep_pos + 4..];

        // Mailpit's Go HTTP server streams JSON responses without a
        // precomputed Content-Length, so larger LIST responses arrive
        // Transfer-Encoding: chunked despite the `Connection: close` request
        // header. Decode the chunk framing before ever converting to a `str`.
        let body = if headers.contains("transfer-encoding: chunked") {
            decode_chunked(body_bytes)
        } else {
            body_bytes.to_vec()
        };
        String::from_utf8_lossy(&body).into_owned()
    }

    /// Decode an HTTP/1.1 chunked-transfer-encoded body into its unwrapped
    /// bytes. Operates purely on `&[u8]` (never a lossy `str` split) so a
    /// chunk boundary landing mid-multi-byte-UTF-8-character never corrupts
    /// the decode.
    fn decode_chunked(mut body: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let nl = match body.windows(2).position(|w| w == b"\r\n") {
                Some(p) => p,
                None => break,
            };
            let size_str = std::str::from_utf8(&body[..nl]).unwrap_or("0").trim();
            let size = usize::from_str_radix(size_str, 16).unwrap_or(0);
            let data_start = nl + 2;
            if size == 0 || data_start + size > body.len() {
                break;
            }
            out.extend_from_slice(&body[data_start..data_start + size]);
            let after_data = data_start + size;
            let next_start = if body.get(after_data..after_data + 2) == Some(b"\r\n") {
                after_data + 2
            } else {
                after_data
            };
            body = &body[next_start..];
        }
        out
    }

    fn http_get_json(host: &str, port: u16, path: &str) -> serde_json::Value {
        let body = http_request("GET", host, port, path);
        serde_json::from_str(&body)
            .unwrap_or_else(|e| panic!("failed to parse Mailpit JSON response: {e}\nbody: {body}"))
    }

    fn message_ids(host: &str) -> Vec<String> {
        let list = http_get_json(host, HTTP_PORT, "/api/v1/messages?limit=250");
        list["messages"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|m| m["ID"].as_str().map(String::from))
            .collect()
    }

    /// Fetch a message's DETAIL (the endpoint whose `To` field is always an
    /// array) and check whether `recipient` appears in it.
    fn detail_addressed_to(host: &str, id: &str, recipient: &str) -> bool {
        let detail = http_get_json(host, HTTP_PORT, &format!("/api/v1/message/{id}"));
        detail["To"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|entry| entry["Address"].as_str() == Some(recipient))
            })
            .unwrap_or(false)
    }

    /// Poll until a message addressed to `recipient` is captured; panics on
    /// timeout.
    pub fn wait_for_recipient_captured(host: &str, recipient: &str) {
        for _ in 0..50 {
            for id in message_ids(host) {
                if detail_addressed_to(host, &id, recipient) {
                    return;
                }
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        panic!("timed out waiting for a message addressed to {recipient} to appear in Mailpit");
    }

    /// Returns the COUNT of captured messages addressed to `recipient` —
    /// recipient-scoped (never a whole-inbox `len()`), so a per-run-unique
    /// recipient makes the count exact even under a shared Mailpit sidecar.
    pub fn count_messages_for_recipient(host: &str, recipient: &str) -> usize {
        message_ids(host)
            .iter()
            .filter(|id| detail_addressed_to(host, id, recipient))
            .count()
    }
}

/// The live clean-path LLM-planner acceptance: a single `caprun
/// send-email-summary` run with `CAPRUN_PLANNER=llm`, driving a real OpenAI
/// tool-call through the sidecar/proxy wiring to an Allowed decision and a
/// real delivered email.
#[cfg(target_os = "linux")]
#[test]
fn llm_planner_clean_allow_delivers() {
    use brokerd::audit::{find_event_by_type, open_audit_db, verify_chain};

    // Keyless CI must not hard-fail — skip cleanly (this plan's <done>
    // requirement). The delivered-live path (run WITH a key, e.g. via
    // scripts/mailpit-verify.sh) is the real proof this test exists for.
    if std::env::var("OPENAI_API_KEY").unwrap_or_default().is_empty() {
        eprintln!(
            "SKIP llm_planner_clean_allow_delivers: OPENAI_API_KEY is unset/empty in this \
             environment — this live test requires a real OpenAI key. Run via \
             scripts/mailpit-verify.sh with OPENAI_API_KEY set in the host env for the real proof."
        );
        return;
    }

    let run_id = uuid::Uuid::new_v4();
    let tmp = std::env::temp_dir().join(format!("caprun_llm_live_{run_id}"));
    std::fs::create_dir_all(&tmp).expect("create tmp dir");
    // F1-safe layout: workspace file under its own subdirectory, audit.db a
    // sibling of that subdirectory (never a direct child of the workspace root).
    let ws_dir = tmp.join("workspace");
    std::fs::create_dir_all(&ws_dir).expect("create workspace dir");
    let audit_db = tmp.join("audit.db");
    let workspace_file = ws_dir.join("clean.txt");
    std::fs::write(&workspace_file, CLEAN_PATH_CONTENT).expect("write workspace file");

    let clean_recipient = format!("llmclean-{run_id}@example.test");

    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    let output = std::process::Command::new(caprun_bin)
        .arg("send-email-summary")
        .arg(&clean_recipient)
        .arg(workspace_file.to_str().unwrap())
        .arg(audit_db.to_str().unwrap())
        // Select the real LLM planner path (Phase 21's Planner-trait seam).
        // OPENAI_API_KEY / CAPRUN_PLANNER_MODEL are inherited from this
        // process's own environment (Command inherits the parent env by
        // default) — mailpit-verify.sh's Task-1 change is what makes a real
        // key present in that ambient environment when run in-container.
        .env("CAPRUN_PLANNER", "llm")
        .output()
        .expect("spawn caprun (send-email-summary, CAPRUN_PLANNER=llm)");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    eprintln!("caprun (llm clean) stdout:\n{stdout}");
    eprintln!("caprun (llm clean) stderr:\n{stderr}");

    assert!(
        output.status.success(),
        "caprun send-email-summary with CAPRUN_PLANNER=llm must exit 0 for a clean, trusted \
         intent (real Allowed decision expected) — got status {:?}",
        output.status
    );
    assert!(
        stdout.contains("Chain verification: PASSED"),
        "caprun's printed audit output must show 'Chain verification: PASSED'"
    );

    // Independently confirm the Allowed decision + delivery in the durable
    // audit DB (never asserted from stdout text alone).
    let conn = open_audit_db(audit_db.to_str().unwrap()).expect("open audit DB");
    let session_id: String = conn
        .query_row(
            "SELECT id FROM sessions ORDER BY rowid DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("at least one session row must exist for the run just performed");

    assert!(
        verify_chain(&conn, &session_id),
        "verify_chain must be true for the LLM-planner-driven session"
    );
    assert!(
        find_event_by_type(&conn, &session_id, "sink_blocked")
            .expect("query sink_blocked")
            .is_none(),
        "clean control leg via the LLM planner must have NO sink_blocked event"
    );
    assert!(
        find_event_by_type(&conn, &session_id, "email_send_succeeded")
            .expect("query email_send_succeeded")
            .is_some(),
        "clean control leg via the LLM planner must have an email_send_succeeded event"
    );

    // The real proof: exactly one message captured by Mailpit for the clean
    // recipient — the LLM planner's PlanNode routed the UserTrusted
    // intent_value_id handle to email.send/to and the executor Allowed it.
    let host = mailpit_client::host();
    mailpit_client::wait_for_recipient_captured(&host, &clean_recipient);
    assert_eq!(
        mailpit_client::count_messages_for_recipient(&host, &clean_recipient),
        1,
        "the LLM-planner-driven clean-path run must deliver EXACTLY ONCE to its own recipient"
    );
}

/// Cross-platform guard: keeps `cargo test -p caprun` meaningful on the macOS
/// dev box (where the live body above is cfg-excluded). The real live
/// assertions run under Colima/Docker on Linux via scripts/mailpit-verify.sh
/// (see the module header command).
#[test]
fn llm_planner_live_accept_guard_binary_present() {
    let caprun_bin = env!("CARGO_BIN_EXE_caprun");
    assert!(
        !caprun_bin.is_empty(),
        "CARGO_BIN_EXE_caprun must resolve — caprun must be built for the live test"
    );
}
