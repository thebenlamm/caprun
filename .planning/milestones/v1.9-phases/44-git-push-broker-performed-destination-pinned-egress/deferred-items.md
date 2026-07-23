# Phase 44 — Deferred / Out-of-Scope Items

## Pre-existing bare-container Linux test failures (NOT regressions from 44-04)

Observed while running the FULL `cargo test -p brokerd` in the bare `rust:1`
container (`docker run … rust:1`) during 44-04 Linux verification. All are
pre-existing / environmental — NOT caused by Plan 44-04 (which touches only
`git_push.rs`, `confirmation.rs`, `audit.rs`, `server.rs`; `process_exec.rs` is
byte-identical to the pre-plan baseline `0933649`).

1. `sinks::process_exec::capture_bytes_tests::run_launcher_capture_bytes_feeds_stdin_binary_round_trip`
2. `sinks::process_exec::capture_bytes_tests::run_launcher_capture_bytes_separates_stdout_from_stderr`
   - These spawn `/bin/cat` / `/bin/sh` under the confined launcher. They fail
     in the bare container (Landlock exec of `/bin/*` under this image), but the
     SAME `run_launcher_capture_bytes` function is exercised successfully by the
     new git.push tests (which spawn `git`), proving stdout/stderr separation
     works. `process_exec.rs` is unmodified by this plan.
3. `email_smtp_acceptance::smtp_03_confirmed_send_captured_by_mailpit`
4. `email_smtp_acceptance::smtp_05_crlf_body_cannot_smuggle_recipient`
5. `replay_cas::allowed_email_send_replay_delivers_once`
   - These require the Mailpit SMTP sidecar. CLAUDE.md: "From Phase 16 onward,
     ALL Linux verification goes through `scripts/mailpit-verify.sh`" — the bare
     `docker run rust:1` recipe has no SMTP listener, so a live `email.send`
     cannot be delivered. Expected failure without the sidecar; not a 44-04
     regression.

Plan 44-04's own Linux done-gate (`cargo test -p brokerd server:: confirmation::`)
passes on BOTH the default and `mock-egress-ca` feature builds.
