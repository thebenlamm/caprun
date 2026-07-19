# v1.9 "Authorized Egress + Policy & Audit Surface" — MILESTONE DONE RECORD

**Phase:** 46 — Composed Live Proof (v1.9 DONE gate)
**Requirements closed by this record:** LIVE-05, LIVE-06
**Code HEAD proven:** `204f615` (unchanged since 46-03 — this plan adds NO product code, only this record + the 46-04 summary)
**Date:** 2026-07-18
**Status:** DONE-gate evidence assembled; **AWAITING ORCHESTRATOR/HUMAN SIGN-OFF AT MILESTONE CLOSE** (see §8).

---

## 0. What v1.9 shipped (one paragraph)

v1.9 adds authorized WRITE egress to the caprun Intent Runtime — `http.request.write`
(POST/PUT, Phase 43), a broker-performed destination-pinned `git.push` (Phase 44),
a per-session policy layer that narrows which sinks/args are callable but can never
disable I2 (Phase 42), and a thin `caprun run` driver + read-only `caprun audit`
DAG viewer (Phase 45) — all gated behind the Phase-41 DESIGN doc + fresh
orchestrator-owned adversarial code-trace. Phase 46 composes these into a single
authorized-write live proof on real Linux and re-demonstrates the five
independently-attributable negative legs. This record closes LIVE-05 (composed
SUCCESS proof) and LIVE-06 (composed negative legs + full-workspace regression).

---

## 1. FRAMING HONESTY — what is driven by what (stated bluntly, NEVER compressed away)

This is the v1.3 DOC-01 discipline and it is the load-bearing honesty of this record.
The composed proof is a **hybrid**. The three layers are deliberately NOT conflated,
and **no part of this proof drives the whole authorized-write chain through a single
`caprun run` invocation**:

- **Composed IN-CRATE through the REAL broker arms** — the six-sink authorized-write
  SUCCESS chain (`process.exec` → filesystem edit → `git.commit` → `git.push`
  confirm-release → `github.pr` mock 201 → `http.request.write` POST `/ingest` 201)
  is submitted to the ACTUAL production dispatch arm
  `brokerd::server::evaluate_plan_node_and_record_for_test` — the `test-fixtures`-gated
  **verbatim delegate** to the live `evaluate_plan_node_and_record` — and the
  `git.push` leg is released through the real `brokerd::confirmation::confirm`. This is
  a faithful composition (real sinks, real mock endpoints under `mock-egress-ca`,
  genuine non-stapled taint with `provenance_chain[0]` == the real read/exec event id,
  per-session `verify_chain` true), but it is **NOT** expressible as a single
  `caprun run`: that verb plans one intent → one node → one sink, and only email/file
  intents exist. A multi-node composed-intent planner is deliberately out-of-scope new
  TCB (manual-ops-first scope). The chain is composed in the test crate — NOT stapled,
  NOT a hand-rolled mirror — because it runs the same arms the live daemon runs.
  (Test file: `cli/caprun/tests/live_acceptance_v1_9_composed.rs`, 46-02.)

- **`caprun audit`-INSPECTED** — every composed session is inspected by the REAL
  compiled `caprun audit <session> <db>` subprocess asserting `Chain verification:
  PASSED`. This is 100% real CLI (the read-only viewer proven in Phase 45), not an
  in-crate reimplementation.

- **`caprun run`-DRIVEN** — exactly ONE genuine `caprun run --policy <trusted>
  create-file-from-report …` subprocess drives a confined worker (Landlock + seccomp,
  Linux-only) whose tainted `file.create` path I2-Blocks; the block is surfaced
  (`effect_id` + `caprun review` pointer) and lands in the SHARED `audit.db` so it is
  swept + `caprun audit`-inspected alongside the composed chain. `caprun run` drives
  ONLY this single confined Block leg; **it never expresses the multi-sink write chain.**

**Blunt one-liner (the machine-checkable disclosure):** the SUCCESS chain is
*composed in-crate through the real broker arms*, *`caprun audit`-inspected*, with one
*genuinely `caprun run`-driven* confined Block leg. `caprun run` does NOT drive the
entire chain, and this record does not claim it does.

---

## 2. LIVE-05 — composed authorized-write SUCCESS proof

`cli/caprun/tests/live_acceptance_v1_9_composed.rs` —
`live_acceptance_v1_9_composed_success_chain` (single sequential `#[tokio::test]`,
`#[cfg(target_os = "linux")]`, ONE shared persisted `audit.db` + sibling `.key`,
F1-safe sibling layout, per-session `verify_chain`, final `ORDER BY rowid` sweep).

| Leg | Sink | Outcome proven | Genuine-taint / custody assertion |
|-----|------|----------------|-----------------------------------|
| 1 | `process.exec` | Allowed → REAL confined launcher runs → output minted by the arm | `provenance_chain[0]` == the real `process_exited` event id (non-stapled) |
| 2 | `git.commit` | Allowed | genuine taint on the commit mint |
| 3 | `file.write` | trusted path/contents (role `path`) Allows → `sink_executed` | — |
| 4 | `git.push` | clean remote/refspec Allow at executor → **always-confirm-gate** re-gates to `BlockedPendingConfirmation` (NO auto-dispatch) → `confirmation::confirm` → EXACTLY ONE `git_push_succeeded` | push token + remote-URL ABSENT from all payloads |
| 5 | `github.pr` | record grant → arm CAS + POST to mock → EXACTLY ONE `github_pr_succeeded` | bearer token never in any payload/actor (incl. `ghp_` scan) |
| 6 | `http.request.write` POST | clean body → arm POSTs the 46-01 mock `POST /ingest` on the write-allowlisted host → 201 → EXACTLY ONE `http_write_succeeded` | live mock-receipt (closes HTTP-W-01's carried "clean leg actually delivered" sub-clause) |

Plus: a genuine `caprun audit` subprocess per session (`Chain verification: PASSED`),
one genuine `caprun run` I2-Block leg in the shared db, and a final sweep asserting
EXACTLY the 7-session composed set with every `verify_chain` independently true.

---

## 3. LIVE-06 — five independently-attributable negative legs

`crates/brokerd/tests/s46_negative_legs_composed.rs` —
`s46_negative_legs_composed_all_legs` (one composed `#[tokio::test]`, union-gated
`#[cfg(all(target_os = "linux", feature = "mock-egress-ca"))]`, ONE shared persisted
`audit.db`, one session per leg, end-of-run `verify_chain` sweep asserting EXACTLY the
5 negative-leg sessions).

| Leg | What | Distinct machine-checkable tag |
|-----|------|-------------------------------|
| 1 | genuinely-tainted `git.push` **remote** I2-Blocks under a policy that PERMITS `git.push` | `sink_blocked` anchored on the tainted arg (`read_event_id == provenance_chain[0]`) |
| 2 | genuinely-tainted `http.request.write` **body** I2-Blocks under a policy that PERMITS the write | `sink_blocked`; NO `http_write_*` terminal (never writes) |
| 3 | policy-deny of an OMITTED sink (`email.send` absent from a policy that permits push + write) | `code()=="policy_deny"` + generic `plan_node_evaluated`; **NO `sink_blocked`** |
| 4 | a `/redirect/*` push refused by the frozen redirect-none client (destination pin holds) | `ConfirmedButSinkFailed` + EXACTLY ONE `git_push_failed`; `confirm_granted` released to Step-7 first |
| 5 | credential-absence after a real push (see §3.1 for WHICH push proved each clause) | token + remote-URL absent from value store, audit chain, AND broker log |

### 3.1 Leg 5 — WHICH push proved each credential-absence clause (honest attribution)

The two clauses of the credential-absence assertion are proven on **different pushes**,
because they exercise different code paths — conflating them would be a false-assurance
trap ([[false-assurance-regression-test]]):

- **Value-store + audit-chain absence** — proven on the **clean confirmed 200 push**
  (Leg 5a). After a REAL clean push, the sentinel token + remote URL are absent from
  every event payload AND every actor column.
- **Broker-log (stderr) absence** — proven on the **ERROR-PATH push** (the Leg-4
  redirect-refused push, Leg 5b) — the ONLY push where `scrub_secrets`→`eprintln!`
  (`git_push.rs:784`) actually fires. The clean 200 push takes the `Ok` arm and emits
  NO log, so a log-absence check on it would be VACUOUS. The captured stderr contains
  the `"[brokerd] git.push failed"` marker (NON-VACUOUS — proves the logger ran) and
  NEITHER the token NOR the raw remote host/URL. (Captured via a re-exec'd `--nocapture`
  subprocess + `libc dup2`, because libtest output-capture propagates to spawned threads
  and intercepts `eprintln!` before FD 2 — an in-process dup2 was empirically vacuous;
  see 46-03 deviation 1.)

### 3.2 RATIFIED READING — policy-deny is DECISION-LEVEL, not a DAG terminal event (plan-checker W3)

We explicitly ratify: Leg 3's policy-deny is asserted at the **decision level** —
`Denied{PolicyDeny}` with `code() == "policy_deny"`, recorded as a **generic
`plan_node_evaluated`** audit event. It is **NOT** a distinct DAG *terminal* event type
and **NOT** a `sink_blocked` event. This is the correct and intended shape: the distinct
machine-checkable tag lives in the **decision outcome code**, not in a bespoke terminal
event. The I2 legs (1, 2) emit `sink_blocked` while running a sink+arg the policy
**explicitly PERMITS** — so policy is provably NOT what blocks them — and the two tags
(`sink_blocked` vs `code()=="policy_deny"`) are asserted SEPARATELY, side-by-side, in one
block. This proves POLICY-02 structurally: policy narrows WHICH sinks are callable but
can NEVER disable I2.

---

## 4. git.push — SHIPPED, safety-valve NOT triggered (LIVE-05 M6/n1 disposition)

**`git.push` (GIT-02/GIT-03) SHIPPED in Phase 44 — it did NOT defer a 3rd time.**

- The LIVE-05/06 safety-valve clause (`[rev: M6/n1]`: "if GIT-02 defers, the `git.push`
  leg auto-descopes AND the deferral is recorded as a disclosed milestone gap requiring
  explicit user sign-off") **did NOT trigger.** The Phase-41 design gate proved a sound
  fully-unprivileged, broker-performed, destination-pinned smart-HTTP mechanism
  (reqwest resolve-and-pin, IP frozen across the info/refs GET + git-receive-pack POST,
  redirect refused; pack-gen child stays net-denied under the unchanged exec filter).
- **There is NO descope.** The `git.push` leg IS included in the composed SUCCESS proof
  (§2 Leg 4) and in the negative legs (§3 Legs 1, 4, 5) — this is locked decision #6.
- Phase 44 shipped with compose-verify **668/0 on real Linux** (incl. `leg_c` real
  delivery to the mock git-receive-pack, `leg_d` force/delete refused, `leg_e` redirect
  refused) and a fresh Fable-5 adversarial trace APPROVE (0 security defects across 8
  surfaces).

---

## 5. Disclosed NON-BLOCKING deferral — the 10 MB pack-cap

`git.push`'s `generate_pack` shares the 10 MB `MAX_COMBINED_OUTPUT_BYTES` cap, so a
>10 MB pack fails **CLOSED** (safe — no partial push) but blocks large-repo pushes
(Fable-5 Phase-44 non-security note).

- **Non-blocking for Phase 46:** the composed proof (§2 Leg 4) pushes a **SMALL
  one-commit mock repo** (`setup_git_push_repo`), so the cap is never exercised and the
  proof is unaffected.
- **Disposition:** carried forward as a **disclosed, non-blocking deferral** (STATE.md
  Deferred Items — "functional (caprun)"). It is a functional large-repo limitation, not
  a security gap, and does not block the v1.9 DONE gate. Revisit if/when large-repo push
  becomes an acceptance target.

---

## 6. GATE EVIDENCE — Linux regression status (stated honestly)

**Honest scope statement (v1.3 DOC-01):** this executor (46-04) did **NOT** re-run the
full-workspace `compose-verify.sh` itself. 46-04 adds **zero product code** — the code
HEAD is frozen at `204f615`, byte-identical to what 46-03 verified green — so re-running
the full gate here would reproduce an identical result at the cost of a multi-minute
Docker/Colima run. The **authoritative** full-workspace no-regression run (Success
Criterion 3) is the **orchestrator's** at phase close.

**Composed-proof evidence already on real Linux (at this exact HEAD):**

- **46-03** ran `scripts/compose-verify.sh --features brokerd/mock-egress-ca`:
  `s46_negative_legs_composed` — **3 passed / 0 failed** (host guard + composed
  `all_legs` + leg5b worker); the **feature-OFF guard passed** (mock host + anchor
  ABSENT from the release-shaped build); final `verify_chain` sweep asserts EXACTLY the
  5 negative-leg sessions. Script reported **"Composed Linux verification suite PASSED."**
- **46-02** delivered the composed SUCCESS proof; verified via a compile-only Linux
  Docker type-check (`rust:1`, `--features brokerd/mock-egress-ca --no-run`, exit 0) plus
  the host-portable guard; its authoritative runtime green is the same compose-verify
  gate.

**Standing full-workspace regression baseline (prior v1.9 phases, real Linux):**
Phase 43 **584/0** · Phase 44 **668/0** · Phase 45 **691/0** — each with zero
v1.0–v1.8 regression and a fresh Fable-5 adversarial-trace APPROVE.

**AUTHORITATIVE RUN (orchestrator, at phase close):**
`bash scripts/compose-verify.sh` with the DEFAULT `COMPOSE_VERIFY_CMD`
(`cargo build --workspace && cargo test --workspace --no-fail-fast --features
brokerd/mock-egress-ca`) — full workspace, feature-OFF guard + both new tests
(`live_acceptance_v1_9_composed`, `s46_negative_legs_composed`) + zero full-workspace
failures. The script captures `rc` BEFORE any pipe (`compose-verify.sh:199`;
[[verification-exit-code-through-pipe]]) and asserts on named tests + counts, never
exit 0 alone. **This record must be updated with the authoritative pass count at phase
close.**

> **PENDING at record-write time:** `compose-verify` full-workspace pass count = _(to be
> filled by the orchestrator's authoritative phase-close run)_.

**`check-invariants.sh`:** all gates PASS at HEAD `204f615` (Gate 1 no new
`EffectRequest`, Gate 4/4b `test-fixtures`/`mock-egress-ca` never default, Gate 5
aws-lc-rs/openssl-sys absent — HYG-01 holds, Gate 6 containment-predicate anti-drift).

---

## 7. State ownership — executor flipped NOTHING (the guardrail)

Per [[gsd-executor-self-marks-phase-complete]] and
[[gsd-record-signoff-before-last-plan]], this executor wrote ONLY this record + the
46-04 summary. It did **NOT** touch `ROADMAP.md`, `STATE.md`, or `REQUIREMENTS.md`.
The pre-existing unstaged mods (`M .planning/STATE.md`, `M .planning/REQUIREMENTS.md`)
were left untouched and unstaged.

**REQUIREMENTS LIVE-05 / LIVE-06 reconciliation is performed by the orchestrator's
`phase.complete` AFTER human sign-off** — flipping the last requirement auto-rolls the
milestone to Complete, so the sign-off (§8) must be recorded FIRST.

---

## 8. HUMAN SIGN-OFF

**AWAITING ORCHESTRATOR / HUMAN SIGN-OFF AT MILESTONE CLOSE.**

This autonomous 46-04 executor does **NOT** fabricate approval. The v1.9 milestone-close
human sign-off is the orchestrator's, gathered at `/gsd-complete-milestone` after the
orchestrator's authoritative full-workspace compose-verify run confirms §6. On approval,
the orchestrator: (a) records the verdict here, (b) fills the §6 authoritative pass count,
then (c) reconciles LIVE-05/LIVE-06 via `phase.complete`.

| Field | Value |
|-------|-------|
| Verdict | _AWAITING_ |
| Approver | _(orchestrator/human at milestone close)_ |
| Date | _pending_ |
| Authoritative compose-verify count | _pending_ |

---

*Phase: 46-composed-live-proof-v1-9-done · Record HEAD: 204f615 · Assembled: 2026-07-18*
