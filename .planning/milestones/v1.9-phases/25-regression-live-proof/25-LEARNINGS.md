---
phase: 25
phase_name: "Regression & Live Proof"
project: "caprun (AgentOS)"
generated: "2026-07-12"
counts:
  decisions: 5
  lessons: 4
  patterns: 4
  surprises: 3
missing_artifacts: []
---

# Phase 25 Learnings: Regression & Live Proof

## Decisions

### The T2-08 live gate was run directly by the orchestrator, not delegated to a subagent
The milestone-close `bash scripts/mailpit-verify.sh` re-run was executed by the orchestrator
itself and the true exit code captured on the line immediately after the invocation, before any
pipe.

**Rationale:** T2-08's entire purpose is a non-laundered, independent re-run. A subagent relaying
"it passed" reintroduces exactly the indirection the gate is designed to distrust. Running it
directly also gives the orchestrator the real log to assert on (sentinel + named counts + held-out
test presence) and to write the durable evidence record. Mirrors the v1.3 coordinator-gate precedent.
**Source:** 25-03-SUMMARY.md (key-decisions), 25-03-PLAN.md

### The held-out T2-06 test is in-process and NOT `#[cfg(target_os = "linux")]`-gated
`slot_type_binding_swapped_subject_recipient_denies` + its Allowed control live in
`crates/brokerd/tests/s9_acceptance.rs` with no cfg gate, driving `mint_from_intent` +
`submit_plan_node` in-process.

**Rationale:** The behavior is identical on Mac and Linux and needs no kernel — cfg-gating would
only slow feedback and gain nothing. Because the file is non-gated, it still runs on real Linux
under `mailpit-verify.sh` at the T2-08 gate, satisfying DESIGN §9's actual intent ("proven on real
Linux") while DESIGN §9's literal "must live behind cfg(linux)" line is explicitly labelled
"informative — not part of the gate." A deliberate, documented deviation.
**Source:** 25-01-PLAN.md (objective, DESIGN §9 note)

### Human sign-off must be recorded into the artifact, and phase.complete stays gated on the verifier
The blocking `autonomous:false` checkpoint required explicit human "approved"; that approval is
durably written into 25-03-SUMMARY.md, and `phase.complete` refuses to run until the verifier
returns `passed`.

**Rationale:** The v1.5 DONE gate is human-only regardless of auto_advance. Keeping `phase.complete`
gated on an independent verdict is what caught the traceability/sign-off recording lag before the
milestone was falsely closed.
**Source:** 25-03-PLAN.md (Task 2), 25-VERIFICATION.md

### Wave 1's two plans were assigned disjoint files so they could run parallel in worktrees
25-01 touches only `crates/brokerd/tests/s9_acceptance.rs`; 25-02 touches only
`.planning/phases/25-regression-live-proof/25-REGRESSION-AUDIT.md` (0 fixture fixes expected).

**Rationale:** Disjoint `files_modified` lists let the two autonomous plans execute concurrently in
isolated worktrees with no merge conflict, halving wall-clock for the wave.
**Source:** 25-01-PLAN.md / 25-02-PLAN.md frontmatter

### A NEEDS-FIX fixture (had any existed) would be corrected by assigning its true role, never by weakening an assertion
The regression audit's reconciliation rule: fix a misrouted fixture by giving the value its correct
`origin_role`; never delete/soften a security assertion, never touch production/TCB logic (stop and
flag instead).

**Rationale:** T2-07 defends against test-suite laundering. "Making the test pass" by weakening the
assertion would reintroduce the very bypass the audit exists to catch. (Moot this phase — 0 NEEDS-FIX.)
**Source:** 25-02-PLAN.md (Task 2), 25-REGRESSION-AUDIT.md

---

## Lessons

### Record the human sign-off into the artifact BEFORE flipping the last plan complete
Marking the final plan complete auto-rolled ROADMAP's phase checkbox to Complete, but the SUMMARY
still said sign-off "awaiting" and REQUIREMENTS.md still read T2-06/T2-08 "Pending." The fresh
verifier — which cannot see the chat where the human approved — correctly returned `gaps_found`.

**Context:** The approval was genuine; only its recording lagged. Writing the sign-off into
25-03-SUMMARY.md first (then `phase.complete` to reconcile REQUIREMENTS.md) avoids tripping the
auto-rollup + independent-verifier sequence. Same class as the logged Phase 15-17 "executor
self-marks phase complete" pattern.
**Source:** 25-VERIFICATION.md (re_verification.notes), session execution

### `roadmap.update-plan-progress` on the LAST plan silently rolls the whole phase to Complete
Flipping the third/last plan to complete did not just update a plan row — it marked the Phase 25
checkbox `[x]` and the Progress table "Complete (2026-07-12)" before verification ran.

**Context:** This meant ROADMAP showed Complete while REQUIREMENTS.md traceability was still Pending —
an inconsistency the verifier flagged. The rollup is convenient but must be paired with running the
verifier + `phase.complete` promptly so tracking files don't diverge.
**Source:** session execution, 25-VERIFICATION.md

### Worktree-run and orchestrator-run plans do NOT write REQUIREMENTS.md; only main-tree sequential plans do
T2-06 (25-01, in a worktree) and T2-08 (25-03, orchestrator-run) never updated REQUIREMENTS.md;
only T2-07 (25-02) did — because worktree executors skip shared-file writes and the orchestrator's
direct run didn't touch it either.

**Context:** The traceability table ends up partially synced until `phase.complete` reconciles it.
Expect REQUIREMENTS.md to lag for any requirement whose plan ran in a worktree or was run directly
by the orchestrator.
**Source:** 25-VERIFICATION.md (re_verification.notes)

### The Linux run has ~40 more passing tests than Mac — and that delta is the proof the security suite ran
Live Linux: 309 passed / 0 failed across 46 suites; Mac post-merge: 269 passed / 0 failed.

**Context:** The ~40-test delta is exactly the Linux-only Landlock + seccomp + no_new_privs + e2e
confined-worker tests that report 0-passed on Mac by design. Their non-zero pass count on Linux is
positive evidence the kernel-enforced suite actually executed — not a discrepancy to reconcile.
**Source:** 25-03-SUMMARY.md, 25-02-SUMMARY.md

---

## Patterns

### Held-out proof via the REAL broker path (mint_from_intent → submit_plan_node), never a stapled deny
Prove enforcement by minting values with genuine mint-time `origin_role` tags, submitting a real
PlanNode, and matching the returned `Denied{SlotTypeMismatch}` via a `match` arm — never by
hand-constructing the DenyReason or calling `ValueStore::mint()` directly.

**When to use:** Any acceptance test that must prove a TCB check fires through production code (not
a fixture that fabricates the outcome). Pair with a correctly-routed Allowed control over the SAME
two values to prove the deny is attributable to the specific check (here Step 1c) and not to I0/I2.
**Source:** 25-01-PLAN.md, 25-VERIFICATION.md

### Independent re-grep discipline: never cite a prior session's file inventory or counts as final
Re-run the actual search commands and record the actual counts, even when they match the prior
session's — the 25-02 audit found a 6th direct-mint file (`file_create.rs` test module) the prior
target list had missed, caught only because the grep was re-run blind.

**When to use:** Any "confirm the prior sweep" audit. A rubber-stamp of a self-reported inventory is
worthless against test-suite-laundering; the value is in the independent re-derivation.
**Source:** 25-02-SUMMARY.md, 25-REGRESSION-AUDIT.md

### Capture `$?` on the line immediately after the invocation, before any pipe
`bash scripts/mailpit-verify.sh > log 2>&1` then `RESULT=$?` on the very next line — assert on the
verbatim PASSED sentinel + named `test result:` counts + held-out-test presence, never on an
exit-0-that-flowed-through-a-pipe.

**When to use:** Every milestone-close or CI gate whose pass/fail you assert on. A `| tail`/`| grep`
between the command and `$?` silently returns the pipe's status; this is a logged prior near-miss
(a false PASS almost shipped this way).
**Source:** 25-03-PLAN.md, 25-03-SUMMARY.md

### Bare default recipe only for the closing gate — no MAILPIT_VERIFY_CMD scoping
The milestone-close run uses the unscoped `cargo build --workspace && cargo test --workspace
--no-fail-fast` default, which rebuilds all sibling binaries and cannot be masked by a scoped run.

**When to use:** The CLOSING verification of any milestone. Scoped runs are fine for iteration speed
during execution, but a scoped invocation once hid a real binary-placement bug (v1.4 caprun-planner)
that only the bare default surfaced.
**Source:** 25-03-PLAN.md (threat T-25-06), CLAUDE.md

---

## Surprises

### The independent verifier tripped `gaps_found` on a bookkeeping gap, not an engineering gap
Every engineering criterion was independently confirmed (the verifier even ran
`cargo test -p brokerd ... slot_type_binding` → 2 passed/0 failed), yet it returned `gaps_found`
because the human sign-off wasn't recorded in the repo and REQUIREMENTS.md was out of sync.

**Impact:** Correct and valuable — it forced the sign-off to be durably recorded and the
traceability reconciled before the phase could close. A pure "did the code work" check would have
missed the record/verdict inconsistency entirely. Resolved by recording the sign-off, then a second
verifier pass flipped to `passed` 4/4 after re-reading the file itself.
**Source:** 25-VERIFICATION.md, session execution

### The regression audit was expected to find ~5 files and 0 issues — it found 9 files, 42 sites, still 0 issues
RESEARCH predicted ~5 Mac-buildable direct-mint files; the independent re-grep found 42 hits across
9 files (31 role-checked), all CORRECT, 0 NEEDS-FIX.

**Impact:** More sites than predicted, but the verdict held — Phase 24's role threading was complete.
The extra files (incl. a `file_create.rs` test module) validated the "re-grep, don't cite" discipline
without changing the outcome.
**Source:** 25-02-SUMMARY.md

### Both wave-1 worktrees merged with zero conflicts and no worktree gotchas
Despite the project's history of worktree record-agent/base gotchas, the two parallel worktrees
recorded, merged, and cleaned up cleanly on the first pass.

**Impact:** Confirmed that disjoint `files_modified` assignment plus dispatching agents one-at-a-time
(to avoid `.git/config.lock` contention) is a reliable recipe for parallel worktree waves on this repo.
**Source:** session execution, 25-01/25-02 SUMMARY.md
