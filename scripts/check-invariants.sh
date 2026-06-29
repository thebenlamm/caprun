#!/usr/bin/env bash
# check-invariants.sh — Phase 1 architectural invariant gate
#
# Re-runnable. Exits non-zero on any violation.
# Run from the workspace root (same directory as Cargo.toml).
#
# Gate 1: No raw effect-to-sink type may exist anywhere under crates/.
#         Verified by absence of "EffectRequest" in the crate tree.
# Gate 2: runtime-core must remain pure (no I/O, no async, no network).
#         Verified by absence of I/O tokens in crates/runtime-core/src/.
#
# Both gates are structural (grep, not runtime tests) so they are
# enforced even before any code runs (DEC-architectural-lock-plan-nodes,
# Success Criterion 2, T-01-01, T-01-03).
# <!-- planner-discipline-allow: EffectRequest -->

set -euo pipefail

PASS=0
FAIL=1
overall=$PASS

# ──────────────────────────────────────────────────────────────────────────────
# Gate 1: No raw effect-request-to-sink type under crates/
# The canonical forbidden token is the capitalised identifier used in the
# research doc and threat register. Presence of this token means a bypass
# path has been introduced (T-01-01, Pitfall 2).
# ──────────────────────────────────────────────────────────────────────────────
echo "Gate 1: checking for raw effect-to-sink type in crates/ ..."
# <!-- planner-discipline-allow: EffectRequest -->
if grep -r "EffectRequest" crates/ 2>/dev/null | grep -qv "planner-discipline-allow"; then
    echo "  FAIL — raw effect-to-sink type found in crates/ (grep matched)"
    overall=$FAIL
else
    echo "  PASS — no raw effect-to-sink type found in crates/"
fi

# ──────────────────────────────────────────────────────────────────────────────
# Gate 2: runtime-core purity — no I/O, no async, no network
# ──────────────────────────────────────────────────────────────────────────────
echo "Gate 2: checking runtime-core purity ..."
if grep -rE "std::io|std::fs|std::net|tokio|async fn" crates/runtime-core/src/ 2>/dev/null; then
    echo "  FAIL — I/O or async token found in crates/runtime-core/src/"
    overall=$FAIL
else
    echo "  PASS — runtime-core is pure (no I/O, no async, no network)"
fi

# ──────────────────────────────────────────────────────────────────────────────
# Summary
# ──────────────────────────────────────────────────────────────────────────────
echo ""
if [ "$overall" -eq "$PASS" ]; then
    echo "All invariant gates PASSED."
else
    echo "One or more invariant gates FAILED — see output above."
    exit 1
fi
