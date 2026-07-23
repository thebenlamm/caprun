---
phase: 41-v1-9-design-gate-fresh-adversarial-code-trace
plan: 01
subsystem: design-gate
status: complete
tags: [design-gate, egress, git-push, http-write, policy, i2, ssrf, supply-chain]
requires:
  - ".planning/research/GIT-PUSH-EGRESS.md (candidate-b pin)"
  - "planning-docs/DESIGN-git-github-http-sinks.md (v1.8 §2/§2.5/§2.7/§9 carried forward)"
provides:
  - "planning-docs/DESIGN-v1.9-egress-policy.md (DESIGN-17 — pins git.push + http-write + policy↔I2)"
affects:
  - "Phase 42 (POLICY-01/02/03), Phase 43 (HTTP-W-01), Phase 44 (GIT-02/03 + HYG-01), Phase 45 (SDK-01/U1), Phase 46 (LIVE-05/06) — all HARD-BLOCKED until DESIGN-18 clears"
tech-stack:
  added: []
  patterns:
    - "decisions-not-options DESIGN doc mirroring v1.8 shape"
    - "carry-forward-by-reference (v1.8 §2/§2.5/§2.7/§9 not restated)"
key-files:
  created:
    - planning-docs/DESIGN-v1.9-egress-policy.md
  modified: []
decisions:
  - "git.push pins research candidate (b): broker-performed git smart-HTTP transfer; child fully net-denied (exec_child_filter verbatim); pin = reqwest .resolve(host,pinned_ip) in the broker app layer, NEVER seccomp"
  - "http.request WRITE gets a DISTINCT write host-allowlist (GET-readable ≠ POST/PUT-writable); body content-sensitive under I2; differential acceptance"
  - "policy is a pre-I2 narrowing gate with a distinct PolicyDeny outcome; I2 stays HARDCODED in the TCB executor, unconditional on every permitted call; POLICY-03 binds policy at session creation via F1 containment reused verbatim from key.rs"
  - "ring-only, ZERO new crates; Gate-5 workspace-scoped absence assertion re-runs after the git.push transport dep is chosen"
  - "git.push safety-valve: disclosed, sign-off-gated deferral (auto-descopes LIVE-05/06) if no sound unprivileged pin proves out — never a silent drop, never a net-allowed child"
  - "DESIGN-18 adversarial trace is ORCHESTRATOR-owned (not a gsd-executor) and re-runs on a mid-build git.push trust-posture/transport-dep pivot"
metrics:
  duration: ~30m
  completed: 2026-07-18
  tasks: 2
  files_created: 1
  files_modified: 0
  commits: 2
---

# Phase 41 Plan 01: v1.9 Egress + Policy Design Gate Summary

Authored the single v1.9 design-gate deliverable — `planning-docs/DESIGN-v1.9-egress-policy.md` — pinning, as decisions-not-options, the three v1.9 TCB surfaces (git.push broker-performed smart-HTTP egress with a fully net-denied child; http.request WRITE on a distinct allowlist; the policy↔I2 boundary with POLICY-03 F1-containment binding), all cited to re-verified `file:line` against real code, with zero TCB code written.

## What was built

- **`planning-docs/DESIGN-v1.9-egress-policy.md`** (10 §-numbered sections + frontmatter):
  - **§0** purpose/scope; carries v1.8 §2/§2.5/§2.7/§9 forward **by reference** (cited, not restated).
  - **§1 git.push** — pins research candidate (b): broker plays HTTP mover (reqwest `.resolve(host,pinned)` at `http_request.rs:337`, TLS broker-side), child does local `git send-pack --stateless-rpc` pack generation and stays **fully net-denied** (`exec_child_filter`, `seccomp.rs:147,163-188` — no relaxation, pin never seccomp). Closes all three research attack points: credential leak (§1.4, carries v1.8 §2.5); redirect/DNS-rebind pin-bypass (§1.5 — one frozen IP across info/refs GET + receive-pack POST, POST 3xx refused via `redirect(Policy::none())` `http_request.rs:335`); payload-vs-destination confirm TOCTOU (§1.6, carries v1.8 §2.7 + anti-TOCTOU freeze). Confirm-release carries v1.8 §9 (`prepare_git_push` + entry-guard extension `confirmation.rs:825-846`). §1.8 send-pack seccomp backstop. §1.9 disclosed sign-off-gated safety-valve.
  - **§2 http.request WRITE** — distinct `WRITE_HOST_ALLOWLIST` (vs shipped `HOST_ALLOWLIST` `http_request.rs:101`); body content-sensitive under I2; reuse SSRF resolve-and-pin (`validate_url`/`ssrf_check`/`vet_resolved`/`resolve_and_pin`); broker-env credential + response scrub; differential acceptance.
  - **§3** ring-only / ZERO new crates (`ring_webpki_tls_config` `http_request.rs:398`); Gate-5 workspace-scoped re-run after transport-dep choice (`check-invariants.sh:211-233`).
  - **§4** fail-closed defaults table (12 rows, new v1.9 mechanisms).
  - **§5 policy↔I2** — pre-I2 narrowing gate + distinct `PolicyDeny` (POLICY-01); I2 HARDCODED/unconditional, never overridden (POLICY-02 LOCKED); POLICY-03 binds policy at session creation reusing the F1 check verbatim from `key.rs` (`load_or_create_key` `key.rs:60`, canonicalize+`starts_with` refusal `key.rs:88-95`, unresolvable=refusal `key.rs:73,166`), immutable, hash audit-DAG-recorded.
  - **§6** §-per-pitfall threat model (12 rows → named mechanisms).
  - **§7** invariant preservation (I0/I1/I2 intact, no raw `EffectRequest` path).
  - **§8** new-symbol summary + mandated gate extensions.
  - **§9** DESIGN-18 orchestrator-owned adversarial trace, re-runs on mid-build git.push trust-posture/transport pivot; executor explicitly does NOT self-run it.
  - **§10** acceptance predicate.

## Deviations from Plan

None — plan executed exactly as written. Both tasks' automated verify gates passed; the Task-1 sentinel was written then replaced by Task 2 (its absence asserted). No package installs, no auth gates, no architectural (Rule 4) decisions required.

## Verification

- Task 1 automated verify: **PASS** (broker-performed / net-denied / resolve(host / distinct write-allowlist / safety-valve / ring-only / §2.5|§2.7|§9 all present; `git status --porcelain -- crates cli` empty).
- Task 2 automated verify: **PASS** (POLICY-03 / F1 containment / pre-I2-narrowing / HARDCODED / orchestrator-owned / re-run / acceptance predicate present; sentinel removed; crates/cli empty).
- Phase checks: deliverable exists; `git status --porcelain -- crates cli` empty (no TCB drift — only `planning-docs/DESIGN-v1.9-egress-policy.md` touched).
- `scripts/check-invariants.sh` exits **0** (all gates pass — no architectural-invariant regression from the doc's prose).

## Known Stubs

None. This is a docs-only design-gate phase; no code, no data-wiring, no placeholders.

## Threat Flags

None. No files created/modified introduce runtime security surface — the sole deliverable is a markdown design doc under `planning-docs/`. The load-bearing risk (an under-specified doc greenlighting an unsound git.push mechanism) is handled by the plan's own threat register (T-41-01) and the orchestrator-owned DESIGN-18 trace that runs after this plan.

## Handoff to orchestrator (DESIGN-18 — NOT an executor task)

The doc is **Draft → pending** the fresh, non-self, orchestrator-owned adversarial code-trace. Per §9 and the plan's HARD CONSTRAINT #2, gsd-executors have no Agent tool; the orchestrator must spawn a fresh non-self reviewer to trace every `file:line` against real code, resolve every BLOCKER/MAJOR, and record the outcome in `planning-docs/DESIGN-GATE-RECORD-v1.9.md` before ANY Phase 42-46 TCB code. The trace re-runs on a mid-build git.push trust-posture/transport-dep pivot.

## Commits

- `824d963` docs(41-01): author v1.9 egress DESIGN scaffold — git.push + http-write + crypto/fail-closed
- `dcdbb05` docs(41-01): author v1.9 policy↔I2 boundary, threat model, adversarial-trace gate

## Self-Check

- FOUND: planning-docs/DESIGN-v1.9-egress-policy.md
- FOUND commit: 824d963
- FOUND commit: dcdbb05

## Self-Check: PASSED
