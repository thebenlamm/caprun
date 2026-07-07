---
phase: 12-content-adapter-confirm-binding-design-gate
plan: 01
subsystem: security-design-doc
tags: [rust, executor, i2, taint, smtp, lettre, seccomp, design-gate]

# Dependency graph
requires:
  - phase: 08-content-adapter-confirm-binding-design-gate  # v1.2 Phase 8 (design-gate precedent)
    provides: DESIGN-session-trust-state.md's Step 0.5 precedence fix (B1), the ExecutorDecision/SinkBlockedAnchor shape this doc hardens
provides:
  - "planning-docs/DESIGN-content-adapter-mediation.md — the CONTENT-01/02 + collect-then-Block + real-SMTP-adapter DESIGN doc, half of the DESIGN-01 gate"
affects: [13-smtp-adapter, 14-content-sensitivity-executor, 15-deterministic-extraction, 16-confirm-binding, plan-02-confirm-binding-doc, plan-03-gate-record]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Collect-then-Block: executor per-arg loop collects ALL sensitive+tainted args before returning one plural Block decision, replacing first-match-wins early return"
    - "Illustrative-code-not-literal convention for Rust snippets in DESIGN docs (matches DESIGN-confirmation-release.md)"
    - "Named, greppable Precedence subsection (matches DESIGN-session-trust-state.md §8)"

key-files:
  created:
    - planning-docs/DESIGN-content-adapter-mediation.md
  modified: []

key-decisions:
  - "CONTENT-01/02 is NOT new classification code — is_content_sensitive already exists at sink_sensitivity.rs:93-98; Phase 14's work is changing Step 3's consequence (no-op to Block), documented explicitly so Phase 14 doesn't duplicate it (D-21)"
  - "ExecutorDecision::BlockedPendingConfirmation and SinkBlockedAnchor are mandated to become plural (Vec<BlockedArg>) to fix the B1-reincarnation risk: a tainted recipient and tainted body on the same plan node must both surface as Blocked, not one silently pre-empting the other (D-14/D-02)"
  - "I2-over-I1 precedence (Step 0.5 runs only after the collect-all loop completes with no Block) is explicitly preserved unchanged, not reordered (D-15)"
  - "SMTP-01's negative net assertion is pointed at the EXISTING crates/sandbox/src/seccomp.rs apply_worker_filter()/confine-probe mechanism, not a new confinement primitive (D-05)"
  - "CRLF/header-injection defense mandates lettre's typed builder exclusively, forbids dangerous_new_pre_encoded/format!-built headers/boring-tls, and requires a Phase-13 adversarial CRLF fixture regardless of lettre's by-construction defense (D-07/D-22)"

patterns-established:
  - "Pattern: DESIGN doc sections named exactly as their acceptance-criteria grep targets (## Content-Sensitivity Classification (CONTENT-01/CONTENT-02), ## Precedence — ..., ## Collect-then-Block (D-14, MUST), etc.) so downstream review/verification greps are unambiguous"

requirements-completed: [DESIGN-01]

coverage:
  - id: D1
    description: "planning-docs/DESIGN-content-adapter-mediation.md exists with all required named sections resolving D-01/D-02/D-14 through D-18/D-21 (content-sensitivity + collect-then-Block) and D-03 through D-07/D-22 (adapter mediation + CRLF defense) as hard MUSTs"
    requirement: "DESIGN-01"
    verification:
      - kind: other
        ref: "for d in D-01 D-02 D-14 D-15 D-16 D-17 D-18 D-21 D-03 D-04 D-05 D-06 D-07 D-22; do grep -q \"$d\" planning-docs/DESIGN-content-adapter-mediation.md || echo MISSING $d; done — all 14 present"
        status: pass
      - kind: other
        ref: "grep -c 'MUST' planning-docs/DESIGN-content-adapter-mediation.md → 52 (>= 20 threshold)"
        status: pass
    human_judgment: true
    rationale: "This document is one half of a security design gate whose actual approval requires D-11's mandated genuinely-adversarial fresh-context review (plan 03), not self-review by the authoring session — grep-completeness proves presence of required tokens, not soundness of the design."

# Metrics
duration: ~35min
completed: 2026-07-07
status: complete
---

# Phase 12 Plan 01: Content-Adapter-Mediation DESIGN Doc Summary

**Authored `planning-docs/DESIGN-content-adapter-mediation.md` (342 lines, 52 MUST/MUST-NOT statements) mandating collect-then-Block executor hardening (fixes the B1-reincarnation risk for a tainted email body) and real broker-mediated SMTP adapter mediation with source-verified CRLF/header-injection defense — all 14 cited D-IDs resolved.**

## Performance

- **Duration:** ~35 min
- **Tasks:** 3 (2 code-producing, 1 verification-only)
- **Files modified:** 1 created

## Accomplishments
- Content-sensitivity classification (CONTENT-01/02) documented as already-implemented (`sink_sensitivity.rs:93-98`), with Phase 14's real work correctly scoped to Step 3's consequence, not new classification — reviewer instructed to independently re-verify (D-21).
- Collect-then-Block mandated as a hard MUST (D-14): per-arg loop collects all sensitive+tainted args before Blocking as a set; `ExecutorDecision`/`SinkBlockedAnchor` become plural; I2-over-I1 precedence (D-15), per-arg unbroken-edge gate (D-16), and whole-set single-shot semantics (D-17) all specified.
- Real SMTP adapter mediation documented: worker-never-sends, secrets-broker-only (D-03/D-04), negative net assertion pointed at the existing seccomp mechanism rather than a new primitive (D-05), Mailpit as the local gate target with live-SES explicitly out of scope (D-06).
- CRLF/header-injection defense (SMTP-05, D-07/D-22) specified with concrete mechanics verified against `lettre`'s own source (allow-list address grammar, RFC 2047 header encoding, header/body structural separation), forbidding `dangerous_new_pre_encoded`, string-built headers, and `boring-tls`, and mandating a Phase-13 adversarial CRLF fixture regardless of the library's by-construction defense.
- Completeness self-check (Task 3): all 14 D-IDs (D-01, D-02, D-14 through D-18, D-21, D-03 through D-07, D-22) present verbatim; `grep -c 'MUST'` = 52 (≥ 20 density target); no source file created under `crates/` — documentation-only phase preserved.

## Task Commits

Each task was committed atomically:

1. **Task 1: Content-sensitivity + collect-then-Block mandate sections** - `c7835d7` (docs)
2. **Task 2: Real-adapter mediation + CRLF/header-injection defense sections** - `08652a8` (docs)
3. **Task 3: Completeness self-check** - no commit (verification-only; all 14 D-IDs and MUST-density already satisfied after Tasks 1-2, no gaps found to repair)

## Files Created/Modified
- `planning-docs/DESIGN-content-adapter-mediation.md` - The DESIGN-01 gate document (half): content-sensitivity semantics, collect-then-Block executor mandate, real SMTP adapter mediation, CRLF/header-injection defense, terminal Done-When acceptance predicate.

## Decisions Made
- Followed the plan's explicit direction throughout — no open decisions left to Claude's discretion beyond phrasing. All Rust snippets marked "illustrative shape, not literal code to paste" per the PATTERNS.md convention.
- Cited existing code by exact file/line (`crates/executor/src/lib.rs:62-143`, `crates/runtime-core/src/executor_decision.rs:108-153`, `crates/executor/src/sink_sensitivity.rs:93-98`) rather than paraphrasing, so the plan-03 adversarial reviewer can re-verify claims in seconds.

## Deviations from Plan

None - plan executed exactly as written. Task 3 found no gaps (all D-IDs and MUST-density already satisfied by Tasks 1-2), so it produced no additional edits or commit, matching its own "repair any gap in-place" instruction (there was no gap to repair).

## Issues Encountered

One environment note: the first `Write` attempt targeted the shared-checkout path (`/Users/benlamm/Workspace/AgentOS/planning-docs/...`) and was correctly rejected by the harness's worktree-isolation guard; corrected to write into this worktree's own path (`.claude/worktrees/agent-ad190f0494b33d9da/planning-docs/...`) before any commit was made. No content was lost.

## User Setup Required

None - no external service configuration required. (Mailpit/`lettre` setup is Phase 13's concern, not this phase's.)

## Next Phase Readiness

- This doc is ready to be read alongside `DESIGN-confirm-binding.md` (plan 02, not yet authored) by plan 03's adversarial reviewer.
- Gate remains BLOCKED: no executor/TCB code for CONTENT-01, SMTP-05, or CONFIRM-03 may be written until `DESIGN-GATE-RECORD-v1.3.md` (plan 03) records Decision: APPROVED / Gate status: UNBLOCKED, per D-11 (genuinely adversarial, not self-review).
- Plan 02 (`DESIGN-confirm-binding.md`, CONFIRM-03) and plan 03 (gate record + adversarial review) are the remaining work in this phase; both should read this document's D-14/D-17/D-19 collect-then-Block/single-shot-set language directly, since D-19 (confirm-binding hash over the FULL blocked set) depends on this document's plural-anchor shape.

---
*Phase: 12-content-adapter-confirm-binding-design-gate*
*Completed: 2026-07-07*
