---
phase: 45
status: passed
verified_by: orchestrator
date: 2026-07-18
---

# Phase 45 Verification — Thin CLI/SDK + Read-Only Audit-DAG Viewer (SDK-01, U1)

**Verdict: PASSED.** Proven green on real Linux via an INDEPENDENT orchestrator re-run of
`compose-verify.sh` (**COMPOSE_VERIFY_EXIT=0, 691 passed / 0 failed**, "Composed Linux verification
suite PASSED"). The fresh non-self Fable-5 adversarial code-trace of the ~1,900-line diff returned
**APPROVE** — M7 anti-laundering and the viewer's fail-closed MAC-key custody both sound under trace
— and surfaced one genuine decision-surface MINOR (a pre-existing Trojan-Source gap in the shared
neutralizer) which was FIXED this phase (commit `e31257a`) and re-verified. 4 plans executed
sequentially on `main`.

## Goal-backward check

Phase 45's goal: an operator can define an intent, point it at a workspace with a trusted policy,
run it end-to-end, and INSPECT the proof — the design-partner-runnable trust surface, ON the
acceptance critical path (Phase 46 LIVE-05 drives + inspects the composed proof through these verbs).

| Requirement | Evidence | Status |
|-------------|----------|--------|
| SDK-01 (run entrypoint + policy binding + Block surfacing + M7 no-laundering) | `caprun run <intent-kind> <intent-param> <workspace-file> [--policy <path>] [audit-db-path]` runs an intent end-to-end against the broker; the bare-positional form + confirm/deny/grant/review verbs are UNCHANGED (extends, does not replace). `--policy` feeds the SAME `bind_policy(Option<&Path>, workspace_root)` call the `CAPRUN_POLICY` env feeds (the POLICY-03 F1 `refuse_if_beneath_workspace` enforcement point; env is the fallback; none → `broker_default()`). On an I2 Block the parent queries `pending_confirmations` and surfaces the blocked `effect_id` + the `caprun review`/confirm/deny pointer (Matt #2 — the design-partner-runnable loop). **M7 anti-laundering (a TCB change):** a FILE-DERIVED `--seed-from-file` literal is minted TAINTED (`ExternalUntrusted`) via the EXISTING broker-side `mint_from_read` site in the ProvideIntent arm (claim_type email_address→EmailRaw / relative_path→PathRaw, + a real `file_read` event + session-demote), so it I2-Blocks in the sink arg; operator-typed literals stay TRUSTED via `mint_from_intent`, DISJOINT; the file-derived signal is threaded per-literal through the `ProvideIntent` proto + worker.rs. NO second mint site (Gate 3 — reuses `mint_from_read`). Proven NON-VACUOUS (forcing the laundering path made the leg fail with `[UserTrusted]`; the real path yields `[ExternalUntrusted, EmailRaw]` rooted on a genuine `file_read` event, verify_chain true). Tests: `s45_sdk_run_surface`, `s45_cli_viewer_acceptance`. | ✅ Complete |
| U1 / VIEW-01 (read-only viewer, fail-closed key, universal neutralization) | A read-only `caprun audit <session>` verb renders the session's events/decisions + the `verify_chain` verdict (no web UI). Uses a load-ONLY `load_existing_key` (F1 `refuse_if_beneath_workspace` + read-or-ERROR, NEVER create) and REFUSES a `:memory:` DB — an absent key makes the viewer REFUSE a verify_chain verdict (fails CLOSED), never a fresh/`:memory:` key; the key loads BEFORE any verdict is printed. Opens the DB `SQLITE_OPEN_READ_ONLY`, mints/appends NOTHING. Every displayed literal is neutralized via the shared `brokerd::display::neutralize_control_chars` (an actor's `\x1b[2K` renders as visible `\x1b`, no raw ESC in stdout). Tests: `audit_viewer` (render / absent-key-fail-closed / `:memory:`-refused / neutralize), `s45_cli_viewer_acceptance`. | ✅ Complete |

## Hard-constraint checks

- **No raw `EffectRequest`** (Gate 1), **no new mint site** (Gate 3 byte-identical — M7 reuses
  `mint_from_read`, the viewer mints nothing), **no new crate** (Gate 5). `check-invariants.sh` all
  gates PASS. ✅
- **SDK-01 "extends, does not replace":** the shipped e2e suite (passes no verb) stays green; the
  confirm/deny/grant/review verbs are untouched. ✅
- **Linux (authoritative, independent orchestrator re-run):** `compose-verify.sh` — **691 passed /
  0 failed, exit 0**. The genuine end-to-end acceptance (`caprun run` over a hostile doc →
  I2-Block → parent surfaces effect_id + review pointer → `caprun review` shows the verbatim
  literal + provenance → `caprun audit` renders the DAG + `Chain verification: PASSED`, loop closed
  by ONE `pending_confirmations` row) runs green, plus the U1 negatives. No v1.0–v1.8 regression
  (prior composed proofs green). ✅

## Adversarial code-trace (standing v1.9 per-phase discipline)

A fresh non-self, orchestrator-owned Fable-5 code-trace of the full Phase-45 diff (`25f41d9..HEAD`)
traced all 6 briefed surfaces against live code and returned **APPROVE** (no BLOCKER/MAJOR):
- **M7 laundering escape — NO ESCAPE.** Defended by TWO independent controls: (a) the session
  status is set by *trusted* `caprun main` from the CLI's own `seed_provenance` (`--seed-from-file`
  → Draft), NOT the worker's flag — so a worker lying about the provenance flag cannot force a live
  effect; and (b) both reachable sinks are `CommitIrreversible`, which a Draft session hard-denies.
  Honest flag → I2 Block; lying flag → Draft Deny. Either way the file-derived value never reaches a
  live effect; the anchor is genuine (roots on a real `file_read` event), Gate 3 holds.
- **Viewer fail-closed — HOLDS.** `load_existing_key` never creates, refuses `:memory:`, hard-errors
  on an absent key BEFORE any verdict; DB opened read-only; mints/appends nothing.
- **Neutralization — complete for what's displayed;** verb dispatch, `--policy` F1 binding, and the
  parameterized audit read path all clean.

**MINOR (pre-existing, FIXED this phase — `e31257a`):** the shared `neutralize_control_chars` gated
only on `char::is_control()` (Unicode category Cc), missing the **Trojan-Source class**
(CVE-2021-42574) — BiDi overrides/embeddings/isolates (`U+202A..U+202E`, `U+2066..U+2069`) and
zero-width joiners (`U+200B..U+200F`, `U+FEFF`), all category Cf. This is a **decision-surface
spoof**: the same shared fn renders TAINTED arg literals on the **git.push confirm prompt**
(`render_git_push_payload_summary` / `render_block_display`), so a tainted remote/refspec carrying
`U+202E` reached the human's confirm terminal visually reversed — defeating the confirm-binding
guarantee. Pre-existing (Phase 45 only *moved* the fn), but the confirm prompt is a live surface and
U1/M3 explicitly restates the guarantee, so fixed here: escape the format-spoof set alongside the
control chars, + a BiDi/zero-width test + a non-spoof-preservation test. The `confirmation.rs`
anti-drift test confirms the git.push confirm path picks up the hardening automatically. Re-verified:
display tests green, check-invariants exit 0, post-fix compose-verify 691/0.

## Process notes

- The plan-checker caught the M7 mechanism as a BLOCKER before execution (the `--seed-from-file`
  laundering path was verified in shipped code); the fix (pin the broker-side `mint_from_read`
  mechanism, thread provenance through ProvideIntent, delete the "seed-only" fallback) was folded in
  and re-verified before executing 45-01. The executor then proved M7 non-vacuous.
- All four gates (compose-verify + Fable-5) plus the neutralizer re-verify passed with no regression.

## Notes

- Phase 45 is on the acceptance critical path: Phase 46 (LIVE-05/06) drives the composed
  exec→fs→git.commit→git.push→github.pr + http POST proof through `caprun run` and inspects it
  through `caprun audit`, with the 5 independently-attributable negative legs.
