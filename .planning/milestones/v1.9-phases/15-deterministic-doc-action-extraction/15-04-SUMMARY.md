---
phase: 15-deterministic-doc-action-extraction
plan: 04
subsystem: security
tags: [taint-tracking, provenance, confined-worker, planner, ipc-protocol, brokerd]

requires:
  - phase: 15-deterministic-doc-action-extraction
    provides: "mint_from_derivation (15-01), EXTRACT-02 audit-DAG gate + CONFIRM-02 fixture (15-02), live worker<->broker IPC wiring — WorkerClaim::DocFragment, ReportDerivedClaim/DerivedClaimReceived, broker dispatch arm (15-03)"
provides:
  - "Confined worker multi-fragment extraction + worker-side concat transform + ReportDerivedClaim send, wired into the live SubmitPlanNode path (EXTRACT-01 confined half complete)"
  - "plan_from_intent emitting to+subject+body PlanArgs, routed by call-site convention (not provenance) — RESEARCH Pitfall 2 closed"
  - "Three distinct UserTrusted handles per SendEmailSummary intent (recipient/subject/body) via three sequential mint_from_intent calls — finding #6's degenerate-handle fix"
  - "Live email hostile-block test (s9_live_email_hostile_block) proving a genuine two-fragment doc-derived recipient + tainted body yields a live two-anchor Block"
affects: [17-acceptance]

tech-stack:
  added: []
  patterns:
    - "Worker-side transform-before-mint: the confined worker applies concat_doc_fragments to its OWN already-extracted fragment values before any IPC round-trip, then obtains a FRESH derived ValueId via ReportDerivedClaim — never resolves a broker handle back to a literal and reuses it as the same handle (DESIGN-confirm-binding.md D-08)."
    - "Call-site-convention routing (not provenance): plan_from_intent places whichever handle the caller (worker) hands it into to/path via a shared Option<ValueId> slot — the planner structurally cannot see taint (PLAN-03), so 'routed by provenance' is a misnomer corrected in this plan (finding #7)."
    - "Multi-mint intent minting: ProvideIntent threads last_event_id/last_event_hash across N sequential mint_from_intent calls (1 for CreateFileFromReport, 3 for SendEmailSummary) so the causal DAG stays one linear chain, never forked."

key-files:
  created: []
  modified:
    - cli/caprun/src/worker.rs
    - cli/caprun/src/planner.rs
    - cli/caprun/tests/planner.rs
    - cli/caprun/tests/s9_live_block.rs
    - cli/caprun/tests/e2e.rs
    - crates/runtime-core/src/intent.rs
    - cli/caprun/src/main.rs
    - crates/brokerd/src/proto.rs
    - crates/brokerd/src/server.rs
    - crates/brokerd/tests/proto_claims.rs
    - crates/runtime-core/tests/intent_taint.rs

key-decisions:
  - "plan_from_intent's final signature is plan_from_intent(intent, intent_value_id, derived_recipient: Option<ValueId>, body: Option<ValueId>, trusted_subject_handle: ValueId, trusted_body_handle: ValueId) — six params, not the four literally quoted in the plan's must_haves bullet. The plan text is internally inconsistent: the must_haves signature bullet names only 4 params, but the SAME task's action text (and Task 3's own text: 'the worker reads all three and passes the trusted subject/body handles into plan_from_intent (Task 2's trusted_subject_handle/trusted_body_handle)') requires two additional always-present trusted handles that have no other route into a PLAN-03-compliant planner (the planner cannot read intent.subject/intent.body literals). Resolved by adding the two extra named params; CreateFileFromReport passes intent_value_id as an unused placeholder for both. Advisor consultation was attempted before committing to this reading but the tool was unavailable in this session; the resolution favors the more specific, load-bearing requirement (three genuinely distinct handles, finding #6, verified via a dedicated proto_claims.rs assertion) over the terser signature-summary bullet, and is called out explicitly here per the 'DESIGN DEVIATION REQUIRED' instruction — this is a plan-text ambiguity, not a deviation from any DESIGN doc."
  - "CreateFileFromReport reuses the SAME derived_recipient: Option<ValueId> slot for its tainted-path handle (previously a bespoke file_value_ids: &[ValueId] slice, narrowed to .first()). This is a like-for-like simplification per the plan's own instruction ('adjust only its call to the shared function... it still takes its file handles the same way') — CreateFileFromReport's behavior and test coverage are unchanged, only the parameter shape is unified."
  - "extract_body_fragment (the Body: marker scanner) is implemented as a private helper INSIDE cli/caprun/src/worker.rs, not in crates/brokerd/src/quarantine.rs, mirroring exactly the test-harness-only helper of the same name in extract_provenance_threading.rs (15-02) but now as PRODUCTION confined-worker code. quarantine.rs is not in this plan's files_modified list, and EXTRACT-01 only requires the extraction to run worker-side (inside the confined process) — it does not mandate which source file hosts the extractor function."
  - "s9_live_email_hostile_block is a NEW test added to tests/s9_live_block.rs, not explicitly named in the plan's task action text but required by the plan's own <verification> line ('live email-BLOCK and live email-ALLOW both present' — only the ALLOW half pre-existed). Uses a fixture mirroring crates/brokerd/tests/fixtures/hostile_doc.txt's CONFIRM-02 shape (Reply-To:/Domain:/Body: markers) driven through the REAL confined-worker + broker + executor stack (not the DB-alone dispatch_request harness Wave 2 used), proving the live (not just DB-alone) two-anchor Block."
  - "The plan's own dag_chain_integrity fix instructions (3-event -> 4-event) undercounted: Task 3's THREE sequential mint_from_intent calls for SendEmailSummary (recipient/subject/body, finding #6) each append their OWN intent_received event, making the benign chain 6 events (session_created, 3x intent_received, fd_granted, plan_node_evaluated), not 4. This was discovered EMPIRICALLY via Colima/Docker (not inferred) after the initial 4-event edit failed the Linux run; the test was corrected to assert 6 events with the full causal chain, then re-verified green under Docker."
  - "s9_live_clean_allow_path's own find_event_by_type('intent_received') LIMIT-1 query is unaffected by the 3-event mint change — it only asserts an intent_received event exists with empty taint (true regardless of which of the three it happens to resolve), not an exact count."

requirements-completed: [EXTRACT-01]

coverage:
  - id: D1
    description: "Confined worker extracts recipient-half doc fragments (Reply-To:/Domain:) + a Body: fragment worker-side, applies the concat transform to its own already-extracted values BEFORE any mint (never resolve-then-reuse), reports raw fragments via ReportClaims(DocFragment), and — only when BOTH recipient halves are present (finding #8) — reports the derived recipient via ReportDerivedClaim to obtain a fresh handle"
    requirement: "EXTRACT-01"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/s9_live_block.rs#s9_live_email_hostile_block (Linux-gated, verified green under Colima/Docker via scripts/mailpit-verify.sh)"
        status: pass
      - kind: integration
        ref: "cli/caprun/tests/planner.rs#plan_from_intent_to_routes_by_derived_recipient_presence"
        status: pass
    human_judgment: false
  - id: D2
    description: "The unconditional email early-exit-on-empty-value_ids is removed: a benign (fragment-free) SendEmailSummary always submits an all-UserTrusted to+subject+body plan node -> Allowed (CONTROL-01's clean half preserved, finding #4)"
    requirement: "EXTRACT-01"
    verification:
      - kind: e2e
        ref: "cli/caprun/tests/s9_live_block.rs#s9_live_clean_allow_path (Linux-gated, verified green under Docker)"
        status: pass
      - kind: e2e
        ref: "cli/caprun/tests/e2e.rs#dag_chain_integrity (Linux-gated, empirically verified 6-event benign chain under Docker)"
        status: pass
    human_judgment: false
  - id: D3
    description: "plan_from_intent emits to+subject+body PlanArgs for email.send, routed by call-site convention (not provenance, finding #7); the three previously-inverting tests (finding #5) are updated to the new named-Option shape, not deleted"
    requirement: "EXTRACT-01"
    verification:
      - kind: unit
        ref: "cli/caprun/tests/planner.rs#plan_from_intent_send_email_summary_emits_to_subject_body, plan_from_intent_to_routes_by_derived_recipient_presence, plan_from_intent_recipient_literal_is_not_visible_to_planner"
        status: pass
    human_judgment: false
  - id: D4
    description: "CaprunIntent::SendEmailSummary carries subject+body; the broker's ProvideIntent arm mints THREE distinct UserTrusted handles (recipient/subject/body) via sequential mint_from_intent calls, threading the causal chain linearly; a clean send is no longer degenerately to==subject==body==recipient (finding #6)"
    requirement: "EXTRACT-01"
    verification:
      - kind: integration
        ref: "crates/brokerd/tests/proto_claims.rs#provide_intent_dispatch_returns_intent_accepted_with_resolvable_handle (asserts subject/body handles distinct from each other AND from the recipient handle, and resolve to their own literals)"
        status: pass
    human_judgment: false

duration: ~2h10min
completed: 2026-07-08
status: complete
---

# Phase 15 Plan 04: Confined Multi-Fragment Extraction + Planner to+subject+body Summary

**The confined worker now extracts and concat-transforms multi-fragment recipients worker-side, reporting raw fragments and the derived recipient over the Plan-03 IPC; the planner emits `to`+`subject`+`body` routed by call-site convention, and the broker mints three genuinely distinct UserTrusted handles per email intent — closing EXTRACT-01's confined half and RESEARCH Pitfall 2.**

## Performance

- **Duration:** ~2h10min
- **Started:** 2026-07-08T~14:10Z
- **Completed:** 2026-07-08T~16:20Z
- **Tasks:** 3 (all auto)
- **Files modified:** 11 (9 in the plan's `files_modified` list + 2 Rule-3 incidental fixes)

## Accomplishments

- **Confined multi-fragment extraction** (`cli/caprun/src/worker.rs`): `extract_doc_fragments` (Reply-To:/Domain: markers) + a new worker-local `extract_body_fragment` (Body: marker, mirrors the 15-02 test-harness helper's shape as production code) run over the hostile bytes the worker already read via the passed fd. All extracted fragments are reported via `ReportClaims(WorkerClaim::DocFragment)`; the worker-side `concat_doc_fragments` transform runs on the worker's OWN extracted values (never a resolved broker literal) BEFORE any mint, and the transformed literal is reported via `ReportDerivedClaim` to obtain a FRESH derived `ValueId`.
- **finding #8's fork resolved exactly as specified**: `derived_recipient` is `Some` ONLY when BOTH recipient-half fragments were extracted (`doc_fragments.len() == 2`) — a lone address mention or a single marker never taints `to`.
- **finding #4 (CONTROL-01 clean half preserved)**: the unconditional `SendEmailSummary && value_ids.is_empty() -> exit 0` early-exit is REMOVED. A benign send now always builds and submits an all-UserTrusted plan node, which the executor Allows.
- **`plan_from_intent`** (`cli/caprun/src/planner.rs`) now emits three `PlanArg`s (`to`/`subject`/`body`) for `email.send`, closing RESEARCH Pitfall 2. Routing is by CALL-SITE CONVENTION (finding #7) — the doc-comment no longer claims "by provenance." `CreateFileFromReport` reuses the same `derived_recipient` slot for its tainted-path handle (unified calling convention, unchanged behavior).
- **Three distinct UserTrusted handles per email intent** (finding #6): `crates/brokerd/src/server.rs`'s `ProvideIntent` arm mints recipient/subject/body via three sequential `mint_from_intent` calls, chaining the causal DAG linearly; `BrokerResponse::IntentAccepted` gains additive `subject_value_id`/`body_value_id` fields (`None` for `CreateFileFromReport`).
- **New live hostile-block test** `s9_live_email_hostile_block` (`cli/caprun/tests/s9_live_block.rs`): a real `caprun` run over a doc carrying the genuine Reply-To:/Domain:/Body: structure blocks end to end with a two-anchor (`to`, `body`) `sink_blocked` event — the plan's own verification line's "live email-BLOCK ... present" requirement.
- **`e2e.rs::dag_chain_integrity`** updated for the DAG shape change: the benign chain is now 6 events (`session_created`, THREE `intent_received` — recipient/subject/body — `fd_granted`, `plan_node_evaluated`), empirically verified under Colima/Docker (see Deviations — the plan's own 3->4 event framing undercounted Task 3's additional two `intent_received` events).

## Task Commits

1. **Task 1: Confined multi-fragment extraction + worker-side transform + derived-claim report** — `e174793` (feat)
2. **Task 2: Planner emits to+subject+body for email.send, routed by call-site convention** — `a2478f2` (feat)
3. **Task 3: Trusted subject/body SOURCE — SendEmailSummary carries + broker mints subject+body UserTrusted handles** — `d22627c` (feat)

_Cross-task compile dependency (mirrors 15-03's precedent, documented in each commit message): commits 1 and 2 do not compile in isolation — worker.rs and planner.rs's tests reference the extended `IntentAccepted`/`CaprunIntent` shapes that land in commit 3. The workspace is fully green once all three commits are applied together (verified below)._

## Files Created/Modified

- `cli/caprun/src/worker.rs` — multi-fragment extraction, worker-side concat transform + `ReportDerivedClaim`, `extract_body_fragment` helper, early-exit removal, updated `plan_from_intent` call site with the trusted subject/body handles.
- `cli/caprun/src/planner.rs` — `plan_from_intent`'s new 6-param signature (named Options + two always-present trusted handles), `to`/`subject`/`body` PlanArgs, call-site-convention doc-comment.
- `cli/caprun/tests/planner.rs` — three inverting tests (finding #5) updated to the new shape; a shared `email_intent` helper constructing the new 3-field intent.
- `cli/caprun/tests/s9_live_block.rs` — doc comments corrected for the new benign/hostile framing; `HOSTILE_EMAIL_CONTENT` fixture + `s9_live_email_hostile_block` (new test).
- `cli/caprun/tests/e2e.rs` — `substrate_demo`/`dag_chain_integrity` doc-comments and assertions updated for the 6-event benign chain (BLOCKER fix, empirically verified).
- `crates/runtime-core/src/intent.rs` — `CaprunIntent::SendEmailSummary` gains `subject`/`body: String` fields.
- `cli/caprun/src/main.rs` — trusted default `subject`/`body` constants, no new CLI surface.
- `crates/brokerd/src/proto.rs` — `BrokerResponse::IntentAccepted` gains additive `subject_value_id`/`body_value_id: Option<ValueId>` fields.
- `crates/brokerd/src/server.rs` — `ProvideIntent` arm mints three sequential UserTrusted handles for `SendEmailSummary`, threading the causal chain.
- `crates/brokerd/tests/proto_claims.rs` (Rule 3, not in plan's files_modified) — three `IntentAccepted` construction/match sites updated for the additive fields; `provide_intent_dispatch_returns_intent_accepted_with_resolvable_handle`'s causal-chain assertion corrected (resolves by id, not `find_event_by_type`'s now-ambiguous LIMIT-1) and extended to assert the three handles are genuinely distinct.
- `crates/runtime-core/tests/intent_taint.rs` (Rule 3, not in plan's files_modified) — `SendEmailSummary` construction updated for the new fields.

## Decisions Made

See `key-decisions` in the frontmatter for the full rationale on each. Summary:

1. **`plan_from_intent`'s signature has 6 params, not the 4 literally named in the plan's must_haves bullet** — resolved a genuine internal inconsistency in the plan text (see Deviations).
2. **`CreateFileFromReport` reuses the `derived_recipient` slot** for its tainted-path handle — a like-for-like parameter-shape unification, no behavior change.
3. **`extract_body_fragment` lives in `worker.rs`**, not `quarantine.rs` — EXTRACT-01 requires worker-side execution, not a specific source file, and `quarantine.rs` is outside this plan's `files_modified`.
4. **`s9_live_email_hostile_block` added** — required by the plan's own `<verification>` line, not explicitly spelled out in the task action text.
5. **`dag_chain_integrity` corrected to 6 events (not 4)** after empirical Docker verification exposed the plan's undercounting of Task 3's additional `intent_received` events.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] `crates/brokerd/tests/proto_claims.rs` and `crates/runtime-core/tests/intent_taint.rs` updated for additive struct changes**

- **Found during:** Task 3 (compiling after the `IntentAccepted`/`CaprunIntent` extensions)
- **Issue:** Neither file is in this plan's `files_modified` list, but both construct/pattern-match the exact structs Task 3 extends: `proto_claims.rs` has three `IntentAccepted` sites (a round-trip test construction, a live-dispatch match, and a second live-dispatch match) and one `CaprunIntent::SendEmailSummary` construction that omit the new fields; `intent_taint.rs` constructs `SendEmailSummary` without `subject`/`body`. All four are genuine compile breaks (Rust struct-variant literals/patterns require all fields unless `..` is used).
- **Fix:** Added the missing fields/`..` at each site. `provide_intent_dispatch_returns_intent_accepted_with_resolvable_handle` additionally needed logical (not just compile) correction: its causal-chain assertion used `find_event_by_type("intent_received")` (LIMIT 1), which after Task 3 returns only the FIRST of three `intent_received` events, not the chain head. Rewrote to resolve `last_event_id` directly by id and assert `event_type == "intent_received"`, plus added an explicit assertion that exactly three `intent_received` events exist and that the subject/body handles are genuinely distinct (finding #6).
- **Files modified:** `crates/brokerd/tests/proto_claims.rs`, `crates/runtime-core/tests/intent_taint.rs`
- **Verification:** `cargo test -p brokerd --test proto_claims` (14/14 pass) and `cargo test -p runtime-core --test intent_taint` (10/10 pass), both included in the full green `cargo test --workspace` run below.
- **Committed in:** `d22627c` (Task 3 commit)

**2. [Rule 4-adjacent, plan-text ambiguity resolution — documented, not a DESIGN deviation] `plan_from_intent`'s signature has 6 params**

- **Found during:** Task 2 (implementing the new signature)
- **Issue:** The plan's must_haves bullet literally quotes a 4-param signature (`intent, intent_vid, derived_recipient, body`), but the immediately-following behavior text (and Task 2/Task 3's own action text) refers to `trusted_subject_handle`/`trusted_body_handle` as if already in scope as function parameters — an internal inconsistency, not a DESIGN-doc conflict. Given PLAN-03 (the planner never reads literals) and finding #6 (three GENUINELY DISTINCT handles, not derived from a shared one), there is no way to thread the two always-present trusted handles into `plan_from_intent` other than as additional parameters.
- **Fix:** Added `trusted_subject_handle: ValueId` and `trusted_body_handle: ValueId` as two additional mandatory params (not `Option`, since Task 3 always supplies them — falling back to `intent_value_id` only for `CreateFileFromReport`'s unused placeholders).
- **Why this doesn't weaken the security control:** The planner still never touches a literal or a taint label — the six parameters are all opaque `ValueId`s, and the function remains a pure, infallible, no-I/O mapping (PLAN-03 unchanged). This is a parameter-count resolution, not a security-boundary change.
- **Files modified:** `cli/caprun/src/planner.rs`, `cli/caprun/src/worker.rs`, `cli/caprun/tests/planner.rs`
- **Verification:** All `planner.rs` tests pass; the advisor tool was attempted for a second opinion before committing to this reading but was unavailable in this session (tool error), so the decision was made using the plan's own internal cross-references (Task 3's explicit mention of "Task 2's trusted_subject_handle/trusted_body_handle") as the tie-breaker over the terser must_haves bullet.
- **Committed in:** `a2478f2` (Task 2 commit)

**3. [Rule 3 - Blocking, discovered via empirical Linux verification] `e2e.rs::dag_chain_integrity` corrected to 6 events, not the plan-specified 4**

- **Found during:** Task 1/3 boundary — verifying the plan's own prescribed 4-event fix under Colima/Docker (per CLAUDE.md's mandatory Linux-only verification recipe) before declaring it done.
- **Issue:** The plan's finding #4 BLOCKER text prescribes updating `dag_chain_integrity` to expect exactly 4 events (adding one `plan_node_evaluated` to the pre-existing 3). This undercounts: Task 3's OWN change (three sequential `mint_from_intent` calls for `SendEmailSummary`, finding #6) appends THREE `intent_received` events, not one, making the true benign chain 6 events. Trusting the plan's arithmetic without running it would have shipped a test that fails on the very first real Linux CI run — exactly the "Mac-invisible Linux-gated" risk this plan's own context flagged as its highest-stakes area.
- **Fix:** Ran the actual test under `docker run --security-opt seccomp=unconfined ... rust:1 cargo test -p caprun --test e2e`, observed the real 6-event DAG print (`session_created -> intent_received x3 -> fd_granted -> plan_node_evaluated`), and corrected the assertion to match — then re-verified green under Docker (both standalone and via `scripts/mailpit-verify.sh`'s full-workspace run).
- **Files modified:** `cli/caprun/tests/e2e.rs`
- **Verification:** `docker run ... rust:1 cargo test -p caprun --test e2e` — 2/2 pass; full `scripts/mailpit-verify.sh` workspace run — all green (see below).
- **Committed in:** `e174793` (Task 1 commit)

---

**Total deviations:** 3 (2 Rule-3 blocking-compile/logic fixes, 1 plan-text-ambiguity resolution documented per the "DESIGN DEVIATION REQUIRED" instruction since it touches a must_haves bullet, though it is not a DESIGN-doc conflict).
**Impact on plan:** All three are necessary for correctness — none weaken any security invariant (PLAN-03, HARD-02/03, EXTRACT-01/02/03 all hold, verified below). No scope creep beyond what compiling/passing the plan's own stated success_criteria required.

## Issues Encountered

- The advisor tool was unavailable in this session ("The advisor tool is unavailable. Do not try to use it again.") at the exact point a second opinion on the `plan_from_intent` signature ambiguity would have been most valuable. Proceeded using the plan's own internal textual cross-references as the tie-breaker (see Deviation #2) and flagged the reasoning explicitly here for reviewer scrutiny.
- `e2e.rs::dag_chain_integrity`'s event count required empirical (not inferred) correction — see Deviation #3. This is precisely the class of risk the plan's own context section called out as "the ONLY plan where a real regression could hide invisibly on this Mac dev machine."

## User Setup Required

None — no external service configuration required.

## Next Phase Readiness

- **EXTRACT-01 confined half is complete.** The extractor + concat transform run entirely worker-side over hostile bytes, emitting only typed claims and plan nodes; a hostile doc yields a live two-anchor Block (`s9_live_email_hostile_block`); a benign doc still yields a live Allowed decision (`s9_live_clean_allow_path`, CONTROL-01's clean half preserved).
- **`cargo test --workspace --no-fail-fast` is FULLY GREEN** — verified twice: on macOS (`cargo build --workspace --tests` + `cargo test --workspace --no-fail-fast`, all green; cfg-linux test bodies compile but their assertion bodies are cfg-excluded on macOS per CLAUDE.md's documented expectation) and on Linux via `scripts/mailpit-verify.sh` (Colima/Docker + Mailpit sidecar) — every test, including all Linux-gated ones (`e2e.rs`, `s9_live_block.rs`'s 5 tests, `s9_acceptance.rs`, `confirm.rs`, `durable_anchor.rs`, `extract_provenance_threading.rs`, `phase5_dispatch.rs`, `email_smtp_acceptance.rs`, `confinement_integration.rs`, `api_spike.rs`, `uds_abstract_spike.rs`, `uds_ipc.rs`), passes. `./scripts/check-invariants.sh` passes all 3 gates.
- **This is the LAST plan in Phase 15** — no more waves. Phase 15 (Deterministic Doc->Action Extraction, EXTRACT-01) is complete pending orchestrator sign-off.
- **DEFERRED to Phase 16** (per this plan's own success_criteria, not re-litigated here): tightening `sink_schema.rs`'s `email.send` `required` set to `["to","subject","body"]` — the live planner already ALWAYS emits all three args, so the schema tightening is a hardening/negative-control follow-up, not a live-path gap.
- **DEFERRED to Phase 17 ACCEPT-01**: the full Linux end-to-end acceptance run (real SMTP A/B — clean send actually reaching Mailpit alongside the hostile block in the SAME process). This plan proves the live TRUSTED send path is reachable and Allowed (`plan_node_evaluated`, no `sink_blocked`) — it does NOT itself run the real-SMTP delivery A/B.

### Framing honesty (recorded per this plan's success_criteria, MAJOR-2 / finding R3)

- **CONTROL-01 / DOC-01**: the honest claim is "a send built from trusted intent is Allowed; a send whose args are doc-derived is Blocked" — NOT "same doc, taint flipped." The benign path (`s9_live_clean_allow_path`) sources `to`/`subject`/`body` from the TRUSTED INTENT, not from the doc — `CLEAN_PATH_CONTENT` contains a plain email address, but that address is never even extracted for `SendEmailSummary` anymore (the Phase 15 extractor only ever looks for `Reply-To:`/`Domain:` markers). The doc content is decorative on the benign path; any doc-derived value is structurally `ExternalUntrusted` and would block at a sensitive arg. Benign and hostile differ in the SOURCE/handle placed in the args (trusted-intent handle vs. doc-derived handle) — the CAUSE of the taint difference, not a taint flip over identical inputs.
- **`s9_live_clean_allow_path`**: asserts the Allowed DECISION only (`plan_node_evaluated` event, no `sink_blocked`) — NOT `sink_executed`/actual Mailpit delivery. "Actually sends to Mailpit" is a Phase 17 claim (ACCEPT-01's real-SMTP A/B); this plan proves only that the trusted send path is reachable and evaluates to Allowed.

### Threat-model notes recorded (finding R1, not blockers, folded in per this plan's instruction)

- **`ProvideIntent` trust boundary**: mints the worker-declared intent as `UserTrusted` rooted at `intent_received`, reachable via worker IPC. This is safe ONLY under the honest-worker-declares-intent-at-startup trust model — the intent is declared BEFORE any untrusted read. This is not introduced by Phase 15 and the derivation machinery does not touch it, but the boundary should be stated explicitly in any Phase 17 narration rather than implying the worker is adversarial at intent-declaration time.
- **`--seed-from-file` trust boundary**: mints file content as `UserTrusted`; only the I0 Draft-start + the `CommitIrreversible` deny prevent the effect. The mint boundary is NOT self-sufficient on its own — defense-in-depth working as designed, but should not be overclaimed as self-contained.

### Link-time-presence note for Phase 17 (finding #13, recorded per this plan's instruction)

The confined worker binary links the WHOLE `brokerd` crate — including `sinks::email_smtp` -> `lettre` -> `native-tls` — so an adversarial reader WILL observe TLS/SMTP code in the confined worker's link closure. This is link-time presence only: post-confinement the worker's sole egress is the broker UDS (Landlock deny-all + seccomp), it never opens a network socket, and the SMTP send happens broker-side after confirmation — the linked code is unreachable from the confined worker.

---
*Phase: 15-deterministic-doc-action-extraction*
*Completed: 2026-07-08*

## Self-Check: PASSED

All 11 claimed modified files found on disk (`cli/caprun/src/worker.rs`, `cli/caprun/src/planner.rs`, `cli/caprun/tests/planner.rs`, `cli/caprun/tests/s9_live_block.rs`, `cli/caprun/tests/e2e.rs`, `crates/runtime-core/src/intent.rs`, `cli/caprun/src/main.rs`, `crates/brokerd/src/proto.rs`, `crates/brokerd/src/server.rs`, `crates/brokerd/tests/proto_claims.rs`, `crates/runtime-core/tests/intent_taint.rs`) plus this SUMMARY.md; all 3 claimed task commit hashes (`e174793`, `a2478f2`, `d22627c`) verified present in `git log --oneline --all`.
