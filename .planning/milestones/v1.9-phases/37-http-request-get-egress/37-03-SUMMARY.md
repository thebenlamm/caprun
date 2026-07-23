---
phase: 37-http-request-get-egress
plan: 03
subsystem: brokerd
tags: [http, taint, mint, quarantine, i1-demotion, anti-staple, gate3]
requires:
  - "37-01 (runtime-core TaintLabel::HttpRaw + executor http.request schema/sensitivity rows)"
  - "37-02 (brokerd::sinks::http_request::invoke_http_get — SSRF-defended GET egress)"
provides:
  - "brokerd::quarantine::mint_from_http — SOLE broker HTTP-taint mint site: event-first http_response_received → non-stapled body mint [ExternalUntrusted, HttpRaw] → atomic in-conn I1 demotion"
  - "server.rs Allowed http.request GET dispatch: resolve url → invoke_http_get → mint_from_http → causal-head advance to the demoted event → shared-cell Draft propagation"
  - "check-invariants.sh Gate 3 fifth mint token (mint_from_http restricted to quarantine.rs + server.rs)"
affects:
  - "Phase 38 (github.pr) reuses the inbound-mint mechanism"
  - "Phase 40 composed live proof (real GET → mint → demote on Linux)"
tech-stack:
  added: []
  patterns:
    - "event-first → mint → atomic in-conn demotion (mirrors mint_from_read Steps 1-4)"
    - "demotion-signal return from evaluate_plan_node_and_record (fn holds only &SessionStatus) → caller writes the shared Arc<Mutex<SessionStatus>> cell (RequestFd exemplar)"
    - "async fetch OUTSIDE the conn lock; lock held only for the synchronous mint"
key-files:
  created:
    - crates/brokerd/tests/s37_http_request.rs
  modified:
    - crates/brokerd/src/quarantine.rs
    - crates/brokerd/src/server.rs
    - scripts/check-invariants.sh
decisions:
  - "Sensitive slot for the anti-staple test = git.commit `message` (content-sensitive, single required arg, schema-minimal); SessionStatus::Active passed to isolate the Block as taint-driven (I2), not draft-only (I1)."
  - "evaluate_plan_node_and_record return widened to a 3-tuple (decision, output_value_id, session_demoted:bool) rather than threading the Arc into it — the fn only reads &SessionStatus; both call sites (planner branch + dispatch_request SubmitPlanNode) propagate the signal to the shared cell."
  - "Task 1 kept the strict RED/GREEN split (test+stub RED, real body+Gate-3 GREEN) so the Gate-3 extension lands in the SAME commit as the real mint (must-have)."
metrics:
  duration: "~15 min"
  completed: "2026-07-18"
  tasks: 3
  commits: 4
  files_touched: 4
status: complete
---

# Phase 37 Plan 03: mint_from_http + http.request GET dispatch (anti-staple inbound taint) Summary

Delivered the one genuinely-new mechanism of the milestone — `mint_from_http` — and wired the `http.request` GET end to end: on an Allowed `Observe` GET the broker fetches (Plan 02), appends a real `http_response_received` audit Event FIRST, mints the response body untrusted-on-arrival rooted on that event (`provenance_chain[0] == event id`, non-stapled), then atomically demotes the session to draft-only (I1) both durably and in the shared in-memory cell. A fetched value routed into a sensitive sink arg deterministically Blocks on that genuine DAG edge.

## What was built

**Task 1 — `mint_from_http` + Gate-3 extension (quarantine.rs, check-invariants.sh).** New `pub fn mint_from_http` beside `mint_from_read`/`mint_from_exec`, mirroring `mint_from_read`'s Steps 1-4 near-verbatim: (1) build an `http_response_received` Event (actor `http-egress`, taint `[ExternalUntrusted, HttpRaw]`, `parent_id` threading the causal head); (2) `append_event`; (3) `store.mint(body, [ExternalUntrusted, HttpRaw], [event_id], Some("http_response"))` — the non-stapled anchor; (4) the SAME atomic in-`conn` I1 demotion (`update_session_status(Draft)` + `session_demoted` Event parented on the response event, one lock — avoids the documented parent-forking bug). Returns the 5-tuple `(event_id, event_hash, value_id, demoted_event_id, demoted_hash)`. Carries `mint_from_read`'s "two separate graphs (value-lineage anchor vs causal parent_id) — never conflated" warning. Unit test `mint_from_http_anchor_identity` asserts the anchor identity, DAG lookup, taint pair, origin_role, and post-mint Draft. In the SAME commit: `check-invariants.sh` Gate 3 gained a fifth `check_mint_token "mint_from_http("` restricted to quarantine.rs + server.rs (DESIGN §10), and both the Gate-3 header + PASS strings name the new mint.

**Task 2 — Allowed http.request GET dispatch (server.rs).** New Allowed arm in `evaluate_plan_node_and_record`: resolves the `url` literal from the broker-owned store (fail-closed if unresolved), awaits `invoke_http_get(url)` OUTSIDE any conn lock. On `Ok(body)`: locks `conn` only for the synchronous `mint_from_http`, advances `*last_event_id`/`*last_event_hash` to the **demoted** event (linear-chain discipline, not the response event), sets `output_value_id`, and raises a `session_demoted` signal. On `Err` (non-allowlisted host / SSRF-range / redirect / transport — DESIGN §8): broker-side Deny via `eprintln` (never the hash chain), no mint, no demotion, no output. Raw body + url never enter any audit payload. The fn holds only `&SessionStatus`, so it returns the demotion signal via a widened 3-tuple; both call sites (planner branch + `dispatch_request` SubmitPlanNode) propagate it to the shared `Arc<Mutex<SessionStatus>>` cell with a monotonic Draft write (RequestFd exemplar) so a later same-connection node observes Draft.

**Task 3 — host-portable integration tests (crates/brokerd/tests/s37_http_request.rs).** No real socket: mints directly via `quarantine::mint_from_http` against an in-memory audit db + ValueStore. `http_fetched_value_blocks_in_sensitive_slot_non_stapled` proves the §3.5 anti-staple pair (route into git.commit `message` → `BlockedPendingConfirmation` AND `provenance_chain[0] == http_response_received` id present in the DAG; `verify_chain` intact). `http_response_demotes_session_to_draft` proves the durable Draft + the `session_demoted`→`http_response_received` causal edge.

## Verification results (real counts, macOS host)

- `cargo build --workspace` — Finished, 0 warnings, 0 errors.
- `cargo test -p brokerd` — brokerd lib **141 passed / 0 failed** (includes `mint_from_http_anchor_identity`); `--test s37_http_request` **2 passed / 0 failed**; all other host-portable integration binaries green (audit_dag 2, durable_anchor 5, extract_provenance_threading 4, harden01 3, phase5_dispatch 6, planner_capability_split 2, planner_reduced_signal 1, proto_claims 14, s9_acceptance 5). **0 failures across all binaries.** Linux-gated binaries (email_smtp_acceptance, git_commit_spawn, process_exec_spawn, replay_cas, two_connection_intent_bypass, uds_*) show 0 tests on macOS — expected per CLAUDE.md `#[cfg(target_os="linux")]` gating, not a gap.
- `./scripts/check-invariants.sh` — **exit 0**, all gates PASS. Gate 3 line now enforces `mint_from_read / mint_from_derivation / mint_from_exec / mint_from_http / .mint()`; the new `mint_from_http(` token is present (check-invariants.sh:137) and green.

## Threat mitigations realized

- **T-37-02 (EoP, exfil/injection):** `mint_from_http` stamps `[ExternalUntrusted, HttpRaw]` with genuine non-stapled provenance; anti-staple test proves the downstream Block rides the real chain.
- **T-37-05 (Spoofing, I1 hole):** atomic in-conn Draft UPDATE + `session_demoted` in `mint_from_http`, AND the shared in-memory cell written Draft in both server dispatch call sites.
- **T-37-06 (Tampering, mint outside loci):** Gate-3 fifth token added in the same commit as the mint.
- **T-37-07 (Info disclosure):** raw body/url → `eprintln` only, never the hash chain.

## Deviations from Plan

**1. [Rule 3 — Blocking] `evaluate_plan_node_and_record` return type widened to a 3-tuple.** The plan anticipated this ("thread the Arc into this path or return a demotion signal to the handle-holder if the function signature does not already carry it"). The fn holds only `session_status: &SessionStatus` (read-only), so it cannot write the shared cell itself — it now returns `(decision, output_value_id, session_demoted: bool)`. Both call sites (planner branch server.rs:618, `dispatch_request` SubmitPlanNode server.rs:1775) consume the signal and perform the monotonic Draft write on the shared `Arc<Mutex<SessionStatus>>`. Committed in 776d254.

No other deviations. No auth gates. No architectural changes.

## TDD Gate Compliance

Task 1 followed strict RED (`test(37-03)` 52bf90a — failing anchor test + `unimplemented!()` stub, crate still compiles per this repo's 37-02 pattern) → GREEN (`feat(37-03)` 4af39d4 — real body + Gate-3 extension in ONE commit, satisfying the "Gate-3 same commit as mint" must-have). Task 2 is `feat` (776d254). Task 3's tests verify already-built behavior, so they pass on first run (no meaningful RED — the mechanism exists from Tasks 1/2); committed as `test(37-03)` 33389cb.

## Known Stubs

None. The macOS `invoke_http_get` no-op stub is Plan 02's (returns `Err`, live GET deferred to Phase 40) and is out of this plan's scope; this plan's mint/demotion/anti-staple logic is fully wired and host-portably tested.

## Self-Check: PASSED

- crates/brokerd/src/quarantine.rs — FOUND (mint_from_http present)
- crates/brokerd/src/server.rs — FOUND (http.request arm present)
- scripts/check-invariants.sh — FOUND (Gate-3 token present, exit 0)
- crates/brokerd/tests/s37_http_request.rs — FOUND (2 tests pass)
- Commits 52bf90a, 4af39d4, 776d254, 33389cb — all present in git log.
