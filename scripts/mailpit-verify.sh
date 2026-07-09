#!/usr/bin/env bash
# mailpit-verify.sh — Mailpit sidecar + Linux verification helper (Phase 13, D-06)
#
# Extends the existing Colima+Docker Linux verification recipe (CLAUDE.md's
# "Linux-only security tests" section) with a `axllent/mailpit` sidecar so
# SMTP-03 (real capture) and SMTP-05 (CRLF fixture) can run against a live
# local capture SMTP + its HTTP API, entirely unprivileged.
#
# A reusable SHELL HELPER, deliberately NOT a docker-compose file — matches
# this project's existing "no docker-compose file in the repo" convention
# (RESEARCH.md Open Question 3 recommendation).
#
# What it does:
#   1. Creates a user-defined Docker network (idempotent — `|| true`).
#   2. Starts `axllent/mailpit` detached on that network, publishing SMTP
#      :1025 and HTTP API :8025.
#   3. Runs the existing `rust:1` verification container ON THE SAME NETWORK
#      with `--security-opt seccomp=unconfined` (required: the default seccomp
#      profile blocks the landlock()/seccomp() syscalls under test) and NO
#      elevated-container-privilege flag — the confinement stack under test is
#      fully unprivileged, and this recipe does not need container privilege
#      either.
#   4. Inside that container: installs `libssl-dev`/`pkg-config` (lettre's
#      default `native-tls` feature is a NEW build dependency this phase
#      introduces — Pitfall 3, RESEARCH.md), then runs the verification
#      command (MAILPIT_VERIFY_CMD, default `cargo test --workspace
#      --no-fail-fast`).
#   5. Stops/removes the Mailpit sidecar unconditionally (trap on EXIT), even
#      if the test run fails, so no stray container is left behind.
#
# Usage:
#   bash scripts/mailpit-verify.sh
# Run from the workspace root (same directory as Cargo.toml).
#
# Env overrides (rarely needed):
#   MAILPIT_NET       — Docker network name (default: caprun-mailpit-net)
#   MAILPIT_NAME      — Mailpit container name (default: caprun-mailpit)
#   MAILPIT_VERIFY_CMD — the command run inside the rust:1 container (default:
#                        `cargo test --workspace --no-fail-fast`). Phase 16
#                        (16-04, BLOCKER-3 3.1): scope a run to a single test,
#                        e.g.
#                        MAILPIT_VERIFY_CMD='cargo test -p caprun --test s9_live_block s9_control_ab_taint_driven' \
#                          bash scripts/mailpit-verify.sh

set -euo pipefail

MAILPIT_NET="${MAILPIT_NET:-caprun-mailpit-net}"
MAILPIT_NAME="${MAILPIT_NAME:-caprun-mailpit}"
# Phase 16 (16-04, BLOCKER-3 3.1): allow a caller to scope the verification
# command to a subset of the suite (e.g. a single new test) instead of always
# running the full `cargo test --workspace --no-fail-fast`. Previously this
# was a bare hardcoded string at the `docker run` invocation below — any plan
# text saying `MAILPIT_VERIFY_CMD='...' bash scripts/mailpit-verify.sh` was
# just a comment, not a real override. Now it is honored.
MAILPIT_VERIFY_CMD="${MAILPIT_VERIFY_CMD:-cargo test --workspace --no-fail-fast}"

cleanup() {
    echo "Cleaning up Mailpit sidecar (${MAILPIT_NAME}) ..."
    docker stop "${MAILPIT_NAME}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "Ensuring Docker network ${MAILPIT_NET} exists ..."
docker network create "${MAILPIT_NET}" >/dev/null 2>&1 || true

echo "Starting Mailpit sidecar (${MAILPIT_NAME}) on ${MAILPIT_NET} ..."
# --rm: auto-remove on stop (cleanup() calls `docker stop`, which then removes
# it). Publishes SMTP :1025 and HTTP API :8025 to the host too, so an
# EXPLORATORY curl from the host (see module doc comment / SUMMARY) can probe
# the live schema without needing a shell inside the network.
#
# --network-alias mailpit (Assumption A2 resolved, kept for convenience/
# debugging): Docker's embedded per-network DNS (127.0.0.11) resolves a
# hostname to a container's NAME or an explicit --network-alias — smoke-tested
# at authoring time (with no alias, a sibling container got NXDOMAIN resolving
# "mailpit"). The actual verification run below does NOT rely on this DNS
# name (see the IP-resolution step and its Rule-1 bug-fix note just below) —
# the alias is kept so a developer can still `curl`/shell into the network by
# the friendly name "mailpit" for manual debugging.
docker run -d --rm --name "${MAILPIT_NAME}" --network "${MAILPIT_NET}" \
    --network-alias mailpit \
    -p 8025:8025 -p 1025:1025 \
    axllent/mailpit >/dev/null

# Resolve Mailpit's container IP OUTSIDE any confined process (Rule 1 bug fix
# discovered at authoring time — see 13-04-SUMMARY.md "Deviations"): the
# pre-existing Phase 13 Plan 03 `negative_net_smtp_mailpit` test drives
# `confine-probe smtp <host> <port>` INSIDE a seccomp-confined child process.
# That process's own default-deny net filter blocks socket() unconditionally
# — which ALSO blocks the DNS query a hostname lookup would need, before
# `confine-probe` ever reaches the `connect()` call whose EPERM it checks for.
# Passing the bare hostname "mailpit" as CAPRUN_SMTP_HOST therefore breaks
# that test with an unrelated "Temporary failure in name resolution" error
# instead of the expected EPERM-at-connect() proof. Resolving to a concrete
# IP here (unconfined, host-side) and passing THAT as CAPRUN_SMTP_HOST avoids
# a DNS lookup ever needing to happen inside the confined probe process,
# while leaving Task 2/3's SMTP-03/05 acceptance tests unaffected (they run
# unconfined and would work with either form).
MAILPIT_IP=$(docker inspect --format '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "${MAILPIT_NAME}")
if [ -z "${MAILPIT_IP}" ]; then
    echo "FAIL — could not resolve ${MAILPIT_NAME}'s container IP" >&2
    exit 1
fi
echo "Resolved Mailpit sidecar IP: ${MAILPIT_IP}"

echo "Running Linux verification suite (rust:1, network=${MAILPIT_NET}) ..."
docker run --rm \
    --security-opt seccomp=unconfined \
    --network "${MAILPIT_NET}" \
    -v "$PWD":/work -w /work \
    -e CARGO_TARGET_DIR=/tmp/lt \
    -e CAPRUN_SMTP_HOST="${MAILPIT_IP}" \
    -e CAPRUN_SMTP_PORT=1025 \
    rust:1 \
    bash -c "apt-get update && apt-get install -y libssl-dev pkg-config && ${MAILPIT_VERIFY_CMD}"

echo "Mailpit-backed Linux verification suite PASSED."

# ──────────────────────────────────────────────────────────────────────────
# EXPLORATORY STEP (Task 1, RESEARCH.md Open Question 2 / Pitfall 4) —
# ACTUALLY RUN against a live Mailpit instance at authoring time (not merely
# predicted from the swagger schema). Method: started a real Mailpit
# container, sent messages via raw SMTP (python smtplib against
# 127.0.0.1:1025, mirroring exactly what lettre's SmtpTransport does on the
# wire), then:
#
#   curl -s http://localhost:8025/api/v1/messages          # LIST endpoint
#   curl -s http://localhost:8025/api/v1/message/<ID>       # DETAIL endpoint
#
# CONFIRMED live field path — the LIST and DETAIL endpoints have DIFFERENT
# shapes for absent Cc/Bcc (a real divergence from Pitfall 4's prediction,
# worth flagging so Task 2/3 don't pick the wrong endpoint):
#
#   GET /api/v1/messages (LIST) -> "Cc": null, "Bcc": null when absent
#     (a per-recipient-array field is NOT reliably an array here — do NOT
#     assert against the list endpoint for the CRLF fixture).
#
#   GET /api/v1/message/{ID} (DETAIL) -> "Cc": [], "Bcc": [] when absent —
#     ALWAYS an array (never null), each entry `{"Name": "", "Address": "..."}`.
#     THIS is the endpoint Task 2/3 assert against.
#
# Empirically verified CRLF-fixture outcome (the actual attack payload run
# against a live Mailpit): a body literal `"hi there\r\nBcc: attacker@evil.com"`
# arrived at Mailpit's DETAIL endpoint as `"Bcc": []` (no smuggled recipient)
# and the full CRLF+Bcc-line text appeared verbatim inside `"Text"` (the body
# field) — proving the injection stays inert body content, never re-parsed as
# a header, exactly as D-22 requires to be VERIFIED rather than assumed.
#
# Field path used by the Task 2/3 acceptance tests (DETAIL endpoint only):
#   message["To"][i]["Address"], message["Cc"][i]["Address"],
#   message["Bcc"][i]["Address"]. This is recorded again in 13-04-SUMMARY.md.
# ──────────────────────────────────────────────────────────────────────────
