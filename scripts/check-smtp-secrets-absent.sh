#!/usr/bin/env bash
# check-smtp-secrets-absent.sh — SMTP-02 structural gate
#
# Re-runnable. Exits non-zero if the SMTP endpoint config token
# `CAPRUN_SMTP_` appears in any worker-spawn code path — the code that
# constructs the confined `caprun-worker` process's env/args.
#
# Files scanned (worker-spawn code paths, enumerated for auditability):
#   - cli/caprun/src/main.rs — specifically the `std::process::Command::new(&worker_binary)`
#     ... `.spawn()` block (the ONLY place that builds caprun-worker's env/args;
#     confirmed by repo-wide grep at authoring time — `caprun-worker` is
#     referenced nowhere else as a spawn target, and no `crates/sandbox/`
#     helper constructs a worker `Command`).
#
# This gate proves the SMTP endpoint/credentials reach ONLY the broker/confirm
# process (crates/brokerd/src/sinks/email_smtp.rs and friends), NEVER the
# confined worker's env, args, or any plan-node payload (SMTP-02, D-04,
# T-13-08). It is a structural (grep) check, not a runtime test — mirrors
# scripts/check-invariants.sh's style and rationale: enforced even before any
# code runs.
#
# Usage: bash scripts/check-smtp-secrets-absent.sh
# Run from the workspace root (same directory as Cargo.toml).

set -euo pipefail

FILE="cli/caprun/src/main.rs"
START_MARKER="std::process::Command::new(&worker_binary)"
END_MARKER=".spawn()"
TOKEN="CAPRUN_SMTP_"

if [ ! -f "$FILE" ]; then
    echo "FAIL — $FILE not found (worker-spawn file missing or moved; update this script)" >&2
    exit 1
fi

echo "Gate: checking for ${TOKEN} in the caprun-worker spawn block of ${FILE} ..."

# Extract the worker-spawn block: from the Command::new(&worker_binary) line
# through its matching .spawn() call (inclusive). Isolating to this block
# (rather than scanning the whole file) avoids false positives from
# unrelated code elsewhere in main.rs, e.g. the `caprun confirm` dispatch
# path, which legitimately lives in the same file but is NOT a worker-spawn
# path (SMTP-02 only restricts what reaches the WORKER's env/args).
BLOCK=$(awk -v start="$START_MARKER" -v end="$END_MARKER" '
    index($0, start) { capturing=1 }
    capturing { print }
    capturing && index($0, end) { exit }
' "$FILE")

if [ -z "$BLOCK" ]; then
    echo "FAIL — could not locate the worker-spawn block (\"$START_MARKER\" ... \"$END_MARKER\") in $FILE" >&2
    echo "       The markers may have drifted from a refactor — update START_MARKER/END_MARKER above." >&2
    exit 1
fi

# Scan non-comment lines only (strip full-line `//` comments so a comment
# documenting "we deliberately never pass CAPRUN_SMTP_ here" doesn't trip
# the gate).
NON_COMMENT=$(printf '%s\n' "$BLOCK" | grep -v '^[[:space:]]*//' || true)

if printf '%s\n' "$NON_COMMENT" | grep -q "$TOKEN"; then
    echo "  FAIL — ${TOKEN} token found in the caprun-worker spawn block (${FILE}):" >&2
    printf '%s\n' "$NON_COMMENT" | grep -n "$TOKEN" >&2
    exit 1
fi

echo "  PASS — no ${TOKEN} token in the caprun-worker spawn block (${FILE})"
exit 0
