---
phase: 14-content-sensitive-sink-arg-blocking
verified: 2026-07-08T04:24:21Z
status: passed
score: 12/12 must-haves verified
behavior_unverified: 0
overrides_applied: 0
---

# Phase 14: Content-Sensitive Sink-Arg Blocking Verification Report

**Phase Goal:** Make `ExecutorDecision::BlockedPendingConfirmation`/`SinkBlockedAnchor` PLURAL and implement collect-then-Block: the executor scans ALL args of a sink call before returning a decision, collecting every content-sensitive-and-tainted arg into one combined Block. Extend content-sensitivity blocking to `body`/`subject`. Descope `attachment` (D-23).
**Verified:** 2026-07-08T04:24:21Z
**Status:** passed
**Re-verification:** No — initial verification

All claims were re-derived independently from source (`git diff` not relied on) and from a fresh `cargo build --workspace` / `cargo test --workspace --no-fail-fast` / `./scripts/check-invariants.sh` run on current `main` HEAD (`16e4d05`). SUMMARY.md files were read only after independent verification, to cross-check, never as evidence.

## Goal Achievement

### Observable Truths (14-01 + 14-02 must_haves, merged)

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | Tainted `body` on email.send Blocks (CONTENT-01) | VERIFIED | `crates/executor/src/lib.rs:117-118` — `sensitive = is_routing_sensitive(..) \|\| is_content_sensitive(..)`; `EMAIL_SEND_CONTENT_SENSITIVE = &["subject","body"]` (`sink_sensitivity.rs:78`). Test `tainted_body_blocks` passes. |
| 2 | Tainted `to` AND tainted `body` surface BOTH in ONE decision (D-14, no first-match-wins) | VERIFIED | `crates/executor/tests/executor_decision.rs:438-485` `collect_then_block_both_to_and_body`: asserts `anchors.len()==2`, both `to`/`body` arg names present, in plan-node order. Ran standalone: `cargo test -p executor collect_then_block_both_to_and_body` → ok. Read the assertion body myself — it genuinely checks len==2 and both names, not a weaker check. |
| 3 | Tainted `body` + TRUSTED `to` STILL Blocks (CONTROL-02 precursor — proves body isn't dead code) | VERIFIED | `executor_decision.rs:491-536` `body_tainted_recipient_trusted_blocks`: `to` minted `UserTrusted`, `body` minted `ExternalUntrusted`; asserts `anchors.len()==1` and `anchors[0].anchor.arg == "body"`. Ran standalone → ok. |
| 4 | `attachment` arg is Denied(UnknownArg) at Step 0 schema gate (D-23) | VERIFIED | `sink_schema.rs:50` `allowed: &["to","cc","bcc","subject","body"]` (no attachment); `sink_sensitivity.rs:78` content-sensitive set also drops it. Test `attachment_denied_unknown_arg` passes. |
| 5 | Content-sensitivity stays ONE hardcoded match arm scoped to email.send only (CONTENT-02) | VERIFIED | `sink_sensitivity.rs:102-107` `is_content_sensitive` — single match on `"email.send"`, `_ => false`. Test `unknown_sink_not_content_sensitive` passes. |
| 6 | Step 0.5 (draft-only CommitIrreversible deny) runs ONLY after per-arg loop completes with no Block (D-15 ordering) | VERIFIED | `lib.rs:160-176` — the `match *session_status` block is textually and logically after the `if !blocked.is_empty() { return ... }` early return; the collect loop (lines 78-158) runs to completion first. |
| 7 | Every blocked-arg element is a verbatim clone (anti-stapling, T-04-03), preserved per-element in the plural shape | VERIFIED | `lib.rs:130-156` — `SinkBlockedAnchor`/`BlockedArg` fields all copied from `record`/`plan_node`/`arg`; `grep -c 'ValueStore::mint'`/`'ValueRecord {'` in lib.rs (excluding comments) = 0 (checked). |
| 8 | `sink_blocked` audit Event carries ALL blocked anchors (`anchors: Vec<SinkBlockedAnchor>`) | VERIFIED | `crates/runtime-core/src/event.rs:44` `pub anchors: Vec<SinkBlockedAnchor>`; `Event::sink_blocked` takes `Vec<SinkBlockedAnchor>` and merges taint via `flat_map`. |
| 9 | Non-`sink_blocked` events serialize byte-identically (golden-byte fixture, `skip_serializing_if`) | VERIFIED | `event.rs:43` `#[serde(default, skip_serializing_if = "Vec::is_empty")]`; test `anchors_empty_event_serializes_byte_identical_and_round_trips` asserts exact `GOLDEN` string match with no `"anchors"` key, and round-trip via `#[serde(default)]`. Ran standalone → ok. |
| 10 | `audit.rs` fails closed on empty `anchors` (Defect-B guard, now plural) | VERIFIED | `crates/brokerd/src/audit.rs:224` `if event.event_type == "sink_blocked" && event.anchors.is_empty()` → error "sink_blocked event requires at least one anchor (Defect B guard)". |
| 11 | `server.rs` constructs plural `sink_blocked` event + `PendingConfirmation` from plural decision; `resolved_args` snapshot unchanged | VERIFIED | `crates/brokerd/src/server.rs:421-491` — destructures `{ anchors }`, builds `Event::sink_blocked(.., anchors.iter().map(|b| b.anchor.clone()).collect())`, writes every `(arg, literal)` pair to `blocked_literals`, sets `PendingConfirmation.effect_id = anchors[0].anchor.effect_id`; the `resolved_args` loop iterates `plan_node.args` (unchanged). |
| 12 | `cargo test --workspace --no-fail-fast` GREEN + `check-invariants.sh` GREEN | VERIFIED | Ran both myself (see below) — 0 failed across all 34 test binaries; Gate 1 + Gate 2 PASS. |

**Score:** 12/12 truths verified (0 present-but-behavior-unverified)

### Specific Checks Required by Coordinator (hard gates)

**Gate 1 — plural collect-then-Block, both directions:**
- `collect_then_block_both_to_and_body` (executor_decision.rs:438-485): independently read the assertion body — `assert_eq!(anchors.len(), 2, ...)`, checks both `"to"` and `"body"` present, and asserts stable order `["to","body"]`. This genuinely proves the composition case, not a weaker stand-in. **Ran standalone, PASS.**
- `body_tainted_recipient_trusted_blocks` (executor_decision.rs:491-536): `to` minted with `TaintLabel::UserTrusted` (not blocking), `body` minted `ExternalUntrusted`; asserts decision is `BlockedPendingConfirmation` with exactly 1 anchor named `"body"`. This genuinely proves the body dimension is live, not routing-redundant. **Ran standalone, PASS.**
- **Gate 1: PASS.**

**Gate 2 — no regression:**
- `s9_acceptance` (crates/brokerd/tests/s9_acceptance.rs): ran standalone by exact name (`cargo test -p brokerd --test s9_acceptance s9_acceptance --exact`) → `1 passed; 0 failed`.
- `file.create`'s dispatch arm in `confirmation.rs:444` (`"file.create" => match crate::sinks::file_create::invoke_file_create_from_resolved(...)`) is untouched by the plural migration — confirmed by reading the surrounding dispatch code; `render_block_display`'s single-arg path (unaffected by the anchor-type change, operates on `resolved_args`) is unchanged.
- `durable_anchor.rs` (4/4 passed) and `phase5_dispatch.rs` (6/6 passed) both green in the full workspace run — genuine-taint assertions intact (`blocked.anchors.first()` migration pattern, not a weakened check).
- **Gate 2: PASS.**

**Additional independently-confirmed items:**
- `cargo build --workspace`: clean, 0 errors.
- `cargo test --workspace --no-fail-fast`: 0 failed across all 34 test binaries (full output captured; grepped for `FAILED`/`error[` — zero matches).
- `./scripts/check-invariants.sh`: Gate 1 (no `EffectRequest` token) PASS; Gate 2 (runtime-core purity) PASS.
- **Exactly 7** `Event::sink_blocked(` call sites project-wide (grep-confirmed): `email_smtp_acceptance.rs:207`, `s9_acceptance.rs:168`, `server.rs:423`, `confirmation.rs:735`, `confirmation.rs:970`, `confirm.rs:88`, `confirm.rs:359` — all 7 pass a `vec![...]`/collection literal, none passes a bare `SinkBlockedAnchor`. Zero singular `BlockedPendingConfirmation { anchor, literal }` / `{ anchor, ..}` destructures survive anywhere (`grep -rn` both patterns → no matches).
- `attachment` genuinely descoped: `grep -c 'attachment' crates/executor/src/sink_sensitivity.rs` and `sink_schema.rs` both 0; proven by passing test `attachment_denied_unknown_arg`.
- `Event.anchors` serialization: `#[serde(default, skip_serializing_if = "Vec::is_empty")]` on `event.rs:43`; golden-byte test proves an empty-anchors event omits the key entirely and round-trips via `#[serde(default)]`.

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `crates/runtime-core/src/executor_decision.rs` | `BlockedArg` struct + plural `BlockedPendingConfirmation{anchors}` | VERIFIED | Present, matches spec exactly (lines 148-179), doc references Phase 16 combined_digest without adding the field. |
| `crates/executor/src/lib.rs` | Unified collect-then-Block loop | VERIFIED | Single `for arg in &plan_node.args` loop (line 78), block returns after loop (line 163), Step 0.5 after (line 176). |
| `crates/executor/src/sink_sensitivity.rs` | `EMAIL_SEND_CONTENT_SENSITIVE = [subject, body]` | VERIFIED | Line 78, attachment absent. |
| `crates/executor/src/sink_schema.rs` | `allowed = [to,cc,bcc,subject,body]` | VERIFIED | Line 50, attachment absent. |
| `crates/runtime-core/src/event.rs` | `Event.anchors: Vec<SinkBlockedAnchor>` + skip_serializing_if | VERIFIED | Lines 43-44. |
| `crates/brokerd/src/server.rs`, `audit.rs`, `confirmation.rs` | plural-aware handling | VERIFIED | All confirmed by direct read. |
| Proof tests (4 new + rewrites) | present and passing | VERIFIED | All named tests found and pass standalone. |

### Key Link Verification

| From | To | Via | Status |
|------|-----|-----|--------|
| `is_content_sensitive` (email.send) | Collect loop → plural `BlockedPendingConfirmation.anchors` | `lib.rs:117-163` | WIRED |
| attachment removed from sensitivity set AND schema set | atomic | Both files confirmed 0 occurrences | WIRED |
| Per-arg loop completes | Step 0.5 | Only then `Allowed` | `lib.rs:160-202` ordering confirmed | WIRED |
| `ExecutorDecision::BlockedPendingConfirmation.anchors` | `server.rs` | `Event::sink_blocked(anchors)` → `audit.rs` empty-check | `server.rs:421-429`, `audit.rs:224` | WIRED |
| `skip_serializing_if = Vec::is_empty` | golden bytes | Preserved for non-block events | Golden-byte test passes | WIRED |
| Every anchor shares one `effect_id` | `PendingConfirmation.effect_id` | `= anchors[0].anchor.effect_id` | `server.rs:481` | WIRED |

### Requirements Coverage

| Requirement | Source Plan | Status | Evidence |
|-------------|------------|--------|----------|
| CONTENT-01 | 14-01, 14-02 | SATISFIED | Body block + plural durability, both proven by passing tests. |
| CONTENT-02 | 14-01 | SATISFIED (code) — REQUIREMENTS.md tracking stale | `is_content_sensitive` is one hardcoded match arm scoped to `email.send`; `unknown_sink_not_content_sensitive` proves it. `.planning/REQUIREMENTS.md` line 20/87 still shows CONTENT-02 as `[ ] Pending` — this is a documentation-tracking gap, not a functional gap. Recommend updating the checkbox/table in a follow-up doc commit. |

### Anti-Patterns Found

None. Scanned all files touched by 14-01/14-02 for `TBD|FIXME|XXX|TODO|HACK|PLACEHOLDER`/stub patterns — the only hit was a pre-existing, unrelated `NotImplemented` doc comment in `executor_decision.rs:182` ("Stub: executor not yet implemented (Phase 1 return value)") describing an enum variant untouched by this phase.

### Informational Note (not a gap — explicitly accepted interim risk)

`render_block_display`'s fail-closed `assert!` for a genuinely-plural block (`confirmation.rs:299-304`) is not exercised by any test in this phase — no test constructs a real 2-blocked-arg `PendingConfirmation` and calls `render_block_display` against it. This matches the phase's own threat model (`T-14-08`, disposition: "accept (interim)") and was honestly flagged by 14-02-SUMMARY.md itself. It is not a declared must-have truth/artifact for Phase 14 (multi-arg narration is explicitly Phase 16 / CONFIRM-04 scope), so it does not gate this phase, but Phase 16 should add a test that exercises the panic path before replacing it with real narration.

### Human Verification Required

None. All must-haves resolved to VERIFIED via direct code inspection plus fresh, independently-run test execution (no reliance on SUMMARY.md claims).

### Gaps Summary

No gaps. Both coordinator-flagged hard gates pass on independent re-verification: the collect-then-Block composition test and the trusted-recipient/tainted-body test both assert exactly what they claim and both pass standalone. No regressions in `s9_acceptance`, `file.create`'s dispatch arm, or the audit-DAG genuine-taint tests (`durable_anchor.rs`, `phase5_dispatch.rs`). Full workspace build/test/invariants are green. `attachment` is genuinely descoped from both the schema and sensitivity sets. `Event.anchors` golden-byte compatibility is preserved. Exactly 7 `Event::sink_blocked(` call sites exist, all vec-wrapped.

One informational item noted above (REQUIREMENTS.md CONTENT-02 checkbox stale relative to code) — recommend a follow-up doc-only commit, not a phase blocker.

---

_Verified: 2026-07-08T04:24:21Z_
_Verifier: Claude (gsd-verifier)_
