# DESIGN GATE RECORD — v1.6 (Security Hardening — close the TCB-local residuals)

**Milestone:** v1.6 — Security Hardening (Phase 26 design gate)
**Document under review:** `planning-docs/DESIGN-security-hardening.md`
**Gate purpose:** Authorize (or block) any `crates/executor` / `crates/brokerd` / `crates/runtime-core`
hardening code for this milestone (Phases 27–30). Mirrors `DESIGN-GATE-RECORD-v1.5.md` (and v1.2 Phase 8
/ v1.3 Phase 12 / v1.4 Phase 18).
**Requirements gated:** DESIGN-11 (doc exists, pins mechanism + fail-closed default for all five
residuals), DESIGN-12 (doc clears a fresh non-self adversarial review with every finding resolved).

## Gate status: ✅ **CLEARED** (2026-07-12, Round 1)

Phases 27–30 are authorized to begin. All three review findings (1 BLOCKER, 1 MAJOR, 1 MINOR) are
resolved in the design doc as Round-1 amendments; no blocker remains. **No `crates/executor` /
`crates/brokerd` / `crates/runtime-core` hardening code was written during this design-gate phase**
(re-confirmed below).

---

## Reviewer identity & independence

- **Mechanism:** a FRESH, INDEPENDENT adversarial reviewer spawned by the orchestrator as a separate
  agent — a **Claude Fable 5** model (`claude-fable-5`), a different model family from the doc's
  authoring context. This satisfies the fresh-context requirement (the project's
  `fresh-context-adversarial-review` discipline: a self-read is not sufficient; the advisor tool was
  unavailable this session, so the standing `Agent(model:"fable")` substitute was used — it has caught
  real blockers in prior milestones).
- **Not a self-review.** The design doc was authored by a `gsd-executor` subagent (Opus) from plan
  26-01; the review was run by the orchestrator spawning a distinct Fable-5 agent with no authoring
  lineage; the findings were verified against live code and folded by the orchestrator. Author and
  reviewer are distinct agents/model families.
- **Code-traced, not prose-read.** The reviewer independently opened and traced
  `cli/caprun/src/main.rs`, `crates/adapter-fs/src/workspace.rs`, `crates/brokerd/src/server.rs`,
  `crates/brokerd/src/audit.rs`, `crates/brokerd/src/confirmation.rs`, `crates/brokerd/src/quarantine.rs`,
  `crates/executor/src/sink_sensitivity.rs`, `crates/executor/src/lib.rs`, `cli/caprun/src/planner.rs`,
  `cli/caprun/src/worker.rs`, both crates' `Cargo.toml`, and `scripts/check-invariants.sh`.
- **Findings independently re-verified by the orchestrator against live code before folding** (per the
  project's "verify each finding against actual code before fixing — AI reviewers generate false
  positives" discipline). All three findings were confirmed REAL (see per-finding code evidence);
  none was a false positive.
- **Effort:** 136k subagent tokens, 21 tool uses.

## Revision History

| Round | Date | Reviewer | Findings | Result |
|-------|------|----------|----------|--------|
| 1 | 2026-07-12 | Fresh independent Fable-5 agent (code-tracing) | 1 BLOCKER, 1 MAJOR, 1 MINOR (0 NIT) | All 3 folded as Round-1 amendments → CLEARED |

---

## Findings & resolutions

### F1 — BLOCKER → §b (RESOLVED)

**Claim.** The HARDEN-02 MAC key (`<audit_path>.key`) is reachable by the **confined worker** — the
in-scope primary adversary, not the D-05 root residual — whenever the operator co-locates the audit DB
under the workspace root. The doc pinned the safe key location as an assertion/operator convention with
no broker-derived guard, so its "fail-closed" key custody fails **open**.

**Code evidence (orchestrator re-verified).**
- `cli/caprun/src/main.rs:149` — `audit_path` is a free-form CLI arg (`caprun … <workspace-file>
  [audit-db-path]`), independent of the workspace root.
- `cli/caprun/src/main.rs:183` — the workspace root is derived separately as `ws_path.parent()`.
- `crates/adapter-fs/src/workspace.rs:90` — `read_within(rel_path) -> io::Result<File>` opens **any**
  path resolving `RESOLVE_BENEATH|RESOLVE_NO_SYMLINKS` the workspace root.
- `crates/brokerd/src/server.rs:996-1047` — the `RequestFd` arm calls `read_within` on the
  worker-supplied path and passes the resulting fd to the worker via `SCM_RIGHTS`, gated only by
  "beneath workspace root."
- Therefore `caprun … ./ws/intent.txt ./ws/audit.db` places `./ws/audit.db.key` beneath the root, and
  a confined worker can `RequestFd("audit.db.key")`, receive the fd, and read the MAC key.

**Resolution (folded, §b).** Pinned a **broker-enforced, fail-closed startup invariant**: the resolved
absolute paths of BOTH the audit DB and its `.key` sibling MUST lie outside the `WorkspaceRoot`'s
`read_within`-reachable set; the broker MUST refuse to run (hard error) if either resolves beneath the
workspace root. Now a broker guarantee, not an operator convention. Phase 28's shared key-load helper
implements and tests the refusal path. §b fail-closed default and §g probe 2 updated to match.

### F2 — MAJOR → §a (RESOLVED)

**Claim.** §a's trusted-label pin ("reuse the SAME `openat2` canonical form already computed for the
fd-open") is not implementable, and its ordering is self-contradictory.

**Code evidence (orchestrator re-verified).** `crates/adapter-fs/src/workspace.rs:90` — `read_within`
returns `std::io::Result<std::fs::File>` **only**; it discards the resolved/canonical path. `root_path()`
(`:71`) returns the root, not the per-file resolved target. So there is no "canonical form already
computed" to reuse — the only value left to a naive implementer is the raw worker-supplied `rel_path`
string (the ad-hoc compare the doc itself forbids). And "demote before `read_within` runs" cannot
coexist with "reuse `read_within`'s result."

**Resolution (folded, §a).** Replaced the pin with a broker-derived **`fstat` `(st_dev, st_ino)`
inode-identity** compare against the CLI-designated `<workspace-file>`, and corrected the ordering to:
open broker fd → `fstat` identity-compare → commit demotion if untrusted → **then** `pass_fd`. Noted as
additional Phase-27 plumbing beyond the `workspace_rel`-threading budget.

### F3 — MINOR → §f/X-04 (RESOLVED)

**Claim.** The X-04 fix (shared `Arc<Mutex<SessionStatus>>`) is sound only with an explicit
construct-once / monotonic-write constraint, else a Planner connection accepted after a demotion could
clobber `Draft` back to `Active` on init.

**Code evidence (orchestrator re-verified).** `server.rs:202`/`:231` each seed a per-connection
`initial_status` clone from `initial_session_status`; `server.rs:1131` is the only demotion write
(`Draft`), one-way. A shared cell re-seeded `Active` per connection would re-open the staleness.

**Resolution (folded, §f/X-04).** Pinned the shared cell as construct-once (seeded `Active` once at
`run_broker_server` entry) and monotonic `Active→Draft` only; no connection-setup path writes `Active`
after creation. Phase 27 adds a test that a Planner connection accepted after demotion observes `Draft`.

---

## Verified as sound (reviewer traced real code; not deferred trust)

The reviewer confirmed, by tracing live code, that the following load-bearing claims are accurate — this
is the evidence the review was a genuine code-trace, not a prose skim:

- **§b unkeyed chain + tail-truncation vuln:** `audit.rs` `compute_event_hash` is unkeyed SHA-256;
  `verify_chain` (`:477-539`) walks from `parent_id IS NULL` and returns `found_any` (≥1 row), so a tail
  `DELETE` terminates at the shorter true leaf and returns `true`. Truncation is currently undetectable;
  the anchored/`event_count` head is genuinely load-bearing.
- **§b second mutable table:** `pending_confirmations.state` is mutated by `transition_state`
  (`confirmation.rs:296-303`, `UPDATE … WHERE state='pending'`), has no MAC, and is outside the `events`
  chain — folding it into the MAC (not silently assuming covered) is correct.
- **§b D-06:** `blocked_literals` lives outside the hash (real `DELETE`); a whole-row MAC over
  `compute_event_hash`'s existing inputs preserves the redactability split.
- **§b key-custody-across-processes premise:** `confirm()` calls `verify_chain` at `confirmation.rs:599`
  in a separate later OS process — a per-process key would break it. (The *solution* was F1.)
- **§c effect_id freshness (the load-bearing fact):** `server.rs:562` mints `effect_id` fresh per call;
  the Allowed `email.send` block (`:791-866`) has no CAS; the residual is self-documented at `:848-854`.
  An `effect_id`-keyed CAS would do nothing. The SEND-01 mirror (`confirmation.rs:701-722`) exists and is
  proven; the content-derived key `SHA256(sink || sorted(name,value_id))` is stable across a literal
  replay and honestly scoped per-plan-node (D-08).
- **§d feature precedent:** `executor/Cargo.toml` has `[features] test-fixtures = []` + self
  dev-dependency; `brokerd/Cargo.toml` has no `[features]` — both exactly as claimed. The
  `#[cfg(test)]`-invisible-to-integration-tests rationale and the featureless-behavioral-negative-gate
  (option c) reasoning are correct; symbol-inspection correctly demoted to optional.
- **§e all three anchors:** `sink_sensitivity.rs:157` `"contents" => None`; `is_content_sensitive`
  (`:102-107`) matches only `email.send`; the inverting unit assertion at `:313-318` (doc cited
  `:313-317`, off by one — negligible). `planner.rs:208` binds `contents` to `intent_value_id` (the
  `"path"`-role handle); the intent literal is minted `origin_role: "path"`. `Some(&["path"])` is the
  only non-regressing value; both edits (a)+(b) genuinely required.
- **§f X-04 "real but un-exercised":** `executor/src/lib.rs` per-arg I2 Block returns
  `BlockedPendingConfirmation` before Step 0.5 (`:203-217`) ever reads `session_status`, so any
  tainted-through-sensitive-arg test masks the stale-status bug; the bug requires an all-clean-args plan
  node on a Draft session. Precedence verified in code.
- **§a "no other Draft-setter" reconciliation:** `quarantine.rs` claims `mint_from_read` is the SOLE I1
  trust-flip site — the doc correctly flags this comment must be corrected in the same Phase-27 PR;
  `fd_requested`'s broker-side flip at `server.rs:1001` is a genuine precedent for broker-side
  per-connection mutation at `RequestFd` entry.
- **check-invariants.sh gates:** Gate 1 (`EffectRequest` ban), Gate 3 (`mint_from_read(`/`.mint(`
  restricted to `quarantine`/`server`/`value_store`) confirmed; §a's reuse of `update_session_status`
  adds no new mint site, as the doc states.

---

## No-TCB-code reconfirmation (DESIGN-12 hard gate)

Re-verified at gate-clearance time (evidence in the phase 26-02 SUMMARY): `git status --porcelain
crates/ cli/` is empty (only `planning-docs/` and `.planning/` changed this phase); no new hardening
mechanism symbols exist (`Hmac`/`chain_anchor`/`sent_plan_nodes`/`is_trusted_labeled` appear only in the
DESIGN doc prose, not under `crates/`); `scripts/check-invariants.sh` is green. The five mechanisms
remain design-only until Phases 27–29 implement them.

---

## Verdict

**CLEAR-WITH-AMENDMENTS → CLEARED.** No load-bearing mechanism was fundamentally unsound (not a FAIL).
F1 was a genuine fail-open in HARDEN-02's headline mechanism, resolved by pinning a broker-enforced
guarantee rather than deferring to implementer discretion. F1/F2 hardened the *enforcement* of §a/§b;
no mechanism choice was weakened and no ruling was resolved by locking a mechanism out. Phases 27–30 are
authorized.
