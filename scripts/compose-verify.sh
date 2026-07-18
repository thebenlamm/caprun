#!/usr/bin/env bash
# compose-verify.sh — composed Linux verification harness (Phase 40, LIVE-03/04)
#
# A SIBLING of scripts/mailpit-verify.sh (NOT an edit to it — that recipe stays
# stable for the single-test SMTP runs). This one stands up BOTH sidecars the
# v1.8 composed live proof needs, on ONE user-defined Docker network:
#
#   1. axllent/mailpit  — the SMTP capture sidecar (email.send regression legs).
#   2. mock GitHub HTTPS — scripts/mock-github/server.py on a python:3-slim
#      sidecar; answers 201 to POST /repos/*/pulls so a REAL github.pr POST
#      completes over REAL TLS while riding the SHIPPED broker egress path
#      (validate_url -> allowlist -> resolve-and-pin) unchanged.
#
# Why a PUBLIC-range subnet (203.0.113.0/24, TEST-NET-3 / RFC 5737): the broker's
# ssrf_check REJECTS loopback / RFC1918 / link-local / CGNAT / metadata / ULA
# destinations. Putting the mock on a documentation/test PUBLIC range lets its
# container IP pass ssrf_check UNMODIFIED — no test-only bypass in the TCB. The
# mock host `github-mock.caprun.test` is add-host-mapped to the mock's fixed IP
# ONLY inside the verification container; `api.github.com` is NEVER remapped, so
# the ENV-01 live GET still reaches REAL GitHub over webpki-roots TLS (T-40-06).
#
# TLS trust: the run enables the NON-DEFAULT `brokerd/mock-egress-ca` cargo
# feature (Plan 40-02), whose single checked-in anchor IS the mock's serving cert
# (scripts/mock-github/certs/github-mock.caprun.test.pem). With the feature OFF
# (every release build) the mock is untrusted and unreachable.
#
# Security posture (T-40-07): the verification container runs with ONLY
# `--security-opt seccomp=unconfined` (required — the default seccomp profile
# blocks the landlock()/seccomp() syscalls under test) and NO elevated-privilege
# docker flag; the confinement stack under test needs none. The container's TRUE
# exit code is captured BEFORE any pipe (verification-exit-code-through-pipe
# lesson) and the script exits non-zero on failure — success is NEVER asserted
# through a pipe.
#
# Usage:
#   bash scripts/compose-verify.sh
# Run from the workspace root (same directory as Cargo.toml).
#
# Env overrides (rarely needed):
#   COMPOSE_NET        — Docker network name       (default: caprun-compose-net)
#   COMPOSE_SUBNET     — network subnet            (default: 203.0.113.0/24)
#   MOCK_GITHUB_IP     — mock GitHub fixed IP      (default: 203.0.113.2)
#   MAILPIT_NAME       — Mailpit container name     (default: caprun-compose-mailpit)
#   MOCK_GITHUB_NAME   — mock GitHub container name (default: caprun-compose-mock-github)
#   OPENAI_API_KEY / CAPRUN_PLANNER_MODEL — forwarded like mailpit-verify.sh
#     (key is empty-tolerant; model forwarded ONLY when set — see that script's
#     note on why an empty forwarded model breaks the LLM-sidecar live call).
#   COMPOSE_VERIFY_CMD — the command run inside the rust:1 container. DEFAULT:
#       cargo build --workspace && cargo test --workspace --no-fail-fast \
#         --features brokerd/mock-egress-ca
#     The leading `cargo build --workspace` is REQUIRED (cargo-test-workspace-
#     missing-sibling-binary): it places the nice-named caprun / caprun-worker /
#     caprun-planner sibling binaries in target/ so the spawned broker+worker
#     process tree (git.commit / process.exec) can find them. The feature-
#     carrying `cargo test` step then rebuilds the `caprun` binary (which hosts
#     the in-process broker egress) WITH the mock anchor via cargo feature
#     unification, so its live github.pr POST validates the mock over TLS.
#     Scope a run to one test, e.g.:
#       COMPOSE_VERIFY_CMD='cargo build --workspace && cargo test -p caprun --test live_acceptance_v1_8_composed --features brokerd/mock-egress-ca' \
#         bash scripts/compose-verify.sh

set -euo pipefail

COMPOSE_NET="${COMPOSE_NET:-caprun-compose-net}"
COMPOSE_SUBNET="${COMPOSE_SUBNET:-203.0.113.0/24}"
MOCK_GITHUB_IP="${MOCK_GITHUB_IP:-203.0.113.2}"
MAILPIT_NAME="${MAILPIT_NAME:-caprun-compose-mailpit}"
MOCK_GITHUB_NAME="${MOCK_GITHUB_NAME:-caprun-compose-mock-github}"
MOCK_GITHUB_HOST="github-mock.caprun.test"

# The default's leading `cargo build --workspace &&` places the bin-only sibling
# binaries (see the module doc comment); the `--features brokerd/mock-egress-ca`
# on BOTH the build- and test- friendly graph is carried by the test step, and
# cargo feature unification propagates it into the spawned `caprun` binary.
COMPOSE_VERIFY_CMD="${COMPOSE_VERIFY_CMD:-cargo build --workspace && cargo test --workspace --no-fail-fast --features brokerd/mock-egress-ca}"

cleanup() {
    echo "Cleaning up sidecars (${MAILPIT_NAME}, ${MOCK_GITHUB_NAME}) ..."
    docker stop "${MAILPIT_NAME}"     >/dev/null 2>&1 || true
    docker stop "${MOCK_GITHUB_NAME}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

echo "Ensuring Docker network ${COMPOSE_NET} (${COMPOSE_SUBNET}) exists ..."
# An explicit PUBLIC-range subnet so the mock's fixed IP passes ssrf_check
# unmodified. `|| true`: idempotent if the network already exists from a prior
# run (a stale network with a different subnet would need a manual
# `docker network rm ${COMPOSE_NET}`).
docker network create --subnet "${COMPOSE_SUBNET}" "${COMPOSE_NET}" >/dev/null 2>&1 || true

echo "Starting Mailpit sidecar (${MAILPIT_NAME}) on ${COMPOSE_NET} ..."
docker run -d --rm --name "${MAILPIT_NAME}" --network "${COMPOSE_NET}" \
    --network-alias mailpit \
    -p 8025:8025 -p 1025:1025 \
    axllent/mailpit >/dev/null

# Resolve Mailpit's container IP host-side / unconfined (same Rule-1 rationale as
# mailpit-verify.sh: a bare hostname passed as CAPRUN_SMTP_HOST would need a DNS
# lookup inside a seccomp-confined probe, which its default-deny net filter
# blocks before the connect() EPERM under test).
MAILPIT_IP=$(docker inspect --format '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "${MAILPIT_NAME}")
if [ -z "${MAILPIT_IP}" ]; then
    echo "FAIL — could not resolve ${MAILPIT_NAME}'s container IP" >&2
    exit 1
fi
echo "Resolved Mailpit sidecar IP: ${MAILPIT_IP}"

echo "Starting mock GitHub HTTPS sidecar (${MOCK_GITHUB_NAME}) at ${MOCK_GITHUB_IP} ..."
# python:3-slim + the stdlib-only server.py (NO pip install). Fixed IP in the
# public range so `github-mock.caprun.test` can be add-host-mapped to it below
# and pass ssrf_check. The certs dir is mounted read-only with the rest of
# scripts/mock-github.
docker run -d --rm --name "${MOCK_GITHUB_NAME}" \
    --network "${COMPOSE_NET}" --ip "${MOCK_GITHUB_IP}" \
    --network-alias "${MOCK_GITHUB_HOST}" \
    -v "$PWD/scripts/mock-github":/mock:ro -w /mock \
    python:3-slim python3 server.py >/dev/null

# Readiness: wait until the mock's TLS listener accepts a connection (bounded).
echo "Waiting for mock GitHub HTTPS listener on ${MOCK_GITHUB_IP}:443 ..."
mock_ready=0
for _ in $(seq 1 30); do
    if docker exec "${MOCK_GITHUB_NAME}" \
        python3 -c "import socket; socket.create_connection(('127.0.0.1', 443), timeout=1).close()" \
        >/dev/null 2>&1; then
        mock_ready=1
        break
    fi
    sleep 1
done
if [ "${mock_ready}" -ne 1 ]; then
    echo "FAIL — mock GitHub HTTPS listener did not come up on ${MOCK_GITHUB_IP}:443" >&2
    docker logs "${MOCK_GITHUB_NAME}" >&2 || true
    exit 1
fi
echo "Mock GitHub HTTPS sidecar is ready."

echo "Running composed Linux verification suite (rust:1, network=${COMPOSE_NET}) ..."
# --add-host maps ONLY github-mock.caprun.test -> the mock IP inside the
# verification container; api.github.com is deliberately NOT remapped, so the
# ENV-01 live GET reaches REAL GitHub (T-40-06). Env forwarding mirrors
# mailpit-verify.sh: OPENAI_API_KEY unconditional-but-empty-tolerant;
# CAPRUN_PLANNER_MODEL forwarded ONLY WHEN SET (an empty forwarded model breaks
# the real OpenAI call — see mailpit-verify.sh's note).
docker_args=(
    --rm
    --security-opt seccomp=unconfined
    --network "${COMPOSE_NET}"
    --add-host "${MOCK_GITHUB_HOST}:${MOCK_GITHUB_IP}"
    -v "$PWD":/work -w /work
    -e CARGO_TARGET_DIR=/tmp/lt
    -e CAPRUN_SMTP_HOST="${MAILPIT_IP}"
    -e CAPRUN_SMTP_PORT=1025
    -e CAPRUN_GITHUB_API_BASE="https://${MOCK_GITHUB_HOST}"
    -e OPENAI_API_KEY="${OPENAI_API_KEY:-}"
)
if [ -n "${CAPRUN_PLANNER_MODEL:-}" ]; then
    docker_args+=(-e "CAPRUN_PLANNER_MODEL=${CAPRUN_PLANNER_MODEL}")
fi

# Capture the TRUE exit code of the container run BEFORE any pipe
# (verification-exit-code-through-pipe): a piped `docker run ... | tail` would
# return tail's status (always 0) and mask a real failure. No pipe here at all —
# rc is the container's own exit status.
set +e
docker run "${docker_args[@]}" \
    rust:1 \
    bash -c "apt-get update && apt-get install -y libssl-dev pkg-config && ${COMPOSE_VERIFY_CMD}"
rc=$?
set -e

if [ "${rc}" -ne 0 ]; then
    echo "FAIL — composed verification suite exited ${rc}" >&2
    exit "${rc}"
fi

echo "Composed Linux verification suite PASSED (Mailpit + mock GitHub)."
