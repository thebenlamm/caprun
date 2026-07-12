# DESIGN GATE RECORD — v1.5 (Slot-Type Binding Enforcement, T2)

**Milestone:** v1.5 — Slot-Type Binding Enforcement (Phase 23 design gate)
**Document under review:** `planning-docs/DESIGN-slot-type-binding.md`
**Gate purpose:** Authorize (or block) any `crates/executor` / `crates/brokerd` mint-site code for
this milestone. Mirrors `DESIGN-GATE-RECORD-v1.4.md` (and v1.2 Phase 8 / v1.3 Phase 12).
**Requirements gated:** DESIGN-07, DESIGN-08, DESIGN-09, DESIGN-10.

## Gate status: ✅ **CLEARED** (2026-07-11, Round 1)

Phase 24 (`crates/executor` expected-role table + `submit_plan_node` Step 1c; `crates/brokerd`
mint-site `origin_role` threading; `runtime_core::DenyReason::SlotTypeMismatch`) is authorized to
begin. All six review findings are resolved in the design doc as Round-1 amendments; no blocker
remains. **No TCB code was written during this design-gate phase** (re-confirmed below).

---

## Reviewer identity & independence

- **Mechanism:** a FRESH, INDEPENDENT adversarial reviewer spawned as a separate agent — a
  **Claude Fable 5** model (`claude-fable-5`), a different model family from the doc's authoring
  context. This satisfies the fresh-context requirement (`DEC-ai-review-satisfies-human-gate` +
  the project's `fresh-context-adversarial-review` discipline: a self-read is not sufficient; a
  self-read caught 0 of 5 blockers a fresh code-tracing reviewer caught in a prior milestone).
- **Not a self-review.** The design doc was authored inline by the orchestrator after three
  `gsd-executor` subagent runs died on a transient API error (see the phase SUMMARY deviation
  notes). The reviewer is a distinct agent/model with no authoring lineage.
- **Code-traced, not prose-read.** The reviewer independently opened and traced
  `value_record.rs`, `executor_decision.rs`, `sink_sensitivity.rs`, `lib.rs`, `quarantine.rs`,
  `proto.rs`, `server.rs`, and `worker.rs`, and re-ran the blast-radius grep itself (below).
- **Effort:** 271k subagent tokens, 19 tool uses.

## Revision History

| Round | Date | Reviewer | Findings | Result |
|-------|------|----------|----------|--------|
| 1 | 2026-07-11 | Fresh independent Fable-5 agent | 1 MAJOR, 3 MINOR, 2 NIT (0 BLOCKER) | All 6 folded as Round-1 amendments → CLEARED |

---

## Independent blast-radius re-verification (DESIGN-07b)

The reviewer re-ran `grep -rn "DenyReason" crates/ cli/ | grep -v /target/` (40 hits) and
classified every hit:

- **Exactly 2 EXHAUSTIVE matches over `DenyReason`**, both in
  `crates/runtime-core/src/executor_decision.rs`: `code()` (`:64-80`) and `Display::fmt`
  (`:83-112`). **Confirms the doc's claim.**
- All other hits are construction sites (`executor/src/lib.rs:85/98/107/180/205`,
  `sink_schema.rs:113-136`, `brokerd/src/lib.rs:80/91`), test-only matches with `other => panic!`
  catch-alls (`executor/tests/executor_decision.rs:371/580/637`), re-exports, or string literals.
- **`cli/caprun/src/worker.rs:370/381` uses `matches!(decision, ExecutorDecision::Allowed)` +
  Debug-format — NOT an exhaustive `DenyReason` match.** Confirms the doc.
- `grep -rn "ExecutorDecision::"`: the only non-test match statement is `server.rs:650-683`, which
  carries a `_ =>` wildcard arm (`:671`). Since the design reuses `Denied { reason }` (no new
  outer variant), **Assumption A3 is discharged: zero outer-enum match updates needed.**

The doc's blast-radius inventory is TRUE against live code, AND the doc correctly mandates Phase 24
re-run the grep rather than trust the count (§5 callout), with `check-invariants.sh`'s no-wildcard
discipline as the compile-time backstop.

---

## Per-Requirement Checklist

| Requirement | Verdict | Evidence |
|-------------|---------|----------|
| **DESIGN-07a** (tag mechanism) | ✅ PASS (nits F5/F6 folded) | `ValueRecord` confirmed as exactly `id/literal/taint/provenance_chain`, no existing role field (`value_record.rs`); side-table + `TaintLabel`-variant rejections sound (`is_untrusted()` no-wildcard match `plan_node.rs:40` would be polluted by a role variant); I0/I1 untouched. |
| **DESIGN-07b** (DenyReason shape + blast radius) | ✅ PASS (shape corrected per F1) | Blast radius = exactly 2 sites, independently grep-confirmed; A3 discharged. Variant field types corrected `&'static → Vec<String>` (F1). |
| **DESIGN-07c** (ordering) | ✅ PASS | `submit_plan_node` step order reproduced accurately (`lib.rs:66-68 / 78-158 / 81-88 / 96-100 / 105-109 / 117-157 / 162-164 / 176-213 / 215` all verified); Step 0.5 (I0) still gated on empty `blocked`; per-arg hard-Deny at 1c matches existing 1/1a/1b early-return discipline; hard-`Denied` over confirmable-Block correctly argued. |
| **DESIGN-08** (claim_type unification) | ✅ PASS (F3/F4 folded) | `claim_type` strings verified verbatim at `quarantine.rs:79/110/176`; unknown-claim_type fail-closed at `:336-340`; trusted-side membership neither over- nor under-broad. F3 (shared `:1317` call site) + F4 (untrusted-role⟹I2-sensitive invariant) folded. |
| **DESIGN-09** (derivation) | ✅ PASS (F2 folded) | No-inheritance/no-union from input roles correctly specified; anti-laundering contrast with taint-union (`quarantine.rs:603-613`) is real; `"recipient"` permits the legit concat flow while `to`-misroutes deny. F2 (Concat arity) folded. |
| **DESIGN-10** (fail-closed default) | ✅ PASS | Both failure shapes → `Denied`, no third path; `None`-vs-`Some(&[])` contract + `.unwrap_or(&[])` red flag pinned; `contents` carve-out explicit (§3 + §7.3 + A2). Cross-cutting sweep found no residual `Allowed` misroute for a `UserTrusted` value at any role-checked slot. |

**Locked-scope check:** no finding was resolved by designing a general role framework, changing
I0/I1 trust classification, or role-tagging a sink beyond `email.send`/`file.create`. The reviewer's
cross-cutting sweep found no scope violation.

---

## Findings & Disposition

| # | Sev | Finding | Evidence | Disposition |
|---|-----|---------|----------|-------------|
| F1 | MAJOR | `SlotTypeMismatch { expected: &'static [&'static str] }` cannot compile — `DenyReason` derives `Deserialize` and crosses the IPC wire; serde can't deserialize `&'static` refs | `executor_decision.rs:14` (derive), `worker.rs:370-381` (wire) | **Resolved** — §5 amended to `expected: Vec<String>`, populated from the static table at construction |
| F2 | MINOR | §4 "the only shape `Concat` produces is `local@domain`" overstated — `Concat` joins N inputs (1 = no `@`, 3+ = `a@b@c`); guaranteed email shape only for 2 inputs | `quarantine.rs:588-593` (only 0-input rejected), `:671-685` (join) | **Resolved** — §4 amended: Phase 24 enforces `inputs.len()==2` for `"concat"`, or relies on I2 (forced `WorkerExtracted`, `:611-613`) as residual cover for degenerate arities |
| F3 | MINOR | §2 recipient/path share ONE mint call site (`server.rs:1317` mints `primary_literal`), distinguished by the intent-variant match `:1294-1300` — hardcoding `"recipient"` at `:1317` would mistag legit paths (fail-closed regression) | `server.rs:1294-1300`, `:1317` | **Resolved** — §2 amended: role selected inside the intent-variant match and threaded, never a per-call-site constant |
| F4 | MINOR (load-bearing) | Untrusted-side `origin_role` is worker-influenced (worker picks `WorkerClaim` variant; `mint_from_read` shape-validates only `doc_fragment`) — safe ONLY because every slot listing an untrusted role is also I2-sensitive; that invariant was unstated | `proto.rs:24-43`, `quarantine.rs:327` | **Resolved** — §3 amended: pinned invariant "an untrusted-origin role may appear in a slot's expected list only if that slot is routing/content-sensitive" |
| F5 | NIT | §1 listed `TaintLabel` as 6 variants; live enum has 8 (`PdfRaw`, `LlmGenerated` omitted) — argument unaffected | `plan_node.rs:13-24` | **Resolved** — §1 corrected to 8 variants |
| F6 | NIT | Phase 24 should add `#[serde(default)]` to `origin_role` so pre-field records still deserialize | `ValueRecord` derives `Deserialize` | **Resolved** — §1 records the `#[serde(default)]` note for Phase 24 |

No BLOCKER findings. The reviewer's cross-cutting conclusion: "Nothing found undermines the
security design itself — the enforcement logic, ordering, fail-closed contract, and anti-laundering
rule all survived code tracing."

---

## How to Verify (human review steps)

1. **Independence:** confirm the reviewer was a fresh agent/model, not the authoring context (§
   "Reviewer identity" above; a different model family — Fable 5).
2. **Blast radius:** re-run `grep -rn "DenyReason" crates/ cli/ | grep -v /target/` and confirm
   exactly 2 exhaustive matches in `executor_decision.rs`.
3. **Amendments:** open `DESIGN-slot-type-binding.md` and confirm each F1–F6 amendment is present
   (search "DESIGN-GATE-RECORD-v1.5 Round 1") and that the load-bearing rulings are intact.
4. **Hard gate:** confirm no TCB code was written (below).

## Hard-gate re-confirmation (no TCB code this phase)

- `git status --porcelain crates/ cli/` → empty (verified at gate close).
- `./scripts/check-invariants.sh` → Gate 1 (no raw effect-to-sink type), Gate 2 (runtime-core
  purity), Gate 3 (mint-call-site restriction) all PASS.
- The only files changed this phase are `planning-docs/*.md` and `.planning/**` planning
  artifacts.

## Decision

**CLEARED.** The design is sound and complete against DESIGN-07/08/09/10; all findings are
resolved without weakening a ruling or violating a locked decision. Phase 24 may begin the
`crates/executor` / `crates/brokerd` mint-site change, and MUST re-confirm the `DenyReason`
blast-radius grep at implementation time (§5).
