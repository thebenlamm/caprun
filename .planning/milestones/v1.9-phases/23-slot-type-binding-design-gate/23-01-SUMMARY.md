---
phase: 23-slot-type-binding-design-gate
plan: 01
status: complete
wave: 1
requirements: [DESIGN-07, DESIGN-08, DESIGN-09, DESIGN-10]
---

# Plan 23-01 Summary — Author DESIGN-slot-type-binding.md

**Outcome:** `planning-docs/DESIGN-slot-type-binding.md` authored (440 lines, §0–§10 + Acceptance
Predicate + Amendments stub), pinning every ruling DESIGN-07/08/09/10 require. Design-gate
discipline held: `git status --porcelain crates/ cli/` empty, all 3 `check-invariants.sh` gates
green. This is the structural half AND the ordering/blast-radius/fail-closed half (both plan
tasks' content) in one authoring pass.

## What was decided (pinned rulings)

- **DESIGN-07a — tag mechanism:** additive `origin_role: Option<String>` on `ValueRecord`
  (`value_record.rs:21-31`); side-table and `TaintLabel`-variant alternatives rejected in writing;
  I0/I1 trust classification explicitly UNAFFECTED (§1).
- **DESIGN-08 — claim_type unification:** untrusted-origin values reuse the existing `claim_type`
  strings (`"email_address"`/`"relative_path"`/`"doc_fragment"`) verbatim as the role tag (they
  are currently derived at `mint_from_read` then discarded — that discard is the gap closed);
  from-scratch tags `"recipient"`/`"subject"`/`"body"`/`"path"` for `ProvideIntent`-minted
  `UserTrusted` values, keyed to the `server.rs` call sites; dual-vocabulary tradeoff named (§2).
- **Expected-role table (T2-03 anticipation):** hardcoded `expected_role(sink, arg) ->
  Option<&'static [&'static str]>` mirroring `sink_sensitivity.rs`; exact per-slot lists pinned
  (`to/cc/bcc => ["recipient","email_address"]`, etc.); `contents` unconstrained (Assumption A2);
  `None`=unconstrained vs `Some(&[])`-never-constructed contract pinned (§3).
- **DESIGN-09 — derivation:** `Concat` output role hardcoded `"recipient"` (function of
  `transform_kind` only, grounded in the byte-verified `local@domain` output shape); NEVER
  inherited/unioned from input fragment roles (anti-laundering) (§4).
- **DESIGN-07b — DenyReason blast radius:** new `DenyReason::SlotTypeMismatch`; blast radius
  exactly 2 exhaustive matches (`code()` + `Display::fmt` in `executor_decision.rs`), grep
  inventory reproduced, `worker.rs` `matches!`/Debug noted as needing no update — explicitly
  flagged as a claim Phase 24 MUST re-confirm via grep, with `check-invariants.sh` no-wildcard as
  the compile-time backstop (§5).
- **DESIGN-07c — ordering:** hard `Denied` via a per-arg "Step 1c" guard between 1b and 2/3, NOT
  `BlockedPendingConfirmation` (a misrouted role is structurally non-confirmable); full
  `submit_plan_node` step order reproduced; Step 0.5 (I0) untouched, I2-before-I0 precedence
  preserved (§6).
- **DESIGN-10 — fail-closed default:** both failure shapes (`None` role at a role-checked slot;
  role ∉ list) → `Denied`; unconstrained-slot carve-out documented as intentional, not fail-open
  (§7).
- **Adversarial preemption (§8):** all 6 reviewer angles pre-answered with pointers to the
  resolving section. **Phase 24/25 map (§9)**, **residual risks + assumptions A1/A2/A3 + pitfalls
  (§10)**, **Done-When acceptance predicate**.

## key-files

created:
- `planning-docs/DESIGN-slot-type-binding.md`
- `.planning/phases/23-slot-type-binding-design-gate/23-01-SUMMARY.md`

## Verification

- Plan Task 1 + Task 2 automated grep gates: both PASS.
- `git status --porcelain crates/ cli/`: empty (HARD GATE — no TCB code).
- `./scripts/check-invariants.sh`: Gate 1/2/3 all PASS.
- All four requirement IDs (DESIGN-07/08/09/10) cited and resolved in the doc.

## Deviations

- **Authored inline by the orchestrator, not a `gsd-executor` subagent.** Three consecutive
  `gsd-executor`/researcher subagent runs died mid-response on a transient "Connection closed
  mid-response" API error (the same flakiness hit the phase researcher, which only succeeded when
  re-spawned fresh). Per the no-whack-a-mole rule, the orchestrator authored the doc inline with
  incremental Write/Edit calls (controlling response size to dodge the drop), faithful to the
  plan's two tasks and both acceptance gates. Gate integrity is preserved because Plan 23-02's
  fresh non-self adversarial review is a different context from this author.

## Self-Check: PASSED

Next: Plan 23-02 runs the fresh (non-self) adversarial gate against this doc and the cited code.
