# Phase 42 — Adversarial Code-Trace Review & Hardening

**Milestone:** v1.9 (Authorized Egress + Policy & Audit Surface)
**Phase:** 42 — Policy Layer: Binding, Enforcement & the I2 Boundary
**Trace type:** post-execution fresh adversarial code-trace + Linux regression gate
**Outcome:** POLICY-02 / POLICY-03 SOUND. 1 MAJOR + 1 MINOR + 1 test-blast-radius finding, all fixed.

---

## Verdict summary

| Finding | Severity | Area | Disposition |
|---------|----------|------|-------------|
| POLICY-02 (no Allow-and-skip path) | — | executor gate / `evaluate` returns `Result<(), PolicyDenyKind>` | **SOUND** — no code path produces an Allow-and-skip result; a permit merely falls through to the unmodified I2 collect-then-Block loop. Not weakenable by any policy value. |
| POLICY-03 (genuine hash-chained `policy_bound`) | — | `cli/caprun/src/main.rs`, `crates/brokerd/src/policy.rs` | **SOUND** — `policy_bound` is a real `append_event` chained onto `session_created` and installed as the broker's seed chain head; provable via `verify_chain`, not stapled. |
| `ArgConstraint::permits` bare `starts_with` bypass | **MAJOR** | `crates/runtime-core/src/policy.rs` | **FIXED** (commit 1a56deb) |
| `policy_bound` broke exact audit-event-count assertions | Linux regression (Mac-invisible) | `cli/caprun/tests/e2e.rs` | **FIXED** (commit ae21e85) |
| Gate 6b anti-drift check evadable | **MINOR** | `scripts/check-invariants.sh` | **FIXED** (commit 28f3a48) |

---

## FIX 1 — MAJOR: bypassable coarse arg-constraint matcher

**Finding.** `ArgConstraint::permits` (policy.rs:63-71) used `literal.starts_with(prefix)`.
Consequences:
- A host allowlist entry `api.example.com` **permitted** `api.example.com.evil.com`
  (attacker-registered sibling domain — bare textual prefix, no boundary check).
- A path entry `/ws/out/` **permitted** `/ws/out/../../etc/passwd` (textual prefix
  satisfied; no `..` traversal guard).

**Blast-radius (why MAJOR not CRITICAL).** Does NOT breach POLICY-02/03, and
`broker_default()` ships with **no** `arg_constraints`, so nothing is broken by
default. But this is the documented narrowing primitive Phases 43/44/46 will
exercise (http-write host allowlist, git.push repo allowlist, LIVE-06 policy-deny
leg) — shipping it bypassable would bake the hole in before those phases lean on it.

**Fix.** Boundary-and-traversal-safe matching, kept PURE (runtime-core is
I/O-forbidden — no url/path crate added; operates on the string):
- `entry_permits()`: permit iff `literal == entry`, OR `entry` is a prefix of
  `literal` terminated at a `/` path boundary (entry ends in `/`, or the next
  literal char is `/`). `api.example.com` no longer matches `api.example.com.evil.com`
  (next char `.`); `https://api.example.com` still matches `https://api.example.com/foo`.
- `has_dotdot_segment()`: for a path-style (`/`-rooted) entry, refuse any literal
  carrying a `..` path segment (split on `/`), so `/ws/out/../..` is denied while
  `/ws/out/v1.2..final.txt` (a dotted filename, not a `..` segment) still permits.
- Doc comments on `ArgConstraint` / `permits` / `allowed_prefixes` updated to the
  new semantics, noting fine-grained URL-host parsing / SSRF resolve-and-pin
  remains the sink layer's job (`http_request.rs`) — this is the coarse session gate.

**Regression tests (all FAIL pre-fix):** `host_suffix_bypass_is_denied`,
`scheme_qualified_host_prefix_permits_at_boundary`, `path_traversal_escape_is_denied`,
`textual_prefix_without_boundary_is_denied`. No existing 42-01 unit test asserted
the old bare-prefix behavior (the two existing arg-constraint tests used a `/`-boundary
prefix and an exact-vs-nonmatch case, both still correct), so none needed loosening.

---

## FIX 2 — Linux regression: `policy_bound` shifted the audit-DAG assertions

**Finding.** POLICY-03's genuine `policy_bound` event now sits immediately after
`session_created` and is the broker's seed chain head. `cli/caprun/tests/e2e.rs`
`dag_chain_integrity` asserted "exactly 8 events" with an indexed sequence +
parent-chain walk — it now observes 9. These tests are `#[cfg(target_os="linux")]`,
so the breakage is invisible on the macOS host ([[cfg-linux-test-blindness]]) and
would surface only in the container.

**Fix.** `policy_bound` is a legitimate, intended audit record — expectations
updated, event NOT removed:
- Count 8 → 9; `policy_bound` inserted as `event[1]` (parent = `session_created`);
  `intent_received`(×3) / `fd_granted` / `plan_node_evaluated` / `email_send_attempted`
  / `email_send_succeeded` shifted to indices 2..8 with each `parent_hash` re-pointed
  by one. `intent_received(recipient)` now parents onto `policy_bound`.
- Chain-description doc comments updated (8-event → 9-event).
- Assertions kept STRICT (exact count, exact positions) — no `>=`/range weakening.

**Blast-radius sweep (all of `cli/caprun/tests/`, Linux-gated).** `e2e.rs` is the
**only** test with a full-session exact-count / indexed-sequence / root-position
assertion. Every other real-binary test is robust to the inserted event because it
uses one of: `verify_chain` (order/count-agnostic hash-chain integrity),
`event_type`-scoped `COUNT(*)` (e.g. `email_send_attempted`, `process_exited`),
relative parent-linkage between two named events, or a **dynamic**
`current_chain_head()` lookup (`live_acceptance_v1_3.rs`, `live_acceptance_v1_8_composed.rs`).
Tests that manually seed their own `session_created` DAG (`confirm.rs`, the `s9_*`
synthetic legs) never go through `main.rs`'s policy binding, so they emit no
`policy_bound`. **Files updated: `cli/caprun/tests/e2e.rs` only.**

---

## FIX 3 — MINOR: Gate 6b anti-drift check evadable

**Finding.** `scripts/check-invariants.sh` Gate 6b hardcoded the containment
call-site list (`key.rs`, `policy.rs`) and detected re-inlines via the
`std::fs::canonicalize` token + `starts_with(`. Two evasions: a THIRD re-inline
site is never scanned, and the method form `path.canonicalize()` carries no
`std::fs::` prefix so it slips past the token entirely.

**Fix.**
- Re-inline candidate set DERIVED dynamically: any file under `crates/ cli/`
  carrying BOTH a canonicalize call AND a `starts_with(` comparison, minus the
  sanctioned helper (`adapter-fs/src/containment.rs`) → FAIL.
- Token broadened to the bare `canonicalize(` via fixed-string `grep -F` (the
  literal `(` is a regex metachar under `-E`, which silently fails to match) —
  catches both `std::fs::canonicalize(` and `.canonicalize()`.
- Inline `planner-discipline-allow` exemption honored (mirrors Gate 1/3).
- 6a marker-uniqueness retained; a positive delegation check on the two long-lived
  consumers retained (catches a consumer that DROPPED delegation without re-inlining
  the full shape).

**Adversarial negatives re-run:** clean → PASS; method-form re-inline (old gate
MISSED) → FAIL 6b; free-fn re-inline → FAIL 6b; annotated → PASS; dropped
delegation → FAIL; duplicate marker → FAIL 6a. Confirmed not a false-PASS.

---

## Verification (macOS host)

- `cargo build --workspace --tests` — clean (warnings only, pre-existing).
- `cargo test -p runtime-core -p executor -p brokerd` — all suites pass, 0 failed
  (runtime-core 189 incl. 4 new policy regression tests).
- `bash scripts/check-invariants.sh` — all gates PASS, exit 0.
- **Note:** the audit-DAG assertions in `e2e.rs` are `#[cfg(target_os="linux")]`
  and fully verify only in the container ([[cfg-linux-test-blindness]]); the
  orchestrator re-runs them via `compose-verify.sh` / `mailpit-verify.sh`.

## Commits

| Fix | Commit | Files |
|-----|--------|-------|
| FIX 1 (MAJOR) | `1a56deb` | `crates/runtime-core/src/policy.rs` |
| FIX 2 (Linux regression) | `ae21e85` | `cli/caprun/tests/e2e.rs` |
| FIX 3 (MINOR) | `28f3a48` | `scripts/check-invariants.sh` |
