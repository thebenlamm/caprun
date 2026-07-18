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
echo "Gate 3: checking mint-call-site restriction (mint_from_read / mint_from_derivation / mint_from_exec / mint_from_http / .mint()) ..."

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
check_mint_token "mint_from_http(" "crates/brokerd/src/quarantine.rs" "crates/brokerd/src/server.rs"
check_mint_token ".mint(" "crates/brokerd/src/quarantine.rs" "crates/brokerd/src/server.rs" "crates/executor/src/value_store.rs"

if [ "$gate3_fail" -eq 0 ]; then
    echo "  PASS — mint_from_read / mint_from_derivation / mint_from_exec / mint_from_http / .mint() restricted to sanctioned loci"
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

# Same never-default discipline for the Phase-40 `mock-egress-ca` feature
# (LIVE-03 / T-40-03). It adds a test CA trust anchor + a test host to the
# broker egress (and, from v1.9 Phase 44, the mock git-receive-pack push host,
# WG-9). Its release-trust-unchanged guard tests are gated
# `#[cfg(not(feature = "mock-egress-ca"))]`, so if the feature ever became a
# `default` those guards would silently compile OUT instead of failing — the
# same false-assurance hazard Gate 4 protects test-fixtures against. Forbid it
# as a default so the mock CA + mock hosts can NEVER ship in a production build.
#
# WORKSPACE-WIDE (v1.9 Phase 44 Plan 05, HYG-01): checked over EVERY member
# Cargo.toml under crates/ and cli/, not just brokerd. Under resolver-3 feature
# unification, ANY workspace member that put `mock-egress-ca` in its own
# `default` list — or in a `default` that forwards to `brokerd/mock-egress-ca`
# — would re-activate the mock CA + mock hosts on the shared brokerd build unit
# for the whole graph, shipping the test egress surface in a release build. A
# brokerd-only scope would miss that. So scan all members.
echo "Gate 4b: checking mock-egress-ca is never a default feature anywhere in the workspace ..."
gate4b_fail=0
while IFS= read -r member_toml; do
    [ -f "$member_toml" ] || continue
    # The member's own `[features]` default list — does it name mock-egress-ca
    # directly, or forward to `<crate>/mock-egress-ca`?
    if awk '/^\[features\]/{f=1; next} /^\[/{f=0} f' "$member_toml" \
         | grep -E '^\s*default\s*=' \
         | grep -q 'mock-egress-ca'; then
        echo "  FAIL — $member_toml declares mock-egress-ca within its default feature set"
        gate4b_fail=1
    fi
done < <(find crates cli -name Cargo.toml 2>/dev/null)
if [ "$gate4b_fail" -eq 0 ]; then
    echo "  PASS — mock-egress-ca is not a default feature of any workspace member"
else
    overall=$FAIL
fi

# ──────────────────────────────────────────────────────────────────────────────
# Gate 5: no aws-lc-rs C-crypto provider anywhere in the workspace build graph
# (Phase 37 FIX 1 — supply-chain / TCB integrity).
#
# The broker (crates/brokerd) links `rustls` and MUST use only the pure-Rust
# `ring` provider (DESIGN §5.1 "minimize untrusted C in the TCB"). Under
# resolver-3 feature unification a single workspace member enabling reqwest's
# `rustls` feature (hyper-rustls default provider + rustls-platform-verifier)
# re-activates the aws-lc-rs C provider on the SHARED `rustls` build unit that
# brokerd also links — silently pulling untrusted C into the broker TCB. Every
# reqwest user (brokerd, cli/caprun-planner) must therefore use
# `rustls-no-provider` + an explicitly-supplied ring `CryptoProvider`.
#
# WORKSPACE scope (NOT `-p brokerd`): feature unification is a workspace-graph
# property, so the assertion must be over the whole graph. `cargo tree -i` prints
# the crate as its first output line when present and errors ("did not match any
# packages") when absent; we match the crate line explicitly (robust to the
# error text). Also assert reqwest introduces NO openssl-sys (lettre's
# pre-existing native-tls path is allowed and, on this graph, pulls none).
# ──────────────────────────────────────────────────────────────────────────────
echo "Gate 5: checking aws-lc-rs is absent from the workspace build graph ..."
if command -v cargo >/dev/null 2>&1; then
    awslc_out=$(cargo tree --workspace -i aws-lc-rs 2>&1 || true)
    if echo "$awslc_out" | grep -qE '^aws-lc-rs v'; then
        echo "  FAIL — aws-lc-rs is in the workspace build graph (C crypto in the TCB):"
        echo "$awslc_out" | sed 's/^/    /'
        overall=$FAIL
    else
        echo "  PASS — aws-lc-rs absent from the workspace build graph (ring-only crypto)"
    fi

    openssl_out=$(cargo tree --workspace -i openssl-sys 2>&1 || true)
    # Allowed ONLY via lettre (native-tls). Fail if any reqwest path pulls it.
    if echo "$openssl_out" | grep -qE '^openssl-sys v' && echo "$openssl_out" | grep -q 'reqwest'; then
        echo "  FAIL — openssl-sys reached via a reqwest path (native-tls leaked into the HTTP client):"
        echo "$openssl_out" | sed 's/^/    /'
        overall=$FAIL
    else
        echo "  PASS — no openssl-sys via reqwest (only lettre's native-tls path is allowed)"
    fi
else
    echo "  SKIP — cargo not found on PATH (cannot audit the build graph)"
fi

# ──────────────────────────────────────────────────────────────────────────────
# Gate 6: containment-predicate anti-drift (v1.9 Phase 42, DESIGN-v1.9 §5.3 /
#         gate-record MAJOR-2).
#
# The at-or-beneath-workspace-root refusal predicate must live in EXACTLY ONE
# place — the shared adapter-fs helper `refuse_if_beneath_workspace` — so that
# BOTH the MAC-key custody path (cli/caprun/src/key.rs, F1) AND the broker
# policy binder (crates/brokerd/src/policy.rs, POLICY-03, lands in Plan 04)
# DELEGATE to it rather than re-inlining a copy that can silently drift to
# weaker semantics at one call site. Realized as TWO complementary checks
# (never a bare comparison-token count, which false-positives on any unrelated
# path comparison and false-negatives on a re-inline spelled differently):
#
#   6a MARKER-UNIQUENESS: the distinctive `containment-predicate` tag (stamped
#      on the canonical refusal line) must appear EXACTLY once across crates/
#      and cli/, and that single hit must be in the shared helper. A verbatim
#      re-inline copies the tag -> count >= 2 -> FAIL. An unrelated comparison
#      carries no tag -> never a false-positive. (Positive exact-count gate, so
#      the marker legitimately appears once in source.)
#   6b ANTI-REINLINE + DELEGATION: the re-inline candidate set is DERIVED
#      dynamically (any file under crates/ cli/ carrying the canonicalize+prefix
#      shape, minus the helper) so a THIRD re-inline site cannot evade a
#      hardcoded list; the canonicalize token is broadened to the bare
#      `canonicalize(` so the method form `path.canonicalize()` is caught too.
#      Any such file must delegate to refuse_if_beneath_workspace instead of
#      re-inlining (catches a divergently-spelled re-inline that omits the
#      marker). A positive delegation check on the long-lived consumers
#      (key.rs, brokerd policy.rs) additionally catches a consumer that DROPPED
#      delegation without re-inlining the full shape; each is checked only once
#      its file exists (skipped gracefully until Plan 04).
# ──────────────────────────────────────────────────────────────────────────────
echo "Gate 6: checking containment-predicate anti-drift (single shared helper + delegation) ..."

gate6_fail=0
CONTAINMENT_MARKER="containment-predicate"
CONTAINMENT_HELPER="crates/adapter-fs/src/containment.rs"

# 6a MARKER-UNIQUENESS ---------------------------------------------------------
marker_hits=$(grep -rln -- "$CONTAINMENT_MARKER" crates/ cli/ 2>/dev/null || true)
marker_count=$(printf '%s' "$marker_hits" | grep -c . || true)
if [ "$marker_count" -ne 1 ]; then
    echo "  FAIL — containment-predicate marker must appear in EXACTLY 1 file, found $marker_count:"
    printf '%s\n' "$marker_hits" | sed 's/^/    /'
    gate6_fail=1
elif [ "$marker_hits" != "$CONTAINMENT_HELPER" ]; then
    echo "  FAIL — the sole containment-predicate marker must live in $CONTAINMENT_HELPER, found in: $marker_hits"
    gate6_fail=1
fi

# 6b ANTI-REINLINE (dynamically-derived site list) + DELEGATION ----------------
# The containment predicate's SHAPE is a filesystem canonicalize paired with a
# path-prefix comparison. That shape must exist in EXACTLY ONE place — the shared
# helper. Rather than trust a HARDCODED call-site list (which a THIRD re-inline
# site would silently evade — v1.9 Phase-42 review MINOR), the re-inline set is
# DERIVED: every file under crates/ cli/ that carries BOTH a canonicalize call
# AND a starts_with( comparison, minus the sanctioned helper. Any remaining file
# re-inlines the predicate and MUST instead delegate to
# refuse_if_beneath_workspace → FAIL.
#
# The canonicalize token is broadened to the bare `canonicalize(` so it catches
# the method form `path.canonicalize()` as well as the free `std::fs::` form —
# the old `std::fs::`-only token let a method-form re-inline evade the gate.
# Fragments keep the gate's own source from containing the literal shape it scans
# for. An inline `planner-discipline-allow` annotation exempts an intentional,
# non-containment use (mirrors Gate 1 / Gate 3).
canon_token=$'canonicalize'"("
prefix_token=$'starts_with'"("

# -F (fixed string): the tokens carry a literal `(`, which under -E/-G would be
# a regex metacharacter and silently fail to match (e.g. `path.canonicalize()`).
reinline_hits=$(grep -rlF -- "$canon_token" crates/ cli/ --include="*.rs" 2>/dev/null || true)
for site in $reinline_hits; do
    [ "$site" = "$CONTAINMENT_HELPER" ] && continue        # the sanctioned sole home
    grep -q -- "$prefix_token" "$site" || continue          # no prefix-compare → not the shape
    if grep -q "planner-discipline-allow" "$site"; then      # intentional non-containment use
        continue
    fi
    echo "  FAIL — $site carries a canonicalize+prefix-compare block (re-inlined containment predicate); must delegate to refuse_if_beneath_workspace"
    gate6_fail=1
done

# DELEGATION (positive check on the long-lived consumers): a consumer that
# silently DROPPED its delegation WITHOUT re-inlining the full shape would slip
# past the anti-reinline scan above, so assert the known containment consumers
# still call the shared helper. Each is checked only once its file exists.
check_delegation_site() {
    local site="$1"
    [ -f "$site" ] || return 0   # skip gracefully if a consumer file is absent
    if ! grep -q "refuse_if_beneath_workspace" "$site"; then
        echo "  FAIL — containment consumer $site does not delegate to refuse_if_beneath_workspace"
        gate6_fail=1
    fi
}
check_delegation_site "cli/caprun/src/key.rs"
check_delegation_site "crates/brokerd/src/policy.rs"

if [ "$gate6_fail" -eq 0 ]; then
    echo "  PASS — containment predicate lives only in the shared helper; all consumers delegate"
else
    overall=$FAIL
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
