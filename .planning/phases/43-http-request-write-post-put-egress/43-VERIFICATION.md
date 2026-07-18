---
phase: 43
status: passed
verified_by: orchestrator
date: 2026-07-18
---

# Phase 43 Verification — `http.request` WRITE (POST/PUT) Egress (HTTP-W-01)

**Verdict: PASSED.** Proven green on real Linux (compose-verify, **584 passed / 0 failed**); the
fresh non-self Fable-5 adversarial code-trace of the full TCB diff returned **APPROVE, 0 defects**
(2 non-actionable NITs). All 4 plans executed sequentially on `main` with zero deviations from the
written steps (one cosmetic commit-trailer omission on `661ab19`, noted below — not a code change).

## Goal-backward check

Phase 43's goal: extend the shipped `http.request` GET path to a DISTINCT, CommitIrreversible
`http.request.write` POST/PUT sink whose body is taint-governed under I2, on a distinct
write-allowlist, proven **differentially** (taint the sole variable), without weakening I0/I1/I2 or
adding any raw `EffectRequest` path or new mint site.

| Requirement | Evidence | Status |
|-------------|----------|--------|
| HTTP-W-01 (I0 discipline) | `sink_effect_class("http.request.write")` has an EXPLICIT `CommitIrreversible` arm (`sink_sensitivity.rs`) + the `_ =>` fail-closed default — a draft/untrusted-seeded session I0-Denies a POST (the **MAJOR-1 I0-escape** fix; a WRITE must NOT inherit the GET's `Observe` fall-through-to-Allowed). A distinct sink id from GET `http.request`; the GET single-arg `{url}` row is untouched. Tests: `http_request_write_is_commit_irreversible`, `leg_a_i0_draft_session_denies_write_commit_irreversible`, `http_write_draft_session_denies_commit_irreversible`. | ✅ (P43) |
| HTTP-W-01 (I2 taint on body/url) | `body` content-sensitive, `url` routing+content-sensitive; a tainted body/url deterministically Blocks under the byte-unchanged I2 collect-then-Block loop, genuinely propagated (minted through the real `mint_from_http` provenance, never stapled), `verify_chain` true. The differential test asserts B/C are byte-identical in url/method/policy — taint is the SOLE variable — with LEG B's Block anchored on the `body` arg specifically. Tests: `legs_b_and_c_taint_is_the_sole_variable`, `dispatch_tainted_body_blocks_and_never_writes`. | ✅ (P43) |
| HTTP-W-01 (method enum) | Exact `{POST,PUT}` fail-closed enum gate in `submit_plan_node` (resolved from the taint-aware value_store); any other literal (garbage/tainted/mis-cased/whitespace/empty) → `DenyReason::InvalidMethod`, and it does NOT mask a body Block for a valid method. Broker-side `validate_write_method` is the single shared enum (precheck + egress, no drift). Tests: `http_write_invalid_method_denies`, `leg_d_method_outside_enum_denies_fail_closed`. | ✅ (P43) |
| HTTP-W-01 (distinct write-allowlist + SSRF) | Distinct `WRITE_HOST_ALLOWLIST` (ships EMPTY/fail-closed — DESIGN §2.1 names no production write host; MOCK_EGRESS_HOST only under the non-default `mock-egress-ca` feature, `not(mock-egress-ca)` invariant test). Gated BEFORE resolve (no rebind window); reuses the shipped `validate_url → resolve_and_pin → vet/ssrf_check → redirect(none)` verbatim — no classifier re-implemented, `invoke_pinned_post` (GET-allowlist + GitHub headers) NOT reused. Tests: `invoke_http_write_rejects_non_write_allowlisted_host_before_resolve`, non-https/userinfo/port/method rejects. | ✅ (P43) |
| HTTP-W-01 (credential + opaque audit, no mint) | Optional broker-env credential (`CAPRUN_HTTP_WRITE_TOKEN`), read only in `write_bearer()`; never a plan arg/ValueNode/audit literal/log line. Two-phase audit events OPAQUE (payload-scrub test greps all persisted `http_write_*` payloads for url/body/token = absent). Module mints NOTHING — check-invariants Gate 3 mint-site allow-list byte-identical; the write response never enters the value store. Tests: `opaque_audit_no_url_body_or_token_in_appended_event`, `allowed_http_request_write_reaches_dispatch_records_terminal_event_no_mint`. | ✅ (P43) |
| HTTP-W-01 (confirm-release, P33/P34 discipline) | A Blocked tainted-body write is releasable by exactly one `caprun confirm`: the Step-4.8 `prepare_http_write` precheck (the SAME fn the dispatch uses — structural no-drift) runs BEFORE Step 5/6 burn the one-shot; a precheck failure is fail-closed-RECOVERABLE (row stays Pending, no dangling `confirm_granted` with no terminal event). Entry-guard allow-list AND Step-7 dispatch match both list the sink (in sync). Tests: `confirm_on_pending_http_write_releases_exactly_once_no_dangling`, `confirm_on_http_write_malformed_precheck_does_not_burn`. | ✅ (P43) |
| HTTP-W-01 (live mock-receipt clause) | **Carried to Phase 46 (LIVE-05/06) by design.** Phase 43 proves the differential at the decision+dispatch boundary — the clean leg REACHES the egress dispatch (a terminal `http_write_failed` event, absent a live endpoint) while the tainted leg Blocks with NO `http_write_*` event and never opens the socket. The requirement's literal "clean leg actually delivered the body to the mock endpoint on real Linux (mock records receipt)" sub-clause needs a live write mock, which does not exist until the composed proof (the write allowlist ships empty). Plan 43-04 scopes this explicitly. | ⏳ → P46 |

## Hard-constraint checks

- **No raw `EffectRequest`** (Gate 1), **no new mint site** (Gate 3 byte-identical — `http_write.rs`
  mints nothing), **DenyReason exhaustive** (`InvalidMethod` owned fields, no wildcard at any call
  site), **Gate 5** aws-lc-rs/openssl-sys absent (ZERO new crates — HYG-01 held; reuses shipped
  reqwest+rustls(ring)+webpki-roots), **Gate 4b** mock-egress-ca non-default. `check-invariants.sh`
  exit 0, all gates PASS. ✅
- **Lock discipline:** the server.rs Allowed arm mints nothing (`output_value_id = None`) and never
  holds the audit-db mutex across the write `.await` (awaits `post_write` before `conn.lock()`). ✅
- **Linux (authoritative):** `compose-verify.sh` (full workspace + `brokerd/mock-egress-ca` + mock
  GitHub) — **584 passed / 0 failed**, script self-reports "Composed Linux verification suite
  PASSED". `s43_http_write_differential` 5/5 (all named legs). No v1.0–v1.8 regression — every prior
  composed live proof still green: `live_acceptance_v1_3_composed`, `..._v1_4_composed_three_legs`,
  `..._v1_7_composed_four_legs`, `..._v1_8_composed_all_legs`, tainted-session confirm/deny — all
  `ok`. No cfg-linux-test-blindness (`cargo build --workspace --tests` clean in the container). ✅

## Adversarial code-trace (standing v1.9 per-phase discipline)

A fresh non-self, orchestrator-owned Fable-5 code-trace of the full ~2,500-line Phase-43 TCB diff
(`89cab31..HEAD`) traced all 8 briefed attack surfaces against live code and returned **APPROVE — 0
defects** (verifier read `http_write.rs`, the `http_request.rs`/executor/confirmation/server diffs,
the s43 test, and the design doc in full). Confirmed sound: the I0-escape is closed (explicit
CommitIrreversible arm + exhaustive Step-0.5 match, distinct schema id); the method gate resolves
from the taint-aware store and does not short-circuit the I2 Block; the taint is genuine (real
provenance, block-everything regression fails leg C twice); write-allowlist gate is before resolve
and genuinely distinct; credential/audit clean (reqwest 0.13.x Display no longer embeds the URL); the
confirm-release audit-gap is closed (precheck-before-burn, one shared `prepare_http_write`,
guard/match in sync); nothing minted; no cross-`.await` lock. Two NITs, neither a security defect and
neither actionable this phase:
- **NIT-1** — the non-2xx `eprintln` prints the server's response body (which a 401/407 could echo a
  URL into). Identical to the shipped `github_pr.rs:205` convention; DESIGN MINOR-4's log-scrub
  binds the git.push legs — if Phase 44 adds a log-scrub helper, apply it here too. **Tracked for
  Phase 44.**
- **NIT-2** — a post-commit network failure records `http_write_failed` though the remote effect may
  have landed. Inherent network ambiguity shared by every egress sink; the event name means "not
  confirmed succeeded."

## Notes

- HTTP-W-01's live mock-endpoint delivery ("mock records receipt") is the one sub-clause NOT proven
  in Phase 43 — it is dispatched to Phase 46 (LIVE-05/06), mirroring how v1.8's `http.request` GET
  sink built + differentially proved in its sink phase and composed live in Phase 40. This is a
  disclosed, tracked hand-off, not a silent gap: the differential *decision + dispatch* proof (clean
  reaches egress, tainted blocks, taint the sole variable) IS complete on real Linux here.
- Pinned-by-design (not a finding): in a Draft session a tainted-body write Blocks (confirmable)
  while a clean-body write is I0-Denied outright — the D-15 Block-precedence shared with
  email.send/github.pr since Phase 14; release requires an explicit human `caprun confirm`, so I0's
  "no auto-authorization" holds.

Full adversarial catalogue is captured in this session's report; the phase diff is `89cab31..HEAD`
(`crates/executor`, `crates/runtime-core`, `crates/brokerd`).
