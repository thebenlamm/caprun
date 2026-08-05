# Phase 51 Adversarial Trace Brief

**Plan 51-07 fix commit:** `442a056e7fffb1d3bd32e6f93e1228908bde31f2`  
**Exact diff range:** `a6b8911f5bcb3344b90416c5fcf57294b827cc0f..442a056e7fffb1d3bd32e6f93e1228908bde31f2`  
**Changed files:**

- `crates/brokerd/src/audit.rs`
- `crates/brokerd/src/server.rs`
- `crates/brokerd/src/confirmation.rs`

Run `git diff a6b8911f5bcb3344b90416c5fcf57294b827cc0f..442a056e7fffb1d3bd32e6f93e1228908bde31f2 -- crates/brokerd/src/audit.rs crates/brokerd/src/server.rs crates/brokerd/src/confirmation.rs` to inspect the landed change. The review must trace the **live code at commit `442a056e7fffb1d3bd32e6f93e1228908bde31f2`**, not the diff alone and not the design note alone. Treat the design note as a claim to challenge, not an authority to agree with.

## Mandatory obligations

Answer every question below with PASS or FAIL and falsifiable file-and-line evidence.

1. **Chain-fork closure.** Can any production path still append an event whose persisted parent is not the durable chain head at insert time? Trace the connection handler's in-memory head threading, `record_github_grant`, the confirm-granted append, and every sink terminal event. Identify any bypass of the append choke point.

2. **Timeout-and-transaction coupling.** Is the busy timeout set on every connection `open_audit_db` returns, including the short-lived connections opened by the grant, confirm, deny, and external-hold-poll paths in `cli/caprun/src/main.rs`? Is the head read inside the same immediate transaction as the insert, or could it be hoisted out under any code path, including the re-entrant path?

3. **Mandatory non-conflation check.** Is the causal chain `parent_id` anywhere conflated with, derived from, assigned to, or reused as the provenance `read_event_id`? Trace `crates/brokerd/src/server.rs:941`, `crates/brokerd/src/server.rs:957`, and `crates/brokerd/src/server.rs:973` specifically. I2 and M7 ride on the provenance graph; a fix that unifies the two graphs can make every test pass while silently destroying I2. Give a direct statement about the values and flows at all three sites; a general reassurance does not answer this obligation.

4. **Verifier integrity.** Are `verify_chain`, `current_chain_head`, `compute_event_hash`, `verify_event_hash`, `compute_anchor_mac`, and `verify_anchor_mac` unchanged? Does HARDEN-02 tail-truncation detection—the MAC over the chain anchor's head plus event count—still hold under the new append ordering?

5. **Inventory completeness and latent instances.** Does every row of `planning-docs/DESIGN-audit-append-concurrency.md` Appendix A appear in the live code with its stated treatment, and is there any production `append_event` call site not in that inventory? Appendix A claims 45 production sites: 2 session roots, 40 chain continuations, and 3 in-transaction sites. Verify that count rather than trusting it. Are the two latent instances—`record_github_grant`'s read-then-append at `crates/brokerd/src/audit.rs:546-561` and the dual-connection same-seed head at `crates/brokerd/src/server.rs:307-382`—actually in the state their design-note dispositions claim?

6. **Atomicity.** Did the sink-blocked group (event append, blocked-literal writes, and pending-confirmation insert) become less atomic? Can a blocked event now exist without its checkpoint or without its literal? Trace all commit and rollback boundaries involved.

7. **Failure modes.** What happens on a transaction rollback mid-append, on a poisoned mutex, and on a genuinely contended writer that exhausts the timeout? Does any path fail open, persist a partial append/anchor pair, or silently continue from an uncommitted head?

8. **Scope.** Does the diff touch `crates/executor/`, any policy surface, any mint site, `Cargo.toml`, `Cargo.lock`, or the `caprun run` output surface? Any such change is out of contract. Confirm the three-file diff mechanically and inspect the live tree for an indirect scope expansion.

## Minimum files to open

The reviewer must open at least:

- `crates/brokerd/src/audit.rs`, `crates/brokerd/src/server.rs`, and `crates/brokerd/src/confirmation.rs` at the fix revision;
- `crates/brokerd/src/quarantine.rs` and `crates/brokerd/src/policy.rs`;
- all terminal sink modules that append outcomes: `email_smtp.rs`, `file_create.rs`, `file_write.rs`, `git_commit.rs`, `git_push.rs`, `github_pr.rs`, `http_write.rs`, and `process_exec.rs`;
- the short-lived audit-connection sites in `cli/caprun/src/main.rs`, including grant, confirm, deny, and external-hold polling;
- `crates/brokerd/tests/audit_chain_fork_regression.rs`;
- `planning-docs/DESIGN-audit-append-concurrency.md`, including Sections 1–10 and every Appendix A row;
- `.planning/phases/51-non-hybrid-live-proof-v1-10-done/51-BLOCKING-DEFECTS.md` for the two known wrong turns and the prior defect claims.

The reviewer may open additional callers, tests, or historical revisions needed to falsify a claim. Record every file actually opened.

## Reviewer independence and spawn constraints

The reviewer must be a fresh-context, read-only, non-authoring agent. The orchestrator owns the spawn; the executor must not select itself as reviewer. A gsd-executor self-read is **not clearance**. The reviewer must not be the author of the design note or fix and must be told explicitly not to assume agreement with the design note, the fix author, or prior reviewers.

The review output must include:

- reviewer kind, identifier, model/runtime, effort, and an explicit author-not-equal-reviewer statement;
- every file opened;
- one PASS or FAIL response for each of the eight numbered obligations, with file-and-line code evidence;
- findings grouped by `BLOCKER`, `MAJOR`, `MINOR`, and `NIT`, each with a falsifiable claim and code evidence;
- explicit counts for every severity, including an explicit **BLOCKER count**;
- a verdict that leaves the gate uncleared if any BLOCKER exists or any MAJOR remains unresolved.

The orchestrator must independently re-verify every reported finding—and any clean-bill claim—against the live code before it is folded into `51-ADVERSARIAL-TRACE.md` or the design gate record.
