#!/usr/bin/env bash
# verify-harden04-featureless.sh — HARDEN-04 (D-10) formalized featureless
# proof (v1.6, Phase 30 Plan 01).
#
# WHY THIS SCRIPT EXISTS (DESIGN-security-hardening.md §d/§j):
# `cli/caprun/tests/harden04_featureless_create_session.rs`'s
# `featureless_create_session_denied_even_with_flag_set` test proves the
# forced-Active `CreateSession` mint arm is PHYSICALLY ABSENT from a
# featureless (default) `brokerd` build. But `scripts/mailpit-verify.sh`'s
# DEFAULT `MAILPIT_VERIFY_CMD` runs a bare `cargo test --workspace`, which
# also builds `crates/brokerd`'s OWN test targets — Cargo's feature resolver
# then unifies `brokerd`'s `test-fixtures` feature onto EVERY member of that
# single build graph, including `cli/caprun`. Under that ambient unification
# the test detects `brokerd::TEST_FIXTURES_ACTIVE == true` and takes its own
# built-in, loud, non-failing SKIP branch (see the test file's own doc
# comment and the `eprintln!` at lines ~203-213) rather than exercising the
# D-10 assertion. A green `cargo test --workspace` run therefore proves
# NOTHING about criterion 4 — it is a false-assurance surface unless this
# script's scoped, genuinely featureless invocation is also run.
#
# This script formalizes the DESIGN-pinned fix as a standalone, host-runnable
# proof, deliberately NOT an edit to the shared `scripts/mailpit-verify.sh`
# (RESEARCH.md Open Question 1: a standalone script keeps every OTHER
# phase's use of the bare `--workspace` recipe unchanged; a genuinely
# featureless `--release` build is a different invocation shape with no test
# targets and no Mailpit dependency of its own, so bolting it unconditionally
# onto the shared script would slow down every future caller of the bare
# recipe for a check only this one criterion needs).
#
# What it does:
#   1. Delegates the actual Linux run to the EXISTING `scripts/mailpit-verify.sh`
#      recipe via a SCOPED `MAILPIT_VERIFY_CMD` override:
#        cargo build --workspace --release && \
#          cargo test -p caprun --test harden04_featureless_create_session
#      The `--release` leg is the genuinely featureless production artifact
#      pinned by DESIGN §j (a release build with no test targets performs no
#      dev-dependency feature unification). The `-p caprun`-scoped `cargo
#      test` leg (NOT `--workspace`) excludes `crates/brokerd`'s own test
#      targets from the build plan, so `brokerd::TEST_FIXTURES_ACTIVE`
#      resolves to `false` here and the D-10 assertion actually executes
#      instead of taking the self-skip branch.
#   2. Captures the delegated run's TRUE exit status into a variable
#      IMMEDIATELY after the command, BEFORE any pipe/tail/grep — this
#      project's standing `verification-exit-code-through-pipe` discipline
#      (a `cmd | tail` returns tail's status, always 0; nearly shipped a
#      false PASS at this project's own Phase 15 gate).
#   3. FALSE-ASSURANCE GUARD (the load-bearing check): inspects the captured
#      log and FAILS (exit 1) if the scoped test still took its self-skip
#      branch — i.e. if ambient feature unification was NOT actually
#      bypassed by this invocation. A green run can never be produced by the
#      D-10 assertion being skipped.
#   4. POSITIVE ASSERTION: requires BOTH the delegated exit status to be 0
#      AND the log to contain the passing libtest line for
#      `featureless_create_session_denied_even_with_flag_set`. A bare exit 0
#      is never treated as proof on its own — the named test must be
#      reported as run and passed.
#   5. OPTIONAL, non-fatal, env-gated defense-in-depth: a `strings`/`nm` scan
#      of the featureless release artifact for a mint-arm marker symbol.
#      DESIGN §d explicitly forbids making symbol inspection the PRIMARY
#      gate (release inlining/stripping bit-rots across Rust/LLVM versions),
#      so this never affects the script's exit status.
#   6. On full success, echoes a clear PASSED sentinel naming criterion 4 and
#      exits 0.
#
# Usage:
#   bash scripts/verify-harden04-featureless.sh
# Run from the workspace root (same directory as Cargo.toml).
#
# Env overrides:
#   HARDEN04_STRINGS_CHECK=1   — enable the optional, non-fatal symbol scan
#                                 (default: off).
#   HARDEN04_LOG               — path to the captured combined stdout+stderr
#                                 log (default: a fresh file under
#                                 "${TMPDIR:-/tmp}").
#
# NOTE: this script authors and syntax-proves the proof path; the
# AUTHORITATIVE green run (real Linux, via Colima/Docker) is executed by the
# orchestrator as part of Phase 30 Plan 02's live-proof wave, not by
# authoring this script.

set -euo pipefail

LOG_FILE="${HARDEN04_LOG:-$(mktemp "${TMPDIR:-/tmp}/harden04-featureless.XXXXXX.log")}"
TEST_NAME="featureless_create_session_denied_even_with_flag_set"

# The exact stable substring of the test's own eprintln! self-skip sentinel
# (cli/caprun/tests/harden04_featureless_create_session.rs, ~lines 203-213):
#   "harden04_featureless_create_session: SKIPPING the D-10 negative
#    assertion -- brokerd::TEST_FIXTURES_ACTIVE is true, ..."
SKIP_SENTINEL="SKIPPING the D-10"

echo "verify-harden04-featureless: delegating to scripts/mailpit-verify.sh with a" \
     "scoped, featureless MAILPIT_VERIFY_CMD ..."
echo "verify-harden04-featureless: log -> ${LOG_FILE}"

# (1) Scoped, genuinely featureless invocation. `--workspace --release`
# builds the featureless production artifact (no test targets, no dev-dep
# feature unification); `-p caprun --test harden04_featureless_create_session`
# (NOT `--workspace`) excludes brokerd's own test targets from the build
# plan so `brokerd::TEST_FIXTURES_ACTIVE` resolves to false.
#
# `set +e` around this one command: under `set -e` (active for the rest of
# this script), a non-zero exit from the delegated run would terminate the
# script IMMEDIATELY, before `rc=$?` on the next line ever executed --
# defeating the whole point of capturing the true exit code. Restored
# immediately after the capture.
set +e
MAILPIT_VERIFY_CMD='cargo build --workspace --release && cargo test -p caprun --test harden04_featureless_create_session' \
  bash scripts/mailpit-verify.sh > "${LOG_FILE}" 2>&1
# (2) Capture the TRUE exit status IMMEDIATELY, before any pipe/tail/grep.
rc=$?
set -e

echo "verify-harden04-featureless: delegated run exit code = ${rc}"
tail -n 60 "${LOG_FILE}" || true

# (3) FALSE-ASSURANCE GUARD: a green run cannot be produced by the D-10
# assertion having been self-skipped. If the sentinel is present, the scoped
# invocation above failed to bypass ambient feature unification (or
# something re-ran the test under `--workspace` semantics) — fail loudly,
# regardless of the delegated exit code.
if grep -q "${SKIP_SENTINEL}" "${LOG_FILE}"; then
  echo "FAIL — criterion 4 NOT proven: the scoped test still took its" \
       "self-skip branch (found sentinel '${SKIP_SENTINEL}' in the log)." >&2
  echo "This means brokerd::TEST_FIXTURES_ACTIVE was true even under the" \
       "scoped -p caprun invocation -- ambient feature unification was NOT" \
       "bypassed. A green exit code here would be a false PASS; refusing" \
       "to report success. See ${LOG_FILE} for the full run." >&2
  exit 1
fi

# (4) POSITIVE ASSERTION: require BOTH exit 0 AND the named test's libtest
# "... ok" line — never trust a bare exit 0 as proof the assertion ran.
if [ "${rc}" -ne 0 ]; then
  echo "FAIL — delegated verification run exited non-zero (${rc})." >&2
  echo "See ${LOG_FILE} for the full run." >&2
  exit 1
fi

if ! grep -Eq "test .*${TEST_NAME}.* \.\.\. ok" "${LOG_FILE}"; then
  echo "FAIL — criterion 4 NOT proven: the named test" \
       "'${TEST_NAME}' was not reported as run+passed in the log." >&2
  echo "A bare exit 0 is not sufficient proof on its own. See ${LOG_FILE}" \
       "for the full run." >&2
  exit 1
fi

# (5) OPTIONAL, non-fatal, env-gated defense-in-depth ONLY. DESIGN §d
# explicitly forbids making this the primary gate (release
# inlining/stripping across Rust/LLVM versions bit-rots this signal), so its
# outcome is reported but never affects the script's exit status.
if [ "${HARDEN04_STRINGS_CHECK:-0}" = "1" ]; then
  echo "verify-harden04-featureless: optional strings/nm defense-in-depth" \
       "scan requested (HARDEN04_STRINGS_CHECK=1) -- informational only," \
       "never fatal."
  RELEASE_LIB="target/release/libbrokerd.rlib"
  if [ -f "${RELEASE_LIB}" ] && command -v strings >/dev/null 2>&1; then
    if strings "${RELEASE_LIB}" | grep -q "HARDEN04_MINT_ARM_PRESENT_v1_6"; then
      echo "verify-harden04-featureless: [informational] mint-arm marker" \
           "symbol FOUND in ${RELEASE_LIB} -- non-fatal, see DESIGN §d for" \
           "why this is defense-in-depth only, not the primary gate."
    else
      echo "verify-harden04-featureless: [informational] mint-arm marker" \
           "symbol NOT found in ${RELEASE_LIB} (expected on a featureless" \
           "build) -- non-fatal."
    fi
  else
    echo "verify-harden04-featureless: [informational] skipping strings" \
         "scan -- ${RELEASE_LIB} or the 'strings' tool is unavailable" \
         "(this build was likely run inside the container, where the" \
         "release artifact is under /tmp/lt, not the mounted host" \
         "target/ dir; this is expected and non-fatal)."
  fi
fi

echo "PASSED — HARDEN-04 criterion 4 proven: the featureless-build D-10" \
     "negative assertion ('${TEST_NAME}') actually executed (no self-skip)" \
     "and passed."
exit 0
