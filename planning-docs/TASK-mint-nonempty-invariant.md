# TASK — Enforce non-empty taint + provenance at `ValueStore::mint`

> ✅ **SHIPPED — landed as Phase 7 plan 07-01 (Wave 1), 2026-07-01.** `ValueStore::mint` now returns `Result` and rejects empty taint/provenance; the typed `DenyReason` base enum was introduced here; the executor empty-taint/empty-provenance guards were moved up (before the sensitivity check — closes codex #5's `[UserTrusted]`+empty hole); the `:108` allow-shape fix was applied. Original task status retained below.

**Status:** READY (follow-up). **Execute AFTER phase 06 completes** (avoids collision with the live GSD window — 06-04/06-05 are running). Small, additive, self-contained; one atomic commit.
**Source:** board-ratified in `DESIGN-durable-anchor-and-label-partition.md` §3 (codex #4/#5 + matt's "enforce at the mint site, don't just assert" hardening).
**Owner decision:** carved out of the review as a standalone follow-up (Option A) rather than interrupting the running phase.

## Why

Make the mint invariant **true at the source**, not merely asserted. Every `ValueRecord` must carry ≥1 taint label and ≥1 provenance event id. Today `ValueStore::mint` (`crates/executor/src/value_store.rs:42`) accepts `vec![]` for both, so the invariant holds only by convention. This is the eliminate-vs-not-triggered fix (matt #8-style): an empty-taint / empty-provenance value becomes **unconstructable through the sanctioned mint path**.

**No live behavior change** — both production mint sites already pass non-empty values:
- `mint_from_read` → `store.mint(claim.value, [ExternalUntrusted, EmailRaw], [event_id])` (`crates/brokerd/src/quarantine.rs:161`).
- `mint_from_intent` → `store.mint(literal, [UserTrusted], [event_id])` (`crates/brokerd/src/quarantine.rs:233`, wired at `server.rs:374`).

## Scope

**This task = the mint-site enforcement ONLY.** The executor defense-in-depth guard (empty → `Denied`, moved above the sensitivity/trust check) **and** the typed `DenyReason` enum are **DEFERRED to Phase 7** — they need the `BlockedPendingConfirmation`/`ExecutorDecision` reshape (REV.2 §4/§6), so adding them now with a throwaway `Denied{reason:String}` would be churn. Cross-ref REV.2 §3/§8. Rationale for the split: once `mint` rejects empty, an empty-taint record cannot exist, so the executor guard is pure defense-in-depth and safely rides with the Phase-7 anchor work.

## Preconditions

- Phase 06 complete (`mint_from_intent` shipped — it is: `quarantine.rs:202`).
- **Re-grep `\.mint(` against final HEAD first** — 06-05 may add a caller. As of this writing the callers are the two live sites above + test call sites (below).

## The change (RED-first)

**1. RED — add failing tests** (`crates/executor/src/value_store.rs` test mod):
- `mint(literal, vec![], vec![id])` → `Err` (empty taint).
- `mint(literal, vec![UserTrusted], vec![])` → `Err` (empty provenance).
- `mint(literal, vec![UserTrusted], vec![id])` → `Ok`.
These fail to compile today (`mint` returns `ValueId`, not `Result`).

**2. GREEN — change the signature + add the checks:**
```rust
// crates/executor/src/value_store.rs
#[derive(Debug, Clone, PartialEq)]
pub enum MintInvariantError { EmptyTaint, EmptyProvenance }   // TCB-local typed error

pub fn mint(&mut self, literal: String, taint: Vec<TaintLabel>, provenance_chain: Vec<Uuid>)
    -> Result<ValueId, MintInvariantError>
{
    if taint.is_empty()            { return Err(MintInvariantError::EmptyTaint); }
    if provenance_chain.is_empty() { return Err(MintInvariantError::EmptyProvenance); }
    let id = ValueId::new();
    self.inner.insert(id.clone(), ValueRecord { id: id.clone(), literal, taint, provenance_chain });
    Ok(id)
}
```

**3. Fix the ripple** (every `.mint(` call site now handles a `Result`):
- **Live (propagate with `?`):** `quarantine.rs:161` (`mint_from_read`) and `quarantine.rs:233` (`mint_from_intent`) — both already return `anyhow::Result`, so `let value_id = store.mint(...)?;` (map the typed error into `anyhow` with context). No behavior change (both pass non-empty).
- **Tests (`.expect(...)`):** `crates/executor/src/value_store.rs:85`, and the valid-mint calls in `crates/executor/tests/executor_decision.rs` (`:65,152,178,183,204,234,260`) become `.expect("valid mint")`.
- **⚠️ The one semantic test-collision — `crates/executor/tests/executor_decision.rs:108`:** `store.mint("boss@company.com", vec![], vec![])` currently uses **empty taint to mean "clean/allow."** The invariant forbids that, and it's semantically wrong post-HARD-02: "allow" is a **non-empty, all-trusted** value, not an empty one. **Change it to `store.mint("boss@company.com", vec![TaintLabel::UserTrusted], vec![event_id]).expect(...)`** — the arg still reaches `Allowed` (no untrusted label), and the test now exercises the real allow-path shape. (Phase 6 likely already has an equivalent `[UserTrusted]`→Allowed test in `runtime-core/tests/intent_taint.rs`; keep both.)

## Done-when

- [ ] `ValueStore::mint` returns `Err` for empty taint OR empty provenance; `Ok` otherwise (3 unit tests).
- [ ] `mint_from_read` / `mint_from_intent` propagate the error (no `unwrap` on the invariant).
- [ ] `executor_decision.rs:108` allow-test uses `[UserTrusted]` + a real event id, still → `Allowed`.
- [ ] `cargo build --workspace` + `cargo test --workspace` green on macOS.
- [ ] No behavior change on live paths (both real mint sites already non-empty).

## Files

`crates/executor/src/value_store.rs` (+ its test mod), `crates/brokerd/src/quarantine.rs` (two live callers), `crates/executor/tests/executor_decision.rs` (Result `.expect()` + the `:108` allow-shape fix). All cross-platform — **no Linux gate**. Blast radius: one TCB signature change, 2 live callers, ~9 test call sites. Land as its own atomic commit (`feat: enforce non-empty taint+provenance at ValueStore::mint`).

## Explicitly NOT in this task (→ Phase 7, REV.2 §4/§6)

Executor empty→`Denied` guard (moved above sensitivity), typed `DenyReason` enum, the durable `SinkBlockedAnchor`, and the `BlockedPendingConfirmation` reshape.
