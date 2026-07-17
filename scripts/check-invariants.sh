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
# Gate 3: mint-call-site restriction (Phase 15, finding #1b + MEDIUM R1)
#
# DETECTION, not PREVENTION: this is a mechanical backstop (defeatable by
# aliasing/wrapping/macro-expanding the call), NOT the load-bearing control.
# The load-bearing PREVENTION is the Result-returning ValueStore::mint
# invariant (EmptyTaint/EmptyProvenance fail-closed guards, crates/executor/
# src/value_store.rs) plus mint_from_derivation's every-element
# file_read-root + concat byte-verify guards (crates/brokerd/src/
# quarantine.rs). This gate only catches the OBVIOUS mistake of a new module
# minting a fresh-rooted ValueRecord by calling mint_from_read /
# mint_from_derivation / ValueStore::mint directly from somewhere other than
# the sanctioned loci — it is defense-in-depth over those guards, not a
# substitute for them.
#
# Restricts four call-site tokens — `mint_from_read(`, `mint_from_derivation(`,
# `mint_from_exec(` (32-05, EXEC-02/EXEC-03), and the ValueStore::mint call
# site `.mint(` (value_store.rs:61, the ACTUAL pub taint+provenance writer
# every mint_from_* helper delegates to) — to:
#   * crates/brokerd/src/quarantine.rs   (the helpers' definition + unit tests)
#   * crates/brokerd/src/server.rs       (the sole dispatch call sites)
#   * crates/executor/src/value_store.rs (`.mint(` ONLY — its own def + tests)
#
# Exemptions (mirroring this project's own test-infrastructure conventions —
# a cargo `tests/*.rs` integration binary, or a `#[cfg(test)] mod tests`
# block, calling these production functions directly to exercise "the SAME
# production functions the handler calls" is NOT a new bypass module; it is
# test-only code that never ships in the production binary):
#   * any file whose path contains "/tests/" (a Cargo integration-test
#     binary, compiled ONLY for `cargo test`);
#   * any line at/after a file's own "#[cfg(test)]" marker (this codebase's
#     convention: exactly one such unit-test module, placed last in the file).
# An inline `<!-- planner-discipline-allow: TOKEN -->` annotation is also
# honored for any other intentional mention (mirrors Gate 1).
# ──────────────────────────────────────────────────────────────────────────────
echo "Gate 3: checking mint-call-site restriction (mint_from_read / mint_from_derivation / mint_from_exec / .mint()) ..."

gate3_fail=0

check_mint_token() {
    local pattern="$1"
    shift
    local allowed_files=("$@")

    local files
    files=$(grep -rl -- "$pattern" crates/ cli/ 2>/dev/null || true)
    for file in $files; do
        # Exemption: any Cargo integration-test binary under a tests/ dir.
        case "$file" in
            */tests/*) continue ;;
        esac

        # Exemption: explicitly allowed production/definition loci.
        local is_allowed=0
        for allowed in "${allowed_files[@]}"; do
            if [ "$file" = "$allowed" ]; then
                is_allowed=1
                break
            fi
        done
        if [ "$is_allowed" -eq 1 ]; then
            continue
        fi

        # Exemption: this file's own #[cfg(test)] unit-test module (if any).
        # `|| true` guards against pipefail: grep exits 1 (no match) when a
        # file has no #[cfg(test)] marker at all, which would otherwise abort
        # the script under `set -euo pipefail`.
        local test_mod_line
        test_mod_line=$(grep -n '#\[cfg(test)\]' "$file" 2>/dev/null | head -1 | cut -d: -f1 || true)

        while IFS=: read -r line content; do
            [ -z "$line" ] && continue
            if echo "$content" | grep -q "planner-discipline-allow"; then
                continue
            fi
            if [ -n "$test_mod_line" ] && [ "$line" -ge "$test_mod_line" ]; then
                continue
            fi
            echo "  FAIL — \"$pattern\" found outside sanctioned loci: $file:$line"
            gate3_fail=1
        done < <(grep -n -- "$pattern" "$file")
    done
}

check_mint_token "mint_from_read(" "crates/brokerd/src/quarantine.rs" "crates/brokerd/src/server.rs"
check_mint_token "mint_from_derivation(" "crates/brokerd/src/quarantine.rs" "crates/brokerd/src/server.rs"
check_mint_token "mint_from_exec(" "crates/brokerd/src/quarantine.rs" "crates/brokerd/src/server.rs"
check_mint_token ".mint(" "crates/brokerd/src/quarantine.rs" "crates/brokerd/src/server.rs" "crates/executor/src/value_store.rs"

if [ "$gate3_fail" -eq 0 ]; then
    echo "  PASS — mint_from_read / mint_from_derivation / mint_from_exec / .mint() restricted to sanctioned loci"
else
    overall=$FAIL
fi

# ──────────────────────────────────────────────────────────────────────────────
# Gate 4: no-default-test-fixtures (v1.6 HARDEN-04 / D-10, Phase 27 review Fix 1)
#
# `brokerd::TEST_FIXTURES_ACTIVE` (crates/brokerd/src/lib.rs) reflects
# `cfg!(feature = "test-fixtures")`, and D-10's negative gate
# (harden04_featureless_create_session.rs) trusts that const to distinguish
# a genuinely featureless build from ambient Cargo feature unification. That
# trust only holds if `test-fixtures` can NEVER be a `default` feature of
# crates/brokerd/Cargo.toml — if it were, TEST_FIXTURES_ACTIVE would read
# true even in a shipped build, the D-10 skip would fire for every build,
# and the CreateSession-mint arm could ship live while looking legitimate.
# Verified by absence of `test-fixtures` inside brokerd's `[features]`
# `default = [ ... ]` list.
# ──────────────────────────────────────────────────────────────────────────────
echo "Gate 4: checking test-fixtures is never a default feature of crates/brokerd ..."

BROKERD_TOML="crates/brokerd/Cargo.toml"
if [ -f "$BROKERD_TOML" ] && \
   awk '/^\[features\]/{f=1; next} /^\[/{f=0} f' "$BROKERD_TOML" \
     | grep -E '^\s*default\s*=' \
     | grep -q 'test-fixtures'; then
    echo "  FAIL — crates/brokerd/Cargo.toml declares test-fixtures within its default feature set"
    overall=$FAIL
else
    echo "  PASS — test-fixtures is not a default feature of crates/brokerd"
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
