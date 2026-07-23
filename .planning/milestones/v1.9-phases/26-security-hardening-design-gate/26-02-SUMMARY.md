# 26-02 SUMMARY — DESIGN-12: fresh non-self adversarial review + gate clearance

**Plan:** 26-02 (wave 2, depends_on 26-01)
**Requirement:** DESIGN-12
**Status:** ✅ Complete — gate CLEARED
**Date:** 2026-07-12

## What was done

1. **Fresh non-self adversarial review (Task 1).** The orchestrator spawned a distinct
   `Agent(model:"fable")` (Claude Fable 5) reviewer — advisor tool unavailable this session, so the
   standing Fable fallback was used per project discipline. The reviewer independently **traced live
   code** (`main.rs`, `adapter-fs/workspace.rs`, `server.rs`, `audit.rs`, `confirmation.rs`,
   `quarantine.rs`, `sink_sensitivity.rs`, `executor/lib.rs`, `planner.rs`, `worker.rs`, both
   `Cargo.toml`s, `check-invariants.sh`) rather than prose-reading the doc. Genuinely non-self: the doc
   was authored by a `gsd-executor` (Opus) in plan 26-01; the reviewer is a different model family with
   no authoring lineage.

2. **Findings verified against code, then folded (Task 2).** The review returned 3 findings — 1
   BLOCKER, 1 MAJOR, 1 MINOR. Per the project "verify each finding against actual code before fixing"
   discipline, the orchestrator independently re-traced each against live source; **all three confirmed
   REAL (no false positives)**:
   - **F1 (BLOCKER) → §b:** the HARDEN-02 MAC key (`<audit_path>.key`) is reachable by the confined
     worker via `RequestFd`/`read_within` when the audit DB is co-located under the workspace root
     (`audit_path` is a free-form CLI arg independent of the workspace root; `read_within` opens
     anything `RESOLVE_BENEATH` the root; `RequestFd` passes the fd). Fixed by pinning a
     broker-enforced fail-closed startup refusal when the audit/key paths resolve beneath the workspace
     root — now a broker guarantee, not operator convention.
   - **F2 (MAJOR) → §a:** the trusted-label "reuse the canonical form `read_within` computed" pin is
     not implementable (`read_within` returns only a `File`, discarding the path) and its ordering was
     self-contradictory. Replaced with a broker-derived `fstat` `(st_dev, st_ino)` inode-identity
     compare; corrected ordering to open-fd → identity-compare → demote-if-untrusted → `pass_fd`.
   - **F3 (MINOR) → §f/X-04:** pinned the shared `Arc<Mutex<SessionStatus>>` as construct-once /
     monotonic `Active→Draft`; no connection-setup path may re-seed `Active` after a demotion.
   All three folded as Round-1 amendments into the relevant §§ (with `> Revised after
   DESIGN-GATE-RECORD-v1.6 Round 1` blockquotes), the doc Status set to CLEARED, and the Amendments §
   populated. No ruling was weakened; F1/F2 hardened *enforcement*, not mechanism choice.

3. **GATE-RECORD written.** `planning-docs/DESIGN-GATE-RECORD-v1.6.md` records reviewer identity &
   independence, the revision-history table, all 3 findings with code evidence + resolution, the
   reviewer's "verified sound" list (proof of genuine tracing), and the no-TCB-code reconfirmation.

4. **Pinned-doc amendment (D-02).** `planning-docs/DESIGN-session-trust-state.md` amended with a
   forward-looking note: the "SOLE trust-flip site" / "no other function may set Draft for I1" letter is
   amended to permit a SECOND broker-side I1 demotion site (`RequestFd` entry), both broker-only; code
   lands Phase 27. Honestly marked as recording the design decision, not current shipped behavior.

5. **Hard-gate reconfirmation (Task 3).** `git status --porcelain crates/ cli/` empty; no new
   mechanism symbols (`Hmac`/`chain_anchor`/`sent_plan_nodes`/`is_trusted_labeled`) under `crates/`;
   `brokerd/Cargo.toml` still has no `[features]`; `contents => None` unchanged; `check-invariants.sh`
   all gates PASS (exit 0). No `crates/executor`/`crates/brokerd`/`crates/runtime-core` hardening code
   was written — the gate held.

## Artifacts
- `planning-docs/DESIGN-security-hardening.md` (amended: F1/F2/F3 folded, Status CLEARED)
- `planning-docs/DESIGN-GATE-RECORD-v1.6.md` (new)
- `planning-docs/DESIGN-session-trust-state.md` (D-02 amendment)

## Deviation note
Per the plan, Task 1 spawns the reviewer via `Agent(model:"fable")`. Because `gsd-executor` has no
Agent tool (cannot spawn a subagent), the non-self review and the finding-fold were run by the
**orchestrator** directly (which owns the Agent tool) — the correct locus for a genuinely non-self
gate, and consistent with the project's `fresh-context-adversarial-review` discipline (the author must
not review its own doc). The Write/Bash portions (fold, GATE-RECORD, amendment, hard-gate) were done
inline by the orchestrator rather than a fresh `gsd-executor`, to keep the load-bearing F1 amendment
under direct verification and avoid a further subagent round-trip. Result is faithful to plan intent.

## DESIGN-12: satisfied. Gate CLEARED — Phases 27–30 authorized.
