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
#      introduces — Pitfall 3, RESEARCH.md), then runs
#      `cargo test --workspace --no-fail-fast`.
#   5. Stops/removes the Mailpit sidecar unconditionally (trap on EXIT), even
#      if the test run fails, so no stray container is left behind.
#
# Usage:
#   bash scripts/mailpit-verify.sh
# Run from the workspace root (same directory as Cargo.toml).
#
# Env overrides (rarely needed):
#   MAILPIT_NET   — Docker network name (default: caprun-mailpit-net)
#   MAILPIT_NAME  — Mailpit container name (default: caprun-mailpit)

set -euo pipefail

MAILPIT_NET="${MAILPIT_NET:-caprun-mailpit-net}"
MAILPIT_NAME="${MAILPIT_NAME:-caprun-mailpit}"

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
# --network-alias mailpit (EMPIRICALLY REQUIRED, Assumption A2 resolved):
# Docker's embedded per-network DNS (127.0.0.11) resolves a hostname to a
# container's NAME or an explicit --network-alias — NOT to an arbitrary
# unrelated hostname. Smoke-tested at authoring time: with the container
# named "${MAILPIT_NAME}" (caprun-mailpit) and NO alias, a sibling container
# on the same network got NXDOMAIN resolving "mailpit". Adding
# `--network-alias mailpit` makes `CAPRUN_SMTP_HOST=mailpit` resolve
# correctly regardless of the container's own --name.
docker run -d --rm --name "${MAILPIT_NAME}" --network "${MAILPIT_NET}" \
    --network-alias mailpit \
    -p 8025:8025 -p 1025:1025 \
    axllent/mailpit >/dev/null

echo "Running Linux verification suite (rust:1, network=${MAILPIT_NET}) ..."
# Container-name DNS via --network-alias mailpit (above) — verified
# empirically at authoring time via `docker run --rm --network
# caprun-mailpit-net alpine:3 getent hosts mailpit`, which resolved to the
# sidecar's container IP. No --network host / explicit port-forward
# workaround needed under this Colima setup.
docker run --rm \
    --security-opt seccomp=unconfined \
    --network "${MAILPIT_NET}" \
    -v "$PWD":/work -w /work \
    -e CARGO_TARGET_DIR=/tmp/lt \
    -e CAPRUN_SMTP_HOST=mailpit \
    -e CAPRUN_SMTP_PORT=1025 \
    rust:1 \
    bash -c "apt-get update && apt-get install -y libssl-dev pkg-config && cargo test --workspace --no-fail-fast"

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
