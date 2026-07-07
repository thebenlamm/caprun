#!/usr/bin/env bash
# check-email-smtp-construction.sh — Phase 13 CRLF/header-injection structural gate
#
# Re-runnable. Exits non-zero on any violation.
# Run from the workspace root (same directory as Cargo.toml).
#
# Gate 1: crates/brokerd/src/sinks/email_smtp.rs — the ONLY code path that
#         performs an SMTP call (D-03) — must NEVER use lettre's raw
#         pre-encoded-header constructor. Verified by absence of the token
#         `dangerous_new_pre_encoded` on any NON-COMMENT line (a doc-comment
#         mention of the forbidden token, e.g. in a warning note, must not
#         self-invalidate this gate).
# Gate 2: The same file must construct the outgoing message through lettre's
#         typed `Message::builder()` — verified by its presence — proving the
#         typed builder is the (only sanctioned) construction path.
#
# Both gates are structural (grep, not runtime tests), mirroring
# check-invariants.sh's style (DESIGN-content-adapter-mediation.md
# "Wire-Message Construction", SMTP-05, D-07/D-22).
#
# <!-- planner-discipline-allow: dangerous_new_pre_encoded -->

set -euo pipefail

PASS=0
FAIL=1
overall=$PASS

TARGET="crates/brokerd/src/sinks/email_smtp.rs"

if [ ! -f "$TARGET" ]; then
    echo "  FAIL — $TARGET not found (run from the workspace root)"
    exit "$FAIL"
fi

# Non-comment lines only: strip lines whose first non-whitespace characters
# are `//`, so a doc-comment reference to the forbidden token (e.g. this
# script's own conventions, or a warning in a doc comment) cannot
# self-invalidate the gate.
NON_COMMENT_LINES="$(grep -vE '^[[:space:]]*//' "$TARGET")"

# ──────────────────────────────────────────────────────────────────────────────
# Gate 1: no raw pre-encoded-header constructor on any non-comment line
# ──────────────────────────────────────────────────────────────────────────────
echo "Gate 1: checking for the raw pre-encoded-header constructor in $TARGET ..."
# <!-- planner-discipline-allow: dangerous_new_pre_encoded -->
if echo "$NON_COMMENT_LINES" | grep -q "dangerous_new_pre_encoded"; then
    echo "  FAIL — raw pre-encoded-header constructor found on a non-comment line"
    overall=$FAIL
else
    echo "  PASS — no raw pre-encoded-header constructor found"
fi

# ──────────────────────────────────────────────────────────────────────────────
# Gate 2: Message::builder must be present (proves the typed builder path)
# ──────────────────────────────────────────────────────────────────────────────
echo "Gate 2: checking for Message::builder usage in $TARGET ..."
if grep -q "Message::builder" "$TARGET"; then
    echo "  PASS — Message::builder is present"
else
    echo "  FAIL — Message::builder not found; the typed builder path is not proven"
    overall=$FAIL
fi

# ──────────────────────────────────────────────────────────────────────────────
# Summary
# ──────────────────────────────────────────────────────────────────────────────
echo ""
if [ "$overall" -eq "$PASS" ]; then
    echo "All CRLF-defense gates PASSED."
else
    echo "One or more CRLF-defense gates FAILED — see output above."
    exit 1
fi
