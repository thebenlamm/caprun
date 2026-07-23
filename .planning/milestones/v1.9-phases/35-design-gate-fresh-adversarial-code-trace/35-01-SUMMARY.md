---
phase: 35-design-gate-fresh-adversarial-code-trace
plan: 01
subsystem: infra
tags: [design-gate, git, github, http, taint, ssrf, seccomp, landlock, rustls, security]

# Dependency graph
requires:
  - phase: 34-effect-breadth-live-proof
    provides: v1.7 caprun-exec-launcher (Pattern B), mint_from_exec, prepare_process_exec confirm-release, env_clear'd confined child
  - phase: 31-effect-breadth-design-gate
    provides: DESIGN-effect-breadth-exec.md (the §-numbered design-gate shape + fail-closed-table + accepted-residuals conventions modeled here)
provides:
  - planning-docs/DESIGN-git-github-http-sinks.md — the reviewed design contract (DESIGN-15) for all four v1.8 sinks
  - Pinned dispatch pattern + effect-class + I2-sensitive args + taint flow + confinement per sink
  - The one NEW mechanism: mint_from_http + TaintLabel::HttpRaw + session demotion
  - FORK decisions (git.push net-allowed child; crypto=ring; github.pr session-scoped auth-grant)
  - 11-pitfall threat model, invariant-preservation, confirm-release discipline, Gate 3 mint_from_http mandate
affects: [36-git-commit, 37-http-request, 38-github-pr, 39-git-push, 40-cli-compose-live-proof]

# Tech tracking
tech-stack:
  added: []  # DOC-ONLY phase — no code, no deps. reqwest/ring/webpki-roots are PINNED for Phases 37+, not installed here.
  patterns:
    - "Design-gate-first: a §-numbered DESIGN doc pins mechanism + fail-closed default for every new sink before any TCB code (8th consecutive milestone)"
    - "Transcribe-not-re-decide: the executor elaborates the AUTHORITATIVE 35-CONTEXT forks, grounding each mechanism claim in real exemplar file:line"

key-files:
  created:
    - planning-docs/DESIGN-git-github-http-sinks.md
  modified: []

key-decisions:
  - "git.commit = MutateReversible (deliberate exception to the unknown→CommitIrreversible default; a local commit is reversible, survives an I1-demoted session)"
  - "git.push = net-allowed confined child (FORK 1): a minimal single-pinned-host:port seccomp relaxation on the child only, worker never gains net — the riskiest new surface, flagged top of the review list"
  - "http.request GET = Observe but the response demotes the session; NEW mint_from_http rooted on a real http_response_received Event + NEW TaintLabel::HttpRaw compile-forced into is_untrusted()"
  - "github.pr = FORK 3 session-scoped capability auth-grant (caprun grant) as an INDEPENDENT gate from the per-PR I2 confirm; a bare confirm cannot create a PR"
  - "crypto = ring (FORK 2, aws-lc-rs acceptable if cleaner); CA roots = compiled-in webpki-roots so env_clear() is hermetic (ENV-01)"

patterns-established:
  - "Pattern B extended: a confined child MAY hold network + a short-lived credential (git.push) rather than widening the broker reference monitor"
  - "Duplicate-effect CAS: content-derived idempotency key committed before the API call (mirrors v1.6 HARDEN-03) generalized from email.send to github.pr"

requirements-completed: [DESIGN-15]

coverage:
  - id: D1
    description: "DESIGN-git-github-http-sinks.md pins mechanism + fail-closed default for all four sinks, closes all 11 pitfalls with a named mechanism, states the 3 forks, proves I0/I1/I2 non-weakening — cleared for the fresh adversarial code-trace"
    requirement: "DESIGN-15"
    verification:
      - kind: automated
        ref: "bash scripts/check-invariants.sh (exit 0) + git status --porcelain crates/ cli/ (empty)"
        status: pass
    human_judgment: true
    rationale: "A design-gate doc's soundness is established by a fresh, non-self adversarial CODE-TRACE (Plan 35-02, orchestrator-owned), not by automation. The grep/gate checks only confirm section presence + no-TCB-code; whether every mechanism claim actually traces to real code and closes each pitfall is a human/reviewer judgment (DESIGN-16)."

# Metrics
duration: 22min
completed: 2026-07-18
status: complete
---

# Phase 35 Plan 01: DESIGN Gate — git/github/http Sinks Summary

**A 658-line, §0-§12 design contract (DESIGN-15) pinning the dispatch pattern, effect-class, I2-sensitive args, taint flow, and confinement for git.commit (MutateReversible), git.push (net-allowed confined child, FORK 1), read-only http.request GET (Observe + the new mint_from_http/HttpRaw mechanism), and github.pr (session-scoped auth-grant + duplicate-PR CAS) — closing all 11 design-gate-blocking pitfalls with a named mechanism each, ready for the fresh non-self adversarial code-trace.**

## Performance

- **Duration:** ~22 min
- **Started:** 2026-07-18
- **Completed:** 2026-07-18
- **Tasks:** 3
- **Files modified:** 1 (created)

## Accomplishments
- Authored `planning-docs/DESIGN-git-github-http-sinks.md` (§0-§12, 13 sections) — the v1.8 design gate that HARD-BLOCKS Phases 36-40 until the DESIGN-16 fresh adversarial code-trace clears it.
- Pinned each sink onto one of caprun's two shipped dispatch patterns (A = in-broker egress / B = broker-spawned confined child) with a concrete cited exemplar (`email_smtp.rs`, `process_exec.rs`), introducing NO third pattern and NO raw effect-request-to-sink path.
- Pinned the one genuinely NEW mechanism — `mint_from_http` rooted on a real `http_response_received` Event with session demotion (I1) + a compile-forced `TaintLabel::HttpRaw` variant — mirroring the shipped `mint_from_read`/`mint_from_exec` non-stapled anchor discipline.
- Transcribed and elaborated the three AUTHORITATIVE 35-CONTEXT forks (git.push net-allowed child; crypto=ring; github.pr session-scoped auth-grant) with rationale, without re-opening them.
- Closed all 11 pitfalls (§6 table), proved I0/I1/I2 non-weakening (§7), mandated the P33/P34 terminal-event-before-terminal-state confirm-release discipline with `prepare_git_push`/`prepare_github_pr` (§9), and mandated the Gate 3 `mint_from_http(` call-site extension (§10).
- `scripts/check-invariants.sh` exits 0; the git diff touches only `planning-docs/`.

## Task Commits

Each task was committed atomically:

1. **Task 1: §0-§2 (scope, git.commit, git.push FORK-1)** - `675d2af` (docs)
2. **Task 2: §3-§5 (http.request + mint_from_http, github.pr + auth-grant + CAS, crypto FORK-2 + env_clear TLS)** - `6a05541` (docs)
3. **Task 3: §6-§12 (threat model, invariant preservation, fail-closed table, confirm-release discipline, Gate 3 mandate, residuals, acceptance predicate)** - `8350853` (docs)

## Files Created/Modified
- `planning-docs/DESIGN-git-github-http-sinks.md` - The reviewed design contract for all four v1.8 external-effect sinks (created).

## Decisions Made
None beyond transcribing the AUTHORITATIVE 35-CONTEXT decisions. Per the plan's hard constraints, the three grey-area forks were NOT re-opened or re-decided — they were transcribed with rationale and grounded in real exemplar source. One author's-discretion pick within a decided fork: FORK 2 crypto = `ring` (35-CONTEXT explicitly delegates the pick to the doc author, "aws-lc-rs acceptable if provider-consistency is materially cleaner"; ring chosen for "minimize untrusted C in the TCB").

## Deviations from Plan
None - plan executed exactly as written. All three tasks' automated verify greps passed, `check-invariants.sh` exits 0, and `git status --porcelain crates/ cli/` is empty at every task boundary.

## Issues Encountered
None. All cited file:line references were read directly this session (process_exec.rs, email_smtp.rs, quarantine.rs, plan_node.rs, effect.rs, sink_sensitivity.rs, confirmation.rs, main.rs, check-invariants.sh) so the doc is traceable for the fresh reviewer. Note: `check-invariants.sh` Gate 3 already restricts `mint_from_exec(` (v1.7 P32) — the doc correctly mandates ADDING a fifth `mint_from_http(` token, which Gate 3 does NOT catch today.

## User Setup Required
None - no external service configuration required this phase (DOC-ONLY).

## Next Phase Readiness
- The design doc is complete and passes its automated gate. **It is NOT yet cleared** — Plan 35-02 (the fresh, non-self, orchestrator-owned adversarial code-trace, DESIGN-16) must clear it with all findings resolved and record the result in `planning-docs/DESIGN-GATE-RECORD-v1.8.md` before ANY Phase 36+ TCB code.
- Top item flagged for the reviewer to pressure-test: the `git.push` net-allowed confined child (§2.1/§2.3/§11) — a confined child WITH network is a genuinely new trust posture.
- Blocker unchanged: no `crates/executor`/`brokerd`/`sandbox`/`runtime-core` code until the gate clears (STATE.md Blockers).

## Self-Check: PASSED

- `planning-docs/DESIGN-git-github-http-sinks.md` — FOUND
- `.planning/phases/35-design-gate-fresh-adversarial-code-trace/35-01-SUMMARY.md` — FOUND
- Commit `675d2af` (Task 1) — FOUND
- Commit `6a05541` (Task 2) — FOUND
- Commit `8350853` (Task 3) — FOUND
- `scripts/check-invariants.sh` — exit 0
- `git status --porcelain crates/ cli/` — empty (no TCB code)

---
*Phase: 35-design-gate-fresh-adversarial-code-trace*
*Completed: 2026-07-18*
